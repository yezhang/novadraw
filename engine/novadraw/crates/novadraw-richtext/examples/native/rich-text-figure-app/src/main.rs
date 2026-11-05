use novadraw::{
    figure::{FigureMeasurement, LabelFigure, MeasureConstraints, PolylineFigure, RectangleFigure},
    geometry::{Dimension, Rectangle},
    graphics::Color,
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
    runtime::Runtime,
    text::{BuiltinFont, FontDescriptor, FontStyle, TextStyle, TextStyleSpan},
    tree::{FigureId, FigureTree},
};
use novadraw_example_support::AppBuilder;
mod interaction;
use interaction::{Canvas, Group, selection};
use novadraw_richtext::{
    ParagraphInput, ReplaceRichTextDocument, RichTextDocumentBuilder, RichTextFigure,
    RichTextHandle, RichTextLayoutSnapshot,
};

const WINDOW_WIDTH: f64 = 1120.0;
const WINDOW_HEIGHT: f64 = 760.0;
const MARGIN: f64 = 24.0;
const GAP: f64 = 24.0;
const CLASS_FRACTION: f64 = 0.38;
const PARAGRAPH_COUNT: usize = 24;
const BODY_SIZE: f32 = 18.0;
const HEADING_SIZE: f32 = 28.0;
const PARAGRAPH_SPACE: f64 = 18.0;
const CARD_WIDTH: f64 = 310.0;
const CARD_HEIGHT: f64 = 142.0;
const CARD_X: f64 = 24.0;
const FIRST_CARD_Y: f64 = 100.0;
const SECOND_CARD_Y: f64 = 346.0;
const ROW_HEIGHT: f64 = 34.0;

fn color(hex: &str) -> Color {
    Color::from_hex(hex).expect("sample palette")
}
fn style(size: f32, hex: &str) -> TextStyle {
    TextStyle::new(
        FontDescriptor::new(BuiltinFont::NotoSansSc.family(), size).expect("sample font"),
        color(hex),
    )
}

struct Columns;
impl LayoutManager for Columns {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(WINDOW_WIDTH, WINDOW_HEIGHT, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        parent: FigureId,
        scene: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError> {
        let bounds = scene.container_bounds(parent)?;
        let margin = MARGIN.min(bounds.width / 4.0).min(bounds.height / 4.0);
        let gap = GAP.min((bounds.width - margin * 2.0).max(0.0));
        let width = (bounds.width - margin * 2.0 - gap).max(0.0);
        let left = width * CLASS_FRACTION;
        let height = (bounds.height - margin * 2.0).max(0.0);
        for (index, (child, _)) in scene.children(parent)?.iter().enumerate() {
            let (x, width) = if index == 0 {
                (bounds.x + margin, left)
            } else {
                (bounds.x + margin + left + gap, width - left)
            };
            out.set_child_bounds(*child, Rectangle::new(x, bounds.y + margin, width, height));
        }
        Ok(())
    }
}

fn document() -> novadraw_richtext::RichTextDocumentSnapshot {
    let mut builder = RichTextDocumentBuilder::default();
    let mut title = ParagraphInput::new("独立富文本 Figure", style(HEADING_SIZE, "#15334b"));
    title.space_after = PARAGRAPH_SPACE;
    builder.push(title).unwrap();
    let mut intro = ParagraphInput::new(
        "这是一份原生富文本文档。正文支持不同字号、颜色和字体样式；滚动条只控制右侧内容。",
        style(BODY_SIZE, "#334155"),
    );
    let highlight = "原生富文本文档";
    let start = intro.text.find(highlight).unwrap();
    intro.spans.push(TextStyleSpan {
        range: start..start + highlight.len(),
        style: TextStyle::new(
            FontDescriptor::new(BuiltinFont::NotoSansSc.family(), BODY_SIZE)
                .unwrap()
                .with_weight(700.0)
                .unwrap(),
            color("#176c68"),
        ),
    });
    intro.space_after = PARAGRAPH_SPACE;
    builder.push(intro).unwrap();
    let mut bidi = ParagraphInput::new(
        "Mixed directions: English · العربية · 中文。窗口变窄时，正文会重新换行。",
        TextStyle::new(
            FontDescriptor::new(BuiltinFont::NotoSansSc.family(), BODY_SIZE)
                .unwrap()
                .with_style(FontStyle::Italic),
            color("#6941a5"),
        ),
    );
    bidi.space_after = PARAGRAPH_SPACE;
    builder.push(bidi).unwrap();
    for index in 1..=PARAGRAPH_COUNT {
        let text = format!(
            "{index:02}  河流穿过山谷，文字沿着页面展开。每个段落拥有自己的内容与样式，\
             但它们不会变成场景中的子 Figure。拖动右侧滑块，可以继续阅读后面的段落。\
             A document can share a scene with diagrams, shapes and other components."
        );
        let mut paragraph = ParagraphInput::new(text, style(BODY_SIZE, "#334155"));
        paragraph.space_after = PARAGRAPH_SPACE;
        builder.push(paragraph).unwrap();
    }
    builder.build()
}

fn label(tree: &mut FigureTree, parent: FigureId, text: &str, bounds: Rectangle) -> FigureId {
    tree.builder()
        .add_child(
            parent,
            Box::new(LabelFigure::new(text).with_bounds(bounds).unwrap()),
        )
        .unwrap()
}
fn class_card(
    tree: &mut FigureTree,
    parent: FigureId,
    y: f64,
    name: &str,
    rows: &[&str],
) -> FigureId {
    let card = tree
        .builder()
        .add_child(
            parent,
            Box::new(
                RectangleFigure::new_with_color(
                    CARD_X,
                    y,
                    CARD_WIDTH,
                    CARD_HEIGHT,
                    color("#e6f0f5"),
                )
                .unwrap()
                .with_stroke(color("#658096"), 1.0)
                .unwrap(),
            ),
        )
        .unwrap();
    label(
        tree,
        card,
        name,
        Rectangle::new(0.0, 0.0, CARD_WIDTH, ROW_HEIGHT),
    );
    tree.builder()
        .add_child(
            card,
            Box::new(PolylineFigure::new(0.0, ROW_HEIGHT, CARD_WIDTH, ROW_HEIGHT).unwrap()),
        )
        .unwrap();
    for (index, row) in rows.iter().enumerate() {
        label(
            tree,
            card,
            row,
            Rectangle::new(0.0, ROW_HEIGHT * (index + 1) as f64, CARD_WIDTH, ROW_HEIGHT),
        );
    }
    selection(
        tree,
        card,
        Rectangle::new(0.0, 0.0, CARD_WIDTH, CARD_HEIGHT),
        false,
    )
}

fn scene() -> (Runtime, RichTextHandle, FigureId, Vec<FigureId>) {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(Canvas(Rectangle::new(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        ))))
        .unwrap();
    tree.builder().set_focusable(root, true).unwrap();
    tree.builder()
        .set_layout_manager(root, Box::new(Columns))
        .unwrap();
    let diagram = tree
        .builder()
        .add_child(
            root,
            Box::new(Group(Rectangle::new(
                0.0,
                0.0,
                CARD_WIDTH + MARGIN * 2.0,
                WINDOW_HEIGHT,
            ))),
        )
        .unwrap();
    let heading = label(
        &mut tree,
        diagram,
        "Document / View",
        Rectangle::new(CARD_X, MARGIN, CARD_WIDTH, ROW_HEIGHT),
    );
    let heading_selection = selection(
        &mut tree,
        heading,
        Rectangle::new(0.0, 0.0, CARD_WIDTH, ROW_HEIGHT),
        false,
    );
    let first_selection = class_card(
        &mut tree,
        diagram,
        FIRST_CARD_Y,
        "Document",
        &[
            "+ paragraphs: Snapshot",
            "+ version: DocumentVersion",
            "+ replaceParagraph()",
        ],
    );
    let second_selection = class_card(
        &mut tree,
        diagram,
        SECOND_CARD_Y,
        "RichTextFigure",
        &[
            "+ document: Snapshot",
            "+ viewport: ScrollPane",
            "+ prepare() / paint()",
        ],
    );
    let center = CARD_X + CARD_WIDTH / 2.0;
    tree.builder()
        .add_child(
            diagram,
            Box::new(
                PolylineFigure::new(center, FIRST_CARD_Y + CARD_HEIGHT, center, SECOND_CARD_Y)
                    .unwrap(),
            ),
        )
        .unwrap();
    const RELATION_HIT_WIDTH: f64 = 12.0;
    let relation_selection = selection(
        &mut tree,
        diagram,
        Rectangle::new(
            center - RELATION_HIT_WIDTH / 2.0,
            FIRST_CARD_Y + CARD_HEIGHT,
            RELATION_HIT_WIDTH,
            SECOND_CARD_Y - FIRST_CARD_Y - CARD_HEIGHT,
        ),
        false,
    );
    let annotation = label(
        &mut tree,
        diagram,
        "renders",
        Rectangle::new(
            center,
            FIRST_CARD_Y + CARD_HEIGHT + ROW_HEIGHT,
            CARD_WIDTH / 2.0,
            ROW_HEIGHT,
        ),
    );
    let annotation_selection = selection(
        &mut tree,
        annotation,
        Rectangle::new(0.0, 0.0, CARD_WIDTH / 2.0, ROW_HEIGHT),
        false,
    );
    let rich_group = tree
        .builder()
        .add_child(root, Box::new(Group(Rectangle::ZERO)))
        .unwrap();
    tree.builder()
        .set_layout_manager(rich_group, Box::new(novadraw::layout::StackLayout::new()))
        .unwrap();
    let handle = RichTextFigure::add_to(
        &mut tree.builder(),
        rich_group,
        Rectangle::new(0.0, 0.0, WINDOW_WIDTH / 2.0, WINDOW_HEIGHT),
        document(),
    )
    .unwrap();
    let rich_selection = selection(&mut tree, rich_group, Rectangle::ZERO, true);
    let mut runtime = Runtime::new(tree);
    for font in BuiltinFont::ALL {
        runtime
            .resources()
            .register_builtin_font(font)
            .expect("bundled sample font");
    }
    (
        runtime,
        handle,
        diagram,
        vec![
            heading_selection,
            first_selection,
            second_selection,
            relation_selection,
            annotation_selection,
            rich_selection,
        ],
    )
}

fn input_point(
    runtime: &Runtime,
    id: FigureId,
) -> Result<novadraw::geometry::Point, Box<dyn std::error::Error>> {
    let bounds = runtime.tree().figure_bounds(id)?;
    Ok(runtime
        .tree()
        .local_to_surface_transform(id)?
        .transform_point(novadraw::geometry::Point::new(
            bounds.width / 4.0,
            bounds.height / 2.0,
        )))
}

fn click(runtime: &mut Runtime, id: FigureId) -> Result<(), Box<dyn std::error::Error>> {
    use novadraw::event::{FocusChange, MouseButton};
    let point = input_point(runtime, id)?;
    let pressed = runtime
        .input()
        .dispatch_mouse_pressed(point.x(), point.y(), MouseButton::Left);
    if pressed.target() != Some(id) || !pressed.is_handled() {
        return Err(format!(
            "selection target mismatch: {:?}, expected {id:?}",
            pressed.target()
        )
        .into());
    }
    runtime
        .input()
        .dispatch_mouse_released(point.x(), point.y(), MouseButton::Left);
    // Requesting the existing focus is a no-op, and proves the click acquired it.
    if runtime.focus().request(id)? != FocusChange::Unchanged {
        return Err("click did not acquire focus".into());
    }
    runtime.frames().stabilize_for_query()?;
    Ok(())
}

fn verify_input(
    runtime: &mut Runtime,
    handle: &RichTextHandle,
    diagram: FigureId,
    selections: &[FigureId],
) -> Result<(), Box<dyn std::error::Error>> {
    use novadraw::{
        container::ScrollBarPart,
        event::{FocusChange, MouseButton, WheelEvent},
    };
    let mut previous = None;
    for &id in selections {
        let old_revision = previous
            .map(|old| runtime.tree().component_revision(old))
            .transpose()?;
        click(runtime, id)?;
        if let (Some(old), Some(revision)) = (previous, old_revision)
            && runtime.tree().component_revision(old)? <= revision
        {
            return Err("previous selection did not clear".into());
        }
        previous = Some(id);
    }
    let rich = *selections.last().ok_or("missing rich text selection")?;
    let point = input_point(runtime, rich)?;
    let viewport = handle.scroll_pane().viewport();
    let before = viewport.view_location().y();
    runtime
        .input()
        .dispatch_scroll(WheelEvent::new(point.x(), point.y(), 0.0, -3.0));
    runtime.frames().stabilize_for_query()?;
    if viewport.view_location().y() != before {
        return Err("selection mode allowed document scrolling".into());
    }
    let outcome =
        runtime
            .input()
            .dispatch_mouse_double_clicked(point.x(), point.y(), MouseButton::Left);
    if !outcome.is_handled() || outcome.target() != Some(rich) {
        return Err("double click did not enter rich text".into());
    }
    runtime.frames().stabilize_for_query()?;
    let transform = runtime.tree().local_to_surface_transform(diagram)?;
    runtime
        .input()
        .dispatch_scroll(WheelEvent::new(point.x(), point.y(), 0.0, -3.0));
    runtime.frames().stabilize_for_query()?;
    if viewport.view_location().y() <= before
        || runtime.tree().local_to_surface_transform(diagram)? != transform
    {
        return Err("editing mode scrolling or isolation failed".into());
    }
    let bar = handle.scroll_pane().vertical_scroll_bar();
    let thumb = runtime
        .tree()
        .child_order(bar)?
        .into_iter()
        .find(|id| {
            runtime
                .tree()
                .layout_constraint::<ScrollBarPart>(*id)
                .is_ok_and(|role| role == Some(&ScrollBarPart::Thumb))
        })
        .ok_or("missing scrollbar thumb")?;
    let thumb_point = input_point(runtime, thumb)?;
    let pressed =
        runtime
            .input()
            .dispatch_mouse_pressed(thumb_point.x(), thumb_point.y(), MouseButton::Left);
    let capture = pressed.capture().ok_or("thumb did not capture")?;
    let before_drag = viewport.view_location().y();
    const DRAG_DISTANCE: f64 = 40.0;
    let moved = runtime
        .input()
        .dispatch_mouse_moved(thumb_point.x(), thumb_point.y() + DRAG_DISTANCE);
    if moved.capture() != Some(capture) {
        return Err("thumb drag lost capture".into());
    }
    let released = runtime.input().dispatch_mouse_released(
        thumb_point.x(),
        thumb_point.y() + DRAG_DISTANCE,
        MouseButton::Left,
    );
    runtime.frames().stabilize_for_query()?;
    if released.capture().is_some()
        || viewport.view_location().y() <= before_drag
        || runtime.focus().request(rich)? != FocusChange::Unchanged
    {
        return Err("thumb drag or editing focus failed".into());
    }
    click(runtime, selections[1])?;
    click(runtime, rich)?; // The input surface must intercept again after focus loss.
    let root = runtime.tree().parent_id(diagram)?.ok_or("missing canvas")?;
    let blank = MARGIN / 2.0;
    let pressed = runtime
        .input()
        .dispatch_mouse_pressed(blank, blank, MouseButton::Left);
    runtime
        .input()
        .dispatch_mouse_released(blank, blank, MouseButton::Left);
    if pressed.target() != Some(root) || runtime.focus().request(root)? != FocusChange::Unchanged {
        return Err("blank canvas did not clear object selection".into());
    }
    runtime.frames().stabilize_for_query()?;
    runtime
        .viewports()
        .set_view_location(viewport.figure_id(), 0.0, 0.0)?;
    runtime.frames().stabilize_for_query()?;
    Ok(())
}

fn verify() -> Result<(), Box<dyn std::error::Error>> {
    let (mut runtime, handle, diagram, selections) = scene();
    runtime.frames().stabilize_for_query()?;
    verify_input(&mut runtime, &handle, diagram, &selections)?;
    let snapshot = runtime
        .tree()
        .prepared_presentation(handle.content())?
        .and_then(|p| p.shared_data::<RichTextLayoutSnapshot>())
        .ok_or("missing rich text snapshot")?;
    let pane = handle.scroll_pane();
    let viewport = pane.viewport();
    let range = viewport.vertical_range();
    if snapshot.paragraph_count() != PARAGRAPH_COUNT + 3 || range.maximum <= range.extent {
        return Err("document does not exceed its viewport".into());
    }
    let diagram_transform = runtime.tree().local_to_surface_transform(diagram)?;
    runtime
        .viewports()
        .set_view_location(viewport.figure_id(), 0.0, range.maximum)?;
    runtime.frames().stabilize_for_query()?;
    if viewport.view_location().y() <= 0.0
        || diagram_transform != runtime.tree().local_to_surface_transform(diagram)?
    {
        return Err("scroll isolation failed".into());
    }
    let original = snapshot.document();
    let first = original.paragraphs()[0].id();
    let changed = original.edit_plain_paragraph(original.version(), first, 0..0, "Hello · ")?;
    let receipt = runtime
        .figure(handle.content())?
        .update_component(ReplaceRichTextDocument {
            expected: original.version(),
            document: changed.clone(),
        })?;
    if !receipt.changed {
        return Err("document edit was not committed".into());
    }
    runtime.frames().stabilize_for_query()?;
    let edited = runtime
        .tree()
        .prepared_presentation(handle.content())?
        .and_then(|p| p.shared_data::<RichTextLayoutSnapshot>())
        .ok_or("missing edited presentation")?;
    if edited.document_version() != changed.version() {
        return Err("presentation did not publish edited source".into());
    }
    let receipt = runtime
        .figure(handle.content())?
        .update_component(ReplaceRichTextDocument {
            expected: changed.version(),
            document: changed.clone(),
        })?;
    if receipt.changed {
        return Err("identical source caused invalidation".into());
    }
    let root = runtime
        .tree()
        .parent_id(diagram)?
        .ok_or("missing app root")?;
    if runtime.tree().child_order(root)?.len() != 2
        || !runtime.tree().child_order(handle.content())?.is_empty()
    {
        return Err("document content expanded into a Figure subtree".into());
    }
    runtime.figure(root)?.set_bounds(Rectangle::new(
        0.0,
        0.0,
        WINDOW_WIDTH / 2.0,
        WINDOW_HEIGHT,
    ))?;
    runtime.frames().stabilize_for_query()?;
    let narrow = runtime
        .tree()
        .prepared_presentation(handle.content())?
        .and_then(|p| p.shared_data::<RichTextLayoutSnapshot>())
        .ok_or("missing narrow presentation")?;
    if narrow.size().height <= edited.size().height {
        return Err("narrow resize did not reflow".into());
    }
    let original = document();
    // A separate instance has a different identity, even at revision zero.
    let stale = runtime
        .figure(handle.content())?
        .update_component(ReplaceRichTextDocument {
            expected: original.version(),
            document: original,
        });
    if stale.is_ok() {
        return Err("stale document identity was accepted".into());
    }
    println!(
        "rich-text-figure-app: {} paragraphs; content height {:.1}; viewport {:.1}; selection, double-click mode, scroll isolation, typed edit, no-change, resize and stale source rejection passed",
        snapshot.paragraph_count(),
        snapshot.size().height,
        range.extent
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--headless") {
        return verify();
    }
    let mut app = AppBuilder::new("Rich Text Figure")
        .with_app_name("rich-text-figure-app")
        .with_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .with_runtime_scenes_boxed(vec![("Document and diagram", Box::new(|| scene().0))]);
    if args.iter().any(|arg| arg == "--screenshot") {
        app = app.with_screenshot(true);
    }
    app.run()
}
