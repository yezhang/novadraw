mod bidi_block;
mod connection_and_anchor;
mod ellipse_hitting;
mod flow_layout_example;
mod grid_layout_example;
mod hello_world;
mod obstruction;
mod scroll_pane_example;
mod simple_tree_example;
mod text_examples;
mod text_interaction;
mod text_surrounding;
mod text_test_border;
mod toolbar_layout_example;
mod tree_example;
mod tree_layout;
mod uml_class_diagram;
mod zoom_example;

use novadraw::{render::BuiltinFont, runtime::Runtime};

pub use bidi_block::bidi_block_example;
pub use connection_and_anchor::connection_and_anchor;
pub use ellipse_hitting::ellipse_hitting;
pub use flow_layout_example::flow_layout_example;
pub use grid_layout_example::grid_layout_example;
pub use hello_world::hello_world;
pub use obstruction::obstruction;
pub use scroll_pane_example::scroll_pane_example;
pub use simple_tree_example::simple_tree_example;
pub use text_examples::{multi_line_label_example, text_flow_example};
pub use text_interaction::{caret_example, text_flow_large_example};
pub use text_surrounding::text_surrounding_figure_example;
pub use toolbar_layout_example::toolbar_layout_example;
pub use tree_example::tree_example;
pub use uml_class_diagram::uml_class_diagram;
pub use zoom_example::zoom_example;

fn runtime_with_fonts() -> Runtime {
    let mut runtime = Runtime::empty();
    for font in BuiltinFont::ALL {
        runtime
            .resources()
            .register_builtin_font(font)
            .expect("built-in font");
    }
    runtime
}

pub fn panel_scene(name: &str) -> Option<novadraw_example_support::panel::PanelScene> {
    Some(match name {
        "GridLayoutExample" => grid_layout_example::panel_scene(),
        "FlowLayoutExample" => flow_layout_example::panel_scene(),
        "ToolbarLayoutExample" => toolbar_layout_example::panel_scene(),
        "TreeExample" => tree_example::panel_scene(),
        _ => return None,
    })
}
