//! Platform-neutral direct text input events and host effects.

use std::ops::Range;

use novadraw::{Rectangle, TextMovement};

use crate::{DirectTextEditSessionId, ExtendTextSelection, TextDelete};

/// Semantic purpose exposed to a platform text-input host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInputPurpose {
    /// One line of plain text.
    SingleLine,
    /// Plain text with hard line breaks.
    Multiline,
}

/// Host operation emitted by the Editor direct-edit session.
#[derive(Clone, Debug, PartialEq)]
pub enum TextInputEffect {
    /// Acquire exclusive text-input focus and place the candidate window.
    Acquire {
        /// Session that owns the lease.
        session: DirectTextEditSessionId,
        /// Keyboard/input purpose.
        purpose: TextInputPurpose,
        /// Current caret rectangle in logical surface coordinates.
        area: Rectangle,
    },
    /// Move the platform input/candidate area.
    SetArea {
        /// Session that owns the lease.
        session: DirectTextEditSessionId,
        /// Current caret rectangle in logical surface coordinates.
        area: Rectangle,
    },
    /// Release text-input focus for the completed or cancelled session.
    Release {
        /// Session whose lease must be released.
        session: DirectTextEditSessionId,
    },
}

impl TextInputEffect {
    /// Returns the session associated with this effect.
    pub const fn session(&self) -> DirectTextEditSessionId {
        match self {
            Self::Acquire { session, .. }
            | Self::SetArea { session, .. }
            | Self::Release { session } => *session,
        }
    }
}

/// Platform-normalized direct text input action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextInputEvent {
    /// Replace or update the current IME preedit.
    Preedit {
        /// Preedit string.
        text: String,
        /// UTF-8 byte selection relative to `text`, or `None` to hide the composition caret.
        selection: Option<Range<usize>>,
    },
    /// Restore the draft captured before composition started.
    CancelComposition,
    /// Insert committed text into the draft.
    InsertText(String),
    /// Delete a selection, visual grapheme, or visual word.
    Delete(TextDelete),
    /// Move the text focus, optionally extending selection.
    Move {
        /// Layout-aware movement.
        movement: TextMovement,
        /// Whether the selection anchor remains fixed.
        extend: ExtendTextSelection,
    },
    /// Select the complete draft.
    SelectAll,
    /// Accept the complete direct-edit session.
    Accept,
    /// Cancel the complete direct-edit session.
    Cancel,
    /// Apply the descriptor's focus-loss policy.
    FocusLost,
    /// The host lost or disabled this session's text-input lease.
    LeaseLost,
}

/// One normalized event tagged with the lease owner that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionTextInputEvent {
    session: DirectTextEditSessionId,
    event: TextInputEvent,
}

impl SessionTextInputEvent {
    /// Creates a session-tagged input event.
    pub const fn new(session: DirectTextEditSessionId, event: TextInputEvent) -> Self {
        Self { session, event }
    }

    /// Returns the source session.
    pub const fn session(&self) -> DirectTextEditSessionId {
        self.session
    }

    /// Returns the normalized input action.
    pub const fn event(&self) -> &TextInputEvent {
        &self.event
    }

    /// Consumes the wrapper and returns the input action.
    pub fn into_event(self) -> TextInputEvent {
        self.event
    }
}
