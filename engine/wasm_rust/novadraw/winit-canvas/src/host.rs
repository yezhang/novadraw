use crate::geometry::{CanvasError, CanvasRect, WindowMetrics};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};
use winit::{
    dpi::{LogicalPosition, PhysicalPosition},
    event::{
        DeviceId, ElementState, Ime, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, Touch,
        TouchPhase, WindowEvent,
    },
    window::WindowId,
};

/// Opaque host-scoped identity. Removed identities are never reused.
#[derive(Clone, Debug)]
pub struct CanvasId {
    owner: Rc<()>,
    serial: u64,
}
impl PartialEq for CanvasId {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.owner, &other.owner) && self.serial == other.serial
    }
}
impl Eq for CanvasId {}

#[derive(Clone, Debug)]
pub enum CanvasEvent {
    Geometry {
        rect: CanvasRect,
        metrics: WindowMetrics,
    },
    PointerEntered,
    PointerLeft,
    PointerMoved {
        position: LogicalPosition<f64>,
    },
    MouseInput {
        position: LogicalPosition<f64>,
        button: MouseButton,
        state: ElementState,
    },
    Wheel {
        position: LogicalPosition<f64>,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },
    Pinch {
        position: LogicalPosition<f64>,
        delta: f64,
        phase: TouchPhase,
    },
    Touch {
        touch: Touch,
        position: LogicalPosition<f64>,
    },
    Keyboard(KeyEvent),
    Modifiers(Modifiers),
    Ime(Ime),
    Focused(bool),
    Cancelled,
    Suspended,
    Resumed,
    Removed,
}

#[derive(Clone, Debug)]
pub struct Delivery {
    pub canvas: CanvasId,
    pub event: CanvasEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedrawToken {
    revision: u64,
    canvas: Option<CanvasId>,
    owner: CanvasId,
}

#[derive(Clone, Debug)]
pub struct Canvas {
    id: CanvasId,
    rect: CanvasRect,
    visible: bool,
    enabled: bool,
    revision: u64,
    rendered: u64,
}
impl Canvas {
    pub fn id(&self) -> CanvasId {
        self.id.clone()
    }
    pub fn rect(&self) -> CanvasRect {
        self.rect
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn redraw_token(&self) -> Option<RedrawToken> {
        (self.revision != self.rendered).then_some(RedrawToken {
            revision: self.revision,
            canvas: Some(self.id.clone()),
            owner: self.id.clone(),
        })
    }
}

/// The caller keeps this host beside its Window and handles the returned delivery batches.
/// The host does not invoke callbacks while mutating routing state.
pub struct CanvasHost {
    window: WindowId,
    owner: Rc<()>,
    next_id: u64,
    metrics: WindowMetrics,
    canvases: Vec<Canvas>,
    pointer: Option<PhysicalPosition<f64>>,
    hovered: Option<CanvasId>,
    focused: Option<CanvasId>,
    capture: Option<CanvasId>,
    buttons: HashSet<MouseButton>,
    touches: Vec<(DeviceId, u64, Option<CanvasId>)>,
    gestures: HashMap<(DeviceId, bool), Option<CanvasId>>,
    suspended: bool,
    window_focused: bool,
    modifiers: Modifiers,
    composite_revision: u64,
    composite_rendered: u64,
}

impl CanvasHost {
    pub fn new(window: WindowId, metrics: WindowMetrics) -> Self {
        Self {
            window,
            owner: Rc::new(()),
            next_id: 1,
            metrics,
            canvases: Vec::new(),
            pointer: None,
            hovered: None,
            focused: None,
            capture: None,
            buttons: HashSet::new(),
            touches: Vec::new(),
            gestures: HashMap::new(),
            suspended: false,
            window_focused: true,
            composite_revision: 1,
            composite_rendered: 0,
            modifiers: Modifiers::default(),
        }
    }
    pub fn metrics(&self) -> WindowMetrics {
        self.metrics
    }
    pub fn canvases(&self) -> &[Canvas] {
        &self.canvases
    }
    pub fn canvas(&self, id: &CanvasId) -> Result<&Canvas, CanvasError> {
        self.canvases
            .iter()
            .find(|c| &c.id == id)
            .ok_or(CanvasError::UnknownCanvas)
    }
    fn canvas_mut(&mut self, id: &CanvasId) -> Result<&mut Canvas, CanvasError> {
        self.canvases
            .iter_mut()
            .find(|c| &c.id == id)
            .ok_or(CanvasError::UnknownCanvas)
    }
    fn next_revision(&self) -> Result<u64, CanvasError> {
        self.composite_revision
            .checked_add(1)
            .ok_or(CanvasError::RevisionExhausted)
    }
    pub fn insert(&mut self, rect: CanvasRect) -> Result<CanvasId, CanvasError> {
        self.metrics.texture_size(rect)?;
        let next = self
            .next_id
            .checked_add(1)
            .ok_or(CanvasError::IdentityExhausted)?;
        let revision = self.next_revision()?;
        let id = CanvasId {
            owner: self.owner.clone(),
            serial: self.next_id,
        };
        self.next_id = next;
        self.composite_revision = revision;
        self.canvases.push(Canvas {
            id: id.clone(),
            rect,
            visible: true,
            enabled: true,
            revision,
            rendered: 0,
        });
        Ok(id)
    }
    pub fn set_rect(
        &mut self,
        id: &CanvasId,
        rect: CanvasRect,
    ) -> Result<Vec<Delivery>, CanvasError> {
        self.metrics.texture_size(rect)?;
        if self.canvas(id)?.rect == rect {
            return Ok(Vec::new());
        }
        let revision = self.next_revision()?;
        let canvas = self.canvas_mut(id)?;
        canvas.rect = rect;
        canvas.revision = revision;
        self.composite_revision = revision;
        let mut deliveries = vec![Delivery {
            canvas: id.clone(),
            event: CanvasEvent::Geometry {
                rect,
                metrics: self.metrics,
            },
        }];
        self.update_hover(&mut deliveries)?;
        Ok(deliveries)
    }
    pub fn set_available(
        &mut self,
        id: &CanvasId,
        visible: bool,
        enabled: bool,
    ) -> Result<Vec<Delivery>, CanvasError> {
        let canvas = self.canvas(id)?;
        if canvas.visible == visible && canvas.enabled == enabled {
            return Ok(Vec::new());
        }
        let revision = self.next_revision()?;
        let mut deliveries = Vec::new();
        if !visible || !enabled {
            self.cancel_canvas(id, &mut deliveries);
        }
        let canvas = self.canvas_mut(id)?;
        canvas.visible = visible;
        canvas.enabled = enabled;
        canvas.revision = revision;
        self.composite_revision = revision;
        self.update_hover(&mut deliveries)?;
        Ok(deliveries)
    }
    pub fn remove(&mut self, id: &CanvasId) -> Result<Vec<Delivery>, CanvasError> {
        self.canvas(id)?;
        let revision = self.next_revision()?;
        let mut deliveries = Vec::new();
        self.cancel_canvas(id, &mut deliveries);
        self.canvases.retain(|c| &c.id != id);
        self.composite_revision = revision;
        deliveries.push(Delivery {
            canvas: id.clone(),
            event: CanvasEvent::Removed,
        });
        self.update_hover(&mut deliveries)?;
        Ok(deliveries)
    }
    pub fn request_redraw(&mut self, id: &CanvasId) -> Result<(), CanvasError> {
        self.canvas(id)?;
        let revision = self.next_revision()?;
        self.canvas_mut(id)?.revision = revision;
        self.composite_revision = revision;
        Ok(())
    }
    pub fn acknowledge_canvas(
        &mut self,
        id: &CanvasId,
        token: RedrawToken,
    ) -> Result<(), CanvasError> {
        let canvas = self.canvas_mut(id)?;
        if token.canvas.as_ref() != Some(id) || token.revision > canvas.revision {
            return Err(CanvasError::InvalidRedrawToken);
        }
        canvas.rendered = canvas.rendered.max(token.revision);
        Ok(())
    }
    pub fn composite_token(&self) -> RedrawToken {
        RedrawToken {
            revision: self.composite_revision,
            canvas: None,
            owner: CanvasId {
                owner: self.owner.clone(),
                serial: 0,
            },
        }
    }
    pub fn acknowledge_composite(&mut self, token: RedrawToken) -> Result<(), CanvasError> {
        if token.canvas.is_some()
            || !Rc::ptr_eq(&token.owner.owner, &self.owner)
            || token.revision > self.composite_revision
        {
            return Err(CanvasError::InvalidRedrawToken);
        }
        self.composite_rendered = self.composite_rendered.max(token.revision);
        Ok(())
    }
    pub fn needs_redraw(&self) -> bool {
        !self.suspended
            && (self.composite_revision != self.composite_rendered
                || self
                    .canvases
                    .iter()
                    .any(|c| c.visible && !c.rect.is_empty() && c.redraw_token().is_some()))
    }
    pub fn captured(&self) -> Option<CanvasId> {
        self.capture.clone()
    }
    pub fn focused(&self) -> Option<CanvasId> {
        self.focused.clone()
    }
    pub fn set_focus(&mut self, id: Option<&CanvasId>) -> Result<Vec<Delivery>, CanvasError> {
        if let Some(id) = id {
            let canvas = self.canvas(id)?;
            if self.suspended || !self.window_focused || !canvas.visible || !canvas.enabled {
                return Err(CanvasError::Unavailable);
            }
        }
        let mut deliveries = Vec::new();
        self.change_focus(id.cloned(), &mut deliveries);
        Ok(deliveries)
    }
    pub fn set_metrics(&mut self, metrics: WindowMetrics) -> Result<Vec<Delivery>, CanvasError> {
        if self.metrics == metrics {
            return Ok(Vec::new());
        }
        for canvas in &self.canvases {
            metrics.texture_size(canvas.rect)?;
        }
        let revision = self.next_revision()?;
        self.metrics = metrics;
        self.composite_revision = revision;
        let mut deliveries = Vec::new();
        if metrics.pixels().width == 0 || metrics.pixels().height == 0 {
            self.cancel_all(&mut deliveries);
            self.pointer = None;
        }
        for canvas in &mut self.canvases {
            canvas.revision = revision;
            deliveries.push(Delivery {
                canvas: canvas.id.clone(),
                event: CanvasEvent::Geometry {
                    rect: canvas.rect,
                    metrics,
                },
            });
        }
        self.update_hover(&mut deliveries)?;
        Ok(deliveries)
    }
    pub fn suspend(&mut self) -> Vec<Delivery> {
        let mut deliveries = Vec::new();
        self.cancel_all(&mut deliveries);
        self.pointer = None;
        self.suspended = true;
        for canvas in &self.canvases {
            deliveries.push(Delivery {
                canvas: canvas.id.clone(),
                event: CanvasEvent::Suspended,
            });
        }
        deliveries
    }
    pub fn resume(&mut self, metrics: WindowMetrics) -> Result<Vec<Delivery>, CanvasError> {
        for canvas in &self.canvases {
            metrics.texture_size(canvas.rect)?;
        }
        let revision = self.next_revision()?;
        self.metrics = metrics;
        self.suspended = false;
        self.composite_revision = revision;
        let mut deliveries = Vec::new();
        for canvas in &mut self.canvases {
            canvas.revision = revision;
            deliveries.push(Delivery {
                canvas: canvas.id.clone(),
                event: CanvasEvent::Resumed,
            });
            deliveries.push(Delivery {
                canvas: canvas.id.clone(),
                event: CanvasEvent::Geometry {
                    rect: canvas.rect,
                    metrics,
                },
            });
        }
        Ok(deliveries)
    }
    fn hit(&self, point: LogicalPosition<f64>) -> Option<CanvasId> {
        if self.suspended {
            return None;
        }
        let pixels = self.metrics.pixels();
        let scale = self.metrics.scale_factor();
        if point.x < 0.0
            || point.y < 0.0
            || point.x >= f64::from(pixels.width) / scale
            || point.y >= f64::from(pixels.height) / scale
        {
            return None;
        }
        self.canvases
            .iter()
            .rev()
            .find(|c| c.visible && c.enabled && c.rect.contains(point))
            .map(Canvas::id)
    }
    fn pointer_point(&self) -> Result<Option<LogicalPosition<f64>>, CanvasError> {
        self.pointer
            .map(|p| self.metrics.physical_to_logical(p))
            .transpose()
    }
    fn pointer_target(&self, point: LogicalPosition<f64>) -> Option<CanvasId> {
        if self.buttons.is_empty() {
            self.hit(point)
        } else {
            self.capture.clone()
        }
    }
    fn update_hover(&mut self, deliveries: &mut Vec<Delivery>) -> Result<(), CanvasError> {
        let target = self
            .pointer_point()?
            .and_then(|point| self.pointer_target(point));
        if target != self.hovered {
            if let Some(old) = self.hovered.take() {
                deliveries.push(Delivery {
                    canvas: old,
                    event: CanvasEvent::PointerLeft,
                });
            }
            if let Some(new) = target.clone() {
                deliveries.push(Delivery {
                    canvas: new,
                    event: CanvasEvent::PointerEntered,
                });
            }
            self.hovered = target;
        }
        Ok(())
    }
    fn change_focus(&mut self, id: Option<CanvasId>, deliveries: &mut Vec<Delivery>) {
        if self.focused != id {
            if let Some(old) = self.focused.take() {
                deliveries.push(Delivery {
                    canvas: old,
                    event: CanvasEvent::Focused(false),
                });
            }
            if let Some(new) = id.clone() {
                deliveries.push(Delivery {
                    canvas: new.clone(),
                    event: CanvasEvent::Focused(true),
                });
                deliveries.push(Delivery {
                    canvas: new,
                    event: CanvasEvent::Modifiers(self.modifiers),
                });
            }
            self.focused = id;
        }
    }
    fn cancel_canvas(&mut self, id: &CanvasId, deliveries: &mut Vec<Delivery>) {
        if self.capture.as_ref() == Some(id) {
            self.capture = None;
            deliveries.push(Delivery {
                canvas: id.clone(),
                event: CanvasEvent::Cancelled,
            });
        }
        for (_, _, target) in &mut self.touches {
            if target.as_ref() == Some(id) {
                *target = None;
                deliveries.push(Delivery {
                    canvas: id.clone(),
                    event: CanvasEvent::Cancelled,
                });
            }
        }
        for target in self.gestures.values_mut() {
            if target.as_ref() == Some(id) {
                *target = None;
                deliveries.push(Delivery {
                    canvas: id.clone(),
                    event: CanvasEvent::Cancelled,
                });
            }
        }
        if self.hovered.as_ref() == Some(id) {
            self.hovered = None;
            deliveries.push(Delivery {
                canvas: id.clone(),
                event: CanvasEvent::PointerLeft,
            });
        }
        if self.focused.as_ref() == Some(id) {
            self.change_focus(None, deliveries);
        }
    }
    fn cancel_all(&mut self, deliveries: &mut Vec<Delivery>) {
        for id in self.canvases.iter().map(Canvas::id).collect::<Vec<_>>() {
            self.cancel_canvas(&id, deliveries);
        }
        self.buttons.clear();
        self.touches.clear();
        self.gestures.clear();
    }
    fn gesture_target(
        &mut self,
        device: DeviceId,
        pinch: bool,
        phase: TouchPhase,
        point: LogicalPosition<f64>,
    ) -> Option<CanvasId> {
        let key = (device, pinch);
        if phase == TouchPhase::Started || !self.gestures.contains_key(&key) {
            let target = self.pointer_target(point);
            self.gestures.insert(key, target);
        }
        let target = self.gestures.get(&key).cloned().flatten();
        if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            self.gestures.remove(&key);
        }
        target
    }
    /// Handle only events belonging to this host's window.
    /// After ScaleFactorChanged, the application also calls set_metrics with Window::inner_size.
    pub fn handle_window_event(
        &mut self,
        window: WindowId,
        event: &WindowEvent,
    ) -> Result<Vec<Delivery>, CanvasError> {
        if window != self.window {
            return Ok(Vec::new());
        }
        let mut deliveries = Vec::new();
        match event {
            WindowEvent::Resized(pixels) => {
                return self.set_metrics(WindowMetrics::new(*pixels, self.metrics.scale_factor())?);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                return self.set_metrics(WindowMetrics::new(self.metrics.pixels(), *scale_factor)?);
            }
            WindowEvent::Focused(false) => {
                self.window_focused = false;
                self.modifiers = Modifiers::default();
                self.cancel_all(&mut deliveries);
                self.pointer = None;
            }
            WindowEvent::Focused(true) => {
                self.window_focused = true;
            }
            _ if self.suspended || !self.window_focused => {}
            WindowEvent::CursorMoved { position, .. } => {
                let point = self.metrics.physical_to_logical(*position)?;
                self.pointer = Some(*position);
                self.update_hover(&mut deliveries)?;
                if let Some(id) = self.pointer_target(point) {
                    let position = self.canvas(&id)?.rect.window_to_local(point);
                    deliveries.push(Delivery {
                        canvas: id,
                        event: CanvasEvent::PointerMoved { position },
                    });
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                if let Some(id) = self.hovered.take() {
                    deliveries.push(Delivery {
                        canvas: id,
                        event: CanvasEvent::PointerLeft,
                    });
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                let point = self.pointer_point()?;
                if *state == ElementState::Pressed && self.buttons.is_empty() {
                    self.capture = point.and_then(|p| self.hit(p));
                    self.change_focus(self.capture.clone(), &mut deliveries);
                }
                let target = self.capture.clone();
                match state {
                    ElementState::Pressed => {
                        self.buttons.insert(*button);
                    }
                    ElementState::Released => {
                        self.buttons.remove(button);
                    }
                }
                if let (Some(id), Some(point)) = (target, point) {
                    let position = self.canvas(&id)?.rect.window_to_local(point);
                    deliveries.push(Delivery {
                        canvas: id,
                        event: CanvasEvent::MouseInput {
                            position,
                            button: *button,
                            state: *state,
                        },
                    });
                } else if *state == ElementState::Released
                    && self.buttons.is_empty()
                    && let Some(id) = &self.capture
                {
                    deliveries.push(Delivery {
                        canvas: id.clone(),
                        event: CanvasEvent::Cancelled,
                    });
                }
                if self.buttons.is_empty() {
                    self.capture = None;
                    self.update_hover(&mut deliveries)?;
                }
            }
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
                ..
            } => {
                if let Some(point) = self.pointer_point()? {
                    let target = if matches!(delta, MouseScrollDelta::LineDelta(..)) {
                        self.pointer_target(point)
                    } else {
                        self.gesture_target(*device_id, false, *phase, point)
                    };
                    if let Some(id) = target {
                        let position = self.canvas(&id)?.rect.window_to_local(point);
                        let delta = match delta {
                            MouseScrollDelta::PixelDelta(p) => {
                                let p = self.metrics.physical_to_logical(*p)?;
                                MouseScrollDelta::PixelDelta(PhysicalPosition::new(p.x, p.y))
                            }
                            lines => *lines,
                        };
                        deliveries.push(Delivery {
                            canvas: id,
                            event: CanvasEvent::Wheel {
                                position,
                                delta,
                                phase: *phase,
                            },
                        });
                    }
                }
                if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    self.gestures.remove(&(*device_id, false));
                }
            }
            WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => {
                if !delta.is_finite() {
                    return Err(CanvasError::InvalidGeometry);
                }
                if let Some(point) = self.pointer_point()?
                    && let Some(id) = self.gesture_target(*device_id, true, *phase, point)
                {
                    let position = self.canvas(&id)?.rect.window_to_local(point);
                    deliveries.push(Delivery {
                        canvas: id,
                        event: CanvasEvent::Pinch {
                            position,
                            delta: *delta,
                            phase: *phase,
                        },
                    });
                }
                if matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    self.gestures.remove(&(*device_id, true));
                }
            }
            WindowEvent::Touch(touch) => {
                let point = self.metrics.physical_to_logical(touch.location)?;
                if touch.phase == TouchPhase::Started {
                    let id = self.hit(point);
                    self.change_focus(id.clone(), &mut deliveries);
                    self.touches
                        .retain(|(device, key, _)| *device != touch.device_id || *key != touch.id);
                    self.touches.push((touch.device_id, touch.id, id));
                }
                if let Some(id) = self
                    .touches
                    .iter()
                    .find(|(device, key, _)| *device == touch.device_id && *key == touch.id)
                    .and_then(|(_, _, id)| id.clone())
                {
                    let position = self.canvas(&id)?.rect.window_to_local(point);
                    deliveries.push(Delivery {
                        canvas: id,
                        event: CanvasEvent::Touch {
                            touch: *touch,
                            position,
                        },
                    });
                }
                if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    self.touches
                        .retain(|(device, key, _)| *device != touch.device_id || *key != touch.id);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(id) = &self.focused {
                    deliveries.push(Delivery {
                        canvas: id.clone(),
                        event: CanvasEvent::Keyboard(event.clone()),
                    });
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = *modifiers;
                if let Some(id) = &self.focused {
                    deliveries.push(Delivery {
                        canvas: id.clone(),
                        event: CanvasEvent::Modifiers(*modifiers),
                    });
                }
            }
            WindowEvent::Ime(ime) => {
                if let Some(id) = &self.focused {
                    deliveries.push(Delivery {
                        canvas: id.clone(),
                        event: CanvasEvent::Ime(ime.clone()),
                    });
                }
            }
            _ => {}
        }
        Ok(deliveries)
    }
}
