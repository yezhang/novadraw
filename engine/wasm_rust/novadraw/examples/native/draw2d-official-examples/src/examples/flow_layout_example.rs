#[cfg(test)]
use novadraw::{
    event::EventContext,
    figure::{FigureStyle, LabelFigure},
};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use novadraw::{
    figure::{
        EllipseFigure, RectangleFigure, RoundedRectangleFigure, TriangleFigure, border::LineBorder,
    },
    geometry::{Dimension, Rectangle},
    graphics::Color,
    layout::{FlowAlignment, FlowDirection, FlowLayout},
    runtime::Runtime,
    tree::FigureId,
};

#[cfg(test)]
use crate::controls::{add_checkbox, add_radio, add_scale};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

use super::runtime_with_fonts;

const WIDTH: f64 = 920.0;
const HEIGHT: f64 = 620.0;
const CONTENT_WIDTH: f64 = 620.0;
const CONTENT_HEIGHT: f64 = 580.0;
const CONTROL_X: f64 = 640.0;
#[cfg(test)]
const CONTROL_WIDTH: f64 = 250.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlowSettings {
    horizontal: bool,
    stretch_minor_axis: bool,
    major_alignment: FlowAlignment,
    minor_alignment: FlowAlignment,
    major_spacing: f64,
    minor_spacing: f64,
}

impl Default for FlowSettings {
    fn default() -> Self {
        Self {
            horizontal: true,
            stretch_minor_axis: false,
            major_alignment: FlowAlignment::Start,
            minor_alignment: FlowAlignment::Start,
            major_spacing: 5.0,
            minor_spacing: 5.0,
        }
    }
}

impl FlowSettings {
    fn layout(self) -> FlowLayout {
        FlowLayout::with_direction(if self.horizontal {
            FlowDirection::Horizontal
        } else {
            FlowDirection::Vertical
        })
        .with_stretch_minor_axis(self.stretch_minor_axis)
        .with_major_alignment(self.major_alignment)
        .with_minor_alignment(self.minor_alignment)
        .with_spacing(self.major_spacing)
        .with_row_spacing(self.minor_spacing)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    content: FigureId,
    controls: Vec<FigureId>,
    settings: Arc<Mutex<FlowSettings>>,
}

pub fn flow_layout_example() -> Runtime {
    fixture().runtime
}

pub fn panel_scene() -> PanelScene {
    let fixture = canvas_fixture(false);
    let content = fixture.content;
    let settings = fixture.settings;
    (
        fixture.runtime,
        Some(Panel::new([CONTROL_X as f32, 20.0], move |ui, runtime| {
            let mut state = settings.lock().expect("Flow settings");
            let before = *state;
            ui.heading("FlowLayout");
            ui.checkbox(&mut state.horizontal, "Horizontal");
            ui.checkbox(&mut state.stretch_minor_axis, "Stretch Minor Axis");
            ui.label("Major Axis");
            for (label, value) in [
                ("Start", FlowAlignment::Start),
                ("Center", FlowAlignment::Center),
                ("End", FlowAlignment::End),
            ] {
                ui.radio_value(&mut state.major_alignment, value, label);
            }
            ui.add(
                egui::Slider::new(&mut state.major_spacing, 0.0..=20.0)
                    .integer()
                    .text("Spacing"),
            );
            ui.separator();
            ui.label("Minor Axis");
            for (label, value) in [
                ("Start", FlowAlignment::Start),
                ("Center", FlowAlignment::Center),
                ("End", FlowAlignment::End),
            ] {
                ui.radio_value(&mut state.minor_alignment, value, label);
            }
            ui.add(
                egui::Slider::new(&mut state.minor_spacing, 0.0..=20.0)
                    .integer()
                    .text("Spacing"),
            );
            if *state != before {
                runtime
                    .figure(content)
                    .expect("Flow container")
                    .set_layout_manager(Box::new(state.layout()))
                    .expect("Flow layout");
            }
        })),
    )
}

fn fixture() -> Fixture {
    canvas_fixture(cfg!(test))
}

fn canvas_fixture(_legacy_controls: bool) -> Fixture {
    let mut runtime = runtime_with_fonts();
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WIDTH,
            HEIGHT,
            Color::WHITE,
        )))
        .expect("FlowLayoutExample root");
    let content = runtime
        .figure(root)
        .expect("root")
        .add(Box::new(
            RectangleFigure::new_with_color(
                10.0,
                10.0,
                CONTENT_WIDTH,
                CONTENT_HEIGHT,
                Color::WHITE,
            )
            .with_border(LineBorder::default_border()),
        ))
        .expect("FlowLayout container");
    let settings = Arc::new(Mutex::new(FlowSettings::default()));
    runtime
        .figure(content)
        .expect("FlowLayout container")
        .set_layout_manager(Box::new(settings.lock().expect("Flow settings").layout()))
        .expect("FlowLayout");
    add_official_shapes(&mut runtime, content);

    #[cfg(not(test))]
    let controls = Vec::new();
    #[cfg(test)]
    let controls = if _legacy_controls {
        let controls_panel = runtime
            .figure(root)
            .expect("root")
            .add(Box::new(
                RectangleFigure::new_with_color(
                    CONTROL_X,
                    10.0,
                    CONTROL_WIDTH,
                    CONTENT_HEIGHT,
                    Color::rgba(0.96, 0.96, 0.96, 1.0),
                )
                .with_border(LineBorder::default_border()),
            ))
            .expect("control panel");
        add_heading(&mut runtime, controls_panel, "FlowLayout Properties", 8.0);

        let mut controls = Vec::new();
        let horizontal = Arc::new(AtomicBool::new(true));
        controls.push(add_checkbox(
            &mut runtime,
            controls_panel,
            Rectangle::new(12.0, 42.0, 210.0, 22.0),
            "Horizontal",
            horizontal,
            layout_bool_action(settings.clone(), content, |state, value| {
                state.horizontal = value;
            }),
        ));
        let stretch = Arc::new(AtomicBool::new(false));
        controls.push(add_checkbox(
            &mut runtime,
            controls_panel,
            Rectangle::new(12.0, 68.0, 210.0, 22.0),
            "Stretch Minor Axis",
            stretch,
            layout_bool_action(settings.clone(), content, |state, value| {
                state.stretch_minor_axis = value;
            }),
        ));

        add_heading(&mut runtime, controls_panel, "Major Axis", 105.0);
        let major_group = Arc::new(AtomicUsize::new(0));
        for (index, (label, alignment)) in [
            ("Top/Left", FlowAlignment::Start),
            ("Middle/Center", FlowAlignment::Center),
            ("Bottom/Right", FlowAlignment::End),
        ]
        .into_iter()
        .enumerate()
        {
            controls.push(add_radio(
                &mut runtime,
                controls_panel,
                Rectangle::new(12.0, 132.0 + index as f64 * 24.0, 210.0, 22.0),
                label,
                major_group.clone(),
                index,
                layout_choice_action(settings.clone(), content, alignment, true),
            ));
        }
        let major_spacing = Arc::new(Mutex::new(5.0));
        controls.push(add_scale(
            &mut runtime,
            controls_panel,
            Rectangle::new(12.0, 210.0, 210.0, 24.0),
            0.0,
            20.0,
            major_spacing,
            layout_scale_action(settings.clone(), content, true),
        ));
        add_caption(&mut runtime, controls_panel, "Spacing", 235.0);

        add_heading(&mut runtime, controls_panel, "Minor Axis", 275.0);
        let minor_group = Arc::new(AtomicUsize::new(0));
        for (index, (label, alignment)) in [
            ("Top/Left", FlowAlignment::Start),
            ("Middle/Center", FlowAlignment::Center),
            ("Bottom/Right", FlowAlignment::End),
        ]
        .into_iter()
        .enumerate()
        {
            controls.push(add_radio(
                &mut runtime,
                controls_panel,
                Rectangle::new(12.0, 302.0 + index as f64 * 24.0, 210.0, 22.0),
                label,
                minor_group.clone(),
                index,
                layout_choice_action(settings.clone(), content, alignment, false),
            ));
        }
        let minor_spacing = Arc::new(Mutex::new(5.0));
        controls.push(add_scale(
            &mut runtime,
            controls_panel,
            Rectangle::new(12.0, 380.0, 210.0, 24.0),
            0.0,
            20.0,
            minor_spacing,
            layout_scale_action(settings.clone(), content, false),
        ));
        add_caption(&mut runtime, controls_panel, "Spacing", 405.0);
        controls
    } else {
        Vec::new()
    };

    Fixture {
        runtime,
        content,
        controls,
        settings,
    }
}

fn add_official_shapes(runtime: &mut Runtime, parent: FigureId) {
    let shapes: Vec<(Box<dyn novadraw::figure::Figure>, Dimension)> = vec![
        (
            Box::new(EllipseFigure::new_with_color(
                0.0,
                0.0,
                60.0,
                40.0,
                Color::rgba(0.0, 0.0, 1.0, 1.0),
            )),
            Dimension::new(60.0, 40.0),
        ),
        (
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                30.0,
                70.0,
                Color::rgba(1.0, 0.0, 0.0, 1.0),
            )),
            Dimension::new(30.0, 70.0),
        ),
        (
            Box::new(RoundedRectangleFigure::new_with_color(
                0.0,
                0.0,
                90.0,
                30.0,
                8.0,
                Color::rgba(1.0, 1.0, 0.0, 1.0),
            )),
            Dimension::new(90.0, 30.0),
        ),
        (
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                80.0,
                Color::rgba(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0),
            )),
            Dimension::new(50.0, 80.0),
        ),
        (
            Box::new(EllipseFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                50.0,
                Color::rgba(0.0, 1.0, 0.0, 1.0),
            )),
            Dimension::new(50.0, 50.0),
        ),
        (
            Box::new(
                TriangleFigure::new(Rectangle::new(0.0, 0.0, 50.0, 50.0))
                    .with_fill_color(Color::BLACK),
            ),
            Dimension::new(50.0, 50.0),
        ),
        (
            Box::new(RoundedRectangleFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                50.0,
                8.0,
                Color::rgba(0.0, 1.0, 1.0, 1.0),
            )),
            Dimension::new(50.0, 50.0),
        ),
        (
            Box::new(RectangleFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                70.0,
                Color::rgba(0.0, 127.0 / 255.0, 0.0, 1.0),
            )),
            Dimension::new(50.0, 70.0),
        ),
        (
            Box::new(
                TriangleFigure::new(Rectangle::new(0.0, 0.0, 50.0, 50.0))
                    .with_fill_color(Color::rgba(1.0, 196.0 / 255.0, 0.0, 1.0)),
            ),
            Dimension::new(50.0, 50.0),
        ),
        (
            Box::new(EllipseFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                50.0,
                Color::rgba(1.0, 0.0, 0.0, 1.0),
            )),
            Dimension::new(50.0, 50.0),
        ),
        (
            Box::new(RoundedRectangleFigure::new_with_color(
                0.0,
                0.0,
                50.0,
                50.0,
                8.0,
                Color::rgba(1.0, 1.0, 0.0, 1.0),
            )),
            Dimension::new(50.0, 50.0),
        ),
    ];
    for (mut figure, size) in shapes {
        if let Some(shape) = figure.as_any_mut().downcast_mut::<RectangleFigure>() {
            shape.stroke_color = Some(Color::BLACK);
            shape.stroke_width = 1.0;
        } else if let Some(shape) = figure.as_any_mut().downcast_mut::<EllipseFigure>() {
            shape.stroke_color = Some(Color::BLACK);
            shape.stroke_width = 1.0;
        } else if let Some(shape) = figure.as_any_mut().downcast_mut::<RoundedRectangleFigure>() {
            shape.stroke_color = Some(Color::BLACK);
            shape.stroke_width = 1.0;
        } else if let Some(shape) = figure.as_any_mut().downcast_mut::<TriangleFigure>() {
            shape.stroke_color = Color::BLACK;
        }
        let child = runtime
            .figure(parent)
            .expect("FlowLayout container")
            .add(figure)
            .expect("official shape");
        runtime
            .figure(child)
            .expect("official shape")
            .set_preferred_size(size)
            .expect("official shape preferred size");
    }
}

#[cfg(test)]
fn layout_bool_action(
    settings: Arc<Mutex<FlowSettings>>,
    content: FigureId,
    update: fn(&mut FlowSettings, bool),
) -> impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut state = settings.lock().expect("Flow settings");
            update(&mut state, value);
            state.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

#[cfg(test)]
fn layout_choice_action(
    settings: Arc<Mutex<FlowSettings>>,
    content: FigureId,
    alignment: FlowAlignment,
    major: bool,
) -> impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static {
    move |_value, context| {
        let layout = {
            let mut state = settings.lock().expect("Flow settings");
            if major {
                state.major_alignment = alignment;
            } else {
                state.minor_alignment = alignment;
            }
            state.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

#[cfg(test)]
fn layout_scale_action(
    settings: Arc<Mutex<FlowSettings>>,
    content: FigureId,
    major: bool,
) -> impl for<'a> Fn(f64, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut state = settings.lock().expect("Flow settings");
            if major {
                state.major_spacing = value.round();
            } else {
                state.minor_spacing = value.round();
            }
            state.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

#[cfg(test)]
fn add_heading(runtime: &mut Runtime, parent: FigureId, text: &str, y: f64) {
    add_text(runtime, parent, text, y, "bold 13px Inter Variable");
}

#[cfg(test)]
fn add_caption(runtime: &mut Runtime, parent: FigureId, text: &str, y: f64) {
    add_text(runtime, parent, text, y, "12px Inter Variable");
}

#[cfg(test)]
fn add_text(runtime: &mut Runtime, parent: FigureId, text: &str, y: f64, font: &str) {
    let label = runtime
        .figure(parent)
        .expect("control panel")
        .add(Box::new(
            LabelFigure::new(text).with_bounds(Rectangle::new(12.0, y, 210.0, 22.0)),
        ))
        .expect("control panel label");
    runtime
        .figure(label)
        .expect("control panel label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some(font.to_owned()),
            ..FigureStyle::default()
        })
        .expect("control panel label style");
}

#[cfg(test)]
mod tests {
    use novadraw::event::MouseButton;

    use super::*;

    #[test]
    fn figure_composition_matches_the_official_shape_sequence_and_control_types() {
        let fixture = fixture();
        let shapes = fixture
            .runtime
            .tree()
            .child_order(fixture.content)
            .expect("FlowLayout shapes");
        assert_eq!(shapes.len(), 11);
        let names = shapes
            .iter()
            .map(|id| {
                fixture
                    .runtime
                    .tree()
                    .node(*id)
                    .expect("shape")
                    .figure_name()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "EllipseFigure",
                "RectangleFigure",
                "RoundedRectangleFigure",
                "RectangleFigure",
                "EllipseFigure",
                "TriangleFigure",
                "RoundedRectangleFigure",
                "RectangleFigure",
                "TriangleFigure",
                "EllipseFigure",
                "RoundedRectangleFigure",
            ]
        );
        let control_names = fixture
            .controls
            .iter()
            .map(|id| {
                fixture
                    .runtime
                    .tree()
                    .node(*id)
                    .expect("control")
                    .figure_name()
            })
            .collect::<Vec<_>>();
        assert_eq!(control_names[0], "CheckBox");
        assert_eq!(control_names[1], "CheckBox");
        assert_eq!(control_names[2], "RadioButton");
        assert!(control_names.contains(&"Scale"));
    }

    #[test]
    fn horizontal_checkbox_replaces_the_layout_with_vertical_flow() {
        let mut fixture = fixture();
        let bounds = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.controls[0])
            .expect("horizontal checkbox");
        fixture.runtime.input().dispatch_mouse_pressed(
            bounds.center().x() + CONTROL_X,
            bounds.center().y() + 10.0,
            MouseButton::Left,
        );
        assert!(!fixture.settings.lock().expect("Flow settings").horizontal);
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }
}
