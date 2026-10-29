use novadraw::{
    figure::{FigureStyle, RectangleFigure},
    graphics::Color,
    runtime::Runtime,
    text::FontDescriptor,
};

/// A mounted style update followed by inherited font and opacity queries.
#[unsafe(no_mangle)]
pub extern "C" fn style_workload(size: f32, alpha: f64, updates: usize) -> f64 {
    let mut runtime = Runtime::empty();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)))
        .unwrap();
    let child = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new(0.0, 0.0, 100.0, 80.0)))
        .unwrap();
    let mut result = 0.0;
    for index in 0..updates {
        let font = format!("{}px Inter Variable", size + (index % 2) as f32);
        runtime
            .figure(root)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(if index % 2 == 0 {
                    Color::BLACK
                } else {
                    Color::RED
                }),
                alpha: Some(novadraw::graphics::Opacity::try_new(alpha).unwrap()),
                font: Some(FontDescriptor::parse(&font).unwrap()),
                ..Default::default()
            })
            .unwrap();
        let style = runtime.tree().resolved_style(child).unwrap();
        let font = style.font;
        result += f64::from(font.size()) + style.alpha.get();
    }
    result
}

#[cfg(test)]
mod contracts {
    include!("../../../../crates/novadraw/tests/p2_s08_style_input_contract.rs");
}
