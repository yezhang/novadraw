//! Browser-owned hidden textarea used by direct text editing.

use std::{cell::RefCell, rc::Rc};

use novadraw::Rectangle;
use novadraw_editor::{SessionTextInputEvent, TextInputEffect, TextInputPurpose};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{CompositionEvent, Document, Event, HtmlTextAreaElement, InputEvent, KeyboardEvent};

use crate::{WebTextInputAction, WebTextInputBridge};

struct DomListener {
    event: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

/// Owns an invisible DOM textarea and queues normalized Editor text-input events.
pub struct WebTextInputHost {
    input: HtmlTextAreaElement,
    bridge: Rc<RefCell<WebTextInputBridge>>,
    events: Rc<RefCell<Vec<SessionTextInputEvent>>>,
    listeners: Vec<DomListener>,
}

impl WebTextInputHost {
    /// Creates and attaches one hidden textarea to the document body.
    pub fn new(document: &Document) -> Result<Self, JsValue> {
        let input = document
            .create_element("textarea")?
            .dyn_into::<HtmlTextAreaElement>()?;
        input.set_attribute("autocomplete", "off")?;
        input.set_attribute("autocorrect", "off")?;
        input.set_attribute("autocapitalize", "off")?;
        input.set_attribute("spellcheck", "false")?;
        input.set_attribute("tabindex", "-1")?;
        let style = input.style();
        style.set_property("position", "fixed")?;
        style.set_property("opacity", "0")?;
        style.set_property("pointer-events", "none")?;
        style.set_property("resize", "none")?;
        style.set_property("overflow", "hidden")?;
        style.set_property("z-index", "-1")?;
        document
            .body()
            .ok_or_else(|| JsValue::from_str("document has no body"))?
            .append_child(&input)?;

        let bridge = Rc::new(RefCell::new(WebTextInputBridge::new()));
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut listeners = Vec::new();

        install_listener(&input, &mut listeners, "compositionstart", {
            let bridge = Rc::clone(&bridge);
            move |_| bridge.borrow_mut().composition_started()
        })?;
        install_listener(&input, &mut listeners, "compositionupdate", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let input = input.clone();
            move |event| {
                let event = event.unchecked_into::<CompositionEvent>();
                let text = event.data().unwrap_or_default();
                let selection = dom_selection_in_utf8(&input, &text);
                if let Some(event) = bridge.borrow_mut().composition_updated(text, selection) {
                    events.borrow_mut().push(event);
                }
            }
        })?;
        install_listener(&input, &mut listeners, "compositionend", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let input = input.clone();
            move |event| {
                let event = event.unchecked_into::<CompositionEvent>();
                if let Some((event, action)) = bridge
                    .borrow_mut()
                    .composition_ended(event.data().unwrap_or_default())
                {
                    events.borrow_mut().push(event);
                    apply_action(&input, action);
                }
            }
        })?;
        install_listener(&input, &mut listeners, "beforeinput", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            move |event| {
                let input_event = event.clone().unchecked_into::<InputEvent>();
                let normalized = match input_event.input_type().as_str() {
                    "deleteContentBackward" => bridge.borrow().delete_backward(),
                    "deleteContentForward" => bridge.borrow().delete_forward(),
                    "deleteWordBackward" => bridge.borrow().delete_word_backward(),
                    "deleteWordForward" => bridge.borrow().delete_word_forward(),
                    _ => None,
                };
                if let Some(normalized) = normalized {
                    event.prevent_default();
                    events.borrow_mut().push(normalized);
                }
            }
        })?;
        install_listener(&input, &mut listeners, "keydown", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            move |event| {
                let keyboard = event.clone().unchecked_into::<KeyboardEvent>();
                let normalized = bridge.borrow_mut().key_pressed(
                    &keyboard.key(),
                    keyboard.shift_key(),
                    keyboard.ctrl_key(),
                    keyboard.alt_key(),
                    keyboard.meta_key(),
                );
                if let Some(normalized) = normalized {
                    event.prevent_default();
                    events.borrow_mut().push(normalized);
                }
            }
        })?;
        install_listener(&input, &mut listeners, "input", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            let input = input.clone();
            move |_| {
                if let Some((event, action)) = bridge.borrow_mut().input(input.value()) {
                    events.borrow_mut().push(event);
                    apply_action(&input, action);
                }
            }
        })?;
        install_listener(&input, &mut listeners, "blur", {
            let bridge = Rc::clone(&bridge);
            let events = Rc::clone(&events);
            move |_| {
                if let Some(event) = bridge.borrow_mut().focus_lost() {
                    events.borrow_mut().push(event);
                }
            }
        })?;

        Ok(Self {
            input,
            bridge,
            events,
            listeners,
        })
    }

    /// Applies one Editor effect to the hidden textarea.
    pub fn apply_effect(&self, effect: &TextInputEffect) {
        if let Some(action) = self.bridge.borrow_mut().apply_effect(effect) {
            apply_action(&self.input, action);
        }
    }

    /// Drains normalized DOM input accumulated since the previous call.
    pub fn take_events(&self) -> Vec<SessionTextInputEvent> {
        std::mem::take(&mut *self.events.borrow_mut())
    }
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
            let _ = input.focus();
        }
        WebTextInputAction::SetArea(area) => set_area(input, area),
        WebTextInputAction::Release => {
            input.set_value("");
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
    utf16_range_to_utf8(text, start, end)
}

fn utf16_range_to_utf8(text: &str, start: usize, end: usize) -> Option<std::ops::Range<usize>> {
    if start > end {
        return None;
    }
    let start = utf16_offset_to_utf8(text, start)?;
    let end = utf16_offset_to_utf8(text, end)?;
    Some(start..end)
}

fn utf16_offset_to_utf8(text: &str, target: usize) -> Option<usize> {
    let mut utf16 = 0;
    for (utf8, character) in text.char_indices() {
        if utf16 == target {
            return Some(utf8);
        }
        utf16 += character.len_utf16();
        if utf16 > target {
            return None;
        }
    }
    (utf16 == target).then_some(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_dom_selection_converts_to_utf8_without_splitting_surrogates() {
        assert_eq!(utf16_range_to_utf8("a😀中", 1, 3), Some(1..5));
        assert_eq!(utf16_range_to_utf8("a😀中", 2, 3), None);
        assert_eq!(utf16_range_to_utf8("a😀中", 3, 4), Some(5..8));
    }
}
