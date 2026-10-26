//! ShapeStylesExample Canvas consumer (EPL-2.0 reference).
//! Axis order matters: the Y setting overrides X when both select the same axis.
use super::runtime_with_fonts;
use novadraw::{
    figure::{Figure, LabelFigure, RectangleFigure},
    geometry::{Point, Rectangle},
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
    fn unsupported(&self) -> Option<&'static str> {
        if self.width == 0.0 {
            Some("Unsupported:\nhairline (0)")
        } else if self.miter < 1.0 {
            Some("Unsupported:\nmiter < 1")
        } else {
            None
        }
    }
    fn stroke(&self) -> StrokeStyle {
        // SWT Cocoa standard dash lengths scale with the final line width.
        let pattern: &[f64] = match self.dash {
            0 => &[],
            1 => &[3.0, 1.0],
            2 => &[3.0, 1.0, 1.0, 1.0],
            3 => &[3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            4 => &[1.0, 1.0],
            _ => unreachable!("upstream line style"),
        };
        let dash = if pattern.is_empty() {
            DashPattern::Solid
        } else {
            DashPattern::Custom(
                CustomDash::try_new(&pattern.iter().map(|v| v * self.width).collect::<Vec<_>>())
                    .expect("positive dash"),
            )
        };
        StrokeStyle::default()
            .with_width(self.width)
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
        context.set_opacity(f64::from(self.style.alpha) / 255.0)?;
        context.set_fill_paint(Color::rgba(1.0, 200.0 / 255.0, 0.0, 1.0));
        context.set_stroke_paint(Color::BLACK);
        context.set_stroke_style(self.style.stroke());
        let inset = self.style.width.max(1.0) / 2.0;
        let rect = Rectangle::new(
            inset,
            inset,
            SHAPE_WIDTH - inset * 2.0,
            SHAPE_HEIGHT - inset * 2.0,
        );
        match self.shape {
            0 => {
                context.fill_ellipse(rect)?;
                context.stroke_ellipse(rect)
            }
            1 => {
                context.fill_rect(rect)?;
                context.stroke_rect(rect)
            }
            4 => context.stroke_polyline(&[
                Point::new(20.0, 20.0),
                Point::new(50.0, 80.0),
                Point::new(10.0, 50.0),
            ]),
            shape => {
                let mut path = Path::new();
                if shape == 2 {
                    let radius = (8.0 - inset / 2.0).max(0.0);
                    let (x, y, w, h) = (rect.x, rect.y, rect.width, rect.height);
                    path.move_to(x + radius, y);
                    path.line_to(x + w - radius, y);
                    path.quad_to(x + w, y, x + w, y + radius);
                    path.line_to(x + w, y + h - radius);
                    path.quad_to(x + w, y + h, x + w - radius, y + h);
                    path.line_to(x + radius, y + h);
                    path.quad_to(x, y + h, x, y + h - radius);
                    path.line_to(x, y + radius);
                    path.quad_to(x, y, x + radius, y);
                } else {
                    let size = rect.width.min(rect.height) / 2.0;
                    path.move_to(rect.x + rect.width / 2.0, rect.y);
                    path.line_to(rect.x + rect.width / 2.0 + size, rect.y + size * 2.0);
                    path.line_to(rect.x + rect.width / 2.0 - size, rect.y + size * 2.0);
                }
                path.close();
                context.fill_path(&path)?;
                context.stroke_path(&path)
            }
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
            if let Some(reason) = style.unsupported() {
                label(runtime, root, reason.into(), bounds);
            } else {
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
                ui.label("Hairline and miter < 1 remain unsupported.");
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
    use novadraw::render::{BackendCapabilities, SurfaceInfo};
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
        assert!(style.unsupported().unwrap().contains("hairline"));
        style.apply(0, 5);
        style.apply(3, 1);
        assert!(style.unsupported().unwrap().contains("miter"));
        style.apply(3, 5);
        style.apply(5, 0);
        style.apply(5, 1);
        assert_eq!(style.aa, AntialiasMode::Off);
    }
}
