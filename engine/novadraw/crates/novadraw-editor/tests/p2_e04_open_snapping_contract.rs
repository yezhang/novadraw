use novadraw::{
    event::DispatchOutcome,
    figure::{Figure, RectangleFigure},
    geometry::{Point, Rectangle},
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
};
use novadraw_editor::{
    Command, EditPartBehavior, EditPartError, EditPartFactory, EditorDomain, GraphicalViewer,
    InteractionRevision, ModelAdapter, ModelEvent, ModelRevision, PartFactoryContext, RequestKey,
    RequestProtocol, Tool, ToolContext, ToolDirective, ToolError, ToolKey, ToolMouseEvent,
    ViewerInputOutcome, container::*, snap::*,
};
use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    num::NonZeroUsize,
    rc::Rc,
};

struct Model {
    version: Rc<Cell<ModelRevision>>,
    resize: Rc<RefCell<ResizeState>>,
    events: Rc<RefCell<Vec<ModelEvent<u32, ()>>>>,
}
struct ResizeState {
    shape: ResizeShape,
    actual: Rectangle,
    rewrite: novadraw::geometry::Vec2,
    rewrite_width: f64,
    builds: usize,
    plans: usize,
    command: bool,
    no_plan: bool,
    invalidate_at_plan: bool,
}
impl Default for ResizeState {
    fn default() -> Self {
        Self {
            shape: ResizeShape::new(
                bounds(1),
                novadraw::geometry::Affine2D::IDENTITY,
                novadraw::geometry::Dimension::new(10.0, 10.0),
                None,
            )
            .unwrap(),
            actual: bounds(1),
            rewrite: novadraw::geometry::Vec2::ZERO,
            rewrite_width: 0.0,
            builds: 0,
            plans: 0,
            command: false,
            no_plan: false,
            invalidate_at_plan: false,
        }
    }
}
impl ModelAdapter for Model {
    type ModelId = u32;
    type Event = ();
    type Error = Infallible;
    fn root(&self) -> u32 {
        0
    }
    fn revision(&self) -> ModelRevision {
        self.version.get()
    }
    fn children(&self, id: u32) -> Result<Vec<u32>, Infallible> {
        Ok(if id == 0 { vec![1, 2] } else { vec![] })
    }
    fn drain_events(&mut self) -> Vec<ModelEvent<u32, ()>> {
        std::mem::take(&mut *self.events.borrow_mut())
    }
}
struct Factory;
struct Part;
fn bounds(id: u32) -> Rectangle {
    match id {
        0 => Rectangle::new(0.0, 0.0, 800.0, 600.0),
        1 => Rectangle::new(40.0, 20.0, 30.0, 20.0),
        _ => Rectangle::new(150.0, 20.0, 30.0, 20.0),
    }
}
impl EditPartFactory<Model> for Factory {
    fn create(
        &mut self,
        _: PartFactoryContext<u32>,
        _: &Model,
    ) -> Result<Box<dyn EditPartBehavior<Model>>, EditPartError> {
        Ok(Box::new(Part))
    }
}
impl EditPartBehavior<Model> for Part {
    fn create_figure(&mut self, _: &Model, id: u32) -> Result<Box<dyn Figure>, EditPartError> {
        Ok(Box::new(RectangleFigure::from_bounds(bounds(id)).unwrap()))
    }
    fn create_policies(
        &mut self,
        _: &Model,
        id: u32,
    ) -> Result<Vec<novadraw_editor::PolicyInstallation<Model>>, EditPartError> {
        if id != 0 {
            return Ok(vec![]);
        }
        Ok(vec![
            novadraw_editor::PolicyInstallation::new(
                novadraw_editor::LayoutSlot::key(),
                ReceiptPolicy,
            )
            .map_err(|error| EditPartError::operation(error.to_string()))?,
        ])
    }
}

struct ReceiptPolicy;
impl novadraw_editor::EditPolicy<Model> for ReceiptPolicy {
    fn register_handlers(builder: &mut novadraw_editor::PolicyHandlerBuilder<'_, Model, Self>) {
        builder.handle::<ReceiptMove>(|_, _, _, candidate, _| {
            Ok(Some(ReceiptMovePlan {
                header: MovePlanHeader::from_candidate(candidate),
                geometry: candidate.geometry().to_vec(),
            }))
        });
        builder.handle::<CheckContainerPermission>(|_, _, _, _, _| {
            Ok(Some(PermissionDecision::Allow))
        });
        builder.handle::<ReceiptCreate>(|_, _, _, candidate, _| {
            Ok(Some(ReceiptCreatePlan {
                header: CreationPlanHeader::from_candidate(candidate),
            }))
        });
        builder.handle::<ReceiptResize>(|_, _, _, candidate, model| {
            let mut state = model.resize.borrow_mut();
            state.plans += 1;
            if state.no_plan {
                return Ok(None);
            }
            let mut actual = candidate.bounds();
            actual.x += state.rewrite.x();
            actual.y += state.rewrite.y();
            actual.width += state.rewrite_width;
            let invalidate = state.invalidate_at_plan;
            drop(state);
            if invalidate {
                model.version.set(model.version.get().next().unwrap());
                model
                    .events
                    .borrow_mut()
                    .push(ModelEvent::new(model.version.get(), 1, ()));
            }
            Ok(Some(ReceiptResizePlan {
                header: ResizePlanHeader::from_candidate(candidate),
                actual,
            }))
        });
    }
}
struct ReceiptResize;
struct ReceiptResizePlan {
    header: ResizePlanHeader<u32>,
    actual: Rectangle,
}
impl RequestProtocol<Model> for ReceiptResize {
    type Payload = ResizeCandidate<u32>;
    type Contribution = ReceiptResizePlan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-receipt-resize")
    }
}
impl ResizeProtocol<Model> for ReceiptResize {
    type Payload = ResizeCandidate<u32>;
    type Plan = ReceiptResizePlan;
    type Request = Self;
    fn source_shape(model: &Model, _: &MoveSource<u32>) -> Result<ResizeShape, ContainerEditError> {
        Ok(model.resize.borrow().shape.clone())
    }
    fn payload(candidate: &ResizeCandidate<u32>) -> Self::Payload {
        candidate.clone()
    }
}
impl ResizePlan<Model> for ReceiptResizePlan {
    fn header(&self) -> &ResizePlanHeader<u32> {
        &self.header
    }
    fn actual_bounds(&self) -> Result<Rectangle, ContainerEditError> {
        Ok(self.actual)
    }
    fn feedback(&self) -> Result<Vec<novadraw_editor::FeedbackVisual>, ContainerEditError> {
        Ok(vec![])
    }
    fn build_command(
        self,
        model: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        let mut state = model.resize.borrow_mut();
        state.builds += 1;
        Ok(state.command.then(|| {
            Box::new(ResizeCommand {
                before: state.actual,
                after: self.actual,
            }) as Box<dyn Command<Model>>
        }))
    }
}
struct ResizeCommand {
    before: Rectangle,
    after: Rectangle,
}
impl ResizeCommand {
    fn apply(model: &mut Model, bounds: Rectangle) {
        model.resize.borrow_mut().actual = bounds;
        model.version.set(model.version.get().next().unwrap());
        model
            .events
            .borrow_mut()
            .push(ModelEvent::new(model.version.get(), 1, ()));
    }
}
impl Command<Model> for ResizeCommand {
    fn label(&self) -> &str {
        "External resize"
    }
    fn execute(&mut self, model: &mut Model) -> Result<(), novadraw_editor::CommandError> {
        Self::apply(model, self.after);
        Ok(())
    }
    fn undo(&mut self, model: &mut Model) -> Result<(), novadraw_editor::CommandError> {
        Self::apply(model, self.before);
        Ok(())
    }
}
struct ReceiptMove;
struct ReceiptMovePlan {
    header: MovePlanHeader<u32>,
    geometry: Vec<ConstraintGeometry>,
}
impl RequestProtocol<Model> for ReceiptMove {
    type Payload = MoveCandidate<u32>;
    type Contribution = ReceiptMovePlan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-receipt-move")
    }
}
impl ContainerMoveProtocol<Model> for ReceiptMove {
    type Payload = MoveCandidate<u32>;
    type Plan = ReceiptMovePlan;
    type Request = Self;
    fn payload(candidate: &MoveCandidate<u32>) -> Self::Payload {
        candidate.clone()
    }
    fn source_geometry(
        model: &Model,
        source: &MoveSource<u32>,
    ) -> Result<ConstraintGeometry, ContainerEditError> {
        Move::source_geometry(model, source)
    }
}
impl MovePlan<Model> for ReceiptMovePlan {
    fn header(&self) -> &MovePlanHeader<u32> {
        &self.header
    }
    fn actual_geometry(&self) -> Result<Option<Vec<ConstraintGeometry>>, ContainerEditError> {
        Ok(Some(self.geometry.clone()))
    }
    fn feedback(&self) -> Result<Vec<novadraw_editor::FeedbackVisual>, ContainerEditError> {
        Ok(vec![])
    }
    fn build_command(
        self,
        _: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        Ok(None)
    }
}
struct ReceiptCreate;
struct ReceiptCreatePlan {
    header: CreationPlanHeader<u32>,
}
impl RequestProtocol<Model> for ReceiptCreate {
    type Payload = CreationCandidate<u32>;
    type Contribution = ReceiptCreatePlan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("external-receipt-create")
    }
}
impl ContainerCreationTemplate<Model> for ReceiptCreate {
    type Payload = CreationCandidate<u32>;
    type Plan = ReceiptCreatePlan;
    type Request = Self;
    fn default_size(&self) -> novadraw::geometry::Dimension {
        novadraw::geometry::Dimension::new(30.0, 20.0)
    }
    fn payload(&self, candidate: &CreationCandidate<u32>) -> Self::Payload {
        candidate.clone()
    }
}
impl CreationPlan<Model> for ReceiptCreatePlan {
    fn header(&self) -> &CreationPlanHeader<u32> {
        &self.header
    }
    fn actual_bounds(&self) -> Result<Option<Rectangle>, ContainerEditError> {
        Ok(Some(self.header.candidate().bounds()))
    }
    fn feedback(&self) -> Result<Vec<novadraw_editor::FeedbackVisual>, ContainerEditError> {
        Ok(vec![])
    }
    fn build_command(
        self,
        _: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        Ok(None)
    }
}
struct ReceiptTool {
    source: novadraw_editor::EditPartId,
    visuals: Rc<Cell<usize>>,
}
impl Tool<Model> for ReceiptTool {
    fn key(&self) -> novadraw_editor::ErasedToolKey {
        ToolKey::<Self>::new("external-receipt-tool").erase()
    }
    fn cancel(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        Ok(())
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let mut session = ctx.begin_snap_session()?;
        let batch = ctx.begin_container_move::<ReceiptMove>(
            &[self.source],
            None,
            Point::new(45.0, 25.0),
            event.revision(),
        )?;
        let candidate = ctx.container_move_candidate(
            &batch,
            ctx.contents(),
            event.location(),
            event.revision(),
        )?;
        let plan = ctx
            .resolve_snapped_container_move::<ReceiptMove>(&mut session, &candidate, settings())?
            .unwrap();
        let feedback = ctx.show_snapped_container_move_feedback(&mut session, &plan)?;
        self.visuals.set(self.visuals.get() + feedback.len());
        ctx.remove_overlay_visuals(&feedback)?;
        ctx.submit_snapped_container_move(&mut session, plan)?;
        let mut session = ctx.begin_snap_session()?;
        let plan = ctx
            .resolve_snapped_container_drop(
                &mut session,
                &ReceiptCreate,
                Point::new(43.0, 120.0),
                Point::ZERO,
                Default::default(),
                event.revision(),
                settings(),
            )?
            .unwrap();
        let feedback = ctx.show_snapped_container_creation_feedback(&mut session, &plan)?;
        self.visuals.set(self.visuals.get() + feedback.len());
        ctx.remove_overlay_visuals(&feedback)?;
        ctx.submit_snapped_container_creation(&mut session, plan)?;
        Ok((ToolDirective::Continue, ctx.dispatch_pressed(event, false)?))
    }
    fn mouse_moved(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
    fn mouse_released(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
}

/// Native/Wasm consumer for actual move/create receipts and the ordinary reference-line renderer.
/// Application plans return legal no-op commands; model mutation is covered by the container suite.
pub fn checked_plan_snap_probe() -> usize {
    let (mut viewer, _) = fixture();
    {
        let mut registry = viewer.snapping().unwrap();
        let capture = registry
            .register_rule(ThresholdCapture::key(), ThresholdCapture)
            .unwrap();
        let alignment = registry
            .register_rule(LineAlignment::key(), LineAlignment)
            .unwrap();
        registry
            .register_provider(
                GuideProvider::key(),
                GuideProvider,
                vec![GuideSpec::new(1, Point::new(44.0, 0.0), Point::new(44.0, 200.0)).unwrap()],
            )
            .unwrap();
        registry
            .register_strategy(
                Guide::key(),
                Guide::new(capture.clone(), alignment),
                (),
                &capture,
            )
            .unwrap();
        registry
            .register_renderer(
                ReferenceLineRenderer::key(),
                ReferenceLineRenderer::new(
                    novadraw::graphics::Color::BLACK,
                    novadraw::render::StrokeStyle::default(),
                    10.0,
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut domain = EditorDomain::new();
    let visuals = Rc::new(Cell::new(0));
    let source = viewer.part_for_model(1).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            ReceiptTool {
                source,
                visuals: visuals.clone(),
            },
        )
        .unwrap();
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(47.0, 25.0),
            novadraw::event::MouseButton::Left,
            Default::default(),
        )
        .unwrap();
    assert_eq!(domain.command_stack().undo_len(), 0);
    visuals.get()
}

struct ExternalResizeLimits;
impl EvaluationRule for ExternalResizeLimits {
    type Payload = ResizeLimitsPayload;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-resize-limits")
    }
    fn version(&self, _: &Self::Payload) -> Result<u64, SnapError> {
        Ok(0)
    }
    fn evaluate(
        &self,
        payload: &Self::Payload,
        ctx: &EvaluationContext<'_>,
    ) -> Result<bool, SnapError> {
        if !ResizeLimits.evaluate(payload, ctx)? {
            return Ok(false);
        }
        let points = ctx.geometry.points();
        let map = payload.surface_to_geometry();
        Ok(map.transform_point(points[1]).x() - map.transform_point(points[0]).x() <= 100.0)
    }
}
/// External Native/Wasm consumer for actual resize feedback, latest release and model history.
pub fn checked_resize_snap_probe() -> Rectangle {
    let (mut viewer, _) = fixture();
    viewer.model().resize.borrow_mut().command = true;
    install_resize(&mut viewer, Some(80.0));
    let limits = viewer
        .snapping()
        .unwrap()
        .register_rule(ExternalResizeLimits::key(), ExternalResizeLimits)
        .unwrap();
    let source = viewer.part_for_model(1).unwrap();
    let mut domain = EditorDomain::new();
    domain
        .set_active_tool(
            &mut viewer,
            ResizeTool::<Model, ReceiptResize, ExternalResizeLimits>::from_source(
                resize_settings(),
                limits,
                source,
                novadraw_editor::ResizeDirection::SouthEast,
                Point::new(70.0, 40.0),
            ),
        )
        .unwrap();
    domain
        .pointer_moved(&mut viewer, Point::new(71.0, 41.0))
        .unwrap();
    domain
        .pointer_released(
            &mut viewer,
            Point::new(74.0, 45.0),
            novadraw::event::MouseButton::Left,
        )
        .unwrap();
    let result = viewer.model().resize.borrow().actual;
    assert_eq!(result, Rectangle::new(40.0, 20.0, 40.0, 25.0));
    assert_eq!(viewer.model().resize.borrow().builds, 1);
    assert_eq!(viewer.model().resize.borrow().plans, 2);
    assert_eq!(domain.command_stack().undo_len(), 1);
    domain.undo(&mut viewer).unwrap();
    assert_eq!(viewer.model().resize.borrow().actual, bounds(1));
    domain.redo(&mut viewer).unwrap();
    assert_eq!(viewer.model().resize.borrow().actual, result);
    result
}
fn resize_settings() -> SnapSettings {
    SnapSettings::new(8.0, 1e-8, settings().budget()).unwrap()
}
fn install_resize(
    viewer: &mut GraphicalViewer<Model, Factory>,
    guide: Option<f64>,
) -> RuleBinding<ResizeLimits> {
    let mut registry = viewer.snapping().unwrap();
    let limits = registry
        .register_rule(ResizeLimits::key(), ResizeLimits)
        .unwrap();
    if let Some(x) = guide {
        let capture = registry
            .register_rule(ThresholdCapture::key(), ThresholdCapture)
            .unwrap();
        let alignment = registry
            .register_rule(LineAlignment::key(), LineAlignment)
            .unwrap();
        registry
            .register_provider(
                GuideProvider::key(),
                GuideProvider,
                vec![GuideSpec::new(1, Point::new(x, 0.0), Point::new(x, 200.0)).unwrap()],
            )
            .unwrap();
        registry
            .register_strategy(
                Guide::key(),
                Guide::new(capture.clone(), alignment),
                (),
                &capture,
            )
            .unwrap();
        registry
            .register_renderer(
                ReferenceLineRenderer::key(),
                ReferenceLineRenderer::new(
                    novadraw::graphics::Color::BLACK,
                    novadraw::render::StrokeStyle::default(),
                    10.0,
                )
                .unwrap(),
            )
            .unwrap();
    }
    limits
}

#[cfg(test)]
struct ResizeProbe {
    source: novadraw_editor::EditPartId,
    limits: RuleBinding<ResizeLimits>,
    direction: novadraw_editor::ResizeDirection,
    press: Point,
    pointer: Point,
    out: Rc<RefCell<Option<Result<Rectangle, String>>>>,
}
#[cfg(test)]
impl Tool<Model> for ResizeProbe {
    fn key(&self) -> novadraw_editor::ErasedToolKey {
        ToolKey::<Self>::new("external-resize-probe").erase()
    }
    fn activate(&mut self, ctx: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        let result = (|| {
            let start =
                ctx.begin_resize::<ReceiptResize>(self.source, self.direction, self.press)?;
            let candidate =
                ctx.resize_candidate(&start, self.pointer, InteractionRevision::initial())?;
            let mut session = ctx.begin_snap_session()?;
            let plan = ctx
                .resolve_snapped_resize::<ReceiptResize>(
                    &mut session,
                    &candidate,
                    resize_settings(),
                    &self.limits,
                )?
                .ok_or_else(|| ToolError::protocol("no resize plan"))?;
            let bounds = plan.actual_bounds();
            let feedback = ctx.show_snapped_resize_feedback(&mut session, &plan)?;
            ctx.remove_overlay_visuals(&feedback)?;
            ctx.submit_snapped_resize(&mut session, plan)?;
            Ok(bounds)
        })();
        *self.out.borrow_mut() = Some(result.map_err(|error: ToolError| error.to_string()));
        Ok(())
    }
    fn cancel(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        Ok(())
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        Ok((ToolDirective::Continue, ctx.dispatch_pressed(event, false)?))
    }
    fn mouse_moved(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
    fn mouse_released(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
}
struct Plan {
    header: MovePlanHeader<u32>,
}
impl MovePlan<Model> for Plan {
    fn header(&self) -> &MovePlanHeader<u32> {
        &self.header
    }
    fn feedback(&self) -> Result<Vec<novadraw_editor::FeedbackVisual>, ContainerEditError> {
        Ok(Vec::new())
    }
    fn build_command(
        self,
        _: &Model,
    ) -> Result<Option<Box<dyn Command<Model>>>, ContainerEditError> {
        panic!("a snap solve must not build commands")
    }
}
struct Move;
impl RequestProtocol<Model> for Move {
    type Payload = MoveCandidate<u32>;
    type Contribution = Plan;
    fn key() -> RequestKey<Self> {
        RequestKey::new("snap-probe-move")
    }
}
impl ContainerMoveProtocol<Model> for Move {
    type Payload = MoveCandidate<u32>;
    type Plan = Plan;
    type Request = Self;
    fn payload(candidate: &MoveCandidate<u32>) -> Self::Payload {
        candidate.clone()
    }
    fn source_geometry(
        _: &Model,
        source: &MoveSource<u32>,
    ) -> Result<ConstraintGeometry, ContainerEditError> {
        ConstraintGeometry::xy(bounds(source.model()))
    }
}
struct Capture {
    source: novadraw_editor::EditPartId,
    out: Rc<RefCell<Option<MoveCandidate<u32>>>>,
}
impl Tool<Model> for Capture {
    fn key(&self) -> novadraw_editor::ErasedToolKey {
        ToolKey::<Self>::new("snap-input-capture").erase()
    }
    fn cancel(&mut self, _: &mut ToolContext<'_, Model>) -> Result<(), ToolError> {
        Ok(())
    }
    fn mouse_pressed(
        &mut self,
        ctx: &mut ToolContext<'_, Model>,
        event: ToolMouseEvent,
    ) -> Result<(ToolDirective, ViewerInputOutcome), ToolError> {
        let target = ctx.contents();
        let revision = InteractionRevision::initial();
        let batch = ctx.begin_container_move::<Move>(
            &[self.source],
            None,
            Point::new(45.0, 25.0),
            revision,
        )?;
        *self.out.borrow_mut() =
            Some(ctx.container_move_candidate(&batch, target, event.location(), revision)?);
        Ok((ToolDirective::Continue, ctx.dispatch_pressed(event, false)?))
    }
    fn mouse_moved(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
    fn mouse_released(
        &mut self,
        _: &mut ToolContext<'_, Model>,
        _: ToolMouseEvent,
    ) -> Result<(ToolDirective, DispatchOutcome), ToolError> {
        Ok((ToolDirective::Continue, DispatchOutcome::default()))
    }
}
fn settings() -> SnapSettings {
    let n = NonZeroUsize::new(64).unwrap();
    SnapSettings::new(
        8.0,
        0.0,
        SnapBudget {
            sources: n,
            features: n,
            candidates: n,
            feedback: n,
        },
    )
    .unwrap()
}
fn fixture() -> (GraphicalViewer<Model, Factory>, MoveCandidate<u32>) {
    let mut viewer = GraphicalViewer::new(
        Model {
            version: Rc::new(Cell::new(ModelRevision::initial())),
            resize: Rc::new(RefCell::new(ResizeState::default())),
            events: Rc::new(RefCell::new(vec![])),
        },
        Factory,
        bounds(0),
    )
    .unwrap();
    let novadraw::runtime::FramePreparation::Ready(submission) = viewer.drive().prepare_submission(
        SurfaceInfo {
            logical_width: 800.0,
            logical_height: 600.0,
            pixel_width: 800,
            pixel_height: 600,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL.with_custom_strokes(),
    ) else {
        panic!("prepare")
    };
    let _ = viewer
        .drive()
        .complete_submission(
            submission.session_id,
            submission.frame_id,
            Ok(RenderDisposition::Presented),
        )
        .unwrap();
    let out = Rc::new(RefCell::new(None));
    let mut domain = EditorDomain::new();
    let source = viewer.part_for_model(1).unwrap();
    domain
        .set_active_tool(
            &mut viewer,
            Capture {
                source,
                out: out.clone(),
            },
        )
        .unwrap();
    domain
        .pointer_pressed(
            &mut viewer,
            Point::new(46.0, 25.0),
            novadraw::event::MouseButton::Left,
            Default::default(),
        )
        .unwrap();
    let candidate = out.borrow_mut().take().unwrap();
    (viewer, candidate)
}
struct Provider;
impl FeatureProvider for Provider {
    type Payload = Rc<Cell<u64>>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-sibling-provider")
    }
    fn version(&self, payload: &Self::Payload) -> Result<u64, SnapError> {
        Ok(payload.get())
    }
    fn features(
        &self,
        _: &Self::Payload,
        input: &SnapInput,
        sink: &mut FeatureSink<'_>,
    ) -> Result<(), SnapError> {
        for (stable, feature) in input.scene().iter().enumerate() {
            sink.push(stable as u64, feature.corners().to_vec())?;
        }
        Ok(())
    }
}
struct Align;
impl EvaluationRule for Align {
    type Payload = f64;
    type Error = Infallible;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-alignment")
    }
    fn version(&self, _: &f64) -> Result<u64, Infallible> {
        Ok(0)
    }
    fn evaluate(
        &self,
        expected: &f64,
        context: &EvaluationContext<'_>,
    ) -> Result<bool, Infallible> {
        Ok(context.value(DofKey::of::<SurfaceTranslationX>()) == Some(*expected))
    }
}
struct Strategy {
    admission: RuleBinding<ThresholdCapture>,
    align: RuleBinding<Align>,
}
impl SnapStrategy for Strategy {
    type Payload = Rc<Cell<u64>>;
    type Prepared = Rc<RefCell<Vec<FeatureId>>>;
    type Error = SnapError;
    fn key() -> ProtocolKey<Self> {
        ProtocolKey::new("external-nonthread-strategy")
    }
    fn version(&self, payload: &Self::Payload) -> Result<u64, SnapError> {
        Ok(payload.get())
    }
    fn prepare(
        &self,
        _: &Self::Payload,
        _: &SnapInput,
        features: &[SnapFeature],
    ) -> Result<Self::Prepared, SnapError> {
        Ok(Rc::new(RefCell::new(
            features.iter().map(|f| f.id()).collect(),
        )))
    }
    fn query(
        &self,
        _: &Self::Payload,
        prepared: &Self::Prepared,
        _: &SnapInput,
        sink: &mut CandidateSink<'_>,
    ) -> Result<(), SnapError> {
        let x = DofKey::of::<SurfaceTranslationX>();
        sink.push(SnapCandidate::new(
            prepared.borrow()[0],
            0,
            vec![(x, 2.0)],
            vec![x],
            self.admission.bind(()),
            self.align.bind(2.0),
            Vec::new(),
        ))
    }
}
fn install(viewer: &mut GraphicalViewer<Model, Factory>) {
    let mut registry = viewer.snapping().unwrap();
    let admission = registry
        .register_rule(ThresholdCapture::key(), ThresholdCapture)
        .unwrap();
    let align = registry.register_rule(Align::key(), Align).unwrap();
    registry
        .register_provider(Provider::key(), Provider, Rc::new(Cell::new(0)))
        .unwrap();
    registry
        .register_strategy(
            Strategy::key(),
            Strategy {
                admission: admission.clone(),
                align,
            },
            Rc::new(Cell::new(0)),
            &admission,
        )
        .unwrap();
}

/// Native/Wasm compilation probe using only published API and an actual E03-issued input.
pub fn open_snap_probe() -> usize {
    let (mut viewer, candidate) = fixture();
    install(&mut viewer);
    let mut session = viewer.begin_snap_session().unwrap();
    let input = viewer
        .snap_move_input(&candidate, settings(), Vec::new())
        .unwrap();
    let solution = viewer.solve_snapping(&mut session, input).unwrap();
    viewer
        .validate_snapping_geometry(
            &mut session,
            &solution,
            solution.geometry().values().to_vec(),
        )
        .unwrap();
    session.metrics().adopted
}

/// Compiles all ordinary built-in providers and strategies in Native and Wasm consumers.
pub fn builtin_snap_probe() -> usize {
    let (mut viewer, candidate) = fixture();
    let mut registration = viewer.snapping().unwrap();
    let capture = registration
        .register_rule(GridCapture::key(), GridCapture)
        .unwrap();
    let nearest = registration
        .register_rule(NearestGrid::key(), NearestGrid)
        .unwrap();
    registration
        .register_provider(
            GridProvider::key(),
            GridProvider,
            GridSpec::new(Point::ZERO, Some(10.0), None).unwrap(),
        )
        .unwrap();
    registration
        .register_strategy(
            Grid::<GridCapture>::key(),
            Grid::new(capture.clone(), nearest),
            (),
            &capture,
        )
        .unwrap();
    let threshold = registration
        .register_rule(ThresholdCapture::key(), ThresholdCapture)
        .unwrap();
    let alignment = registration
        .register_rule(LineAlignment::key(), LineAlignment)
        .unwrap();
    registration
        .register_provider(GeometryProvider::key(), GeometryProvider, ())
        .unwrap();
    registration
        .register_provider(
            GuideProvider::key(),
            GuideProvider,
            vec![GuideSpec::new(1, Point::new(44.0, 0.0), Point::new(44.0, 100.0)).unwrap()],
        )
        .unwrap();
    registration
        .register_strategy(
            Geometry::key(),
            Geometry::new(threshold.clone(), alignment.clone()),
            (),
            &threshold,
        )
        .unwrap();
    registration
        .register_strategy(
            Guide::key(),
            Guide::new(threshold.clone(), alignment),
            (),
            &threshold,
        )
        .unwrap();
    let mut session = viewer.begin_snap_session().unwrap();
    let input = viewer
        .snap_move_input(&candidate, settings(), Vec::new())
        .unwrap();
    let solution = viewer.solve_snapping(&mut session, input).unwrap();
    assert_eq!(solution.geometry().values(), &[-1.0, 0.0]);
    let input = viewer
        .snap_move_input(&candidate, settings(), Vec::new())
        .unwrap();
    let solution = viewer.solve_snapping(&mut session, input).unwrap();
    viewer
        .validate_snapping_geometry(
            &mut session,
            &solution,
            solution.geometry().values().to_vec(),
        )
        .unwrap();
    session.metrics().cache_hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        fmt,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    #[test]
    fn resize_actual_plan_and_latest_release_submit_once_and_undo_redo() {
        checked_resize_snap_probe();
    }
    fn resize_probe(
        viewer: &mut GraphicalViewer<Model, Factory>,
        direction: novadraw_editor::ResizeDirection,
        press: Point,
        pointer: Point,
        guide: Option<f64>,
    ) -> Result<Rectangle, String> {
        let limits = install_resize(viewer, guide);
        let source = viewer.part_for_model(1).unwrap();
        let out = Rc::new(RefCell::new(None));
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                viewer,
                ResizeProbe {
                    source,
                    limits,
                    direction,
                    press,
                    pointer,
                    out: out.clone(),
                },
            )
            .unwrap();
        out.borrow_mut().take().unwrap()
    }
    #[test]
    fn all_resize_directions_preserve_every_fixed_edge_in_actual_plan() {
        use novadraw_editor::ResizeDirection::*;
        for (direction, press, pointer, expected) in [
            (North, (55.0, 20.0), (58.0, 17.0), (40.0, 17.0, 30.0, 23.0)),
            (
                NorthEast,
                (70.0, 20.0),
                (73.0, 17.0),
                (40.0, 17.0, 33.0, 23.0),
            ),
            (East, (70.0, 30.0), (73.0, 33.0), (40.0, 20.0, 33.0, 20.0)),
            (
                SouthEast,
                (70.0, 40.0),
                (73.0, 43.0),
                (40.0, 20.0, 33.0, 23.0),
            ),
            (South, (55.0, 40.0), (58.0, 43.0), (40.0, 20.0, 30.0, 23.0)),
            (
                SouthWest,
                (40.0, 40.0),
                (37.0, 43.0),
                (37.0, 20.0, 33.0, 23.0),
            ),
            (West, (40.0, 30.0), (37.0, 33.0), (37.0, 20.0, 33.0, 20.0)),
            (
                NorthWest,
                (40.0, 20.0),
                (37.0, 17.0),
                (37.0, 17.0, 33.0, 23.0),
            ),
        ] {
            let (mut viewer, _) = fixture();
            let actual = resize_probe(
                &mut viewer,
                direction,
                Point::new(press.0, press.1),
                Point::new(pointer.0, pointer.1),
                None,
            )
            .unwrap();
            assert_eq!(
                actual,
                Rectangle::new(expected.0, expected.1, expected.2, expected.3),
                "{direction:?}"
            );
            assert_eq!(viewer.model().resize.borrow().builds, 1);
        }
    }
    #[test]
    fn proportional_resize_snaps_as_one_coupled_freedom() {
        let (mut viewer, _) = fixture();
        viewer.model().resize.borrow_mut().shape = ResizeShape::new(
            bounds(1),
            novadraw::geometry::Affine2D::IDENTITY,
            novadraw::geometry::Dimension::new(10.0, 10.0),
            Some(1.5),
        )
        .unwrap();
        let actual = resize_probe(
            &mut viewer,
            novadraw_editor::ResizeDirection::SouthEast,
            Point::new(70.0, 40.0),
            Point::new(74.0, 44.0),
            Some(80.0),
        )
        .unwrap();
        assert!((actual.width - 40.0).abs() < 1e-8);
        assert!((actual.height - 40.0 / 1.5).abs() < 1e-8);
        assert_eq!(actual.x, 40.0);
        assert_eq!(actual.y, 20.0);
    }
    #[test]
    fn resize_minimum_and_actual_plan_rewrite_reject_before_command_construction() {
        for rewrite in [false, true] {
            let (mut viewer, _) = fixture();
            if rewrite {
                viewer.model().resize.borrow_mut().rewrite =
                    novadraw::geometry::Vec2::new(1.0, 0.0);
            }
            let end = if rewrite {
                Point::new(74.0, 44.0)
            } else {
                Point::new(45.0, 25.0)
            };
            assert!(
                resize_probe(
                    &mut viewer,
                    novadraw_editor::ResizeDirection::SouthEast,
                    Point::new(70.0, 40.0),
                    end,
                    None
                )
                .is_err()
            );
            assert_eq!(viewer.model().resize.borrow().builds, 0);
            assert_eq!(viewer.model().resize.borrow().actual, bounds(1));
        }
    }
    #[test]
    fn rotated_resize_uses_geometry_domain_edges_and_real_surface_corners() {
        let (mut viewer, _) = fixture();
        let map = novadraw::geometry::Affine2D::new(0.0, 2.0, -3.0, 0.0, 300.0, 0.0);
        viewer.model().resize.borrow_mut().shape = ResizeShape::new(
            bounds(1),
            map,
            novadraw::geometry::Dimension::new(10.0, 10.0),
            None,
        )
        .unwrap();
        let actual = resize_probe(
            &mut viewer,
            novadraw_editor::ResizeDirection::NorthWest,
            map.transform_point(Point::new(40.0, 20.0)),
            map.transform_point(Point::new(37.0, 17.0)),
            None,
        )
        .unwrap();
        assert_eq!(actual, Rectangle::new(37.0, 17.0, 33.0, 23.0));
    }
    #[test]
    fn external_resize_limits_rule_and_typed_handle_use_same_tool_and_registry() {
        let (mut viewer, _) = fixture();
        let limits = viewer
            .snapping()
            .unwrap()
            .register_rule(ExternalResizeLimits::key(), ExternalResizeLimits)
            .unwrap();
        viewer.model().resize.borrow_mut().command = true;
        let source = viewer.part_for_model(1).unwrap();
        viewer
            .add_handle_visual_with_action::<novadraw_editor::overlay::ResizeAction>(
                source,
                novadraw_editor::ResizeDirection::SouthEast,
                Box::new(RectangleFigure::new(68.0, 38.0, 6.0, 6.0).unwrap()),
            )
            .unwrap();
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                &mut viewer,
                ResizeTool::<Model, ReceiptResize, ExternalResizeLimits>::new(
                    resize_settings(),
                    limits,
                ),
            )
            .unwrap();
        domain
            .pointer_pressed(
                &mut viewer,
                Point::new(70.0, 40.0),
                novadraw::event::MouseButton::Left,
                Default::default(),
            )
            .unwrap();
        domain
            .pointer_released(
                &mut viewer,
                Point::new(74.0, 45.0),
                novadraw::event::MouseButton::Left,
            )
            .unwrap();
        assert_eq!(
            viewer.model().resize.borrow().actual,
            Rectangle::new(40.0, 20.0, 34.0, 25.0)
        );
        assert_eq!(domain.command_stack().undo_len(), 1);
    }
    #[test]
    fn stationary_resize_zoom_rebuilds_geometry_before_latest_release() {
        let (mut viewer, _) = fixture();
        viewer.model().resize.borrow_mut().command = true;
        let limits = install_resize(&mut viewer, None);
        let source = viewer.part_for_model(1).unwrap();
        let parent = viewer.parts().parent(source).unwrap();
        let mut domain = EditorDomain::new();
        domain
            .set_active_tool(
                &mut viewer,
                ResizeTool::<Model, ReceiptResize>::from_source(
                    resize_settings(),
                    limits,
                    source,
                    novadraw_editor::ResizeDirection::SouthEast,
                    Point::new(70.0, 40.0),
                ),
            )
            .unwrap();
        let pointer = Point::new(74.0, 45.0);
        domain.pointer_moved(&mut viewer, pointer).unwrap();
        let previous = viewer.model().resize.borrow().plans;
        domain
            .set_viewport_scale_at(&mut viewer, 2.0, Some(Point::new(40.0, 20.0)))
            .unwrap();
        assert!(viewer.model().resize.borrow().plans > previous);
        let latest = viewer
            .child_content_to_surface(parent)
            .unwrap()
            .inverse()
            .unwrap()
            .transform_point(pointer);
        let previous = viewer.model().resize.borrow().plans;
        domain
            .pointer_released(&mut viewer, pointer, novadraw::event::MouseButton::Left)
            .unwrap();
        assert_eq!(viewer.model().resize.borrow().plans, previous + 1);
        assert_eq!(
            viewer.model().resize.borrow().actual,
            Rectangle::new(
                40.0,
                20.0,
                30.0 + latest.x() - 70.0,
                20.0 + latest.y() - 40.0
            )
        );
        assert_eq!(viewer.model().resize.borrow().builds, 1);
    }
    #[test]
    fn cancelled_or_invalidated_resize_cannot_reactivate_from_later_pointer() {
        for invalidated in [false, true] {
            let (mut viewer, _) = fixture();
            viewer.model().resize.borrow_mut().command = true;
            let limits = install_resize(&mut viewer, None);
            let source = viewer.part_for_model(1).unwrap();
            let mut domain = EditorDomain::new();
            domain
                .set_active_tool(
                    &mut viewer,
                    ResizeTool::<Model, ReceiptResize>::from_source(
                        resize_settings(),
                        limits,
                        source,
                        novadraw_editor::ResizeDirection::SouthEast,
                        Point::new(70.0, 40.0),
                    ),
                )
                .unwrap();
            domain
                .pointer_moved(&mut viewer, Point::new(74.0, 45.0))
                .unwrap();
            if invalidated {
                viewer
                    .model()
                    .version
                    .set(viewer.model().version.get().next().unwrap());
                viewer.model().events.borrow_mut().push(ModelEvent::new(
                    viewer.model().version.get(),
                    1,
                    (),
                ));
                domain
                    .pointer_moved(&mut viewer, Point::new(75.0, 46.0))
                    .unwrap();
            } else {
                domain.cancel_tool(&mut viewer).unwrap();
            }
            domain
                .pointer_released(
                    &mut viewer,
                    Point::new(76.0, 47.0),
                    novadraw::event::MouseButton::Left,
                )
                .unwrap();
            assert_eq!(viewer.model().resize.borrow().builds, 0);
            assert_eq!(domain.command_stack().undo_len(), 0);
        }
    }
    #[test]
    fn foreign_or_retired_resize_limits_reject_before_actual_plan_callbacks() {
        for retired in [false, true] {
            let (mut viewer, _) = fixture();
            let limits = if retired {
                let limits = install_resize(&mut viewer, None);
                viewer
                    .snapping()
                    .unwrap()
                    .replace_rule(&limits, ResizeLimits)
                    .unwrap();
                limits
            } else {
                let (mut foreign, _) = fixture();
                install_resize(&mut foreign, None)
            };
            let source = viewer.part_for_model(1).unwrap();
            let mut domain = EditorDomain::new();
            domain
                .set_active_tool(
                    &mut viewer,
                    ResizeTool::<Model, ReceiptResize>::from_source(
                        resize_settings(),
                        limits,
                        source,
                        novadraw_editor::ResizeDirection::SouthEast,
                        Point::new(70.0, 40.0),
                    ),
                )
                .unwrap();
            assert!(
                domain
                    .pointer_released(
                        &mut viewer,
                        Point::new(74.0, 45.0),
                        novadraw::event::MouseButton::Left
                    )
                    .is_err()
            );
            assert_eq!(viewer.model().resize.borrow().plans, 0);
            assert_eq!(viewer.model().resize.borrow().builds, 0);
        }
    }
    #[test]
    fn resize_actual_alignment_rewrite_or_release_callback_invalidation_never_retries() {
        for invalidate in [false, true] {
            let (mut viewer, _) = fixture();
            {
                let mut state = viewer.model().resize.borrow_mut();
                state.command = true;
                state.invalidate_at_plan = invalidate;
                state.rewrite_width = if invalidate { 0.0 } else { -1.0 };
            }
            let limits = install_resize(&mut viewer, Some(80.0));
            let source = viewer.part_for_model(1).unwrap();
            let mut domain = EditorDomain::new();
            domain
                .set_active_tool(
                    &mut viewer,
                    ResizeTool::<Model, ReceiptResize>::from_source(
                        resize_settings(),
                        limits,
                        source,
                        novadraw_editor::ResizeDirection::SouthEast,
                        Point::new(70.0, 40.0),
                    ),
                )
                .unwrap();
            let result = domain.pointer_released(
                &mut viewer,
                Point::new(74.0, 45.0),
                novadraw::event::MouseButton::Left,
            );
            if !invalidate {
                assert!(result.is_err());
            }
            assert_eq!(viewer.model().resize.borrow().plans, 1);
            assert_eq!(viewer.model().resize.borrow().builds, 0);
            assert_eq!(domain.command_stack().undo_len(), 0);
        }
    }
    #[test]
    fn actual_plan_receipts_and_reference_line_renderer_use_external_public_protocols() {
        assert_eq!(checked_plan_snap_probe(), 2);
    }
    #[test]
    fn external_payloads_prepared_and_rules_share_public_registry() {
        assert_eq!(open_snap_probe(), 1);
    }
    #[test]
    fn builtin_providers_and_strategies_share_public_registry_and_reuse_checked_assets() {
        assert_eq!(builtin_snap_probe(), 3);
    }
    #[test]
    fn checked_scene_preserves_empty_vs_foreign_failure_and_real_corners() {
        let (viewer, _) = fixture();
        let part = viewer.part_for_model(2).unwrap();
        let feature = viewer.snap_scene_feature(part).unwrap().unwrap();
        assert_eq!(feature.corners().len(), 4);
        assert_eq!(feature.corners()[2].x() - feature.corners()[0].x(), 30.0);
        let (foreign, _) = fixture();
        assert!(
            viewer
                .snap_scene_feature(foreign.part_for_model(2).unwrap())
                .is_err()
        );
    }
    #[test]
    fn viewport_change_invalidates_receipt_before_final_evaluation() {
        let (mut viewer, candidate) = fixture();
        install(&mut viewer);
        let mut session = viewer.begin_snap_session().unwrap();
        let input = viewer
            .snap_move_input(&candidate, settings(), Vec::new())
            .unwrap();
        let solution = viewer.solve_snapping(&mut session, input).unwrap();
        viewer.set_viewport_scale_at(2.0, None).unwrap();
        assert!(matches!(
            viewer.validate_snapping_geometry(&mut session, &solution, vec![2.0, 0.0]),
            Err(SnapError::Invalidated)
        ));
    }
    struct PanicRule;
    impl EvaluationRule for PanicRule {
        type Payload = ();
        type Error = Infallible;
        fn key() -> ProtocolKey<Self> {
            ProtocolKey::new("external-panic")
        }
        fn version(&self, _: &()) -> Result<u64, Infallible> {
            Ok(0)
        }
        fn evaluate(&self, _: &(), _: &EvaluationContext<'_>) -> Result<bool, Infallible> {
            panic!("external snap panic")
        }
    }
    #[test]
    fn evaluator_panic_faults_viewer_and_stops_registration() {
        let (mut viewer, candidate) = fixture();
        let binding = viewer
            .snapping()
            .unwrap()
            .register_rule(PanicRule::key(), PanicRule)
            .unwrap();
        let mut session = viewer.begin_snap_session().unwrap();
        let input = viewer
            .snap_move_input(&candidate, settings(), vec![binding.bind(())])
            .unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || viewer.solve_snapping(&mut session, input)
            ))
            .is_err()
        );
        assert!(viewer.is_faulted());
        assert_eq!(
            viewer.panic_diagnostic().unwrap().message(),
            Some("external snap panic")
        );
        assert!(matches!(viewer.snapping(), Err(SnapError::Faulted)));
    }
    #[derive(Debug)]
    struct Source(Rc<Cell<u64>>);
    impl fmt::Display for Source {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("external source")
        }
    }
    impl Error for Source {}
    struct Failure;
    impl EvaluationRule for Failure {
        type Payload = Rc<Cell<u64>>;
        type Error = Source;
        fn key() -> ProtocolKey<Self> {
            ProtocolKey::new("external-source")
        }
        fn version(&self, _: &Self::Payload) -> Result<u64, Source> {
            Ok(0)
        }
        fn evaluate(
            &self,
            payload: &Self::Payload,
            _: &EvaluationContext<'_>,
        ) -> Result<bool, Source> {
            Err(Source(payload.clone()))
        }
    }
    #[test]
    fn rule_error_retains_original_nonthread_source_without_faulting_viewer() {
        let (mut viewer, candidate) = fixture();
        let binding = viewer
            .snapping()
            .unwrap()
            .register_rule(Failure::key(), Failure)
            .unwrap();
        let input = viewer
            .snap_move_input(
                &candidate,
                settings(),
                vec![binding.bind(Rc::new(Cell::new(71)))],
            )
            .unwrap();
        let mut session = viewer.begin_snap_session().unwrap();
        let error = viewer.solve_snapping(&mut session, input).err().unwrap();
        let source = error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Source>()
            .unwrap();
        assert_eq!(source.0.get(), 71);
        assert!(!viewer.is_faulted());
    }
    #[test]
    fn frozen_model_revision_cannot_be_adopted_as_new_gesture_baseline() {
        let (mut viewer, candidate) = fixture();
        let input = viewer
            .snap_move_input(&candidate, settings(), Vec::new())
            .unwrap();
        viewer
            .model()
            .version
            .set(viewer.model().revision().next().unwrap());
        let mut session = viewer.begin_snap_session().unwrap();
        assert!(matches!(
            viewer.solve_snapping(&mut session, input),
            Err(SnapError::Invalidated)
        ));
    }

    struct CallbackFault {
        stage: &'static str,
        panic: bool,
        active: Cell<bool>,
    }
    impl CallbackFault {
        fn call(&self, stage: &'static str) -> Result<(), Source> {
            if self.stage == stage && self.active.get() {
                if self.panic {
                    std::panic::panic_any(format!("fault at {stage}"));
                }
                return Err(Source(Rc::new(Cell::new(93))));
            }
            Ok(())
        }
    }
    struct FaultProvider(Rc<CallbackFault>);
    impl FeatureProvider for FaultProvider {
        type Payload = ();
        type Error = Source;
        fn key() -> ProtocolKey<Self> {
            ProtocolKey::new("fault-provider")
        }
        fn version(&self, _: &()) -> Result<u64, Source> {
            self.0.call("snap.provider.version")?;
            Ok(0)
        }
        fn features(&self, _: &(), _: &SnapInput, _: &mut FeatureSink<'_>) -> Result<(), Source> {
            self.0.call("snap.provider.features")
        }
    }
    struct FaultStrategy(Rc<CallbackFault>);
    impl SnapStrategy for FaultStrategy {
        type Payload = ();
        type Prepared = ();
        type Error = Source;
        fn key() -> ProtocolKey<Self> {
            ProtocolKey::new("fault-strategy")
        }
        fn version(&self, _: &()) -> Result<u64, Source> {
            self.0.call("snap.strategy.version")?;
            Ok(0)
        }
        fn prepare(&self, _: &(), _: &SnapInput, _: &[SnapFeature]) -> Result<(), Source> {
            self.0.call("snap.strategy.prepare")
        }
        fn reuse_prepared(
            &self,
            _: &(),
            _: &(),
            _: &SnapInput,
            _: &[SnapFeature],
        ) -> Result<bool, Source> {
            self.0.call("snap.strategy.reuse-prepared")?;
            Ok(true)
        }
        fn query(
            &self,
            _: &(),
            _: &(),
            _: &SnapInput,
            _: &mut CandidateSink<'_>,
        ) -> Result<(), Source> {
            self.0.call("snap.strategy.query")
        }
    }
    struct FaultRule(Rc<CallbackFault>);
    impl EvaluationRule for FaultRule {
        type Payload = ();
        type Error = Source;
        fn key() -> ProtocolKey<Self> {
            ProtocolKey::new("fault-rule")
        }
        fn version(&self, _: &()) -> Result<u64, Source> {
            self.0.call("snap.rule.version")?;
            Ok(0)
        }
        fn evaluate(&self, _: &(), _: &EvaluationContext<'_>) -> Result<bool, Source> {
            self.0.call("snap.rule.evaluate")?;
            Ok(true)
        }
    }
    #[test]
    fn every_extension_boundary_retains_error_source_or_panic_location() {
        for stage in [
            "snap.provider.version",
            "snap.provider.features",
            "snap.strategy.version",
            "snap.strategy.prepare",
            "snap.strategy.query",
            "snap.strategy.reuse-prepared",
            "snap.rule.version",
            "snap.rule.evaluate",
        ] {
            for panic in [false, true] {
                let (mut viewer, candidate) = fixture();
                let fault = Rc::new(CallbackFault {
                    stage,
                    panic,
                    active: Cell::new(true),
                });
                let mut registry = viewer.snapping().unwrap();
                let admission = registry
                    .register_rule(ThresholdCapture::key(), ThresholdCapture)
                    .unwrap();
                let rule = registry
                    .register_rule(FaultRule::key(), FaultRule(fault.clone()))
                    .unwrap();
                registry
                    .register_provider(FaultProvider::key(), FaultProvider(fault.clone()), ())
                    .unwrap();
                registry
                    .register_strategy(FaultStrategy::key(), FaultStrategy(fault), (), &admission)
                    .unwrap();
                let input = viewer
                    .snap_move_input(&candidate, settings(), vec![rule.bind(())])
                    .unwrap();
                let mut session = viewer.begin_snap_session().unwrap();
                if stage == "snap.strategy.reuse-prepared" {
                    viewer.solve_snapping(&mut session, input).unwrap();
                }
                let input = viewer
                    .snap_move_input(&candidate, settings(), vec![rule.bind(())])
                    .unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    viewer.solve_snapping(&mut session, input)
                }));
                if panic {
                    assert!(result.is_err(), "{stage}");
                    assert!(viewer.is_faulted(), "{stage}");
                    let diagnostic = viewer.panic_diagnostic().unwrap();
                    assert_eq!(
                        diagnostic.message(),
                        Some(format!("fault at {stage}").as_str())
                    );
                    assert_eq!(session.metrics().callback_boundary, Some(stage));
                    assert!(matches!(viewer.snapping(), Err(SnapError::Faulted)));
                } else {
                    let error = result.unwrap().err().unwrap();
                    let SnapError::Extension(ref extension) = error else {
                        panic!("{error}")
                    };
                    assert_eq!(extension.boundary(), stage);
                    let source = error
                        .source()
                        .unwrap()
                        .source()
                        .unwrap()
                        .downcast_ref::<Source>()
                        .unwrap();
                    assert_eq!(source.0.get(), 93);
                    assert!(!viewer.is_faulted());
                    assert!(viewer.snapping().is_ok());
                }
            }
        }
    }
    #[test]
    fn final_rule_failures_preserve_source_and_panic_without_requery() {
        for stage in ["snap.rule.version", "snap.rule.evaluate"] {
            for panic in [false, true] {
                let (mut viewer, candidate) = fixture();
                let fault = Rc::new(CallbackFault {
                    stage,
                    panic,
                    active: Cell::new(false),
                });
                let rule = viewer
                    .snapping()
                    .unwrap()
                    .register_rule(FaultRule::key(), FaultRule(fault.clone()))
                    .unwrap();
                let input = viewer
                    .snap_move_input(&candidate, settings(), vec![rule.bind(())])
                    .unwrap();
                let mut session = viewer.begin_snap_session().unwrap();
                let solution = viewer.solve_snapping(&mut session, input).unwrap();
                fault.active.set(true);
                let queries = session.metrics().query_calls;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    viewer.validate_snapping_geometry(&mut session, &solution, vec![0.0, 0.0])
                }));
                assert_eq!(session.metrics().query_calls, queries);
                if panic {
                    assert!(result.is_err());
                    assert!(viewer.is_faulted());
                    assert_eq!(session.metrics().callback_boundary, Some(stage));
                } else {
                    let error = result.unwrap().err().unwrap();
                    assert!(error.source().unwrap().source().unwrap().is::<Source>());
                    assert!(!viewer.is_faulted());
                }
            }
        }
    }
}
