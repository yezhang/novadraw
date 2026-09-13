use std::{collections::HashMap, convert::Infallible, sync::Arc};

use novadraw::{
    BuiltinFont, Color, Figure, KeyModifiers, MouseButton, PlatformHost, Rectangle,
    RectangleFigure, RenderBackend, RenderOutcome, RootFigure, backend::vello::VelloRenderer,
};
use novadraw_apps::WinitPlatformHost;
use novadraw_editor::{
    EditPartBehavior, EditPartError, EditPartFactory, GraphicalViewer, ModelAdapter, ModelEvent,
    ModelRevision, PartFactoryContext, VisualUpdateContext,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

const WIDTH: f64 = 820.0;
const HEIGHT: f64 = 560.0;
const HANDLE_SIZE: f64 = 10.0;
const PRIMARY_HANDLE_COLOR: Color = Color {
    r: 0.98,
    g: 0.78,
    b: 0.12,
    a: 1.0,
};
const SECONDARY_HANDLE_COLOR: Color = Color {
    r: 0.12,
    g: 0.78,
    b: 0.82,
    a: 1.0,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct NodeId(u64);

#[derive(Clone, Copy)]
enum NodeKind {
    Canvas,
    Shape(Color),
    Widget,
}

#[derive(Clone, Copy)]
struct Node {
    bounds: Rectangle,
    kind: NodeKind,
}

struct DemoModel {
    revision: ModelRevision,
    nodes: HashMap<NodeId, Node>,
    children: HashMap<NodeId, Vec<NodeId>>,
}

impl DemoModel {
    fn new() -> Self {
        Self {
            revision: ModelRevision::initial(),
            nodes: HashMap::from([
                (
                    NodeId(1),
                    Node {
                        bounds: Rectangle::new(0.0, 0.0, WIDTH, HEIGHT),
                        kind: NodeKind::Canvas,
                    },
                ),
                (
                    NodeId(2),
                    Node {
                        bounds: Rectangle::new(80.0, 100.0, 180.0, 120.0),
                        kind: NodeKind::Shape(Color::hex("#2878D0")),
                    },
                ),
                (
                    NodeId(3),
                    Node {
                        bounds: Rectangle::new(330.0, 190.0, 200.0, 130.0),
                        kind: NodeKind::Shape(Color::hex("#38A169")),
                    },
                ),
                (
                    NodeId(4),
                    Node {
                        bounds: Rectangle::new(610.0, 80.0, 120.0, 60.0),
                        kind: NodeKind::Widget,
                    },
                ),
            ]),
            children: HashMap::from([(NodeId(1), vec![NodeId(2), NodeId(3), NodeId(4)])]),
        }
    }
}

impl ModelAdapter for DemoModel {
    type ModelId = NodeId;
    type Event = ();
    type Error = Infallible;

    fn root(&self) -> Self::ModelId {
        NodeId(1)
    }

    fn revision(&self) -> ModelRevision {
        self.revision
    }

    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error> {
        Ok(self.children.get(&model).cloned().unwrap_or_default())
    }

    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>> {
        Vec::new()
    }
}

struct DemoPart;

impl EditPartBehavior<DemoModel> for DemoPart {
    fn create_figure(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
    ) -> Result<Box<dyn Figure>, EditPartError> {
        let node = model.nodes[&model_id];
        Ok(match node.kind {
            NodeKind::Canvas => Box::new(RootFigure::new(
                node.bounds.x,
                node.bounds.y,
                node.bounds.width,
                node.bounds.height,
            )),
            NodeKind::Shape(color) => Box::new(
                RectangleFigure::new_with_color(
                    node.bounds.x,
                    node.bounds.y,
                    node.bounds.width,
                    node.bounds.height,
                    color,
                )
                .with_stroke(Color::hex("#17202A"), 2.0),
            ),
            NodeKind::Widget => {
                Box::new(novadraw::ButtonFigure::new("Widget").with_bounds(node.bounds))
            }
        })
    }

    fn refresh_visuals(
        &mut self,
        model: &DemoModel,
        model_id: NodeId,
        context: &mut VisualUpdateContext<'_>,
    ) -> Result<(), EditPartError> {
        context.set_primary_bounds(model.nodes[&model_id].bounds)?;
        Ok(())
    }
}

struct DemoFactory;

impl EditPartFactory<DemoModel> for DemoFactory {
    fn create(
        &mut self,
        _context: PartFactoryContext<NodeId>,
        _model: &DemoModel,
    ) -> Result<Box<dyn EditPartBehavior<DemoModel>>, EditPartError> {
        Ok(Box::new(DemoPart))
    }
}

type DemoViewer = GraphicalViewer<DemoModel, DemoFactory>;

struct DemoApp {
    window: Option<Arc<Window>>,
    host: Option<WinitPlatformHost>,
    renderer: Option<VelloRenderer>,
    viewer: Option<DemoViewer>,
    cursor: Option<(f64, f64)>,
    modifiers: KeyModifiers,
    handles: Vec<novadraw::FigureId>,
}

impl DemoApp {
    fn new() -> Self {
        Self {
            window: None,
            host: None,
            renderer: None,
            viewer: None,
            cursor: None,
            modifiers: KeyModifiers::default(),
            handles: Vec::new(),
        }
    }

    fn request_redraw(&self) {
        if let Some(host) = &self.host {
            host.request_redraw();
        }
    }

    fn sync_surface(&mut self) {
        let Some(host) = &self.host else {
            return;
        };
        let surface = host.surface_info();
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(
                surface.pixel_width,
                surface.pixel_height,
                surface.scale_factor,
            );
        }
        if let Some(viewer) = &mut self.viewer {
            viewer
                .runtime_mut()
                .resize_logical_viewport(surface.logical_width, surface.logical_height)
                .expect("window size must be valid");
        }
        self.request_redraw();
    }

    fn sync_selection_handles(&mut self) {
        let Some(viewer) = &mut self.viewer else {
            return;
        };
        for figure in self.handles.drain(..) {
            let _ = viewer.remove_overlay_visual(figure);
        }
        let selected = viewer.selection().items().to_vec();
        let primary = viewer.selection().primary();
        for part in selected {
            let Some(bounds) = viewer.part_bounds_in_surface(part) else {
                continue;
            };
            let color = if Some(part) == primary {
                PRIMARY_HANDLE_COLOR
            } else {
                SECONDARY_HANDLE_COLOR
            };
            for (x, y) in [
                (bounds.x, bounds.y),
                (bounds.x + bounds.width, bounds.y),
                (bounds.x, bounds.y + bounds.height),
                (bounds.x + bounds.width, bounds.y + bounds.height),
            ] {
                let handle = RectangleFigure::new_with_color(
                    x - HANDLE_SIZE / 2.0,
                    y - HANDLE_SIZE / 2.0,
                    HANDLE_SIZE,
                    HANDLE_SIZE,
                    color,
                )
                .with_stroke(Color::BLACK, 1.0);
                if let Ok((_, figure)) = viewer.add_handle_visual(part, Box::new(handle)) {
                    self.handles.push(figure);
                }
            }
        }
    }

    fn render(&mut self) {
        let (Some(viewer), Some(renderer), Some(host)) =
            (&mut self.viewer, &mut self.renderer, &self.host)
        else {
            return;
        };
        let Some(submission) = viewer
            .runtime_mut()
            .prepare_submission(host.surface_info(), renderer.capabilities())
        else {
            return;
        };
        let outcome = renderer.submit(&submission);
        viewer.runtime_mut().complete_submission(
            submission.session_id,
            submission.frame_id,
            outcome,
        );
        if outcome == RenderOutcome::Retry || viewer.runtime().has_pending_update() {
            host.request_redraw();
        }
    }

    fn update_title(&self) {
        let (Some(window), Some(viewer)) = (&self.window, &self.viewer) else {
            return;
        };
        window.set_title(&format!(
            "Novadraw Node Editor - {} selected",
            viewer.selection().items().len()
        ));
    }
}

impl ApplicationHandler<()> for DemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let window = self.window.clone().unwrap_or_else(|| {
            Arc::new(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("Novadraw Node Editor")
                            .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
                            .with_resizable(true),
                    )
                    .expect("window creation failed"),
            )
        });
        let size = window.inner_size();
        let scale = window.scale_factor();
        self.renderer = Some(VelloRenderer::new(
            Arc::clone(&window),
            f64::from(size.width) / scale,
            f64::from(size.height) / scale,
        ));
        self.host = Some(WinitPlatformHost::new(Arc::clone(&window)));
        self.window = Some(window);

        if let Some(viewer) = &mut self.viewer {
            viewer
                .runtime_mut()
                .reset_backend_session()
                .expect("backend session reset failed");
        } else {
            let mut viewer = GraphicalViewer::new(
                DemoModel::new(),
                DemoFactory,
                Rectangle::new(0.0, 0.0, WIDTH, HEIGHT),
            )
            .expect("demo Viewer construction failed");
            viewer
                .runtime_mut()
                .register_builtin_font(BuiltinFont::Inter)
                .expect("built-in font registration failed");
            self.viewer = Some(viewer);
        }
        self.update_title();
        self.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.sync_surface();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y));
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                if let Some(viewer) = &mut self.viewer {
                    viewer.dispatch_mouse_moved(position.x / scale, position.y / scale);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                if let Some(viewer) = &mut self.viewer {
                    viewer.pointer_exited();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some((x, y)) = self.cursor else {
                    return;
                };
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    _ => MouseButton::None,
                };
                if let Some(viewer) = &mut self.viewer {
                    match state {
                        ElementState::Pressed => {
                            let changed = viewer
                                .dispatch_mouse_pressed(
                                    x / scale,
                                    y / scale,
                                    button,
                                    self.modifiers,
                                )
                                .map(|outcome| outcome.selection().is_some())
                                .unwrap_or(false);
                            if changed {
                                self.sync_selection_handles();
                                self.update_title();
                            }
                        }
                        ElementState::Released => {
                            viewer.dispatch_mouse_released(x / scale, y / scale, button);
                        }
                    }
                }
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = KeyModifiers {
                    shift: state.shift_key(),
                    control: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && event.physical_key == PhysicalKey::Code(KeyCode::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::Focused(false) => {
                if let Some(viewer) = &mut self.viewer {
                    viewer.pointer_exited();
                    viewer.runtime_mut().cancel_gestures();
                    viewer.runtime_mut().release_focus();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(viewer) = &mut self.viewer {
            viewer.pointer_exited();
        }
        self.renderer = None;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("G3 manual validation:");
    println!("  click blue/green nodes to replace selection");
    println!("  Shift-click appends; Control/Command-click toggles");
    println!("  click blank canvas to clear");
    println!("  click the Widget button; selection must not change");
    println!("  yellow handles mark primary, cyan handles mark secondary");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut DemoApp::new())?;
    Ok(())
}
