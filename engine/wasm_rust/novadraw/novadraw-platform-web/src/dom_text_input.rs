//! Browser-owned hidden textarea used by direct text editing.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use novadraw::{Point, Rectangle};
use novadraw_editor::{SessionTextInputEvent, TextInputEffect, TextInputPurpose};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{CompositionEvent, Document, Event, HtmlTextAreaElement, InputEvent, KeyboardEvent};

use crate::{WebTextInputAction, WebTextInputBridge, text_input::utf16_range_to_utf8};

struct DomListener {
    event: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

/// Owns an invisible DOM textarea and queues normalized Editor text-input events.
pub struct WebTextInputHost {
    input: HtmlTextAreaElement,
    bridge: Rc<RefCell<WebTextInputBridge>>,
    events: Rc<RefCell<Vec<SessionTextInputEvent>>>,
    surface_origin: Cell<Point>,
    logical_area: Cell<Option<Rectangle>>,
    listeners: Vec<DomListener>,
}

impl WebTextInputHost {
    /// Creates and attaches one hidden textarea to the document body.
    pub fn new(document: &Document) -> Result<Self, JsValue> {
        Self::new_with_event_ready(document, || {})
    }

    /// Creates a hidden textarea and invokes `event_ready` after input is queued.
    pub fn new_with_event_ready(
        document: &Document,
        event_ready: impl Fn() + 'static,
    ) -> Result<Self, JsValue> {
        let input = document
            .create_element("textarea")?
            .dyn_into::<HtmlTextAreaElement>()?;
        input.set_attribute("autocomplete", "off")?;
        input.set_attribute("autocorrect", "off")?;
        input.set_attribute("autocapitalize", "off")?;
        input.set_attribute("spellcheck", "false")?;
        input.set_attribute("tabindex", "-1")?;
        input.set_attribute("aria-label", "Novadraw text input")?;
        input.set_attribute("data-novadraw-text-input", "true")?;
        input.set_attribute("data-active", "false")?;
        let style = input.style();
        style.set_property("position", "fixed")?;
        style.set_property("opacity", "0.01")?;
        style.set_property("color", "transparent")?;
        style.set_property("background", "transparent")?;
        style.set_property("border", "0")?;
        style.set_property("padding", "0")?;
        style.set_property("caret-color", "transparent")?;
        style.set_property("pointer-events", "none")?;
        style.set_property("resize", "none")?;
        style.set_property("overflow", "hidden")?;
        style.set_property("z-index", "1")?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("document has no body"))?
            .append_child(&input)?;

        let bridge = Rc::new(RefCell::new(WebTextInputBridge::new()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let event_ready: Rc<dyn Fn()> = Rc::new(event_ready);
        let mut listeners = Vec::new();

        install_listener(&input, &mut listeners, "compositionstart", {
            let bridge = Rc::clone(&bridge);
            move |_| bridge.borrow_mut().composition_started()
        })?;
        install_listener(&input, &mut listeners, "compositionupdate", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |event| {
                let event = event.unchecked_into::<CompositionEvent>();
                let text = event.data().unwrap_or_default();
                // The DOM value and selection are updated after `compositionupdate`.
                let normalized = bridge.borrow_mut().composition_updated(text, None);
                if let Some(event) = normalized {
                    events.borrow_mut().push(event);
                    event_ready();
                }
            }
        })?;
        install_listener(&input, &mut listeners, "compositionend", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            let input = input.clone();
            move |event| {
                let event = event.unchecked_into::<CompositionEvent>();
                let normalized = bridge
                    .borrow_mut()
                    .composition_ended(event.data().unwrap_or_default());
                if let Some((event, action)) = normalized {
                    events.borrow_mut().push(event);
                    apply_action(&input, action);
                    event_ready();
                }
            }
        })?;
        install_listener(&input, &mut listeners, "beforeinput", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |event| {
                let input_event = event.clone().unchecked_into::<InputEvent>();
                let normalized = {
                    let bridge = bridge.borrow();
                    match input_event.input_type().as_str() {
                        "deleteContentBackward" => bridge.delete_backward(),
                        "deleteContentForward" => bridge.delete_forward(),
                        "deleteWordBackward" => bridge.delete_word_backward(),
                        "deleteWordForward" => bridge.delete_word_forward(),
                        _ => None,
                    }
                };
                if let Some(normalized) = normalized {
                    event.prevent_default();
                    events.borrow_mut().push(normalized);
                    event_ready();
                }
            }
        })?;
        install_listener(&input, &mut listeners, "keydown", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |event| {
                let keyboard = event.clone().unchecked_into::<KeyboardEvent>();
                let normalized = {
                    bridge.borrow_mut().key_pressed(
                        &keyboard.key(),
                        keyboard.shift_key(),
                        keyboard.ctrl_key(),
                        keyboard.alt_key(),
                        keyboard.meta_key(),
                    )
                };
                if let Some(normalized) = normalized {
                    event.prevent_default();
                    events.borrow_mut().push(normalized);
                    event_ready();
                }
            }
        })?;
        install_listener(&input, &mut listeners, "input", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            let input = input.clone();
            move |_| {
                if bridge.borrow().is_composing() {
                    let text = input.value();
                    let selection = dom_selection_in_utf8(&input, &text);
                    let normalized = bridge.borrow().composition_reconciled(text, selection);
                    if let Some(event) = normalized {
                        events.borrow_mut().push(event);
                        event_ready();
                    }
                } else {
                    let normalized = bridge.borrow_mut().input(input.value());
                    if let Some((event, action)) = normalized {
                        events.borrow_mut().push(event);
                        apply_action(&input, action);
                        event_ready();
                    }
                }
            }
        })?;
        install_listener(&input, &mut listeners, "blur", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let event_ready = Rc::clone(&event_ready);
            move |_| {
                let normalized = bridge.borrow_mut().focus_lost();
                if let Some(event) = normalized {
                    events.borrow_mut().push(event);
                    event_ready();
                }
            }
        })?;

        Ok(Self {
            input,
            bridge,
            events,
            surface_origin: Cell::new(Point::ORIGIN),
            logical_area: Cell::new(None),
            listeners,
        })
    }

    /// Returns the session currently owning the hidden textarea.
    pub fn active_session(&self) -> Option<novadraw_editor::DirectTextEditSessionId> {
        self.bridge.borrow().active_session()
    }

    /// Updates the canvas origin in browser client coordinates.
    pub fn set_surface_origin(&self, origin: Point) {
        if !origin.x().is_finite() || !origin.y().is_finite() {
            return;
        }
        self.surface_origin.set(origin);
        if let Some(area) = self.logical_area.get() {
            set_area(&self.input, translated_area(area, origin));
        }
    }

    /// Applies one Editor effect to the hidden textarea.
    pub fn apply_effect(&self, effect: &TextInputEffect) {
        if let Some(action) = self.bridge.borrow_mut().apply_effect(effect) {
            match action {
                WebTextInputAction::Acquire { purpose, area } => {
                    self.logical_area.set(Some(area));
                    apply_action(
                        &self.input,
                        WebTextInputAction::Acquire {
                            purpose,
                            area: translated_area(area, self.surface_origin.get()),
                        },
                    );
                }
                WebTextInputAction::SetArea(area) => {
                    self.logical_area.set(Some(area));
                    apply_action(
                        &self.input,
                        WebTextInputAction::SetArea(translated_area(
                            area,
                            self.surface_origin.get(),
                        )),
                    );
                }
                WebTextInputAction::Release => {
                    self.logical_area.set(None);
                    apply_action(&self.input, WebTextInputAction::Release);
                }
                WebTextInputAction::ClearValue => {
                    apply_action(&self.input, WebTextInputAction::ClearValue);
                }
            }
        }
    }

    /// Drains normalized DOM input accumulated since the previous call.
    pub fn take_events(&self) -> Vec<SessionTextInputEvent> {
        std::mem::take(&mut *self.events.borrow_mut())
    }
}

fn translated_area(area: Rectangle, origin: Point) -> Rectangle {
    Rectangle::new(
        area.x + origin.x(),
        area.y + origin.y(),
        area.width,
        area.height,
    )
}

impl Drop for WebTextInputHost {
    fn drop(&mut self) {
        let target: &web_sys::EventTarget = self.input.as_ref();
        for listener in &self.listeners {
            let _ = target.remove_event_listener_with_callback(
                listener.event,
                listener.callback.as_ref().unchecked_ref(),
            );
        }
        self.input.remove();
    }
}

fn install_listener(
    input: &HtmlTextAreaElement,
    listeners: &mut Vec<DomListener>,
    event: &'static str,
    callback: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
    input.add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())?;
    listeners.push(DomListener { event, callback });
    Ok(())
}

fn apply_action(input: &HtmlTextAreaElement, action: WebTextInputAction) {
    match action {
        WebTextInputAction::Acquire { purpose, area } => {
            let enter_key_hint = match purpose {
                TextInputPurpose::SingleLine => "done",
                TextInputPurpose::Multiline => "enter",
            };
            let _ = input.set_attribute("enterkeyhint", enter_key_hint);
            set_area(input, area);
            input.set_value("");
            let _ = input.set_attribute("data-active", "true");
            let _ = input.focus();
        }
        WebTextInputAction::SetArea(area) => set_area(input, area),
        WebTextInputAction::Release => {
            input.set_value("");
            let _ = input.set_attribute("data-active", "false");
            let _ = input.blur();
        }
        WebTextInputAction::ClearValue => input.set_value(""),
    }
}

fn set_area(input: &HtmlTextAreaElement, area: Rectangle) {
    let style = input.style();
    let _ = style.set_property("left", &format!("{}px", area.x));
    let _ = style.set_property("top", &format!("{}px", area.y));
    let _ = style.set_property("width", &format!("{}px", area.width.max(1.0)));
    let _ = style.set_property("height", &format!("{}px", area.height.max(1.0)));
}

fn dom_selection_in_utf8(
    input: &HtmlTextAreaElement,
    text: &str,
) -> Option<std::ops::Range<usize>> {
    let start = input.selection_start().ok().flatten()? as usize;
    let end = input.selection_end().ok().flatten()? as usize;
    utf16_range_to_utf8(text, start..end).ok()
}
