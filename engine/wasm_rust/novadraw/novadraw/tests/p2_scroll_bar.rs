use novadraw::{
    container::{
        DefaultRangeModel, RangeModel, ScrollBarConfig, ScrollBarFigure, ScrollBarPart,
        ScrollOrientation, SetScrollBarModel,
    },
    event::{EventContext, MonotonicTime, MouseButton},
    figure::{
        ACTIVITY, ActivityCapability, Figure, FigureActivity, FigureCapabilityBuilder,
        FigureCapabilityRegistrationError, RectangleFigure,
    },
    geometry::Rectangle,
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
    tree::{FigureId, FigureSubtree},
};

struct ExternalRange(DefaultRangeModel);
impl RangeModel for ExternalRange {
    fn snapshot(&self) -> novadraw::container::RangeModelSnapshot {
        self.0.snapshot()
    }
    fn set_all(
        &self,
        minimum: f64,
        extent: f64,
        maximum: f64,
    ) -> Result<novadraw::container::RangeChangeSet, novadraw::container::RangeModelError> {
        self.0.set_all(minimum, extent, maximum)
    }
    fn set_minimum(
        &self,
        value: f64,
    ) -> Result<novadraw::container::RangeChangeSet, novadraw::container::RangeModelError> {
        self.0.set_minimum(value)
    }
    fn set_maximum(
        &self,
        value: f64,
    ) -> Result<novadraw::container::RangeChangeSet, novadraw::container::RangeModelError> {
        self.0.set_maximum(value)
    }
    fn set_extent(
        &self,
        value: f64,
    ) -> Result<novadraw::container::RangeChangeSet, novadraw::container::RangeModelError> {
        self.0.set_extent(value)
    }
    fn set_value(
        &self,
        value: f64,
    ) -> Result<novadraw::container::RangeChangeSet, novadraw::container::RangeModelError> {
        self.0.set_value(value)
    }
    fn add_listener(
        &self,
        listener: Arc<dyn novadraw::container::RangeListener>,
    ) -> novadraw::container::RangeListenerId {
        self.0.add_listener(listener)
    }
    fn remove_listener(&self, id: novadraw::container::RangeListenerId) -> bool {
        self.0.remove_listener(id)
    }
}
use std::sync::{Arc, Mutex};

fn frame(runtime: &mut Runtime) {
    let submission = runtime
        .frames()
        .prepare_submission(
            SurfaceInfo {
                logical_width: 400.0,
                logical_height: 400.0,
                pixel_width: 400,
                pixel_height: 400,
                scale_factor: 1.0,
            },
            BackendCapabilities::RETAINED_PARTIAL,
        )
        .into_ready()
        .unwrap();
    let _ = runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
}

fn fixture(
    orientation: ScrollOrientation,
) -> (Runtime, FigureId, FigureId, Arc<DefaultRangeModel>) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 400.0)))
        .unwrap();
    let model = Arc::new(DefaultRangeModel::new(10.0, 20.0, 110.0).unwrap());
    model.set_value(30.0).unwrap();
    let bounds = match orientation {
        ScrollOrientation::Horizontal => Rectangle::new(30.0, 40.0, 200.0, 14.0),
        ScrollOrientation::Vertical => Rectangle::new(30.0, 40.0, 14.0, 200.0),
    };
    let description = ScrollBarFigure::new(model.clone())
        .with_bounds(bounds)
        .with_config(ScrollBarConfig {
            orientation,
            ..ScrollBarConfig::default()
        })
        .unwrap()
        .with_part(
            ScrollBarPart::Thumb,
            Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)),
        )
        .into_subtree();
    let thumb_node = description
        .parts
        .iter()
        .find(|(part, _)| *part == ScrollBarPart::Thumb)
        .unwrap()
        .1;
    let receipt = runtime
        .figure(root)
        .unwrap()
        .add_subtree(description.subtree)
        .unwrap();
    let bar = receipt.figure(description.root).unwrap();
    let thumb = receipt.figure(thumb_node).unwrap();
    frame(&mut runtime);
    (runtime, bar, thumb, model)
}

#[test]
fn horizontal_and_vertical_layout_use_nonzero_minimum_and_parent_coordinates() {
    for orientation in [ScrollOrientation::Horizontal, ScrollOrientation::Vertical] {
        let (runtime, bar, thumb, _) = fixture(orientation);
        assert_eq!(runtime.tree().child_order(bar).unwrap().len(), 5);
        let bounds = runtime.tree().figure_bounds(thumb).unwrap();
        let expected_start = 14.0 + (172.0 - 34.4) * 20.0 / 80.0;
        match orientation {
            ScrollOrientation::Horizontal => {
                assert!((bounds.x - expected_start).abs() < 1e-9);
                assert_eq!(bounds.y, 0.0);
            }
            ScrollOrientation::Vertical => {
                assert!((bounds.y - expected_start).abs() < 1e-9);
                assert_eq!(bounds.x, 0.0);
            }
        }
    }
}

#[test]
fn external_model_changes_and_replacement_retire_old_model() {
    let (mut runtime, bar, thumb, model) = fixture(ScrollOrientation::Horizontal);
    model.set_value(90.0).unwrap();
    frame(&mut runtime);
    let moved = runtime.tree().figure_bounds(thumb).unwrap().x;
    assert!(moved > 100.0);
    let replacement = Arc::new(ExternalRange(
        DefaultRangeModel::new(0.0, 10.0, 100.0).unwrap(),
    ));
    runtime
        .figure(bar)
        .unwrap()
        .update_component(SetScrollBarModel(replacement.clone()))
        .unwrap();
    frame(&mut runtime);
    assert_eq!(runtime.tree().figure_bounds(thumb).unwrap().x, 14.0);
    model.set_value(10.0).unwrap();
    replacement.set_value(50.0).unwrap();
    frame(&mut runtime);
    assert!(runtime.tree().figure_bounds(thumb).unwrap().x > 80.0);
}

#[test]
fn hold_repeats_without_catchup_and_release_cancels() {
    let (mut runtime, _, _, model) = fixture(ScrollOrientation::Horizontal);
    runtime
        .input()
        .dispatch_mouse_pressed(225.0, 47.0, MouseButton::Right);
    assert_eq!(model.value(), 30.0);
    runtime
        .input()
        .dispatch_mouse_released(225.0, 47.0, MouseButton::Right);
    runtime
        .input()
        .dispatch_mouse_pressed(225.0, 47.0, MouseButton::Left);
    assert_eq!(model.value(), 40.0);
    assert_eq!(
        runtime.animation().next_wake_deadline(),
        Some(MonotonicTime::from_micros(250_000))
    );
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(249_999))
        .unwrap();
    assert_eq!(model.value(), 40.0);
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(1_000_000))
        .unwrap();
    assert_eq!(model.value(), 50.0);
    runtime
        .input()
        .dispatch_mouse_released(225.0, 47.0, MouseButton::Left);
    assert_eq!(runtime.animation().next_wake_deadline(), None);
    runtime
        .animation()
        .advance_time(MonotonicTime::from_micros(2_000_000))
        .unwrap();
    assert_eq!(model.value(), 50.0);
}

#[test]
fn hidden_disabled_exit_and_removal_cancel_activity() {
    for action in 0..4 {
        let (mut runtime, bar, _, model) = fixture(ScrollOrientation::Horizontal);
        runtime
            .input()
            .dispatch_mouse_pressed(225.0, 47.0, MouseButton::Left);
        match action {
            0 => {
                runtime.figure(bar).unwrap().set_visible(false).unwrap();
            }
            1 => {
                runtime.figure(bar).unwrap().set_enabled(false).unwrap();
            }
            2 => {
                runtime.input().dispatch_mouse_moved(300.0, 300.0);
            }
            _ => {
                let root = runtime.tree().parent_id(bar).unwrap().unwrap();
                runtime.figure(root).unwrap().remove(bar).unwrap();
            }
        }
        runtime
            .animation()
            .advance_time(MonotonicTime::from_micros(500_000))
            .unwrap();
        assert_eq!(model.value(), 40.0);
        assert_eq!(runtime.animation().next_wake_deadline(), None);
    }
}

#[test]
fn short_track_and_empty_range_hide_thumb() {
    let (mut runtime, bar, thumb, model) = fixture(ScrollOrientation::Horizontal);
    runtime
        .figure(bar)
        .unwrap()
        .set_bounds(Rectangle::new(30.0, 40.0, 30.0, 14.0))
        .unwrap();
    frame(&mut runtime);
    assert!(!runtime.tree().is_visible(thumb).unwrap());
    runtime
        .figure(bar)
        .unwrap()
        .set_bounds(Rectangle::new(30.0, 40.0, 200.0, 14.0))
        .unwrap();
    model.set_extent(100.0).unwrap();
    frame(&mut runtime);
    assert!(!runtime.tree().is_visible(thumb).unwrap());
}

#[test]
fn custom_thumb_minimum_controls_layout_and_drag_ratio() {
    let (mut runtime, bar, thumb, model) = fixture(ScrollOrientation::Horizontal);
    runtime
        .figure(thumb)
        .unwrap()
        .set_minimum_size(novadraw::geometry::Dimension::new(80.0, 14.0))
        .unwrap();
    frame(&mut runtime);
    let bounds = runtime.tree().figure_bounds(thumb).unwrap();
    assert_eq!(bounds.width, 80.0);
    let x = 30.0 + bounds.x + bounds.width / 2.0;
    runtime
        .input()
        .dispatch_mouse_pressed(x, 47.0, MouseButton::Left);
    runtime.input().dispatch_mouse_moved(x + 23.0, 47.0);
    runtime
        .input()
        .dispatch_mouse_released(x + 23.0, 47.0, MouseButton::Left);
    assert!((model.value() - 50.0).abs() < 1e-9);
    assert_eq!(runtime.tree().child_order(bar).unwrap().len(), 5);
}

#[test]
fn config_rejects_nonfinite_and_nonpositive_geometry() {
    for value in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert!(
            ScrollBarFigure::default()
                .with_config(ScrollBarConfig {
                    thickness: value,
                    ..ScrollBarConfig::default()
                })
                .is_err()
        );
    }
}

#[test]
fn replacing_thumb_uses_atomic_subtree_admission() {
    let (mut runtime, bar, thumb, _) = fixture(ScrollOrientation::Horizontal);
    struct Invalid;
    impl Figure for Invalid {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, f64::NAN, 1.0)
        }
        fn name(&self) -> &'static str {
            "Invalid"
        }
    }
    assert!(
        runtime
            .figure(bar)
            .unwrap()
            .replace_child_subtree(thumb, FigureSubtree::new(Box::new(Invalid)))
            .is_err()
    );
    assert!(runtime.tree().node(thumb).is_ok());
    let mut replacement = FigureSubtree::new(Box::new(RectangleFigure::new(0.0, 0.0, 1.0, 1.0)));
    let node = replacement.root();
    replacement
        .set_constraint(node, Box::new(ScrollBarPart::Thumb))
        .unwrap();
    let receipt = runtime
        .figure(bar)
        .unwrap()
        .replace_child_subtree(thumb, replacement)
        .unwrap();
    let new_thumb = receipt.figure(node).unwrap();
    frame(&mut runtime);
    assert!(runtime.tree().node(thumb).is_err());
    assert!(runtime.tree().figure_bounds(new_thumb).unwrap().width > 30.0);
}

struct ExternalActivity {
    polls: Arc<Mutex<Vec<MonotonicTime>>>,
    panic: bool,
}
impl Figure for ExternalActivity {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 10.0, 10.0)
    }
    fn name(&self) -> &'static str {
        "ExternalActivity"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(ACTIVITY, ActivityCapability::of::<Self>())
    }
}
impl FigureActivity for ExternalActivity {
    fn poll(&self, now: MonotonicTime, _: bool, _: &mut EventContext<'_>) {
        assert!(!self.panic, "external activity failure");
        self.polls.lock().unwrap().push(now);
    }
    fn next_wake_deadline(&self) -> Option<MonotonicTime> {
        Some(MonotonicTime::from_micros(10))
    }
}

#[test]
fn external_activity_shares_clock_and_panic_boundary() {
    for panic in [false, true] {
        let mut runtime = Runtime::empty();
        let root = runtime
            .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 100.0)))
            .unwrap();
        let polls = Arc::new(Mutex::new(Vec::new()));
        runtime
            .figure(root)
            .unwrap()
            .add(Box::new(ExternalActivity {
                polls: polls.clone(),
                panic,
            }))
            .unwrap();
        assert_eq!(
            runtime.animation().next_wake_deadline(),
            Some(MonotonicTime::from_micros(10))
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime
                .animation()
                .advance_time(MonotonicTime::from_micros(10))
                .unwrap();
        }));
        assert_eq!(result.is_err(), panic);
        assert_eq!(runtime.is_faulted(), panic);
        if panic {
            assert_eq!(runtime.animation().next_wake_deadline(), None);
        } else {
            assert_eq!(*polls.lock().unwrap(), vec![MonotonicTime::from_micros(10)]);
        }
    }
}
