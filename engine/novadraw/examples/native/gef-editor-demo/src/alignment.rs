//! Shapes alignment is an external typed request, resolved by each Shape's policy.
use crate::{Session, model::Diagram};
use novadraw::geometry::Rectangle;
use novadraw_editor::{EditingContribution, RequestKey, RequestMetadata, RequestProtocol};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

impl Alignment {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Left => "Align left",
            Self::HorizontalCenter => "Align horizontal centers",
            Self::Right => "Align right",
            Self::Top => "Align top",
            Self::VerticalCenter => "Align vertical centers",
            Self::Bottom => "Align bottom",
        }
    }

    pub fn bounds(self, mut bounds: Rectangle, reference: Rectangle) -> Rectangle {
        match self {
            Self::Left => bounds.x = reference.x,
            Self::HorizontalCenter => {
                bounds.x = reference.x + (reference.width - bounds.width) / 2.0;
            }
            Self::Right => bounds.x = reference.x + reference.width - bounds.width,
            Self::Top => bounds.y = reference.y,
            Self::VerticalCenter => {
                bounds.y = reference.y + (reference.height - bounds.height) / 2.0;
            }
            Self::Bottom => bounds.y = reference.y + reference.height - bounds.height,
        }
        bounds
    }
}

pub(crate) struct AlignShapes;

pub(crate) struct AlignmentRequest {
    pub alignment: Alignment,
    pub reference: Rectangle,
}

impl RequestProtocol<Diagram> for AlignShapes {
    type Payload = AlignmentRequest;
    type Contribution = EditingContribution<Diagram>;

    fn key() -> RequestKey<Self> {
        RequestKey::new("Align shapes")
    }
}

pub(crate) fn can_align(session: &Session) -> bool {
    let selection = session.viewer.selection();
    let model = session.viewer.model();
    let is_shape = |part| {
        session
            .viewer
            .parts()
            .get(part)
            .and_then(|part| part.model_id())
            .is_some_and(|id| model.shapes.contains_key(&id))
    };
    !session.domain.has_active_gesture()
        && selection.primary().is_some_and(is_shape)
        && selection
            .items()
            .iter()
            .filter(|part| is_shape(**part))
            .count()
            >= 2
}

pub(crate) fn align(session: &mut Session, alignment: Alignment) -> Result<(), String> {
    if !can_align(session) {
        return Ok(());
    }
    let primary = session.viewer.selection().primary().unwrap();
    let primary_id = session
        .viewer
        .parts()
        .get(primary)
        .unwrap()
        .model_id()
        .unwrap();
    let reference = session.viewer.model().shapes[&primary_id].bounds;
    // This flat sample's Shapes share a parent and store bounds in the same model space.
    // Capture the reference once; skip unchanged parts so a repeated align adds no history.
    let targets = session
        .viewer
        .selection()
        .items()
        .iter()
        .copied()
        .filter(|part| {
            session
                .viewer
                .parts()
                .get(*part)
                .and_then(|part| part.model_id())
                .and_then(|id| session.viewer.model().shapes.get(&id))
                .is_some_and(|shape| alignment.bounds(shape.bounds, reference) != shape.bounds)
        })
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Ok(());
    }
    let revision = session
        .domain
        .next_interaction_revision()
        .map_err(|e| e.to_string())?;
    let metadata = RequestMetadata::new(revision, targets);
    session
        .domain
        .execute::<AlignShapes, _>(
            &mut session.viewer,
            &metadata,
            &AlignmentRequest {
                alignment,
                reference,
            },
        )
        .map_err(|e| e.to_string())
}
