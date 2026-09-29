//! DOM text-input normalization for the optional Novadraw editor integration.

use std::ops::Range;

use novadraw::Rectangle;
use novadraw_editor::{
    DirectTextEditSessionId, SessionTextInputEvent, TextDelete, TextInputEffect, TextInputEvent,
    TextInputPurpose,
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
                    novadraw::TextMovement::PreviousWord
                } else {
                    novadraw::TextMovement::PreviousVisual
                },
                extend,
            },
            "ArrowRight" => TextInputEvent::Move {
                movement: if alt || control {
                    novadraw::TextMovement::NextWord
                } else {
                    novadraw::TextMovement::NextVisual
                },
                extend,
            },
            "ArrowUp" => TextInputEvent::Move {
                movement: novadraw::TextMovement::PreviousLine,
                extend,
            },
            "ArrowDown" => TextInputEvent::Move {
                movement: novadraw::TextMovement::NextLine,
                extend,
            },
            "Home" => TextInputEvent::Move {
                movement: novadraw::TextMovement::LineStart,
                extend,
            },
            "End" => TextInputEvent::Move {
                movement: novadraw::TextMovement::LineEnd,
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
}
