use novadraw::{
    figure::{FigureStyle, RectangleFigure, flow::*},
    geometry::Rectangle,
    layout::StackLayout,
    runtime::Runtime,
    text::BuiltinFont,
    tree::{FigureId, FigureTree},
};

pub fn make(name: &str) -> (Runtime, FigureId, String) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 600.0, 500.0)))
        .unwrap();
    tree.builder()
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .unwrap();
    let pane = tree
        .builder()
        .add_scroll_pane_to(root, Rectangle::ZERO)
        .unwrap();
    let mut runtime = Runtime::new(tree);
    runtime
        .resources()
        .register_builtin_font(BuiltinFont::Inter)
        .unwrap();
    runtime
        .viewports()
        .set_tracks_width(pane.viewport().figure_id(), true)
        .unwrap();
    let page = runtime
        .viewports()
        .set_scroll_contents(
            &pane,
            Box::new(
                FlowPageFigure::new(Rectangle::new(0.0, 0.0, 600.0, 500.0)).with_style(
                    FigureStyle {
                        font: Some("13px Inter Variable".into()),
                        ..Default::default()
                    },
                ),
            ),
        )
        .unwrap();
    let (count, text, wrap) = match name {
        "paragraph" => (1, "alpha beta gamma delta epsilon. ".repeat(1600), true),
        "line" => (1, "abcdefghij".repeat(5120), false),
        "mixed" => (40, "mixed Figure text ".repeat(64), true),
        _ => panic!("unknown performance workload"),
    };
    let mut first = None;
    for _ in 0..count {
        let block = runtime
            .figure(page)
            .unwrap()
            .add(Box::new(
                BlockFlowFigure::new(FlowBlockStyle {
                    wrap,
                    ..Default::default()
                })
                .unwrap(),
            ))
            .unwrap();
        let source = runtime
            .figure(block)
            .unwrap()
            .add(Box::new(StructuredTextFigure::new(&text)))
            .unwrap();
        first.get_or_insert(source);
        if name == "mixed" {
            let adapter = runtime
                .figure(block)
                .unwrap()
                .add(Box::new(FlowAdapterFigure::new()))
                .unwrap();
            let rectangle = runtime
                .figure(adapter)
                .unwrap()
                .add(Box::new(RectangleFigure::new(0.0, 0.0, 20.0, 18.0)))
                .unwrap();
            runtime
                .figure(rectangle)
                .unwrap()
                .set_preferred_size(novadraw::geometry::Dimension::new(20.0, 18.0))
                .unwrap();
            runtime
                .figure(block)
                .unwrap()
                .add(Box::new(StructuredTextFigure::new(" after inline Figure.")))
                .unwrap();
        }
    }
    (runtime, first.unwrap(), text)
}
