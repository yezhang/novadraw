use std::convert::Infallible;

use novadraw::{
    ComponentUpdateError, Figure, FigureComponentContext, FigureComponentUpdate, NdCanvas,
    PreparedFigureUpdate, Rectangle, RectangleFigure,
};
use novadraw_editor::{
    EditPartBehavior, EditPartError, EditPartFactory, GraphicalViewer, ModelAdapter, ModelEvent,
    ModelRevision, PartFactoryContext, VisualBuildContext, VisualUpdateContext,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ModelId;

#[derive(Clone, Copy)]
struct Changed;

struct BadgeModel {
    revision: ModelRevision,
    text: String,
    events: Vec<ModelEvent<ModelId, Changed>>,
    foreign_update_target: Option<novadraw::FigureId>,
}

impl BadgeModel {
    fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.revision = self.revision.next().unwrap();
        self.events
            .push(ModelEvent::new(self.revision, ModelId, Changed));
    }
}

impl ModelAdapter for BadgeModel {
    type Error = Infallible;
    type Event = Changed;
    type ModelId = ModelId;

    fn root(&self) -> Self::ModelId {
        ModelId
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, _model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(Vec::new())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        std::mem::take(&mut self.events)
    }
}

struct BadgeFigure {
    text: String,
}

impl Figure for BadgeFigure {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(4.0, 4.0, 80.0, 24.0)
    }

    fn name(&self) -> &'static str {
        "ExternalBadgeFigure"
    }

    fn paint_figure(&self, _canvas: &mut NdCanvas) {}
}

struct SetBadgeText(String);

impl FigureComponentUpdate for SetBadgeText {
    type Figure = BadgeFigure;
    type Prepared = String;
    type Error = &'static str;

    fn prepare(
        self,
        _current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        if self.0.is_empty() {
            Err("badge text cannot be empty")
        } else {
            Ok(PreparedFigureUpdate::paint(self.0))
        }
    }

    fn commit(prepared: Self::Prepared, target: &mut Self::Figure) {
        target.text = prepared;
    }
}

struct ProbeBadge;

impl FigureComponentUpdate for ProbeBadge {
    type Figure = BadgeFigure;
    type Prepared = ();
    type Error = String;

    fn prepare(
        self,
        current: &Self::Figure,
        _context: FigureComponentContext,
    ) -> Result<PreparedFigureUpdate<Self::Prepared>, Self::Error> {
        Err(current.text.clone())
    }

    fn commit(_prepared: Self::Prepared, _target: &mut Self::Figure) {
        unreachable!("probe updates never commit");
    }
}

struct BadgePart {
    badge: Option<novadraw::FigureId>,
}

impl EditPartBehavior<BadgeModel> for BadgePart {
    fn create_figure(
        &mut self,
        _model: &BadgeModel,
        _model_id: ModelId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(RectangleFigure::new(0.0, 0.0, 120.0, 40.0)))
    }

    fn configure_visual(
        &mut self,
        _model: &BadgeModel,
        _model_id: ModelId,
        context: &mut VisualBuildContext<'_>,
    ) -> Result<(), EditPartError> {
        self.badge = Some(context.add_child(
            context.primary(),
            Box::new(BadgeFigure {
                text: "uninitialized".to_owned(),
            }),
        )?);
        Ok(())
    }

    fn refresh_visuals(
        &mut self,
        model: &BadgeModel,
        _model_id: ModelId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.update_visual_component(
            model.foreign_update_target.unwrap_or(
                self.badge
                    .ok_or_else(|| EditPartError::operation("badge visual is not configured"))?,
            ),
            SetBadgeText(model.text.clone()),
        )?;
        Ok(())
    }
}

struct BadgeFactory;

impl EditPartFactory<BadgeModel> for BadgeFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<ModelId>,
        _model: &BadgeModel,
    ) -> Result<Box<dyn EditPartBehavior<BadgeModel>>, EditPartError> {
        Ok(Box::new(BadgePart { badge: None }))
    }
}

#[test]
fn external_compound_visual_updates_private_component_during_model_refresh() {
    let model = BadgeModel {
        revision: ModelRevision::initial(),
        text: "initial".to_owned(),
        events: Vec::new(),
        foreign_update_target: None,
    };
    let mut viewer =
        GraphicalViewer::new(model, BadgeFactory, Rectangle::new(0.0, 0.0, 320.0, 200.0)).unwrap();
    let part = viewer.part_for_model(ModelId).unwrap();
    let badge = viewer.parts().get(part).unwrap().visuals()[1];
    assert_eq!(viewer.runtime().tree().component_revision(badge), Some(1));

    viewer.model_mut().unwrap().set_text("refreshed");
    assert!(viewer.refresh().unwrap());

    assert_eq!(viewer.runtime().tree().component_revision(badge), Some(2));
    let snapshot = viewer
        .runtime_mut()
        .figure(badge)
        .unwrap()
        .update_component(ProbeBadge)
        .unwrap_err();
    assert!(matches!(snapshot, ComponentUpdateError::Rejected(text) if text == "refreshed"));
}

#[test]
fn model_refresh_rejects_a_component_target_outside_the_part_visuals() {
    let mut foreign_tree = novadraw::FigureTree::new();
    let foreign = foreign_tree.builder().set_contents(Box::new(BadgeFigure {
        text: "foreign".to_owned(),
    }));
    let model = BadgeModel {
        revision: ModelRevision::initial(),
        text: "initial".to_owned(),
        events: Vec::new(),
        foreign_update_target: Some(foreign),
    };

    assert!(
        GraphicalViewer::new(model, BadgeFactory, Rectangle::new(0.0, 0.0, 320.0, 200.0),).is_err()
    );
}
