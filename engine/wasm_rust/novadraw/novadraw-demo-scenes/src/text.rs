use novadraw::{
    Alignment, BuiltinFont, Color, FigureId, FigureStyle, ImageData, ImageFigure, LabelFigure,
    Rectangle, RectangleFigure, Runtime, TextPlacement, TitleBarBorder,
};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const PANEL_FILL: Color = Color {
    r: 0.97,
    g: 0.98,
    b: 1.0,
    a: 1.0,
};
const PANEL_STROKE: Color = Color {
    r: 0.72,
    g: 0.77,
    b: 0.86,
    a: 1.0,
};
const HEADING: Color = Color {
    r: 0.08,
    g: 0.19,
    b: 0.34,
    a: 1.0,
};
const ACCENT: Color = Color {
    r: 0.04,
    g: 0.45,
    b: 0.62,
    a: 1.0,
};
const ICON_SIZE: u32 = 32;
const DOCUMENT_LEFT: u32 = 6;
const DOCUMENT_TOP: u32 = 3;
const DOCUMENT_RIGHT: u32 = 26;
const DOCUMENT_BOTTOM: u32 = 29;
const FOLD_SIZE: u32 = 7;

pub type RuntimeSceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

pub fn entries() -> Vec<RuntimeSceneEntry> {
    vec![
        ("Typography_CJK", Box::new(typography_scene)),
        ("Ellipsis", Box::new(ellipsis_scene)),
        ("Icon_Placement", Box::new(icon_placement_scene)),
        ("Style_Inheritance", Box::new(style_inheritance_scene)),
        ("TitleBarBorder", Box::new(title_bar_border_scene)),
        ("Image_Resources", Box::new(image_resources_scene)),
    ]
}

fn runtime() -> Runtime {
    let mut runtime = Runtime::empty();
    for font in BuiltinFont::ALL {
        runtime.register_builtin_font(font).expect("built-in font");
    }
    runtime
}

fn root(runtime: &mut Runtime) -> FigureId {
    let root = runtime.set_contents(Box::new(RectangleFigure::new_with_color(
        0.0,
        0.0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        Color::rgba(0.93, 0.94, 0.95, 1.0),
    )));
    runtime.set_figure_style(
        root,
        FigureStyle {
            foreground: Some(HEADING),
            font: Some("16px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    root
}

fn panel(runtime: &mut Runtime, parent: FigureId, bounds: Rectangle) -> FigureId {
    runtime.add_figure(
        parent,
        Box::new(
            RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                PANEL_FILL,
            )
            .with_stroke(PANEL_STROKE, 1.0),
        ),
    )
}

fn add_label(
    runtime: &mut Runtime,
    parent: FigureId,
    text: &str,
    bounds: Rectangle,
    font: &str,
    color: Color,
) -> FigureId {
    let id = runtime.add_figure(parent, Box::new(LabelFigure::new(text).with_bounds(bounds)));
    runtime.set_figure_style(
        id,
        FigureStyle {
            foreground: Some(color),
            font: Some(font.to_string()),
            ..FigureStyle::default()
        },
    );
    id
}

fn document_check_icon() -> ImageData {
    let mut pixels = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    for y in DOCUMENT_TOP..DOCUMENT_BOTTOM {
        for x in DOCUMENT_LEFT..DOCUMENT_RIGHT {
            let offset = ((y * ICON_SIZE + x) * 4) as usize;
            let folded_corner = x >= DOCUMENT_RIGHT - FOLD_SIZE && y < DOCUMENT_TOP + FOLD_SIZE;
            let [red, green, blue] = if folded_corner {
                [186, 230, 253]
            } else {
                [15, 118, 190]
            };
            pixels[offset..offset + 4].copy_from_slice(&[red, green, blue, 255]);
        }
    }

    for (x, y) in [
        (11, 17),
        (12, 18),
        (13, 19),
        (14, 20),
        (15, 19),
        (16, 18),
        (17, 17),
        (18, 16),
        (19, 15),
        (20, 14),
    ] {
        let offset = ((y * ICON_SIZE + x) * 4) as usize;
        pixels[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
    }
    ImageData::from_rgba(ICON_SIZE, ICON_SIZE, pixels, 1.0)
}

fn typography_scene() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime);
    add_label(
        &mut runtime,
        root,
        "Text layout: Inter, Noto Sans SC, JetBrains Mono",
        Rectangle::new(40.0, 30.0, 700.0, 42.0),
        "26px Inter Variable",
        HEADING,
    );

    let latin = panel(
        &mut runtime,
        root,
        Rectangle::new(40.0, 100.0, 340.0, 180.0),
    );
    add_label(
        &mut runtime,
        latin,
        "Inter Variable",
        Rectangle::new(24.0, 24.0, 280.0, 32.0),
        "22px Inter Variable",
        ACCENT,
    );
    add_label(
        &mut runtime,
        latin,
        "The quick brown fox jumps over 123.",
        Rectangle::new(24.0, 82.0, 290.0, 30.0),
        "16px Inter Variable",
        HEADING,
    );

    let cjk = panel(
        &mut runtime,
        root,
        Rectangle::new(420.0, 100.0, 340.0, 180.0),
    );
    add_label(
        &mut runtime,
        cjk,
        "Noto Sans SC",
        Rectangle::new(24.0, 24.0, 280.0, 32.0),
        "22px Noto Sans SC",
        ACCENT,
    );
    add_label(
        &mut runtime,
        cjk,
        "图形工具包需要稳定一致的中文测量。",
        Rectangle::new(24.0, 82.0, 290.0, 30.0),
        "16px Noto Sans SC",
        HEADING,
    );

    let mono = panel(
        &mut runtime,
        root,
        Rectangle::new(40.0, 320.0, 720.0, 120.0),
    );
    add_label(
        &mut runtime,
        mono,
        "let layout = engine.shape(\"glyph run\");",
        Rectangle::new(24.0, 42.0, 660.0, 32.0),
        "18px JetBrains Mono",
        Color::rgba(0.16, 0.35, 0.2, 1.0),
    );
    runtime
}

fn ellipsis_scene() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime);
    add_label(
        &mut runtime,
        root,
        "Width-constrained labels use measured grapheme-safe ellipsis",
        Rectangle::new(40.0, 30.0, 720.0, 42.0),
        "24px Inter Variable",
        HEADING,
    );
    for (index, (width, text)) in [
        (
            580.0,
            "A long English label remains readable before truncation.",
        ),
        (340.0, "中文标签按照真实字形宽度截断并保留省略号。"),
        (180.0, "Family emoji stays intact: 👨‍👩‍👧‍👦 with a suffix"),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 110.0 + index as f64 * 130.0;
        let card = panel(&mut runtime, root, Rectangle::new(80.0, y, 640.0, 84.0));
        add_label(
            &mut runtime,
            card,
            text,
            Rectangle::new(28.0, 27.0, width, 30.0),
            "18px Inter Variable",
            if index == 1 { ACCENT } else { HEADING },
        );
    }
    runtime
}

fn icon_placement_scene() -> Runtime {
    let mut runtime = runtime();
    let icon = runtime.register_image();
    runtime
        .complete_image(icon, document_check_icon())
        .expect("icon resource");
    let root = root(&mut runtime);
    add_label(
        &mut runtime,
        root,
        "Image + text placement and alignment",
        Rectangle::new(40.0, 30.0, 700.0, 42.0),
        "24px Inter Variable",
        HEADING,
    );
    for (index, placement) in [
        TextPlacement::East,
        TextPlacement::West,
        TextPlacement::North,
        TextPlacement::South,
    ]
    .into_iter()
    .enumerate()
    {
        let column = index % 2;
        let row = index / 2;
        let card = panel(
            &mut runtime,
            root,
            Rectangle::new(
                80.0 + column as f64 * 360.0,
                120.0 + row as f64 * 180.0,
                280.0,
                130.0,
            ),
        );
        let label = runtime.add_figure(
            card,
            Box::new(
                LabelFigure::new(format!("{placement:?}"))
                    .with_icon(icon)
                    .with_bounds(Rectangle::new(24.0, 24.0, 232.0, 82.0)),
            ),
        );
        runtime
            .set_label_text_placement(label, placement)
            .expect("label placement");
        runtime
            .set_label_alignment(label, Alignment::Center)
            .expect("label alignment");
        runtime.set_figure_style(
            label,
            FigureStyle {
                foreground: Some(ACCENT),
                font: Some("18px Inter Variable".to_string()),
                ..FigureStyle::default()
            },
        );
    }
    runtime
}

fn style_inheritance_scene() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime);
    add_label(
        &mut runtime,
        root,
        "Resolved style reaches Label glyph paint",
        Rectangle::new(40.0, 30.0, 700.0, 42.0),
        "24px Inter Variable",
        HEADING,
    );
    let inherited = panel(
        &mut runtime,
        root,
        Rectangle::new(80.0, 120.0, 640.0, 140.0),
    );
    runtime.set_figure_style(
        inherited,
        FigureStyle {
            foreground: Some(Color::rgba(0.42, 0.15, 0.62, 1.0)),
            font: Some("22px Noto Sans SC".to_string()),
            ..FigureStyle::default()
        },
    );
    runtime.add_figure(
        inherited,
        Box::new(
            LabelFigure::new("继承颜色和字体，不在 Label 中复制通用样式。")
                .with_bounds(Rectangle::new(36.0, 50.0, 560.0, 36.0)),
        ),
    );

    let overridden = panel(
        &mut runtime,
        root,
        Rectangle::new(80.0, 310.0, 640.0, 140.0),
    );
    runtime.set_figure_style(
        overridden,
        FigureStyle {
            foreground: Some(Color::rgba(0.14, 0.36, 0.18, 1.0)),
            font: Some("20px JetBrains Mono".to_string()),
            ..FigureStyle::default()
        },
    );
    runtime.add_figure(
        overridden,
        Box::new(
            LabelFigure::new("inherited: FontDescriptor -> GlyphRun")
                .with_bounds(Rectangle::new(36.0, 50.0, 560.0, 36.0)),
        ),
    );
    runtime
}

fn title_bar_border_scene() -> Runtime {
    let mut runtime = runtime();
    let title_bar = TitleBarBorder::new(
        "TitleBarBorder measures text before layout",
        Color::rgba(0.08, 0.35, 0.62, 1.0),
    )
    .with_alignment(Alignment::Start)
    .with_padding(18.0, 8.0);
    let root = runtime.set_contents(Box::new(
        RectangleFigure::new_with_color(80.0, 100.0, 640.0, 300.0, PANEL_FILL)
            .with_border(title_bar),
    ));
    runtime.set_figure_style(
        root,
        FigureStyle {
            foreground: Some(Color::WHITE),
            font: Some("20px Inter Variable".to_string()),
            ..FigureStyle::default()
        },
    );
    runtime.add_figure(
        root,
        Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            640.0,
            300.0,
            Color::rgba(0.78, 0.9, 0.84, 1.0),
        )),
    );
    runtime
}

fn image_resources_scene() -> Runtime {
    const PNG: &[u8] = include_bytes!("../../screenshot/shape-app_0_Rectangle_Fill_1771989903.png");
    const SVG: &[u8] = include_bytes!("../assets/image-demo.svg");

    let mut runtime = runtime();
    let root = root(&mut runtime);
    add_label(
        &mut runtime,
        root,
        "ImageFigure: decoded files and resource states",
        Rectangle::new(40.0, 28.0, 720.0, 42.0),
        "24px Inter Variable",
        HEADING,
    );

    for (index, label) in ["PNG Ready", "SVG Ready", "Pending", "Failed"]
        .into_iter()
        .enumerate()
    {
        let x = 45.0 + index as f64 * 190.0;
        let card = panel(&mut runtime, root, Rectangle::new(x, 110.0, 165.0, 300.0));
        add_label(
            &mut runtime,
            card,
            label,
            Rectangle::new(12.0, 18.0, 141.0, 28.0),
            "16px Inter Variable",
            HEADING,
        );
        let image = runtime.register_image();
        match index {
            0 => runtime
                .complete_image_bytes(image, PNG, 12.5)
                .expect("PNG fixture"),
            1 => runtime
                .complete_image_bytes(image, SVG, 1.25)
                .expect("SVG fixture"),
            2 => {}
            3 => {
                let _ = runtime.complete_image_bytes(image, b"not an image", 1.0);
            }
            _ => unreachable!(),
        }
        runtime.add_figure(
            card,
            Box::new(ImageFigure::new(image).with_bounds(Rectangle::new(18.0, 68.0, 129.0, 190.0))),
        );
    }
    runtime
}
