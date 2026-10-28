//! Native retained-pixel verification using the same host pattern as tree-animation-proof.
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{WindowAttributes, WindowId},
};

#[derive(Default)]
struct Proof {
    window: Option<Arc<winit::window::Window>>,
}
impl ApplicationHandler for Proof {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Layout consumer evidence")
                        .with_inner_size(LogicalSize::new(832, 640)),
                )
                .unwrap(),
        );
        window.request_redraw();
        self.window = Some(window);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if event == WindowEvent::RedrawRequested
            && let Some(window) = self.window.take()
        {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                draw2d_official_examples::verify_layout_consumers(window);
            }))
            .is_err()
            {
                std::process::exit(1);
            }
            event_loop.exit();
        }
    }
}
fn main() {
    EventLoop::new()
        .unwrap()
        .run_app(&mut Proof::default())
        .unwrap();
}
