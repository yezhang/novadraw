//! Literal official graph fixtures, projected by Core rather than positioned by the example.
use std::sync::Arc;

use novadraw::{
    connection::{
        AnchorId, ConnectionFigure, ConnectionLocator, PolygonDecorationFigure, RouteEnd,
        RouterBinding, RouterId,
    },
    figure::{
        FigureStyle, GraphProjectionFigure, LabelFigure, PreparationBudget, RectangleFigure,
        SetGraphProjection, border::LineBorder,
    },
    geometry::Rectangle,
    graphics::Color,
    layout::graph::{
        GraphBudget, GraphDirection, GraphEndpoint, GraphLayoutRequest, GraphLayoutService,
        GraphProjection, GraphProjectionBinding, GraphProjectionRouter, GraphSolveContext,
        register_compound_graph, register_directed_graph, register_layered_protocol,
    },
    runtime::Runtime,
    tree::FigureId,
};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

#[path = "../../../../scenes/src/compound_fixtures.rs"]
mod compound;
#[path = "../../../../scenes/src/graph_fixtures.rs"]
mod directed;

const WIDTH: f64 = 1200.0;
const HEIGHT: f64 = 800.0;
const CANVAS_X: f64 = 250.0;
const CANVAS_Y: f64 = 20.0;
const BUDGET: GraphBudget = GraphBudget::new(200_000_000, 20_000_000);
const FONT: &str = "12px Inter Variable";
const NODE_BACKGROUND: Color = Color::rgba(192.0 / 255.0, 192.0 / 255.0, 192.0 / 255.0, 1.0);
const EDGE_COLOR: Color = Color::rgba(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0);
const GROUP_COLOR: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);
const NODE_BORDER: f64 = 1.0;
const ROOT_BORDER: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Settings {
    compound: bool,
    fixture: usize,
    direction: GraphDirection,
}
impl Settings {
    fn names(self) -> &'static [&'static str] {
        if self.compound {
            compound::COMPOUND_FIXTURES
        } else {
            directed::DIRECTED_FIXTURES
        }
    }
}

#[derive(Clone)]
struct Registrations {
    router: RouterId,
    anchors: Vec<AnchorId>,
}
impl Registrations {
    fn retire(self, runtime: &mut Runtime) {
        runtime
            .connections()
            .remove_router(self.router)
            .expect("retired graph router");
        for anchor in self.anchors {
            runtime
                .connections()
                .remove_anchor(anchor)
                .expect("retired graph anchor");
        }
    }
}

pub fn directed_graph() -> Runtime {
    panel_scene(false).0
}
pub fn compound_graph() -> Runtime {
    panel_scene(true).0
}
pub fn verification_entries() -> Vec<crate::SceneEntry> {
    [
        ("DirectedOffsetSouth", false, 0, GraphDirection::South),
        ("DirectedGraph2East", false, 7, GraphDirection::East),
        ("CompoundChainsSouth", true, 2, GraphDirection::South),
        ("CompoundNestedEast", true, 9, GraphDirection::East),
    ]
    .into_iter()
    .map(|(name, compound, fixture, direction)| {
        (
            name,
            Box::new(move || {
                let mut runtime = super::runtime_with_fonts();
                install(
                    &mut runtime,
                    Settings {
                        compound,
                        fixture,
                        direction,
                    },
                    &mut None,
                );
                runtime
            }) as Box<dyn FnMut() -> Runtime>,
        )
    })
    .collect()
}
pub fn panel_scene(is_compound: bool) -> PanelScene {
    let mut settings = Settings {
        compound: is_compound,
        fixture: 0,
        direction: GraphDirection::South,
    };
    let mut runtime = super::runtime_with_fonts();
    let mut registrations = None;
    let mut owner = install(&mut runtime, settings, &mut registrations);
    (
        runtime,
        Some(Panel::new([20.0, 20.0], move |ui, runtime| {
            let before = settings;
            let response = egui::ComboBox::from_label("Graph")
                .selected_text(settings.names()[settings.fixture])
                .show_ui(ui, |ui| {
                    for (index, name) in settings.names().iter().enumerate() {
                        let item = ui.selectable_value(&mut settings.fixture, index, *name);
                        #[cfg(test)]
                        ui.ctx().data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new(format!("graph-fixture-{index}")),
                                item.rect,
                            )
                        });
                        let _ = item;
                    }
                });
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(egui::Id::new("graph-fixture-menu"), response.response.rect)
            });
            let _ = response;
            let response = egui::ComboBox::from_label("Graph Direction")
                .selected_text(if settings.direction == GraphDirection::South {
                    "SOUTH"
                } else {
                    "EAST"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.direction, GraphDirection::South, "SOUTH");
                    let item =
                        ui.selectable_value(&mut settings.direction, GraphDirection::East, "EAST");
                    #[cfg(test)]
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(egui::Id::new("graph-east"), item.rect));
                    let _ = item;
                });
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("graph-direction-menu"),
                    response.response.rect,
                )
            });
            let _ = response;
            if settings != before {
                owner = install(runtime, settings, &mut registrations);
            }
            let viewport = runtime.tree().parent_id(owner).unwrap().unwrap();
            let handle = runtime.viewports().handle(viewport).unwrap();
            let mut location = handle.view_location();
            let mut x = location.x();
            let mut y = location.y();
            let extent = runtime
                .tree()
                .prepared_presentation(owner)
                .unwrap()
                .map(|presentation| presentation.minimum_size());
            if let Some(extent) = extent {
                let bounds = runtime.tree().figure_bounds(viewport).unwrap();
                let horizontal = (extent.width - bounds.width).max(0.0);
                let vertical = (extent.height - bounds.height).max(0.0);
                if horizontal > 0.0 {
                    ui.add(egui::Slider::new(&mut x, 0.0..=horizontal).text("Horizontal"));
                }
                if vertical > 0.0 {
                    ui.add(egui::Slider::new(&mut y, 0.0..=vertical).text("Vertical"));
                }
                location = novadraw::geometry::Point::new(x, y);
                if location != handle.view_location() {
                    runtime
                        .viewports()
                        .set_view_location(viewport, x, y)
                        .unwrap();
                }
            }
        })),
    )
}

fn install(
    runtime: &mut Runtime,
    settings: Settings,
    registrations: &mut Option<Registrations>,
) -> FigureId {
    let mut context = GraphSolveContext::new(BUDGET);
    let (request, labels) = if settings.compound {
        let fixture = compound::compound_fixture(
            settings.names()[settings.fixture],
            settings.direction,
            &mut context,
        )
        .expect("official compound input");
        (
            fixture.request,
            fixture
                .labels
                .into_iter()
                .map(|(entity, label)| (entity, label.to_owned()))
                .collect::<Vec<_>>(),
        )
    } else {
        let fixture = directed::directed_fixture(
            settings.names()[settings.fixture],
            settings.direction,
            &mut context,
        )
        .expect("official directed input");
        let labels = fixture
            .request
            .nodes()
            .iter()
            .zip(fixture.labels)
            .map(|(node, label)| (GraphEndpoint::Node(node.id()), label))
            .collect();
        (fixture.request, labels)
    };
    runtime
        .frames()
        .set_preparation_budget(PreparationBudget::new(BUDGET.work, BUDGET.allocation));
    let root = runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::WHITE)
                .expect("valid figure bounds"),
        ))
        .expect("graph Canvas");
    if let Some(previous) = registrations.take() {
        previous.retire(runtime);
    }
    let viewport = runtime
        .viewports()
        .add(
            root,
            Rectangle::new(CANVAS_X, CANVAS_Y, WIDTH - CANVAS_X, HEIGHT - CANVAS_Y),
        )
        .unwrap();
    let viewport = viewport.figure_id();
    runtime
        .viewports()
        .set_tracks_width(viewport, true)
        .unwrap();
    runtime
        .viewports()
        .set_tracks_height(viewport, true)
        .unwrap();
    let owner = runtime
        .viewports()
        .set_contents(
            viewport,
            Box::new(
                GraphProjectionFigure::new(Rectangle::ZERO).expect("valid detached figure input"),
            ),
        )
        .expect("graph Panel");
    *registrations = Some(populate(runtime, owner, request, labels, &mut context));
    owner
}

fn populate(
    runtime: &mut Runtime,
    owner: FigureId,
    request: GraphLayoutRequest,
    labels: Vec<(GraphEndpoint, String)>,
    context: &mut GraphSolveContext,
) -> Registrations {
    let mut entities = Vec::new();
    // Subgraphs are transparent, ordered before nodes, and remain siblings of their members.
    for group in request.groups() {
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(
                RectangleFigure::new_with_color(0.0, 0.0, 0.0, 0.0, Color::TRANSPARENT)
                    .expect("valid figure bounds")
                    .with_border(
                        LineBorder::new(GROUP_COLOR, group.border_insets.left)
                            .expect("valid group border"),
                    )
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        entities.push((GraphEndpoint::Group(group.id()), figure));
    }
    for node in request.nodes() {
        let entity = GraphEndpoint::Node(node.id());
        let label = &labels.iter().find(|entry| entry.0 == entity).unwrap().1;
        let incoming = request.edges().iter().any(|edge| edge.target == entity);
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(
                LabelFigure::new(label)
                    .with_border(
                        LineBorder::new(
                            Color::BLACK,
                            if incoming { NODE_BORDER } else { ROOT_BORDER },
                        )
                        .expect("valid node border"),
                    )
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        runtime
            .figure(figure)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(Color::BLACK),
                background: Some(NODE_BACKGROUND),
                font: Some(novadraw::text::FontDescriptor::parse(FONT).unwrap()),
                ..FigureStyle::default()
            })
            .unwrap();
        runtime.figure(figure).unwrap().set_opaque(true).unwrap();
        entities.push((entity, figure));
    }
    let initial_router = runtime.connections().direct_router();
    let mut edges = Vec::new();
    for edge in request.edges() {
        let figure = runtime
            .figure(owner)
            .unwrap()
            .add(Box::new(
                ConnectionFigure::new()
                    .with_stroke(EDGE_COLOR, NODE_BORDER)
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        runtime
            .figure(figure)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(EDGE_COLOR),
                ..FigureStyle::default()
            })
            .unwrap();
        let connection = runtime
            .connections()
            .register_state(
                figure,
                None,
                None,
                RouterBinding::Explicit {
                    router: initial_router,
                },
                None,
            )
            .unwrap();
        let decoration = runtime
            .figure(figure)
            .unwrap()
            .add(Box::new(
                PolygonDecorationFigure::triangle()
                    .with_fill_color(EDGE_COLOR)
                    .with_stroke(EDGE_COLOR, NODE_BORDER)
                    .expect("valid detached figure input"),
            ))
            .unwrap();
        runtime
            .connections()
            .set_locator(connection, decoration, Box::new(ConnectionLocator::Target))
            .unwrap();
        edges.push((edge.id(), connection));
    }
    let binding =
        Arc::new(GraphProjectionBinding::new(owner, &request, entities, edges, context).unwrap());
    let router = runtime
        .connections()
        .register_router(Box::new(GraphProjectionRouter::new(binding.clone())));
    let mut anchors = Vec::new();
    for &(edge, connection) in binding.edges() {
        let source = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Source).unwrap(),
        ));
        let target = runtime.connections().register_anchor(Box::new(
            binding.anchor(&request, edge, RouteEnd::Target).unwrap(),
        ));
        anchors.extend([source, target]);
        runtime
            .connections()
            .set_source(connection, Some(source))
            .unwrap();
        runtime
            .connections()
            .set_target(connection, Some(target))
            .unwrap();
        runtime
            .connections()
            .set_router_binding(connection, RouterBinding::Explicit { router })
            .unwrap();
    }
    let mut service = GraphLayoutService::new();
    register_layered_protocol(&mut service).unwrap();
    register_directed_graph(&mut service).unwrap();
    register_compound_graph(&mut service).unwrap();
    runtime
        .figure(owner)
        .unwrap()
        .update_component(SetGraphProjection(
            GraphProjection::new(request, Arc::new(service), binding, vec![], BUDGET).unwrap(),
        ))
        .unwrap();
    Registrations { router, anchors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::layout::graph::GraphProjectionSnapshot;

    #[test]
    fn native_graph_submissions_include_every_glyph_font_resource() {
        use novadraw::render::{BackendCapabilities, RenderCommandKind, ResourceSync, SurfaceInfo};
        for (name, mut factory) in verification_entries() {
            let mut runtime = factory();
            runtime
                .frames()
                .resize_logical_viewport(WIDTH, HEIGHT)
                .unwrap();
            let submission = runtime
                .frames()
                .prepare_submission(
                    SurfaceInfo {
                        logical_width: WIDTH,
                        logical_height: HEIGHT,
                        pixel_width: WIDTH as u32,
                        pixel_height: HEIGHT as u32,
                        scale_factor: 1.0,
                    },
                    BackendCapabilities::RETAINED_PARTIAL,
                )
                .into_ready()
                .unwrap();
            let ResourceSync::Snapshot(resources) = submission.resources else {
                panic!("{name}: first frame requires a resource snapshot");
            };
            for command in submission.commands {
                if let RenderCommandKind::DrawGlyphRun { run, .. } = command.kind {
                    assert!(
                        resources
                            .ready
                            .iter()
                            .any(|resource| resource.id == run.font.resource_id()
                                && resource.revision == run.font.revision()),
                        "{name}: missing font {:?} revision {}",
                        run.font.resource_id(),
                        run.font.revision(),
                    );
                }
            }
        }
    }

    #[test]
    fn host_combo_clicks_rebuild_fixture_and_direction() {
        for compound in [false, true] {
            let (mut runtime, mut panel) = panel_scene(compound);
            let panel = panel.as_mut().unwrap();
            let context = egui::Context::default();
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(WIDTH as f32, HEIGHT as f32),
                )),
                ..Default::default()
            };
            let _ = context.run_ui(raw.clone(), |ui| panel.show(ui, &mut runtime));
            let click = |id: &str, runtime: &mut Runtime, panel: &mut Panel| {
                for _ in 0..2 {
                    let _ = context.run_ui(raw.clone(), |ui| panel.show(ui, runtime));
                }
                let pos = context
                    .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(id)))
                    .unwrap()
                    .center();
                for pressed in [true, false] {
                    let mut input = raw.clone();
                    input.events = vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ];
                    let _ = context.run_ui(input, |ui| panel.show(ui, runtime));
                }
            };
            let old = runtime.tree().contents().unwrap();
            click("graph-direction-menu", &mut runtime, panel);
            click("graph-east", &mut runtime, panel);
            assert!(runtime.tree().node(old).is_err());
            runtime.frames().stabilize_for_query().unwrap();
            let viewport = runtime
                .tree()
                .child_order(runtime.tree().contents().unwrap())
                .unwrap()[0];
            let owner = runtime.tree().child_order(viewport).unwrap()[0];
            assert_eq!(
                runtime
                    .tree()
                    .prepared_presentation(owner)
                    .unwrap()
                    .unwrap()
                    .data::<GraphProjectionSnapshot>()
                    .unwrap()
                    .request()
                    .direction(),
                GraphDirection::East
            );
            let old = runtime.tree().contents().unwrap();
            click("graph-fixture-menu", &mut runtime, panel);
            click("graph-fixture-2", &mut runtime, panel);
            assert!(runtime.tree().node(old).is_err());
            runtime.frames().stabilize_for_query().unwrap();
            let root = runtime.tree().contents().unwrap();
            let viewport = runtime.tree().child_order(root).unwrap()[0];
            let owner = runtime.tree().child_order(viewport).unwrap()[0];
            let count = runtime
                .tree()
                .prepared_presentation(owner)
                .unwrap()
                .unwrap()
                .data::<GraphProjectionSnapshot>()
                .unwrap()
                .request()
                .nodes()
                .len();
            assert_eq!(count, if compound { 10 } else { 11 });
        }
    }

    #[test]
    fn every_official_graph_switch_replaces_bindings_and_publishes_labels_and_routes() {
        let mut runtime = super::super::runtime_with_fonts();
        let mut previous = None;
        let mut registrations: Option<Registrations> = None;
        for is_compound in [false, true] {
            for direction in [GraphDirection::South, GraphDirection::East] {
                let mut settings = Settings {
                    compound: is_compound,
                    fixture: 0,
                    direction,
                };
                for index in 0..settings.names().len() {
                    settings.fixture = index;
                    let retired = registrations.clone();
                    let owner = install(&mut runtime, settings, &mut registrations);
                    if let Some(retired) = retired {
                        assert!(matches!(
                            runtime.connections().remove_router(retired.router),
                            Err(novadraw::connection::ConnectionRuntimeError::UnknownRouter(
                                _
                            ))
                        ));
                        for anchor in retired.anchors {
                            assert!(matches!(
                                runtime.connections().remove_anchor(anchor),
                                Err(novadraw::connection::ConnectionRuntimeError::UnknownAnchor(
                                    _
                                ))
                            ));
                        }
                    }
                    if let Some(old) = previous {
                        assert!(runtime.tree().node(old).is_err());
                    }
                    runtime.frames().stabilize_for_query().unwrap();
                    let tree = runtime.tree();
                    let presentation = tree.prepared_presentation(owner).unwrap().unwrap();
                    let graph = presentation.data::<GraphProjectionSnapshot>().unwrap();
                    for &(entity, figure) in graph.binding().entities() {
                        assert_eq!(tree.parent_id(figure).unwrap(), Some(owner));
                        assert_eq!(
                            tree.figure_bounds(figure).unwrap(),
                            graph.result().bounds(entity).unwrap()
                        );
                    }
                    for &(edge, connection) in graph.binding().edges() {
                        let transform =
                            tree.local_to_parent_transform(connection.figure()).unwrap();
                        let points: Vec<_> = tree
                            .connection_route_points(connection.figure())
                            .unwrap()
                            .unwrap()
                            .iter()
                            .map(|point| transform.transform_point(*point))
                            .collect();
                        assert_eq!(points, graph.result().route(edge).unwrap().points);
                        assert_eq!(tree.child_order(connection.figure()).unwrap().len(), 1);
                    }
                    let labels: Vec<_> = graph
                        .binding()
                        .entities()
                        .iter()
                        .filter_map(|&(entity, figure)| {
                            matches!(entity, GraphEndpoint::Node(_)).then_some(figure)
                        })
                        .collect();
                    for label in labels {
                        assert!(runtime.text().label_layout(label).is_ok());
                    }
                    previous = Some(owner);
                }
            }
        }
    }
}
