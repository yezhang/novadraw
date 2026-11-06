use novadraw::event::{PropertyValue, PropertyValueType};
use novadraw::text::{
    ParagraphAlignment, ParagraphDirection, StyledTextContent, TextStyle, TextStyleSpan,
};
use ropey::Rope;
use std::{fmt, ops::Range, sync::Arc};
use uuid::Uuid;

/// Stable within a document, including after other paragraphs change.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParagraphId(Uuid);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentVersion {
    instance: Uuid,
    revision: u64,
    snapshot: Uuid,
}

impl DocumentVersion {
    pub fn revision(self) -> u64 {
        self.revision
    }
}
impl PropertyValueType for DocumentVersion {
    fn into_property_value(self) -> PropertyValue {
        PropertyValue::Text(format!(
            "{}:{}:{}",
            self.instance, self.revision, self.snapshot
        ))
    }
}

/// Explicit source admission limits, independent of Runtime preparation limits.
#[derive(Clone, Copy, Debug)]
pub struct DocumentLimits {
    pub bytes: usize,
    pub paragraphs: usize,
    pub spans: usize,
}

impl Default for DocumentLimits {
    fn default() -> Self {
        Self {
            bytes: 8 * 1024 * 1024,
            paragraphs: 16_384,
            spans: 65_536,
        }
    }
}

#[derive(Clone, Debug)]
pub enum DocumentError {
    LimitExceeded,
    InvalidStyle(String),
    InvalidRange,
    MissingParagraph,
    StaleVersion,
    RevisionOverflow,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rich text document: {self:?}")
    }
}
impl std::error::Error for DocumentError {}

/// One explicit hard paragraph. Spans are resolved, ordered and non-overlapping.
/// Nested style overlays and cross-paragraph edits are a later slice.
#[derive(Clone)]
pub struct ParagraphInput {
    pub text: String,
    pub style: TextStyle,
    pub spans: Vec<TextStyleSpan>,
    pub direction: ParagraphDirection,
    pub alignment: ParagraphAlignment,
    pub space_after: f64,
}

impl ParagraphInput {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
            spans: Vec::new(),
            direction: ParagraphDirection::Auto,
            alignment: ParagraphAlignment::Leading,
            space_after: 0.0,
        }
    }
    fn validate(&self) -> Result<(), DocumentError> {
        if !self.space_after.is_finite() || self.space_after < 0.0 {
            return Err(DocumentError::InvalidStyle(
                "invalid paragraph spacing".into(),
            ));
        }
        StyledTextContent::new(self.text.clone(), self.style.clone(), self.spans.clone())
            .map_err(|e| DocumentError::InvalidStyle(e.to_string()))?;
        Ok(())
    }
}

/// Rope types stay private. Immutable paragraph assets are shared across revisions.
pub struct RichTextParagraph {
    id: ParagraphId,
    text: Rope,
    style: TextStyle,
    spans: Vec<TextStyleSpan>,
    pub(crate) direction: ParagraphDirection,
    pub(crate) alignment: ParagraphAlignment,
    pub(crate) space_after: f64,
}

impl RichTextParagraph {
    fn from_input(id: ParagraphId, input: ParagraphInput) -> Self {
        Self {
            id,
            text: Rope::from_str(&input.text),
            style: input.style,
            spans: input.spans,
            direction: input.direction,
            alignment: input.alignment,
            space_after: input.space_after,
        }
    }
    pub fn id(&self) -> ParagraphId {
        self.id
    }
    pub fn len_bytes(&self) -> usize {
        self.text.len_bytes()
    }
    pub fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }
    pub fn text(&self) -> String {
        self.text.to_string()
    }
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
    pub fn spans(&self) -> &[TextStyleSpan] {
        &self.spans
    }
    pub(crate) fn input(&self) -> ParagraphInput {
        ParagraphInput {
            text: self.text(),
            style: self.style.clone(),
            spans: self.spans.clone(),
            direction: self.direction,
            alignment: self.alignment,
            space_after: self.space_after,
        }
    }
}

#[derive(Clone)]
pub struct RichTextDocumentSnapshot {
    version: DocumentVersion,
    paragraphs: Arc<[Arc<RichTextParagraph>]>,
    limits: DocumentLimits,
    bytes: usize,
    spans: usize,
}

impl RichTextDocumentSnapshot {
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn paragraphs(&self) -> &[Arc<RichTextParagraph>] {
        &self.paragraphs
    }
    pub fn len_bytes(&self) -> usize {
        self.bytes
    }
    pub fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    /// Replaces a single paragraph without changing its identity. Unchanged
    /// paragraphs retain their immutable Rope and style assets.
    pub fn replace_paragraph(
        &self,
        expected: DocumentVersion,
        id: ParagraphId,
        input: ParagraphInput,
    ) -> Result<Self, DocumentError> {
        if expected != self.version {
            return Err(DocumentError::StaleVersion);
        }
        let index = self
            .paragraphs
            .iter()
            .position(|p| p.id == id)
            .ok_or(DocumentError::MissingParagraph)?;
        let old = &self.paragraphs[index];
        let bytes = self
            .bytes
            .checked_sub(old.len_bytes())
            .and_then(|n| n.checked_add(input.text.len()))
            .ok_or(DocumentError::LimitExceeded)?;
        let spans = self
            .spans
            .checked_sub(old.spans.len())
            .and_then(|n| n.checked_add(input.spans.len()))
            .ok_or(DocumentError::LimitExceeded)?;
        if bytes > self.limits.bytes || spans > self.limits.spans {
            return Err(DocumentError::LimitExceeded);
        }
        input.validate()?;
        let same = old.text() == input.text
            && old.style == input.style
            && old.spans == input.spans
            && old.direction == input.direction
            && old.alignment == input.alignment
            && old.space_after == input.space_after;
        if same {
            return Ok(self.clone());
        }
        let revision = self
            .version
            .revision
            .checked_add(1)
            .ok_or(DocumentError::RevisionOverflow)?;
        let mut paragraphs = self.paragraphs.to_vec();
        paragraphs[index] = Arc::new(RichTextParagraph::from_input(id, input));
        Ok(Self {
            version: DocumentVersion {
                revision,
                snapshot: Uuid::new_v4(),
                ..self.version
            },
            paragraphs: paragraphs.into(),
            limits: self.limits,
            bytes,
            spans,
        })
    }

    /// UTF-8 byte edit inside one paragraph. Styles must be supplied explicitly
    /// when editing a styled paragraph; this basic entry accepts unstyled text.
    pub fn edit_plain_paragraph(
        &self,
        expected: DocumentVersion,
        id: ParagraphId,
        range: Range<usize>,
        replacement: &str,
    ) -> Result<Self, DocumentError> {
        if expected != self.version {
            return Err(DocumentError::StaleVersion);
        }
        let paragraph = self
            .paragraphs
            .iter()
            .find(|p| p.id == id)
            .ok_or(DocumentError::MissingParagraph)?;
        let mut input = paragraph.input();
        if range.start > range.end
            || range.end > input.text.len()
            || !input.text.is_char_boundary(range.start)
            || !input.text.is_char_boundary(range.end)
            || !input.spans.is_empty()
        {
            return Err(DocumentError::InvalidRange);
        }
        let bytes = self
            .bytes
            .checked_sub(range.len())
            .and_then(|n| n.checked_add(replacement.len()))
            .ok_or(DocumentError::LimitExceeded)?;
        if bytes > self.limits.bytes {
            return Err(DocumentError::LimitExceeded);
        }
        input.text.replace_range(range, replacement);
        self.replace_paragraph(expected, id, input)
    }

    /// Edits one paragraph while preserving surviving spans. Inserted text inherits
    /// the span covering the replacement start (right affinity at span boundaries).
    pub fn edit_paragraph(
        &self,
        expected: DocumentVersion,
        id: ParagraphId,
        range: Range<usize>,
        replacement: &str,
    ) -> Result<Self, DocumentError> {
        if expected != self.version {
            return Err(DocumentError::StaleVersion);
        }
        let paragraph = self
            .paragraphs
            .iter()
            .find(|p| p.id == id)
            .ok_or(DocumentError::MissingParagraph)?;
        let mut input = paragraph.input();
        if range.start > range.end
            || range.end > input.text.len()
            || !input.text.is_char_boundary(range.start)
            || !input.text.is_char_boundary(range.end)
        {
            return Err(DocumentError::InvalidRange);
        }
        if self
            .bytes
            .checked_sub(range.len())
            .and_then(|n| n.checked_add(replacement.len()))
            .is_none_or(|bytes| bytes > self.limits.bytes)
        {
            return Err(DocumentError::LimitExceeded);
        }
        let inserted_style = input
            .spans
            .iter()
            .find(|s| s.range.contains(&range.start))
            .map(|s| s.style.clone());
        let mut spans = Vec::new();
        for span in &input.spans {
            if span.range.start < range.start {
                let end = span.range.end.min(range.start);
                if span.range.start < end {
                    spans.push(TextStyleSpan {
                        range: span.range.start..end,
                        style: span.style.clone(),
                    });
                }
            }
        }
        if !replacement.is_empty()
            && let Some(style) = inserted_style
        {
            spans.push(TextStyleSpan {
                range: range.start..range.start + replacement.len(),
                style,
            });
        }
        for span in &input.spans {
            if span.range.end > range.end {
                let start =
                    span.range.start.max(range.end) - range.end + range.start + replacement.len();
                let end = span.range.end - range.end + range.start + replacement.len();
                spans.push(TextStyleSpan {
                    range: start..end,
                    style: span.style.clone(),
                });
            }
        }
        let mut merged: Vec<TextStyleSpan> = Vec::new();
        for span in spans {
            if let Some(last) = merged.last_mut()
                && last.range.end == span.range.start
                && last.style == span.style
            {
                last.range.end = span.range.end;
            } else {
                merged.push(span);
            }
        }
        input.text.replace_range(range, replacement);
        input.spans = merged;
        self.replace_paragraph(expected, id, input)
    }
}

pub struct RichTextDocumentBuilder {
    limits: DocumentLimits,
    inputs: Vec<ParagraphInput>,
    bytes: usize,
    spans: usize,
}
impl Default for RichTextDocumentBuilder {
    fn default() -> Self {
        Self::new(DocumentLimits::default())
    }
}
impl RichTextDocumentBuilder {
    pub fn new(limits: DocumentLimits) -> Self {
        Self {
            limits,
            inputs: Vec::new(),
            bytes: 0,
            spans: 0,
        }
    }
    pub fn push(&mut self, input: ParagraphInput) -> Result<(), DocumentError> {
        let bytes = self
            .bytes
            .checked_add(input.text.len())
            .ok_or(DocumentError::LimitExceeded)?;
        let spans = self
            .spans
            .checked_add(input.spans.len())
            .ok_or(DocumentError::LimitExceeded)?;
        if bytes > self.limits.bytes
            || spans > self.limits.spans
            || self.inputs.len() >= self.limits.paragraphs
        {
            return Err(DocumentError::LimitExceeded);
        }
        input.validate()?;
        self.inputs.push(input);
        self.bytes = bytes;
        self.spans = spans;
        Ok(())
    }
    pub fn build(self) -> RichTextDocumentSnapshot {
        RichTextDocumentSnapshot {
            version: DocumentVersion {
                instance: Uuid::new_v4(),
                revision: 0,
                snapshot: Uuid::new_v4(),
            },
            paragraphs: self
                .inputs
                .into_iter()
                .map(|input| {
                    Arc::new(RichTextParagraph::from_input(
                        ParagraphId(Uuid::new_v4()),
                        input,
                    ))
                })
                .collect(),
            limits: self.limits,
            bytes: self.bytes,
            spans: self.spans,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{graphics::Color, text::FontDescriptor};
    fn input(text: &str) -> ParagraphInput {
        ParagraphInput::new(
            text,
            TextStyle::new(FontDescriptor::default(), Color::BLACK),
        )
    }
    #[test]
    fn edits_preserve_identity_sharing_and_reject_invalid_unicode_and_stale_versions() {
        let mut builder = RichTextDocumentBuilder::default();
        builder.push(input("你好🙂")).unwrap();
        builder.push(input("second")).unwrap();
        let original = builder.build();
        let first = original.paragraphs()[0].id();
        assert!(matches!(
            original.edit_plain_paragraph(original.version(), first, 1..2, "x"),
            Err(DocumentError::InvalidRange)
        ));
        let changed = original
            .edit_plain_paragraph(original.version(), first, 6..10, "!")
            .unwrap();
        assert_eq!(changed.paragraphs()[0].text(), "你好!");
        assert_eq!(changed.paragraphs()[0].id(), first);
        let fork = original
            .edit_plain_paragraph(original.version(), first, 6..10, "?")
            .unwrap();
        assert_eq!(fork.version().revision(), changed.version().revision());
        assert_ne!(fork.version(), changed.version());
        assert!(Arc::ptr_eq(
            &changed.paragraphs()[1],
            &original.paragraphs()[1]
        ));
        assert_eq!(original.paragraphs()[0].text(), "你好🙂");
        assert!(matches!(
            changed.edit_plain_paragraph(original.version(), first, 0..0, "x"),
            Err(DocumentError::StaleVersion)
        ));
        let same = changed
            .edit_plain_paragraph(changed.version(), first, 6..7, "!")
            .unwrap();
        assert_eq!(same.version(), changed.version());
    }
    #[test]
    fn rejected_admission_does_not_consume_builder_capacity() {
        let mut builder = RichTextDocumentBuilder::new(DocumentLimits {
            bytes: 4,
            paragraphs: 1,
            spans: 1,
        });
        assert!(builder.push(input("too long")).is_err());
        let mut invalid = input("中");
        invalid.spans.push(TextStyleSpan {
            range: 1..2,
            style: invalid.style.clone(),
        });
        assert!(builder.push(invalid).is_err());
        builder.push(input("ok")).unwrap();
        assert_eq!(builder.build().len_bytes(), 2);
    }
}
