use gef_editor_demo::{AppState, app_view};
use xilem::{EventLoop, WindowOptions, Xilem};

fn main() -> Result<(), xilem::winit::error::EventLoopError> {
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let (driver, windows) = Xilem::new_simple(
        AppState::default(),
        app_view,
        WindowOptions::new("Novadraw — Shapes editor"),
    )
    .into_driver_and_windows(move |event| proxy.send_event(event).map_err(|error| error.0));
    novadraw_platform_masonry::host::run(
        event_loop,
        windows,
        driver,
        xilem::masonry::theme::default_property_set(),
    )
}
