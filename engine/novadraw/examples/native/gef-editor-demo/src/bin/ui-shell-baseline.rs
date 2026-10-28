//! Matched native UI shell for release artifact size comparison.
use xilem::{EventLoop, WidgetView, WindowOptions, Xilem, view::label};

fn app(_: &mut ()) -> impl WidgetView<()> + use<> {
    label("Shapes editor")
}

fn main() -> Result<(), xilem::winit::error::EventLoopError> {
    let event_loop = EventLoop::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let (driver, windows) = Xilem::new_simple((), app, WindowOptions::new("UI shell baseline"))
        .into_driver_and_windows(move |event| proxy.send_event(event).map_err(|error| error.0));
    novadraw_platform_masonry::host::run(
        event_loop,
        windows,
        driver,
        xilem::masonry::theme::default_property_set(),
    )
}
