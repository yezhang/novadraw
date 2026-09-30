//! Browser `EditContext` host for canvas-backed direct text editing.

use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

use js_sys::{Array, Reflect};
use novadraw::{Point, Rectangle};
use novadraw_editor::{
    DirectTextEditSessionId, SessionTextInputEvent, TextInputEffect, TextInputSnapshot,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{DomRect, Event, HtmlCanvasElement, KeyboardEvent};

use crate::text_input::{
    WebEditContextBridge, WebTextOffsetError, utf8_range_to_utf16, utf16_code_unit_ranges_to_utf8,
};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = web_sys::EventTarget, js_name = EditContext)]
    #[derive(Clone, Debug)]
    type EditContext;

    #[wasm_bindgen(constructor, catch, js_class = EditContext)]
    fn new() -> Result<EditContext, JsValue>;

    #[wasm_bindgen(method, getter, js_name = text)]
    fn edit_context_text(this: &EditContext) -> String;

    #[wasm_bindgen(method, getter, js_name = selectionStart)]
    fn edit_context_selection_start(this: &EditContext) -> u32;

    #[wasm_bindgen(method, getter, js_name = selectionEnd)]
    fn edit_context_selection_end(this: &EditContext) -> u32;

    #[wasm_bindgen(method, catch, js_name = updateText)]
    fn update_text(this: &EditContext, start: u32, end: u32, text: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = updateSelection)]
    fn update_selection(this: &EditContext, start: u32, end: u32) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = updateControlBounds)]
    fn update_control_bounds(this: &EditContext, bounds: &DomRect) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = updateSelectionBounds)]
    fn update_selection_bounds(this: &EditContext, bounds: &DomRect) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = updateCharacterBounds)]
    fn update_character_bounds(
        this: &EditContext,
        range_start: u32,
        bounds: &Array,
    ) -> Result<(), JsValue>;

    #[wasm_bindgen(extends = Event, js_name = TextUpdateEvent)]
    #[derive(Clone, Debug)]
    type TextUpdateEvent;

    #[wasm_bindgen(method, getter, js_name = updateRangeStart)]
    fn update_range_start(this: &TextUpdateEvent) -> u32;

    #[wasm_bindgen(method, getter = text)]
    fn text_update_text(this: &TextUpdateEvent) -> String;

    #[wasm_bindgen(method, getter, js_name = selectionStart)]
    fn text_update_selection_start(this: &TextUpdateEvent) -> u32;

    #[wasm_bindgen(method, getter, js_name = selectionEnd)]
    fn text_update_selection_end(this: &TextUpdateEvent) -> u32;

    #[wasm_bindgen(extends = Event, js_name = CharacterBoundsUpdateEvent)]
    #[derive(Clone, Debug)]
    type CharacterBoundsUpdateEvent;

    #[wasm_bindgen(method, getter, js_name = rangeStart)]
    fn range_start(this: &CharacterBoundsUpdateEvent) -> u32;

    #[wasm_bindgen(method, getter, js_name = rangeEnd)]
    fn range_end(this: &CharacterBoundsUpdateEvent) -> u32;
}

struct DomListener {
    target: web_sys::EventTarget,
    event: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

/// Browser-native text input host attached directly to a canvas through `EditContext`.
pub struct WebEditContextHost {
    canvas: HtmlCanvasElement,
    context: EditContext,
    bridge: Rc<RefCell<WebEditContextBridge>>,
    events: Rc<RefCell<Vec<Result<SessionTextInputEvent, WebTextOffsetError>>>>,
    surface_origin: Rc<Cell<Point>>,
    logical_area: Cell<Option<Rectangle>>,
    original_role: Option<String>,
    original_aria_multiline: Option<String>,
    listeners: Vec<DomListener>,
}

impl WebEditContextHost {
    /// Returns whether the current browser exposes the EditContext API.
    pub fn is_supported() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        Reflect::has(window.as_ref(), &JsValue::from_str("EditContext")).unwrap_or(false)
    }

    /// Creates an EditContext host and attaches its event listeners.
    ///
    /// The character-bounds callback receives UTF-8 ranges in the active Editor draft and must
    /// return matching rectangles in logical surface coordinates.
    pub fn new_with_callbacks(
        canvas: &HtmlCanvasElement,
        event_ready: impl Fn() + 'static,
        character_bounds: impl Fn(Vec<Range<usize>>) -> Option<Vec<Rectangle>> + 'static,
    ) -> Result<Option<Self>, JsValue> {
        if !Self::is_supported()
            || !Reflect::has(canvas.as_ref(), &JsValue::from_str("editContext"))?
        {
            return Ok(None);
        }

        let context = EditContext::new()?;
        let bridge = Rc::new(RefCell::new(WebEditContextBridge::new()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let event_ready: Rc<dyn Fn()> = Rc::new(event_ready);
        let character_bounds: Rc<dyn Fn(Vec<Range<usize>>) -> Option<Vec<Rectangle>>> =
            Rc::new(character_bounds);
        let surface_origin = Rc::new(Cell::new(Point::ORIGIN));
        let original_role = canvas.get_attribute("role");
        let original_aria_multiline = canvas.get_attribute("aria-multiline");
        let mut listeners = Vec::new();

        listeners.push(install_listener(context.as_ref(), "compositionstart", {
            let bridge = Rc::clone(&bridge);
            move |_| bridge.borrow_mut().composition_started()
        })?);
        listeners.push(install_listener(context.as_ref(), "compositionend", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            let context = context.clone();
            move |_| {
                let selection = context.edit_context_selection_start() as usize
                    ..context.edit_context_selection_end() as usize;
                let normalized = bridge
                    .borrow_mut()
                    .composition_ended(context.edit_context_text(), selection);
                if let Some(event) = transpose_event(normalized, &events) {
                    events.borrow_mut().push(Ok(event));
                }
                event_ready();
            }
        })?);
        listeners.push(install_listener(context.as_ref(), "textupdate", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            let context = context.clone();
            move |event| {
                let event = event.unchecked_into::<TextUpdateEvent>();
                let selection = event.text_update_selection_start() as usize
                    ..event.text_update_selection_end() as usize;
                let normalized = bridge.borrow().text_updated(
                    context.edit_context_text(),
                    event.update_range_start() as usize,
                    &event.text_update_text(),
                    selection,
                );
                if let Some(event) = transpose_event(normalized, &events) {
                    events.borrow_mut().push(Ok(event));
                }
                event_ready();
            }
        })?);
        listeners.push(install_listener(
            context.as_ref(),
            "characterboundsupdate",
            {
                let character_bounds = Rc::clone(&character_bounds);
                let context = context.clone();
                let surface_origin = Rc::clone(&surface_origin);
                move |event| {
                    let event = event.unchecked_into::<CharacterBoundsUpdateEvent>();
                    let requested = event.range_start() as usize..event.range_end() as usize;
                    let ranges = match utf16_code_unit_ranges_to_utf8(
                        &context.edit_context_text(),
                        requested,
                    ) {
                        Ok(ranges) => ranges,
                        Err(_) => return,
                    };
                    let Some(bounds) = character_bounds(ranges) else {
                        return;
                    };
                    let origin = surface_origin.get();
                    let array = Array::new();
                    for bounds in bounds {
                        let Ok(rect) = DomRect::new_with_x_and_y_and_width_and_height(
                            bounds.x + origin.x(),
                            bounds.y + origin.y(),
                            bounds.width,
                            bounds.height,
                        ) else {
                            return;
                        };
                        array.push(rect.as_ref());
                    }
                    let _ = context.update_character_bounds(event.range_start(), &array);
                }
            },
        )?);
        listeners.push(install_listener(canvas.as_ref(), "keydown", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |event| {
                let keyboard = event.clone().unchecked_into::<KeyboardEvent>();
                let normalized = {
                    bridge.borrow().key_pressed(
                        &keyboard.key(),
                        keyboard.ctrl_key(),
                        keyboard.meta_key(),
                    )
                };
                if let Some(normalized) = normalized {
                    event.prevent_default();
                    events.borrow_mut().push(Ok(normalized));
                    event_ready();
                }
            }
        })?);
        listeners.push(install_listener(canvas.as_ref(), "blur", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |_| {
                let event = { bridge.borrow_mut().focus_lost() };
                if let Some(event) = event {
                    events.borrow_mut().push(Ok(event));
                    event_ready();
                }
            }
        })?);

        Ok(Some(Self {
            canvas: canvas.clone(),
            context,
            bridge,
            events,
            surface_origin,
            logical_area: Cell::new(None),
            original_role,
            original_aria_multiline,
            listeners,
        }))
    }

    /// Returns the session currently owning browser text input.
    pub fn active_session(&self) -> Option<DirectTextEditSessionId> {
        self.bridge.borrow().active_session()
    }

    /// Updates the canvas origin in browser client coordinates.
    pub fn set_surface_origin(&self, origin: Point) -> Result<(), JsValue> {
        if !origin.x().is_finite() || !origin.y().is_finite() {
            return Ok(());
        }
        self.surface_origin.set(origin);
        self.update_bounds()
    }

    /// Applies one Editor lease effect and synchronizes the current draft snapshot.
    pub fn apply_effect(
        &self,
        effect: &TextInputEffect,
        snapshot: Option<&TextInputSnapshot>,
    ) -> Result<bool, JsValue> {
        if !self.bridge.borrow_mut().apply_effect(effect) {
            return Ok(false);
        }
        match effect {
            TextInputEffect::Acquire { purpose, area, .. } => {
                self.logical_area.set(Some(*area));
                self.attach()?;
                self.canvas.set_attribute("role", "textbox")?;
                self.canvas.set_attribute(
                    "aria-multiline",
                    if *purpose == novadraw_editor::TextInputPurpose::Multiline {
                        "true"
                    } else {
                        "false"
                    },
                )?;
                self.synchronize_snapshot(snapshot.ok_or_else(|| {
                    JsValue::from_str("EditContext acquire requires an active Editor snapshot")
                })?)?;
                self.update_bounds()?;
                self.canvas.focus()?;
            }
            TextInputEffect::SetArea { area, .. } => {
                self.logical_area.set(Some(*area));
                self.synchronize_snapshot(snapshot.ok_or_else(|| {
                    JsValue::from_str("EditContext area update requires an active Editor snapshot")
                })?)?;
                self.update_bounds()?;
            }
            TextInputEffect::Release { .. } => {
                self.logical_area.set(None);
                self.detach()?;
                restore_attribute(&self.canvas, "role", self.original_role.as_deref())?;
                restore_attribute(
                    &self.canvas,
                    "aria-multiline",
                    self.original_aria_multiline.as_deref(),
                )?;
            }
        }
        Ok(true)
    }

    /// Drains normalized browser input accumulated since the previous call.
    pub fn take_events(&self) -> Result<Vec<SessionTextInputEvent>, WebTextOffsetError> {
        std::mem::take(&mut *self.events.borrow_mut())
            .into_iter()
            .collect()
    }

    fn attach(&self) -> Result<(), JsValue> {
        Reflect::set(
            self.canvas.as_ref(),
            &JsValue::from_str("editContext"),
            self.context.as_ref(),
        )
        .and_then(|attached| {
            attached
                .then_some(())
                .ok_or_else(|| JsValue::from_str("browser rejected canvas EditContext attachment"))
        })
    }

    fn detach(&self) -> Result<(), JsValue> {
        Reflect::set(
            self.canvas.as_ref(),
            &JsValue::from_str("editContext"),
            &JsValue::NULL,
        )
        .and_then(|detached| {
            detached
                .then_some(())
                .ok_or_else(|| JsValue::from_str("browser rejected canvas EditContext detach"))
        })
    }

    fn synchronize_snapshot(&self, snapshot: &TextInputSnapshot) -> Result<(), JsValue> {
        let current = self.context.edit_context_text();
        if current != snapshot.text() {
            self.context.update_text(
                0,
                u32::try_from(current.encode_utf16().count())
                    .map_err(|_| JsValue::from_str("EditContext text exceeds u32 offsets"))?,
                snapshot.text(),
            )?;
        }
        let selection = utf8_range_to_utf16(snapshot.text(), snapshot.selection())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.context.update_selection(
            u32::try_from(selection.start)
                .map_err(|_| JsValue::from_str("EditContext selection exceeds u32 offsets"))?,
            u32::try_from(selection.end)
                .map_err(|_| JsValue::from_str("EditContext selection exceeds u32 offsets"))?,
        )
    }

    fn update_bounds(&self) -> Result<(), JsValue> {
        if self.bridge.borrow().active_session().is_none() {
            return Ok(());
        }
        let control = self.canvas.get_bounding_client_rect();
        self.context.update_control_bounds(&control)?;
        let Some(area) = self.logical_area.get() else {
            return Ok(());
        };
        let origin = self.surface_origin.get();
        let selection = DomRect::new_with_x_and_y_and_width_and_height(
            area.x + origin.x(),
            area.y + origin.y(),
            area.width.max(1.0),
            area.height.max(1.0),
        )?;
        self.context.update_selection_bounds(&selection)
    }
}

impl Drop for WebEditContextHost {
    fn drop(&mut self) {
        for listener in &self.listeners {
            let _ = listener.target.remove_event_listener_with_callback(
                listener.event,
                listener.callback.as_ref().unchecked_ref(),
            );
        }
        let _ = self.detach();
        let _ = restore_attribute(&self.canvas, "role", self.original_role.as_deref());
        let _ = restore_attribute(
            &self.canvas,
            "aria-multiline",
            self.original_aria_multiline.as_deref(),
        );
    }
}

fn transpose_event(
    result: Result<Option<SessionTextInputEvent>, WebTextOffsetError>,
    events: &Rc<RefCell<Vec<Result<SessionTextInputEvent, WebTextOffsetError>>>>,
) -> Option<SessionTextInputEvent> {
    match result {
        Ok(event) => event,
        Err(error) => {
            events.borrow_mut().push(Err(error));
            None
        }
    }
}

fn install_listener(
    target: &web_sys::EventTarget,
    name: &'static str,
    callback: impl FnMut(Event) + 'static,
) -> Result<DomListener, JsValue> {
    let callback = Closure::<dyn FnMut(Event)>::new(callback);
    target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
    Ok(DomListener {
        target: target.clone(),
        event: name,
        callback,
    })
}

fn restore_attribute(
    canvas: &HtmlCanvasElement,
    name: &str,
    value: Option<&str>,
) -> Result<(), JsValue> {
    match value {
        Some(value) => canvas.set_attribute(name, value),
        None => canvas.remove_attribute(name),
    }
}
