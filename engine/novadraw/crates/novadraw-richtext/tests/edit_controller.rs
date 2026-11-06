use novadraw::{
    graphics::Color,
    text::{FontDescriptor, TextAffinity, TextStyle, TextStyleSpan},
};
use novadraw_richtext::{
    DocumentError, ParagraphInput, RichTextDocumentBuilder, RichTextEditController,
    RichTextSelection,
};

fn controller(text: &str) -> RichTextEditController {
    let mut builder = RichTextDocumentBuilder::default();
    builder.push(ParagraphInput::new(text, style())).unwrap();
    RichTextEditController::new(builder.build()).unwrap()
}
fn style() -> TextStyle {
    TextStyle::new(
        FontDescriptor::parse("18px Inter Variable").unwrap(),
        Color::BLACK,
    )
}

#[test]
fn emoji_composition_rollback_commit_and_no_change_preserve_single_draft() {
    let mut c = controller("A👩‍💻👍🏽🇨🇳e\u{301}Z");
    let original = c.document().clone();
    let paragraph = original.paragraphs()[0].id();
    let cursor = c
        .cursor(paragraph, "A👩‍💻".len(), TextAffinity::Downstream)
        .unwrap();
    c.set_selection(RichTextSelection {
        anchor: cursor,
        focus: cursor,
    })
    .unwrap();
    c.delete(true).unwrap();
    assert_eq!(c.document().paragraphs()[0].text(), "A👍🏽🇨🇳e\u{301}Z");
    assert!(matches!(
        c.set_selection(RichTextSelection {
            anchor: cursor,
            focus: cursor
        }),
        Err(DocumentError::StaleVersion)
    ));
    c.insert("👩‍💻").unwrap();
    assert!(!c.is_changed());
    let before = c.document().version();
    c.preedit("你", Some(0..3)).unwrap();
    c.preedit("你好", Some(3..6)).unwrap();
    assert_eq!(c.document().paragraphs()[0].text(), "A👩‍💻你好👍🏽🇨🇳e\u{301}Z");
    assert_eq!(c.caret().unwrap().text.byte_offset(), "A👩‍💻你好".len());
    assert!(c.cancel_composition());
    assert_eq!(c.document().version(), before);
    c.preedit("候选", Some(0..6)).unwrap();
    c.insert("提交🙂").unwrap();
    assert!(c.composition().is_none());
    assert_eq!(
        c.document().paragraphs()[0].text(),
        "A👩‍💻提交🙂👍🏽🇨🇳e\u{301}Z"
    );
    assert_eq!(original.paragraphs()[0].text(), "A👩‍💻👍🏽🇨🇳e\u{301}Z");
}

#[test]
fn candidate_clones_are_isolated_and_span_edits_preserve_surviving_format() {
    let mut builder = RichTextDocumentBuilder::default();
    let mut input = ParagraphInput::new("abcdef", style());
    let styled = TextStyle::new(
        FontDescriptor::parse("18px Inter Variable").unwrap(),
        Color::rgba(1.0, 0.0, 0.0, 1.0),
    );
    input.spans.push(TextStyleSpan {
        range: 1..5,
        style: styled.clone(),
    });
    builder.push(input).unwrap();
    let c = RichTextEditController::new(builder.build()).unwrap();
    let mut candidate = c.clone();
    let id = c.document().paragraphs()[0].id();
    let a = candidate.cursor(id, 2, TextAffinity::Downstream).unwrap();
    let f = candidate.cursor(id, 4, TextAffinity::Downstream).unwrap();
    candidate
        .set_selection(RichTextSelection {
            anchor: a,
            focus: f,
        })
        .unwrap();
    candidate.insert("🙂").unwrap();
    let p = &candidate.document().paragraphs()[0];
    assert_eq!(p.text(), "ab🙂ef");
    assert_eq!(
        p.spans(),
        &[TextStyleSpan {
            range: 1..7,
            style: styled
        }]
    );
    assert_eq!(c.document().paragraphs()[0].text(), "abcdef");
    let before = candidate.document().version();
    assert!(candidate.preedit("🙂", Some(1..2)).is_err());
    assert_eq!(candidate.document().version(), before);
}

#[test]
fn identical_replacement_publishes_collapsed_selection_without_document_history() {
    let mut c = controller("abc");
    let version = c.document().version();
    let id = c.document().paragraphs()[0].id();
    c.set_selection(RichTextSelection {
        anchor: c.cursor(id, 0, TextAffinity::Downstream).unwrap(),
        focus: c.cursor(id, 3, TextAffinity::Downstream).unwrap(),
    })
    .unwrap();
    assert!(c.insert("abc").unwrap());
    assert_eq!(c.selection().anchor, c.selection().focus);
    assert_eq!(c.selection().focus.text.byte_offset(), 3);
    assert_eq!(c.document().version(), version);
    assert!(!c.is_changed());
}
