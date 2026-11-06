use super::*;
use egui::{Event, PointerButton};
use novadraw_ui::editor_canvas::EditorCanvasInput;

struct Harness {
    context: egui::Context,
    input: EditorCanvasInput,
    viewer: GraphicalViewer<Diagram, ShapeFactory>,
    domain: novadraw_editor::EditorDomain<Diagram>,
    time: f64,
}
impl Harness {
    fn new() -> Self {
        let mut model = Diagram::default();
        model
            .shapes
            .retain(|id, _| *id == gef_editor_demo::model::FIRST);
        model
            .order
            .retain(|id| *id == gef_editor_demo::model::FIRST);
        model.edges.clear();
        let mut viewer = GraphicalViewer::new(
            model,
            ShapeFactory::new(WorkspaceSettings::default()),
            Rectangle::new(0.0, 0.0, 480.0, 320.0),
        )
        .unwrap();
        viewer
            .drive()
            .resize_logical_viewport(480.0, 320.0)
            .unwrap();
        viewer.stabilize_for_query().unwrap();
        let mut result = Self {
            context: egui::Context::default(),
            input: EditorCanvasInput::new(egui::Id::unique("test-canvas")),
            viewer,
            domain: novadraw_editor::EditorDomain::new(),
            time: 0.0,
        };
        result.frame(Vec::new(), true, true);
        result
    }
    fn frame(&mut self, events: Vec<Event>, enabled: bool, focused: bool) {
        self.time += 0.1;
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 340.0),
                )),
                events,
                time: Some(self.time),
                focused,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        let output = self.input.show(ui, &mut self.viewer, &mut self.domain);
                        assert!(output.error.is_none(), "{:?}", output.error);
                    });
                });
            },
        );
        output.textures_delta.clear();
    }
    fn button(pos: egui::Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        }
    }
    fn bounds(&self) -> Rectangle {
        self.viewer.model().shapes[&gef_editor_demo::model::FIRST].bounds
    }
    fn down(&mut self, button: PointerButton) {
        let point = egui::pos2(110.0, 110.0);
        self.frame(vec![Event::PointerMoved(point)], true, true);
        self.frame(vec![Self::button(point, button, true)], true, true);
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.context.tex_manager().write().take_delta().clear();
    }
}

#[test]
fn canvas_selects_and_commits_drag_outside_then_undo_redo_share_domain() {
    let mut h = Harness::new();
    let before = h.bounds();
    h.down(PointerButton::Primary);
    assert_eq!(h.viewer.selection().items().len(), 1);
    let outside = egui::pos2(540.0, 120.0);
    h.frame(vec![Event::PointerMoved(outside)], true, true);
    h.frame(
        vec![Harness::button(outside, PointerButton::Primary, false)],
        true,
        true,
    );
    let after = h.bounds();
    assert_ne!(after, before);
    assert!(!h.domain.has_active_gesture());
    assert!(h.domain.has_undo());
    h.domain.undo(&mut h.viewer).unwrap();
    assert_eq!(h.bounds(), before);
    h.domain.redo(&mut h.viewer).unwrap();
    assert_eq!(h.bounds(), after);
}

#[test]
fn disabled_and_window_focus_loss_cancel_drag_without_model_commit() {
    for (enabled, focused) in [(false, true), (true, false)] {
        let mut h = Harness::new();
        let before = h.bounds();
        h.down(PointerButton::Primary);
        h.frame(
            vec![Event::PointerMoved(egui::pos2(160.0, 160.0))],
            true,
            true,
        );
        h.frame(Vec::new(), enabled, focused);
        assert_eq!(h.bounds(), before);
        assert!(!h.domain.has_active_gesture());
        assert!(!h.domain.has_undo());
    }
}

#[test]
fn middle_pan_is_viewport_only_and_unrelated_release_does_not_end_it() {
    let mut h = Harness::new();
    let before = h.bounds();
    h.down(PointerButton::Middle);
    let origin = h.viewer.viewport_origin().unwrap();
    h.frame(
        vec![Harness::button(
            egui::pos2(110.0, 110.0),
            PointerButton::Secondary,
            false,
        )],
        true,
        true,
    );
    h.frame(
        vec![Event::PointerMoved(egui::pos2(140.0, 145.0))],
        true,
        true,
    );
    assert_ne!(h.viewer.viewport_origin().unwrap(), origin);
    h.frame(
        vec![Harness::button(
            egui::pos2(140.0, 145.0),
            PointerButton::Middle,
            false,
        )],
        true,
        true,
    );
    let final_origin = h.viewer.viewport_origin().unwrap();
    h.frame(
        vec![Event::PointerMoved(egui::pos2(180.0, 180.0))],
        true,
        true,
    );
    assert_eq!(h.viewer.viewport_origin().unwrap(), final_origin);
    assert_eq!(h.bounds(), before);
    assert!(!h.domain.has_undo());
}

#[test]
fn wheel_is_scoped_to_canvas_and_escape_cancels() {
    let mut h = Harness::new();
    h.frame(
        vec![Event::PointerMoved(egui::pos2(100.0, 100.0))],
        true,
        true,
    );
    let origin = h.viewer.viewport_origin().unwrap();
    h.frame(
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -30.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        }],
        true,
        true,
    );
    assert_ne!(h.viewer.viewport_origin().unwrap(), origin);
    h.frame(
        vec![Event::PointerMoved(egui::pos2(600.0, 400.0))],
        true,
        true,
    );
    let origin = h.viewer.viewport_origin().unwrap();
    h.frame(
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -30.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        }],
        true,
        true,
    );
    assert_eq!(h.viewer.viewport_origin().unwrap(), origin);
    h.down(PointerButton::Primary);
    h.frame(
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        true,
        true,
    );
    assert!(!h.domain.has_active_gesture());
}

#[test]
fn hide_releases_focus_and_capture_without_committing() {
    let mut h = Harness::new();
    let before = h.bounds();
    h.down(PointerButton::Primary);
    h.frame(
        vec![Event::PointerMoved(egui::pos2(160.0, 160.0))],
        true,
        true,
    );
    h.input
        .deactivate(&h.context, &mut h.viewer, &mut h.domain)
        .unwrap();
    assert_eq!(h.bounds(), before);
    assert!(!h.domain.has_active_gesture());
    assert!(h.context.memory(|m| m.focused()).is_none());
}
