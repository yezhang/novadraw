use novadraw::{
    graphics::Color,
    text::{
        BuiltinFont, FontDescriptor, StyledTextContent, StyledTextRequest, TextConstraints,
        TextContentError, TextContentStore, TextError, TextInlineBox, TextStyle, TextStyleSpan,
        TextSystem, shaping::TextAnalysisRequest,
    },
};

fn style() -> TextStyle {
    TextStyle::new(
        FontDescriptor::new(BuiltinFont::Inter.family(), 14.0).unwrap(),
        Color::BLACK,
    )
}

fn spans() -> Vec<TextStyleSpan> {
    vec![TextStyleSpan {
        range: 0..3,
        style: style(),
    }]
}

#[test]
fn shared_request_analysis_and_clone_keep_the_same_checked_content() {
    let store = TextContentStore::new(usize::MAX);
    let content = store.create("中hello".into(), style(), spans()).unwrap();
    let usage = store.usage().unwrap();
    let request =
        StyledTextRequest::from_content(content.clone(), vec![], TextConstraints::UNBOUNDED)
            .unwrap();
    let cloned = request.clone();
    let analysis = TextAnalysisRequest::new(&request).unwrap();
    for other in [cloned.content(), analysis.request().content(), &content] {
        assert!(std::ptr::eq(request.text().as_ptr(), other.text().as_ptr()));
        assert!(std::ptr::eq(
            request.spans().as_ptr(),
            other.spans().as_ptr()
        ));
        assert!(std::ptr::eq(request.default_style(), other.default_style()));
    }
    assert_eq!(store.usage().unwrap(), usage);
    let owned = StyledTextRequest::new(
        "中hello",
        style(),
        spans(),
        vec![],
        TextConstraints::UNBOUNDED,
    )
    .unwrap();
    assert_eq!(request, owned);
    drop(store);
    drop(content);
    drop(request);
    drop(cloned);
    assert_eq!(analysis.request().text(), "中hello");
    assert_eq!(
        analysis.request().content().usage().unwrap().retained_units,
        usage.retained_units
    );
}

#[test]
fn shrink_preserves_old_leases_and_refuses_positive_new_charges() {
    let store = TextContentStore::new(usize::MAX);
    let content = store.create("中hello".into(), style(), spans()).unwrap();
    let retained = content.retained_units();
    let peer = store.clone();
    peer.set_limit(0).unwrap();
    assert!(store.usage().unwrap().is_over_limit());
    let clone = content.clone();
    assert_eq!(content.usage().unwrap().content_count, 1);
    assert!(matches!(store.create("new".into(), style(), vec![]),
        Err(TextError::Content(error)) if matches!(*error, TextContentError::Capacity {
            retained: current, limit: 0, ..
        } if current == retained)));
    drop(content);
    assert_eq!(store.usage().unwrap().retained_units, retained);
    assert_eq!(clone.text(), "中hello");
    drop(clone);
    assert_eq!(store.usage().unwrap().retained_units, 0);
    assert_eq!(store.usage().unwrap().content_count, 0);
    peer.set_limit(usize::MAX).unwrap();
    assert!(store.create("new".into(), style(), vec![]).is_ok());
}

#[test]
fn invalid_content_is_rejected_without_installing_or_charging_it() {
    let store = TextContentStore::new(usize::MAX);
    for range in [0..1, 0..8, 3..3] {
        assert_eq!(
            store.create(
                "中a".into(),
                style(),
                vec![TextStyleSpan {
                    range,
                    style: style(),
                }]
            ),
            Err(TextError::InvalidStyleRange)
        );
        assert_eq!(store.usage().unwrap().retained_units, 0);
        assert_eq!(store.usage().unwrap().content_count, 0);
    }
}

#[test]
fn shared_construction_rechecks_current_constraints_boxes_and_utf8() {
    let content = StyledTextContent::new("中a", style(), vec![]).unwrap();
    for offset in [1, 5] {
        let input = TextInlineBox {
            id: 1,
            byte_offset: offset,
            width: 1.0,
            height: 1.0,
        };
        assert_eq!(
            StyledTextRequest::from_content(
                content.clone(),
                vec![input],
                TextConstraints::UNBOUNDED
            ),
            Err(TextError::InvalidInlineBox)
        );
    }
    let input = TextInlineBox {
        id: 1,
        byte_offset: 3,
        width: f32::NAN,
        height: 1.0,
    };
    assert_eq!(
        StyledTextRequest::from_content(content.clone(), vec![input], TextConstraints::UNBOUNDED),
        Err(TextError::InvalidMetric)
    );
    let input = TextInlineBox {
        id: 1,
        byte_offset: 3,
        width: 1.0,
        height: 1.0,
    };
    assert_eq!(
        StyledTextRequest::from_content(
            content.clone(),
            vec![input.clone(), input],
            TextConstraints::UNBOUNDED
        ),
        Err(TextError::InvalidInlineBox)
    );
    assert_eq!(
        StyledTextRequest::from_content(
            content,
            vec![],
            TextConstraints {
                max_width: Some(f32::NAN)
            }
        ),
        Err(TextError::InvalidMetric)
    );
}

#[test]
fn owned_and_shared_requests_produce_identical_layout_and_interaction() {
    let content = StyledTextContent::new("中hello world wrap", style(), spans()).unwrap();
    let shared =
        StyledTextRequest::from_content(content, vec![], TextConstraints::new(Some(80.0)).unwrap())
            .unwrap();
    let owned = StyledTextRequest::new(
        shared.text(),
        style(),
        spans(),
        vec![],
        shared.constraints(),
    )
    .unwrap();
    let mut system = TextSystem::new();
    system.register_builtin_font(BuiltinFont::Inter).unwrap();
    let mut measure = system.measure(style().font().clone());
    let a = measure.layout_styled(&shared).unwrap();
    let b = measure.layout_styled(&owned).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.request(), &shared);
}

#[test]
fn external_consumer_registers_owned_typed_input_through_public_api() {
    use novadraw::{
        figure::{
            PreparationInputErrorKind, PreparationInputKey, PreparationInputPayload,
            PreparationInputRegistry, PreparationScene,
        },
        tree::FigureId,
    };
    #[derive(Eq, PartialEq)]
    struct ExternalVersion(u64);
    impl PreparationInputPayload for ExternalVersion {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    let key = PreparationInputKey::<ExternalVersion>::new("external.version");
    let mut registry = PreparationInputRegistry::new();
    registry
        .register(key, |_: FigureId, _: &PreparationScene<'_>| {
            Ok(ExternalVersion(1))
        })
        .unwrap();
    let duplicate = registry
        .register(key, |_: FigureId, _: &PreparationScene<'_>| {
            Ok(ExternalVersion(2))
        })
        .unwrap_err();
    assert_eq!(
        duplicate.kind,
        PreparationInputErrorKind::DuplicateRegistration
    );
    assert_eq!(duplicate.source, None);
    assert_eq!(
        duplicate.payload_type,
        std::any::TypeId::of::<ExternalVersion>()
    );
}

#[test]
fn external_consumer_reads_and_publishes_typed_evidence_through_runtime() {
    use novadraw::{
        figure::{
            Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureDrawing,
            FigureMeasurement, FigurePreparation, FigurePresentation, PREPARATION,
            PreparationCapability, PreparationInputKey, PreparationInputPayload,
            PreparationInputRegistry, PreparationInputSnapshot, PreparationScene, PreparedScene,
        },
        geometry::{Dimension, Rectangle},
        graphics::{GraphicsError, PaintContext},
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, Runtime},
        text::MeasureContext,
        tree::FigureId,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    #[derive(Eq, PartialEq)]
    struct Version(u64);
    impl PreparationInputPayload for Version {
        fn retained_units(&self) -> u64 {
            0
        }
    }
    const KEY: PreparationInputKey<Version> = PreparationInputKey::new("external.runtime.version");
    struct Drawing;
    impl FigureDrawing for Drawing {
        fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            Ok(())
        }
    }
    struct Producer(PreparationInputRegistry);
    impl Figure for Producer {
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        }
        fn name(&self) -> &'static str {
            "ExternalInputProducer"
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(PREPARATION, PreparationCapability::of::<Self>())
        }
    }
    impl FigurePreparation for Producer {
        fn prepare(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Err(TextError::InvalidPreparedScene(
                "frozen input required".into(),
            ))
        }
        fn prepare_scene(
            &self,
            _: &mut MeasureContext<'_>,
            bounds: Rectangle,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparedScene, TextError> {
            let evidence = scene.input(&self.0, KEY, scene.root())?;
            let width = evidence.value(KEY).unwrap().0 as f64;
            let presentation = FigurePresentation::new(
                FigureMeasurement::new(width, 10.0, None),
                Dimension::ZERO,
                bounds,
                Arc::new(Drawing),
            )?
            .with_data(Arc::new(evidence));
            Ok(PreparedScene::new(presentation))
        }
    }
    let version = Arc::new(AtomicU64::new(11));
    let reader_version = version.clone();
    let mut registry = PreparationInputRegistry::new();
    registry
        .register(KEY, move |_: FigureId, _: &PreparationScene<'_>| {
            Ok(Version(reader_version.load(Ordering::Relaxed)))
        })
        .unwrap();
    let mut runtime = Runtime::empty();
    let root = runtime.set_contents(Box::new(Producer(registry))).unwrap();
    let surface = SurfaceInfo {
        logical_width: 100.0,
        logical_height: 100.0,
        pixel_width: 100,
        pixel_height: 100,
        scale_factor: 1.0,
    };
    let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("expected checked external preparation");
    };
    runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    let old = runtime
        .tree()
        .prepared_presentation(root)
        .unwrap()
        .unwrap()
        .clone();
    let evidence = old.data::<PreparationInputSnapshot>().unwrap();
    assert_eq!(evidence.value(KEY).unwrap().0, 11);
    version.store(12, Ordering::Relaxed);
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 101.0, 100.0))
        .unwrap();
    let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("expected fresh external preparation");
    };
    runtime
        .frames()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    assert_eq!(evidence.value(KEY).unwrap().0, 11);
    let current = runtime.tree().prepared_presentation(root).unwrap().unwrap();
    assert_eq!(
        current
            .data::<PreparationInputSnapshot>()
            .unwrap()
            .value(KEY)
            .unwrap()
            .0,
        12
    );
}

#[test]
fn external_flow_root_and_handler_share_measure_prepare_and_preserve_old_sources() {
    use novadraw::{
        figure::{
            CONTAINER, ContainerCapability, Figure, FigureCapabilityBuilder,
            FigureCapabilityRegistrationError, FigureContainer, FigurePreparation,
            FigurePresentation, MeasureConstraints, PREPARATION, PreparationCapability,
            PreparationMeasureError, PreparationMeasurement, PreparationScene, PreparedScene,
            flow::{
                ContributionKey, DeclaredFlowHandler, FLOW, FlowAssembly, FlowCapability,
                FlowContributionContext, FlowContributionPayload, FlowContributionRegistry,
                FlowContributionSink, FlowContributor, FlowError, FlowLayoutService,
                FlowReuseDeclaration, FlowReuseIdentity, FlowSnapshot,
            },
        },
        geometry::Rectangle,
        render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
        runtime::{FramePreparation, Runtime},
        text::MeasureContext,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct ExternalText;
    impl FlowContributionPayload for ExternalText {}
    const TEXT: ContributionKey<ExternalText> = ContributionKey::new("external.same-model");
    struct Page {
        service: FlowLayoutService,
        identity: FlowReuseIdentity,
    }
    impl Figure for Page {
        fn name(&self) -> &'static str {
            "ExternalReusablePage"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 150.0, 100.0)
        }
        fn register_capabilities(
            &self,
            out: &mut FigureCapabilityBuilder,
        ) -> Result<(), FigureCapabilityRegistrationError> {
            out.register(CONTAINER, ContainerCapability::of::<Self>())?;
            out.register(FLOW, FlowCapability::of::<Self>())?;
            out.register(PREPARATION, PreparationCapability::of::<Self>())
        }
    }
    impl FigureContainer for Page {}
    impl FlowContributor for Page {
        fn assembly_reuse(
            &self,
            context: &FlowContributionContext<'_>,
        ) -> Result<Option<FlowReuseDeclaration>, FlowError> {
            context
                .scene
                .budget()
                .allocate(
                    "external declaration",
                    std::mem::size_of::<std::any::TypeId>() as u64,
                )
                .map_err(TextError::from)?;
            Ok(Some(self.identity.declaration(0).with_contribution(TEXT)))
        }
        fn contribute(
            &self,
            _: &FlowContributionContext<'_>,
            out: &mut FlowContributionSink,
        ) -> Result<(), FlowError> {
            out.append(TEXT, ExternalText);
            Ok(())
        }
    }
    impl FigurePreparation for Page {
        fn measure_scene(
            &self,
            context: &mut MeasureContext<'_>,
            constraints: MeasureConstraints,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparationMeasurement, PreparationMeasureError> {
            self.service.measure(context, constraints, scene)
        }
        fn prepare(
            &self,
            _: &mut MeasureContext<'_>,
            _: Rectangle,
        ) -> Result<FigurePresentation, TextError> {
            Err(TextError::InvalidPreparedScene("scene required".into()))
        }
        fn prepare_scene(
            &self,
            context: &mut MeasureContext<'_>,
            bounds: Rectangle,
            scene: &PreparationScene<'_>,
        ) -> Result<PreparedScene, TextError> {
            self.service
                .prepare(context, bounds, scene)
                .map_err(FlowError::into_text)
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let handler_identity = FlowReuseIdentity::new();
    let mut registry = FlowContributionRegistry::standard();
    registry
        .register(
            TEXT,
            DeclaredFlowHandler::new(
                move |_: &ExternalText,
                      context: &FlowContributionContext<'_>,
                      out: &mut FlowAssembly<'_>| {
                    observed.fetch_add(1, Ordering::Relaxed);
                    out.text(
                        context.source,
                        "external 中 paragraph",
                        context.text_style()?,
                    )
                },
                move |_: &FlowContributionContext<'_>| Ok(Some(handler_identity.declaration(0))),
            ),
        )
        .unwrap();
    let service = FlowLayoutService::new(registry).with_assembly_reuse(true);
    service
        .set_assembly_limits(1024 * 1024, 4 * 1024 * 1024)
        .unwrap();
    let mut runtime = Runtime::empty();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(Page {
            service: service.clone(),
            identity: FlowReuseIdentity::new(),
        }))
        .unwrap();
    let surface = SurfaceInfo {
        logical_width: 150.0,
        logical_height: 100.0,
        pixel_width: 150,
        pixel_height: 100,
        scale_factor: 1.0,
    };
    let initial = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL);
    let FramePreparation::Ready(frame) = initial else {
        panic!("external flow must publish: {initial:?}");
    };
    let _ = runtime
        .frames()
        .complete_submission(
            frame.session_id,
            frame.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    let old = runtime
        .tree()
        .prepared_presentation(root)
        .unwrap()
        .unwrap()
        .clone();
    let snapshot = old.data::<FlowSnapshot>().unwrap();
    assert_eq!(
        snapshot.paragraphs[0].layout.request().text(),
        "external 中 paragraph"
    );
    let sources = snapshot.paragraphs[0].sources.clone();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 80.0, 100.0))
        .unwrap();
    let FramePreparation::Ready(frame) = runtime
        .frames()
        .prepare_submission(surface, BackendCapabilities::RETAINED_PARTIAL)
    else {
        panic!("changed width must publish");
    };
    let _ = runtime
        .frames()
        .complete_submission(
            frame.session_id,
            frame.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    assert_eq!(sources[0].source, root);
    assert_eq!(
        snapshot.paragraphs[0].layout.request().text(),
        "external 中 paragraph"
    );
    // No root-specific or handler-specific private entry point was used.
    drop(old);
    drop(runtime);
    drop(service);
    assert_eq!(sources[0].source, root);
}
