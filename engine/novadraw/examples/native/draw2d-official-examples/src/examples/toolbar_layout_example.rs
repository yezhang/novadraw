#[cfg(test)]
use novadraw::{
    event::EventContext,
    figure::{FigureStyle, LabelFigure},
    geometry::Rectangle,
};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use novadraw::{
    figure::{
        EllipseFigure, RectangleFigure, RoundedRectangleFigure,
        border::{LineBorder, MarginBorder},
    },
    geometry::Dimension,
    graphics::Color,
    layout::{FillLayout, MinorAlignment, ToolbarLayout, ToolbarOrientation},
    runtime::Runtime,
    tree::FigureId,
};

#[cfg(test)]
use crate::controls::{add_checkbox, add_radio, add_scale};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

use super::runtime_with_fonts;

const WIDTH: f64 = 820.0;
const HEIGHT: f64 = 520.0;
const CONTENT_WIDTH: f64 = 540.0;
const CONTENT_HEIGHT: f64 = 480.0;
#[cfg(test)]
const CONTROL_X: f64 = 560.0;
const CONTROL_WIDTH: f64 = 240.0;
const CANVAS_INSET: f64 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ToolbarSettings {
    horizontal: bool,
    stretch_minor_axis: bool,
    minor_alignment: MinorAlignment,
    spacing: f64,
}

impl Default for ToolbarSettings {
    fn default() -> Self {
        Self {
            horizontal: true,
            stretch_minor_axis: false,
            minor_alignment: MinorAlignment::Start,
            spacing: 0.0,
        }
    }
}

impl ToolbarSettings {
    fn layout(self) -> ToolbarLayout {
        ToolbarLayout::new()
            .with_orientation(if self.horizontal {
                ToolbarOrientation::Horizontal
            } else {
                ToolbarOrientation::Vertical
            })
            .with_stretch_minor_axis(self.stretch_minor_axis)
            .with_minor_alignment(self.minor_alignment)
            .with_spacing(self.spacing)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
struct Fixture {
    runtime: Runtime,
    content: FigureId,
    controls: Vec<FigureId>,
    settings: Arc<Mutex<ToolbarSettings>>,
}

pub fn toolbar_layout_example() -> Runtime {
    fixture().runtime
}

pub fn panel_scene() -> PanelScene {
    let fixture = canvas_fixture(false);
    let content = fixture.content;
    let settings = fixture.settings;
    (
        fixture.runtime,
        Some(
            Panel::new([-CANVAS_INSET as f32, 20.0], move |ui, runtime| {
                let mut state = settings.lock().expect("Toolbar settings");
                let before = *state;
                let state = &mut *state;
                ui.heading("ToolbarLayout");
                for (label, value) in [
                    ("Horizontal", &mut state.horizontal),
                    ("Stretch Minor Axis", &mut state.stretch_minor_axis),
                ] {
                    let response = ui.checkbox(value, label);
                    #[cfg(test)]
                    super::panel_test::record(ui, label, &response);
                    let _ = response;
                }
                for (label, value) in [
                    ("Start", MinorAlignment::Start),
                    ("Center", MinorAlignment::Center),
                    ("End", MinorAlignment::End),
                ] {
                    let response = ui.radio_value(&mut state.minor_alignment, value, label);
                    #[cfg(test)]
                    super::panel_test::record(ui, label, &response);
                    let _ = response;
                }
                ui.add(
                    egui::Slider::new(&mut state.spacing, 0.0..=20.0)
                        .integer()
                        .text("Spacing"),
                );
                if *state != before {
                    runtime
                        .figure(content)
                        .expect("Toolbar container")
                        .set_layout_manager(Box::new(state.layout()))
                        .expect("Toolbar layout");
                }
            })
            .with_anchor(egui::Align2::RIGHT_TOP, CONTROL_WIDTH as f32),
        ),
    )
}

fn fixture() -> Fixture {
    canvas_fixture(cfg!(test))
}

fn canvas_fixture(_legacy_controls: bool) -> Fixture {
    let mut runtime = runtime_with_fonts();
    let mut root_figure = RectangleFigure::new_with_color(0.0, 0.0, WIDTH, HEIGHT, Color::WHITE)
        .expect("valid figure bounds");
    if !_legacy_controls {
        root_figure = root_figure
            .with_border(
                MarginBorder::new(novadraw::geometry::Insets::new(
                    CANVAS_INSET,
                    CANVAS_INSET,
                    HEIGHT - CANVAS_INSET - CONTENT_HEIGHT,
                    WIDTH - CANVAS_INSET - CONTENT_WIDTH,
                ))
                .expect("valid margins"),
            )
            .expect("valid detached figure input");
    }
    let root = runtime
        .set_contents(Box::new(root_figure))
        .expect("ToolbarLayoutExample root");
    if !_legacy_controls {
        runtime
            .figure(root)
            .expect("Toolbar root")
            .set_layout_manager(Box::new(FillLayout::new()))
            .expect("Toolbar Canvas layout");
    }
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
            .expect("valid figure bounds")
            .with_border(LineBorder::default_border())
            .expect("valid detached figure input"),
        ))
        .expect("ToolbarLayout container");
    let settings = Arc::new(Mutex::new(ToolbarSettings::default()));
    runtime
        .figure(content)
        .expect("ToolbarLayout container")
        .set_layout_manager(Box::new(
            settings.lock().expect("Toolbar settings").layout(),
        ))
        .expect("ToolbarLayout");
    add_official_shapes(&mut runtime, content);

    #[cfg(not(test))]
    let controls = Vec::new();
    #[cfg(test)]
    let controls = if _legacy_controls {
        let panel = runtime
            .figure(root)
            .expect("root")
            .add(Box::new(
                RectangleFigure::new_with_color(
                    CONTROL_X,
                    10.0,
                    240.0,
                    CONTENT_HEIGHT,
                    Color::rgba(0.96, 0.96, 0.96, 1.0),
                )
                .expect("valid figure bounds")
                .with_border(LineBorder::default_border())
                .expect("valid detached figure input"),
            ))
            .expect("control panel");
        add_heading(&mut runtime, panel, "ToolbarLayout Properties", 10.0);

        let mut controls = Vec::new();
        controls.push(add_checkbox(
            &mut runtime,
            panel,
            Rectangle::new(12.0, 45.0, 210.0, 22.0),
            "Horizontal",
            Arc::new(AtomicBool::new(true)),
            toolbar_bool_action(settings.clone(), content, |state, value| {
                state.horizontal = value;
            }),
        ));
        controls.push(add_checkbox(
            &mut runtime,
            panel,
            Rectangle::new(12.0, 72.0, 210.0, 22.0),
            "Stretch Minor Axis",
            Arc::new(AtomicBool::new(false)),
            toolbar_bool_action(settings.clone(), content, |state, value| {
                state.stretch_minor_axis = value;
            }),
        ));

        add_heading(&mut runtime, panel, "Minor Axis", 112.0);
        let alignment = Arc::new(AtomicUsize::new(0));
        for (index, (text, value)) in [
            ("Top/Left", MinorAlignment::Start),
            ("Middle/Center", MinorAlignment::Center),
            ("Bottom/Right", MinorAlignment::End),
        ]
        .into_iter()
        .enumerate()
        {
            controls.push(add_radio(
                &mut runtime,
                panel,
                Rectangle::new(12.0, 140.0 + index as f64 * 26.0, 210.0, 22.0),
                text,
                alignment.clone(),
                index,
                toolbar_alignment_action(settings.clone(), content, value),
            ));
        }
        controls.push(add_scale(
            &mut runtime,
            panel,
            Rectangle::new(12.0, 225.0, 210.0, 24.0),
            0.0,
            20.0,
            Arc::new(Mutex::new(5.0)),
            toolbar_scale_action(settings.clone(), content),
        ));
        add_caption(&mut runtime, panel, "Spacing", 252.0);
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
            Box::new(
                EllipseFigure::new_with_color(
                    0.0,
                    0.0,
                    60.0,
                    40.0,
                    Color::rgba(0.0, 0.0, 1.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
            Dimension::new(60.0, 40.0),
        ),
        (
            Box::new(
                RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    30.0,
                    70.0,
                    Color::rgba(1.0, 0.0, 0.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
            Dimension::new(30.0, 70.0),
        ),
        (
            Box::new(
                RoundedRectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    90.0,
                    30.0,
                    8.0,
                    Color::rgba(1.0, 1.0, 0.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
            Dimension::new(90.0, 30.0),
        ),
        (
            Box::new(
                RectangleFigure::new_with_color(
                    0.0,
                    0.0,
                    50.0,
                    80.0,
                    Color::rgba(128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
            Dimension::new(50.0, 80.0),
        ),
        (
            Box::new(
                EllipseFigure::new_with_color(
                    0.0,
                    0.0,
                    50.0,
                    50.0,
                    Color::rgba(0.0, 1.0, 0.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
            Dimension::new(50.0, 50.0),
        ),
    ];
    for (mut figure, size) in shapes {
        if let Some(shape) = figure.as_any_mut().downcast_mut::<RectangleFigure>() {
            *shape = shape
                .clone()
                .with_stroke(Color::BLACK, 1.0)
                .expect("valid shape stroke");
        } else if let Some(shape) = figure.as_any_mut().downcast_mut::<EllipseFigure>() {
            *shape = shape
                .clone()
                .with_stroke(Color::BLACK, 1.0)
                .expect("valid shape stroke");
        } else if let Some(shape) = figure.as_any_mut().downcast_mut::<RoundedRectangleFigure>() {
            *shape = shape
                .clone()
                .with_stroke(Color::BLACK, 1.0)
                .expect("valid shape stroke");
        }
        let child = runtime
            .figure(parent)
            .expect("ToolbarLayout container")
            .add(figure)
            .expect("official toolbar shape");
        runtime
            .figure(child)
            .expect("official toolbar shape")
            .set_preferred_size(size)
            .expect("official toolbar preferred size");
    }
}

#[cfg(test)]
fn toolbar_bool_action(
    settings: Arc<Mutex<ToolbarSettings>>,
    content: FigureId,
    update: fn(&mut ToolbarSettings, bool),
) -> impl for<'a> Fn(bool, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut state = settings.lock().expect("Toolbar settings");
            update(&mut state, value);
            state.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

#[cfg(test)]
fn toolbar_alignment_action(
    settings: Arc<Mutex<ToolbarSettings>>,
    content: FigureId,
    alignment: MinorAlignment,
) -> impl for<'a> Fn(usize, &mut EventContext<'a>) + Send + Sync + 'static {
    move |_value, context| {
        let layout = {
            let mut state = settings.lock().expect("Toolbar settings");
            state.minor_alignment = alignment;
            state.layout()
        };
        context
            .effects()
            .set_layout_manager_later(content, Box::new(layout));
    }
}

#[cfg(test)]
fn toolbar_scale_action(
    settings: Arc<Mutex<ToolbarSettings>>,
    content: FigureId,
) -> impl for<'a> Fn(f64, &mut EventContext<'a>) + Send + Sync + 'static {
    move |value, context| {
        let layout = {
            let mut state = settings.lock().expect("Toolbar settings");
            state.spacing = value.round();
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
            LabelFigure::new(text)
                .with_bounds(Rectangle::new(12.0, y, 210.0, 22.0))
                .expect("valid detached figure input"),
        ))
        .expect("control label");
    runtime
        .figure(label)
        .expect("control label")
        .set_style(FigureStyle {
            foreground: Some(Color::BLACK),
            font: Some(novadraw::text::FontDescriptor::parse(font).unwrap()),
            ..FigureStyle::default()
        })
        .expect("control label style");
}

#[cfg(test)]
mod tests {
    use novadraw::event::MouseButton;

    use super::*;

    #[test]
    fn native_panel_alignment_and_stretch_commit_geometry_without_mock_controls() {
        let mut driver = super::super::panel_test::Driver::new("ToolbarLayoutExample");
        let root = driver.runtime.tree().contents().unwrap();
        let content = driver.runtime.tree().child_order(root).unwrap()[0];
        let shapes = driver.runtime.tree().child_order(content).unwrap();
        driver
            .runtime
            .frames()
            .resize_logical_viewport(WIDTH, HEIGHT)
            .unwrap();
        driver.draw(Vec::new());
        for (label, expected_y) in [("Center", 219.0), ("End", 438.0), ("Start", 0.0)] {
            driver.click(label);
            assert_eq!(
                driver.runtime.tree().figure_bounds(shapes[0]).unwrap().y,
                expected_y
            );
        }
        driver.click("Stretch Minor Axis");
        assert_eq!(
            driver
                .runtime
                .tree()
                .figure_bounds(shapes[0])
                .unwrap()
                .height,
            CONTENT_HEIGHT - 2.0
        );
        driver.click("Horizontal");
        assert_eq!(
            driver
                .runtime
                .tree()
                .figure_bounds(shapes[0])
                .unwrap()
                .width,
            CONTENT_WIDTH - 2.0
        );
        driver.click("Stretch Minor Axis");
        driver.click("Horizontal");
        let first = driver.runtime.tree().figure_bounds(shapes[0]).unwrap();
        assert_eq!((first.width, first.height), (60.0, 40.0));
    }

    #[test]
    fn stretch_orientation_shrink_and_reset_preserve_official_preferred_sizes() {
        let mut fixture = canvas_fixture(false);
        let shapes = fixture.runtime.tree().child_order(fixture.content).unwrap();
        let sizes = [
            (60.0, 40.0),
            (30.0, 70.0),
            (90.0, 30.0),
            (50.0, 80.0),
            (50.0, 50.0),
        ];
        for (horizontal, stretch, width, height) in [
            (true, false, 540.0, 480.0),
            (true, true, 540.0, 480.0),
            (false, true, 540.0, 480.0),
            (false, false, 540.0, 480.0),
            (true, false, 120.0, 100.0),
            (true, false, 540.0, 480.0),
        ] {
            let settings = ToolbarSettings {
                horizontal,
                stretch_minor_axis: stretch,
                ..ToolbarSettings::default()
            };
            fixture
                .runtime
                .frames()
                .resize_logical_viewport(
                    WIDTH + width - CONTENT_WIDTH,
                    HEIGHT + height - CONTENT_HEIGHT,
                )
                .unwrap();
            fixture
                .runtime
                .figure(fixture.content)
                .unwrap()
                .set_layout_manager(Box::new(settings.layout()))
                .unwrap();
            fixture.runtime.frames().stabilize_for_query().unwrap();
            let bounds: Vec<_> = shapes
                .iter()
                .map(|id| fixture.runtime.tree().figure_bounds(*id).unwrap())
                .collect();
            for (index, (rect, (preferred_width, preferred_height))) in
                bounds.iter().zip(sizes).enumerate()
            {
                if horizontal {
                    assert_eq!(
                        rect.height,
                        if stretch {
                            height - 2.0
                        } else {
                            preferred_height
                        }
                    );
                    if width > 300.0 {
                        assert_eq!(rect.width, preferred_width);
                    } else {
                        assert!(rect.width <= preferred_width);
                    }
                    if index > 0 {
                        assert_eq!(rect.x, bounds[index - 1].x + bounds[index - 1].width);
                    }
                } else {
                    assert_eq!(
                        rect.width,
                        if stretch {
                            width - 2.0
                        } else {
                            preferred_width
                        }
                    );
                    assert_eq!(rect.height, preferred_height);
                }
            }
        }
    }

    #[test]
    fn official_shape_sequence_and_control_types_are_preserved() {
        let fixture = fixture();
        let shapes = fixture
            .runtime
            .tree()
            .child_order(fixture.content)
            .expect("toolbar shapes");
        assert_eq!(shapes.len(), 5);
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
            ]
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.controls[0])
                .expect("checkbox")
                .figure_name(),
            "CheckBox"
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(fixture.controls[2])
                .expect("radio")
                .figure_name(),
            "RadioButton"
        );
        assert_eq!(
            fixture
                .runtime
                .tree()
                .node(*fixture.controls.last().expect("scale"))
                .expect("scale")
                .figure_name(),
            "Scale"
        );
    }

    #[test]
    fn horizontal_checkbox_switches_the_toolbar_orientation() {
        let mut fixture = fixture();
        let local = fixture
            .runtime
            .tree()
            .figure_bounds(fixture.controls[0])
            .expect("horizontal checkbox");
        fixture.runtime.input().dispatch_mouse_pressed(
            CONTROL_X + local.center().x(),
            10.0 + local.center().y(),
            MouseButton::Left,
        );
        assert!(
            !fixture
                .settings
                .lock()
                .expect("Toolbar settings")
                .horizontal
        );
        assert!(fixture.runtime.take_deferred_mutation_errors().is_empty());
    }
}
