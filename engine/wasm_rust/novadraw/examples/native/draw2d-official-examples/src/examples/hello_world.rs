use novadraw::{
    figure::{FigureStyle, LabelFigure, RectangleFigure},
    geometry::Rectangle,
    graphics::Color,
    runtime::Runtime,
};

use super::runtime_with_fonts;

const WIDTH: f64 = 200.0;
const HEIGHT: f64 = 100.0;

pub fn hello_world() -> Runtime {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("HelloWorld root");
    let label = runtime
        .figure(root)
        .expect("HelloWorld root")
        .add(Box::new(
            LabelFigure::new("Hello World").with_bounds(Rectangle::new(0.0, 0.0, WIDTH, HEIGHT)),
        ))
        .expect("HelloWorld label");
    runtime
        .figure(label)
        .expect("HelloWorld label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some("16px Inter Variable".to_owned()),
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
}
