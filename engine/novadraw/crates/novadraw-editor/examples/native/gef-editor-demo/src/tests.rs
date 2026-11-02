use crate::{
    Alignment, AppState, OutlineItemKind, Session, ToolMode, UiCommand,
    model::{Diagram, EdgeStyle, FIRST, INITIAL_BOUNDS, ShapeKind},
    session_from_model, sync,
    ui::{EditorUi, UiOutput},
};
use egui::{Event, Key, Modifiers, PointerButton, RawInput, Rect, TextureId, pos2, vec2};
use novadraw::event::{KeyModifiers, MouseButton};
use novadraw::geometry::{Point, Rectangle};
use novadraw::render::{
    BackendCapabilities, RenderBackend, RenderDisposition, RenderResult, RenderSubmission,
};

fn session() -> Session {
    let session = session_from_model(Diagram::minimal()).expect("valid minimal diagram");
    // Coordinate contracts exercise free movement, independently of workspace snapping.
    session.set_snap_enabled(false);
    session
}

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
    let _ = context.run_ui(
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

#[test]
fn six_alignments_use_primary_shape_and_one_undo_entry() {
    let initial = [
        Rectangle::new(10.0, 20.0, 60.0, 40.0),
        Rectangle::new(150.0, 170.0, 80.0, 60.0),
        Rectangle::new(300.0, 250.0, 100.0, 80.0),
    ];
    for (alignment, expected) in [
        (Alignment::Left, [(300.0, 20.0), (300.0, 170.0)]),
        (Alignment::HorizontalCenter, [(320.0, 20.0), (310.0, 170.0)]),
        (Alignment::Right, [(340.0, 20.0), (320.0, 170.0)]),
        (Alignment::Top, [(10.0, 250.0), (150.0, 250.0)]),
        (Alignment::VerticalCenter, [(10.0, 270.0), (150.0, 260.0)]),
        (Alignment::Bottom, [(10.0, 290.0), (150.0, 270.0)]),
    ] {
        let mut model = Diagram::default();
        for (id, bounds) in (1..=3).zip(initial) {
            model.shapes.get_mut(&id).unwrap().bounds = bounds;
        }
        let mut session = session_from_model(model).unwrap();
        let parts = (1..=3)
            .map(|id| session.viewer.part_for_model(id).unwrap())
            .collect::<Vec<_>>();
        session.viewer.set_selection(&parts).unwrap();
        let mut app = AppState::default();
        sync(&mut app, &mut session).unwrap();
        assert!(app.can_align);
        command(&mut session, &mut app, UiCommand::Align(alignment));
        for (index, (x, y)) in expected.into_iter().enumerate() {
            let actual = session.viewer.model().shapes[&(index as u64 + 1)].bounds;
            assert_eq!(
                actual,
                Rectangle::new(x, y, initial[index].width, initial[index].height)
            );
        }
        assert_eq!(session.viewer.model().shapes[&3].bounds, initial[2]);
        assert_eq!(session.viewer.selection().items(), parts);
        assert_eq!(session.viewer.model().edges.len(), 6);
        command(&mut session, &mut app, UiCommand::Align(alignment));
        command(&mut session, &mut app, UiCommand::Undo);
        for (id, bounds) in (1..=3).zip(initial) {
            assert_eq!(session.viewer.model().shapes[&id].bounds, bounds);
        }
        assert!(
            !session.domain.has_undo(),
            "repeat alignment must not add history"
        );
        command(&mut session, &mut app, UiCommand::Redo);
        assert_eq!(session.viewer.model().shapes[&1].bounds.x, expected[0].0);
        assert_eq!(session.viewer.model().shapes[&1].bounds.y, expected[0].1);
    }
}

#[test]
fn alignment_filters_connections_and_respects_disabled_and_primary_selection() {
    let mut session = crate::session();
    let mut app = AppState::default();
    sync(&mut app, &mut session).unwrap();
    assert!(!app.can_align);
    command(&mut session, &mut app, UiCommand::Align(Alignment::Left));
    assert!(!session.domain.has_undo());
    let first = session.viewer.part_for_model(1).unwrap();
    let second = session.viewer.part_for_model(2).unwrap();
    let connection = session
        .viewer
        .connection_part_for_model(7)
        .unwrap()
        .edit_part();
    session
        .viewer
        .set_selection(&[first, second, connection])
        .unwrap();
    sync(&mut app, &mut session).unwrap();
    assert!(
        !app.can_align,
        "connection cannot serve as the alignment reference"
    );
    command(&mut session, &mut app, UiCommand::Align(Alignment::Left));
    assert!(!session.domain.has_undo());
    session
        .viewer
        .set_selection(&[first, connection, second])
        .unwrap();
    sync(&mut app, &mut session).unwrap();
    assert!(app.can_align);
    app.disabled = true;
    command(&mut session, &mut app, UiCommand::Align(Alignment::Left));
    assert!(!session.domain.has_undo());
    app.disabled = false;
    command(&mut session, &mut app, UiCommand::Align(Alignment::Left));
    assert_eq!(session.viewer.model().shapes[&1].bounds.x, 270.0);
    assert_eq!(session.viewer.model().edges[&7].source, 1);
    assert_eq!(session.viewer.model().edges[&7].target, 2);
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
fn default_showcase_projects_shapes_connections_and_routed_geometry() {
    let mut session = crate::session();
    assert_eq!(session.viewer.model().shapes.len(), 6);
    assert_eq!(session.viewer.model().edges.len(), 6);
    let shapes = &session.viewer.model().shapes;
    for row in [[1, 2, 3], [4, 5, 6]] {
        assert_eq!(shapes[&row[0]].bounds.y, shapes[&row[1]].bounds.y);
        assert_eq!(shapes[&row[1]].bounds.y, shapes[&row[2]].bounds.y);
    }
    for column in [[1, 4], [2, 5], [3, 6]] {
        assert_eq!(shapes[&column[0]].bounds.x, shapes[&column[1]].bounds.x);
    }
    assert!(
        shapes
            .values()
            .all(|shape| shape.bounds.width == 120.0 && shape.bounds.height == 80.0)
    );
    assert_eq!(session.viewer.selection().items().len(), 1);
    assert_eq!(session.handle_visual_count(), 8);
    assert!(!session.domain.has_undo());
    assert!(!session.domain.is_dirty());
    assert!(
        session
            .viewer
            .model()
            .edges
            .values()
            .any(|edge| edge.style == EdgeStyle::Solid)
    );
    assert!(
        session
            .viewer
            .model()
            .edges
            .values()
            .any(|edge| edge.style == EdgeStyle::Dashed)
    );
    assert_eq!(session.viewer.parts().connection_parts().len(), 6);
    for edge in session.viewer.model().edges.keys() {
        let part = session
            .viewer
            .connection_part_for_model(*edge)
            .expect("showcase connection is projected");
        assert_eq!(
            session
                .viewer
                .connection_route_points_in_surface(part)
                .map(|points| points.len()),
            Some(2),
            "aligned showcase edge {edge} must not contain redundant bends"
        );
    }

    let mut app = AppState::default();
    sync(&mut app, &mut session).unwrap();
    assert_eq!(app.outline.len(), 12);
    assert_eq!(app.selected, [FIRST]);
    assert_eq!(
        app.property_subject,
        Some((FIRST, session.viewer.model().shapes[&FIRST].bounds))
    );
    command(&mut session, &mut app, UiCommand::Select(7));
    assert_eq!(session.handle_visual_count(), 2);
    assert_eq!(
        session.viewer.selection().primary(),
        session
            .viewer
            .connection_part_for_model(7)
            .map(|part| part.edit_part())
    );
    let mut backend = TestBackend::default();
    session.render([640.0, 480.0], 1.0, &mut backend).unwrap();
    assert_eq!(backend.submissions, 1);
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
    assert!(output.interaction.unwrap());
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
fn move_feedback_carries_selection_outline_and_handles_until_commit() {
    let mut session = session();
    let mut app = AppState::default();
    command(&mut session, &mut app, UiCommand::Select(FIRST));
    assert_eq!(session.handle_visual_count(), 8);

    session
        .domain
        .pointer_pressed(
            &mut session.viewer,
            Point::new(90.0, 100.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    session
        .domain
        .pointer_moved(&mut session.viewer, Point::new(140.0, 130.0))
        .unwrap();
    sync(&mut app, &mut session).unwrap();

    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
    assert_eq!(session.handle_visual_count(), 0);
    let feedback = session
        .viewer
        .runtime()
        .tree()
        .child_order(session.viewer.root_layers().scaled_feedback())
        .unwrap();
    assert_eq!(feedback.len(), 9);
    assert_eq!(
        session
            .viewer
            .runtime()
            .tree()
            .figure_bounds(feedback[0])
            .unwrap(),
        Rectangle::new(110.0, 100.0, 120.0, 80.0)
    );

    session
        .domain
        .pointer_released(
            &mut session.viewer,
            Point::new(140.0, 130.0),
            MouseButton::Left,
        )
        .unwrap();
    sync(&mut app, &mut session).unwrap();
    assert_eq!(
        session.viewer.model().shapes[&FIRST].bounds,
        Rectangle::new(110.0, 100.0, 120.0, 80.0)
    );
    assert_eq!(session.handle_visual_count(), 8);
    assert!(
        session
            .viewer
            .runtime()
            .tree()
            .child_order(session.viewer.root_layers().scaled_feedback())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn projected_resize_handle_commits_one_bounds_command_and_tracks_geometry() {
    let mut session = session();
    let mut app = AppState::default();
    command(&mut session, &mut app, UiCommand::Select(FIRST));
    assert_eq!(session.handle_visual_count(), 8);

    session
        .domain
        .pointer_pressed(
            &mut session.viewer,
            Point::new(180.0, 150.0),
            MouseButton::Left,
            KeyModifiers::default(),
        )
        .unwrap();
    session
        .domain
        .pointer_moved(&mut session.viewer, Point::new(210.0, 170.0))
        .unwrap();
    assert!(
        session
            .domain
            .pointer_released(
                &mut session.viewer,
                Point::new(210.0, 170.0),
                MouseButton::Left,
            )
            .unwrap()
            .command_executed()
    );
    sync(&mut app, &mut session).unwrap();
    assert_eq!(
        session.viewer.model().shapes[&FIRST].bounds,
        Rectangle::new(60.0, 70.0, 150.0, 100.0)
    );
    assert_eq!(session.handle_visual_count(), 8);

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
    assert_eq!(created.bounds, Rectangle::new(460.0, 320.0, 120.0, 80.0));
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
fn tool_mode_tracks_typed_tools_and_one_shot_completion() {
    let mut session = session();
    let mut app = AppState::default();
    assert_eq!(app.tool_mode, ToolMode::Selection);

    command(&mut session, &mut app, UiCommand::Marquee);
    assert_eq!(app.tool_mode, ToolMode::Marquee);
    command(&mut session, &mut app, UiCommand::Rectangle);
    assert_eq!(app.tool_mode, ToolMode::Rectangle);
    click(&mut session, Point::new(500.0, 320.0));
    sync(&mut app, &mut session).unwrap();
    assert_eq!(app.tool_mode, ToolMode::Selection);

    command(
        &mut session,
        &mut app,
        UiCommand::Connect(EdgeStyle::Dashed),
    );
    assert_eq!(app.tool_mode, ToolMode::DashedConnection);
    click(&mut session, Point::new(90.0, 100.0));
    click(&mut session, Point::new(320.0, 200.0));
    sync(&mut app, &mut session).unwrap();
    assert_eq!(app.tool_mode, ToolMode::Selection);
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
fn outline_projection_keeps_shape_and_connection_types_explicit() {
    let mut session = session();
    let mut app = AppState::default();
    sync(&mut app, &mut session).unwrap();
    assert_eq!(
        app.outline.iter().map(|item| item.kind).collect::<Vec<_>>(),
        [OutlineItemKind::Rectangle, OutlineItemKind::Ellipse,]
    );

    connect(
        &mut session,
        &mut app,
        EdgeStyle::Dashed,
        Point::new(90.0, 100.0),
        Point::new(320.0, 200.0),
    );
    assert_eq!(
        app.outline.last().map(|item| item.kind),
        Some(OutlineItemKind::Connection {
            style: EdgeStyle::Dashed,
            source: FIRST,
            target: 2,
        })
    );
}

#[test]
fn invalid_geometry_draft_does_not_enter_command_history() {
    let mut session = session();
    let mut app = AppState::default();
    command(&mut session, &mut app, UiCommand::Select(FIRST));
    app.properties = [80.0, 90.0, 10.0, 100.0];
    app.pending.push_back(UiCommand::ApplyProperties);
    assert!(sync(&mut app, &mut session).is_err());
    assert_eq!(session.viewer.model().shapes[&FIRST].bounds, INITIAL_BOUNDS);
    assert!(!session.domain.has_undo());
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
    app.properties = [80.0, 90.0, 160.0, 100.0];
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
    let mut app = AppState {
        path_input: directory.join("good.json").to_string_lossy().into_owned(),
        ..Default::default()
    };
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
fn layer_visibility_and_lock_are_undoable_and_gate_canvas_editing() {
    let mut session = session();
    let mut app = AppState::default();
    command(&mut session, &mut app, UiCommand::Select(FIRST));

    command(&mut session, &mut app, UiCommand::ToggleVisibility(FIRST));
    assert!(!session.viewer.model().presentation(FIRST).visible);
    assert!(matches!(
        session.viewer.target_at(90.0, 100.0),
        novadraw_editor::ViewerTarget::Contents(_)
    ));
    command(&mut session, &mut app, UiCommand::Undo);
    assert!(session.viewer.model().presentation(FIRST).visible);

    command(&mut session, &mut app, UiCommand::ToggleLock(FIRST));
    assert!(session.viewer.model().presentation(FIRST).locked);
    session.viewer.clear_selection();
    sync(&mut app, &mut session).unwrap();
    command(&mut session, &mut app, UiCommand::Select(FIRST));
    assert_eq!(app.selected, [FIRST]);
    command(&mut session, &mut app, UiCommand::Delete);
    assert!(session.viewer.model().shapes.contains_key(&FIRST));
    command(&mut session, &mut app, UiCommand::Undo);
    assert!(!session.viewer.model().presentation(FIRST).locked);
}

#[test]
fn groups_order_names_and_appearance_roundtrip_through_history_and_json() {
    let mut session = session();
    let mut app = AppState::default();
    let first = session.viewer.part_for_model(FIRST).unwrap();
    let second = session.viewer.part_for_model(2).unwrap();
    session.viewer.set_selection(&[first, second]).unwrap();
    sync(&mut app, &mut session).unwrap();

    command(&mut session, &mut app, UiCommand::Group);
    let group_id = *session.viewer.model().groups.keys().next().unwrap();
    assert_eq!(session.viewer.model().groups[&group_id].members, [FIRST, 2]);
    command(&mut session, &mut app, UiCommand::Undo);
    assert!(session.viewer.model().groups.is_empty());
    command(&mut session, &mut app, UiCommand::Redo);

    session.viewer.set_selection(&[second, first]).unwrap();
    sync(&mut app, &mut session).unwrap();
    command(&mut session, &mut app, UiCommand::BringForward);
    assert_eq!(session.viewer.model().order, [2, FIRST]);
    command(&mut session, &mut app, UiCommand::Undo);
    assert_eq!(session.viewer.model().order, [FIRST, 2]);
    command(&mut session, &mut app, UiCommand::Redo);

    command(
        &mut session,
        &mut app,
        UiCommand::Rename("Primary node".into()),
    );
    let mut presentation = session.viewer.model().presentation(FIRST);
    presentation.fill = [87, 166, 120, 255];
    presentation.stroke = [176, 40, 40, 255];
    presentation.opacity = 65;
    command(
        &mut session,
        &mut app,
        UiCommand::SetAppearance(presentation),
    );

    let json = session.viewer.model().to_json().unwrap();
    let reopened = Diagram::from_json(&json).unwrap();
    assert_eq!(reopened.order, [2, FIRST]);
    assert_eq!(reopened.name(FIRST), "Primary node");
    assert_eq!(reopened.presentation(FIRST), presentation);
    assert_eq!(reopened.groups[&group_id].members, [FIRST, 2]);
}

#[test]
fn workspace_grid_snap_and_zoom_do_not_change_document_history() {
    let mut session = session();
    let mut app = AppState::default();
    let before = session.viewer.model().to_json().unwrap();
    assert!(!session.domain.is_dirty());

    for command in [
        UiCommand::ToggleGrid,
        UiCommand::ToggleSnap,
        UiCommand::ZoomIn,
        UiCommand::ZoomOut,
        UiCommand::ZoomReset,
        UiCommand::ZoomFit,
    ] {
        crate::tests::command(&mut session, &mut app, command);
    }

    assert_eq!(session.viewer.model().to_json().unwrap(), before);
    assert!(!session.domain.has_undo());
    assert!(!session.domain.is_dirty());
}

#[test]
fn fit_visible_shapes_centers_content_in_viewport() {
    let mut session = session_from_model(Diagram::showcase()).unwrap();
    session
        .viewer
        .drive()
        .resize_logical_viewport(760.0, 720.0)
        .unwrap();
    session.viewer.stabilize_for_query().unwrap();
    session.fit_visible_shapes().unwrap();

    let origin = session.viewer.viewport_origin().unwrap();
    let scale = session.viewer.viewport_scale().unwrap();
    let viewport = session.viewer.viewport_bounds_in_surface().unwrap();
    let extent = session
        .viewer
        .model()
        .shapes
        .values()
        .map(|shape| shape.bounds)
        .reduce(|extent, bounds| extent.union(bounds))
        .unwrap();
    let left = (extent.x - origin.x()) * scale;
    let right = viewport.width - (extent.x + extent.width - origin.x()) * scale;
    let top = (extent.y - origin.y()) * scale;
    let bottom = viewport.height - (extent.y + extent.height - origin.y()) * scale;

    const FIT_TOLERANCE: f64 = 1e-9;
    assert!(
        (scale - 680.0 / 540.0).abs() < FIT_TOLERANCE,
        "unexpected fit scale: {scale}"
    );
    assert!(
        (left - right).abs() < FIT_TOLERANCE,
        "horizontal margins differ: left={left}, right={right}"
    );
    assert!(
        (top - bottom).abs() < FIT_TOLERANCE,
        "vertical margins differ: top={top}, bottom={bottom}"
    );
}

#[test]
fn creation_uses_session_grid_snap_without_persisting_the_setting() {
    let mut session = session_from_model(Diagram::minimal()).unwrap();
    let mut app = AppState::default();
    command(
        &mut session,
        &mut app,
        UiCommand::Drop(ShapeKind::Rectangle, Point::new(453.0, 307.0)),
    );
    assert_eq!(
        session.viewer.model().shapes[&3].bounds,
        Rectangle::new(448.0, 304.0, 120.0, 80.0)
    );
    let document = session.viewer.model().to_json().unwrap();
    assert!(!document.contains("snap_to_grid"));
    assert!(!document.contains("show_grid"));
}

#[test]
fn document_parser_rejects_invalid_references_and_geometry() {
    let valid = Diagram::default().to_json().unwrap();
    assert!(Diagram::from_json(&valid).is_ok());
    let mut document: serde_json::Value = serde_json::from_str(&valid).unwrap();
    document["order"] = serde_json::json!([1, 1]);
    assert!(Diagram::from_json(&document.to_string()).is_err());

    let mut document: serde_json::Value = serde_json::from_str(&valid).unwrap();
    document["shapes"]["1"]["bounds"][2] = serde_json::json!(-1.0);
    assert!(Diagram::from_json(&document.to_string()).is_err());

    let mut document: serde_json::Value = serde_json::from_str(&valid).unwrap();
    document["version"] = serde_json::json!(99);
    assert!(Diagram::from_json(&document.to_string()).is_err());
}
