use std::collections::{HashMap, HashSet};

use crate::{FigureGraph, FigureId, GestureSessionId};

#[derive(Clone, Copy, Default)]
struct GestureState {
    target: Option<FigureId>,
    scroll_controller: Option<Option<FigureId>>,
    zoom_controller: Option<Option<FigureId>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PointerId(u64);

impl PointerId {
    pub const PRIMARY: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Default)]
struct PointerState {
    target: Option<FigureId>,
    cursor_target: Option<FigureId>,
    captured: Option<FigureId>,
}

/// Persistent input state for one [`crate::Runtime`].
///
/// The state references Figure nodes by generational ID but does not own tree
/// topology. Runtime validates these references after every structural change.
#[derive(Default)]
pub struct InteractionState {
    pointers: HashMap<PointerId, PointerState>,
    hover_source: Option<FigureId>,
    focus_owner: Option<FigureId>,
    hovered: HashSet<FigureId>,
    pressed: HashSet<FigureId>,
    gestures: HashMap<GestureSessionId, GestureState>,
}

impl InteractionState {
    pub fn mouse_target(&self) -> Option<FigureId> {
        self.pointer_target(PointerId::PRIMARY)
    }

    pub fn focus_owner(&self) -> Option<FigureId> {
        self.focus_owner
    }

    pub fn captured(&self) -> Option<FigureId> {
        self.pointer_capture(PointerId::PRIMARY)
    }

    pub fn is_hovered(&self, id: FigureId) -> bool {
        self.hovered.contains(&id)
    }

    pub fn is_pressed(&self, id: FigureId) -> bool {
        self.pressed.contains(&id)
    }

    pub(crate) fn set_mouse_target(&mut self, id: Option<FigureId>) {
        self.pointer_mut(PointerId::PRIMARY).target = id;
    }

    pub(crate) fn cursor_target(&self) -> Option<FigureId> {
        self.pointers
            .get(&PointerId::PRIMARY)
            .and_then(|pointer| pointer.cursor_target)
    }

    pub(crate) fn set_cursor_target(&mut self, id: Option<FigureId>) {
        self.pointer_mut(PointerId::PRIMARY).cursor_target = id;
    }

    pub(crate) fn hover_source(&self) -> Option<FigureId> {
        self.hover_source
    }

    pub(crate) fn set_hover_source(&mut self, id: Option<FigureId>) {
        self.hover_source = id;
    }

    pub(crate) fn set_focus_owner(&mut self, id: Option<FigureId>) {
        self.focus_owner = id;
    }

    pub(crate) fn set_captured(&mut self, id: Option<FigureId>) {
        self.pointer_mut(PointerId::PRIMARY).captured = id;
    }

    pub(crate) fn set_hovered(&mut self, id: FigureId, hovered: bool) {
        if hovered {
            self.hovered.insert(id);
        } else {
            self.hovered.remove(&id);
        }
    }

    pub(crate) fn set_pressed(&mut self, id: FigureId, pressed: bool) {
        if pressed {
            self.pressed.insert(id);
        } else {
            self.pressed.remove(&id);
        }
    }

    pub(crate) fn gesture_target(&self, session_id: GestureSessionId) -> Option<FigureId> {
        self.gestures
            .get(&session_id)
            .and_then(|state| state.target)
    }

    pub(crate) fn has_gesture_session(&self, session_id: GestureSessionId) -> bool {
        self.gestures.contains_key(&session_id)
    }

    pub(crate) fn set_gesture_target(
        &mut self,
        session_id: GestureSessionId,
        target: Option<FigureId>,
    ) {
        if session_id != GestureSessionId::IMPULSE {
            self.gestures.insert(
                session_id,
                GestureState {
                    target,
                    ..GestureState::default()
                },
            );
        }
    }

    pub(crate) fn scroll_controller(
        &self,
        session_id: GestureSessionId,
    ) -> Option<Option<FigureId>> {
        self.gestures
            .get(&session_id)
            .and_then(|state| state.scroll_controller)
    }

    pub(crate) fn pin_scroll_controller(
        &mut self,
        session_id: GestureSessionId,
        controller: Option<FigureId>,
    ) -> Option<FigureId> {
        let state = self.gestures.get_mut(&session_id)?;
        if state.scroll_controller.is_none() {
            state.scroll_controller = Some(controller);
        }
        state.scroll_controller.flatten()
    }

    pub(crate) fn zoom_controller(&self, session_id: GestureSessionId) -> Option<Option<FigureId>> {
        self.gestures
            .get(&session_id)
            .and_then(|state| state.zoom_controller)
    }

    pub(crate) fn pin_zoom_controller(
        &mut self,
        session_id: GestureSessionId,
        controller: Option<FigureId>,
    ) -> Option<FigureId> {
        let state = self.gestures.get_mut(&session_id)?;
        if state.zoom_controller.is_none() {
            state.zoom_controller = Some(controller);
        }
        state.zoom_controller.flatten()
    }

    pub(crate) fn clear_gesture_target(&mut self, session_id: GestureSessionId) {
        self.gestures.remove(&session_id);
    }

    pub(crate) fn clear_gestures(&mut self) {
        self.gestures.clear();
    }

    pub fn pointer_target(&self, pointer: PointerId) -> Option<FigureId> {
        self.pointers.get(&pointer).and_then(|state| state.target)
    }

    pub fn pointer_capture(&self, pointer: PointerId) -> Option<FigureId> {
        self.pointers.get(&pointer).and_then(|state| state.captured)
    }

    fn pointer_mut(&mut self, pointer: PointerId) -> &mut PointerState {
        self.pointers.entry(pointer).or_default()
    }

    pub fn reconcile(&mut self, tree: &FigureGraph) {
        self.retain_figures(|id| {
            tree.is_attached(id)
                && tree.is_effectively_visible(id)
                && tree.is_effectively_enabled(id)
        });
    }

    fn retain_figures(&mut self, mut eligible: impl FnMut(FigureId) -> bool) {
        self.pointers.retain(|_, pointer| {
            pointer.target = pointer.target.filter(|id| eligible(*id));
            pointer.cursor_target = pointer.cursor_target.filter(|id| eligible(*id));
            pointer.captured = pointer.captured.filter(|id| eligible(*id));
            pointer.target.is_some()
                || pointer.cursor_target.is_some()
                || pointer.captured.is_some()
        });
        self.hover_source = self.hover_source.filter(|id| eligible(*id));
        self.focus_owner = self.focus_owner.filter(|id| eligible(*id));
        self.hovered.retain(|id| eligible(*id));
        self.pressed.retain(|id| eligible(*id));
        self.gestures.retain(|_, state| {
            state.target.is_none_or(&mut eligible)
                && state.scroll_controller.flatten().is_none_or(&mut eligible)
                && state.zoom_controller.flatten().is_none_or(&mut eligible)
        });
    }
}

#[cfg(test)]
mod tests {
    use slotmap::KeyData;

    use super::*;

    fn figure_id(value: u64) -> FigureId {
        FigureId::from(KeyData::from_ffi(value))
    }

    #[test]
    fn gesture_target_and_typed_controllers_share_one_session() {
        let mut state = InteractionState::default();
        let session = GestureSessionId::new(7);
        let target = figure_id(1);
        let scroll = figure_id(2);
        let zoom = figure_id(3);

        state.set_gesture_target(session, Some(target));
        state.pin_scroll_controller(session, Some(scroll));
        state.pin_zoom_controller(session, Some(zoom));

        state.pin_scroll_controller(session, Some(zoom));
        state.pin_zoom_controller(session, Some(scroll));

        assert_eq!(state.gesture_target(session), Some(target));
        assert_eq!(state.scroll_controller(session), Some(Some(scroll)));
        assert_eq!(state.zoom_controller(session), Some(Some(zoom)));

        state.clear_gesture_target(session);
        assert!(!state.has_gesture_session(session));
    }
}
