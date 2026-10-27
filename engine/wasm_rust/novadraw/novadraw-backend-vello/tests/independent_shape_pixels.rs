use novadraw::{
    figure::{EdgeStroke, RectangleCorners, RectangleEdges, RoundedRectangleFigure, ShapeContour},
    geometry::{Dimension, Point, Rectangle},
    graphics::Color,
    render::{RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::{Runtime, SetRectangleEdges},
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use vello::wgpu;

#[test]
#[ignore = "requires a native GPU adapter"]
fn corner_color_seams_preserve_alpha_and_disabled_edge_clears_pixels() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
    for (size, height, scale, mode) in [
        (64.0, 64.0, 1.0, 0),
        (64.0, 64.0, 1.5, 0),
        (64.0, 64.0, 2.0, 0),
        (800.0, 800.0, 1.0, 0),
        (800.0, 800.0, 2.0, 0),
        (228.0, 158.0, 1.0, 1),
        (228.0, 158.0, 1.5, 1),
        (228.0, 158.0, 2.0, 1),
        (228.0, 158.0, 1.0, 2),
        (228.0, 158.0, 1.5, 2),
        (228.0, 158.0, 2.0, 2),
    ] {
        let asymmetric = mode != 0;
        let radii = if asymmetric {
            [30.0, 12.0, 0.0, 0.0]
        } else {
            [20.0; 4]
        };
        let mut dimensions = radii.map(|r| Dimension::new(r, r));
        if mode == 2 {
            dimensions[0].height = 18.0;
        }
        let corners = RectangleCorners::new(dimensions).unwrap();
        let surface = SurfaceInfo {
            logical_width: size,
            logical_height: height,
            pixel_width: (size * scale) as u32,
            pixel_height: (height * scale) as u32,
            scale_factor: scale,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("independent edge evidence"),
            size: wgpu::Extent3d {
                width: surface.pixel_width,
                height: surface.pixel_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut runtime = Runtime::empty();
        let red = Color::rgba8(255, 0, 0, 128);
        let blue = Color::rgba8(0, 0, 255, 128);
        let figure = RoundedRectangleFigure::new_with_color(
            4.0,
            4.0,
            size - 8.0,
            height - 8.0,
            0.0,
            Color::TRANSPARENT,
        )
        .with_stroke(red, 10.0)
        .with_corners(corners)
        .unwrap()
        .with_edges(RectangleEdges([
            EdgeStroke::Color(red),
            EdgeStroke::Color(blue),
            EdgeStroke::Color(red),
            EdgeStroke::Color(blue),
        ]))
        .unwrap();
        let root = runtime.set_contents(Box::new(figure)).unwrap();
        let first = render(
            &mut runtime,
            &mut renderer,
            &texture,
            surface,
            &device,
            &queue,
        );
        // Center of the wide corner ring lies on the color split, far from either contour.
        for logical in if mode == 2 {
            &[][..]
        } else if asymmetric {
            &[16.0, 17.0, 18.0][..]
        } else {
            &[11.0, 12.0, 13.0][..]
        } {
            let coordinate = (logical * scale) as u32;
            let alpha = pixel(&first, surface.pixel_width, coordinate, coordinate)[3];
            assert!(
                (127..=129).contains(&alpha),
                "shared corner seam alpha={alpha}, scale={scale}, point={coordinate}"
            );
        }
        let outer =
            ShapeContour::rectangle(Rectangle::new(4.0, 4.0, size - 8.0, height - 8.0), corners)
                .unwrap();
        let inner = ShapeContour::rectangle(
            Rectangle::new(14.0, 14.0, size - 28.0, height - 28.0),
            RectangleCorners::new(
                dimensions
                    .map(|r| Dimension::new((r.width - 10.0).max(0.0), (r.height - 10.0).max(0.0))),
            )
            .unwrap(),
        )
        .unwrap();
        // Check all centers inside the top-left ring, excluding its AA contour boundary.
        for y in 0..((radii[0] + 16.0) * scale) as u32 {
            for x in 0..((radii[0] + 16.0) * scale) as u32 {
                let interior = [-2.0, 0.0, 2.0].into_iter().all(|dy| {
                    [-2.0, 0.0, 2.0].into_iter().all(|dx| {
                        let p = Point::new(
                            (f64::from(x) + 0.5 + dx) / scale,
                            (f64::from(y) + 0.5 + dy) / scale,
                        );
                        outer.contains(p) && !inner.contains(p)
                    })
                });
                if interior {
                    let alpha = pixel(&first, surface.pixel_width, x, y)[3];
                    assert!(
                        (127..=129).contains(&alpha),
                        "ring seam alpha={alpha}, x={x}, y={y}, scale={scale}, asymmetric={asymmetric}"
                    );
                }
            }
        }
        let bottom = (size / 2.0 * scale) as u32;
        let y = ((height - 7.0) * scale) as u32;
        assert_eq!(pixel(&first, surface.pixel_width, bottom, y)[3], 128);
        runtime
            .figure(root)
            .unwrap()
            .update_component(SetRectangleEdges::new(RectangleEdges([
                EdgeStroke::Color(red),
                EdgeStroke::Color(blue),
                EdgeStroke::None,
                EdgeStroke::Color(blue),
            ])))
            .unwrap();
        let after = render(
            &mut runtime,
            &mut renderer,
            &texture,
            surface,
            &device,
            &queue,
        );
        assert_eq!(pixel(&after, surface.pixel_width, bottom, y)[3], 0);
        assert_eq!(pixel(&after, surface.pixel_width, 0, 0), [0; 4]);
        if asymmetric {
            let boundary_y = ((height - 4.0) * scale) as u32;
            // Every pixel across both side widths reaches the exact bottom,
            // including the inner endpoints formerly removed by the bevel.
            for (start, end) in [(4.0, 14.0), (size - 14.0, size - 4.0)] {
                for x in (start * scale) as u32..(end * scale) as u32 {
                    assert_eq!(
                        pixel(&after, surface.pixel_width, x, boundary_y - 1)[3],
                        128,
                        "free endpoint x={x}, scale={scale}"
                    );
                    assert_eq!(
                        pixel(&after, surface.pixel_width, x, boundary_y)[3],
                        0,
                        "endpoint extends past bounds x={x}, scale={scale}"
                    );
                }
            }
        }
        // Same geometry and paint: exercise the stable backend preparation cache.
        let warm = render(
            &mut runtime,
            &mut renderer,
            &texture,
            surface,
            &device,
            &queue,
        );
        assert_eq!(warm, after);
        #[cfg(feature = "native")]
        if mode == 1 && scale == 2.0 {
            let mut rgba = after.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                if pixel[3] > 0 {
                    for channel in 0..3 {
                        pixel[channel] =
                            (u32::from(pixel[channel]) * 255 / u32::from(pixel[3])).min(255) as u8;
                    }
                }
            }
            let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("target/shape-artifacts/independent-corner-butt-gpu.png");
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            image::save_buffer(
                output,
                &rgba,
                surface.pixel_width,
                surface.pixel_height,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }
}

fn render(
    runtime: &mut Runtime,
    renderer: &mut VelloTextureRenderer,
    texture: &wgpu::Texture,
    surface: SurfaceInfo,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Vec<u8> {
    let start = std::time::Instant::now();
    let mut backend = renderer.bind(texture);
    runtime.frames().request_full_redraw();
    let frame = runtime
        .frames()
        .prepare_submission(surface, backend.capabilities())
        .into_ready()
        .unwrap();
    let core_us = start.elapsed().as_secs_f64() * 1e6;
    let submit_start = std::time::Instant::now();
    let result = backend.submit(&frame);
    let submit_us = submit_start.elapsed().as_secs_f64() * 1e6;
    assert_eq!(result, Ok(RenderDisposition::Presented));
    println!(
        "{}px: Core prepare={core_us:.1} us, backend submit={submit_us:.1} us (GPU readback excluded)",
        surface.pixel_width
    );
    assert_eq!(
        runtime
            .frames()
            .complete_submission(frame.session_id, frame.frame_id, result)
            .unwrap(),
        novadraw::runtime::SubmissionAcknowledgement::Presented
    );
    let row = (surface.pixel_width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("independent edge readback"),
        size: u64::from(row * surface.pixel_height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(surface.pixel_height),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |result| {
        tx.send(result).unwrap()
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = buffer.get_mapped_range(..);
    data.chunks(row as usize)
        .flat_map(|r| r[..surface.pixel_width as usize * 4].iter().copied())
        .collect()
}

fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    bytes[index..index + 4].try_into().unwrap()
}
