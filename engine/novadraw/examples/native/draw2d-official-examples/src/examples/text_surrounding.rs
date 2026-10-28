use super::text_examples::{TextFixture, add, canvas, font, text};
use novadraw::{
    figure::{
        EllipseFigure, RectangleFigure, RoundedRectangleFigure, TriangleFigure,
        border::LineBorder,
        flow::{FlowAdapterFigure, FlowBlockStyle, FlowPageFigure},
    },
    geometry::{Dimension, Rectangle},
    graphics::Color,
    layout::{StackLayout, ToolbarLayout},
    runtime::Runtime,
    text::ParagraphDirection,
};

pub fn text_surrounding_figure_example() -> Runtime {
    let TextFixture {
        mut runtime, page, ..
    } = canvas();
    text(
        &mut runtime,
        page,
        "This is the text on the left side, and شطص.  There has to be a lot of text here to see the first proxy wrap.  So, here we go.",
    );
    let adapter = add(&mut runtime, page, FlowAdapterFigure::new());
    let group = add(
        &mut runtime,
        adapter,
        RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, Color::WHITE)
            .with_border(LineBorder::new(Color::rgba(1.0, 0.65, 0.0, 1.0), 3.0)),
    );
    runtime
        .figure(group)
        .expect("proxy")
        .set_layout_manager(Box::new(ToolbarLayout::new()))
        .expect("ToolbarLayout");
    let rounded = add(
        &mut runtime,
        group,
        RoundedRectangleFigure::new_with_color(0.0, 0.0, 100.0, 40.0, 25.0, Color::GREEN)
            .with_stroke(Color::BLACK, 1.0),
    );
    let triangle = add(
        &mut runtime,
        group,
        TriangleFigure::new(Rectangle::new(0.0, 0.0, 80.0, 80.0))
            .with_fill_color(Color::RED)
            .with_stroke_color(Color::BLACK),
    );
    let ellipse = add(
        &mut runtime,
        group,
        EllipseFigure::new_with_color(0.0, 0.0, 70.0, 70.0, Color::BLUE)
            .with_stroke(Color::BLACK, 1.0),
    );
    for (id, width, height) in [
        (rounded, 100.0, 40.0),
        (triangle, 80.0, 80.0),
        (ellipse, 70.0, 70.0),
    ] {
        runtime
            .figure(id)
            .expect("shape")
            .set_preferred_size(Dimension::new(width, height))
            .expect("preferred shape");
    }
    text(&mut runtime, page, "This is the right side text.");
    let adapter = add(&mut runtime, page, FlowAdapterFigure::new());
    let group = add(
        &mut runtime,
        adapter,
        RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, Color::WHITE)
            .with_border(LineBorder::new(Color::rgba(1.0, 0.65, 0.0, 1.0), 3.0)),
    );
    runtime
        .figure(group)
        .expect("proxy")
        .set_layout_manager(Box::new(StackLayout::new()))
        .expect("StackLayout");
    let inner = add(
        &mut runtime,
        group,
        FlowPageFigure::new(Rectangle::new(0.0, 0.0, 240.0, 100.0))
            .with_style(font("13px Arial"))
            .with_block_style(FlowBlockStyle {
                direction: ParagraphDirection::RightToLeft,
                ..FlowBlockStyle::default()
            })
            .expect("RTL"),
    );
    text(
        &mut runtime,
        inner,
        "Text inside a proxy figure with BiDi: شطص عسـ زظص.",
    );
    runtime
}
