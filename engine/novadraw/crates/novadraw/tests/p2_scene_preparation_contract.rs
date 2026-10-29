//! Independent consumer of frozen preparation and atomic layout publication.
use novadraw::{
    event::{EventContext, FigureEvent, FigureListener, ListenerDirective, MonotonicTime},
    figure::{
        ACTIVITY, ActivityCapability, CONTAINER, ContainerCapability, Figure, FigureActivity,
        FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigureContainer, FigureDrawing,
        FigureMeasurement, FigurePreparation, FigurePresentation, FigureStyle, PREPARATION,
        PreparationCapability, PreparationScene, PreparedScene,
    },
    geometry::{Dimension, Rectangle},
    graphics::{Color, GraphicsError, PaintContext},
    layout::LayoutOutput,
    render::{BackendCapabilities, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
    text::{MeasureContext, TextError},
    tree::FigureTree,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Empty;
struct ExternalDecoration(Arc<Mutex<novadraw::connection::PreparedDecorationGeometry>>);
impl Figure for ExternalDecoration {
    fn initial_bounds(&self) -> Rectangle {
        self.0.lock().unwrap().bounds()
    }
    fn name(&self) -> &'static str {
        "ExternalDecoration"
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(
            novadraw::figure::CONNECTION_DECORATION,
            novadraw::figure::ConnectionDecorationCapability::of::<Self>(),
        )
    }
}
impl novadraw::connection::ConnectionDecorationBehavior for ExternalDecoration {
    fn decoration_geometry(&self) -> novadraw::connection::PreparedDecorationGeometry {
        self.0.lock().unwrap().clone()
    }
    fn prepare_decoration_geometry(
        &self,
        placement: novadraw::connection::LocatorPlacement,
    ) -> Result<
        novadraw::connection::PreparedDecorationGeometry,
        novadraw::connection::DecorationError,
    > {
        novadraw::connection::PreparedDecorationGeometry::new(
            Rectangle::new(placement.point.x(), placement.point.y(), 4.0, 4.0),
            vec![
                novadraw::geometry::Point::ZERO,
                novadraw::geometry::Point::new(4.0, 4.0),
            ],
        )
    }
    fn commit_decoration_geometry(
        &mut self,
        geometry: novadraw::connection::PreparedDecorationGeometry,
    ) {
        *self.0.lock().unwrap() = geometry;
    }
}

#[test]
fn rejected_feedback_restores_routes_and_external_decoration_geometry() {
    use novadraw::connection::{ChopboxAnchor, ConnectionFigure, ConnectionLocator, RouterBinding};
    let (mut runtime, page, children, mode) = fixture();
    let connection_figure = runtime
        .figure(page)
        .unwrap()
        .add(Box::new(ConnectionFigure::new()))
        .unwrap();
    let geometry = Arc::new(Mutex::new(
        novadraw::connection::PreparedDecorationGeometry::new(Rectangle::ZERO, vec![]).unwrap(),
    ));
    let decoration = runtime
        .figure(connection_figure)
        .unwrap()
        .add(Box::new(ExternalDecoration(geometry.clone())))
        .unwrap();
    let source = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(children[0])));
    let target = runtime
        .connections()
        .register_anchor(Box::new(ChopboxAnchor::new(children[1])));
    let connection = runtime
        .connections()
        .register_state(
            connection_figure,
            Some(source),
            Some(target),
            RouterBinding::Inherited { layer: page },
            None,
        )
        .unwrap();
    runtime
        .connections()
        .set_locator(connection, decoration, Box::new(ConnectionLocator::Target))
        .unwrap();
    assert!(frame(&mut runtime));
    let before_route = runtime
        .tree()
        .connection_route_points(connection_figure)
        .unwrap()
        .unwrap()
        .clone();
    let before_bounds = runtime.tree().figure_bounds(connection_figure).unwrap();
    let before_decoration = geometry.lock().unwrap().clone();
    let before_count = runtime.connections().routing_stats().route_calculations;
    mode.store(5, Ordering::Relaxed);
    assert!(!frame(&mut runtime));
    assert!(runtime.connections().routing_stats().route_calculations > before_count);
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(connection_figure)
            .unwrap(),
        Some(&before_route)
    );
    assert_eq!(
        runtime.tree().figure_bounds(connection_figure).unwrap(),
        before_bounds
    );
    assert_eq!(*geometry.lock().unwrap(), before_decoration);
    assert_eq!(
        runtime.tree().figure_bounds(decoration).unwrap(),
        before_decoration.bounds()
    );
    mode.store(0, Ordering::Relaxed);
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime
            .tree()
            .connection_route_points(connection_figure)
            .unwrap(),
        Some(&before_route)
    );
}

impl FigureDrawing for Empty {
    fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        Ok(())
    }
}

fn presentation(width: f64, height: f64) -> FigurePresentation {
    FigurePresentation::new(
        FigureMeasurement::new(width, height, None),
        Dimension::new(0.0, 0.0),
        Rectangle::new(0.0, 0.0, width, height),
        Arc::new(Empty),
    )
    .unwrap()
}

struct ExternalPage {
    mode: Arc<AtomicU64>,
}
impl Figure for ExternalPage {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 120.0, 80.0)
    }
    fn name(&self) -> &'static str {
        "ExternalPage"
    }
    fn register_capabilities(
        &self,
        output: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        output.register(CONTAINER, ContainerCapability::of::<Self>())?;
        output.register(PREPARATION, PreparationCapability::of::<Self>())
    }
}
impl FigureContainer for ExternalPage {}
impl FigurePreparation for ExternalPage {
    fn measure_scene(
        &self,
        _: &mut MeasureContext<'_>,
        _: novadraw::figure::MeasureConstraints,
        _: &PreparationScene<'_>,
    ) -> Result<novadraw::figure::PreparationMeasurement, novadraw::figure::PreparationMeasureError>
    {
        let width = if self.revision() == 8 { 200.0 } else { 120.0 };
        Ok(novadraw::figure::PreparationMeasurement {
            preferred: FigureMeasurement::new(width, 80.0, None),
            minimum: Dimension::ZERO,
        })
    }

    fn revision(&self) -> u64 {
        self.mode.load(Ordering::Relaxed)
    }
    fn prepare(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        Ok(presentation(120.0, 80.0))
    }
    fn prepare_scene(
        &self,
        _: &mut MeasureContext<'_>,
        _: Rectangle,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparedScene, TextError> {
        let mode = self.revision();
        let children = scene.children(scene.root())?;
        let color = scene.style(children[0])?.foreground;
        let root = presentation(if mode == 8 { 200.0 } else { 120.0 }, 80.0)
            .with_data(Arc::new((mode, color)));
        let mut candidate = PreparedScene::new(root);
        let mut output = LayoutOutput::new();
        if mode == 6 && scene.client_area(children[0])?.width == 70.0 {
            return Err(TextError::InvalidPreparedScene(
                "late measurement failed".into(),
            ));
        }
        let width = if matches!(mode, 5 | 6 | 8) && scene.client_area(children[0])?.width == 60.0 {
            70.0
        } else {
            60.0
        };
        output.set_child_bounds(
            children[0],
            Rectangle::new(if mode == 0 { 0.0 } else { 10.0 }, 0.0, width, 30.0),
        );
        output.set_child_bounds(
            children[1],
            Rectangle::new(80.0, 0.0, if mode == 1 { f64::NAN } else { 20.0 }, 30.0),
        );
        if mode == 4 {
            output.set_child_bounds(scene.root(), Rectangle::new(0.0, 0.0, 1.0, 1.0));
        }
        candidate.layout(scene.root(), output);
        candidate.present(
            children[0],
            presentation(width, 30.0).with_hit_regions(vec![
                Rectangle::new(0.0, 0.0, 20.0, 10.0),
                Rectangle::new(40.0, 20.0, 20.0, 10.0),
            ])?,
        );
        if mode == 2 {
            return Err(TextError::InvalidPreparedScene(
                "external measurement failed".into(),
            ));
        }
        if mode == 3 {
            self.mode.store(4, Ordering::Relaxed);
        }
        Ok(candidate)
    }
}

struct SourceActivity {
    watched: novadraw::tree::FigureId,
    fired: AtomicBool,
}
struct Moves(Arc<Mutex<Vec<FigureEvent>>>);
impl FigureListener for Moves {
    fn figure_moved(&self, event: FigureEvent) -> ListenerDirective {
        self.0.lock().unwrap().push(event);
        ListenerDirective::Keep
    }
}
impl Figure for SourceActivity {
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(0.0, 0.0, 10.0, 10.0)
    }
    fn name(&self) -> &'static str {
        "SourceActivity"
    }
    fn register_capabilities(
        &self,
        output: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        output.register(ACTIVITY, ActivityCapability::of::<Self>())
    }
}
impl FigureActivity for SourceActivity {
    fn poll(&self, _: MonotonicTime, _: bool, context: &mut EventContext<'_>) {
        if context.scene().figure_bounds(self.watched).unwrap().width == 70.0
            && !self.fired.swap(true, Ordering::Relaxed)
        {
            let id = context.target_id();
            context
                .effects()
                .set_bounds_later(id, Rectangle::new(15.0, 25.0, 35.0, 45.0));
            context.effects().set_style_later(
                id,
                FigureStyle {
                    foreground: Some(Color::BLUE),
                    ..FigureStyle::default()
                },
            );
        }
    }
}

#[test]
fn activity_source_write_survives_later_preparation_failure() {
    let (mut runtime, page, children, mode) = fixture();
    let source = runtime
        .figure(page)
        .unwrap()
        .add(Box::new(SourceActivity {
            watched: children[0],
            fired: AtomicBool::new(false),
        }))
        .unwrap();
    assert!(frame(&mut runtime));
    let moves = Arc::new(Mutex::new(Vec::new()));
    runtime
        .observations()
        .add_figure_listener(Box::new(Moves(moves.clone())));
    let before = runtime.tree().figure_bounds(children[0]).unwrap();
    mode.store(6, Ordering::Relaxed);
    assert!(!frame(&mut runtime));
    assert_eq!(runtime.tree().figure_bounds(children[0]).unwrap(), before);
    assert_eq!(
        runtime.tree().figure_bounds(source).unwrap(),
        Rectangle::new(15.0, 25.0, 35.0, 45.0)
    );
    assert_eq!(
        runtime.tree().node(source).unwrap().style().foreground,
        Some(Color::BLUE)
    );
    assert_eq!(
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<(u64, Color)>()
            .unwrap()
            .0,
        0
    );
    mode.store(0, Ordering::Relaxed);
    assert!(frame(&mut runtime));
    let moves = moves.lock().unwrap();
    assert!(moves.iter().any(|event| matches!(event,
        FigureEvent::FigureMoved { figure_id, .. } if *figure_id == source)));
    assert!(!moves.iter().any(|event| matches!(event,
        FigureEvent::FigureMoved { figure_id, new_bounds, .. }
            if *figure_id == children[0] && new_bounds.width == 70.0)));
}

#[test]
fn rejected_feedback_restores_viewport_ranges_and_content_geometry() {
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            novadraw::figure::RectangleFigure::new(0.0, 0.0, 120.0, 80.0)
                .expect("valid figure bounds"),
        ))
        .unwrap();
    let viewport = tree
        .builder()
        .add_viewport_to(root, Rectangle::new(0.0, 0.0, 100.0, 50.0))
        .unwrap();
    let mode = Arc::new(AtomicU64::new(0));
    let page = tree
        .builder()
        .add_child(
            viewport.figure_id(),
            Box::new(ExternalPage { mode: mode.clone() }),
        )
        .unwrap();
    for x in [0.0, 80.0] {
        tree.builder()
            .add_child_with_bounds(page, Rectangle::new(x, 0.0, 10.0, 10.0), Color::RED)
            .unwrap();
    }
    let mut runtime = Runtime::new(tree);
    assert!(frame(&mut runtime));
    let horizontal = viewport.horizontal_range();
    let vertical = viewport.vertical_range();
    let bounds = runtime.tree().figure_bounds(page).unwrap();
    mode.store(8, Ordering::Relaxed);
    assert!(!frame(&mut runtime));
    assert_eq!(viewport.horizontal_range(), horizontal);
    assert_eq!(viewport.vertical_range(), vertical);
    assert_eq!(runtime.tree().figure_bounds(page).unwrap(), bounds);
    mode.store(0, Ordering::Relaxed);
    assert!(frame(&mut runtime));
}

#[test]
fn nonconverging_feedback_keeps_published_prefix_and_committed_source_change() {
    let (mut runtime, page, children, mode) = fixture();
    assert!(frame(&mut runtime));
    let before = runtime.tree().node(children[0]).unwrap().bounds();
    runtime
        .figure(page)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::BLUE),
            ..FigureStyle::default()
        })
        .unwrap();
    mode.store(5, Ordering::Relaxed);
    assert!(!frame(&mut runtime));
    assert_eq!(runtime.tree().node(children[0]).unwrap().bounds(), before);
    assert_eq!(
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<(u64, Color)>()
            .unwrap()
            .0,
        0,
    );
    mode.store(0, Ordering::Relaxed);
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<(u64, Color)>()
            .unwrap()
            .1,
        Color::BLUE,
    );
}

fn fixture() -> (
    Runtime,
    novadraw::tree::FigureId,
    [novadraw::tree::FigureId; 2],
    Arc<AtomicU64>,
) {
    let mode = Arc::new(AtomicU64::new(0));
    let mut tree = FigureTree::new();
    let page = tree
        .builder()
        .set_contents(Box::new(ExternalPage { mode: mode.clone() }))
        .unwrap();
    let a = tree
        .builder()
        .add_child_with_bounds(page, Rectangle::new(1.0, 2.0, 10.0, 10.0), Color::RED)
        .unwrap();
    let b = tree
        .builder()
        .add_child_with_bounds(page, Rectangle::new(3.0, 4.0, 10.0, 10.0), Color::RED)
        .unwrap();
    (Runtime::new(tree), page, [a, b], mode)
}

fn frame(runtime: &mut Runtime) -> bool {
    runtime.frames().request_full_redraw();
    let result = runtime.frames().prepare_submission(
        SurfaceInfo {
            logical_width: 120.0,
            logical_height: 80.0,
            pixel_width: 120,
            pixel_height: 80,
            scale_factor: 1.0,
        },
        BackendCapabilities::RETAINED_PARTIAL,
    );
    if let Ok(frame) = result.into_ready() {
        let _ = runtime
            .frames()
            .complete_submission(
                frame.session_id,
                frame.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        true
    } else {
        false
    }
}

#[test]
fn failure_keeps_all_old_bounds_and_snapshot_data() {
    for failure in [1, 2, 3, 4] {
        let (mut runtime, page, children, mode) = fixture();
        assert!(frame(&mut runtime));
        let before = runtime.tree().node(children[0]).unwrap().bounds();
        let before_b = runtime.tree().node(children[1]).unwrap().bounds();
        mode.store(failure, Ordering::Relaxed);
        assert!(!frame(&mut runtime));
        assert_eq!(runtime.tree().node(children[0]).unwrap().bounds(), before);
        assert_eq!(runtime.tree().node(children[1]).unwrap().bounds(), before_b);
        assert_eq!(
            runtime
                .tree()
                .prepared_presentation(page)
                .unwrap()
                .unwrap()
                .data::<(u64, Color)>()
                .unwrap()
                .0,
            0
        );
    }
}

#[test]
fn fragment_blank_does_not_hit_but_normal_descendants_remain_reachable() {
    let (mut runtime, page, children, _) = fixture();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime.tree().hit_test_simple((5.0, 5.0)),
        Some(children[0])
    );
    assert_eq!(runtime.tree().hit_test_simple((30.0, 15.0)), Some(page));
    assert_eq!(
        runtime.tree().hit_test_simple((50.0, 25.0)),
        Some(children[0])
    );
    let grandchild = runtime
        .figure(children[0])
        .unwrap()
        .add(Box::new(
            novadraw::figure::RectangleFigure::new_with_color(25.0, 12.0, 10.0, 8.0, Color::RED)
                .expect("valid figure bounds"),
        ))
        .unwrap();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime.tree().hit_test_simple((30.0, 15.0)),
        Some(grandchild)
    );
}

#[test]
fn inherited_paint_only_style_invalidates_prepared_subtree() {
    let (mut runtime, page, _, _) = fixture();
    assert!(frame(&mut runtime));
    runtime
        .figure(page)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::RED),
            ..FigureStyle::default()
        })
        .unwrap();
    assert!(frame(&mut runtime));
    assert_eq!(
        runtime
            .tree()
            .prepared_presentation(page)
            .unwrap()
            .unwrap()
            .data::<(u64, Color)>()
            .unwrap()
            .1,
        Color::RED
    );
}
