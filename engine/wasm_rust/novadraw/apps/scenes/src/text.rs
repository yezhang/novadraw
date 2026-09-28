use novadraw::{
    Alignment, BuiltinFont, Color, FigureId, FigureStyle, ImageData, ImageFigure, LabelFigure,
    Rectangle, RectangleFigure, Runtime, TextPlacement, TitleBarBorder,
};

use crate::{DemoSuite, SceneSpec};

const WINDOW_WIDTH: f64 = 800.0;
const WINDOW_HEIGHT: f64 = 600.0;
const PANEL_FILL: Color = Color::rgba(0.97, 0.98, 1.0, 1.0);
const PANEL_STROKE: Color = Color::rgba(0.72, 0.77, 0.86, 1.0);
const HEADING: Color = Color::rgba(0.08, 0.19, 0.34, 1.0);
const ACCENT: Color = Color::rgba(0.04, 0.45, 0.62, 1.0);
const ICON_SIZE: u32 = 32;
const DOCUMENT_LEFT: u32 = 6;
const DOCUMENT_TOP: u32 = 3;
const DOCUMENT_RIGHT: u32 = 26;
const DOCUMENT_BOTTOM: u32 = 29;
const FOLD_SIZE: u32 = 7;
const IMAGE_CARD_LEFT: f64 = 35.0;
const IMAGE_CARD_STRIDE: f64 = 180.0;
const IMAGE_CARD_WIDTH: f64 = 160.0;
#[cfg(test)]
const WEB_MINIMUM_CONTENT_WIDTH: f64 = 760.0;

pub type RuntimeSceneEntry = (&'static str, Box<dyn FnMut() -> Runtime>);

pub fn entries() -> Vec<RuntimeSceneEntry> {
    suite().into_entries()
}

pub fn suite() -> DemoSuite {
    let size = (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    DemoSuite::new(
        "text-image",
        "Text / Image",
        vec![
            SceneSpec::runtime_visual("typography-cjk", "Typography_CJK", size, typography_scene),
            SceneSpec::runtime_visual("ellipsis", "Ellipsis", size, ellipsis_scene),
            SceneSpec::runtime_visual(
                "icon-placement",
                "Icon_Placement",
                size,
                icon_placement_scene,
            ),
            SceneSpec::runtime_visual(
                "style-inheritance",
                "Style_Inheritance",
                size,
                style_inheritance_scene,
            ),
            SceneSpec::runtime_visual(
                "title-bar-border",
                "TitleBarBorder",
                size,
                title_bar_border_scene,
            ),
            SceneSpec::runtime_visual(
                "image-resources",
                "Image_Resources",
                size,
                image_resources_scene,
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

fn root(runtime: &mut Runtime) -> FigureId {
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            Color::rgba(0.93, 0.94, 0.95, 1.0),
        )))
        .expect("valid Runtime mutation");
    runtime
        .figure(root)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(HEADING),
            font: Some("16px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    root
}

fn panel(runtime: &mut Runtime, parent: FigureId, bounds: Rectangle) -> FigureId {
    runtime
        .container(parent)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                PANEL_FILL,
            )
            .with_stroke(PANEL_STROKE, 1.0),
        ))
        .expect("valid Runtime mutation")
}

fn add_label(
    runtime: &mut Runtime,
    parent: FigureId,
    text: &str,
    bounds: Rectangle,
    font: &str,
    color: Color,
) -> FigureId {
    let id = runtime
        .container(parent)
        .unwrap()
        .add(Box::new(LabelFigure::new(text).with_bounds(bounds)))
        .expect("valid Runtime mutation");
    runtime
        .figure(id)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(color),
            font: Some(font.to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
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
        let label = runtime
            .container(card)
            .unwrap()
            .add(Box::new(
                LabelFigure::new(format!("{placement:?}"))
                    .with_icon(icon)
                    .with_bounds(Rectangle::new(24.0, 24.0, 232.0, 82.0)),
            ))
            .expect("valid Runtime mutation");
        runtime
            .figure(label)
            .unwrap()
            .set_label_text_placement(placement)
            .expect("label placement");
        runtime
            .figure(label)
            .unwrap()
            .set_label_alignment(Alignment::Center)
            .expect("label alignment");
        runtime
            .figure(label)
            .unwrap()
            .set_style(FigureStyle {
                foreground: Some(ACCENT),
                font: Some("18px Inter Variable".to_string()),
                ..FigureStyle::default()
            })
            .expect("valid Runtime mutation");
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
    runtime
        .figure(inherited)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::rgba(0.42, 0.15, 0.62, 1.0)),
            font: Some("22px Noto Sans SC".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    runtime
        .container(inherited)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("继承颜色和字体，不在 Label 中复制通用样式。")
                .with_bounds(Rectangle::new(36.0, 50.0, 560.0, 36.0)),
        ))
        .expect("valid Runtime mutation");

    let overridden = panel(
        &mut runtime,
        root,
        Rectangle::new(80.0, 310.0, 640.0, 140.0),
    );
    runtime
        .figure(overridden)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::rgba(0.14, 0.36, 0.18, 1.0)),
            font: Some("20px JetBrains Mono".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    runtime
        .container(overridden)
        .unwrap()
        .add(Box::new(
            LabelFigure::new("inherited: FontDescriptor -> GlyphRun")
                .with_bounds(Rectangle::new(36.0, 50.0, 560.0, 36.0)),
        ))
        .expect("valid Runtime mutation");
    runtime
}

fn title_bar_border_scene() -> Runtime {
    let mut runtime = runtime();
    let root = root(&mut runtime);
    let title_bar = TitleBarBorder::new(
        "TitleBarBorder measures text before layout",
        Color::rgba(0.08, 0.35, 0.62, 1.0),
    )
    .with_alignment(Alignment::Start)
    .with_padding(18.0, 8.0);
    let panel = runtime
        .container(root)
        .unwrap()
        .add(Box::new(
            RectangleFigure::new_with_color(80.0, 100.0, 640.0, 300.0, PANEL_FILL)
                .with_border(title_bar),
        ))
        .expect("valid Runtime mutation");
    runtime
        .figure(panel)
        .unwrap()
        .set_style(FigureStyle {
            foreground: Some(Color::WHITE),
            font: Some("20px Inter Variable".to_string()),
            ..FigureStyle::default()
        })
        .expect("valid Runtime mutation");
    runtime
        .container(panel)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            640.0,
            300.0,
            Color::rgba(0.78, 0.9, 0.84, 1.0),
        )))
        .expect("valid Runtime mutation");
    runtime
}

fn image_resources_scene() -> Runtime {
    const PNG: &[u8] =
        include_bytes!("../../../screenshot/shape-app_0_Rectangle_Fill_1771989903.png");
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
        let x = IMAGE_CARD_LEFT + index as f64 * IMAGE_CARD_STRIDE;
        let card = panel(
            &mut runtime,
            root,
            Rectangle::new(x, 110.0, IMAGE_CARD_WIDTH, 300.0),
        );
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
        runtime
            .container(card)
            .unwrap()
            .add(Box::new(
                ImageFigure::new(image).with_bounds(Rectangle::new(18.0, 68.0, 129.0, 190.0)),
            ))
            .expect("valid Runtime mutation");
    }
    runtime
}

#[cfg(test)]
mod tests {
    use novadraw::{BackendCapabilities, SurfaceInfo};

    use super::*;

    #[test]
    fn shared_text_suite_builds_every_web_scene_with_required_resources() {
        let mut suite = suite();
        assert_eq!(suite.scenes.len(), 6);

        for scene in &mut suite.scenes {
            let mut runtime = scene.build();
            let submission = runtime
                .prepare_submission(
                    SurfaceInfo {
                        logical_width: WINDOW_WIDTH,
                        logical_height: WINDOW_HEIGHT,
                        pixel_width: WINDOW_WIDTH as u32,
                        pixel_height: WINDOW_HEIGHT as u32,
                        scale_factor: 1.0,
                    },
                    BackendCapabilities::RETAINED_PARTIAL
                        .with_glyph_runs()
                        .with_image_resources(),
                )
                .expect("text/image scene must produce a supported initial frame");

            assert!(!submission.commands.is_empty(), "empty scene: {}", scene.id);
        }
    }

    #[test]
    fn title_bar_border_is_a_panel_below_the_host_contents() {
        let runtime = title_bar_border_scene();
        let root = runtime.tree().contents().expect("host contents");
        let panel = runtime
            .tree()
            .child_order(root)
            .expect("root children")
            .into_iter()
            .find(|child| {
                runtime.tree().figure_bounds(*child)
                    == Some(Rectangle::new(80.0, 100.0, 640.0, 300.0))
            })
            .expect("title bar panel");

        assert_eq!(
            runtime.tree().figure_bounds(root),
            Some(Rectangle::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT))
        );
        assert_eq!(runtime.tree().child_order(panel).unwrap().len(), 1);
    }

    #[test]
    fn image_resource_cards_fit_the_standard_web_content_width() {
        let runtime = image_resources_scene();
        let root = runtime.tree().contents().expect("host contents");
        let cards = runtime
            .tree()
            .child_order(root)
            .expect("root children")
            .into_iter()
            .filter_map(|child| runtime.tree().figure_bounds(child))
            .filter(|bounds| bounds.y == 110.0 && bounds.height == 300.0)
            .collect::<Vec<_>>();

        assert_eq!(cards.len(), 4);
        assert!(
            cards
                .iter()
                .all(|bounds| bounds.x + bounds.width <= WEB_MINIMUM_CONTENT_WIDTH)
        );
    }
}
