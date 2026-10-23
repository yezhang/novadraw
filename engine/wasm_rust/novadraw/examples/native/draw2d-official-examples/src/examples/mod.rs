mod bidi_block;
mod connection_and_anchor;
mod ellipse_hitting;
mod flow_layout_example;
mod graph_examples;
mod grid_layout_example;
mod hello_world;
mod obstruction;
#[cfg(test)]
mod panel_test;
mod path_example;
mod printing;
mod scroll_pane_example;
mod simple_tree_example;
mod text_examples;
mod text_interaction;
mod text_surrounding;
mod text_test_border;
mod thumbnail;
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
pub use graph_examples::{
    compound_graph, directed_graph, verification_entries as graph_verification_entries,
};
pub use grid_layout_example::grid_layout_example;
pub use hello_world::hello_world;
pub use obstruction::obstruction;
pub use path_example::path_example;
pub use printing::print_example;
pub use scroll_pane_example::scroll_pane_example;
pub use simple_tree_example::simple_tree_example;
pub use text_examples::{multi_line_label_example, text_flow_example};
pub use text_interaction::{caret_example, text_flow_large_example};
pub use text_surrounding::text_surrounding_figure_example;
pub use thumbnail::thumbnail_example;
pub use thumbnail::verification_entries as thumbnail_verification_entries;
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

/// Normal BUTTON_SCROLLBAR scheme; the final reserved row/column follows the
/// source ButtonScheme inset calculation. System colors use a portable palette.
fn scrollbar_label_border() -> novadraw::figure::border::CompoundBorder {
    use novadraw::{
        figure::border::{CompoundBorder, MarginBorder, SchemeBorder},
        graphics::Color,
    };
    CompoundBorder::new(
        SchemeBorder::new(
            vec![Some(Color::rgba(0.94, 0.94, 0.94, 1.0)), Some(Color::WHITE)],
            vec![Some(Color::BLACK), Some(Color::rgba(0.5, 0.5, 0.5, 1.0))],
        ),
        MarginBorder::new(Color::TRANSPARENT, 0.0).with_margins(0.0, 0.0, 1.0, 1.0),
    )
}

pub fn panel_scene(name: &str) -> Option<novadraw_example_support::panel::PanelScene> {
    Some(match name {
        "GridLayoutExample" => grid_layout_example::panel_scene(),
        "FlowLayoutExample" => flow_layout_example::panel_scene(),
        "ToolbarLayoutExample" => toolbar_layout_example::panel_scene(),
        "TreeExample" => tree_example::panel_scene(),
        "DirectedGraphDemo" => graph_examples::panel_scene(false),
        "CompoundGraphDemo" => graph_examples::panel_scene(true),
        "ThumbnailExample" => thumbnail::panel_scene(),
        _ => return None,
    })
}
