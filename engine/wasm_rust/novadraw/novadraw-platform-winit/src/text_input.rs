//! Winit IME bridge for the optional Novadraw editor integration.

use novadraw_editor::{
    DirectTextEditSessionId, ExtendTextSelection, SessionTextInputEvent, TextDelete,
    TextInputEffect, TextInputEvent, TextInputPurpose,
};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::Ime,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

/// Applies Editor text-input effects and tags Winit IME events with the active lease.
#[derive(Debug, Default)]
pub struct WinitTextInputBridge {
    active: Option<DirectTextEditSessionId>,
    composing: bool,
    purpose: Option<TextInputPurpose>,
    suppress_text: Option<String>,
}

impl WinitTextInputBridge {
    /// Creates an idle bridge.
    pub const fn new() -> Self {
        Self {
            active: None,
            composing: false,
            purpose: None,
            suppress_text: None,
        }
    }

    /// Returns the session currently owning Winit IME input.
    pub const fn active_session(&self) -> Option<DirectTextEditSessionId> {
        self.active
    }

    /// Returns whether Winit has reported a non-empty preedit.
    pub const fn is_composing(&self) -> bool {
        self.composing
    }

    /// Applies one Editor effect to a Winit window.
    ///
    /// Returns `false` for stale effects that do not belong to the active lease.
    pub fn apply_effect(&mut self, window: &Window, effect: &TextInputEffect) -> bool {
        match effect {
            TextInputEffect::Acquire {
                session,
                purpose,
                area,
            } => {
                self.active = Some(*session);
                self.composing = false;
                self.purpose = Some(*purpose);
                self.suppress_text = None;
                window.set_ime_allowed(true);
                set_cursor_area(window, *area);
                true
            }
            TextInputEffect::SetArea { session, area } if self.active == Some(*session) => {
                set_cursor_area(window, *area);
                true
            }
            TextInputEffect::Release { session } if self.active == Some(*session) => {
                self.active = None;
                self.composing = false;
                self.purpose = None;
                self.suppress_text = None;
                window.set_ime_allowed(false);
                true
            }
            TextInputEffect::SetArea { .. } | TextInputEffect::Release { .. } => false,
        }
    }

    /// Converts one Winit IME event into the platform-neutral Editor protocol.
    pub fn adapt_ime(&mut self, event: Ime) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        let event = normalize_ime(event, &mut self.composing)?;
        if let TextInputEvent::InsertText(text) = &event {
            self.suppress_text = Some(text.clone());
        }
        if matches!(event, TextInputEvent::LeaseLost) {
            self.active = None;
            self.purpose = None;
        }
        Some(SessionTextInputEvent::new(session, event))
    }

    /// Converts one pressed Winit key into direct-edit navigation or committed text.
    pub fn adapt_key_pressed(
        &mut self,
        key: PhysicalKey,
        text: Option<&str>,
        modifiers: novadraw::KeyModifiers,
    ) -> Option<SessionTextInputEvent> {
        let session = self.active?;
        if self.composing && key != PhysicalKey::Code(KeyCode::Escape) {
            return None;
        }
        let extend = if modifiers.shift {
            ExtendTextSelection::Yes
        } else {
            ExtendTextSelection::No
        };
        let event = match key {
            PhysicalKey::Code(KeyCode::Escape) if self.composing => {
                self.composing = false;
                TextInputEvent::CancelComposition
            }
            PhysicalKey::Code(KeyCode::Escape) => TextInputEvent::Cancel,
            PhysicalKey::Code(KeyCode::Enter)
                if self.purpose == Some(TextInputPurpose::Multiline)
                    && !modifiers.control
                    && !modifiers.meta =>
            {
                TextInputEvent::InsertText("\n".to_owned())
            }
            PhysicalKey::Code(KeyCode::Enter) => TextInputEvent::Accept,
            PhysicalKey::Code(KeyCode::Backspace) => {
                TextInputEvent::Delete(if modifiers.alt || modifiers.control {
                    TextDelete::PreviousWord
                } else {
                    TextDelete::PreviousVisual
                })
            }
            PhysicalKey::Code(KeyCode::Delete) => {
                TextInputEvent::Delete(if modifiers.alt || modifiers.control {
                    TextDelete::NextWord
                } else {
                    TextDelete::NextVisual
                })
            }
            PhysicalKey::Code(KeyCode::ArrowLeft) => TextInputEvent::Move {
                movement: if modifiers.alt || modifiers.control {
                    novadraw::TextMovement::PreviousWord
                } else {
                    novadraw::TextMovement::PreviousVisual
                },
                extend,
            },
            PhysicalKey::Code(KeyCode::ArrowRight) => TextInputEvent::Move {
                movement: if modifiers.alt || modifiers.control {
                    novadraw::TextMovement::NextWord
                } else {
                    novadraw::TextMovement::NextVisual
                },
                extend,
            },
            PhysicalKey::Code(KeyCode::ArrowUp) => TextInputEvent::Move {
                movement: novadraw::TextMovement::PreviousLine,
                extend,
            },
            PhysicalKey::Code(KeyCode::ArrowDown) => TextInputEvent::Move {
                movement: novadraw::TextMovement::NextLine,
                extend,
            },
            PhysicalKey::Code(KeyCode::Home) => TextInputEvent::Move {
                movement: novadraw::TextMovement::LineStart,
                extend,
            },
            PhysicalKey::Code(KeyCode::End) => TextInputEvent::Move {
                movement: novadraw::TextMovement::LineEnd,
                extend,
            },
            PhysicalKey::Code(KeyCode::KeyA) if modifiers.control || modifiers.meta => {
                TextInputEvent::SelectAll
            }
            _ if !self.composing && !modifiers.control && !modifiers.meta && !modifiers.alt => {
                let text = text?.to_owned();
                if text.is_empty() {
                    return None;
                }
                if self.suppress_text.as_deref() == Some(text.as_str()) {
                    self.suppress_text = None;
                    return None;
                }
                self.suppress_text = None;
                TextInputEvent::InsertText(text)
            }
            _ => return None,
        };
        Some(SessionTextInputEvent::new(session, event))
    }
}

fn normalize_ime(event: Ime, composing: &mut bool) -> Option<TextInputEvent> {
    Some(match event {
        Ime::Enabled => return None,
        Ime::Preedit(text, _) if text.is_empty() => {
            *composing = false;
            TextInputEvent::CancelComposition
        }
        Ime::Preedit(text, selection) => {
            *composing = true;
            TextInputEvent::Preedit {
                text,
                selection: selection.map(|(start, end)| start..end),
            }
        }
        Ime::Commit(text) => {
            *composing = false;
            TextInputEvent::InsertText(text)
        }
        Ime::Disabled => {
            *composing = false;
            TextInputEvent::LeaseLost
        }
    })
}

fn set_cursor_area(window: &Window, area: novadraw::Rectangle) {
    window.set_ime_cursor_area(
        LogicalPosition::new(area.x, area.y),
        LogicalSize::new(area.width, area.height),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preedit_commit_and_disable_have_deterministic_editor_events() {
        let mut composing = false;
        assert!(matches!(
            normalize_ime(Ime::Preedit("ni".to_owned(), Some((2, 2))), &mut composing),
            Some(TextInputEvent::Preedit {
                text,
                selection: Some(selection)
            }) if text == "ni" && selection == (2..2)
        ));
        assert!(composing);
        assert_eq!(
            normalize_ime(Ime::Commit("你".to_owned()), &mut composing),
            Some(TextInputEvent::InsertText("你".to_owned()))
        );
        assert!(!composing);
        assert_eq!(
            normalize_ime(Ime::Disabled, &mut composing),
            Some(TextInputEvent::LeaseLost)
        );
    }

    #[test]
    fn empty_preedit_cancels_the_editor_composition_base() {
        let mut composing = true;
        assert_eq!(
            normalize_ime(Ime::Preedit(String::new(), None), &mut composing),
            Some(TextInputEvent::CancelComposition)
        );
        assert!(!composing);
    }
}
