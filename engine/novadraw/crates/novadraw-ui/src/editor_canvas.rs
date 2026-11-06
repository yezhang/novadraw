//! egui host for one Editor Viewer/Domain and its shared-GPU Canvas.
use crate::{
    canvas::CanvasSurface,
    ime::{Composer, Edit, InputStep},
};
use egui::{Event, Key, PointerButton, Response};
use novadraw::{
    event::{Key as EditorKey, KeyEvent, KeyEventKind, KeyModifiers, MouseButton},
    geometry::Point,
    render::RenderBackend,
    runtime::{FramePreparation, MonotonicTime, SubmissionAcknowledgement},
    text::TextMovement,
};
use novadraw_editor::{
    DirectTextEditSessionId, EditPartFactory, EditorDomain, ExtendTextSelection, GraphicalViewer,
    ModelAdapter, SessionTextInputEvent, TextDelete, TextEditMode, TextInputEffect, TextInputEvent,
};
use std::time::Duration;

const SCROLL_LINE_POINTS: f32 = 40.0;
const AUTOEXPOSE_INTERVAL: Duration = Duration::from_millis(16);
const MICROS_PER_SECOND: f64 = 1_000_000.0;

/// Application-facing result; model and history remain in the Editor.
pub struct CanvasOutput {
    pub response: Response,
    pub error: Option<String>,
}

/// Persistent Canvas. Construct once, then call `show` in an egui panel body.
///
/// `viewer` and `domain` are the same objects used by toolbar commands. Call
/// `deactivate` before hiding the panel or replacing its document.
pub struct EditorCanvas<A: ModelAdapter + 'static, F: EditPartFactory<A>> {
    pub viewer: GraphicalViewer<A, F>,
    pub domain: EditorDomain<A>,
    surface: CanvasSurface,
    input: EditorCanvasInput,
    last_error: Option<String>,
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> EditorCanvas<A, F> {
    pub fn new(
        id: egui::Id,
        gpu: egui_wgpu::RenderState,
        viewer: GraphicalViewer<A, F>,
    ) -> Result<Self, String> {
        Self::from_parts(id, gpu, viewer, EditorDomain::new())
    }

    /// Transfers existing Editor state, preserving its command history and session.
    pub fn from_parts(
        id: egui::Id,
        gpu: egui_wgpu::RenderState,
        viewer: GraphicalViewer<A, F>,
        domain: EditorDomain<A>,
    ) -> Result<Self, String> {
        Ok(Self {
            viewer,
            domain,
            surface: CanvasSurface::new(gpu)?,
            input: EditorCanvasInput::new(id),
            last_error: None,
        })
    }

    pub fn error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Resolves text focus by its policy and cancels uncommitted pointer gestures.
    pub fn deactivate(&mut self, context: &egui::Context) -> Result<(), String> {
        self.input
            .deactivate(context, &mut self.viewer, &mut self.domain)
    }

    /// Recreates resources after the application obtains a new eframe device.
    pub fn recreate_gpu(&mut self, gpu: egui_wgpu::RenderState) -> Result<(), String> {
        let replacement = CanvasSurface::new(gpu)?;
        self.viewer
            .drive()
            .reset_backend_session()
            .map_err(|e| e.to_string())?;
        self.surface = replacement;
        self.viewer.drive().request_full_redraw();
        Ok(())
    }

    /// Fills available space. Disable with `ui.add_enabled_ui(false, ...)`.
    pub fn show(&mut self, ui: &mut egui::Ui) -> CanvasOutput {
        let response = self.input.allocate(ui);
        let result = (|| -> Result<(), String> {
            if self
                .surface
                .resize(response.rect.size(), response.ctx.pixels_per_point())?
            {
                if let Some(surface) = self.surface.surface() {
                    self.viewer
                        .drive()
                        .resize_logical_viewport(surface.logical_width, surface.logical_height)
                        .map_err(|e| e.to_string())?;
                }
                self.viewer.drive().request_full_redraw();
            }
            if self.surface.surface().is_none() {
                self.input
                    .deactivate(&response.ctx, &mut self.viewer, &mut self.domain)?;
                return Ok(());
            }
            self.input
                .handle(&response, &mut self.viewer, &mut self.domain)?;
            let now = host_time(response.ctx.input(|i| i.time));
            self.viewer
                .drive()
                .advance_time(now)
                .map_err(|e| e.to_string())?;
            if self.domain.autoexpose_requested() {
                let elapsed = Duration::from_secs_f32(response.ctx.input(|i| i.stable_dt).max(0.0));
                let tick = self
                    .domain
                    .autoexpose_tick(&mut self.viewer, elapsed)
                    .map_err(|e| e.to_string())?;
                if tick.continue_requested() {
                    response.ctx.request_repaint_after(AUTOEXPOSE_INTERVAL);
                }
            }
            let viewer = &mut self.viewer;
            if let Some(result) = self.surface.with_backend(|surface, backend| {
                match viewer
                    .drive()
                    .prepare_submission(surface, backend.capabilities())
                {
                    FramePreparation::Ready(submission) => {
                        let result = backend.submit(&submission);
                        match viewer
                            .drive()
                            .complete_submission(submission.session_id, submission.frame_id, result)
                            .map_err(|e| e.to_string())?
                        {
                            SubmissionAcknowledgement::Presented
                            | SubmissionAcknowledgement::Skipped => Ok(false),
                            SubmissionAcknowledgement::Retry => Ok(true),
                            other => Err(format!("Canvas submission: {other:?}")),
                        }
                    }
                    FramePreparation::Idle | FramePreparation::Suspended => Ok(false),
                    other => Err(format!("Canvas preparation: {other:?}")),
                }
            }) && result?
            {
                response.ctx.request_repaint();
            }
            self.input.publish_ime(&response, &mut self.viewer);
            if let Some(deadline) = self.viewer.drive().next_wake_deadline() {
                response.ctx.request_repaint_after(Duration::from_micros(
                    deadline.as_micros().saturating_sub(now.as_micros()),
                ));
            }
            Ok(())
        })();
        self.last_error = result.err();
        if self.last_error.is_some() {
            let _ = self
                .input
                .abort(&response.ctx, &mut self.viewer, &mut self.domain);
        }
        self.surface.paint(ui, response.rect);
        CanvasOutput {
            response,
            error: self.last_error.clone(),
        }
    }
}

impl<A: ModelAdapter + 'static, F: EditPartFactory<A>> Drop for EditorCanvas<A, F> {
    fn drop(&mut self) {
        let _ = self.domain.cancel_tool(&mut self.viewer);
        self.viewer.drive().cancel_gestures();
        if self.viewer.edit_session().is_some() {
            let _ = self.domain.cancel_direct_text_edit(&mut self.viewer);
        }
        if let Some(context) = self.input.context.clone() {
            let _ = self
                .input
                .deactivate(&context, &mut self.viewer, &mut self.domain);
        }
    }
}

/// Input-only adapter for consumers with their own rendering/session wrapper.
///
/// This is the same adapter used by `EditorCanvas`; it requires no GPU to test.
pub struct EditorCanvasInput {
    id: egui::Id,
    captured: Option<PointerButton>,
    pan: Option<(Point, Point)>,
    hovered: bool,
    focused: bool,
    composer: Composer,
    lease: Option<DirectTextEditSessionId>,
    caret: Option<novadraw::geometry::Rectangle>,
    context: Option<egui::Context>,
}

impl EditorCanvasInput {
    pub fn new(id: egui::Id) -> Self {
        Self {
            id,
            captured: None,
            pan: None,
            hovered: false,
            focused: false,
            composer: Composer::new().with_owner(id),
            lease: None,
            caret: None,
            context: None,
        }
    }

    /// Allocates and dispatches input without rendering. The caller drives frames.
    pub fn show<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        ui: &mut egui::Ui,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
    ) -> CanvasOutput {
        let response = self.allocate(ui);
        let error = self.handle(&response, viewer, domain).err();
        if error.is_some() {
            let _ = self.abort(&response.ctx, viewer, domain);
        } else {
            self.publish_ime(&response, viewer);
        }
        CanvasOutput { response, error }
    }

    fn allocate(&self, ui: &mut egui::Ui) -> Response {
        let (_, rect) = ui.allocate_space(ui.available_size().max(egui::Vec2::ZERO));
        ui.interact(rect, self.id, egui::Sense::click_and_drag())
            .on_hover_text("Diagram editor Canvas")
    }

    fn cancel<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
    ) -> Result<(), String> {
        self.captured = None;
        self.pan = None;
        let result = domain.cancel_tool(viewer);
        viewer.drive().cancel_gestures();
        result.map_err(|e| e.to_string())
    }

    fn abort<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        context: &egui::Context,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
    ) -> Result<(), String> {
        let result = if viewer.edit_session().is_some() {
            domain
                .cancel_direct_text_edit(viewer)
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        self.composer.discard(context);
        let _ = self.composer.update(context, false);
        self.lease = None;
        self.caret = None;
        self.focused = false;
        context.memory_mut(|m| m.surrender_focus(self.id));
        viewer.drive().release_focus();
        let cleanup = self.cancel(viewer, domain);
        viewer.take_text_input_effects();
        result.and(cleanup)
    }

    pub fn deactivate<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        context: &egui::Context,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
    ) -> Result<(), String> {
        let text_result = if viewer.edit_session().is_some() {
            domain
                .direct_text_focus_lost(viewer)
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            Ok(())
        };
        self.composer.discard(context);
        let _ = self.composer.update(context, false);
        self.lease = None;
        self.caret = None;
        self.focused = false;
        context.memory_mut(|m| m.surrender_focus(self.id));
        viewer.drive().release_focus();
        let cancel_result = self.cancel(viewer, domain);
        // Discard release effects after the local descriptor has been withdrawn.
        viewer.take_text_input_effects();
        text_result.and(cancel_result)
    }

    fn handle<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        response: &Response,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
    ) -> Result<(), String> {
        let context = &response.ctx;
        self.context = Some(context.clone());
        if !response.enabled() || !context.input(|i| i.focused) {
            if self.focused || self.captured.is_some() || viewer.edit_session().is_some() {
                self.deactivate(context, viewer, domain)?;
            }
            return Ok(());
        }
        let editing = viewer.edit_session().is_some() && context.memory(|m| m.has_focus(self.id));
        let batch_lease = viewer.edit_session().map(|s| s.session());
        if self.lease != viewer.edit_session().map(|s| s.session()) {
            self.composer.discard(context);
        }
        let steps = if editing {
            self.composer.update_ordered(context, true)
        } else {
            context.input(|i| i.events.iter().cloned().map(InputStep::Event).collect())
        };
        let mut consumed_keys = false;
        for step in steps {
            match step {
                InputStep::Edit(edit) => {
                    if batch_lease.is_some()
                        && viewer.edit_session().map(|s| s.session()) == batch_lease
                        && context.memory(|m| m.has_focus(self.id))
                    {
                        self.edit(context, viewer, domain, edit)?;
                        consumed_keys = true;
                    }
                }
                InputStep::Event(event) => match event {
                    Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    } => {
                        let inside = response.rect.contains(layer_position(response, pos))
                            && context.layer_id_at(pos) == Some(response.layer_id);
                        if pressed
                            && inside
                            && (response.contains_pointer() || response.is_pointer_button_down_on())
                        {
                            if self.captured.is_some() {
                                continue;
                            }
                            response.request_focus();
                            let point = local(response, pos);
                            if button == PointerButton::Middle {
                                self.pan = Some((
                                    point,
                                    viewer.viewport_origin().map_err(|e| e.to_string())?,
                                ));
                                self.captured = Some(button);
                            } else if let Some(button_value) = mouse_button(button) {
                                let outcome = domain
                                    .pointer_pressed(
                                        viewer,
                                        point,
                                        button_value,
                                        key_modifiers(modifiers),
                                    )
                                    .map_err(|e| e.to_string())?;
                                if domain.has_active_gesture()
                                    || outcome.dispatch().capture().is_some()
                                    || outcome.dispatch().is_handled()
                                {
                                    self.captured = Some(button);
                                }
                            }
                        } else if !pressed && self.captured == Some(button) {
                            if self.pan.take().is_none()
                                && let Some(button_value) = mouse_button(button)
                            {
                                domain
                                    .pointer_released(viewer, local(response, pos), button_value)
                                    .map_err(|e| e.to_string())?;
                            }
                            self.captured = None;
                        } else if pressed
                            && !inside
                            && (self.focused || viewer.edit_session().is_some())
                        {
                            self.deactivate(context, viewer, domain)?;
                        }
                    }
                    Event::PointerMoved(pos)
                        if self.captured.is_some() || response.contains_pointer() =>
                    {
                        let point = local(response, pos);
                        if let Some((start, origin)) = self.pan {
                            let zoom = viewer.viewport_scale().map_err(|e| e.to_string())?;
                            domain
                                .set_viewport_origin(
                                    viewer,
                                    Point::new(
                                        origin.x() + (start.x() - point.x()) / zoom,
                                        origin.y() + (start.y() - point.y()) / zoom,
                                    ),
                                )
                                .map_err(|e| e.to_string())?;
                        } else {
                            domain
                                .pointer_moved(viewer, point)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Event::PointerGone if self.captured.is_none() => domain.pointer_exited(viewer),
                    Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if response.contains_pointer() => {
                        let points = match unit {
                            egui::MouseWheelUnit::Point => delta,
                            egui::MouseWheelUnit::Line => delta * SCROLL_LINE_POINTS,
                            egui::MouseWheelUnit::Page => delta * response.rect.size(),
                        };
                        if modifiers.ctrl || modifiers.command {
                            // egui's zoom convention: one e-fold per 200 logical points.
                            const ZOOM_POINTS: f64 = 200.0;
                            let zoom = viewer.viewport_scale().map_err(|e| e.to_string())?;
                            let at = context.pointer_latest_pos().map(|p| local(response, p));
                            domain
                                .set_viewport_scale_at(
                                    viewer,
                                    zoom * (f64::from(points.y) / ZOOM_POINTS).exp(),
                                    at,
                                )
                                .map_err(|e| e.to_string())?;
                        } else {
                            let origin = viewer.viewport_origin().map_err(|e| e.to_string())?;
                            let zoom = viewer.viewport_scale().map_err(|e| e.to_string())?;
                            domain
                                .set_viewport_origin(
                                    viewer,
                                    Point::new(
                                        origin.x() - f64::from(points.x) / zoom,
                                        origin.y() - f64::from(points.y) / zoom,
                                    ),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Event::Zoom(factor) if response.contains_pointer() => {
                        let zoom = viewer.viewport_scale().map_err(|e| e.to_string())?;
                        domain
                            .set_viewport_scale_at(
                                viewer,
                                zoom * f64::from(factor),
                                context.pointer_latest_pos().map(|p| local(response, p)),
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    Event::Key {
                        key,
                        pressed,
                        repeat,
                        modifiers,
                        ..
                    } if context.memory(|m| m.has_focus(self.id)) && !editing => {
                        if pressed && key == Key::Escape {
                            self.cancel(viewer, domain)?;
                        } else if pressed && !repeat && modifiers.command && key == Key::Z {
                            if modifiers.shift {
                                domain.redo(viewer)
                            } else {
                                domain.undo(viewer)
                            }
                            .map_err(|e| e.to_string())?;
                        } else if let Some(key) = editor_key(key) {
                            domain
                                .key_event(
                                    viewer,
                                    KeyEvent {
                                        kind: if pressed {
                                            KeyEventKind::Pressed
                                        } else {
                                            KeyEventKind::Released
                                        },
                                        key,
                                        modifiers: key_modifiers(modifiers),
                                    },
                                )
                                .map_err(|e| e.to_string())?;
                        } else {
                            continue;
                        }
                        consumed_keys = true;
                    }
                    Event::WindowFocused(false) => self.deactivate(context, viewer, domain)?,
                    _ => {}
                },
            }
        }
        if self.focused && !context.memory(|m| m.has_focus(self.id)) {
            self.deactivate(context, viewer, domain)?;
        }
        self.focused = context.memory(|m| m.has_focus(self.id));
        if self.hovered && !response.contains_pointer() && self.captured.is_none() {
            domain.pointer_exited(viewer);
        }
        self.hovered = response.contains_pointer();
        if consumed_keys {
            context.input_mut(|i| {
                i.events.retain(|e| {
                    !matches!(
                        e,
                        Event::Key { .. }
                            | Event::Text(_)
                            | Event::Paste(_)
                            | Event::Copy
                            | Event::Cut
                    )
                })
            });
        }
        Ok(())
    }

    fn text<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
        event: TextInputEvent,
    ) -> Result<(), String> {
        let Some(session) = viewer.edit_session().map(|s| s.session()) else {
            return Ok(());
        };
        domain
            .handle_text_input_event(viewer, SessionTextInputEvent::new(session, event))
            .map_err(|e| e.to_string())
    }

    fn edit<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        context: &egui::Context,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
        edit: Edit,
    ) -> Result<(), String> {
        let event = match edit {
            Edit::Preedit { text, active } => {
                if text.is_empty() {
                    TextInputEvent::CancelComposition
                } else {
                    let selection =
                        active.map(|r| char_byte(&text, r.start)..char_byte(&text, r.end));
                    TextInputEvent::Preedit { text, selection }
                }
            }
            Edit::Insert(text) | Edit::Paste(text) => TextInputEvent::InsertText(text),
            Edit::LineBreak { modifiers } => {
                return self.text_key(viewer, domain, Key::Enter, modifiers);
            }
            Edit::Key { key, modifiers, .. } => {
                return self.text_key(viewer, domain, key, modifiers);
            }
            Edit::Copy | Edit::Cut => {
                let cut = matches!(edit, Edit::Cut);
                let snapshot = viewer.edit_input_snapshot().map_err(|e| e.to_string())?;
                let selected = snapshot.selection();
                if selected.is_empty() {
                    return Ok(());
                }
                context.copy_text(snapshot.text()[selected].to_owned());
                if cut {
                    TextInputEvent::Delete(TextDelete::NextVisual)
                } else {
                    return Ok(());
                }
            }
            Edit::DeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                // Official web corrections count scalar characters, not visual graphemes.
                let snapshot = viewer.edit_input_snapshot().map_err(|e| e.to_string())?;
                let selection = snapshot.selection();
                let start = snapshot.text()[..selection.start]
                    .char_indices()
                    .rev()
                    .nth(before_chars.saturating_sub(1))
                    .map_or(0, |(i, _)| i);
                let start = if before_chars == 0 {
                    selection.start
                } else {
                    start
                };
                let end = selection.end + char_byte(&snapshot.text()[selection.end..], after_chars);
                let mut text = snapshot.text().to_owned();
                text.replace_range(start..end, "");
                TextInputEvent::Synchronize(novadraw_editor::TextInputSnapshot::new(
                    text,
                    start..start,
                    None,
                ))
            }
        };
        self.text(viewer, domain, event)
    }

    fn text_key<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        viewer: &mut GraphicalViewer<A, F>,
        domain: &mut EditorDomain<A>,
        key: Key,
        modifiers: egui::Modifiers,
    ) -> Result<(), String> {
        let word = modifiers.alt || modifiers.ctrl;
        let movement = match key {
            Key::ArrowLeft => Some(if word {
                TextMovement::PreviousWord
            } else {
                TextMovement::PreviousVisual
            }),
            Key::ArrowRight => Some(if word {
                TextMovement::NextWord
            } else {
                TextMovement::NextVisual
            }),
            Key::ArrowUp => Some(TextMovement::PreviousLine),
            Key::ArrowDown => Some(TextMovement::NextLine),
            Key::Home => Some(if modifiers.command {
                TextMovement::DocumentStart
            } else {
                TextMovement::LineStart
            }),
            Key::End => Some(if modifiers.command {
                TextMovement::DocumentEnd
            } else {
                TextMovement::LineEnd
            }),
            _ => None,
        };
        let event = if let Some(movement) = movement {
            TextInputEvent::Move {
                movement,
                extend: if modifiers.shift {
                    ExtendTextSelection::Yes
                } else {
                    ExtendTextSelection::No
                },
            }
        } else {
            match key {
                Key::Escape => TextInputEvent::Cancel,
                Key::Enter
                    if viewer
                        .edit_session()
                        .is_some_and(|s| s.mode() == TextEditMode::Multiline)
                        && !modifiers.command =>
                {
                    TextInputEvent::InsertText("\n".into())
                }
                Key::Enter => TextInputEvent::Accept,
                Key::Backspace => TextInputEvent::Delete(if word {
                    TextDelete::PreviousWord
                } else {
                    TextDelete::PreviousVisual
                }),
                Key::Delete => TextInputEvent::Delete(if word {
                    TextDelete::NextWord
                } else {
                    TextDelete::NextVisual
                }),
                Key::A if modifiers.command => TextInputEvent::SelectAll,
                _ => return Ok(()),
            }
        };
        self.text(viewer, domain, event)
    }

    fn publish_ime<A: ModelAdapter + 'static, F: EditPartFactory<A>>(
        &mut self,
        response: &Response,
        viewer: &mut GraphicalViewer<A, F>,
    ) {
        for effect in viewer.take_text_input_effects() {
            match effect {
                TextInputEffect::Acquire { session, area, .. } => {
                    self.lease = Some(session);
                    self.caret = Some(area);
                }
                TextInputEffect::SetArea { session, area } if self.lease == Some(session) => {
                    self.caret = Some(area)
                }
                TextInputEffect::Release { session } if self.lease == Some(session) => {
                    self.lease = None;
                    self.caret = None;
                    self.composer.discard(&response.ctx);
                    let _ = self.composer.update(&response.ctx, false);
                }
                _ => {}
            }
        }
        if response.ctx.memory(|m| m.has_focus(self.id))
            && let Some(area) = self.caret
        {
            self.composer
                .activate(&response.ctx, egui::IMEPurpose::Normal);
            let rect = egui::Rect::from_min_size(
                response.rect.min + egui::vec2(area.x as f32, area.y as f32),
                egui::vec2(area.width as f32, area.height as f32),
            );
            let transform = response
                .ctx
                .layer_transform_to_global(response.layer_id)
                .unwrap_or_default();
            self.composer.set_caret(&response.ctx, transform * rect);
            // A pointer may have acquired an edit session in this pass. Its first
            // keyboard pass starts Composer; request that pass without polling.
            if !self.focused {
                response.ctx.request_repaint();
            }
        }
    }
}

fn char_byte(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(i, _)| i)
}
fn host_time(time: f64) -> MonotonicTime {
    MonotonicTime::from_micros((time.max(0.0) * MICROS_PER_SECOND) as u64)
}
fn local(response: &Response, position: egui::Pos2) -> Point {
    let position = layer_position(response, position);
    Point::new(
        f64::from(position.x - response.rect.min.x),
        f64::from(position.y - response.rect.min.y),
    )
}
fn layer_position(response: &Response, position: egui::Pos2) -> egui::Pos2 {
    response
        .ctx
        .layer_transform_to_global(response.layer_id)
        .map_or(position, |transform| transform.inverse() * position)
}
fn mouse_button(button: PointerButton) -> Option<MouseButton> {
    match button {
        PointerButton::Primary => Some(MouseButton::Left),
        PointerButton::Secondary => Some(MouseButton::Right),
        PointerButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}
fn key_modifiers(m: egui::Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: m.shift,
        control: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd,
    }
}
fn editor_key(key: Key) -> Option<EditorKey> {
    Some(match key {
        Key::Enter => EditorKey::Enter,
        Key::Escape => EditorKey::Escape,
        Key::Tab => EditorKey::Tab,
        Key::ArrowUp => EditorKey::ArrowUp,
        Key::ArrowDown => EditorKey::ArrowDown,
        Key::ArrowLeft => EditorKey::ArrowLeft,
        Key::ArrowRight => EditorKey::ArrowRight,
        Key::Space => EditorKey::Character(' '),
        _ if key.name().chars().count() == 1 => {
            EditorKey::Character(key.name().chars().next()?.to_ascii_lowercase())
        }
        _ => return None,
    })
}
