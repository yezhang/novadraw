use crate::{Config, HEIGHT, Result, WIDTH};
use novadraw::{
    figure::{
        FigureMeasurement, LabelFigure, MeasureConstraints, PreparationBudget, RectangleFigure,
    },
    geometry::{Dimension, Point, Rectangle},
    graphics::Color,
    layout::{LayoutError, LayoutManager, LayoutOutput, LayoutSceneQuery},
    runtime::Runtime,
    text::{
        BuiltinFont, FontDescriptor, TextAffinity, TextLayoutDiagnostics, TextPosition, TextStyle,
    },
    tree::{FigureId, FigureTree},
};
use novadraw_richtext::{
    ParagraphId, ParagraphInput, ReplaceRichTextDocument, RichTextDocumentBuilder,
    RichTextDocumentSnapshot, RichTextEditController, RichTextFigure, RichTextHandle,
    RichTextLayoutSnapshot, RichTextSelection,
};
use serde_json::{Value, json};
use std::{ops::Range, sync::Arc, time::Instant};
use unicode_segmentation::UnicodeSegmentation;

const MARGIN: f64 = 24.0;
const HEADER: f64 = 110.0;
const CARET_WIDTH: f64 = 2.0;
const PARAGRAPH_GAP: f64 = 12.0;
const EMOJI_PATH: &str = "/System/Library/Fonts/Apple Color Emoji.ttc";
const BODY: &str = "编辑改变文档，排版决定换行。河流穿过山谷，文字沿着页面展开。\
    Editing changes source; layout determines wrapping. 中文 English 日本語 🙂 👩‍💻 👍🏽。 ";
const LONG_PARAGRAPHS: usize = 128;
const LONG_REPEATS: usize = 20;
const GIANT_REPEATS: usize = LONG_PARAGRAPHS * LONG_REPEATS;
const MIXED_REPEATS: usize = 8;
const WORK_BUDGET: u64 = 80_000_000;
const ALLOCATION_BUDGET: u64 = 64_000_000;
pub const PAYLOAD: &str = "编辑🙂";

fn color(hex: &str) -> Color {
    Color::from_hex(hex).expect("sample palette")
}
fn style(size: f32, family: &str) -> TextStyle {
    TextStyle::new(FontDescriptor::new(family, size).unwrap(), color("#273b50"))
}
pub(crate) fn document(config: &Config) -> Result<RichTextDocumentSnapshot> {
    let mut builder = RichTextDocumentBuilder::default();
    for (text, size, family) in [
        ("点击文字，开始编辑", 28.0, BuiltinFont::NotoSansSc.family()),
        (
            "14 px 小字 · 中文 English",
            14.0,
            BuiltinFont::NotoSansSc.family(),
        ),
        (
            "40 px 大字 · 修改与换行",
            40.0,
            BuiltinFont::NotoSansSc.family(),
        ),
        ("64 px 字号", 64.0, BuiltinFont::NotoSansSc.family()),
        ("🙂 🚀 🎉 ❤️ 👍🏽 👩‍💻 👨‍👩‍👧‍👦 🇨🇳 1️⃣", 36.0, "Apple Color Emoji"),
    ] {
        let mut input = ParagraphInput::new(text, style(size, family));
        input.space_after = PARAGRAPH_GAP;
        builder.push(input)?;
    }
    let (paragraphs, repeats) = match config.load.as_str() {
        "large" => (LONG_PARAGRAPHS, LONG_REPEATS),
        "giant" => (1, GIANT_REPEATS),
        _ => (24, MIXED_REPEATS),
    };
    for index in 0..paragraphs {
        let mut input = ParagraphInput::new(
            format!("{index:03} · {}", BODY.repeat(repeats)),
            style(18.0, BuiltinFont::NotoSansSc.family()),
        );
        input.space_after = PARAGRAPH_GAP;
        builder.push(input)?;
    }
    Ok(builder.build())
}

struct Page;
impl LayoutManager for Page {
    fn preferred_measurement(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> std::result::Result<FigureMeasurement, LayoutError> {
        Ok(FigureMeasurement::new(WIDTH, HEIGHT, None))
    }
    fn minimum_size(
        &self,
        _: FigureId,
        _: MeasureConstraints,
        _: &LayoutSceneQuery<'_>,
    ) -> std::result::Result<Dimension, LayoutError> {
        Ok(Dimension::ZERO)
    }
    fn layout(
        &mut self,
        parent: FigureId,
        scene: &LayoutSceneQuery<'_>,
        out: &mut LayoutOutput,
    ) -> std::result::Result<(), LayoutError> {
        let b = scene.container_bounds(parent)?;
        let w = (b.width - MARGIN * 2.0).max(0.0);
        for (i, (id, _)) in scene.children(parent)?.iter().enumerate() {
            let bounds = match i {
                0 => Rectangle::new(MARGIN, MARGIN, w, 32.0),
                1 => Rectangle::new(MARGIN, 60.0, w, 30.0),
                2 => Rectangle::new(MARGIN, HEADER, w, (b.height - HEADER - MARGIN).max(0.0)),
                _ => continue, // Caret geometry comes from the current prepared text.
            };
            out.set_child_bounds(*id, bounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    pub paragraph: ParagraphId,
    pub byte: usize,
    pub affinity: TextAffinity,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Timing {
    pub mutation_ns: u128,
    pub layout_ns: u128,
}
pub struct Editor {
    pub runtime: Runtime,
    pub handle: RichTextHandle,
    caret: FigureId,
    pub controller: RichTextEditController,
    pub timing: Timing,
    pub metadata: Value,
}
impl Editor {
    pub fn new(config: &Config) -> Result<Self> {
        let document = document(config)?;
        let metadata = json!({
            "load":config.load, "bytes":document.len_bytes(),
            "characters":document.paragraphs().iter().map(|p|p.text().chars().count()).sum::<usize>(),
            "paragraphs":document.paragraphs().len(),
            "longest_paragraph_characters":document.paragraphs().iter().map(|p|p.text().chars().count()).max(),
            "gpu_submission":false,
            "preparation_budget":{"work":WORK_BUDGET,"allocation":ALLOCATION_BUDGET},
            "sizes_px":[14,18,28,36,40,64], "emoji_font":EMOJI_PATH,
        });
        let controller = RichTextEditController::new(document.clone())?;
        let mut tree = FigureTree::new();
        let root = tree
            .builder()
            .set_contents(Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                WIDTH,
                HEIGHT,
                color("#eaf0f5"),
            )?))?;
        tree.builder().set_layout_manager(root, Box::new(Page))?;
        for text in [
            "Richtext Edit  ·  点击定位 / 输入文字 / 字素删除",
            "F2 头部插入   F3 中部插入   F4 尾部插入   F5 编辑回放   F6 首页   F7 末尾",
        ] {
            tree.builder().add_child(
                root,
                Box::new(
                    LabelFigure::new(text).with_bounds(Rectangle::new(0.0, 0.0, WIDTH, 30.0))?,
                ),
            )?;
        }
        let handle = RichTextFigure::add_to(&mut tree.builder(), root, Rectangle::ZERO, document)?;
        let caret = tree.builder().add_child(
            root,
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                CARET_WIDTH,
                20.0,
                color("#087e8b"),
            )?),
        )?;
        tree.builder().set_enabled(caret, false)?;
        let mut runtime = Runtime::new(tree);
        for font in BuiltinFont::ALL {
            runtime.resources().register_builtin_font(font)?;
        }
        let bytes = std::fs::read(EMOJI_PATH)?;
        let id = runtime.resources().register_font()?;
        runtime
            .resources()
            .complete_font(id, novadraw::render::FontData::new(bytes))?;
        runtime
            .frames()
            .set_preparation_budget(PreparationBudget::new(WORK_BUDGET, ALLOCATION_BUDGET));
        runtime.frames().resize_logical_viewport(WIDTH, HEIGHT)?;
        runtime.frames().stabilize_for_query()?;
        let mut editor = Self {
            runtime,
            handle,
            caret,
            controller,
            timing: Timing::default(),
            metadata,
        };
        editor.update_caret(false)?;
        Ok(editor)
    }
    pub fn snapshot(&self) -> Result<Arc<RichTextLayoutSnapshot>> {
        self.runtime
            .tree()
            .prepared_presentation(self.handle.content())?
            .and_then(|p| p.shared_data::<RichTextLayoutSnapshot>())
            .ok_or_else(|| "missing admitted rich text layout".into())
    }
    pub fn current_text(&self) -> Result<String> {
        self.snapshot()?
            .document()
            .paragraphs()
            .iter()
            .find(|p| p.id() == self.cursor().paragraph)
            .map(|p| p.text())
            .ok_or_else(|| "missing active paragraph".into())
    }
    pub fn replace(&mut self, range: Range<usize>, replacement: &str) -> Result<Timing> {
        let _edit = TextLayoutDiagnostics::default().time("richtext.edit.total");
        let snapshot = self.snapshot()?;
        let source = snapshot.document();
        let start = Instant::now();
        {
            let _mutation = TextLayoutDiagnostics::default().time("richtext.edit.mutation");
            let paragraph = self.cursor().paragraph;
            let anchor =
                self.controller
                    .cursor(paragraph, range.start, TextAffinity::Downstream)?;
            let focus = self
                .controller
                .cursor(paragraph, range.end, TextAffinity::Downstream)?;
            self.controller
                .set_selection(RichTextSelection { anchor, focus })?;
            self.controller.insert(replacement)?;
            let changed = self.controller.document().clone();
            self.runtime
                .figure(self.handle.content())?
                .update_component(ReplaceRichTextDocument {
                    expected: source.version(),
                    document: changed,
                })?;
        }
        let mutation_ns = start.elapsed().as_nanos();
        let layout = Instant::now();
        {
            let _layout = TextLayoutDiagnostics::default().time("richtext.edit.layout");
            self.runtime.frames().stabilize_for_query()?;
        }
        let timing = Timing {
            mutation_ns,
            layout_ns: layout.elapsed().as_nanos(),
        };
        self.update_caret(true)?;
        self.timing = timing;
        Ok(timing)
    }
    pub fn insert(&mut self, text: &str) -> Result<Timing> {
        self.replace(self.cursor().byte..self.cursor().byte, text)
    }
    pub fn delete(&mut self, backwards: bool) -> Result<()> {
        let text = self.current_text()?;
        let byte = self.cursor().byte;
        let other = neighbor(&text, byte, !backwards);
        self.replace(other.min(byte)..other.max(byte), "")?;
        Ok(())
    }
    pub fn locate(&mut self, region: usize) -> Result<()> {
        let s = self.snapshot()?;
        let paragraphs = s.document().paragraphs();
        let midpoint = s.document().len_bytes() / 2;
        let mut prefix = 0;
        let index = match region {
            0 => 0,
            1 => paragraphs
                .iter()
                .position(|p| {
                    let end = prefix + p.len_bytes();
                    if midpoint <= end {
                        true
                    } else {
                        prefix = end;
                        false
                    }
                })
                .unwrap_or(paragraphs.len() - 1),
            _ => paragraphs.len() - 1,
        };
        let paragraph = &paragraphs[index];
        let text = paragraph.text();
        let byte = match region {
            0 => 0,
            1 => text
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .take_while(|i| *i <= midpoint.saturating_sub(prefix))
                .last()
                .unwrap_or(0),
            _ => text.len(),
        };
        self.set_cursor(Cursor {
            paragraph: paragraph.id(),
            byte,
            affinity: TextAffinity::Downstream,
        })?;
        self.update_caret(true)
    }
    pub fn click(&mut self, surface: Point) -> Result<()> {
        self.runtime.frames().stabilize_for_query()?;
        let viewport = self.handle.scroll_pane().viewport().figure_id();
        let b = self.runtime.tree().figure_bounds(viewport)?;
        let Some(inverse) = self.runtime.tree().surface_to_local_transform(viewport)? else {
            return Ok(());
        };
        let p = inverse.transform_point(surface);
        if p.x() < 0.0 || p.y() < 0.0 || p.x() >= b.width || p.y() >= b.height {
            return Ok(());
        }
        let Some(inverse) = self
            .runtime
            .tree()
            .surface_to_local_transform(self.handle.content())?
        else {
            return Ok(());
        };
        let point = inverse.transform_point(surface);
        if let Some(p) = self.snapshot()?.hit_test(point)? {
            self.set_cursor(Cursor {
                paragraph: p.paragraph,
                byte: p.text.byte_offset(),
                affinity: p.text.affinity(),
            })?;
            self.update_caret(false)?;
        }
        Ok(())
    }
    pub fn update_caret(&mut self, reveal: bool) -> Result<()> {
        let snapshot = self.snapshot()?;
        let position = snapshot.position(
            self.cursor().paragraph,
            TextPosition::new(self.cursor().byte, self.cursor().affinity),
        )?;
        let bounds = snapshot.caret_bounds(&position)?;
        let viewport = self.handle.scroll_pane().viewport();
        let vb = self.runtime.tree().figure_bounds(viewport.figure_id())?;
        if reveal {
            let y = viewport.view_location().y();
            let target = if bounds.y < y {
                bounds.y
            } else if bounds.y + bounds.height > y + vb.height {
                bounds.y + bounds.height - vb.height
            } else {
                y
            };
            self.runtime.viewports().set_view_location(
                viewport.figure_id(),
                0.0,
                target.max(0.0),
            )?;
            self.runtime.frames().stabilize_for_query()?;
        }
        let transform = self
            .runtime
            .tree()
            .local_to_surface_transform(self.handle.content())?;
        let point = transform.transform_point(Point::new(bounds.x, bounds.y));
        let vp = self
            .runtime
            .tree()
            .local_to_surface_transform(viewport.figure_id())?
            .transform_point(Point::ZERO);
        let rect = Rectangle::new(point.x(), point.y(), CARET_WIDTH, bounds.height)
            .intersection(Rectangle::new(vp.x(), vp.y(), vb.width, vb.height));
        if let Some(rect) = rect {
            self.runtime.figure(self.caret)?.set_bounds(rect)?;
        }
        self.runtime
            .figure(self.caret)?
            .set_visible(rect.is_some())?;
        self.runtime.frames().stabilize_for_query()?;
        Ok(())
    }
    pub fn cursor(&self) -> Cursor {
        let cursor = self.controller.selection().focus;
        Cursor {
            paragraph: cursor.paragraph,
            byte: cursor.text.byte_offset(),
            affinity: cursor.text.affinity(),
        }
    }
    fn set_cursor(&mut self, cursor: Cursor) -> Result<()> {
        let cursor = self
            .controller
            .cursor(cursor.paragraph, cursor.byte, cursor.affinity)?;
        self.controller.set_selection(RichTextSelection {
            anchor: cursor,
            focus: cursor,
        })?;
        Ok(())
    }
}

fn neighbor(text: &str, byte: usize, forwards: bool) -> usize {
    if forwards {
        text.grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > byte)
            .unwrap_or(text.len())
    } else {
        text.grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i < byte)
            .last()
            .unwrap_or(0)
    }
}

pub fn verify(config: &Config) -> Result<()> {
    let mut editor = Editor::new(config)?;
    let before = editor.snapshot()?;
    let old = before.position(
        editor.cursor().paragraph,
        TextPosition::new(0, TextAffinity::Downstream),
    )?;
    let original = editor.current_text()?;
    editor.insert("👩‍💻")?;
    assert!(editor.current_text()?.starts_with("👩‍💻"));
    editor.delete(true)?;
    assert_eq!(editor.current_text()?, original);
    assert!(editor.snapshot()?.caret_bounds(&old).is_err());
    assert_eq!(before.caret_bounds(&old)?.y, 0.0);
    for region in 0..3 {
        editor.locate(region)?;
        let old_text = editor.current_text()?;
        let byte = editor.cursor().byte;
        editor.insert(PAYLOAD)?;
        assert_eq!(&editor.current_text()?[byte..byte + PAYLOAD.len()], PAYLOAD);
        editor.replace(byte..byte + PAYLOAD.len(), "")?;
        assert_eq!(editor.current_text()?, old_text);
    }
    editor.locate(1)?;
    let s = editor.snapshot()?;
    let p = s.position(
        editor.cursor().paragraph,
        TextPosition::new(editor.cursor().byte, editor.cursor().affinity),
    )?;
    let b = s.caret_bounds(&p)?;
    assert!(b.y > 0.0);
    let transform = editor
        .runtime
        .tree()
        .local_to_surface_transform(editor.handle.content())?;
    editor.click(transform.transform_point(Point::new(b.x, b.y + b.height / 2.0)))?;
    assert_eq!(editor.cursor().paragraph, p.paragraph);
    let wide = editor.snapshot()?;
    editor
        .runtime
        .frames()
        .resize_logical_viewport(WIDTH / 2.0, HEIGHT)?;
    editor.runtime.frames().stabilize_for_query()?;
    assert!(editor.snapshot()?.caret_bounds(&p).is_err());
    assert!(editor.snapshot()?.size().height > wide.size().height);
    assert!(
        editor
            .runtime
            .tree()
            .child_order(editor.handle.content())?
            .is_empty()
    );
    println!(
        "richtext-figure-edit-app: Unicode insert/delete, three edit locations, restored content, caret identity, hit-test, scroll reveal and resize passed; {}",
        editor.metadata
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_keeps_combining_emoji_and_flags_whole() {
        let text = "A👩‍💻👍🏽🇨🇳e\u{301}Z";
        let mut i = 0;
        for grapheme in text.graphemes(true) {
            let next = neighbor(text, i, true);
            assert_eq!(&text[i..next], grapheme);
            assert_eq!(neighbor(text, next, false), i);
            i = next;
        }
    }
    #[test]
    fn real_runtime_edit_contract() {
        verify(&Config::parse(&[]).unwrap()).unwrap();
    }
}
