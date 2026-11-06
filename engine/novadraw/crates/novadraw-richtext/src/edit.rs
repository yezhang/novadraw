//! Reusable document editing state; no Editor, host lease, or persistent history.

use crate::{
    DocumentError, DocumentVersion, ParagraphId, RichTextDocumentSnapshot, RichTextLayoutSnapshot,
};
use novadraw::{
    geometry::Point,
    text::{TextAffinity, TextMovement, TextPosition},
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Document-bound logical cursor; layout positions are admitted separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextCursor {
    pub document: DocumentVersion,
    pub paragraph: ParagraphId,
    pub text: TextPosition,
}

/// One controller-owned directed selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextSelection {
    pub anchor: RichTextCursor,
    pub focus: RichTextCursor,
}

/// Active composition range and host-relative caret.
#[derive(Clone, Debug)]
pub struct RichTextComposition {
    pub range: RichTextSelection,
    pub selection: Option<Range<usize>>,
}

#[derive(Clone)]
struct CompositionBase {
    document: RichTextDocumentSnapshot,
    selection: RichTextSelection,
}

/// Specialized state reused by independent and Editor-coordinated hosts.
#[derive(Clone)]
pub struct RichTextEditController {
    initial: RichTextDocumentSnapshot,
    document: RichTextDocumentSnapshot,
    selection: RichTextSelection,
    composition: Option<RichTextComposition>,
    base: Option<CompositionBase>,
}
impl RichTextEditController {
    /// Starts a draft at the first explicit paragraph.
    pub fn new(document: RichTextDocumentSnapshot) -> Result<Self, DocumentError> {
        let paragraph = document
            .paragraphs()
            .first()
            .ok_or(DocumentError::MissingParagraph)?
            .id();
        let cursor = RichTextCursor {
            document: document.version(),
            paragraph,
            text: TextPosition::new(0, TextAffinity::Downstream),
        };
        Ok(Self {
            initial: document.clone(),
            document,
            selection: RichTextSelection {
                anchor: cursor,
                focus: cursor,
            },
            composition: None,
            base: None,
        })
    }
    pub fn document(&self) -> &RichTextDocumentSnapshot {
        &self.document
    }
    pub fn initial_document(&self) -> &RichTextDocumentSnapshot {
        &self.initial
    }
    pub fn selection(&self) -> RichTextSelection {
        self.selection
    }
    pub fn composition(&self) -> Option<&RichTextComposition> {
        self.composition.as_ref()
    }
    /// Compares content/format, so insert followed by delete does not create history.
    pub fn is_changed(&self) -> bool {
        self.initial.paragraphs().len() != self.document.paragraphs().len()
            || self
                .initial
                .paragraphs()
                .iter()
                .zip(self.document.paragraphs())
                .any(|(a, b)| {
                    a.id() != b.id()
                        || a.text() != b.text()
                        || a.style() != b.style()
                        || a.spans() != b.spans()
                        || a.direction != b.direction
                        || a.alignment != b.alignment
                        || a.space_after != b.space_after
                })
    }
    /// Checks document identity and UTF-8 boundary before storing a selection.
    pub fn set_selection(&mut self, selection: RichTextSelection) -> Result<(), DocumentError> {
        self.validate(selection.anchor)?;
        self.validate(selection.focus)?;
        if self.composition.is_some() {
            return Err(DocumentError::InvalidRange);
        }
        self.selection = selection;
        Ok(())
    }
    pub fn cursor(
        &self,
        paragraph: ParagraphId,
        byte: usize,
        affinity: TextAffinity,
    ) -> Result<RichTextCursor, DocumentError> {
        let cursor = RichTextCursor {
            document: self.document.version(),
            paragraph,
            text: TextPosition::new(byte, affinity),
        };
        self.validate(cursor)?;
        Ok(cursor)
    }
    fn validate(&self, cursor: RichTextCursor) -> Result<(), DocumentError> {
        if cursor.document != self.document.version() {
            return Err(DocumentError::StaleVersion);
        }
        let text = self
            .document
            .paragraphs()
            .iter()
            .find(|p| p.id() == cursor.paragraph)
            .ok_or(DocumentError::MissingParagraph)?
            .text();
        if cursor.text.byte_offset() > text.len()
            || !text.is_char_boundary(cursor.text.byte_offset())
        {
            return Err(DocumentError::InvalidRange);
        }
        Ok(())
    }
    fn replace_selection_inner(&mut self, replacement: &str) -> Result<bool, DocumentError> {
        let s = self.selection;
        self.validate(s.anchor)?;
        self.validate(s.focus)?;
        // Explicit hard-paragraph topology changes are a separate document protocol.
        if s.anchor.paragraph != s.focus.paragraph {
            return Err(DocumentError::InvalidRange);
        }
        let start = s.anchor.text.byte_offset().min(s.focus.text.byte_offset());
        let end = s.anchor.text.byte_offset().max(s.focus.text.byte_offset());
        let document = self.document.edit_paragraph(
            self.document.version(),
            s.focus.paragraph,
            start..end,
            replacement,
        )?;
        let cursor = RichTextCursor {
            document: document.version(),
            paragraph: s.focus.paragraph,
            text: TextPosition::new(start + replacement.len(), TextAffinity::Downstream),
        };
        let selection = RichTextSelection {
            anchor: cursor,
            focus: cursor,
        };
        let changed = document.version() != self.document.version() || selection != self.selection;
        self.document = document;
        self.selection = selection;
        Ok(changed)
    }
    /// Inserts text into the draft. Composition commit does not accept a host session.
    pub fn insert(&mut self, text: &str) -> Result<bool, DocumentError> {
        let mut next = self.clone();
        if next.composition.is_some() {
            next.cancel_composition();
        }
        let changed = next.replace_selection_inner(text)?;
        next.composition = None;
        next.base = None;
        let changed = changed || self.composition.is_some();
        *self = next;
        Ok(changed)
    }
    /// Updates preedit from its immutable base rather than stacking replacements.
    pub fn preedit(
        &mut self,
        text: &str,
        selection: Option<Range<usize>>,
    ) -> Result<bool, DocumentError> {
        if let Some(r) = &selection
            && (r.start > r.end
                || r.end > text.len()
                || !text.is_char_boundary(r.start)
                || !text.is_char_boundary(r.end))
        {
            return Err(DocumentError::InvalidRange);
        }
        if text.is_empty() {
            return Ok(self.cancel_composition());
        }
        let mut next = self.clone();
        let base = next.base.clone().unwrap_or_else(|| CompositionBase {
            document: next.document.clone(),
            selection: next.selection,
        });
        next.document = base.document.clone();
        next.selection = base.selection;
        let start = next
            .selection
            .anchor
            .text
            .byte_offset()
            .min(next.selection.focus.text.byte_offset());
        next.replace_selection_inner(text)?;
        let end = next.selection.focus;
        let start_cursor = next.cursor(end.paragraph, start, TextAffinity::Downstream)?;
        next.composition = Some(RichTextComposition {
            range: RichTextSelection {
                anchor: start_cursor,
                focus: end,
            },
            selection,
        });
        next.base = Some(base);
        *self = next;
        Ok(true)
    }
    /// Restores the document and selection captured before preedit.
    pub fn cancel_composition(&mut self) -> bool {
        let Some(base) = self.base.take() else {
            return false;
        };
        self.document = base.document;
        self.selection = base.selection;
        self.composition = None;
        true
    }
    pub fn caret(&self) -> Result<RichTextCursor, DocumentError> {
        if let Some(c) = &self.composition
            && let Some(r) = &c.selection
        {
            return self.cursor(
                c.range.anchor.paragraph,
                c.range.anchor.text.byte_offset() + r.end,
                TextAffinity::Downstream,
            );
        }
        Ok(self.selection.focus)
    }
    /// Deletes a selected range, or one complete adjacent Unicode grapheme.
    pub fn delete(&mut self, backwards: bool) -> Result<bool, DocumentError> {
        if self.composition.is_some() {
            return Err(DocumentError::InvalidRange);
        }
        let mut next = self.clone();
        if next.selection.anchor == next.selection.focus {
            let cursor = next.selection.focus;
            let text = next
                .document
                .paragraphs()
                .iter()
                .find(|p| p.id() == cursor.paragraph)
                .ok_or(DocumentError::MissingParagraph)?
                .text();
            let byte = neighbor(&text, cursor.text.byte_offset(), !backwards);
            next.selection.anchor =
                next.cursor(cursor.paragraph, byte, TextAffinity::Downstream)?;
        }
        let changed = next.replace_selection_inner("")?;
        *self = next;
        Ok(changed)
    }
    /// Moves using the richtext layout's visual/word/line navigation.
    pub fn move_selection(
        &mut self,
        layout: &RichTextLayoutSnapshot,
        movement: TextMovement,
        extend: bool,
    ) -> Result<(), DocumentError> {
        if layout.document_version() != self.document.version() {
            return Err(DocumentError::StaleVersion);
        }
        let focus = self.selection.focus;
        let position = layout
            .position(focus.paragraph, focus.text)
            .map_err(|_| DocumentError::InvalidRange)?;
        let moved = layout
            .move_position(&position, movement)
            .map_err(|_| DocumentError::InvalidRange)?;
        let cursor = self.cursor(
            moved.paragraph,
            moved.text.byte_offset(),
            moved.text.affinity(),
        )?;
        self.set_selection(RichTextSelection {
            anchor: if extend {
                self.selection.anchor
            } else {
                cursor
            },
            focus: cursor,
        })
    }
    pub fn select_at(
        &mut self,
        layout: &RichTextLayoutSnapshot,
        point: Point,
        extend: bool,
    ) -> Result<(), DocumentError> {
        if layout.document_version() != self.document.version() {
            return Err(DocumentError::StaleVersion);
        }
        if let Some(p) = layout
            .hit_test(point)
            .map_err(|_| DocumentError::InvalidRange)?
        {
            let cursor = self.cursor(p.paragraph, p.text.byte_offset(), p.text.affinity())?;
            self.set_selection(RichTextSelection {
                anchor: if extend {
                    self.selection.anchor
                } else {
                    cursor
                },
                focus: cursor,
            })?;
        }
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
