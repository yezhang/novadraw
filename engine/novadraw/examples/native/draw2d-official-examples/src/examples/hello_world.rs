use novadraw::{
    figure::{FigureStyle, LabelFigure, RectangleFigure},
    geometry::Rectangle,
    graphics::Color,
    layout::StackLayout,
    runtime::Runtime,
};

use super::runtime_with_fonts;

const WIDTH: f64 = 200.0;
const HEIGHT: f64 = 100.0;

pub fn hello_world() -> Runtime {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::WHITE)
                .expect("valid example bounds"),
        ))
        .expect("HelloWorld root");
    runtime
        .figure(root)
        .expect("HelloWorld root")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("HelloWorld fills the Canvas");
    let label = runtime
        .figure(root)
        .expect("HelloWorld root")
        .add(Box::new(
            LabelFigure::new("Hello World")
                .with_bounds(Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))
                .expect("valid label bounds"),
        ))
        .expect("HelloWorld label");
    runtime
        .figure(label)
        .expect("HelloWorld label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some(novadraw::text::FontDescriptor::parse("16px Inter Variable").unwrap()),
            ..FigureStyle::default()
        })
        .expect("HelloWorld label style");
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_tree_matches_the_official_example() {
        let runtime = hello_world();
        let root = runtime.tree().contents().expect("root");
        let children = runtime.tree().child_order(root).expect("root children");

        assert_eq!(children.len(), 1);
        assert_eq!(
            runtime.tree().figure_bounds(root),
            Ok(Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))
        );
        assert_eq!(
            runtime.tree().figure_bounds(children[0]),
            Ok(Rectangle::new(0.0, 0.0, WIDTH, HEIGHT))
        );
    }

    #[test]
    fn label_fills_the_canvas_after_shrink_grow_and_restore() {
        let mut runtime = hello_world();
        let root = runtime.tree().contents().unwrap();
        let label = runtime.tree().child_order(root).unwrap()[0];
        for (width, height) in [(80.0, 40.0), (420.0, 240.0), (WIDTH, HEIGHT)] {
            runtime
                .figure(root)
                .unwrap()
                .set_bounds(Rectangle::new(0.0, 0.0, width, height))
                .unwrap();
            runtime.frames().stabilize_for_query().unwrap();
            assert_eq!(
                runtime.tree().figure_bounds(label).unwrap(),
                Rectangle::new(0.0, 0.0, width, height)
            );
        }
    }
}
