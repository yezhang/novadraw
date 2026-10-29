use novadraw::{
    figure::RectangleFigure,
    geometry::Rectangle,
    graphics::Color,
    render::{RenderBackend, RenderDisposition, RenderFailure, SurfaceInfo},
    runtime::{FramePreparation, Runtime, SubmissionAcknowledgement},
    tree::FigureTree,
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use vello::wgpu;

#[test]
#[ignore = "requires a native GPU adapter"]
fn texture_backend_rebuilds_full_frames_validates_targets_and_rejects_stale_sessions() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
    let mut tree = FigureTree::new();
    let root = tree
        .builder()
        .set_contents(Box::new(
            RectangleFigure::new_with_color(0.0, 0.0, 32.0, 16.0, Color::rgba(0.0, 0.0, 0.0, 1.0))
                .expect("valid figure bounds"),
        ))
        .unwrap();
    let shape = tree
        .builder()
        .add_child(
            root,
            Box::new(
                RectangleFigure::new_with_color(
                    2.0,
                    2.0,
                    6.0,
                    6.0,
                    Color::rgba(1.0, 0.0, 0.0, 1.0),
                )
                .expect("valid figure bounds"),
            ),
        )
        .unwrap();
    let mut runtime = Runtime::new(tree);
    let surface = SurfaceInfo {
        pixel_width: 64,
        pixel_height: 32,
        scale_factor: 2.0,
        logical_width: 32.0,
        logical_height: 16.0,
    };
    let make_texture = |width| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("external target contract"),
            size: wgpu::Extent3d {
                width,
                height: 32,
                depth_or_array_layers: 1,
            },
            // Application targets may have mips; the backend writes only level zero.
            mip_level_count: 2,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let target = make_texture(64);
    let wrong_target = make_texture(32);
    let FramePreparation::Ready(first) = runtime
        .frames()
        .prepare_submission(surface, renderer.bind(&target).capabilities())
    else {
        panic!("expected first frame")
    };
    assert!(matches!(
        renderer.bind(&wrong_target).submit(&first),
        Err(RenderFailure::Fatal(_))
    ));
    let result = renderer.bind(&target).submit(&first);
    assert_eq!(result, Ok(RenderDisposition::Presented));
    assert_eq!(
        runtime
            .frames()
            .complete_submission(first.session_id, first.frame_id, result)
            .unwrap(),
        SubmissionAcknowledgement::Presented
    );
    let first_pixels = read_pixels(&device, &queue, &target);
    assert_eq!(pixel(&first_pixels, 8, 8), [255, 0, 0, 255]);
    runtime
        .figure(shape)
        .unwrap()
        .set_bounds(Rectangle::new(18.0, 2.0, 6.0, 6.0))
        .unwrap();
    let FramePreparation::Ready(mut second) = runtime
        .frames()
        .prepare_submission(surface, renderer.bind(&target).capabilities())
    else {
        panic!("expected second frame")
    };
    second.damage.set_union(Rectangle::new(18.0, 2.0, 6.0, 6.0));
    assert!(matches!(
        renderer.bind(&target).submit(&second),
        Err(RenderFailure::Fatal(_))
    ));
    second.damage.set_full();
    let result = renderer.bind(&target).submit(&second);
    assert_eq!(result, Ok(RenderDisposition::Presented));
    assert_eq!(
        runtime
            .frames()
            .complete_submission(second.session_id, second.frame_id, result)
            .unwrap(),
        SubmissionAcknowledgement::Presented
    );
    let second_pixels = read_pixels(&device, &queue, &target);
    assert_eq!(
        pixel(&second_pixels, 8, 8),
        [0, 0, 0, 255],
        "old location must be erased"
    );
    assert_eq!(pixel(&second_pixels, 40, 8), [255, 0, 0, 255]);
    runtime.frames().reset_backend_session().unwrap();
    let FramePreparation::Ready(replacement) = runtime
        .frames()
        .prepare_submission(surface, renderer.bind(&target).capabilities())
    else {
        panic!("expected replacement session")
    };
    assert_eq!(
        renderer.bind(&target).submit(&replacement),
        Ok(RenderDisposition::Presented)
    );
    assert!(matches!(
        renderer.bind(&target).submit(&first),
        Err(RenderFailure::Fatal(_))
    ));
}

fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    bytes[y * 256 + x * 4..y * 256 + x * 4 + 4]
        .try_into()
        .unwrap()
}

fn read_pixels(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("external target pixels"),
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
