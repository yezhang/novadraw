use std::time::Duration;

use novadraw::{
    BuiltinFont, ButtonFigure, Color, FigureId, FigureStyle, LabelFigure, MonotonicTime,
    MouseButton, Rectangle, RectangleFigure, Runtime, ToggleFigure, TooltipTiming,
};

use crate::{DemoSuite, SceneSpec, ValidationKind};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const TITLE_COLOR: Color = Color {
    r: 0.08,
    g: 0.15,
    b: 0.25,
    a: 1.0,
};
const CAPTION_COLOR: Color = Color {
    r: 0.32,
    g: 0.36,
    b: 0.42,
    a: 1.0,
};
const SCENE_BACKGROUND: Color = Color {
    r: 0.94,
    g: 0.95,
    b: 0.97,
    a: 1.0,
};
const INHERITED_PANEL_BACKGROUND: Color = Color {
    r: 0.84,
    g: 0.9,
    b: 0.96,
    a: 1.0,
};
const TOOLTIP_BOUNDARY_X: f64 = 600.0;
const TOOLTIP_BOUNDARY_Y: f64 = 535.0;
const TOOLTIP_BOUNDARY_WIDTH: f64 = 150.0;
const TOOLTIP_BOUNDARY_HEIGHT: f64 = 32.0;
const TOOLTIP_POINTER_X: f64 = 700.0;
const TOOLTIP_POINTER_Y: f64 = 550.0;
const TOOLTIP_VISUAL_HIDE_DELAY: Duration = Duration::from_secs(3_600);
#[cfg(test)]
const WEB_MINIMUM_CONTENT_HEIGHT: f64 = 575.0;

pub type RuntimeSceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

pub fn entries() -> Vec<RuntimeSceneEntry> {
    vec![
        ("Button_States", Box::new(button_states)),
        ("Toggle_States", Box::new(toggle_states)),
        ("Interactive_Widgets", Box::new(interactive_widgets)),
        ("Tooltip_Boundary_Visual", Box::new(tooltip_boundary_visual)),
        ("Tooltip_Accessibility", Box::new(tooltip_accessibility)),
    ]
}

pub fn suite() -> DemoSuite {
    DemoSuite::new(
        "widgets",
        "Widgets",
        vec![
            SceneSpec::runtime_visual(
                "button-states",
                "Button States",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                button_states,
            ),
            SceneSpec::runtime_visual(
                "toggle-states",
                "Toggle States",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                toggle_states,
            ),
            SceneSpec::runtime(
                "interactive-widgets",
                "Interactive Widgets",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                ValidationKind::Interactive,
                interactive_widgets,
            ),
            SceneSpec::runtime(
                "tooltip-boundary-visual",
                "Tooltip Boundary Visual",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                ValidationKind::Visual,
                tooltip_boundary_visual,
            ),
            SceneSpec::runtime(
                "tooltip-accessibility",
                "Tooltip and Accessibility",
                (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
                ValidationKind::Interactive,
                tooltip_accessibility,
            ),
        ],
    )
}

fn runtime() -> Runtime {
    let mut runtime = Runtime::empty();
    for font in BuiltinFont::ALL {
        runtime.register_builtin_font(font).expect("built-in font");
    }
    runtime
}

fn root(runtime: &mut Runtime, title: &str) -> FigureId {
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            SCENE_BACKGROUND,
        )))
        .expect("valid Runtime mutation");
    let heading = runtime
        .add_figure(
            root,
            Box::new(LabelFigure::new(title).with_bounds(Rectangle::new(40.0, 28.0, 720.0, 42.0))),
        )
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            heading,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("26px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
    root
}

fn add_caption(runtime: &mut Runtime, root: FigureId, text: &str, y: f64) {
    let caption = runtime
        .add_figure(
            root,
            Box::new(LabelFigure::new(text).with_bounds(Rectangle::new(60.0, y, 180.0, 32.0))),
        )
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            caption,
            FigureStyle {
                foreground: Some(CAPTION_COLOR),
                font: Some("15px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
}

fn add_button(runtime: &mut Runtime, root: FigureId, text: &str, bounds: Rectangle) -> FigureId {
    let button = runtime
        .add_figure(root, Box::new(ButtonFigure::new(text).with_bounds(bounds)))
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            button,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("16px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
    button
}

fn button_states() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime, "Button visual states");

    for (label, y, enabled) in [
        ("Normal", 120.0, true),
        ("Pressed + focus", 200.0, true),
        ("Disabled", 280.0, false),
    ] {
        add_caption(&mut runtime, root, label, y + 5.0);
        let button = add_button(
            &mut runtime,
            root,
            if enabled {
                "Apply changes"
            } else {
                "Unavailable"
            },
            Rectangle::new(270.0, y, 210.0, 48.0),
        );
        if !enabled {
            runtime
                .set_enabled(button, false)
                .expect("valid Runtime mutation");
        }
    }

    runtime.dispatch_mouse_pressed(300.0, 220.0, MouseButton::Left);
    runtime
}

fn toggle_states() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime, "Toggle owns selection state");

    add_caption(&mut runtime, root, "Not selected", 140.0);
    let first = runtime
        .add_figure(
            root,
            Box::new(
                ToggleFigure::new("Snap to grid")
                    .with_bounds(Rectangle::new(270.0, 135.0, 210.0, 48.0)),
            ),
        )
        .expect("valid Runtime mutation");
    add_caption(&mut runtime, root, "Selected", 225.0);
    let second = runtime
        .add_figure(
            root,
            Box::new(
                ToggleFigure::new("Show guides")
                    .with_bounds(Rectangle::new(270.0, 220.0, 210.0, 48.0))
                    .with_selected(true),
            ),
        )
        .expect("valid Runtime mutation");
    for id in [first, second] {
        runtime
            .set_figure_style(
                id,
                FigureStyle {
                    foreground: Some(TITLE_COLOR),
                    font: Some("16px Inter Variable".to_string()),
                    ..FigureStyle::default()
                },
            )
            .expect("valid Runtime mutation");
    }
    runtime.request_focus(second).expect("focusable toggle");
    runtime
}

fn interactive_widgets() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime, "Click, drag away, or use Enter / Space");

    add_button(
        &mut runtime,
        root,
        "Momentary action",
        Rectangle::new(210.0, 150.0, 230.0, 52.0),
    );
    let toggle = runtime
        .add_figure(
            root,
            Box::new(
                ToggleFigure::new("Persistent toggle")
                    .with_bounds(Rectangle::new(210.0, 240.0, 230.0, 52.0)),
            ),
        )
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            toggle,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("16px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
    runtime
}

fn tooltip_accessibility() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime, "Tooltip source and accessibility roles");
    runtime
        .set_figure_style(
            root,
            FigureStyle {
                background: Some(SCENE_BACKGROUND),
                tooltip: Some(Some("Inherited from the scene root".to_string())),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");

    let inherited_panel = runtime
        .add_figure(
            root,
            Box::new(RectangleFigure::new_with_color(
                70.0,
                125.0,
                300.0,
                150.0,
                INHERITED_PANEL_BACKGROUND,
            )),
        )
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            inherited_panel,
            FigureStyle {
                background: Some(INHERITED_PANEL_BACKGROUND),
                tooltip: Some(Some("Inherited from the blue container".to_string())),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
    let inherited_label = runtime
        .add_figure(
            inherited_panel,
            Box::new(
                LabelFigure::new("Inherited tooltip")
                    .with_bounds(Rectangle::new(35.0, 48.0, 230.0, 42.0)),
            ),
        )
        .expect("valid Runtime mutation");
    runtime
        .set_figure_style(
            inherited_label,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("18px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");

    let button = add_button(
        &mut runtime,
        root,
        "Accessible action",
        Rectangle::new(440.0, 155.0, 230.0, 52.0),
    );
    runtime
        .set_figure_style(
            button,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("16px Inter Variable".to_string()),
                tooltip: Some(Some("Button role with a default action".to_string())),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");

    let boundary = add_button(
        &mut runtime,
        root,
        "Bottom edge",
        Rectangle::new(
            TOOLTIP_BOUNDARY_X,
            TOOLTIP_BOUNDARY_Y,
            TOOLTIP_BOUNDARY_WIDTH,
            TOOLTIP_BOUNDARY_HEIGHT,
        ),
    );
    runtime
        .set_figure_style(
            boundary,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("15px Inter Variable".to_string()),
                tooltip: Some(Some(
                    "Flips above and clamps inside the surface".to_string(),
                )),
                ..FigureStyle::default()
            },
        )
        .expect("valid Runtime mutation");
    runtime
}

fn tooltip_boundary_visual() -> Runtime {
    let mut runtime = tooltip_accessibility();
    runtime
        .set_tooltip_timing(TooltipTiming::new(Duration::ZERO, TOOLTIP_VISUAL_HIDE_DELAY).unwrap())
        .unwrap();
    runtime.dispatch_mouse_moved(TOOLTIP_POINTER_X, TOOLTIP_POINTER_Y);
    runtime.advance_time(MonotonicTime::ZERO).unwrap();
    runtime
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_scene_starts_with_the_boundary_tooltip_visible() {
        let runtime = tooltip_boundary_visual();
        let tooltip = runtime.visible_tooltip().expect("visible tooltip");

        assert_eq!(tooltip.text, "Flips above and clamps inside the surface");
        assert_eq!(tooltip.anchor.x(), TOOLTIP_POINTER_X);
        assert_eq!(tooltip.anchor.y(), TOOLTIP_POINTER_Y);
        assert!(TOOLTIP_BOUNDARY_Y + TOOLTIP_BOUNDARY_HEIGHT <= WEB_MINIMUM_CONTENT_HEIGHT);
        assert_eq!(
            runtime.next_wake_deadline(),
            Some(MonotonicTime::from_micros(
                TOOLTIP_VISUAL_HIDE_DELAY.as_micros() as u64
            ))
        );
    }

    #[test]
    fn interactive_tooltip_scene_uses_the_default_hover_delay() {
        let mut runtime = tooltip_accessibility();
        runtime.dispatch_mouse_moved(400.0, 300.0);

        assert_eq!(
            runtime.next_wake_deadline(),
            Some(MonotonicTime::from_micros(500_000))
        );
        assert!(runtime.visible_tooltip().is_none());
    }

    #[test]
    fn tooltip_style_replacement_preserves_scene_and_panel_backgrounds() {
        let runtime = tooltip_accessibility();
        let root = runtime.tree().contents().expect("scene contents");
        let panel = runtime
            .tree()
            .child_order(root)
            .expect("root children")
            .into_iter()
            .find(|child| {
                runtime.tree().figure_bounds(*child)
                    == Some(Rectangle::new(70.0, 125.0, 300.0, 150.0))
            })
            .expect("inherited tooltip panel");

        assert_eq!(
            runtime.tree().resolved_style(root).unwrap().background,
            SCENE_BACKGROUND
        );
        assert_eq!(
            runtime.tree().resolved_style(panel).unwrap().background,
            INHERITED_PANEL_BACKGROUND
        );
    }
}
