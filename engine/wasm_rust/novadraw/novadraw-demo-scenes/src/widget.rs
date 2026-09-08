use novadraw::{
    BuiltinFont, ButtonFigure, Color, FigureId, FigureStyle, LabelFigure, MouseButton, Rectangle,
    RectangleFigure, Runtime, ToggleFigure,
};

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

pub type RuntimeSceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

pub fn entries() -> Vec<RuntimeSceneEntry> {
    vec![
        ("Button_States", Box::new(button_states)),
        ("Toggle_States", Box::new(toggle_states)),
        ("Interactive_Widgets", Box::new(interactive_widgets)),
    ]
}

fn runtime() -> Runtime {
    let mut runtime = Runtime::empty();
    for font in BuiltinFont::ALL {
        runtime.register_builtin_font(font).expect("built-in font");
    }
    runtime
}

fn root(runtime: &mut Runtime, title: &str) -> FigureId {
    let root = runtime.set_contents(Box::new(RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        Color::rgba(0.94, 0.95, 0.97, 1.0),
    )));
    let heading = runtime.add_figure(
        root,
        Box::new(LabelFigure::new(title).with_bounds(Rectangle::new(40.0, 28.0, 720.0, 42.0))),
    );
    runtime.set_figure_style(
        heading,
        FigureStyle {
            foreground: Some(TITLE_COLOR),
            font: Some("26px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    root
}

fn add_caption(runtime: &mut Runtime, root: FigureId, text: &str, y: f64) {
    let caption = runtime.add_figure(
        root,
        Box::new(LabelFigure::new(text).with_bounds(Rectangle::new(60.0, y, 180.0, 32.0))),
    );
    runtime.set_figure_style(
        caption,
        FigureStyle {
            foreground: Some(CAPTION_COLOR),
            font: Some("15px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
}

fn add_button(runtime: &mut Runtime, root: FigureId, text: &str, bounds: Rectangle) -> FigureId {
    let button = runtime.add_figure(root, Box::new(ButtonFigure::new(text).with_bounds(bounds)));
    runtime.set_figure_style(
        button,
        FigureStyle {
            foreground: Some(TITLE_COLOR),
            font: Some("16px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
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
            runtime.set_enabled(button, false);
        }
    }

    runtime.dispatch_mouse_pressed(300.0, 220.0, MouseButton::Left);
    runtime
}

fn toggle_states() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime, "Toggle owns selection state");

    add_caption(&mut runtime, root, "Not selected", 140.0);
    let first = runtime.add_figure(
        root,
        Box::new(
            ToggleFigure::new("Snap to grid")
                .with_bounds(Rectangle::new(270.0, 135.0, 210.0, 48.0)),
        ),
    );
    add_caption(&mut runtime, root, "Selected", 225.0);
    let second = runtime.add_figure(
        root,
        Box::new(
            ToggleFigure::new("Show guides")
                .with_bounds(Rectangle::new(270.0, 220.0, 210.0, 48.0))
                .with_selected(true),
        ),
    );
    for id in [first, second] {
        runtime.set_figure_style(
            id,
            FigureStyle {
                foreground: Some(TITLE_COLOR),
                font: Some("16px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        );
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
    let toggle = runtime.add_figure(
        root,
        Box::new(
            ToggleFigure::new("Persistent toggle")
                .with_bounds(Rectangle::new(210.0, 240.0, 230.0, 52.0)),
        ),
    );
    runtime.set_figure_style(
        toggle,
        FigureStyle {
            foreground: Some(TITLE_COLOR),
            font: Some("16px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    runtime
}
