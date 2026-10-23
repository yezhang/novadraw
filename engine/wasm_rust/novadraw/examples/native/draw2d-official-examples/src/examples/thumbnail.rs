//! One live producer, a main input view and an independently sized read-only overview.
use std::sync::{Arc, Mutex};

use novadraw::{
    event::{EventContext, FigureEventHandler, MouseButton, MouseEvent},
    figure::{
        Figure, FigureCapabilityBuilder, FigureCapabilityRegistrationError, FigurePreparation,
        FigurePresentation, INPUT, InputCapability, MeasureConstraints, PREPARATION,
        PreparationCapability, PreparationMeasureError, PreparationMeasurement, PreparationScene,
        RectangleFigure,
    },
    geometry::Rectangle,
    graphics::Color,
    runtime::Runtime,
    scene::{
        ASPECT_FIT_VIEW, AspectFitView, SCENE_VIEW, SOURCE_LOCAL_CAPTURE, SceneBindingGroup,
        SceneError, SceneEvent, SceneHandoff, SceneSubscriptionId, SceneViewBindingId,
        SceneViewCapability, SceneViewConsumer, SceneViewFigure, SceneViewHandle,
        SourceLocalCapture,
    },
    text::{MeasureContext, TextError},
    tree::FigureId,
};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

use super::{
    connection_and_anchor::DraggableRectangle, runtime_with_fonts, zoom_example::GroupBoxBorder,
};

const SOURCE_SIZE: f64 = 600.0;
const MAIN_X: f64 = 20.0;
const MAIN_Y: f64 = 20.0;
const OVERVIEW_X: f64 = 650.0;
const OVERVIEW_Y: f64 = 20.0;

#[derive(Clone, Copy)]
enum Input {
    Press(f64, f64, MouseButton),
    Move(f64, f64),
    Release(f64, f64, MouseButton),
    Exit,
}

// Host input transport contains values only. It never exposes the source Runtime to a Figure.
struct MainView {
    view: SceneViewFigure,
    input: Arc<Mutex<Vec<Input>>>,
}
impl SceneViewConsumer for MainView {
    fn scene_view(&self) -> &SceneViewHandle {
        self.view.scene_view()
    }
}
impl Figure for MainView {
    fn name(&self) -> &'static str {
        "ThumbnailSourceView"
    }
    fn initial_bounds(&self) -> Rectangle {
        Rectangle::new(MAIN_X, MAIN_Y, SOURCE_SIZE, SOURCE_SIZE)
    }
    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        out.register(SCENE_VIEW, SceneViewCapability::of::<Self>())?;
        out.register(PREPARATION, PreparationCapability::of::<Self>())?;
        out.register(INPUT, InputCapability::of::<Self>())
    }
}
impl FigurePreparation for MainView {
    fn revision(&self) -> u64 {
        FigurePreparation::revision(&self.view)
    }
    fn measure_scene(
        &self,
        context: &mut MeasureContext<'_>,
        constraints: MeasureConstraints,
        scene: &PreparationScene<'_>,
    ) -> Result<PreparationMeasurement, PreparationMeasureError> {
        self.view.measure_scene(context, constraints, scene)
    }
    fn prepare(
        &self,
        context: &mut MeasureContext<'_>,
        client: Rectangle,
    ) -> Result<FigurePresentation, TextError> {
        self.view.prepare(context, client)
    }
}
impl FigureEventHandler for MainView {
    fn on_mouse_pressed(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.input
            .lock()
            .expect("input")
            .push(Input::Press(event.x, event.y, event.button));
        true
    }
    fn on_mouse_dragged(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.input
            .lock()
            .expect("input")
            .push(Input::Move(event.x, event.y));
        true
    }
    fn on_mouse_released(&self, event: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.input
            .lock()
            .expect("input")
            .push(Input::Release(event.x, event.y, event.button));
        true
    }
    fn on_mouse_exited(&self, _: &MouseEvent, _: &mut EventContext<'_>) -> bool {
        self.input.lock().expect("input").push(Input::Exit);
        true
    }
}

struct ThumbnailHost {
    source: Runtime,
    subscription: SceneSubscriptionId,
    handoff: SceneHandoff,
    bindings: Vec<SceneViewBindingId>,
    overview: FigureId,
    input: Arc<Mutex<Vec<Input>>>,
    active: bool,
}
impl ThumbnailHost {
    fn drive(&mut self, target: &mut Runtime) -> Result<(), SceneError> {
        let input = std::mem::take(&mut *self.input.lock().expect("input"));
        for event in input {
            match event {
                Input::Press(x, y, button) => {
                    self.source.input().dispatch_mouse_pressed(x, y, button);
                }
                Input::Move(x, y) => {
                    self.source.input().dispatch_mouse_moved(x, y);
                }
                Input::Release(x, y, button) => {
                    self.source.input().dispatch_mouse_released(x, y, button);
                }
                Input::Exit => self.source.input().pointer_exited(),
            }
        }
        if self.active && self.source.scenes().needs_capture(self.subscription)? {
            // Capture errors are delivered through the same handoff and retain the accepted image.
            let _ = self.source.scenes().capture(self.subscription);
        }
        while let Some(event) = self.handoff.take()? {
            match event {
                SceneEvent::Ready(snapshot) => {
                    for &binding in &self.bindings {
                        target.scenes().install_view(binding, snapshot.clone())?;
                    }
                }
                SceneEvent::Closed(closed) => {
                    for binding in self.bindings.drain(..) {
                        target.scenes().close_view(binding, closed)?;
                    }
                }
                SceneEvent::Error(error) => return Err(error),
            }
        }
        Ok(())
    }
}

fn build() -> (Runtime, ThumbnailHost) {
    let group = SceneBindingGroup::default();
    let mut source = runtime_with_fonts();
    source.scenes().join_group(group.clone()).expect("group");
    let root = source
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, SOURCE_SIZE, SOURCE_SIZE, Color::WHITE)
                .with_border(novadraw::figure::border::LineBorder::new(Color::BLACK, 1.0)),
        ))
        .expect("source");
    for (bounds, color) in [
        (
            Rectangle::new(50.0, 50.0, 200.0, 200.0),
            Color::rgba(0.0, 1.0, 0.0, 1.0),
        ),
        (
            Rectangle::new(350.0, 350.0, 150.0, 200.0),
            Color::rgba(0.0, 0.0, 1.0, 1.0),
        ),
    ] {
        source
            .figure(root)
            .expect("source")
            .add(Box::new(DraggableRectangle::new(bounds, color)))
            .expect("rectangle");
    }
    let source_id = source.scenes().register_source(root).expect("source");
    let (subscription, config, handoff) = source
        .scenes()
        .subscribe(
            source_id,
            SOURCE_LOCAL_CAPTURE,
            SourceLocalCapture::default(),
        )
        .expect("subscription");
    let mut target = runtime_with_fonts();
    target.scenes().join_group(group).expect("group");
    let target_root = target
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            1200.0,
            800.0,
            Color::rgba(0.94, 0.94, 0.94, 1.0),
        )))
        .expect("target");
    let input = Arc::new(Mutex::new(Vec::new()));
    let main_handle = target
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .expect("view");
    let main = target
        .figure(target_root)
        .expect("root")
        .add(Box::new(MainView {
            view: SceneViewFigure::new(main_handle),
            input: input.clone(),
        }))
        .expect("main");
    let handle = target
        .scenes()
        .create_view(ASPECT_FIT_VIEW, AspectFitView::default())
        .expect("view");
    let title = target
        .text()
        .layout(
            "Overview Figure",
            &novadraw::text::FontDescriptor::parse("12px Inter Variable").expect("font"),
            novadraw::text::TextConstraints::UNBOUNDED,
        )
        .expect("title");
    let overview = target
        .figure(target_root)
        .expect("root")
        .add(Box::new(
            SceneViewFigure::new(handle)
                .with_bounds(Rectangle::new(OVERVIEW_X, OVERVIEW_Y, 200.0, 200.0))
                .with_border(GroupBoxBorder { title }),
        ))
        .expect("overview");
    let bindings = [main, overview]
        .into_iter()
        .map(|id| {
            target
                .scenes()
                .bind_view(id, source_id, subscription, config)
                .expect("binding")
        })
        .collect();
    let mut host = ThumbnailHost {
        source,
        subscription,
        handoff,
        bindings,
        overview,
        input,
        active: true,
    };
    host.drive(&mut target).expect("initial capture");
    (target, host)
}

pub fn thumbnail_example() -> Runtime {
    // Single-Runtime callers receive the live source. The Native panel entry owns
    // both Runtimes and is the shared-view consumer.
    build().1.source
}

pub fn panel_scene() -> PanelScene {
    let (target, mut host) = build();
    let mut width = 200.0;
    let mut height = 200.0;
    (
        target,
        Some(Panel::new([650.0, 260.0], move |ui, target| {
            ui.label("Drag either rectangle in the source view.");
            let changed = ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(0.0..=500.0)
                        .prefix("width "),
                )
                .changed()
                | ui.add(
                    egui::DragValue::new(&mut height)
                        .range(0.0..=500.0)
                        .prefix("height "),
                )
                .changed();
            if changed {
                target
                    .figure(host.overview)
                    .expect("overview")
                    .set_bounds(Rectangle::new(OVERVIEW_X, OVERVIEW_Y, width, height))
                    .expect("size");
            }
            if host.active && ui.button("Deactivate source").clicked() {
                host.source
                    .scenes()
                    .cancel(host.subscription)
                    .expect("cancel");
                host.active = false;
            }
            if let Err(error) = host.drive(target) {
                ui.label(error.to_string());
            }
        })),
    )
}

pub fn verification_entries() -> Vec<novadraw_example_support::panel::PanelSceneEntry> {
    [
        "ThumbnailInitial",
        "ThumbnailDraggedWide",
        "ThumbnailClosed",
    ]
    .into_iter()
    .enumerate()
    .map(|(stage, name)| {
        (
            name,
            Box::new(move || {
                let (mut target, mut host) = build();
                if stage == 1 {
                    host.input.lock().expect("input").extend([
                        Input::Press(100.0, 100.0, MouseButton::Left),
                        Input::Move(180.0, 130.0),
                        Input::Release(180.0, 130.0, MouseButton::Left),
                    ]);
                    target
                        .figure(host.overview)
                        .expect("overview")
                        .set_bounds(Rectangle::new(OVERVIEW_X, OVERVIEW_Y, 450.0, 120.0))
                        .expect("size");
                } else if stage == 2 {
                    host.source
                        .scenes()
                        .cancel(host.subscription)
                        .expect("cancel");
                    host.active = false;
                }
                host.drive(&mut target).expect("drive");
                (
                    target,
                    Some(Panel::new([650.0, 300.0], move |ui, target| {
                        ui.label(name);
                        host.drive(target).expect("drive");
                    })),
                )
            }) as Box<dyn FnMut() -> PanelScene>,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::render::{BackendCapabilities, RenderCommandKind, SurfaceInfo};

    #[test]
    fn one_source_drag_updates_both_views_and_resize_does_not_recapture() {
        let (mut target, mut host) = build();
        let before = target
            .scenes()
            .installed_view(host.bindings[0])
            .unwrap()
            .unwrap();
        let source_root = host.source.tree().contents().unwrap();
        let green = host.source.tree().child_order(source_root).unwrap()[0];
        target
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: 1200.0,
                    logical_height: 800.0,
                    pixel_width: 1200,
                    pixel_height: 800,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL,
            )
            .into_ready()
            .unwrap();
        target
            .input()
            .dispatch_mouse_pressed(MAIN_X + 100.0, MAIN_Y + 100.0, MouseButton::Left);
        target
            .input()
            .dispatch_mouse_moved(MAIN_X + 140.0, MAIN_Y + 120.0);
        target
            .input()
            .dispatch_mouse_released(MAIN_X + 140.0, MAIN_Y + 120.0, MouseButton::Left);
        host.drive(&mut target).unwrap();
        assert_eq!(
            host.source.tree().figure_bounds(green).unwrap(),
            Rectangle::new(90.0, 70.0, 200.0, 200.0)
        );
        let a = target
            .scenes()
            .installed_view(host.bindings[0])
            .unwrap()
            .unwrap();
        let b = target
            .scenes()
            .installed_view(host.bindings[1])
            .unwrap()
            .unwrap();
        assert_eq!(a.snapshot(), b.snapshot());
        assert!(a.snapshot().stable_source_epoch() > before.snapshot().stable_source_epoch());
        target
            .figure(host.overview)
            .unwrap()
            .set_bounds(Rectangle::new(OVERVIEW_X, OVERVIEW_Y, 320.0, 100.0))
            .unwrap();
        host.drive(&mut target).unwrap();
        assert_eq!(
            a.snapshot(),
            target
                .scenes()
                .installed_view(host.bindings[1])
                .unwrap()
                .unwrap()
                .snapshot()
        );
        assert!(
            !host
                .source
                .scenes()
                .needs_capture(host.subscription)
                .unwrap()
        );
        assert!(
            a.drawing()
                .commands()
                .iter()
                .any(|c| matches!(c.kind, RenderCommandKind::FillRect { .. }))
        );
        host.source.scenes().cancel(host.subscription).unwrap();
        host.active = false;
        host.drive(&mut target).unwrap();
        assert!(host.bindings.is_empty());
    }
}
