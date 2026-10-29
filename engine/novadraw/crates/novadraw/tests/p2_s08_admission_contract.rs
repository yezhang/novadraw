use novadraw::{
    figure::{
        Figure, RectangleFigure, ShapeGeometryError,
        border::{
            Border, BorderInputError, BorderMetrics, BorderSide, BorderSnapshot, CompoundBorder,
            LineBorder, MarginBorder, TitleBarBorder,
        },
    },
    geometry::{Dimension, Insets, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::{
        CapabilityUpdateError, FramePreparation, ReplaceFigureBorder, Runtime, RuntimeMutationError,
    },
    text::{FontDescriptor, TextError, shaping::TextLayoutEngine},
    tree::{FigureSubtree, FigureTree, GraphMutationError},
};
use std::error::Error;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct Initial {
    bounds: Rectangle,
    insets: Insets,
    calls: Arc<[AtomicUsize; 3]>,
}
impl Figure for Initial {
    fn name(&self) -> &'static str {
        "external initial"
    }
    fn initial_bounds(&self) -> Rectangle {
        self.calls[0].fetch_add(1, Ordering::Relaxed);
        self.bounds
    }
    fn initial_insets(&self) -> Insets {
        self.calls[1].fetch_add(1, Ordering::Relaxed);
        self.insets
    }
    fn initial_style(&self) -> novadraw::figure::FigureStyle {
        self.calls[2].fetch_add(1, Ordering::Relaxed);
        Default::default()
    }
}
fn initial(bounds: Rectangle, insets: Insets) -> (Initial, Arc<[AtomicUsize; 3]>) {
    let calls = Arc::new(std::array::from_fn(|_| AtomicUsize::new(0)));
    (
        Initial {
            bounds,
            insets,
            calls: calls.clone(),
        },
        calls,
    )
}
fn root(runtime: &mut Runtime) -> novadraw::tree::FigureId {
    runtime
        .set_contents(Box::new(
            RectangleFigure::new(0.0, 0.0, 120.0, 80.0).expect("valid figure bounds"),
        ))
        .unwrap()
}
fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 120.0,
        logical_height: 80.0,
        pixel_width: 120,
        pixel_height: 80,
        scale_factor: 1.0,
    }
}
fn settle(runtime: &mut Runtime) {
    match runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
    {
        FramePreparation::Ready(frame) => {
            assert_eq!(
                runtime
                    .frames()
                    .complete_submission(
                        frame.session_id,
                        frame.frame_id,
                        Ok(RenderDisposition::Presented)
                    )
                    .unwrap(),
                novadraw::runtime::SubmissionAcknowledgement::Presented
            );
        }
        FramePreparation::Idle => {}
        other => panic!("unexpected frame: {other:?}"),
    }
}

#[test]
fn initial_candidates_are_captured_once_and_legal_boundaries_are_preserved() {
    let bounds = Rectangle::new(-10.0, -20.0, 0.0, 0.0);
    let insets = Insets::new(0.0, 100.0, 30.0, 0.0);
    let (figure, calls) = initial(bounds, insets);
    let mut tree = FigureTree::new();
    let id = tree.builder().set_contents(Box::new(figure)).unwrap();
    assert_eq!(tree.figure_bounds(id).unwrap(), bounds);
    assert_eq!(tree.insets(id).unwrap(), insets);
    for calls in calls.iter() {
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn bounds_extent_overflow_is_rejected_before_publication_with_geometry_source() {
    let mut runtime = Runtime::empty();
    let old = root(&mut runtime);
    settle(&mut runtime);
    let (figure, calls) = initial(Rectangle::new(f64::MAX, 0.0, f64::MAX, 1.0), Insets::ZERO);
    let error = runtime.set_contents(Box::new(figure)).unwrap_err();
    assert!(matches!(
        error,
        RuntimeMutationError::Graph(GraphMutationError::InvalidInitialBounds(
            ShapeGeometryError::NonFinite
        ))
    ));
    assert!(error.source().unwrap().source().is_some());
    assert_eq!(runtime.tree().contents(), Some(old));
    assert!(!runtime.frames().has_pending_update());
    for calls in calls.iter() {
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn each_external_inset_and_combination_is_rejected_by_builder_runtime_and_subtree() {
    for (insets, expected) in [
        (
            Insets::new(f64::NAN, 0.0, 0.0, 0.0),
            BorderInputError::NonFinite {
                field: "inset",
                side: Some(BorderSide::Top),
            },
        ),
        (
            Insets::new(0.0, -1.0, 0.0, 0.0),
            BorderInputError::Negative {
                field: "inset",
                side: Some(BorderSide::Left),
            },
        ),
        (
            Insets::new(0.0, 0.0, f64::INFINITY, 0.0),
            BorderInputError::NonFinite {
                field: "inset",
                side: Some(BorderSide::Bottom),
            },
        ),
        (
            Insets::new(0.0, 0.0, 0.0, -1.0),
            BorderInputError::Negative {
                field: "inset",
                side: Some(BorderSide::Right),
            },
        ),
        (
            Insets::new(f64::MAX, 0.0, f64::MAX, 0.0),
            BorderInputError::Overflow {
                field: "vertical insets",
            },
        ),
        (
            Insets::new(0.0, f64::MAX, 0.0, f64::MAX),
            BorderInputError::Overflow {
                field: "horizontal insets",
            },
        ),
    ] {
        let mut tree = FigureTree::new();
        let (figure, _) = initial(Rectangle::ZERO, insets);
        assert_eq!(
            tree.builder().set_contents(Box::new(figure)),
            Err(GraphMutationError::InvalidInitialInsets(expected.clone()))
        );
        assert_eq!(tree.contents(), None);
        let mut runtime = Runtime::empty();
        let parent = root(&mut runtime);
        settle(&mut runtime);
        let revision = runtime.tree().component_revision(parent).unwrap();
        let (figure, _) = initial(Rectangle::ZERO, insets);
        assert_eq!(
            runtime.figure(parent).unwrap().add(Box::new(figure)),
            Err(RuntimeMutationError::Graph(
                GraphMutationError::InvalidInitialInsets(expected.clone())
            ))
        );
        let (figure, _) = initial(Rectangle::ZERO, insets);
        let subtree = FigureSubtree::new(Box::new(figure));
        assert!(runtime.set_contents_subtree(subtree).is_err());
        assert_eq!(runtime.tree().contents(), Some(parent));
        assert_eq!(runtime.tree().component_revision(parent).unwrap(), revision);
        assert!(runtime.tree().child_order(parent).unwrap().is_empty());
        assert!(!runtime.frames().has_pending_update());
    }
}

#[derive(Clone)]
struct ChangingBorder {
    values: Arc<Mutex<(Insets, Dimension)>>,
    calls: Arc<[AtomicUsize; 2]>,
}
impl Border for ChangingBorder {
    fn get_insets(&self) -> Insets {
        self.calls[0].fetch_add(1, Ordering::Relaxed);
        self.values.lock().unwrap().0
    }
    fn preferred_size(&self) -> Dimension {
        self.calls[1].fetch_add(1, Ordering::Relaxed);
        self.values.lock().unwrap().1
    }
    fn paint(&self, _: Rectangle, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}
fn changing() -> ChangingBorder {
    ChangingBorder {
        values: Arc::new(Mutex::new((Insets::uniform(1.0), Dimension::ZERO))),
        calls: Arc::new(std::array::from_fn(|_| AtomicUsize::new(0))),
    }
}

#[test]
fn checked_metrics_cannot_be_created_with_invalid_values() {
    assert!(BorderMetrics::new(Insets::uniform(-1.0), Dimension::ZERO).is_err());
    assert!(BorderMetrics::new(Insets::ZERO, Dimension::new(f64::NAN, 0.0)).is_err());
}

#[test]
fn replacement_installs_one_capture_and_rejection_preserves_published_state() {
    let mut runtime = Runtime::empty();
    let id = root(&mut runtime);
    let border = changing();
    let shared: Arc<dyn Border> = Arc::new(border.clone());
    runtime
        .figure(id)
        .unwrap()
        .update_capability(ReplaceFigureBorder::new(Some(shared.clone())))
        .unwrap();
    assert_eq!(border.calls[0].load(Ordering::Relaxed), 1);
    assert_eq!(border.calls[1].load(Ordering::Relaxed), 1);
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::uniform(1.0));
    settle(&mut runtime);
    let revision = runtime.tree().component_revision(id).unwrap();
    *border.values.lock().unwrap() = (Insets::new(0.0, f64::MAX, 0.0, f64::MAX), Dimension::ZERO);
    assert_eq!(
        runtime
            .figure(id)
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(shared.clone()))),
        Err(CapabilityUpdateError::Rejected(
            BorderInputError::Overflow {
                field: "horizontal insets"
            }
        ))
    );
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::uniform(1.0));
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert!(!runtime.frames().has_pending_update());
    *border.values.lock().unwrap() = (Insets::uniform(1.0), Dimension::ZERO);
    assert!(
        !runtime
            .figure(id)
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(shared)))
            .unwrap()
            .changed
    );
    runtime
        .figure(id)
        .unwrap()
        .update_capability(ReplaceFigureBorder::set(
            LineBorder::new(Color::RED, 2.0).unwrap(),
        ))
        .unwrap();
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::uniform(2.0));
    settle(&mut runtime);
}

#[test]
fn nested_external_metrics_are_rechecked_after_successful_construction() {
    let child = changing();
    let inner = CompoundBorder::new(MarginBorder::default(), child.clone()).unwrap();
    let outer = CompoundBorder::new(MarginBorder::default(), inner).unwrap();
    *child.values.lock().unwrap() = (Insets::ZERO, Dimension::new(-1.0, 0.0));
    let mut runtime = Runtime::empty();
    let id = root(&mut runtime);
    settle(&mut runtime);
    let error = runtime
        .figure(id)
        .unwrap()
        .update_capability(ReplaceFigureBorder::set(outer))
        .unwrap_err();
    let CapabilityUpdateError::Rejected(BorderInputError::Child {
        role: "inner",
        source,
    }) = error
    else {
        panic!("lost outer child path");
    };
    let BorderInputError::Child {
        role: "inner",
        source,
    } = *source
    else {
        panic!("lost nested child path");
    };
    assert_eq!(
        *source,
        BorderInputError::Negative {
            field: "preferred width",
            side: None
        }
    );
    assert!(!runtime.frames().has_pending_update());
}

#[test]
fn owner_combination_overflow_rejects_replacement_before_source_commit() {
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(novadraw::text::BuiltinFont::Inter)
        .unwrap();
    let id = root(&mut runtime);
    settle(&mut runtime);
    let revision = runtime.tree().component_revision(id).unwrap();
    let border = CompoundBorder::new(
        MarginBorder::new(Insets::new(f64::MAX * 0.4, 0.0, 0.0, 0.0)).unwrap(),
        TitleBarBorder::new("Title", Color::BLUE)
            .with_padding(0.0, f64::MAX * 0.4)
            .unwrap(),
    )
    .unwrap();
    let error = runtime
        .figure(id)
        .unwrap()
        .update_capability(ReplaceFigureBorder::set(border))
        .unwrap_err();
    let CapabilityUpdateError::Effect(RuntimeMutationError::BorderPreparation { figure, source }) =
        error
    else {
        panic!("lost owner preparation error");
    };
    assert_eq!(figure, id);
    assert!(
        matches!(source, TextError::BorderInput(ref error) if matches!(**error, BorderInputError::Overflow { .. })),
        "{source:?}"
    );
    assert!(source.source().is_some());
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::ZERO);
    assert!(!runtime.frames().has_pending_update());
    runtime
        .figure(id)
        .unwrap()
        .update_capability(ReplaceFigureBorder::set(TitleBarBorder::new(
            "Recovered",
            Color::BLUE,
        )))
        .unwrap();
    assert!(runtime.tree().insets(id).unwrap().top > 0.0);
    settle(&mut runtime);
}

struct PanicOwner;
impl Border for PanicOwner {
    fn get_insets(&self) -> Insets {
        Insets::ZERO
    }
    fn paint(&self, _: Rectangle, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
    fn has_owner_snapshot(&self) -> bool {
        true
    }
    fn resolve_owner_snapshot(
        &self,
        _: Option<&BorderSnapshot>,
        _: &FontDescriptor,
        _: &mut dyn TextLayoutEngine,
    ) -> Result<Option<BorderSnapshot>, TextError> {
        panic!("external owner callback");
    }
}
#[test]
fn owner_callback_panic_retains_existing_fault_semantics() {
    let mut runtime = Runtime::empty();
    let id = root(&mut runtime);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime
                .figure(id)
                .unwrap()
                .update_capability(ReplaceFigureBorder::set(PanicOwner))
        }))
        .is_err()
    );
    assert!(runtime.is_faulted());
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::ZERO);
}

#[test]
fn detached_border_replacement_uses_checked_candidate_and_one_capture() {
    use novadraw::figure::{BorderedFigure, border::BorderCandidate};
    let mut figure = RectangleFigure::from_bounds(Rectangle::ZERO).unwrap();
    let border = changing();
    let shared: Arc<dyn Border> = Arc::new(border.clone());
    let candidate = BorderCandidate::new(Some(shared.clone())).unwrap();
    assert_eq!(candidate.metrics().insets(), Insets::uniform(1.0));
    assert!(figure.replace_border(candidate).is_none());
    assert_eq!(border.calls[0].load(Ordering::Relaxed), 1);
    assert_eq!(border.calls[1].load(Ordering::Relaxed), 1);
    *border.values.lock().unwrap() = (Insets::uniform(-1.0), Dimension::ZERO);
    assert!(BorderCandidate::new(Some(shared.clone())).is_err());
    assert!(Arc::ptr_eq(figure.border().unwrap(), &shared));
    *border.values.lock().unwrap() = (Insets::uniform(2.0), Dimension::ZERO);
    figure.replace_border(BorderCandidate::new(Some(shared)).unwrap());
    assert_eq!(Figure::initial_insets(&figure), Insets::uniform(2.0));
    assert!(figure.replace_border(BorderCandidate::default()).is_some());
    assert!(figure.border().is_none());
}

#[test]
fn external_border_changed_after_detached_construction_rejects_whole_subtree() {
    let border = changing();
    let candidate = RectangleFigure::from_bounds(Rectangle::ZERO)
        .unwrap()
        .with_border(border.clone())
        .unwrap();
    *border.values.lock().unwrap() = (Insets::ZERO, Dimension::new(f64::NAN, 0.0));
    let mut runtime = Runtime::empty();
    let id = root(&mut runtime);
    settle(&mut runtime);
    let revision = runtime.tree().component_revision(id).unwrap();
    let subtree = FigureSubtree::new(Box::new(candidate.clone()));
    assert!(runtime.set_contents_subtree(subtree).is_err());
    assert_eq!(runtime.tree().contents(), Some(id));
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert!(!runtime.frames().has_pending_update());
    *border.values.lock().unwrap() = (Insets::uniform(1.0), Dimension::ZERO);
    let next = runtime
        .set_contents_subtree(FigureSubtree::new(Box::new(candidate)))
        .unwrap();
    assert_ne!(runtime.tree().contents(), Some(id));
    std::hint::black_box(next);
    settle(&mut runtime);
}

struct SelectiveOwner {
    title: TitleBarBorder,
    reject_large: Arc<std::sync::atomic::AtomicBool>,
}
impl Border for SelectiveOwner {
    fn get_insets(&self) -> Insets {
        Insets::ZERO
    }
    fn paint(&self, _: Rectangle, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
    fn has_owner_snapshot(&self) -> bool {
        true
    }
    fn resolve_owner_snapshot(
        &self,
        previous: Option<&BorderSnapshot>,
        font: &FontDescriptor,
        text: &mut dyn TextLayoutEngine,
    ) -> Result<Option<BorderSnapshot>, TextError> {
        if self.reject_large.load(Ordering::Relaxed) && font.size() >= 24.0 {
            return Err(TextError::BorderInput(Box::new(
                BorderInputError::Negative {
                    field: "external owner metric",
                    side: None,
                },
            )));
        }
        self.title.resolve_owner_snapshot(previous, font, text)
    }
}

#[test]
fn shared_owner_snapshots_remain_isolated_when_one_replacement_fails_and_recovers() {
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(novadraw::text::BuiltinFont::Inter)
        .unwrap();
    let parent = root(&mut runtime);
    let mut owners = Vec::new();
    for (size, width) in [(12.0, 40.0), (24.0, 80.0)] {
        let id = runtime
            .figure(parent)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new(0.0, 0.0, width, 60.0).unwrap(),
            ))
            .unwrap();
        runtime
            .figure(id)
            .unwrap()
            .set_style(novadraw::figure::FigureStyle {
                font: Some(FontDescriptor::parse(&format!("{size}px Inter")).unwrap()),
                ..Default::default()
            })
            .unwrap();
        runtime
            .figure(id)
            .unwrap()
            .update_capability(ReplaceFigureBorder::set(TitleBarBorder::new(
                "Old",
                Color::BLUE,
            )))
            .unwrap();
        owners.push(id);
    }
    settle(&mut runtime);
    let old_large = runtime.tree().insets(owners[1]).unwrap();
    let old_revision = runtime.tree().component_revision(owners[1]).unwrap();
    let reject = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let shared: Arc<dyn Border> = Arc::new(
        CompoundBorder::new(
            MarginBorder::new(Insets::uniform(2.0)).unwrap(),
            SelectiveOwner {
                title: TitleBarBorder::new("Shared", Color::RED),
                reject_large: reject.clone(),
            },
        )
        .unwrap(),
    );
    runtime
        .figure(owners[0])
        .unwrap()
        .update_capability(ReplaceFigureBorder::new(Some(shared.clone())))
        .unwrap();
    let small = runtime.tree().insets(owners[0]).unwrap();
    settle(&mut runtime);
    assert!(
        runtime
            .figure(owners[1])
            .unwrap()
            .update_capability(ReplaceFigureBorder::new(Some(shared.clone())))
            .is_err()
    );
    assert_eq!(runtime.tree().insets(owners[1]).unwrap(), old_large);
    assert_eq!(
        runtime.tree().component_revision(owners[1]).unwrap(),
        old_revision
    );
    assert_eq!(runtime.tree().insets(owners[0]).unwrap(), small);
    assert!(!runtime.frames().has_pending_update());
    reject.store(false, Ordering::Relaxed);
    runtime
        .figure(owners[1])
        .unwrap()
        .update_capability(ReplaceFigureBorder::new(Some(shared)))
        .unwrap();
    let large = runtime.tree().insets(owners[1]).unwrap();
    assert!(large.top > small.top);
    assert_eq!(runtime.tree().insets(owners[0]).unwrap(), small);
    settle(&mut runtime);
}

#[test]
fn deferred_border_rejection_is_observable_without_revision_damage_or_notification() {
    use novadraw::{
        event::{
            EventContext, FigureEventHandler, ListenerDirective, MouseButton, MouseEvent,
            NotificationRecord, ObservationListener,
        },
        figure::{
            BORDER, BorderCapability, BorderedFigure, FigureCapabilityBuilder,
            FigureCapabilityRegistrationError, INPUT, InputCapability, border::BorderCandidate,
        },
        runtime::StableSceneQuery,
    };
    struct Trigger {
        figure: RectangleFigure,
        next: ChangingBorder,
    }
    impl Figure for Trigger {
        fn initial_bounds(&self) -> Rectangle {
            self.figure.initial_bounds()
        }
        fn name(&self) -> &'static str {
            "external deferred border"
        }
        fn get_border(&self) -> Option<&dyn Border> {
            Figure::get_border(&self.figure)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(INPUT, InputCapability::of::<Self>())?;
            out.register(BORDER, BorderCapability::of::<Self>())
        }
    }
    impl BorderedFigure for Trigger {
        fn border(&self) -> Option<&Arc<dyn Border>> {
            self.figure.border()
        }
        fn replace_border(&mut self, candidate: BorderCandidate) -> Option<Arc<dyn Border>> {
            self.figure.replace_border(candidate)
        }
    }
    impl FigureEventHandler for Trigger {
        fn on_mouse_pressed(&self, _: &MouseEvent, context: &mut EventContext<'_>) -> bool {
            context
                .effects()
                .update_capability_later(ReplaceFigureBorder::set(self.next.clone()));
            true
        }
    }
    struct Count(Arc<AtomicUsize>);
    impl ObservationListener for Count {
        fn observed(&self, _: &NotificationRecord, _: StableSceneQuery<'_>) -> ListenerDirective {
            self.0.fetch_add(1, Ordering::Relaxed);
            ListenerDirective::Keep
        }
    }
    let mut runtime = Runtime::empty();
    let border = changing();
    *border.values.lock().unwrap() = (Insets::uniform(-1.0), Dimension::ZERO);
    let id = runtime
        .set_contents(Box::new(Trigger {
            figure: RectangleFigure::new(0.0, 0.0, 120.0, 80.0).unwrap(),
            next: border.clone(),
        }))
        .unwrap();
    settle(&mut runtime);
    let revision = runtime.tree().component_revision(id).unwrap();
    let notifications = Arc::new(AtomicUsize::new(0));
    runtime
        .observations()
        .add_observation_listener(Box::new(Count(notifications.clone())));
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert_eq!(
        runtime.take_deferred_mutation_errors(),
        vec![RuntimeMutationError::ComponentUpdateRejected(id)]
    );
    assert_eq!(runtime.tree().component_revision(id).unwrap(), revision);
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::ZERO);
    assert_eq!(notifications.load(Ordering::Relaxed), 0);
    assert!(!runtime.frames().has_pending_update());
    *border.values.lock().unwrap() = (Insets::uniform(2.0), Dimension::ZERO);
    runtime
        .input()
        .dispatch_mouse_pressed(5.0, 5.0, MouseButton::Left);
    assert!(runtime.take_deferred_mutation_errors().is_empty());
    assert_eq!(runtime.tree().insets(id).unwrap(), Insets::uniform(2.0));
    settle(&mut runtime);
    assert!(notifications.load(Ordering::Relaxed) > 0);
}
