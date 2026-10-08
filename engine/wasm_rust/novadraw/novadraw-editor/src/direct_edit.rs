//! Viewer-scoped direct text editing contracts and transient state.

use std::{error::Error, fmt, ops::Range, sync::Arc};

use novadraw::FigureId;
use novadraw::figure::{FlowTextPosition, FlowTextRange};
use novadraw::text::{TextAffinity, TextMovement};

use crate::{
    Command, EditPartId, EditorNamespace, FeedbackVisual, ModelAdapter, ModelRevision, PolicyError,
    TextInputSnapshot,
};

/// Stable application-defined text feature exposed by one EditPart.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DirectTextFeature(Arc<str>);

impl DirectTextFeature {
    /// Creates a non-empty feature key.
    pub fn new(value: impl AsRef<str>) -> Result<Self, DirectTextFeatureError> {
        let value = value.as_ref();
        if value.is_empty() {
            Err(DirectTextFeatureError)
        } else {
            Ok(Self(Arc::from(value)))
        }
    }

    /// Returns the application-defined feature name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Empty direct-edit feature keys are invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTextFeatureError;

impl fmt::Display for DirectTextFeatureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("direct text feature must not be empty")
    }
}

impl Error for DirectTextFeatureError {}

/// Editing behavior for line-break input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextEditMode {
    /// Line breaks request acceptance instead of entering the draft.
    SingleLine,
    /// Line breaks are ordinary draft content.
    Multiline,
}

/// Action applied when the host text-input focus is lost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusLossPolicy {
    /// Accept a valid draft.
    Accept,
    /// Discard the draft.
    Cancel,
}

/// Immutable application description used to start one direct-edit session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTextEditDescriptor {
    feature: DirectTextFeature,
    initial_text: String,
    source_revision: ModelRevision,
    mode: TextEditMode,
    focus_loss: FocusLossPolicy,
}

impl DirectTextEditDescriptor {
    /// Creates a descriptor for one application text feature.
    pub fn new(
        feature: DirectTextFeature,
        initial_text: impl Into<String>,
        source_revision: ModelRevision,
        mode: TextEditMode,
        focus_loss: FocusLossPolicy,
    ) -> Self {
        Self {
            feature,
            initial_text: initial_text.into(),
            source_revision,
            mode,
            focus_loss,
        }
    }

    /// Returns the stable feature key.
    pub const fn feature(&self) -> &DirectTextFeature {
        &self.feature
    }

    /// Returns the initial draft projected from the application model.
    pub fn initial_text(&self) -> &str {
        &self.initial_text
    }

    /// Returns the application revision captured when editing started.
    pub const fn source_revision(&self) -> ModelRevision {
        self.source_revision
    }

    /// Returns the line-break behavior.
    pub const fn mode(&self) -> TextEditMode {
        self.mode
    }

    /// Returns the configured focus-loss behavior.
    pub const fn focus_loss(&self) -> FocusLossPolicy {
        self.focus_loss
    }
}

/// Typed intent to start direct editing of one feature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTextEditRequest {
    source: EditPartId,
    feature: DirectTextFeature,
}

impl DirectTextEditRequest {
    /// Creates a request for one source part and feature.
    pub const fn new(source: EditPartId, feature: DirectTextFeature) -> Self {
        Self { source, feature }
    }

    /// Returns the source EditPart.
    pub const fn source(&self) -> EditPartId {
        self.source
    }

    /// Returns the requested feature.
    pub const fn feature(&self) -> &DirectTextFeature {
        &self.feature
    }
}

/// Namespaced identity of one direct-edit session.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DirectTextEditSessionId {
    namespace: EditorNamespace,
    sequence: u64,
}

impl DirectTextEditSessionId {
    pub(crate) const fn new(namespace: EditorNamespace, sequence: u64) -> Self {
        Self {
            namespace,
            sequence,
        }
    }

    /// Returns the owning Viewer namespace.
    pub const fn namespace(self) -> EditorNamespace {
        self.namespace
    }

    /// Returns the Viewer-local sequence number.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// Whether a movement replaces or extends the current text selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtendTextSelection {
    /// Collapse the selection at the moved focus.
    No,
    /// Preserve the anchor and move only the focus.
    Yes,
}

/// Grapheme- or word-aware deletion requested from the active draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextDelete {
    /// Delete the previous visual grapheme cluster.
    PreviousVisual,
    /// Delete the next visual grapheme cluster.
    NextVisual,
    /// Delete to the previous visual word boundary.
    PreviousWord,
    /// Delete to the next visual word boundary.
    NextWord,
}

impl TextDelete {
    pub(crate) const fn movement(self) -> TextMovement {
        match self {
            Self::PreviousVisual => TextMovement::PreviousVisual,
            Self::NextVisual => TextMovement::NextVisual,
            Self::PreviousWord => TextMovement::PreviousWord,
            Self::NextWord => TextMovement::NextWord,
        }
    }

    pub(crate) const fn is_backward(self) -> bool {
        matches!(self, Self::PreviousVisual | Self::PreviousWord)
    }
}

/// Active IME preedit range and optional caret/selection inside that range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTextComposition {
    range: FlowTextRange,
    selection: Option<Range<usize>>,
}

impl DirectTextComposition {
    /// Returns the preedit range in the current draft.
    pub const fn range(&self) -> FlowTextRange {
        self.range
    }

    /// Returns the UTF-8 byte range relative to the preedit string.
    pub fn selection(&self) -> Option<Range<usize>> {
        self.selection.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompositionBase {
    draft: String,
    selection: FlowTextRange,
}

/// Read-only snapshot of one active direct-edit session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectTextEditState {
    session: DirectTextEditSessionId,
    source: EditPartId,
    feature: DirectTextFeature,
    source_revision: ModelRevision,
    initial_text: String,
    draft: String,
    selection: FlowTextRange,
    composition: Option<DirectTextComposition>,
    composition_base: Option<CompositionBase>,
    mode: TextEditMode,
    focus_loss: FocusLossPolicy,
}

impl DirectTextEditState {
    pub(crate) fn new(
        session: DirectTextEditSessionId,
        source: EditPartId,
        descriptor: &DirectTextEditDescriptor,
    ) -> Self {
        let draft = descriptor.initial_text.clone();
        let focus = document_end(&draft);
        Self {
            session,
            source,
            feature: descriptor.feature.clone(),
            source_revision: descriptor.source_revision,
            initial_text: draft.clone(),
            draft,
            selection: FlowTextRange::new(focus, focus),
            composition: None,
            composition_base: None,
            mode: descriptor.mode,
            focus_loss: descriptor.focus_loss,
        }
    }

    /// Returns the session identity.
    pub const fn session(&self) -> DirectTextEditSessionId {
        self.session
    }

    /// Returns the fixed source EditPart.
    pub const fn source(&self) -> EditPartId {
        self.source
    }

    /// Returns the fixed feature key.
    pub const fn feature(&self) -> &DirectTextFeature {
        &self.feature
    }

    /// Returns the source revision captured at session start.
    pub const fn source_revision(&self) -> ModelRevision {
        self.source_revision
    }

    /// Returns the immutable text captured at session start.
    pub fn initial_text(&self) -> &str {
        &self.initial_text
    }

    /// Returns the current transient draft.
    pub fn draft(&self) -> &str {
        &self.draft
    }

    /// Returns the directed text selection.
    pub const fn selection(&self) -> FlowTextRange {
        self.selection
    }

    /// Returns the current IME preedit range, if composition is active.
    pub const fn composition(&self) -> Option<&DirectTextComposition> {
        self.composition.as_ref()
    }

    /// Returns the line-break behavior.
    pub const fn mode(&self) -> TextEditMode {
        self.mode
    }

    /// Returns the configured focus-loss behavior.
    pub const fn focus_loss(&self) -> FocusLossPolicy {
        self.focus_loss
    }

    /// Returns whether accepting this draft would change the application text.
    pub fn is_changed(&self) -> bool {
        self.draft != self.initial_text
    }

    /// Returns the complete draft state for a platform text-input host.
    pub fn input_snapshot(&self) -> Result<TextInputSnapshot, DirectTextEditError> {
        let selection = absolute_range(&self.draft, self.selection)?;
        let composition = self
            .composition
            .as_ref()
            .map(|composition| absolute_range(&self.draft, composition.range))
            .transpose()?;
        Ok(TextInputSnapshot::new(
            self.draft.clone(),
            selection,
            composition,
        ))
    }

    pub(crate) fn set_selection(
        &mut self,
        selection: FlowTextRange,
    ) -> Result<(), DirectTextEditError> {
        position_to_offset(&self.draft, selection.anchor())?;
        position_to_offset(&self.draft, selection.focus())?;
        self.selection = FlowTextRange::new(
            document_position(selection.anchor()),
            document_position(selection.focus()),
        );
        Ok(())
    }

    pub(crate) fn replace_selection(
        &mut self,
        replacement: &str,
    ) -> Result<bool, DirectTextEditError> {
        if self.mode == TextEditMode::SingleLine
            && replacement
                .chars()
                .any(|character| matches!(character, '\n' | '\r'))
        {
            return Err(DirectTextEditError::LineBreakNotAllowed);
        }
        let anchor = position_to_offset(&self.draft, self.selection.anchor())?;
        let focus = position_to_offset(&self.draft, self.selection.focus())?;
        let start = anchor.min(focus);
        let end = anchor.max(focus);
        if start == end && replacement.is_empty() {
            return Ok(false);
        }
        self.draft.replace_range(start..end, replacement);
        let caret = position_from_offset(&self.draft, start + replacement.len())?;
        self.selection = FlowTextRange::new(caret, caret);
        Ok(true)
    }

    pub(crate) fn set_preedit(
        &mut self,
        text: &str,
        selection: Option<Range<usize>>,
    ) -> Result<bool, DirectTextEditError> {
        validate_relative_range(text, selection.as_ref())?;
        if self.composition.is_none() {
            self.composition_base = Some(CompositionBase {
                draft: self.draft.clone(),
                selection: self.selection,
            });
        } else if let Some(composition) = &self.composition {
            self.selection = composition.range;
        }
        let start = {
            let anchor = position_to_offset(&self.draft, self.selection.anchor())?;
            let focus = position_to_offset(&self.draft, self.selection.focus())?;
            anchor.min(focus)
        };
        self.replace_selection(text)?;
        let range = FlowTextRange::new(
            position_from_offset(&self.draft, start)?,
            position_from_offset(&self.draft, start + text.len())?,
        );
        self.composition = Some(DirectTextComposition { range, selection });
        self.selection = range;
        Ok(true)
    }

    pub(crate) fn synchronize_input(
        &mut self,
        snapshot: &TextInputSnapshot,
    ) -> Result<bool, DirectTextEditError> {
        let text = snapshot.text();
        if self.mode == TextEditMode::SingleLine
            && text
                .chars()
                .any(|character| matches!(character, '\n' | '\r'))
        {
            return Err(DirectTextEditError::LineBreakNotAllowed);
        }

        let selection_offsets = snapshot.selection();
        let selection = range_from_offsets(text, selection_offsets.clone())?;
        let composition_offsets = snapshot.composition();
        let composition = composition_offsets
            .as_ref()
            .map(|range| {
                if selection_offsets.start < range.start || selection_offsets.end > range.end {
                    return Err(DirectTextEditError::InvalidPreeditRange);
                }
                Ok(DirectTextComposition {
                    range: range_from_offsets(text, range.clone())
                        .map_err(|_| DirectTextEditError::InvalidPreeditRange)?,
                    selection: Some(
                        selection_offsets.start - range.start..selection_offsets.end - range.start,
                    ),
                })
            })
            .transpose()?;

        let changed =
            self.draft != text || self.selection != selection || self.composition != composition;
        if !changed {
            return Ok(false);
        }

        let composition_base = if composition.is_some() {
            self.composition_base.clone().or_else(|| {
                Some(CompositionBase {
                    draft: self.draft.clone(),
                    selection: self.selection,
                })
            })
        } else {
            None
        };
        self.draft = text.to_owned();
        self.selection = selection;
        self.composition = composition;
        self.composition_base = composition_base;
        Ok(true)
    }

    pub(crate) fn commit_preedit(&mut self, text: &str) -> Result<bool, DirectTextEditError> {
        if self.mode == TextEditMode::SingleLine
            && text
                .chars()
                .any(|character| matches!(character, '\n' | '\r'))
        {
            return Err(DirectTextEditError::LineBreakNotAllowed);
        }
        if let Some(composition) = self.composition.take() {
            self.selection = composition.range;
        }
        self.composition_base = None;
        self.replace_selection(text)
    }

    pub(crate) fn cancel_preedit(&mut self) -> bool {
        let Some(base) = self.composition_base.take() else {
            return false;
        };
        self.draft = base.draft;
        self.selection = base.selection;
        self.composition = None;
        true
    }

    pub(crate) fn select_all(&mut self) {
        self.selection = FlowTextRange::new(
            FlowTextPosition::new(0, 0, TextAffinity::Downstream),
            document_end(&self.draft),
        );
    }

    pub(crate) fn caret_position(&self) -> Result<FlowTextPosition, DirectTextEditError> {
        let Some(composition) = &self.composition else {
            return Ok(self.selection.focus());
        };
        let Some(selection) = &composition.selection else {
            return Ok(composition.range.focus());
        };
        let start = position_to_offset(&self.draft, composition.range.anchor())?;
        position_from_offset(&self.draft, start + selection.end)
    }

    pub(crate) fn delete_range(
        &mut self,
        first: FlowTextPosition,
        second: FlowTextPosition,
    ) -> Result<bool, DirectTextEditError> {
        self.selection = FlowTextRange::new(first, second);
        self.replace_selection("")
    }
}

/// Policy-produced transient visuals for one draft revision.
pub struct DirectTextFeedback {
    visuals: Vec<FeedbackVisual>,
    text_visual: usize,
}

impl DirectTextFeedback {
    /// Creates feedback and identifies the TextFlow visual used for interaction queries.
    pub fn new(visuals: Vec<FeedbackVisual>, text_visual: usize) -> Result<Self, PolicyError> {
        if text_visual >= visuals.len() {
            return Err(PolicyError::operation(
                "direct-edit text feedback index is out of bounds",
            ));
        }
        Ok(Self {
            visuals,
            text_visual,
        })
    }

    pub(crate) fn into_parts(self) -> (Vec<FeedbackVisual>, usize) {
        (self.visuals, self.text_visual)
    }
}

/// Gesture-scoped direct-edit projection created by an EditPolicy.
pub trait DirectTextEdit<A: ModelAdapter> {
    /// Returns the immutable session descriptor.
    fn descriptor(&self) -> &DirectTextEditDescriptor;

    /// Creates transient draft, selection, caret, and preedit visuals.
    fn feedback(
        &mut self,
        state: &DirectTextEditState,
        model: &A,
    ) -> Result<DirectTextFeedback, PolicyError>;

    /// Validates the final draft before stable feedback is removed.
    fn validate(&mut self, _state: &DirectTextEditState, _model: &A) -> Result<(), PolicyError> {
        Ok(())
    }

    /// Builds the one model-only Command used to accept the draft.
    fn command(
        &mut self,
        state: &DirectTextEditState,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}

pub(crate) struct ActiveDirectTextEdit<A: ModelAdapter> {
    pub(crate) state: DirectTextEditState,
    pub(crate) plan: Box<dyn DirectTextEdit<A>>,
    pub(crate) feedback: Vec<FigureId>,
    pub(crate) text_feedback: FigureId,
    pub(crate) horizontal_scroll: f64,
    pub(crate) caret_feedback: Option<FigureId>,
    pub(crate) caret_visible: bool,
}

pub(crate) struct PreparedDirectTextEdit<A: ModelAdapter> {
    pub(crate) active: ActiveDirectTextEdit<A>,
    pub(crate) command: Option<Box<dyn Command<A>>>,
}

/// Failure while editing a direct-text draft.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectTextEditError {
    /// No policy exposes the requested feature.
    UnsupportedFeature,
    /// More than one direct-edit session would own Viewer input.
    SessionAlreadyActive,
    /// No direct-edit session is active.
    NoActiveSession,
    /// The source part belongs to another Viewer or has retired.
    ForeignOrRetiredPart,
    /// A text position is outside the current draft or is not a UTF-8 boundary.
    InvalidTextPosition,
    /// The application revision changed since editing started.
    StaleSourceRevision,
    /// A line break was inserted into a single-line session.
    LineBreakNotAllowed,
    /// Session identity allocation was exhausted.
    SessionIdentityExhausted,
    /// The policy feedback did not expose a TextFlow interaction target.
    InvalidFeedbackTarget,
    /// A preedit caret or selection is not a valid UTF-8 range.
    InvalidPreeditRange,
    /// A session cannot be accepted while IME preedit remains active.
    ActiveComposition,
    /// An input event belongs to a released or superseded host lease.
    TextInputLeaseLost,
}

impl fmt::Display for DirectTextEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFeature => {
                formatter.write_str("the requested direct text feature is unsupported")
            }
            Self::SessionAlreadyActive => {
                formatter.write_str("a direct text edit session is already active")
            }
            Self::NoActiveSession => formatter.write_str("no direct text edit session is active"),
            Self::ForeignOrRetiredPart => {
                formatter.write_str("the direct text source is foreign or retired")
            }
            Self::InvalidTextPosition => formatter.write_str("text position is invalid"),
            Self::StaleSourceRevision => {
                formatter.write_str("the direct text source revision is stale")
            }
            Self::LineBreakNotAllowed => {
                formatter.write_str("line breaks are not allowed in this direct edit mode")
            }
            Self::SessionIdentityExhausted => {
                formatter.write_str("direct text edit session identity is exhausted")
            }
            Self::InvalidFeedbackTarget => {
                formatter.write_str("direct edit feedback has no TextFlow target")
            }
            Self::InvalidPreeditRange => formatter.write_str("preedit range is invalid"),
            Self::ActiveComposition => {
                formatter.write_str("direct text edit has an active composition")
            }
            Self::TextInputLeaseLost => {
                formatter.write_str("text input event belongs to an inactive lease")
            }
        }
    }
}

impl Error for DirectTextEditError {}

pub(crate) fn position_to_offset(
    text: &str,
    position: FlowTextPosition,
) -> Result<usize, DirectTextEditError> {
    let ranges = paragraph_ranges(text);
    let range = ranges
        .get(position.paragraph())
        .ok_or(DirectTextEditError::InvalidTextPosition)?;
    let offset = range
        .start
        .checked_add(position.byte_offset())
        .ok_or(DirectTextEditError::InvalidTextPosition)?;
    if offset > range.end || !text.is_char_boundary(offset) {
        return Err(DirectTextEditError::InvalidTextPosition);
    }
    Ok(offset)
}

pub(crate) fn position_from_offset(
    text: &str,
    offset: usize,
) -> Result<FlowTextPosition, DirectTextEditError> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err(DirectTextEditError::InvalidTextPosition);
    }
    let ranges = paragraph_ranges(text);
    if let Some((paragraph, range)) = ranges
        .iter()
        .enumerate()
        .find(|(_, range)| offset >= range.start && offset <= range.end)
    {
        return Ok(FlowTextPosition::new(
            paragraph,
            offset - range.start,
            TextAffinity::Downstream,
        ));
    }
    let paragraph = text[..offset].bytes().filter(|byte| *byte == b'\n').count();
    let range = ranges
        .get(paragraph)
        .ok_or(DirectTextEditError::InvalidTextPosition)?;
    Ok(FlowTextPosition::new(
        paragraph,
        offset - range.start,
        TextAffinity::Downstream,
    ))
}

fn absolute_range(text: &str, range: FlowTextRange) -> Result<Range<usize>, DirectTextEditError> {
    let anchor = position_to_offset(text, range.anchor())?;
    let focus = position_to_offset(text, range.focus())?;
    Ok(anchor.min(focus)..anchor.max(focus))
}

fn range_from_offsets(
    text: &str,
    range: Range<usize>,
) -> Result<FlowTextRange, DirectTextEditError> {
    if range.start > range.end {
        return Err(DirectTextEditError::InvalidTextPosition);
    }
    Ok(FlowTextRange::new(
        position_from_offset(text, range.start)?,
        position_from_offset(text, range.end)?,
    ))
}

fn document_end(text: &str) -> FlowTextPosition {
    let ranges = paragraph_ranges(text);
    let paragraph = ranges.len() - 1;
    FlowTextPosition::new(paragraph, ranges[paragraph].len(), TextAffinity::Upstream)
}

fn document_position(position: FlowTextPosition) -> FlowTextPosition {
    FlowTextPosition::new(
        position.paragraph(),
        position.byte_offset(),
        position.affinity(),
    )
}

fn paragraph_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (offset, character) in text.char_indices() {
        if character == '\n' {
            ranges.push(start..offset);
            start = offset + character.len_utf8();
        }
    }
    ranges.push(start..text.len());
    ranges
}

fn validate_relative_range(
    text: &str,
    range: Option<&Range<usize>>,
) -> Result<(), DirectTextEditError> {
    let Some(range) = range else {
        return Ok(());
    };
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(DirectTextEditError::InvalidPreeditRange);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(text: &str, mode: TextEditMode) -> DirectTextEditDescriptor {
        DirectTextEditDescriptor::new(
            DirectTextFeature::new("label").unwrap(),
            text,
            ModelRevision::initial(),
            mode,
            FocusLossPolicy::Cancel,
        )
    }

    #[test]
    fn draft_positions_preserve_paragraph_boundaries() {
        let namespace = EditorNamespace::new();
        let source = EditPartId::from_local(namespace, slotmap::KeyData::from_ffi(1));
        let mut state = DirectTextEditState::new(
            DirectTextEditSessionId::new(namespace, 1),
            source,
            &descriptor("first\nsecond", TextEditMode::Multiline),
        );
        state
            .set_selection(FlowTextRange::new(
                FlowTextPosition::new(0, 5, TextAffinity::Downstream),
                FlowTextPosition::new(1, 0, TextAffinity::Downstream),
            ))
            .unwrap();

        assert!(state.replace_selection(" ").unwrap());
        assert_eq!(state.draft(), "first second");
        assert_eq!(state.selection().focus().byte_offset(), 6);
    }

    #[test]
    fn single_line_draft_rejects_line_breaks() {
        let namespace = EditorNamespace::new();
        let source = EditPartId::from_local(namespace, slotmap::KeyData::from_ffi(1));
        let mut state = DirectTextEditState::new(
            DirectTextEditSessionId::new(namespace, 1),
            source,
            &descriptor("name", TextEditMode::SingleLine),
        );

        assert_eq!(
            state.replace_selection("\n"),
            Err(DirectTextEditError::LineBreakNotAllowed)
        );
        assert_eq!(state.draft(), "name");
    }

    #[test]
    fn synchronized_input_snapshot_updates_draft_selection_and_composition_atomically() {
        let namespace = EditorNamespace::new();
        let source = EditPartId::from_local(namespace, slotmap::KeyData::from_ffi(1));
        let mut state = DirectTextEditState::new(
            DirectTextEditSessionId::new(namespace, 1),
            source,
            &descriptor("a中", TextEditMode::SingleLine),
        );

        assert!(
            state
                .synchronize_input(&TextInputSnapshot::new("a😀中", 5..5, Some(1..5),))
                .unwrap()
        );
        assert_eq!(state.draft(), "a😀中");
        assert_eq!(
            absolute_range(state.draft(), state.selection()).unwrap(),
            5..5
        );
        assert_eq!(
            state.composition().map(|composition| absolute_range(
                state.draft(),
                composition.range()
            )
            .unwrap()),
            Some(1..5)
        );
        assert_eq!(state.composition().unwrap().selection(), Some(4..4));

        assert!(
            state
                .synchronize_input(&TextInputSnapshot::new("a你中", 4..4, None))
                .unwrap()
        );
        assert_eq!(state.draft(), "a你中");
        assert!(state.composition().is_none());
    }

    #[test]
    fn synchronized_input_snapshot_rejects_invalid_utf8_boundaries() {
        let namespace = EditorNamespace::new();
        let source = EditPartId::from_local(namespace, slotmap::KeyData::from_ffi(1));
        let mut state = DirectTextEditState::new(
            DirectTextEditSessionId::new(namespace, 1),
            source,
            &descriptor("a😀", TextEditMode::SingleLine),
        );

        assert_eq!(
            state.synchronize_input(&TextInputSnapshot::new("a😀", 2..2, None)),
            Err(DirectTextEditError::InvalidTextPosition)
        );
        assert_eq!(state.draft(), "a😀");
    }
}
