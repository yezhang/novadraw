#![cfg(feature = "gpu")]

use winit::dpi::PhysicalSize;
use winit_canvas::{
    geometry::{CanvasRect, WindowMetrics},
    gpu::{CANVAS_FORMAT, CanvasCompositor, CanvasLayer, CanvasTarget},
};

#[test]
#[ignore = "requires a native GPU adapter"]
fn independent_textures_compose_in_order_with_clip_alpha_and_old_area_erasure() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let metrics = WindowMetrics::new(PhysicalSize::new(64, 32), 2.0).unwrap();
    let left_rect = CanvasRect::new(-4.0, 4.0, 16.0, 8.0).unwrap();
    let right_rect = CanvasRect::new(8.0, 4.0, 12.0, 8.0).unwrap();
    let left = CanvasTarget::new(&device, left_rect, metrics)
        .unwrap()
        .unwrap();
    let right = CanvasTarget::new(&device, right_rect, metrics)
        .unwrap()
        .unwrap();
    assert_eq!(left.size(), [32, 16]);
    queue.write_texture(
        left.texture().as_image_copy(),
        &[255, 0, 0, 255].repeat(32 * 16),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(32 * 4),
            rows_per_image: Some(16),
        },
        left.texture().size(),
    );
    queue.write_texture(
        right.texture().as_image_copy(),
        &[0, 128, 0, 128].repeat(24 * 16),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(24 * 4),
            rows_per_image: Some(16),
        },
        right.texture().size(),
    );
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("compositor test output"),
        size: wgpu::Extent3d {
            width: 64,
            height: 32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: CANVAS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = output.create_view(&Default::default());
    let compositor = CanvasCompositor::new(&device, CANVAS_FORMAT);
    for (erase, layers) in [
        (
            false,
            vec![
                CanvasLayer {
                    target: &left,
                    rect: left_rect,
                },
                CanvasLayer {
                    target: &right,
                    rect: right_rect,
                },
            ],
        ),
        (true, Vec::new()),
    ] {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("canvas readback"),
            size: 256 * 32,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        compositor
            .compose(
                &device,
                &mut encoder,
                &view,
                metrics,
                wgpu::Color::BLACK,
                &layers,
            )
            .unwrap();
        encoder.copy_texture_to_buffer(
            output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(32),
                },
            },
            output.size(),
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
        let bytes = buffer.get_mapped_range(..);
        let pixel = |x: usize, y: usize| &bytes[y * 256 + x * 4..y * 256 + x * 4 + 4];
        assert_eq!(
            pixel(10, 2),
            &[0, 0, 0, 255],
            "content must be clipped vertically"
        );
        if erase {
            assert_eq!(
                pixel(10, 12),
                &[0, 0, 0, 255],
                "old canvas area must be erased"
            );
        } else {
            assert_eq!(
                pixel(0, 12),
                &[255, 0, 0, 255],
                "negative origin must be clipped by the window"
            );
            let overlap = pixel(18, 12);
            assert!((i16::from(overlap[0]) - 127).abs() <= 1);
            assert_eq!(
                &overlap[1..],
                &[128, 0, 255],
                "upper canvas must use premultiplied alpha"
            );
            assert_eq!(
                pixel(48, 12),
                &[0, 0, 0, 255],
                "canvas must not spill outside its bounds"
            );
        }
        drop(bytes);
        buffer.unmap();
    }
}
