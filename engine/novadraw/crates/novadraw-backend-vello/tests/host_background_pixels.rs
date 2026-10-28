use novadraw::{
    figure::RectangleFigure,
    geometry::{Affine2D, Rectangle},
    graphics::Color,
    host::{
        BackgroundFrameInput, BackgroundInputVersions, HostBackgroundContent, HostBackgroundId,
        HostBackgroundSnapshot,
    },
    render::{CommandRecorder, RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use vello::wgpu;

#[test]
#[ignore = "requires a native GPU adapter"]
fn background_pixels_follow_host_position_preserve_canvas_alpha_and_clear_after_removal() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host background evidence"),
        size: wgpu::Extent3d {
            width: 64,
            height: 32,
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
    let orange = Color::rgba8(255, 165, 0, 255);
    let root = runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            32.0,
            16.0,
            Color::TRANSPARENT,
        )))
        .unwrap();
    let opaque = runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            2.0, 2.0, 6.0, 6.0, orange,
        )))
        .unwrap();
    runtime
        .figure(root)
        .unwrap()
        .add(Box::new(RectangleFigure::new_with_color(
            10.0,
            2.0,
            6.0,
            6.0,
            Color::rgba8(255, 0, 0, 128),
        )))
        .unwrap();
    let mut recorder = CommandRecorder::new();
    let mut g = recorder.paint();
    g.set_fill_paint(Color::rgba8(0, 255, 0, 255));
    g.fill_rect(Rectangle::new(0.0, 0.0, 16.0, 16.0)).unwrap();
    g.set_fill_paint(Color::rgba8(0, 0, 255, 255));
    g.fill_rect(Rectangle::new(16.0, 0.0, 16.0, 16.0)).unwrap();
    drop(g);
    let content = HostBackgroundContent {
        coverage: Rectangle::new(0.0, 0.0, 32.0, 16.0),
        drawing: recorder.finish().unwrap(),
    };
    let mut input = BackgroundFrameInput {
        surface: SurfaceInfo {
            logical_width: 32.0,
            logical_height: 16.0,
            pixel_width: 64,
            pixel_height: 32,
            scale_factor: 2.0,
        },
        versions: BackgroundInputVersions {
            surface: 1,
            position: 1,
            dpi: 1,
        },
        host_to_canvas: Affine2D::IDENTITY,
    };
    let source = HostBackgroundId::new();
    let request = runtime
        .frames()
        .begin_background(source, input, Color::WHITE)
        .unwrap();
    runtime
        .frames()
        .complete_background(
            request,
            Ok(HostBackgroundSnapshot::new(request, 1, content.clone()).unwrap()),
        )
        .unwrap();
    submit(&mut runtime, &mut renderer, &target, input);
    let initial = read_pixels(&device, &queue, &target);
    assert_eq!(
        pixel(&initial, 8, 8),
        [255, 165, 0, 255],
        "opaque Canvas hides host"
    );
    assert_eq!(
        pixel(&initial, 40, 8),
        [0, 0, 255, 255],
        "transparent Canvas exposes host"
    );
    let alpha = pixel(&initial, 24, 8);
    assert!(
        (126..=130).contains(&alpha[0])
            && (126..=130).contains(&alpha[1])
            && alpha[2] == 0
            && alpha[3] == 255
    );
    // A content-only move cannot move the host background.
    runtime
        .figure(opaque)
        .unwrap()
        .set_bounds(Rectangle::new(20.0, 2.0, 6.0, 6.0))
        .unwrap();
    submit(&mut runtime, &mut renderer, &target, input);
    let scrolled = read_pixels(&device, &queue, &target);
    assert_eq!(pixel(&scrolled, 8, 8), [0, 255, 0, 255]);
    assert_eq!(pixel(&scrolled, 44, 8), [255, 165, 0, 255]);
    // Canvas placement samples another host location; content coordinates stay unchanged.
    input.versions.position += 1;
    input.host_to_canvas = Affine2D::from_translation(-16.0, 0.0);
    let request = runtime
        .frames()
        .begin_background(source, input, Color::WHITE)
        .unwrap();
    runtime
        .frames()
        .complete_background(
            request,
            Ok(HostBackgroundSnapshot::new(request, 2, content).unwrap()),
        )
        .unwrap();
    submit(&mut runtime, &mut renderer, &target, input);
    let moved = read_pixels(&device, &queue, &target);
    assert_eq!(pixel(&moved, 8, 8), [0, 0, 255, 255]);
    assert_eq!(
        pixel(&moved, 60, 8),
        [255, 255, 255, 255],
        "coverage leaves explicit clear outside"
    );
    runtime
        .frames()
        .remove_background(Color::TRANSPARENT)
        .unwrap();
    submit(&mut runtime, &mut renderer, &target, input);
    let removed = read_pixels(&device, &queue, &target);
    assert_eq!(
        pixel(&removed, 8, 8),
        [0, 0, 0, 0],
        "removed background must erase old pixels"
    );
    assert_eq!(pixel(&removed, 44, 8), [255, 165, 0, 255]);
}

fn submit(
    runtime: &mut Runtime,
    renderer: &mut VelloTextureRenderer,
    target: &wgpu::Texture,
    input: BackgroundFrameInput,
) {
    let mut backend = renderer.bind(target);
    let frame = runtime
        .frames()
        .prepare_submission(input.surface, backend.capabilities())
        .into_ready()
        .unwrap();
    let result = backend.submit(&frame);
    assert_eq!(result, Ok(RenderDisposition::Presented));
    assert_eq!(
        runtime
            .frames()
            .complete_submission(frame.session_id, frame.frame_id, result)
            .unwrap(),
        novadraw::runtime::SubmissionAcknowledgement::Presented
    );
}
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[y * 256 + x * 4..y * 256 + x * 4 + 4]
        .try_into()
        .unwrap()
}
fn read_pixels(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("host background readback"),
        size: 256 * 32,
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
                bytes_per_row: Some(256),
                rows_per_image: Some(32),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |result| {
        tx.send(result).unwrap();
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    buffer.get_mapped_range(..).to_vec()
}
