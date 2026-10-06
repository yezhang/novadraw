//! Native application evidence: real Vello pixels at two device scales.

#[path = "graphics-proof/pixels.rs"]
mod pixels;

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use novadraw::render::{RenderBackend, RenderCommandKind, RenderOutcome, SurfaceInfo};
use novadraw_backend_vello::VelloRenderer;
use novadraw_example_scenes::graphics::{SIZE, fixture};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

struct Proof {
    present: bool,
    window: Option<Arc<Window>>,
    deadline: Option<Instant>,
    completed: bool,
}

impl Proof {
    fn finish(&mut self, event_loop: &ActiveEventLoop) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.deadline = None;
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.capture())).is_err() {
            std::process::exit(1);
        }
        event_loop.exit();
    }

    fn capture(&self) {
        let output = PathBuf::from("target/verification/p2-g01").join(if self.present {
            "present"
        } else {
            "offscreen"
        });
        std::fs::create_dir_all(&output).unwrap();
        let window = self.window.as_ref().unwrap();
        for dpi in [1, 2] {
            let surface = SurfaceInfo {
                logical_width: SIZE.0 as f64,
                logical_height: SIZE.1 as f64,
                pixel_width: SIZE.0 * dpi,
                pixel_height: SIZE.1 * dpi,
                scale_factor: dpi as f64,
            };
            let mut renderer = VelloRenderer::new(window.clone(), surface)
                .unwrap_or_else(|error| panic!("initialize Vello renderer: {error}"));
            let mut scene = fixture();
            for phase in [
                "baseline",
                "shown",
                "moved",
                "cancelled",
                "miter-large",
                "miter-restored",
            ] {
                match phase {
                    "shown" => scene.show_feedback(),
                    "moved" => scene.move_feedback(),
                    "cancelled" => scene.cancel_feedback(),
                    "miter-large" => {
                        scene
                            .runtime
                            .point_list(scene.miter)
                            .unwrap()
                            .set_miter_limit(8.0)
                            .unwrap();
                    }
                    "miter-restored" => {
                        scene
                            .runtime
                            .point_list(scene.miter)
                            .unwrap()
                            .set_miter_limit(1.0)
                            .unwrap();
                    }
                    _ => {}
                }
                let submission = scene
                    .runtime
                    .prepare_submission(surface, renderer.capabilities())
                    .into_ready()
                    .expect("changed scene produces a frame");
                // Reject a new frame before its resources/session can be accepted.
                // The untouched frame must still be accepted afterwards.
                if phase == "shown" {
                    let mut invalid = submission.clone();
                    let command = invalid
                        .commands
                        .iter_mut()
                        .find(|c| matches!(c.kind, RenderCommandKind::FillRect { .. }))
                        .unwrap();
                    if let RenderCommandKind::FillRect { rect, .. } = &mut command.kind {
                        rect.x = f64::MAX;
                    }
                    assert!(matches!(
                        renderer.submit(&invalid),
                        RenderOutcome::InvalidGraphicsInput(_)
                    ));
                    assert!(matches!(
                        renderer.render_for_screenshot(&invalid),
                        RenderOutcome::InvalidGraphicsInput(_)
                    ));
                    let mut invalid_glyph = submission.clone();
                    let command = invalid_glyph
                        .commands
                        .iter_mut()
                        .find(|c| matches!(c.kind, RenderCommandKind::DrawGlyphRun { .. }))
                        .unwrap();
                    if let RenderCommandKind::DrawGlyphRun { run, .. } = &mut command.kind {
                        run.skew_degrees = Some(f32::NAN);
                    }
                    assert!(matches!(
                        renderer.submit(&invalid_glyph),
                        RenderOutcome::InvalidGraphicsInput(_)
                    ));
                    assert!(matches!(
                        renderer.render_for_screenshot(&invalid_glyph),
                        RenderOutcome::InvalidGraphicsInput(_)
                    ));
                    renderer
                        .screenshot(&output.join(format!("native-{dpi}-rejected.png")))
                        .unwrap();
                }
                // --present additionally exercises partial damage and surface presentation.
                let outcome = if self.present {
                    renderer.submit(&submission)
                } else {
                    renderer.render_for_screenshot(&submission)
                };
                assert_eq!(
                    outcome,
                    RenderOutcome::Presented,
                    "dpi={dpi}, phase={phase}"
                );
                scene.runtime.complete_submission(
                    submission.session_id,
                    submission.frame_id,
                    outcome,
                );
                renderer
                    .screenshot(&output.join(format!("native-{dpi}-{phase}.png")))
                    .unwrap();
            }
            // Fresh full repaint must match cancellation through partial damage.
            scene.runtime.request_full_redraw();
            let submission = scene
                .runtime
                .prepare_submission(surface, renderer.capabilities())
                .into_ready()
                .unwrap();
            assert_eq!(
                renderer.render_for_screenshot(&submission),
                RenderOutcome::Presented
            );
            renderer
                .screenshot(&output.join(format!("native-{dpi}-full.png")))
                .unwrap();
            pixels::verify(&output.join(format!("native-{dpi}-baseline.png")), dpi);
            pixels::verify_sequence(&output, dpi);
        }
        println!(
            "P2-G01 GPU pixel assertions passed at DPI 1 and 2; surface present: {}",
            self.present
        );
    }
}

impl ApplicationHandler for Proof {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Graphics contract evidence")
                        .with_inner_size(LogicalSize::new(SIZE.0, SIZE.1)),
                )
                .unwrap(),
        );
        window.request_redraw();
        self.window = Some(window);
        if self.present {
            const REDRAW_TIMEOUT: Duration = Duration::from_secs(10);
            let deadline = Instant::now() + REDRAW_TIMEOUT;
            self.deadline = Some(deadline);
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            self.finish(event_loop);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            // Catch assertion failures before they unwind across the platform callback.
            self.finish(event_loop);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            eprintln!("P2-G01 surface evidence unavailable: no window redraw within 10 seconds");
            std::process::exit(1);
        }
    }
}

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    if let [_, flag, path, dpi] = arguments.as_slice() {
        assert_eq!(flag, "--verify-image");
        pixels::verify(std::path::Path::new(path), dpi.parse().unwrap());
        println!("P2-G01 pixel assertions passed: {path}");
        return;
    }
    EventLoop::new()
        .unwrap()
        .run_app(&mut Proof {
            present: arguments.iter().any(|arg| arg == "--present"),
            window: None,
            deadline: None,
            completed: false,
        })
        .unwrap();
}
