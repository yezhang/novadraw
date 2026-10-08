use novadraw::figure::{FlowPage, FlowParagraph, FlowWrapping, InlineTextFragment};
use novadraw::render::{BuiltinFont, RenderCommandKind};
use novadraw::{Rectangle, RectangleFigure, Runtime, TextFlowFigure};

fn page() -> FlowPage {
    FlowPage::new(vec![
        FlowParagraph::new(vec![
            InlineTextFragment::new("English "),
            InlineTextFragment::new("العربية"),
        ]),
        FlowParagraph::new(vec![
            InlineTextFragment::new("中文段落 "),
            InlineTextFragment::new("wraps across several words"),
        ]),
    ])
}

#[test]
fn text_flow_shapes_fragments_paragraphs_bidi_and_soft_wrap_as_one_layout() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 240.0)))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(TextFlowFigure::new(
            Rectangle::new(20.0, 20.0, 120.0, 180.0),
            page(),
        )))
        .unwrap();

    let frame = runtime.record_full_frame();
    let layout = runtime.text_flow_layout(flow).unwrap();
    assert_eq!(
        layout.key().text(),
        "English العربية\n中文段落 wraps across several words"
    );
    assert!(layout.line_metrics().len() > 2);
    assert!(!layout.glyph_runs().is_empty());
    assert!(
        frame
            .commands()
            .iter()
            .any(|command| matches!(command.kind, RenderCommandKind::DrawGlyphRun { .. }))
    );
}

#[test]
fn text_flow_truncation_is_utf8_safe_and_respects_the_line_limit() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    runtime
        .register_builtin_font(BuiltinFont::NotoSansSc)
        .unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 220.0, 160.0)))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            TextFlowFigure::new(
                Rectangle::new(10.0, 10.0, 90.0, 120.0),
                FlowPage::from_text("第一行内容 mixed content 第二行内容 and more words"),
            )
            .with_wrapping(FlowWrapping::Truncate { max_lines: 2 }),
        ))
        .unwrap();

    runtime.record_full_frame();
    let layout = runtime.text_flow_layout(flow).unwrap();
    assert!(layout.is_truncated());
    assert!(layout.line_metrics().len() <= 2);
    assert!(
        layout
            .key()
            .text()
            .is_char_boundary(layout.visible_range().end)
    );
}

#[test]
fn text_flow_runtime_mutation_and_width_change_refresh_the_cached_layout() {
    let mut runtime = Runtime::empty();
    runtime.register_builtin_font(BuiltinFont::Inter).unwrap();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 320.0, 240.0)))
        .unwrap();
    let flow = runtime
        .container(root)
        .unwrap()
        .add(Box::new(TextFlowFigure::new(
            Rectangle::new(20.0, 20.0, 180.0, 180.0),
            FlowPage::from_text("alpha beta gamma delta epsilon"),
        )))
        .unwrap();
    runtime.record_full_frame();
    let wide_lines = runtime.text_flow_layout(flow).unwrap().line_metrics().len();

    runtime
        .figure(flow)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 20.0, 70.0, 180.0))
        .unwrap();
    runtime.record_full_frame();
    let narrow_lines = runtime.text_flow_layout(flow).unwrap().line_metrics().len();
    assert!(narrow_lines > wide_lines);

    assert!(
        runtime
            .text_flow(flow)
            .unwrap()
            .replace_page(FlowPage::from_text("replacement paragraph"))
            .unwrap()
    );
    assert!(
        runtime
            .text_flow(flow)
            .unwrap()
            .set_wrapping(FlowWrapping::NoWrap)
            .unwrap()
    );
    runtime.record_full_frame();
    let layout = runtime.text_flow_layout(flow).unwrap();
    assert_eq!(layout.key().text(), "replacement paragraph");
    assert_eq!(layout.line_metrics().len(), 1);
}
