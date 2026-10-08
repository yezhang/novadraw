//! DOM text-input normalization for the optional Novadraw editor integration.

use std::ops::Range;

use novadraw::Rectangle;
use novadraw_editor::{
    DirectTextEditSessionId, SessionTextInputEvent, TextDelete, TextInputEffect, TextInputEvent,
    TextInputPurpose, TextInputSnapshot,
};

/// DOM operation requested by [`WebTextInputBridge`].
#[derive(Clone, Debug, PartialEq)]
pub enum WebTextInputAction {
    /// Create/focus the hidden input host and position it at the caret.
    Acquire {
        /// Input purpose used to configure the hidden control.
        purpose: TextInputPurpose,
        /// Caret rectangle in CSS logical pixels.
        area: Rectangle,
    },
    /// Move the hidden input host to the latest caret rectangle.
    SetArea(Rectangle),
    /// Blur and clear the hidden input host.
    Release,
    /// Clear the host value after one deterministic input reconciliation.
    ClearValue,
}

/// Stateful bridge between DOM composition/input callbacks and Editor lease events.
#[derive(Debug, Default)]
pub struct WebTextInputBridge {
    active: Option<DirectTextEditSessionId>,
    composing: bool,
    suppress_input: Option<String>,
    purpose: Option<TextInputPurpose>,
}

/// Platform-neutral state adapter for a browser `EditContext`.
#[derive(Debug, Default)]
pub struct WebEditContextBridge {
    active: Option<DirectTextEditSessionId>,
    composing: bool,
    purpose: Option<TextInputPurpose>,
}

/// Invalid UTF-16 offsets received from or sent to a browser text context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebTextOffsetError;

impl std::fmt::Display for WebTextOffsetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("browser text offset splits a UTF-16 sequence")
    }
}

impl std::error::Error for WebTextOffsetError {}

impl WebEditContextBridge {
    /// Creates an idle EditContext bridge.
    pub const fn new() -> Self {
        Self {
            active: None,
            composing: false,
            purpose: None,
        }
    }

    /// Returns the session currently owning the EditContext.
    pub const fn active_session(&self) -> Option<DirectTextEditSessionId> {
        self.active
    }

    /// Returns whether the browser is in an active composition.
    pub const fn is_composing(&self) -> bool {
        self.composing
    }

    /// Applies lease changes emitted by the Editor.
    ///
    /// Returns `false` for stale area/release effects.
    pub fn apply_effect(&mut self, effect: &TextInputEffect) -> bool {
        match effect {
            TextInputEffect::Acquire {
                session, purpose, ..
            } => {
                self.active = Some(*session);
                self.composing = false;
                self.purpose = Some(*purpose);
                true
            }
            TextInputEffect::SetArea { session, .. } if self.active == Some(*session) => true,
            TextInputEffect::Release { session } if self.active == Some(*session) => {
                self.active = None;
                self.composing = false;
                self.purpose = None;
                true
            }
            TextInputEffect::SetArea { .. } | TextInputEffect::Release { .. } => false,
        }
    }

    /// Marks the start of browser-managed composition.
    pub fn composition_started(&mut self) {
        self.composing = self.active.is_some();
    }

    /// Converts an EditContext `textupdate` snapshot into one atomic Editor event.
    pub fn text_updated(
        &self,
        text: impl Into<String>,
        update_start_utf16: usize,
        replacement: &str,
        selection_utf16: Range<usize>,
    ) -> Result<Option<SessionTextInputEvent>, WebTextOffsetError> {
        let Some(session) = self.active else {
            return Ok(None);
        };
        let snapshot = normalize_edit_context_snapshot(
            text.into(),
            update_start_utf16,
            replacement,
            selection_utf16,
            self.composing,
        )?;
        Ok(Some(SessionTextInputEvent::new(
            session,
            TextInputEvent::Synchronize(snapshot),
        )))
    }

    /// Ends composition and synchronizes the browser's committed buffer.
    pub fn composition_ended(
        &mut self,
        text: impl Into<String>,
        selection_utf16: Range<usize>,
    ) -> Result<Option<SessionTextInputEvent>, WebTextOffsetError> {
        self.composing = false;
        let Some(session) = self.active else {
            return Ok(None);
        };
        let text = text.into();
        let selection = utf16_range_to_utf8(&text, selection_utf16)?;
        Ok(Some(SessionTextInputEvent::new(
            session,
            TextInputEvent::Synchronize(TextInputSnapshot::new(text, selection, None)),
        )))
    }

    /// Normalizes loss of the canvas text-input focus.
    pub fn focus_lost(&mut self) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        self.composing = false;
        Some(SessionTextInputEvent::new(
            session,
            TextInputEvent::FocusLost,
        ))
    }

    /// Normalizes commands not handled as EditContext text updates.
    pub fn key_pressed(
        &self,
        key: &str,
        control: bool,
        meta: bool,
    ) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        let event = match key {
            "Escape" if !self.composing => TextInputEvent::Cancel,
            "Enter" if self.purpose == Some(TextInputPurpose::Multiline) && !control && !meta => {
                TextInputEvent::InsertText("\n".to_owned())
            }
            "Enter" if !self.composing => TextInputEvent::Accept,
            _ => return None,
        };
        Some(SessionTextInputEvent::new(session, event))
    }
}

impl WebTextInputBridge {
    /// Creates an idle bridge.
    pub const fn new() -> Self {
        Self {
            active: None,
            composing: false,
            suppress_input: None,
            purpose: None,
        }
    }

    /// Returns the session currently owning the hidden input host.
    pub const fn active_session(&self) -> Option<DirectTextEditSessionId> {
        self.active
    }

    /// Returns whether DOM composition is active.
    pub const fn is_composing(&self) -> bool {
        self.composing
    }

    /// Converts an Editor host effect into a DOM operation.
    ///
    /// Stale area/release effects return `None`.
    pub fn apply_effect(&mut self, effect: &TextInputEffect) -> Option<WebTextInputAction> {
        match effect {
            TextInputEffect::Acquire {
                session,
                purpose,
                area,
            } => {
                self.active = Some(*session);
                self.composing = false;
                self.suppress_input = None;
                self.purpose = Some(*purpose);
                Some(WebTextInputAction::Acquire {
                    purpose: *purpose,
                    area: *area,
                })
            }
            TextInputEffect::SetArea { session, area } if self.active == Some(*session) => {
                Some(WebTextInputAction::SetArea(*area))
            }
            TextInputEffect::Release { session } if self.active == Some(*session) => {
                self.active = None;
                self.composing = false;
                self.suppress_input = None;
                self.purpose = None;
                Some(WebTextInputAction::Release)
            }
            TextInputEffect::SetArea { .. } | TextInputEffect::Release { .. } => None,
        }
    }

    /// Normalizes `compositionstart`.
    pub fn composition_started(&mut self) {
        self.composing = self.active.is_some();
    }

    /// Normalizes `compositionupdate`.
    pub fn composition_updated(
        &mut self,
        text: impl Into<String>,
        selection: Option<Range<usize>>,
    ) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        self.composing = true;
        Some(SessionTextInputEvent::new(
            session,
            TextInputEvent::Preedit {
                text: text.into(),
                selection,
            },
        ))
    }

    /// Reconciles preedit after the browser has updated the DOM input value and selection.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn composition_reconciled(
        &self,
        text: impl Into<String>,
        selection: Option<Range<usize>>,
    ) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        let event = normalize_composition_input(self.composing, text.into(), selection)?;
        Some(SessionTextInputEvent::new(session, event))
    }

    /// Normalizes `compositionend` without accepting the direct-edit session.
    pub fn composition_ended(
        &mut self,
        committed: impl Into<String>,
    ) -> Option<(SessionTextInputEvent, WebTextInputAction)> {
        let session = self.active?;
        self.composing = false;
        let committed = committed.into();
        self.suppress_input = Some(committed.clone());
        Some((
            SessionTextInputEvent::new(session, TextInputEvent::InsertText(committed)),
            WebTextInputAction::ClearValue,
        ))
    }

    /// Normalizes a non-composition DOM `input` value.
    ///
    /// The hidden host is cleared after each event, so its complete value is one committed
    /// insertion and never becomes application model state.
    pub fn input(
        &mut self,
        host_value: impl Into<String>,
    ) -> Option<(SessionTextInputEvent, WebTextInputAction)> {
        let session = self.active?;
        let host_value = host_value.into();
        if self.suppress_input.as_deref() == Some(host_value.as_str()) {
            self.suppress_input = None;
            return None;
        }
        self.suppress_input = None;
        let event = normalize_input(self.composing, host_value)?;
        Some((
            SessionTextInputEvent::new(session, event),
            WebTextInputAction::ClearValue,
        ))
    }

    /// Normalizes a cancellable `beforeinput` deletion.
    pub fn delete_backward(&self) -> Option<SessionTextInputEvent> {
        self.delete(TextDelete::PreviousVisual)
    }

    /// Normalizes a cancellable forward `beforeinput` deletion.
    pub fn delete_forward(&self) -> Option<SessionTextInputEvent> {
        self.delete(TextDelete::NextVisual)
    }

    /// Normalizes a cancellable backward word deletion.
    pub fn delete_word_backward(&self) -> Option<SessionTextInputEvent> {
        self.delete(TextDelete::PreviousWord)
    }

    /// Normalizes a cancellable forward word deletion.
    pub fn delete_word_forward(&self) -> Option<SessionTextInputEvent> {
        self.delete(TextDelete::NextWord)
    }

    /// Normalizes host focus loss.
    pub fn focus_lost(&mut self) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        self.composing = false;
        Some(SessionTextInputEvent::new(
            session,
            TextInputEvent::FocusLost,
        ))
    }

    /// Normalizes a DOM `keydown` used for editing commands rather than text insertion.
    pub fn key_pressed(
        &mut self,
        key: &str,
        shift: bool,
        control: bool,
        alt: bool,
        meta: bool,
    ) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        if self.composing && key != "Escape" {
            return None;
        }
        let extend = if shift {
            novadraw_editor::ExtendTextSelection::Yes
        } else {
            novadraw_editor::ExtendTextSelection::No
        };
        let event = match key {
            "Escape" if self.composing => {
                self.composing = false;
                TextInputEvent::CancelComposition
            }
            "Escape" => TextInputEvent::Cancel,
            "Enter" if self.purpose == Some(TextInputPurpose::Multiline) && !control && !meta => {
                TextInputEvent::InsertText("\n".to_owned())
            }
            "Enter" => TextInputEvent::Accept,
            "Backspace" => TextInputEvent::Delete(if alt || control {
                TextDelete::PreviousWord
            } else {
                TextDelete::PreviousVisual
            }),
            "Delete" => TextInputEvent::Delete(if alt || control {
                TextDelete::NextWord
            } else {
                TextDelete::NextVisual
            }),
            "ArrowLeft" => TextInputEvent::Move {
                movement: if alt || control {
                    novadraw::text::TextMovement::PreviousWord
                } else {
                    novadraw::text::TextMovement::PreviousVisual
                },
                extend,
            },
            "ArrowRight" => TextInputEvent::Move {
                movement: if alt || control {
                    novadraw::text::TextMovement::NextWord
                } else {
                    novadraw::text::TextMovement::NextVisual
                },
                extend,
            },
            "ArrowUp" => TextInputEvent::Move {
                movement: novadraw::text::TextMovement::PreviousLine,
                extend,
            },
            "ArrowDown" => TextInputEvent::Move {
                movement: novadraw::text::TextMovement::NextLine,
                extend,
            },
            "Home" => TextInputEvent::Move {
                movement: novadraw::text::TextMovement::LineStart,
                extend,
            },
            "End" => TextInputEvent::Move {
                movement: novadraw::text::TextMovement::LineEnd,
                extend,
            },
            "a" | "A" if control || meta => TextInputEvent::SelectAll,
            _ => return None,
        };
        Some(SessionTextInputEvent::new(session, event))
    }

    fn delete(&self, deletion: TextDelete) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        (!self.composing)
            .then(|| SessionTextInputEvent::new(session, TextInputEvent::Delete(deletion)))
    }
}

fn normalize_input(composing: bool, value: String) -> Option<TextInputEvent> {
    (!composing && !value.is_empty()).then_some(TextInputEvent::InsertText(value))
}

#[cfg(any(target_arch = "wasm32", test))]
fn normalize_composition_input(
    composing: bool,
    value: String,
    selection: Option<Range<usize>>,
) -> Option<TextInputEvent> {
    composing.then_some(TextInputEvent::Preedit {
        text: value,
        selection,
    })
}

pub(crate) fn utf16_range_to_utf8(
    text: &str,
    range: Range<usize>,
) -> Result<Range<usize>, WebTextOffsetError> {
    if range.start > range.end {
        return Err(WebTextOffsetError);
    }
    Ok(utf16_offset_to_utf8(text, range.start)?..utf16_offset_to_utf8(text, range.end)?)
}

pub(crate) fn utf16_offset_to_utf8(text: &str, target: usize) -> Result<usize, WebTextOffsetError> {
    let mut utf16 = 0;
    for (utf8, character) in text.char_indices() {
        if utf16 == target {
            return Ok(utf8);
        }
        utf16 += character.len_utf16();
        if utf16 > target {
            return Err(WebTextOffsetError);
        }
    }
    (utf16 == target)
        .then_some(text.len())
        .ok_or(WebTextOffsetError)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn utf8_range_to_utf16(
    text: &str,
    range: Range<usize>,
) -> Result<Range<usize>, WebTextOffsetError> {
    if range.start > range.end {
        return Err(WebTextOffsetError);
    }
    Ok(utf8_offset_to_utf16(text, range.start)?..utf8_offset_to_utf16(text, range.end)?)
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn utf16_code_unit_ranges_to_utf8(
    text: &str,
    range: Range<usize>,
) -> Result<Vec<Range<usize>>, WebTextOffsetError> {
    if range.start > range.end {
        return Err(WebTextOffsetError);
    }
    let utf16_len = text.encode_utf16().count();
    if range.end > utf16_len {
        return Err(WebTextOffsetError);
    }

    let mut result = Vec::with_capacity(range.end - range.start);
    let mut utf16_start = 0;
    for (utf8_start, character) in text.char_indices() {
        let utf8_end = utf8_start + character.len_utf8();
        let utf16_end = utf16_start + character.len_utf16();
        for unit in utf16_start..utf16_end {
            if range.contains(&unit) {
                result.push(utf8_start..utf8_end);
            }
        }
        utf16_start = utf16_end;
    }
    Ok(result)
}

#[cfg(any(target_arch = "wasm32", test))]
fn utf8_offset_to_utf16(text: &str, target: usize) -> Result<usize, WebTextOffsetError> {
    if target > text.len() || !text.is_char_boundary(target) {
        return Err(WebTextOffsetError);
    }
    Ok(text[..target].encode_utf16().count())
}

fn normalize_edit_context_snapshot(
    text: String,
    update_start_utf16: usize,
    replacement: &str,
    selection_utf16: Range<usize>,
    composing: bool,
) -> Result<TextInputSnapshot, WebTextOffsetError> {
    let selection = utf16_range_to_utf8(&text, selection_utf16)?;
    let composition = if composing {
        let end = update_start_utf16
            .checked_add(replacement.encode_utf16().count())
            .ok_or(WebTextOffsetError)?;
        Some(utf16_range_to_utf8(&text, update_start_utf16..end)?)
    } else {
        None
    };
    Ok(TextInputSnapshot::new(text, selection, composition))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_input_value_is_one_committed_insert_and_then_cleared() {
        assert_eq!(
            normalize_input(false, "你好".to_owned()),
            Some(TextInputEvent::InsertText("你好".to_owned()))
        );
        assert_eq!(normalize_input(false, String::new()), None);
    }

    #[test]
    fn input_during_composition_is_not_committed_twice() {
        assert_eq!(normalize_input(true, "ni".to_owned()), None);
    }

    #[test]
    fn composing_input_reconciles_preedit_with_post_dom_selection() {
        assert_eq!(
            normalize_composition_input(true, "pinyin".to_owned(), Some(6..6)),
            Some(TextInputEvent::Preedit {
                text: "pinyin".to_owned(),
                selection: Some(6..6),
            })
        );
        assert_eq!(
            normalize_composition_input(false, "pinyin".to_owned(), Some(6..6)),
            None
        );
    }

    #[test]
    fn edit_context_updates_are_normalized_to_utf8_snapshots() {
        assert_eq!(
            normalize_edit_context_snapshot("a😀中".to_owned(), 1, "😀", 3..3, true),
            Ok(TextInputSnapshot::new("a😀中", 5..5, Some(1..5)))
        );
        assert_eq!(utf8_range_to_utf16("a😀中", 1..5), Ok(1..3));
        assert_eq!(
            utf16_code_unit_ranges_to_utf8("a😀中", 1..4),
            Ok(vec![1..5, 1..5, 5..8])
        );
    }

    #[test]
    fn edit_context_rejects_offsets_inside_surrogate_pairs() {
        assert_eq!(utf16_range_to_utf8("a😀中", 1..3), Ok(1..5));
        assert_eq!(utf16_range_to_utf8("a😀中", 3..4), Ok(5..8));
        assert_eq!(utf16_range_to_utf8("a😀中", 2..3), Err(WebTextOffsetError));
        assert_eq!(utf8_range_to_utf16("a😀中", 2..5), Err(WebTextOffsetError));
    }
}
