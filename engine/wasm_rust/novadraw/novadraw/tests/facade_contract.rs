use novadraw::prelude::*;

fn accepts_figure(_: &dyn Figure) {}
fn accepts_layout(_: &dyn LayoutManager) {}
fn accepts_host<T: PlatformHost>() {}

#[test]
fn prelude_supports_a_backend_neutral_scene() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 240.0)));
    let runtime = Runtime::new(tree);

    accepts_figure(&RectangleFigure::new(0.0, 0.0, 1.0, 1.0));
    accepts_layout(&FillLayout::new());
    accepts_host::<novadraw::host::HeadlessHost>();
    let _: Option<&dyn RenderBackend> = None;

    assert_eq!(
        runtime.tree().figure_bounds(root),
        Some(Rectangle::new(0.0, 0.0, 320.0, 240.0))
    );
}

#[test]
fn specialist_protocols_are_available_through_named_modules() {
    let _: Option<novadraw::geometry::Precision> = None;
    let _: Option<novadraw::graphics::Path> = None;
    let _: Option<novadraw::graphics::ImageDrawError> = None;
    let _: Option<novadraw::figure::FigureMeasurement> = None;
    let _: Option<novadraw::layout::LayoutOutput> = None;
    let _: Option<novadraw::connection::RouteOutput> = None;
    let _: Option<novadraw::event::DispatchOutcome> = None;
    let _: Option<novadraw::runtime::FramePreparation> = None;
    let _: Option<novadraw::render::RenderSubmission> = None;
    let _: Option<novadraw::editor::HandleRole> = None;
    let _: Option<novadraw::advanced::NodeState> = None;
}

#[cfg(feature = "native-vello")]
#[test]
fn native_backend_is_available_only_through_the_backend_module() {
    fn accepts_vello(_: Option<novadraw::backend::vello::VelloRenderer>) {}
    accepts_vello(None);
}

#[cfg(feature = "web-vello")]
#[test]
fn web_backend_is_available_only_through_the_backend_module() {
    fn accepts_vello(_: Option<novadraw::backend::vello::VelloRenderer>) {}
    accepts_vello(None);
}
