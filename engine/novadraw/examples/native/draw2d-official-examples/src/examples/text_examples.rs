//! Canvas contents from org.eclipse.draw2d.examples.text (EPL-2.0).
use novadraw::{
    container::ScrollPaneHandle,
    figure::{
        FigureStyle, RectangleFigure,
        flow::{
            BlockFlowFigure, FlowBlockStyle, FlowPageFigure, InlineFlowFigure, StructuredTextFigure,
        },
    },
    geometry::Rectangle,
    layout::StackLayout,
    render::BuiltinFont,
    runtime::Runtime,
    text::ParagraphAlignment,
    tree::{FigureId, FigureTree},
};

const PAGE_WIDTH: f64 = 600.0;
const PAGE_HEIGHT: f64 = 500.0;
const BASE_FONT: &str = "13px Arial";
const SENTENCE: &str = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twenty-one twenty-two twenty-three twenty-four twenty-five twenty-six twenty-seven twenty-eight twenty-nine thirty thirty-one thirty-two thirty-three thirty-four thirty-five thirty-six thirty-seven thirty-eight thirty-nine forty forty-one forty-two forty-three forty-four forty-five forty-six forty-seven forty-eight forty-nine fifty fifty-one fifty-two fifty-three fifty-four fifty-five fifty-six fifty-seven fifty-eight fifty-nine sixty>>>";
pub(super) const MULTI_LINE: &str = "This is a  oijeoi aeif jaoiewjf oaiew jfoaiewapijewpfk apoewkf paokwe pfokawpehfaowephf hawoiejf oaweijf paowkefp aoewkfpa pawkfe paoewkf paowekf pawokefoaiwjefo iajewoifja oewijf poerk pgaokew faewfpokew ff oaewkfp aewpofk apwejfoiuajweo foiwajef poawkefpawkje pfoakewpf aewpfkpaowke fpawfgfpowaegpawepmulti-line label.";

pub(super) struct TextFixture {
    pub runtime: Runtime,
    pub page: FigureId,
    pub pane: ScrollPaneHandle,
}

pub(super) fn canvas() -> TextFixture {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new_with_color(
                0.0,
                0.0,
                PAGE_WIDTH,
                PAGE_HEIGHT,
                novadraw::graphics::Color::WHITE,
            )
            .expect("valid example bounds"),
        ))
        .expect("Canvas");
    tree.builder()
        .set_layout_manager(root, Box::new(StackLayout::new()))
        .expect("Canvas layout");
    let pane = tree
        .builder()
        .add_scroll_pane_to(root, Rectangle::ZERO)
        .expect("FigureCanvas scroll pane");
    let mut runtime = Runtime::new(tree);
    for font in BuiltinFont::ALL {
        runtime
            .resources()
            .register_builtin_font(font)
            .expect("font");
    }
    for bytes in [
        include_bytes!("../../assets/NotoSansHebrew-VF.ttf").as_slice(),
        include_bytes!("../../assets/NotoSansJP-VF.ttf").as_slice(),
    ] {
        let id = runtime
            .resources()
            .register_font()
            .expect("multilingual font");
        runtime
            .resources()
            .complete_font(id, novadraw::render::FontData::new(bytes.to_vec()))
            .expect("multilingual font data");
    }
    runtime
        .viewports()
        .set_tracks_width(pane.viewport().figure_id(), true)
        .expect("tracks width");
    let page = runtime
        .viewports()
        .set_scroll_contents(
            &pane,
            Box::new(
                FlowPageFigure::new(Rectangle::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT))
                    .expect("valid page bounds")
                    .with_style(font(BASE_FONT)),
            ),
        )
        .expect("FlowPage");
    TextFixture {
        runtime,
        page,
        pane,
    }
}

pub(super) fn font(descriptor: &str) -> FigureStyle {
    // Use bundled equivalents so absent SWT font names cannot choose an arbitrary fallback.
    let descriptor = descriptor
        .replace("Arial", "Inter Variable")
        .replace("Helvetica", "Inter Variable")
        .replace("Tahoma", "Inter Variable")
        .replace("Courier", "JetBrains Mono");
    FigureStyle {
        font: Some(novadraw::text::FontDescriptor::parse(&descriptor).unwrap()),
        ..FigureStyle::default()
    }
}

pub(super) fn add(
    runtime: &mut Runtime,
    parent: FigureId,
    figure: impl novadraw::figure::Figure + 'static,
) -> FigureId {
    runtime
        .figure(parent)
        .expect("parent")
        .add(Box::new(figure))
        .expect("flow child")
}

pub(super) fn text(runtime: &mut Runtime, parent: FigureId, value: &str) -> FigureId {
    add(runtime, parent, StructuredTextFigure::new(value))
}

pub(super) fn block(runtime: &mut Runtime, parent: FigureId, style: FlowBlockStyle) -> FigureId {
    add(
        runtime,
        parent,
        BlockFlowFigure::new(style).expect("block style"),
    )
}

pub fn text_flow_example() -> Runtime {
    let TextFixture {
        mut runtime, page, ..
    } = canvas();
    let title = block(&mut runtime, page, FlowBlockStyle::default());
    add(
        &mut runtime,
        title,
        StructuredTextFigure::new("Draw2D TextFlow Example")
            .with_style(font("bold 20px Helvetica")),
    );
    let paragraph = block(&mut runtime, page, FlowBlockStyle::default());
    text(
        &mut runtime,
        paragraph,
        "A paragraph is created by using a BlockFlow figure. A paragraph usually wraps to the width of the current page. To see this, try resizing the example's window. The contents of this paragraph were created using a TextFlow figure in its default layout, which breaks at whitespace. a block of text that doesn't wrap can be created by using the SimpleTextLayout on a TextFlow",
    );
    let code = block(
        &mut runtime,
        page,
        FlowBlockStyle {
            wrap: false,
            ..FlowBlockStyle::default()
        },
    );
    add(&mut runtime, code, StructuredTextFigure::new("/**\n * The SimpleTextLayout only breaks at newlines\n * It can be used to render source code.\r */\r\npublic void foo() {\n    //TABs are not handled currently, only spaces.\n    System.out.println(\"foo\")\n}").with_style(font("12px Courier")));
    let nested = block(&mut runtime, page, FlowBlockStyle::default());
    text(
        &mut runtime,
        nested,
        "A paragraph can contain multiple children, including nested structure using InlineFlows. ",
    );
    let inline = add(
        &mut runtime,
        nested,
        InlineFlowFigure::new().with_style(FigureStyle {
            foreground: Some(novadraw::graphics::Color::rgba(0.0, 0.0, 0.5, 1.0)),
            ..FigureStyle::default()
        }),
    );
    text(
        &mut runtime,
        inline,
        "This TextFlow is inside an InlineFlow with a blue foreground color. Children can have ",
    );
    add(
        &mut runtime,
        inline,
        StructuredTextFigure::new("Bold ").with_style(font("bold 13px Helvetica")),
    );
    text(&mut runtime, inline, "or ");
    add(
        &mut runtime,
        inline,
        StructuredTextFigure::new("Italic ").with_style(font("italic 13px Helvetica")),
    );
    text(
        &mut runtime,
        inline,
        "Font, or override other inherited attributes.",
    );
    let baseline = block(&mut runtime, page, FlowBlockStyle::default());
    for (i, value) in [
        "Text fragments ",
        "with different ",
        "Font sizes will ",
        "have their ",
        "baseline ",
        "vertically ",
        "aligned ",
        "within ",
        "the current ",
        "line",
    ]
    .iter()
    .enumerate()
    {
        let size = (i + 8) as f64 * 4.0 / 3.0;
        add(
            &mut runtime,
            baseline,
            StructuredTextFigure::new(*value).with_style(font(&format!("{size}px Helvetica"))),
        );
    }
    let aligned = block(
        &mut runtime,
        page,
        FlowBlockStyle {
            alignment: Some(ParagraphAlignment::Trailing),
            ..FlowBlockStyle::default()
        },
    );
    text(
        &mut runtime,
        aligned,
        "An inline flow figure's aligment is determined by the block flow in which it resides. This block is aligned using ",
    );
    add(
        &mut runtime,
        aligned,
        StructuredTextFigure::new("PositionConstants.RIGHT").with_style(font("12px Courier")),
    );
    text(
        &mut runtime,
        aligned,
        ", which results in right text aligment",
    );
    runtime
}

pub(super) fn populate_large(runtime: &mut Runtime, page: FigureId) -> FigureId {
    let mut target = None;
    for i in 0..20 {
        let paragraph = block(runtime, page, FlowBlockStyle::default());
        if i == 0 {
            target = Some(add(
                runtime,
                paragraph,
                StructuredTextFigure::new("Normal text.").with_style(FigureStyle {
                    tooltip: Some(Some("This is a Tooltip".into())),
                    ..FigureStyle::default()
                }),
            ));
        }
        let inline = add(runtime, paragraph, InlineFlowFigure::new());
        text(runtime, inline, "This is the first small sentence. ");
        let nested = add(runtime, inline, InlineFlowFigure::new());
        for _ in 0..4 {
            text(runtime, nested, SENTENCE);
        }
        let sizes = block(
            runtime,
            page,
            FlowBlockStyle {
                alignment: Some(ParagraphAlignment::Center),
                ..FlowBlockStyle::default()
            },
        );
        for size in 5..25 {
            add(
                runtime,
                sizes,
                StructuredTextFigure::new(format!("{size} pt. Font ")).with_style(font(&format!(
                    "{}px Helvetica",
                    f64::from(size) * 4.0 / 3.0
                ))),
            );
        }
    }
    target.expect("first paragraph target")
}

pub fn multi_line_label_example() -> Runtime {
    let TextFixture {
        mut runtime, pane, ..
    } = canvas();
    let page = super::text_interaction::label_page(&mut runtime, &pane);
    text(&mut runtime, page, MULTI_LINE);
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn six_text_scenes_stabilize_and_publish_flow_snapshots() {
        for (name, factory) in [
            ("text-flow", text_flow_example as fn() -> Runtime),
            (
                "large",
                super::super::text_interaction::text_flow_large_example,
            ),
            ("caret", super::super::text_interaction::caret_example),
            ("bidi", super::super::bidi_block::bidi_block_example),
            (
                "surrounding",
                super::super::text_surrounding::text_surrounding_figure_example,
            ),
            ("label", multi_line_label_example),
        ] {
            let mut runtime = factory();
            let result = runtime.frames().stabilize_for_query();
            assert!(result.is_ok(), "{name}: {result:?}");
            let root = runtime.tree().contents().expect("root");
            let mut pending = vec![root];
            let mut snapshots = 0;
            while let Some(id) = pending.pop() {
                snapshots += usize::from(
                    runtime
                        .tree()
                        .prepared_presentation(id)
                        .unwrap()
                        .is_some_and(|p| {
                            p.data::<novadraw::figure::flow::FlowSnapshot>().is_some()
                        }),
                );
                pending.extend(runtime.tree().child_order(id).unwrap());
            }
            assert!(snapshots > 0, "{name}: no flow");
        }
    }
    #[test]
    fn inline_figure_metrics_are_stable_across_font_registry_instances() {
        for _ in 0..20 {
            let mut runtime = super::super::text_surrounding::text_surrounding_figure_example();
            let result = runtime.frames().stabilize_for_query();
            assert!(result.is_ok(), "{result:?}");
        }
    }

    #[test]
    fn narrowing_the_canvas_reflows_text_and_keeps_real_children() {
        let mut runtime = super::super::text_surrounding::text_surrounding_figure_example();
        let root = runtime.tree().contents().unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let pane = runtime.tree().child_order(root).unwrap()[0];
        let viewport = runtime
            .tree()
            .child_order(pane)
            .unwrap()
            .into_iter()
            .find(|id| runtime.tree().node(*id).unwrap().figure_name() == "ViewportFigure")
            .unwrap();
        let page = runtime.tree().child_order(viewport).unwrap()[0];
        let before = runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<novadraw::figure::flow::FlowSnapshot>()
            .unwrap()
            .clone();
        runtime
            .figure(root)
            .unwrap()
            .set_bounds(Rectangle::new(0.0, 0.0, 180.0, 500.0))
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let after = runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<novadraw::figure::flow::FlowSnapshot>()
            .unwrap();
        assert!(
            after.size.height > before.size.height,
            "before={:?}, after={:?}, viewport={:?}, page={:?}",
            before.size,
            after.size,
            runtime.tree().figure_bounds(viewport),
            runtime.tree().figure_bounds(page)
        );
        assert!(after.paragraphs[0].layout.inline_placements().len() >= 2);
        for (&child, bounds) in &after.placements {
            if runtime.tree().node(child).unwrap().figure_name() == "RectangleFigure" {
                assert!(bounds.width > 0.0 && bounds.height > 0.0);
                assert!(!runtime.tree().child_order(child).unwrap().is_empty());
            }
        }
    }
}
