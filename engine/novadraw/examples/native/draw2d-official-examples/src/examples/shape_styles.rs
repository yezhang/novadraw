//! ShapeStylesExample Canvas consumer (EPL-2.0 reference).
//! Axis order matters: the Y setting overrides X when both select the same axis.
use super::runtime_with_fonts;
use novadraw::{
    figure::{Figure, LabelFigure, RectangleFigure},
    geometry::{Affine2D, Point, Rectangle},
    graphics::{
        AntialiasMode, Color, CustomDash, DashPattern, GraphicsError, LineCap, PaintContext, Path,
        StrokeStyle,
    },
    runtime::Runtime,
    tree::FigureId,
};
use novadraw_example_support::panel::{Panel, PanelScene, egui};

const CELL_WIDTH: f64 = 108.0;
const CELL_HEIGHT: f64 = 88.0;
const LABEL_WIDTH: f64 = 100.0;
const HEADER_HEIGHT: f64 = 50.0;
const SHAPE_WIDTH: f64 = 100.0;
const SHAPE_HEIGHT: f64 = 75.0;
const CORNER_DIMENSION: f64 = 8.0;
// Portable equivalents of SWT buttonLightest/buttonDarker; not native theme colors.
const DISABLED_LIGHT: Color = Color::WHITE;
const DISABLED_DARK: Color = Color::rgba(0.5, 0.5, 0.5, 1.0);
const DISABLED_OFFSET: f64 = 1.0;
const COCOA_ZERO_WIDTH: f64 = 1.0;
const WIDTHS: &[f64] = &[0.0, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];
const MITERS: &[f64] = &[0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0];
const OFFSETS: &[f64] = &[0.0, 0.25, 0.5, 1.0, 2.0, 2.5, 5.0, 10.0];
const ALPHAS: &[u8] = &[0, 10, 50, 100, 150, 200, 255];
const DEFAULT_WIDTHS: &[f64] = &[0.0, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];
const AXES: &[&str] = &[
    "Line Width",
    "Line Style",
    "Line Cap",
    "Line Miter Limit",
    "Line Dash Offset",
    "Anti-Aliasing",
    "Alpha",
    "Enabled",
];
const SHAPES: &[&str] = &[
    "Ellipse",
    "Rectangle",
    "RoundedRectangle",
    "Triangle",
    "Polyline",
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Settings {
    x: usize,
    y: usize,
    shape: usize,
    width: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            x: 0,
            y: 1,
            shape: 0,
            width: 5.0,
        }
    }
}

#[derive(Clone)]
struct CellStyle {
    width: f64,
    dash: usize,
    cap: LineCap,
    miter: f64,
    offset: f64,
    aa: AntialiasMode,
    alpha: u8,
    enabled: bool,
}
impl CellStyle {
    fn new(width: f64) -> Self {
        Self {
            width,
            dash: 0,
            cap: LineCap::Butt,
            miter: 10.0,
            offset: 0.0,
            aa: AntialiasMode::On,
            alpha: 255,
            enabled: true,
        }
    }
    fn apply(&mut self, axis: usize, index: usize) {
        match axis {
            0 => self.width = WIDTHS[index],
            1 => self.dash = index,
            2 => self.cap = [LineCap::Butt, LineCap::Round, LineCap::Square][index],
            3 => self.miter = MITERS[index],
            4 => self.offset = OFFSETS[index],
            5 => {
                self.aa = [
                    AntialiasMode::On,
                    AntialiasMode::Off,
                    AntialiasMode::Default,
                ][index]
            }
            6 => self.alpha = ALPHAS[index],
            7 => self.enabled = index == 0,
            _ => unreachable!("fixed upstream axis"),
        }
    }
    fn stroke(&self) -> StrokeStyle {
        // SWT Cocoa standard dash lengths scale with the final line width.
        let pattern: &[f64] = match (self.width == 0.0, self.dash) {
            (true, 0) => &[],
            (true, 1) => &[18.0, 6.0],
            (true, 2) => &[9.0, 6.0, 3.0, 6.0],
            (true, 3) => &[9.0, 3.0, 3.0, 3.0, 3.0, 3.0],
            (true, 4) => &[3.0, 3.0],
            (false, 0) => &[],
            (false, 1) => &[3.0, 1.0],
            (false, 2) => &[3.0, 1.0, 1.0, 1.0],
            (false, 3) => &[3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            (false, 4) => &[1.0, 1.0],
            _ => unreachable!("upstream line style"),
        };
        let width = if self.width == 0.0 {
            COCOA_ZERO_WIDTH
        } else {
            self.width
        };
        let dash = if pattern.is_empty() {
            DashPattern::Solid
        } else {
            DashPattern::Custom(
                CustomDash::try_new(&pattern.iter().map(|v| v * width).collect::<Vec<_>>())
                    .expect("positive dash"),
            )
        };
        StrokeStyle::default()
            .with_width(width)
            .expect("supported width")
            .with_cap(self.cap)
            .with_miter_limit(self.miter)
            .expect("supported miter")
            .with_dash_pattern(dash)
            .with_dash_offset(self.offset)
            .expect("offset")
    }
}

fn labels(axis: usize) -> Vec<String> {
    match axis {
        0 => WIDTHS.iter().map(ToString::to_string).collect(),
        1 => ["SOLID", "DASH", "DASHDOT", "DASHDOTDOT", "DOT"]
            .map(str::to_owned)
            .to_vec(),
        2 => ["FLAT", "ROUND", "SQUARE"].map(str::to_owned).to_vec(),
        3 => MITERS.iter().map(ToString::to_string).collect(),
        4 => OFFSETS.iter().map(ToString::to_string).collect(),
        5 => ["ON", "OFF", "DEFAULT"].map(str::to_owned).to_vec(),
        6 => ALPHAS.iter().map(ToString::to_string).collect(),
        7 => ["true", "false"].map(str::to_owned).to_vec(),
        _ => unreachable!("fixed upstream axis"),
    }
}

// An external Figure exercises the public Graphics API for every shape and style.
// It owns only detached configuration; attached changes replace it through Runtime.
fn outline_rectangle(width: f64) -> Rectangle {
    let half = width.max(1.0) / 2.0;
    let leading = half.floor();
    let total = leading + half.ceil();
    Rectangle::new(leading, leading, SHAPE_WIDTH - total, SHAPE_HEIGHT - total)
}

fn rounded_path(rect: Rectangle, corner_dimension: f64) -> Path {
    // SWT round-rectangle dimensions describe the full corner ellipse.
    let rx = corner_dimension.min(rect.width).max(0.0) / 2.0;
    let ry = corner_dimension.min(rect.height).max(0.0) / 2.0;
    let (left, top) = (rect.x, rect.y);
    let (right, bottom) = (left + rect.width, top + rect.height);
    let mut path = Path::new();
    path.move_to(left + rx, top);
    path.line_to(right - rx, top);
    path.arc_to_degrees(rx, ry, 0.0, false, true, right, top + ry);
    path.line_to(right, bottom - ry);
    path.arc_to_degrees(rx, ry, 0.0, false, true, right - rx, bottom);
    path.line_to(left + rx, bottom);
    path.arc_to_degrees(rx, ry, 0.0, false, true, left, bottom - ry);
    path.line_to(left, top + ry);
    path.arc_to_degrees(rx, ry, 0.0, false, true, left + rx, top);
    path.close();
    path
}

fn triangle_path() -> Path {
    // The default NORTH triangle uses integer layout and is independent of width.
    let width = SHAPE_WIDTH - 1.0;
    let height = SHAPE_HEIGHT - 1.0;
    let size = height.min((width / 2.0).floor()).max(1.0);
    let center = (width / 2.0).floor();
    let top = ((height - size) / 2.0).floor();
    let mut path = Path::new();
    path.move_to(center, top);
    path.line_to(center - size, top + size);
    path.line_to(center + size, top + size);
    path.close();
    path
}

struct Sample {
    shape: usize,
    style: CellStyle,
    bounds: Rectangle,
}
impl Figure for Sample {
    fn initial_bounds(&self) -> Rectangle {
        self.bounds
    }
    fn name(&self) -> &'static str {
        "ShapeStylesSample"
    }
    fn paint(&self, context: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
        context.set_antialias(self.style.aa);
        context.set_opacity(novadraw::graphics::Opacity::try_new(
            f64::from(self.style.alpha) / 255.0,
        )?)?;
        context.set_stroke_style(self.style.stroke());
        if self.style.enabled {
            context.set_fill_paint(Color::rgba(1.0, 200.0 / 255.0, 0.0, 1.0));
            context.set_stroke_paint(Color::BLACK);
        } else {
            context.push_state();
            let light = context
                .concat_transform(Affine2D::from_translation(DISABLED_OFFSET, DISABLED_OFFSET))
                .and_then(|()| {
                    context.set_fill_paint(DISABLED_LIGHT);
                    context.set_stroke_paint(DISABLED_LIGHT);
                    self.paint_shape(context)
                });
            let restored = context.pop_state();
            light?;
            restored?;
            context.set_fill_paint(DISABLED_DARK);
            context.set_stroke_paint(DISABLED_DARK);
        }
        self.paint_shape(context)
    }
}

impl Sample {
    fn paint_shape(&self, context: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
        let rect = outline_rectangle(self.style.width);
        match self.shape {
            0 => {
                context.fill_ellipse(rect)?;
                context.stroke_ellipse(rect)
            }
            1 => {
                context.fill_rect(Rectangle::new(0.0, 0.0, SHAPE_WIDTH, SHAPE_HEIGHT))?;
                context.stroke_rect(rect)
            }
            2 => {
                context.fill_path(&rounded_path(
                    Rectangle::new(0.0, 0.0, SHAPE_WIDTH, SHAPE_HEIGHT),
                    CORNER_DIMENSION,
                ))?;
                let corner =
                    (CORNER_DIMENSION - (self.style.width.max(1.0) / 2.0).floor()).max(0.0);
                context.stroke_path(&rounded_path(rect, corner))
            }
            3 => {
                let path = triangle_path();
                context.fill_path(&path)?;
                context.stroke_path(&path)
            }
            4 => context.stroke_polyline(&[
                Point::new(20.0, 20.0),
                Point::new(50.0, 80.0),
                Point::new(10.0, 50.0),
            ]),
            _ => unreachable!("fixed upstream shape"),
        }
    }
}

fn add(runtime: &mut Runtime, root: FigureId, figure: Box<dyn Figure>) -> FigureId {
    runtime
        .figure(root)
        .expect("root")
        .add(figure)
        .expect("matrix item")
}
fn label(runtime: &mut Runtime, root: FigureId, text: String, bounds: Rectangle) {
    let id = add(runtime, root, Box::new(LabelFigure::new(text)));
    runtime
        .figure(id)
        .expect("label")
        .set_bounds(bounds)
        .expect("label bounds");
}
fn rebuild(runtime: &mut Runtime, settings: Settings) {
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            1200.0,
            950.0,
            Color::WHITE,
        )))
        .expect("matrix root");
    let xs = labels(settings.x);
    let ys = labels(settings.y);
    for (x, text) in xs.iter().enumerate() {
        label(
            runtime,
            root,
            format!("{}\n{text}", AXES[settings.x]),
            Rectangle::new(
                LABEL_WIDTH + x as f64 * CELL_WIDTH,
                0.0,
                CELL_WIDTH,
                HEADER_HEIGHT,
            ),
        );
    }
    for (y, text) in ys.iter().enumerate() {
        let top = HEADER_HEIGHT + y as f64 * CELL_HEIGHT;
        label(
            runtime,
            root,
            format!("{}\n{text}", AXES[settings.y]),
            Rectangle::new(0.0, top, LABEL_WIDTH, CELL_HEIGHT),
        );
        for x in 0..xs.len() {
            let mut style = CellStyle::new(settings.width);
            style.apply(settings.x, x);
            style.apply(settings.y, y);
            let bounds = Rectangle::new(
                LABEL_WIDTH + x as f64 * CELL_WIDTH,
                top,
                SHAPE_WIDTH,
                CELL_HEIGHT,
            );
            {
                let enabled = style.enabled;
                let id = add(
                    runtime,
                    root,
                    Box::new(Sample {
                        shape: settings.shape,
                        style,
                        bounds,
                    }),
                );
                runtime
                    .figure(id)
                    .expect("sample")
                    .set_enabled(enabled)
                    .expect("enabled");
            }
        }
    }
}
pub fn shape_styles_example() -> Runtime {
    let mut runtime = runtime_with_fonts();
    rebuild(&mut runtime, Settings::default());
    runtime
}
pub(super) fn panel_scene() -> PanelScene {
    let mut settings = Settings::default();
    (
        shape_styles_example(),
        Some(
            Panel::new([-8.0, 20.0], move |ui, runtime| {
                let before = settings;
                for (name, value) in [("X Axis", &mut settings.x), ("Y Axis", &mut settings.y)] {
                    egui::ComboBox::from_label(name)
                        .selected_text(AXES[*value])
                        .show_ui(ui, |ui| {
                            for (index, text) in AXES.iter().enumerate() {
                                ui.selectable_value(value, index, *text);
                            }
                        });
                }
                egui::ComboBox::from_label("Shape")
                    .selected_text(SHAPES[settings.shape])
                    .show_ui(ui, |ui| {
                        for (index, text) in SHAPES.iter().enumerate() {
                            ui.selectable_value(&mut settings.shape, index, *text);
                        }
                    });
                egui::ComboBox::from_label("Line Width")
                    .selected_text(settings.width.to_string())
                    .show_ui(ui, |ui| {
                        for width in DEFAULT_WIDTHS {
                            ui.selectable_value(&mut settings.width, *width, width.to_string());
                        }
                    });
                ui.label("Cocoa profile: width 0 uses 1 logical unit and ZERO dashes.");
                if settings != before {
                    rebuild(runtime, settings);
                }
            })
            .with_anchor(egui::Align2::RIGHT_TOP, 212.0),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        graphics::PathOp,
        render::{BackendCapabilities, RenderCommandKind, SurfaceInfo},
    };

    #[test]
    fn disabled_shape_draws_both_shades_and_restores_the_offset_before_dark() {
        let mut runtime = runtime_with_fonts();
        let mut style = CellStyle::new(5.0);
        style.enabled = false;
        style.aa = AntialiasMode::Off;
        let sample = runtime
            .set_contents(Box::new(Sample {
                shape: 1,
                style,
                bounds: Rectangle::new(0.0, 0.0, 100.0, 88.0),
            }))
            .unwrap();
        runtime.figure(sample).unwrap().set_enabled(false).unwrap();
        let submission = runtime
            .frames()
            .prepare_submission(
                SurfaceInfo {
                    logical_width: 100.0,
                    logical_height: 88.0,
                    pixel_width: 100,
                    pixel_height: 88,
                    scale_factor: 1.0,
                },
                BackendCapabilities::RETAINED_PARTIAL
                    .with_per_operation_antialias()
                    .with_custom_strokes(),
            )
            .into_ready()
            .unwrap();
        let fills = submission
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| match &command.kind {
                RenderCommandKind::FillRect {
                    antialias,
                    paint: novadraw::graphics::Paint::Solid(color),
                    ..
                } => Some((index, *antialias, *color)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].1, AntialiasMode::Off);
        assert_eq!(fills[0].2, DISABLED_LIGHT);
        assert_eq!(fills[1].1, AntialiasMode::Off);
        assert_eq!(fills[1].2, DISABLED_DARK);
        assert!(
            submission.commands[..fills[0].0]
                .iter()
                .any(|command| matches!(
                    &command.kind,
                    RenderCommandKind::ConcatTransform { matrix }
                        if *matrix == Affine2D::from_translation(1.0, 1.0)
                ))
        );
        assert!(
            submission.commands[fills[0].0..fills[1].0]
                .iter()
                .any(|command| matches!(&command.kind, RenderCommandKind::PopState))
        );
        assert_eq!(
            submission
                .commands
                .iter()
                .filter(|command| matches!(
                    &command.kind,
                    RenderCommandKind::StrokeRect {
                        antialias: AntialiasMode::Off,
                        ..
                    }
                ))
                .count(),
            2
        );
    }

    #[test]
    fn official_shape_geometry_uses_integer_outlines_and_separate_fill() {
        for (width, expected) in [
            (0.25, Rectangle::new(0.0, 0.0, 99.0, 74.0)),
            (2.5, Rectangle::new(1.0, 1.0, 97.0, 72.0)),
            (5.0, Rectangle::new(2.0, 2.0, 95.0, 70.0)),
            (30.0, Rectangle::new(15.0, 15.0, 70.0, 45.0)),
        ] {
            let mut runtime = runtime_with_fonts();
            runtime
                .set_contents(Box::new(Sample {
                    shape: 1,
                    style: CellStyle::new(width),
                    bounds: Rectangle::new(0.0, 0.0, SHAPE_WIDTH, SHAPE_HEIGHT),
                }))
                .unwrap();
            let submission = runtime
                .frames()
                .prepare_submission(
                    SurfaceInfo {
                        logical_width: SHAPE_WIDTH,
                        logical_height: SHAPE_HEIGHT,
                        pixel_width: 100,
                        pixel_height: 75,
                        scale_factor: 1.0,
                    },
                    BackendCapabilities::RETAINED_PARTIAL
                        .with_per_operation_antialias()
                        .with_custom_strokes(),
                )
                .into_ready()
                .unwrap();
            assert!(submission.commands.iter().any(|command| matches!(
                &command.kind,
                RenderCommandKind::FillRect { rect, .. }
                    if *rect == Rectangle::new(0.0, 0.0, 100.0, 75.0)
            )));
            assert!(submission.commands.iter().any(|command| matches!(
                &command.kind,
                RenderCommandKind::StrokeRect { rect, stroke, .. }
                    if *rect == expected && stroke.width() == width
            )));
        }
        assert_eq!(
            triangle_path().operations(),
            &[
                PathOp::MoveTo(Point::new(49.0, 12.0)),
                PathOp::LineTo(Point::new(0.0, 61.0)),
                PathOp::LineTo(Point::new(98.0, 61.0)),
                PathOp::Close,
            ]
        );
        let path = rounded_path(Rectangle::new(0.0, 0.0, 100.0, 75.0), 8.0);
        assert_eq!(path.operations()[0], PathOp::MoveTo(Point::new(4.0, 0.0)));
        assert_eq!(
            path.operations()
                .iter()
                .filter(|op| matches!(
                    op,
                    PathOp::Arc { radii, .. }
                        if *radii == novadraw::geometry::Dimension::new(4.0, 4.0)
                ))
                .count(),
            4
        );
    }

    #[test]
    #[ignore = "requires a native GPU; run through render.antialias"]
    fn shape_styles_native_matrix_pixels_and_scene_replacement() {
        use novadraw::render::{RenderBackend, RenderDisposition};
        use novadraw_backend_vello::{texture::VelloTextureRenderer, wgpu};
        const SIZE: u32 = 1024;
        const ROW: u32 = SIZE * 4;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ShapeStyles matrix evidence"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/verification/per-operation-antialias/matrix");
        std::fs::create_dir_all(&output).unwrap();
        let surface = SurfaceInfo {
            logical_width: f64::from(SIZE),
            logical_height: f64::from(SIZE),
            pixel_width: SIZE,
            pixel_height: SIZE,
            scale_factor: 1.0,
        };
        let mut runtime = runtime_with_fonts();
        for shape in 0..SHAPES.len() {
            for axis in 0..AXES.len() {
                rebuild(
                    &mut runtime,
                    Settings {
                        x: axis,
                        y: 5,
                        shape,
                        width: 5.0,
                    },
                );
                let mut backend = renderer.bind(&target);
                let frame = runtime
                    .frames()
                    .prepare_submission(surface, backend.capabilities())
                    .into_ready()
                    .unwrap();
                let result = backend.submit(&frame);
                assert_eq!(
                    result,
                    Ok(RenderDisposition::Presented),
                    "shape={shape} axis={axis}"
                );
                assert_eq!(
                    runtime
                        .frames()
                        .complete_submission(frame.session_id, frame.frame_id, result)
                        .unwrap(),
                    novadraw::runtime::SubmissionAcknowledgement::Presented
                );
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("matrix readback"),
                    size: u64::from(ROW * SIZE),
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut encoder = device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    target.as_image_copy(),
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(ROW),
                            rows_per_image: Some(SIZE),
                        },
                    },
                    target.size(),
                );
                queue.submit([encoder.finish()]);
                let (tx, rx) = std::sync::mpsc::channel();
                buffer.map_async(wgpu::MapMode::Read, .., move |r| tx.send(r).unwrap());
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(std::time::Duration::from_secs(10)),
                    })
                    .unwrap();
                rx.recv().unwrap().unwrap();
                let bytes = buffer.get_mapped_range(..);
                assert!(
                    shape == 4
                        || bytes
                            .chunks_exact(4)
                            .any(|p| p[0] > 150 && p[1] > 50 && p[2] < 20)
                );
                // Outside all cells must be cleared even after a larger preceding matrix.
                assert_eq!(&bytes[SIZE as usize * ROW as usize - 4..], &[0, 0, 0, 0]);
                let right = LABEL_WIDTH as usize + labels(axis).len() * CELL_WIDTH as usize;
                for y in HEADER_HEIGHT as usize..(HEADER_HEIGHT + 3.0 * CELL_HEIGHT) as usize {
                    for x in right..SIZE as usize {
                        let index = y * ROW as usize + x * 4;
                        assert_eq!(
                            &bytes[index..index + 4],
                            &[255, 255, 255, 255],
                            "old matrix pixels shape={shape} axis={axis} at {x},{y}"
                        );
                    }
                }
                image::save_buffer(
                    output.join(format!("shape-{shape}-axis-{axis}.png")),
                    &bytes,
                    SIZE,
                    SIZE,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
    #[test]
    fn shape_styles_all_axes_and_shapes_use_public_graphics_without_clamping() {
        assert_eq!(labels(4).len(), 8);
        for shape in 0..SHAPES.len() {
            for axis in 0..AXES.len() {
                let mut runtime = runtime_with_fonts();
                rebuild(
                    &mut runtime,
                    Settings {
                        x: axis,
                        y: 5,
                        shape,
                        width: 5.0,
                    },
                );
                let submission = runtime
                    .frames()
                    .prepare_submission(
                        SurfaceInfo {
                            logical_width: 1200.0,
                            logical_height: 950.0,
                            pixel_width: 1200,
                            pixel_height: 950,
                            scale_factor: 1.0,
                        },
                        BackendCapabilities::RETAINED_PARTIAL
                            .with_per_operation_antialias()
                            .with_custom_strokes(),
                    )
                    .into_ready()
                    .expect("matrix");
                assert!(
                    submission
                        .commands
                        .iter()
                        .any(|c| c.kind.antialias() == Some(AntialiasMode::Off))
                );
            }
        }
        let mut style = CellStyle::new(5.0);
        style.apply(0, 0);
        assert_eq!(style.width, 0.0);
        assert_eq!(style.stroke().width(), COCOA_ZERO_WIDTH);
        style.apply(0, 5);
        style.apply(3, 1);
        assert_eq!(style.stroke().miter_limit(), 0.5);
        assert_eq!(
            style.stroke().effective_join(),
            novadraw::graphics::LineJoin::Bevel
        );
        style.apply(3, 5);
        style.apply(5, 0);
        style.apply(5, 1);
        assert_eq!(style.aa, AntialiasMode::Off);
    }

    #[test]
    fn cocoa_zero_dash_profile_preserves_input_and_uses_upstream_arrays() {
        for (dash, expected) in [
            (0, &[][..]),
            (1, &[18.0, 6.0][..]),
            (2, &[9.0, 6.0, 3.0, 6.0][..]),
            (3, &[9.0, 3.0, 3.0, 3.0, 3.0, 3.0][..]),
            (4, &[3.0, 3.0][..]),
        ] {
            let mut style = CellStyle::new(0.0);
            style.dash = dash;
            assert_eq!(style.stroke().dash_lengths().as_ref(), expected);
            assert_eq!(style.width, 0.0);
        }
    }

    #[test]
    #[ignore = "requires native GPU; run through render.low-miter"]
    fn low_miter_pixels_equal_bevel_for_open_closed_reversed_paths() {
        use novadraw::graphics::LineJoin;
        struct StrokeFigure {
            path: Path,
            stroke: StrokeStyle,
            aa: AntialiasMode,
        }
        impl Figure for StrokeFigure {
            fn name(&self) -> &'static str {
                "LowMiterConsumer"
            }
            fn initial_bounds(&self) -> Rectangle {
                Rectangle::new(0.0, 0.0, 128.0, 128.0)
            }
            fn paint(
                &self,
                context: &mut PaintContext<'_>,
                _: Rectangle,
            ) -> Result<(), GraphicsError> {
                context.set_antialias(self.aa);
                context.set_stroke_paint(Color::BLACK);
                context.set_stroke_style(self.stroke.clone());
                context.stroke_path(&self.path)
            }
        }
        let mut gpu = super::super::gpu_test_support::GpuCanvas::new(128, 128, 1.0);
        let mut runtime = runtime_with_fonts();
        for aa in [AntialiasMode::On, AntialiasMode::Off] {
            for closed in [false, true] {
                for reverse in [false, true] {
                    let mut points = [
                        Point::new(20.0, 100.0),
                        Point::new(64.0, 15.0),
                        Point::new(75.0, 100.0),
                    ];
                    if reverse {
                        points.reverse();
                    }
                    let mut path = Path::new();
                    path.move_to(points[0].x(), points[0].y());
                    for point in &points[1..] {
                        path.line_to(point.x(), point.y());
                    }
                    if closed {
                        path.close();
                    }
                    for limit in [0.0, 0.5] {
                        let stroke = StrokeStyle::default()
                            .with_width(12.0)
                            .unwrap()
                            .with_miter_limit(limit)
                            .unwrap();
                        runtime
                            .set_contents(Box::new(StrokeFigure {
                                path: path.clone(),
                                stroke: stroke.clone(),
                                aa,
                            }))
                            .unwrap();
                        let low = gpu.render(&mut runtime, true);
                        runtime
                            .set_contents(Box::new(StrokeFigure {
                                path: path.clone(),
                                stroke: stroke.with_join(LineJoin::Bevel),
                                aa,
                            }))
                            .unwrap();
                        let bevel = gpu.render(&mut runtime, true);
                        assert_eq!(
                            low, bevel,
                            "aa={aa:?} closed={closed} reverse={reverse} limit={limit}"
                        );
                        assert!(low.chunks_exact(4).any(|pixel| pixel[3] != 0));
                    }
                }
            }
        }
    }
}
