use crate::{
    AppState, Session, UiCommand,
    model::{Diagram, EdgeStyle, FIRST, INITIAL_BOUNDS, ShapeKind},
    session, sync,
    ui::{EditorUi, UiOutput},
};
use egui::{Event, Key, Modifiers, PointerButton, RawInput, Rect, TextureId, pos2, vec2};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};
use novadraw::render::{
    BackendCapabilities, RenderBackend, RenderDisposition, RenderResult, RenderSubmission,
};

fn click(session: &mut Session, point: Point) {
    session
        .domain
        .pointer_pressed(
            &mut session.viewer,
            point,
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    session
        .domain
        .pointer_released(&mut session.viewer, point, MouseButton::Left)
        .unwrap();
}

fn command(session: &mut Session, app: &mut AppState, command: UiCommand) {
    app.pending.push_back(command);
    sync(app, session).unwrap();
}

fn connect(
    session: &mut Session,
    app: &mut AppState,
    style: EdgeStyle,
    source: Point,
    target: Point,
) {
    command(session, app, UiCommand::Connect(style));
    click(session, source);
    click(session, target);
    sync(app, session).unwrap();
}

fn egui_frame(
    context: &egui::Context,
    editor_ui: &mut EditorUi,
    app: &mut AppState,
    session: &mut Session,
    events: Vec<Event>,
) -> UiOutput {
    let mut output = None;
    context.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1180.0, 820.0))),
            events,
            ..Default::default()
        },
        |root| {
            output = Some(editor_ui.show(root, app, session, TextureId::User(1)));
        },
    );
    output.unwrap()
}

fn canvas_position(app: &AppState, local: Point) -> egui::Pos2 {
    pos2(
        app.canvas_bounds.x as f32 + local.x() as f32,
        app.canvas_bounds.y as f32 + local.y() as f32,
    )
}

#[derive(Default)]
struct TestBackend {
    submissions: usize,
    surfaces: Vec<[u32; 2]>,
}

impl RenderBackend for TestBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
            .with_glyph_runs()
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
            .with_host_background()
            .with_per_operation_antialias()
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        self.submissions += 1;
        self.surfaces.push([
            submission.surface.pixel_width,
            submission.surface.pixel_height,
        ]);
        Ok(RenderDisposition::Presented)
    }

    fn resize(&mut self, _: u32, _: u32, _: f64) {}
}

#[test]
fn egui_canvas_uses_local_coordinates_and_one_history_entry() {
    let context = egui::Context::default();
    let mut editor_ui = EditorUi::default();
    let mut app = AppState::default();
    let mut session = session();
    sync(&mut app, &mut session).unwrap();
    egui_frame(&context, &mut editor_ui, &mut app, &mut session, vec![]);
    let start = canvas_position(&app, Point::new(90.0, 100.0));
    let end = canvas_position(&app, Point::new(140.0, 130.0));
    let output = egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::default(),
            },
        ],
    );
    assert_eq!(output.interaction.unwrap(), true);
    assert_eq!(
        session.viewer.model().shapes[&FIRST].bounds,
        Rectangle::new(110.0, 100.0, 120.0, 80.0)
    );
    assert!(session.domain.has_undo());
    command(&mut session, &mut app, UiCommand::Undo);
    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
    assert!(!session.domain.has_undo());
}

#[test]
fn egui_focus_loss_cancels_active_drag_without_committing() {
    let context = egui::Context::default();
    let mut editor_ui = EditorUi::default();
    let mut app = AppState::default();
    let mut session = session();
    sync(&mut app, &mut session).unwrap();
    egui_frame(&context, &mut editor_ui, &mut app, &mut session, vec![]);
    let start = canvas_position(&app, Point::new(90.0, 100.0));
    let end = canvas_position(&app, Point::new(140.0, 130.0));
    egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
            Event::PointerMoved(end),
        ],
    )
    .interaction
    .unwrap();
    assert!(session.domain.has_active_gesture());
    egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![Event::WindowFocused(false)],
    )
    .interaction
    .unwrap();
    assert!(!session.domain.has_active_gesture());
    assert!(!session.domain.has_undo());
    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
}

#[test]
fn egui_template_drop_creates_at_canvas_local_position() {
    let context = egui::Context::default();
    let mut editor_ui = EditorUi::default();
    let mut app = AppState::default();
    let mut session = session();
    sync(&mut app, &mut session).unwrap();
    egui_frame(&context, &mut editor_ui, &mut app, &mut session, vec![]);
    egui::DragAndDrop::set_payload(&context, ShapeKind::Ellipse);
    let drop = canvas_position(&app, Point::new(460.0, 320.0));
    egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![
            Event::PointerMoved(drop),
            Event::PointerButton {
                pos: drop,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::default(),
            },
        ],
    );
    sync(&mut app, &mut session).unwrap();
    let created = session.viewer.model().shapes.get(&3).unwrap();
    assert_eq!(created.kind, ShapeKind::Ellipse);
    assert_eq!(created.bounds, Rectangle::new(400.0, 280.0, 120.0, 80.0));
}

#[test]
fn focused_canvas_shortcuts_undo_without_rebuilding_session() {
    let context = egui::Context::default();
    let mut editor_ui = EditorUi::default();
    let mut app = AppState::default();
    let mut session = session();
    sync(&mut app, &mut session).unwrap();
    egui_frame(&context, &mut editor_ui, &mut app, &mut session, vec![]);
    let namespace = session.viewer.namespace();
    let start = canvas_position(&app, Point::new(90.0, 100.0));
    let end = canvas_position(&app, Point::new(140.0, 130.0));
    egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::default(),
            },
        ],
    );
    egui_frame(
        &context,
        &mut editor_ui,
        &mut app,
        &mut session,
        vec![Event::Key {
            key: Key::Z,
            physical_key: Some(Key::Z),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                command: true,
                ctrl: true,
                ..Default::default()
            },
        }],
    )
    .interaction
    .unwrap();
    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
    assert_eq!(session.viewer.namespace(), namespace);
    assert!(session.domain.has_redo());
}

#[test]
fn create_delete_and_undo_share_the_editor_command_chain() {
    let mut session = session();
    let mut app = AppState::default();
    command(
        &mut session,
        &mut app,
        UiCommand::Drop(ShapeKind::Ellipse, Point::new(450.0, 300.0)),
    );
    assert_eq!(session.viewer.model().shapes.len(), 3);
    command(&mut session, &mut app, UiCommand::Select(3));
    command(&mut session, &mut app, UiCommand::Delete);
    assert_eq!(session.viewer.model().shapes.len(), 2);
    command(&mut session, &mut app, UiCommand::Undo);
    assert_eq!(session.viewer.model().shapes.len(), 3);
    command(&mut session, &mut app, UiCommand::Undo);
    assert_eq!(session.viewer.model().shapes.len(), 2);
}

#[test]
fn connection_creation_rejects_self_and_duplicate_and_preserves_style() {
    let mut session = session();
    let mut app = AppState::default();
    let first = Point::new(90.0, 100.0);
    let second = Point::new(320.0, 200.0);
    connect(&mut session, &mut app, EdgeStyle::Dashed, first, second);
    assert_eq!(session.viewer.model().edges.len(), 1);
    assert_eq!(session.viewer.model().edges[&3].style, EdgeStyle::Dashed);
    connect(&mut session, &mut app, EdgeStyle::Solid, first, second);
    assert_eq!(session.viewer.model().edges.len(), 1);
    command(&mut session, &mut app, UiCommand::Connect(EdgeStyle::Solid));
    click(&mut session, first);
    click(&mut session, first);
    assert_eq!(session.viewer.model().edges.len(), 1);
}

#[test]
fn geometry_savepoint_and_open_roundtrip_preserve_document_state() {
    let directory = std::env::temp_dir().join(format!(
        "novadraw-egui-shapes-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let mut session = session();
    let mut app = AppState::default();
    command(&mut session, &mut app, UiCommand::Select(FIRST));
    app.properties = ["80".into(), "90".into(), "160".into(), "100".into()];
    command(&mut session, &mut app, UiCommand::ApplyProperties);
    assert_eq!(
        session.viewer.model().shapes[&FIRST].bounds,
        Rectangle::new(80.0, 90.0, 160.0, 100.0)
    );
    app.path_input = directory.join("shapes.json").to_string_lossy().into_owned();
    command(&mut session, &mut app, UiCommand::SaveAs);
    assert!(!session.domain.is_dirty());
    let saved = session.viewer.model().to_json().unwrap();
    command(
        &mut session,
        &mut app,
        UiCommand::Drop(ShapeKind::Rectangle, Point::new(520.0, 340.0)),
    );
    assert!(session.domain.is_dirty());
    command(&mut session, &mut app, UiCommand::DiscardAndOpen);
    assert_eq!(session.viewer.model().to_json().unwrap(), saved);
    assert!(!session.domain.has_undo());
    assert!(!session.domain.is_dirty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn failed_save_as_does_not_move_savepoint_or_replace_path() {
    let directory = std::env::temp_dir().join(format!("novadraw-egui-save-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut session = session();
    let mut app = AppState::default();
    app.path_input = directory.join("good.json").to_string_lossy().into_owned();
    command(&mut session, &mut app, UiCommand::SaveAs);
    command(
        &mut session,
        &mut app,
        UiCommand::Drop(ShapeKind::Rectangle, Point::new(500.0, 300.0)),
    );
    let good_path = app.current_path.clone();
    app.path_input = directory
        .join("missing/failed.json")
        .to_string_lossy()
        .into_owned();
    app.pending.push_back(UiCommand::SaveAs);
    assert!(sync(&mut app, &mut session).is_err());
    assert_eq!(app.current_path, good_path);
    assert!(session.domain.is_dirty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn session_render_tracks_dpi_without_replacing_viewer_or_history() {
    let mut session = session();
    let namespace = session.viewer.namespace();
    let mut backend = TestBackend::default();
    session.render([640.0, 480.0], 1.0, &mut backend).unwrap();
    session.render([420.0, 320.0], 2.0, &mut backend).unwrap();
    session.render([280.0, 210.0], 1.5, &mut backend).unwrap();
    assert_eq!(backend.surfaces, [[640, 480], [840, 640], [420, 315]]);
    assert_eq!(session.viewer.namespace(), namespace);
    assert!(!session.domain.has_undo());
    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
}

#[test]
fn document_parser_rejects_invalid_references_and_geometry() {
    let valid = Diagram::default().to_json().unwrap();
    assert!(Diagram::from_json(&valid).is_ok());
    let duplicate_id = valid.replace("\"id\":2", "\"id\":1");
    assert!(Diagram::from_json(&duplicate_id).is_err());
    let invalid_geometry = valid.replace("\"width\":120.0", "\"width\":-1.0");
    assert!(Diagram::from_json(&invalid_geometry).is_err());
    let invalid_version = valid.replace("\"version\":1", "\"version\":99");
    assert!(Diagram::from_json(&invalid_version).is_err());
}
