//! Independent K08 experiment; this is not a production OFF implementation.
//! Run: cargo run -p novadraw-backend-vello --example shape_aa_feasibility
use std::{error::Error, path::Path, time::Instant};
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene,
    kurbo::{Affine, BezPath, Cap, Circle, Point, Rect, Shape, Stroke, StrokeOpts, stroke},
    peniko::{BlendMode, Color, Fill, Gradient},
    wgpu,
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const ROW_BYTES: u32 = WIDTH * 4;
const OUTPUT: &str = "target/verification/shape-aa-feasibility";

// Deliberately bounded brute-force oracle. A production implementation needs
// scanline crossings, explicit budgets and checked device-space conversion.
fn center_sample_runs(path: &BezPath) -> (BezPath, usize) {
    let mut result = BezPath::new();
    let mut runs = 0;
    for y in 0..HEIGHT {
        let mut start = None;
        for x in 0..=WIDTH {
            let inside =
                x < WIDTH && path.winding(Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5)) != 0;
            if inside {
                start.get_or_insert(x);
            } else if let Some(left) = start.take() {
                result.extend(
                    Rect::new(
                        f64::from(left),
                        f64::from(y),
                        f64::from(x),
                        f64::from(y + 1),
                    )
                    .path_elements(0.1),
                );
                runs += 1;
            }
        }
    }
    (result, runs)
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    texture: wgpu::Texture,
}

impl Gpu {
    fn new() -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        println!("adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
        let renderer = Renderer::new(&device, RendererOptions::default())?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("K08 center sampling experiment"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self {
            device,
            queue,
            renderer,
            texture,
        })
    }

    fn render(&mut self, scene: &Scene, name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        self.renderer.render_to_texture(
            &self.device,
            &self.queue,
            scene,
            &self.texture.create_view(&Default::default()),
            &RenderParams {
                base_color: Color::TRANSPARENT,
                width: WIDTH,
                height: HEIGHT,
                antialiasing_method: AaConfig::Area,
            },
        )?;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("K08 readback"),
            size: u64::from(ROW_BYTES * HEIGHT),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ROW_BYTES),
                    rows_per_image: Some(HEIGHT),
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |value| {
            tx.send(value).expect("readback receiver");
        });
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })?;
        rx.recv()??;
        let pixels = buffer.get_mapped_range(..).to_vec();
        std::fs::write(Path::new(OUTPUT).join(format!("{name}.rgba")), &pixels)?;
        Ok(pixels)
    }
}

fn scene(path: &BezPath, color: Color) -> Scene {
    let mut result = Scene::new();
    result.fill(Fill::NonZero, Affine::IDENTITY, color, None, path);
    result
}

fn main() -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(OUTPUT)?;
    let mut gpu = Gpu::new()?;
    let original = Affine::new([1.1, 0.2, -0.15, 0.9, 2.25, 1.75])
        * Circle::new((29.0, 29.0), 20.0).to_path(0.01);
    let start = Instant::now();
    let (binary, runs) = center_sample_runs(&original);
    let mask_cpu_micros = start.elapsed().as_micros();
    let on = gpu.render(&scene(&original, Color::WHITE), "on")?;
    let off = gpu.render(&scene(&binary, Color::WHITE), "off")?;
    let partial_on = on
        .chunks_exact(4)
        .filter(|p| p[3] > 0 && p[3] < 255)
        .count();
    assert!(
        partial_on > 0,
        "Area must have fractional geometric coverage"
    );
    for (i, pixel) in off.chunks_exact(4).enumerate() {
        let x = (i as u32) % WIDTH;
        let y = (i as u32) / WIDTH;
        let inside = original.winding(Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5)) != 0;
        assert_eq!(
            pixel[3],
            if inside { 255 } else { 0 },
            "center sample at {x},{y}"
        );
    }
    let translucent = gpu.render(
        &scene(&binary, Color::from_rgba8(255, 255, 255, 128)),
        "off-alpha",
    )?;
    for (mask, pixel) in off.chunks_exact(4).zip(translucent.chunks_exact(4)) {
        assert_eq!(pixel[3], if mask[3] == 255 { 128 } else { 0 });
    }

    let gradient =
        Gradient::new_linear((0.0, 0.0), (64.0, 64.0)).with_stops([Color::BLACK, Color::WHITE]);
    let gradient_scene = |path: &BezPath| {
        let mut result = Scene::new();
        result.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &gradient,
            Some(Affine::new([1.2, 0.1, -0.2, 0.9, 3.25, 5.75])),
            path,
        );
        result
    };
    let gradient_on = gpu.render(&gradient_scene(&original), "gradient-on")?;
    let gradient_off = gpu.render(&gradient_scene(&binary), "gradient-off")?;
    let mut gradient_samples = 0;
    for (a, b) in gradient_on
        .chunks_exact(4)
        .zip(gradient_off.chunks_exact(4))
    {
        if a[3] == 255 && b[3] == 255 {
            assert!(a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 1));
            gradient_samples += 1;
        }
    }
    assert!(gradient_samples > 0);

    let mut curve = BezPath::new();
    curve.move_to((8.25, 10.75));
    curve.curve_to((52.0, 3.0), (2.0, 48.0), (51.75, 52.25));
    let outline = Affine::new([0.9, 0.15, -0.1, 1.0, 4.25, -0.75])
        * stroke(
            curve.iter(),
            &Stroke::new(3.0)
                .with_caps(Cap::Round)
                .with_dashes(1.25, [5.0, 3.0]),
            &StrokeOpts::default(),
            0.01,
        );
    let (stroke_binary, stroke_runs) = center_sample_runs(&outline);
    let stroke_pixels = gpu.render(&scene(&stroke_binary, Color::WHITE), "stroke-off")?;
    for (i, p) in stroke_pixels.chunks_exact(4).enumerate() {
        let point = Point::new(
            f64::from(i as u32 % WIDTH) + 0.5,
            f64::from(i as u32 / WIDTH) + 0.5,
        );
        assert_eq!(p[3], if outline.winding(point) != 0 { 255 } else { 0 });
    }

    // One binary path and individual integer runs must agree under a curved AA
    // clip with mixed ON/OFF ordering and alpha. This checks layer integration.
    let clip = Circle::new((32.25, 32.75), 16.2);
    let mixed = |split: bool| {
        let mut result = scene(&original, Color::from_rgba8(0, 0, 255, 128));
        result.push_layer(
            Fill::NonZero,
            BlendMode::default(),
            0.75,
            Affine::IDENTITY,
            &clip,
        );
        if split {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    if off[((y * WIDTH + x) * 4 + 3) as usize] == 255 {
                        result.fill(
                            Fill::NonZero,
                            Affine::IDENTITY,
                            Color::from_rgba8(255, 0, 0, 128),
                            None,
                            &Rect::new(
                                f64::from(x),
                                f64::from(y),
                                f64::from(x + 1),
                                f64::from(y + 1),
                            ),
                        );
                    }
                }
            }
        } else {
            result.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(255, 0, 0, 128),
                None,
                &binary,
            );
        }
        result.pop_layer();
        result.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(0, 255, 0, 128),
            None,
            &Rect::new(20.25, 20.75, 42.25, 42.75),
        );
        result
    };
    let combined = gpu.render(&mixed(false), "mixed")?;
    let reference = gpu.render(&mixed(true), "mixed-reference")?;
    let max_delta = combined
        .iter()
        .zip(&reference)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(
        max_delta <= 1,
        "integer cells must agree with combined runs: {max_delta}"
    );
    std::fs::write(
        Path::new(OUTPUT).join("report.json"),
        format!(
            "{{\"status\":\"passed\",\"width\":{WIDTH},\"height\":{HEIGHT},\
             \"runs\":{runs},\"mask_cpu_micros\":{mask_cpu_micros},\
             \"on_partial_pixels\":{partial_on},\"off_center_samples\":{},\
             \"gradient_interior_samples\":{gradient_samples},\"stroke_runs\":{stroke_runs},\
             \"mixed_max_channel_delta\":{max_delta},\
             \"scope\":\"native-area-transformed-circle-dashed-curve-gradient-alpha-curved-clip-order; not production\"}}\n",
            WIDTH * HEIGHT
        ),
    )?;
    println!("passed: {runs} runs, {partial_on} ON partial pixels, mixed delta {max_delta}");
    Ok(())
}
