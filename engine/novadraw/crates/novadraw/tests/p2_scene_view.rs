use novadraw::{
    figure::{ImageFigure, RectangleFigure},
    geometry::Rectangle,
    render::{
        BackendCapabilities, ImageData, RenderCommandKind, RenderDisposition, ResourceSync,
        SurfaceInfo,
    },
    runtime::{FramePreparation, Runtime},
    scene::{
        ASPECT_FIT_VIEW, AspectFitView, SOURCE_LOCAL_CAPTURE, SceneBindingGroup, SceneCloseReason,
        SceneClosed, SceneError, SceneSnapshot, SceneSourceId, SceneSubscriptionId,
        SceneViewBindingId, SceneViewFigure, SourceLocalCapture,
    },
};

fn producer(
    group: &SceneBindingGroup,
) -> (
    Runtime,
    SceneSourceId,
    SceneSubscriptionId,
    novadraw::runtime::ImageId,
) {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 50.0)))
        .unwrap();
    let image = runtime.resources().register_image().unwrap();
    runtime
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![255, 0, 0, 255], 1.0))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            ImageFigure::new(image).with_bounds(Rectangle::new(5.0, 5.0, 10.0, 10.0)),
        ))
        .unwrap();
    runtime.scenes().join_group(group.clone()).unwrap();
    let source = runtime.scenes().register_source(root).unwrap();
    let (subscription, _, _) = runtime
        .scenes()
        .subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())
        .unwrap();
    (runtime, source, subscription, image)
}

fn destination(
    group: &SceneBindingGroup,
    snapshot: &SceneSnapshot,
) -> (Runtime, SceneViewBindingId, novadraw::tree::FigureId) {
    let mut runtime = Runtime::empty();
    runtime.scenes().join_group(group.clone()).unwrap();
    let handle = runtime
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .unwrap();
    let figure = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(
            SceneViewFigure::new(handle).with_bounds(Rectangle::new(0.0, 0.0, 200.0, 200.0)),
        ))
        .unwrap();
    let binding = runtime
        .scenes()
        .bind_view(
            figure,
            snapshot.source(),
            snapshot.subscription(),
            snapshot.config_revision(),
        )
        .unwrap();
    (runtime, binding, root)
}
fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 200.0,
        logical_height: 200.0,
        pixel_width: 400,
        pixel_height: 400,
        scale_factor: 2.0,
    }
}
fn frame(runtime: &mut Runtime) -> novadraw::render::RenderSubmission {
    match runtime
        .frames()
        .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL)
    {
        FramePreparation::Ready(frame) => frame,
        other => panic!("expected frame, got {other:?}"),
    }
}
fn present(runtime: &mut Runtime, frame: &novadraw::render::RenderSubmission) {
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

#[test]
fn independent_destinations_import_once_and_reject_retired_delivery() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let snapshot = source.scenes().capture(subscription).unwrap();
    let (mut a, binding_a, _) = destination(&group, &snapshot);
    let (mut b, binding_b, _) = destination(&group, &snapshot);
    assert!(
        a.scenes()
            .install_view(binding_a, snapshot.clone())
            .unwrap()
    );
    assert!(
        !a.scenes()
            .install_view(binding_a, snapshot.clone())
            .unwrap()
    );
    assert!(
        b.scenes()
            .install_view(binding_b, snapshot.clone())
            .unwrap()
    );
    let installed_a = a.scenes().installed_view(binding_a).unwrap().unwrap();
    let installed_b = b.scenes().installed_view(binding_b).unwrap().unwrap();
    assert_ne!(
        installed_a.drawing().resources().ready[0].id,
        snapshot.drawing().resources().ready[0].id
    );
    assert_ne!(
        installed_a.drawing().resources().ready[0].id,
        installed_b.drawing().resources().ready[0].id
    );
    a.scenes().unbind_view_dependency(binding_a).unwrap();
    assert_eq!(
        a.scenes().install_view(binding_a, snapshot.clone()),
        Err(SceneError::StaleSceneDelivery)
    );
    assert!(b.scenes().installed_view(binding_b).unwrap().is_some());
    let ready = frame(&mut b);
    assert!(
        ready
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
    );
    present(&mut b, &ready);
}

#[test]
fn same_epoch_new_configuration_updates_without_resetting_epoch_baseline() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let first = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, _) = destination(&group, &first);
    target
        .scenes()
        .install_view(binding, first.clone())
        .unwrap();
    let config = source
        .scenes()
        .configure(
            subscription,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture {
                rectangle: Some(Rectangle::new(3.0, 5.0, 30.0, 10.0)),
            },
        )
        .unwrap();
    let second = source.scenes().capture(subscription).unwrap();
    assert_eq!(first.stable_source_epoch(), second.stable_source_epoch());
    assert_eq!(
        target.scenes().install_view(binding, second.clone()),
        Err(SceneError::StaleSceneDelivery)
    );
    target.scenes().expect_view_config(binding, config).unwrap();
    target
        .scenes()
        .install_view(binding, second.clone())
        .unwrap();
    assert_eq!(
        target.scenes().install_view(binding, first),
        Err(SceneError::StaleSceneDelivery)
    );
    let installed = target.scenes().installed_view(binding).unwrap().unwrap();
    assert_eq!(
        installed.snapshot().source_rectangle(),
        second.source_rectangle()
    );
}

#[test]
fn old_and_new_resource_versions_coexist_and_survive_inflight_reset() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, image) = producer(&group);
    let first = source.scenes().capture(subscription).unwrap();
    let (mut target, a, root) = destination(&group, &first);
    target.scenes().install_view(a, first.clone()).unwrap();
    let handle = target
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let figure_b = target
        .figure(root)
        .unwrap()
        .add(Box::new(
            SceneViewFigure::new(handle).with_bounds(Rectangle::new(100.0, 100.0, 50.0, 50.0)),
        ))
        .unwrap();
    let b = target
        .scenes()
        .bind_view(
            figure_b,
            first.source(),
            subscription,
            first.config_revision(),
        )
        .unwrap();
    target.scenes().install_view(b, first).unwrap();
    let old = target.scenes().installed_view(a).unwrap().unwrap();
    let old_frame = frame(&mut target);
    source
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 255, 255], 1.0))
        .unwrap();
    let second = source.scenes().capture(subscription).unwrap();
    target.scenes().install_view(b, second).unwrap();
    let new = target.scenes().installed_view(b).unwrap().unwrap();
    let old_id = old.drawing().resources().ready[0].id;
    let new_id = new.drawing().resources().ready[0].id;
    assert_ne!(old_id, new_id);
    assert_eq!(old.drawing().resources().ready[0].revision, 1);
    assert_eq!(new.drawing().resources().ready[0].revision, 1);
    target.frames().reset_backend_session().unwrap();
    assert!(
        target
            .frames()
            .complete_submission(
                old_frame.session_id,
                old_frame.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .is_err()
    );
    let recovered = frame(&mut target);
    let ResourceSync::Snapshot(resources) = &recovered.resources else {
        panic!("session recovery baseline");
    };
    assert!(resources.ready.iter().any(|update| update.id == old_id));
    assert!(resources.ready.iter().any(|update| update.id == new_id));
    present(&mut target, &recovered);
}

#[test]
fn source_disposal_clears_target_while_suspended_and_ready_cannot_restore_it() {
    let group = SceneBindingGroup::default();
    let (mut source, source_id, subscription, _) = producer(&group);
    let snapshot = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, _) = destination(&group, &snapshot);
    target
        .scenes()
        .install_view(binding, snapshot.clone())
        .unwrap();
    let ready = frame(&mut target);
    present(&mut target, &ready);
    source.scenes().remove_source(source_id).unwrap();
    target
        .scenes()
        .close_view(
            binding,
            SceneClosed {
                source: source_id,
                subscription,
                reason: SceneCloseReason::SourceRemoved,
            },
        )
        .unwrap();
    assert!(matches!(
        target.scenes().installed_view(binding),
        Err(SceneError::StaleSceneDelivery)
    ));
    assert_eq!(
        target.scenes().install_view(binding, snapshot),
        Err(SceneError::StaleSceneDelivery)
    );
    let mut suspended = surface();
    suspended.pixel_width = 0;
    assert!(matches!(
        target
            .frames()
            .prepare_submission(suspended, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Suspended
    ));
    let cleared = frame(&mut target);
    assert!(cleared.damage.is_full());
    assert!(
        !cleared
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
    );
}

#[test]
fn capture_evidence_is_rechecked_against_current_source_topology() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let old = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, _) = destination(&group, &old);
    let root = source.tree().contents().unwrap();
    source
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(20.0, 20.0, 5.0, 5.0)))
        .unwrap();
    assert!(matches!(
        target.scenes().install_view(binding, old),
        Err(SceneError::Admission(
            novadraw::scene::SceneAdmissionError::StaleSceneAdmission
        ))
    ));
    assert!(target.scenes().installed_view(binding).unwrap().is_none());
    let fresh = source.scenes().capture(subscription).unwrap();
    target.scenes().install_view(binding, fresh).unwrap();
}

#[test]
fn retry_rebuilds_current_view_and_keeps_unacknowledged_session_baseline() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, image) = producer(&group);
    let first = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, _) = destination(&group, &first);
    target.scenes().install_view(binding, first).unwrap();
    let old_id = target
        .scenes()
        .installed_view(binding)
        .unwrap()
        .unwrap()
        .drawing()
        .resources()
        .ready[0]
        .id;
    let old_frame = frame(&mut target);
    source
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![0, 0, 255, 255], 1.0))
        .unwrap();
    let latest = source.scenes().capture(subscription).unwrap();
    target.scenes().install_view(binding, latest).unwrap();
    let new_id = target
        .scenes()
        .installed_view(binding)
        .unwrap()
        .unwrap()
        .drawing()
        .resources()
        .ready[0]
        .id;
    assert_eq!(
        target
            .frames()
            .complete_submission(
                old_frame.session_id,
                old_frame.frame_id,
                Ok(RenderDisposition::Retry)
            )
            .unwrap(),
        novadraw::runtime::SubmissionAcknowledgement::Retry
    );
    let retry = frame(&mut target);
    assert!(retry.damage.is_full());
    assert!(retry.commands.iter().any(|command| matches!(command.kind, RenderCommandKind::Image { image, .. } if image.resource_id() == new_id)));
    assert!(!retry.commands.iter().any(|command| matches!(command.kind, RenderCommandKind::Image { image, .. } if image.resource_id() == old_id)));
    let ResourceSync::Snapshot(resources) = &retry.resources else {
        panic!("unacknowledged initial session remains a full baseline");
    };
    assert!(resources.ready.iter().any(|update| update.id == new_id));
    present(&mut target, &retry);
}

#[test]
fn font_references_are_rewritten_and_source_revisions_use_distinct_ready_ids() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let font = source
        .resources()
        .register_builtin_font(novadraw::render::BuiltinFont::Inter)
        .unwrap();
    let root = source.tree().contents().unwrap();
    source
        .figure(root)
        .unwrap()
        .add(Box::new(novadraw::figure::LabelFigure::new(
            "frozen glyphs",
        )))
        .unwrap();
    let first = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, _) = destination(&group, &first);
    target
        .scenes()
        .install_view(binding, first.clone())
        .unwrap();
    let old = target.scenes().installed_view(binding).unwrap().unwrap();
    let old_font = old
        .drawing()
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::DrawGlyphRun { run, .. } => Some(run.font.clone()),
            _ => None,
        })
        .unwrap();
    assert_ne!(old_font.resource_id(), font.resource_id());
    source
        .resources()
        .complete_font(
            font,
            novadraw::render::FontData::new(novadraw::render::BuiltinFont::Inter.bytes().to_vec()),
        )
        .unwrap();
    assert!(source.scenes().needs_capture(subscription).unwrap());
    let second = source.scenes().capture(subscription).unwrap();
    target.scenes().install_view(binding, second).unwrap();
    let new = target.scenes().installed_view(binding).unwrap().unwrap();
    let new_font = new
        .drawing()
        .commands()
        .iter()
        .find_map(|command| match &command.kind {
            RenderCommandKind::DrawGlyphRun { run, .. } => Some(run.font.clone()),
            _ => None,
        })
        .unwrap();
    assert_ne!(old_font.resource_id(), new_font.resource_id());
    assert_eq!(old_font.revision(), 1);
    assert_eq!(new_font.revision(), 1);
    let ready = frame(&mut target);
    let ResourceSync::Snapshot(resources) = &ready.resources else {
        panic!("initial snapshot");
    };
    assert!(
        resources
            .ready
            .iter()
            .any(|resource| resource.id == old_font.resource_id())
    );
    assert!(
        resources
            .ready
            .iter()
            .any(|resource| resource.id == new_font.resource_id())
    );
    assert_eq!(
        target.resources().remove(old_font.resource_id()),
        Err(novadraw::runtime::ResourceError::ImmutableImport)
    );
}

const EXTERNAL_POLICY: novadraw::scene::SceneViewPolicyKey<bool> =
    novadraw::scene::SceneViewPolicyKey::new("external.consumer", "checked-display");

#[test]
fn target_opacity_composes_with_frozen_source_opacity_without_mutating_snapshot() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let source_root = source.tree().contents().unwrap();
    source
        .figure(source_root)
        .unwrap()
        .set_style(novadraw::figure::FigureStyle {
            alpha: Some(novadraw::graphics::Opacity::try_new(0.8).unwrap()),
            ..Default::default()
        })
        .unwrap();
    let snapshot = source.scenes().capture(subscription).unwrap();
    let (mut target, binding, root) = destination(&group, &snapshot);
    target
        .figure(root)
        .unwrap()
        .set_style(novadraw::figure::FigureStyle {
            alpha: Some(novadraw::graphics::Opacity::try_new(0.5).unwrap()),
            ..Default::default()
        })
        .unwrap();
    target
        .scenes()
        .install_view(binding, snapshot.clone())
        .unwrap();
    let ready = frame(&mut target);
    assert!(ready.commands.iter().any(|command| matches!(command.kind, RenderCommandKind::Image { alpha, .. } if (alpha - 0.4).abs() < 1e-9)));
    assert!(snapshot.drawing().commands().iter().any(|command| matches!(command.kind, RenderCommandKind::Image { alpha, .. } if (alpha - 0.8).abs() < 1e-9)));
}

#[test]
fn rebinding_same_source_retires_identity_without_resetting_epoch_baseline() {
    let group = SceneBindingGroup::default();
    let (mut source, source_id, subscription, image) = producer(&group);
    let first = source.scenes().capture(subscription).unwrap();
    let mut target = Runtime::empty();
    target.scenes().join_group(group.clone()).unwrap();
    let handle = target
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let view = target
        .set_contents(Box::new(SceneViewFigure::new(handle)))
        .unwrap();
    let a = target
        .scenes()
        .bind_view(view, source_id, subscription, first.config_revision())
        .unwrap();
    target.scenes().install_view(a, first.clone()).unwrap();
    source
        .resources()
        .complete_image(image, ImageData::from_rgba(1, 1, vec![0; 4], 1.0))
        .unwrap();
    let latest = source.scenes().capture(subscription).unwrap();
    target.scenes().install_view(a, latest.clone()).unwrap();
    let b = target
        .scenes()
        .bind_view(view, source_id, subscription, first.config_revision())
        .unwrap();
    assert_ne!(a, b);
    assert!(target.scenes().installed_view(b).unwrap().is_none());
    assert_eq!(
        target.scenes().install_view(a, latest.clone()),
        Err(SceneError::StaleSceneDelivery)
    );
    assert_eq!(
        target.scenes().install_view(b, first),
        Err(SceneError::StaleSceneDelivery)
    );
    target.scenes().install_view(b, latest).unwrap();
}

struct ViewMeasurementProbe {
    view: novadraw::tree::FigureId,
    observed: std::sync::Arc<std::sync::Mutex<Vec<novadraw::geometry::Dimension>>>,
}
impl novadraw::layout::LayoutManager for ViewMeasurementProbe {
    fn preferred_measurement(
        &self,
        _: novadraw::tree::FigureId,
        constraints: novadraw::figure::MeasureConstraints,
        query: &novadraw::layout::LayoutSceneQuery<'_>,
    ) -> Result<novadraw::figure::FigureMeasurement, novadraw::layout::LayoutError> {
        query.preferred_measurement(self.view, constraints)
    }
    fn minimum_size(
        &self,
        _: novadraw::tree::FigureId,
        constraints: novadraw::figure::MeasureConstraints,
        query: &novadraw::layout::LayoutSceneQuery<'_>,
    ) -> Result<novadraw::geometry::Dimension, novadraw::layout::LayoutError> {
        query.minimum_size(self.view, constraints)
    }
    fn layout(
        &mut self,
        _: novadraw::tree::FigureId,
        query: &novadraw::layout::LayoutSceneQuery<'_>,
        _: &mut novadraw::layout::LayoutOutput,
    ) -> Result<(), novadraw::layout::LayoutError> {
        use novadraw::figure::MeasureConstraints;
        let mut observed = self.observed.lock().unwrap();
        observed.clear();
        for constraints in [
            MeasureConstraints::UNBOUNDED,
            MeasureConstraints::width(80.0).unwrap(),
            MeasureConstraints::height(40.0).unwrap(),
            MeasureConstraints::bounded(80.0, 40.0).unwrap(),
        ] {
            observed.push(query.preferred_measurement(self.view, constraints)?.size());
        }
        observed.push(query.minimum_size(self.view, MeasureConstraints::UNBOUNDED)?);
        Ok(())
    }
}
#[test]
fn border_insets_are_applied_once_during_authoritative_view_measurement() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let snapshot = source.scenes().capture(subscription).unwrap();
    let mut target = Runtime::empty();
    target.scenes().join_group(group.clone()).unwrap();
    let root = target
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 200.0, 200.0)))
        .unwrap();
    let handle = target
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .unwrap();
    let border = novadraw::figure::border::MarginBorder::new(novadraw::graphics::Color::BLACK, 0.0)
        .with_margins(10.0, 10.0, 10.0, 10.0);
    let view = target
        .figure(root)
        .unwrap()
        .add(Box::new(
            SceneViewFigure::new(handle)
                .with_bounds(Rectangle::new(0.0, 0.0, 200.0, 200.0))
                .with_border(border),
        ))
        .unwrap();
    let binding = target
        .scenes()
        .bind_view(
            view,
            snapshot.source(),
            subscription,
            snapshot.config_revision(),
        )
        .unwrap();
    target.scenes().install_view(binding, snapshot).unwrap();
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    target
        .figure(root)
        .unwrap()
        .set_layout_manager(Box::new(ViewMeasurementProbe {
            view,
            observed: observed.clone(),
        }))
        .unwrap();
    let ready = frame(&mut target);
    assert_eq!(
        *observed.lock().unwrap(),
        vec![
            novadraw::geometry::Dimension::new(120.0, 70.0),
            novadraw::geometry::Dimension::new(80.0, 50.0),
            novadraw::geometry::Dimension::new(60.0, 40.0),
            novadraw::geometry::Dimension::new(60.0, 40.0),
            novadraw::geometry::Dimension::new(20.0, 20.0),
        ]
    );
    assert!(ready.commands.iter().any(|command| matches!(command.kind,
        RenderCommandKind::Clip { rect } if rect == Rectangle::new(10.0, 10.0, 180.0, 180.0))));
}
struct ExternalPolicy;
impl novadraw::scene::SceneViewPolicy<bool> for ExternalPolicy {
    fn measure(
        &self,
        fail: &bool,
        natural: novadraw::geometry::Dimension,
        _: novadraw::figure::MeasureConstraints,
    ) -> Result<novadraw::figure::PreparationMeasurement, SceneError> {
        if *fail {
            return Err(SceneError::Policy("external measurement rejected".into()));
        }
        Ok(novadraw::figure::PreparationMeasurement {
            preferred: novadraw::figure::FigureMeasurement::new(
                natural.width,
                natural.height,
                None,
            ),
            minimum: novadraw::geometry::Dimension::ZERO,
        })
    }
    fn place(
        &self,
        _: &bool,
        _: novadraw::geometry::Dimension,
        client: Rectangle,
    ) -> Result<novadraw::scene::SceneViewPlacement, SceneError> {
        Ok(novadraw::scene::SceneViewPlacement {
            transform: novadraw::geometry::Affine2D::from_translation(client.x, client.y),
            clip: client,
        })
    }
}
struct ExternalFigure(novadraw::scene::SceneViewHandle);
impl novadraw::scene::SceneViewConsumer for ExternalFigure {
    fn scene_view(&self) -> &novadraw::scene::SceneViewHandle {
        &self.0
    }
}
impl novadraw::figure::Figure for ExternalFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 100.0, 50.0)
    }
    fn name(&self) -> &'static str {
        "ExternalSharedSceneFigure"
    }
    fn register_capabilities(
        &self,
        out: &mut novadraw::figure::FigureCapabilityBuilder,
    ) -> Result<(), novadraw::figure::FigureCapabilityRegistrationError> {
        out.register(
            novadraw::scene::SCENE_VIEW,
            novadraw::scene::SceneViewCapability::of::<Self>(),
        )?;
        out.register(
            novadraw::figure::PREPARATION,
            novadraw::figure::PreparationCapability::of::<Self>(),
        )
    }
}
impl novadraw::figure::FigurePreparation for ExternalFigure {
    fn revision(&self) -> u64 {
        self.0.revision().unwrap()
    }
    fn measure_scene(
        &self,
        _: &mut novadraw::text::MeasureContext<'_>,
        constraints: novadraw::figure::MeasureConstraints,
        scene: &novadraw::figure::PreparationScene<'_>,
    ) -> Result<novadraw::figure::PreparationMeasurement, novadraw::figure::PreparationMeasureError>
    {
        self.0
            .measure(constraints)
            .map_err(|error| novadraw::figure::PreparationMeasureError {
                figure: scene.root(),
                outer_constraints: constraints,
                content_constraints: Some(constraints),
                kind: novadraw::figure::PreparationMeasureErrorKind::Text(Box::new(
                    novadraw::text::TextError::SceneView(error.to_string()),
                )),
            })
    }
    fn prepare(
        &self,
        _: &mut novadraw::text::MeasureContext<'_>,
        client: Rectangle,
    ) -> Result<novadraw::figure::FigurePresentation, novadraw::text::TextError> {
        self.0.prepare(client)
    }
}

#[test]
fn external_figure_and_policy_share_admission_error_and_recovery_paths() {
    let group = SceneBindingGroup::default();
    let (mut source, _, subscription, _) = producer(&group);
    let snapshot = source.scenes().capture(subscription).unwrap();
    let mut target = Runtime::empty();
    target.scenes().join_group(group.clone()).unwrap();
    target
        .scenes()
        .register_view_policy(EXTERNAL_POLICY, ExternalPolicy)
        .unwrap();
    let handle = target.scenes().create_view(EXTERNAL_POLICY, false).unwrap();
    let view = target
        .set_contents(Box::new(ExternalFigure(handle)))
        .unwrap();
    let binding = target
        .scenes()
        .bind_view(
            view,
            snapshot.source(),
            subscription,
            snapshot.config_revision(),
        )
        .unwrap();
    target
        .scenes()
        .install_view(binding, snapshot.clone())
        .unwrap();
    let ready = frame(&mut target);
    present(&mut target, &ready);
    target
        .scenes()
        .configure_view(binding, EXTERNAL_POLICY, true)
        .unwrap();
    assert!(matches!(
        target
            .frames()
            .prepare_submission(surface(), BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(_)
    ));
    assert_eq!(
        *target
            .scenes()
            .installed_view(binding)
            .unwrap()
            .unwrap()
            .snapshot(),
        *snapshot
    );
    target
        .scenes()
        .configure_view(binding, EXTERNAL_POLICY, false)
        .unwrap();
    let recovered = frame(&mut target);
    assert!(recovered.damage.is_full());
    assert!(
        recovered
            .commands
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::Image { .. }))
    );
}
