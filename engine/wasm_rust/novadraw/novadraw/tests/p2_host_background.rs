//! External provider uses the public registry and the same completion path as the default.
use novadraw::{
    figure::RectangleFigure,
    geometry::{Affine2D, Rectangle},
    graphics::Color,
    host::{
        BackgroundFrameInput, BackgroundInputVersions, HostBackgroundContent,
        HostBackgroundError as Error, HostBackgroundId, HostBackgroundProvider,
        HostBackgroundProviderKey, HostBackgroundRegistry, HostBackgroundRequest,
        HostBackgroundSnapshot, RECORDED_BACKGROUND,
    },
    render::{
        BackendCapabilities, CommandRecorder, RenderCommandKind, RenderDisposition,
        RenderSubmission, ResourceSync, SurfaceInfo,
    },
    runtime::{FramePreparation, FramePreparationError, Runtime},
};

const CUSTOM: HostBackgroundProviderKey<Color> =
    HostBackgroundProviderKey::new("test.host", "color");
const CAPS: BackendCapabilities = BackendCapabilities::RETAINED_PARTIAL.with_host_background();
struct Provider;
impl HostBackgroundProvider<Color> for Provider {
    fn provide(
        &self,
        color: &Color,
        _: HostBackgroundRequest,
    ) -> Result<HostBackgroundContent, Error> {
        Ok(content(*color))
    }
}
fn content(color: Color) -> HostBackgroundContent {
    let mut recorder = CommandRecorder::new();
    let mut g = recorder.paint();
    g.set_fill_paint(color);
    g.fill_rect(Rectangle::new(0.0, 0.0, 300.0, 300.0)).unwrap();
    drop(g);
    HostBackgroundContent {
        coverage: Rectangle::new(0.0, 0.0, 300.0, 300.0),
        drawing: recorder.finish().unwrap(),
    }
}
fn input() -> BackgroundFrameInput {
    BackgroundFrameInput {
        surface: SurfaceInfo {
            logical_width: 100.0,
            logical_height: 100.0,
            pixel_width: 200,
            pixel_height: 200,
            scale_factor: 2.0,
        },
        versions: BackgroundInputVersions {
            surface: 1,
            position: 1,
            dpi: 1,
        },
        host_to_canvas: Affine2D::from_translation(-10.0, -20.0),
    }
}
fn runtime() -> Runtime {
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            5.0,
            5.0,
            10.0,
            10.0,
            Color::BLACK,
        )))
        .unwrap();
    runtime
}
fn install(
    runtime: &mut Runtime,
    source: HostBackgroundId,
    input: BackgroundFrameInput,
    revision: u64,
    color: Color,
) -> HostBackgroundSnapshot {
    let request = runtime
        .frames()
        .begin_background(source, input, Color::WHITE)
        .unwrap();
    let mut registry = HostBackgroundRegistry::default();
    registry.register(CUSTOM, Provider).unwrap();
    let snapshot = registry.provide(CUSTOM, &color, request, revision).unwrap();
    runtime
        .frames()
        .complete_background(request, Ok(snapshot.clone()))
        .unwrap();
    snapshot
}
fn frame(runtime: &mut Runtime, input: BackgroundFrameInput) -> RenderSubmission {
    runtime
        .frames()
        .prepare_submission(input.surface, CAPS)
        .into_ready()
        .unwrap()
}
fn complete(runtime: &mut Runtime, frame: &RenderSubmission, disposition: RenderDisposition) {
    let acknowledgement = runtime
        .frames()
        .complete_submission(frame.session_id, frame.frame_id, Ok(disposition))
        .unwrap();
    assert_eq!(
        acknowledgement,
        match disposition {
            RenderDisposition::Presented => novadraw::runtime::SubmissionAcknowledgement::Presented,
            RenderDisposition::Retry => novadraw::runtime::SubmissionAcknowledgement::Retry,
            RenderDisposition::Skipped => novadraw::runtime::SubmissionAcknowledgement::Skipped,
        }
    );
}

#[test]
fn background_only_update_wakes_and_freezes_before_canvas_and_new_update_survives_presented() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Wake(AtomicUsize);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let mut runtime = runtime();
    let wake = Arc::new(Wake(AtomicUsize::new(0)));
    runtime.frames().set_background_waker(wake.clone().into());
    let source = HostBackgroundId::new();
    install(&mut runtime, source, input(), 1, Color::WHITE);
    let frozen = frame(&mut runtime, input());
    assert_eq!(frozen.background_input.as_deref(), Some(&input()));
    assert!(frozen.damage.is_full());
    let colors: Vec<_> = frozen
        .commands
        .iter()
        .filter_map(|c| match c.kind {
            RenderCommandKind::FillRect { ref paint, .. } => Some(paint.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(colors, vec![Color::WHITE.into(), Color::BLACK.into()]);
    install(&mut runtime, source, input(), 2, Color::BLACK);
    assert!(matches!(
        runtime.frames().prepare_submission(input().surface, CAPS),
        FramePreparation::AwaitingCompletion
    ));
    complete(&mut runtime, &frozen, RenderDisposition::Presented);
    assert!(runtime.frames().has_pending_update());
    let changed = frame(&mut runtime, input());
    assert_ne!(frozen.commands, changed.commands);
    assert!(wake.0.load(Ordering::SeqCst) >= 3);
    complete(&mut runtime, &changed, RenderDisposition::Presented);
    assert!(matches!(
        runtime.frames().prepare_submission(input().surface, CAPS),
        FramePreparation::Idle
    ));
}

#[test]
fn pending_failure_removal_and_late_completion_keep_explicit_clear() {
    let mut runtime = runtime();
    let request = runtime
        .frames()
        .begin_background(HostBackgroundId::new(), input(), Color::TRANSPARENT)
        .unwrap();
    let pending = frame(&mut runtime, input());
    assert_eq!(pending.clear_color, Some(Color::TRANSPARENT));
    complete(&mut runtime, &pending, RenderDisposition::Presented);
    let error = Error::Provider("offline".into());
    assert_eq!(
        runtime
            .frames()
            .complete_background(request, Err(error.clone())),
        Err(error.clone())
    );
    assert_eq!(runtime.frames().background_error(), Some(&error));
    assert!(runtime.frames().current_background().is_none());
    let snapshot = HostBackgroundRegistry::default()
        .provide(RECORDED_BACKGROUND, &content(Color::WHITE), request, 1)
        .unwrap();
    runtime
        .frames()
        .complete_background(request, Ok(snapshot.clone()))
        .unwrap();
    let accepted = frame(&mut runtime, input());
    complete(&mut runtime, &accepted, RenderDisposition::Presented);
    let request2 = runtime
        .frames()
        .begin_background(request.source(), input(), Color::WHITE)
        .unwrap();
    assert_eq!(
        runtime.frames().complete_background(request2, Err(error)),
        Err(Error::Provider("offline".into()))
    );
    assert_eq!(runtime.frames().current_background(), Some(&snapshot));
    runtime.frames().remove_background(Color::BLACK).unwrap();
    assert_eq!(
        runtime.frames().complete_background(request, Ok(snapshot)),
        Err(Error::StaleRequest)
    );
    let removed = frame(&mut runtime, input());
    assert_eq!(removed.clear_color, Some(Color::BLACK));
    assert!(removed.damage.is_full());
    assert_eq!(
        removed
            .commands
            .iter()
            .filter(|c| matches!(c.kind, RenderCommandKind::FillRect { .. }))
            .count(),
        1
    );
}

#[test]
fn movement_and_dpi_mismatch_reject_mixed_frame_until_new_candidate() {
    let mut runtime = runtime();
    let source = HostBackgroundId::new();
    let old = install(&mut runtime, source, input(), 1, Color::WHITE);
    let initial = frame(&mut runtime, input());
    complete(&mut runtime, &initial, RenderDisposition::Presented);
    let mut moved = input();
    moved.versions.position += 1;
    moved.host_to_canvas = Affine2D::from_translation(-40.0, -60.0);
    runtime.frames().set_background_input(moved).unwrap();
    assert!(matches!(
        runtime.frames().prepare_submission(moved.surface, CAPS),
        FramePreparation::Error(FramePreparationError::HostBackground(Error::InputMismatch))
    ));
    assert!(runtime.frames().has_pending_update());
    assert_eq!(
        runtime.frames().complete_background(old.request(), Ok(old)),
        Err(Error::InputMismatch)
    );
    install(&mut runtime, source, moved, 2, Color::WHITE);
    let moving = frame(&mut runtime, moved);
    assert!(moving.commands.iter().any(|c| matches!(c.kind,
        RenderCommandKind::SetTransform { matrix } if matrix == moved.host_to_canvas)));
    complete(&mut runtime, &moving, RenderDisposition::Presented);
    let mut dpi = moved;
    dpi.surface.scale_factor = 1.0;
    dpi.surface.pixel_width = 100;
    dpi.surface.pixel_height = 100;
    assert_eq!(
        runtime.frames().set_background_input(dpi),
        Err(Error::StaleInput)
    );
    dpi.versions.surface += 1;
    dpi.versions.dpi += 1;
    runtime.frames().set_background_input(dpi).unwrap();
    assert!(matches!(
        runtime.frames().prepare_submission(dpi.surface, CAPS),
        FramePreparation::Error(FramePreparationError::HostBackground(Error::InputMismatch))
    ));
    install(&mut runtime, source, dpi, 3, Color::WHITE);
    let ready = frame(&mut runtime, dpi);
    assert_eq!(ready.background_input.as_deref(), Some(&dpi));
    assert_eq!(
        runtime.frames().set_background_input(input()),
        Err(Error::StaleInput)
    );
}

#[test]
fn duplicate_conflict_and_foreign_request_do_not_replace_current_background() {
    let mut runtime = runtime();
    let source = HostBackgroundId::new();
    let accepted = install(&mut runtime, source, input(), 1, Color::WHITE);
    runtime
        .frames()
        .complete_background(accepted.request(), Ok(accepted.clone()))
        .unwrap();
    let conflict =
        HostBackgroundSnapshot::new(accepted.request(), 1, content(Color::BLACK)).unwrap();
    assert_eq!(
        runtime
            .frames()
            .complete_background(accepted.request(), Ok(conflict)),
        Err(Error::SnapshotConflict)
    );
    assert_eq!(runtime.frames().current_background(), Some(&accepted));
    let foreign = Runtime::empty()
        .frames()
        .begin_background(source, input(), Color::WHITE)
        .unwrap();
    assert_eq!(
        runtime
            .frames()
            .complete_background(foreign, Err(Error::Provider("late".into()))),
        Err(Error::StaleRequest)
    );
}

#[test]
fn retry_and_reset_preserve_frozen_resource_payloads_and_reject_old_token() {
    use novadraw::{
        geometry::Point,
        graphics::Graphics,
        render::BuiltinFont,
        text::{TextConstraints, TextSystem},
    };
    let mut text = TextSystem::new();
    text.register_builtin_font(BuiltinFont::Inter).unwrap();
    let mut recorder = CommandRecorder::new();
    let mut g = Graphics::new(&mut text, &mut recorder);
    g.set_fill_paint(Color::BLACK);
    let layout = g
        .layout_text("Host background", TextConstraints::UNBOUNDED)
        .unwrap();
    g.fill_text(&layout, Point::ZERO).unwrap();
    drop(g);
    let drawing = recorder.finish().unwrap();
    assert!(!drawing.resources().ready.is_empty());
    let mut runtime = runtime();
    let request = runtime
        .frames()
        .begin_background(HostBackgroundId::new(), input(), Color::WHITE)
        .unwrap();
    let snapshot = HostBackgroundSnapshot::new(
        request,
        1,
        HostBackgroundContent {
            coverage: content(Color::WHITE).coverage,
            drawing,
        },
    )
    .unwrap();
    runtime
        .frames()
        .complete_background(request, Ok(snapshot))
        .unwrap();
    let first = frame(&mut runtime, input());
    let imported = match &first.resources {
        ResourceSync::Snapshot(s) => s.ready[0].id,
        _ => panic!("initial snapshot"),
    };
    complete(&mut runtime, &first, RenderDisposition::Retry);
    let retry = frame(&mut runtime, input());
    assert_eq!(first.commands, retry.commands);
    runtime.frames().reset_backend_session().unwrap();
    assert!(
        runtime
            .frames()
            .complete_submission(
                retry.session_id,
                retry.frame_id,
                Ok(RenderDisposition::Presented)
            )
            .is_err()
    );
    let reset = frame(&mut runtime, input());
    let ResourceSync::Snapshot(s) = &reset.resources else {
        panic!("reset snapshot")
    };
    assert!(s.ready.iter().any(|r| r.id == imported));
    assert_eq!(retry.commands, reset.commands);
    assert!(reset.damage.is_full());
}

#[test]
fn incomplete_resource_closure_rejects_without_partial_import() {
    let mut recorder = CommandRecorder::new();
    recorder
        .paint()
        .draw_image(
            novadraw::render::ImageResourceRef::new(
                novadraw::render::ResourceId::new(uuid::Uuid::nil(), 1),
                1,
                1,
                1,
                1.0,
            ),
            Rectangle::new(0.0, 0.0, 1.0, 1.0),
        )
        .unwrap();
    let mut runtime = runtime();
    let source = HostBackgroundId::new();
    let accepted = install(&mut runtime, source, input(), 1, Color::WHITE);
    let request = runtime
        .frames()
        .begin_background(source, input(), Color::WHITE)
        .unwrap();
    let broken = HostBackgroundSnapshot::new(
        request,
        2,
        HostBackgroundContent {
            coverage: content(Color::WHITE).coverage,
            drawing: recorder.finish().unwrap(),
        },
    )
    .unwrap();
    assert!(matches!(
        runtime.frames().complete_background(request, Ok(broken)),
        Err(Error::Import(_))
    ));
    assert_eq!(runtime.frames().current_background(), Some(&accepted));
}

#[test]
fn unsupported_backend_and_modified_composition_are_rejected_before_submission() {
    let mut runtime = runtime();
    install(
        &mut runtime,
        HostBackgroundId::new(),
        input(),
        1,
        Color::WHITE,
    );
    assert!(matches!(
        runtime
            .frames()
            .prepare_submission(input().surface, BackendCapabilities::RETAINED_PARTIAL),
        FramePreparation::Error(FramePreparationError::UnsupportedRenderCapability(
            novadraw::render::UnsupportedRenderCapability {
                capability: novadraw::render::RenderCapability::HostBackground
            }
        ))
    ));
    let mut ready = frame(&mut runtime, input());
    assert!(
        ready
            .validate_composition(BackendCapabilities::RETAINED_PARTIAL)
            .is_err()
    );
    ready.background_input.as_mut().unwrap().surface.pixel_width += 1;
    assert!(ready.validate_composition(CAPS).is_err());
    ready.background_input = Some(Box::new(input()));
    ready.damage.set_union(Rectangle::new(0.0, 0.0, 1.0, 1.0));
    assert!(ready.validate_composition(CAPS).is_err());
}
