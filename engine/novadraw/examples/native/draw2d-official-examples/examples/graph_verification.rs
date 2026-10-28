//! Native graph paths, labels, decorations and compound border verification.
use draw2d_official_examples::graph_verification_entries;
use novadraw_example_support::AppBuilder;

fn main() {
    AppBuilder::new("Graph verification")
        .with_app_name("draw2d-official-examples")
        .with_size(1200.0, 800.0)
        .with_runtime_scenes_boxed(graph_verification_entries())
        .with_screenshot(true)
        .run()
        .expect("graph verification");
}
