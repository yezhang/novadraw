use novadraw::{
    figure::RectangleFigure,
    geometry::Rectangle,
    graphics::{AntialiasMode, Color},
    render::{
        ImageResourceRef, RenderBackend, RenderCommand, RenderCommandKind, RenderDisposition,
        RenderFailure, RenderSubmission, ResourceId, ResourceSnapshot, ResourceSync, SurfaceInfo,
    },
    runtime::{FramePreparation, Runtime},
};
use novadraw_backend_vello::{
    AntialiasConfig,
    scene::{Scene, VelloSceneRenderer},
};
use vello::kurbo::{Affine, Rect};
use vello::peniko::Fill;

fn surface(scale: f64) -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 32.0,
        logical_height: 16.0,
        pixel_width: (32.0 * scale) as u32,
        pixel_height: (16.0 * scale) as u32,
        scale_factor: scale,
    }
}

fn fixture() -> Runtime {
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(
            RectangleFigure::new_with_color(2.0, 2.0, 6.0, 6.0, Color::RED)
                .expect("valid figure bounds"),
        ))
        .unwrap();
    runtime
}

fn submission(
    runtime: &mut Runtime,
    renderer: &mut VelloSceneRenderer,
    target: &mut Scene,
    scale: f64,
) -> RenderSubmission {
    let FramePreparation::Ready(submission) = runtime
        .frames()
        .prepare_submission(surface(scale), renderer.bind(target).capabilities())
    else {
        panic!("expected a full submission")
    };
    submission
}

fn seed() -> Scene {
    let mut target = Scene::new();
    target.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        vello::peniko::Color::from_rgb8(0, 255, 0),
        None,
        &Rect::new(3.0, 4.0, 5.0, 6.0),
    );
    target
}

fn fingerprint(scene: &Scene) -> (Vec<u32>, Vec<u32>, u32, u32) {
    let encoding = scene.encoding();
    (
        encoding.path_data.clone(),
        encoding.draw_data.clone(),
        encoding.n_paths,
        encoding.n_clips,
    )
}

#[test]
fn invalid_skipped_and_stale_submissions_preserve_the_target() {
    let mut runtime = fixture();
    let mut renderer = VelloSceneRenderer::new();
    let mut target = seed();
    let first = submission(&mut runtime, &mut renderer, &mut target, 2.0);
    let old = fingerprint(&target);
    let mut invalid = first.clone();
    invalid.damage.set_union(Rectangle::new(2.0, 2.0, 6.0, 6.0));
    assert!(matches!(
        renderer.bind(&mut target).submit(&invalid),
        Err(RenderFailure::Fatal(_))
    ));
    assert_eq!(fingerprint(&target), old);
    invalid = first.clone();
    invalid.surface.scale_factor = f64::NAN;
    assert!(renderer.bind(&mut target).submit(&invalid).is_err());
    assert_eq!(fingerprint(&target), old);
    invalid = first.clone();
    invalid.commands.clear();
    invalid.surface.scale_factor = 1.0e-40;
    assert!(renderer.bind(&mut target).submit(&invalid).is_err());
    assert_eq!(fingerprint(&target), old);
    invalid = first.clone();
    invalid.surface.pixel_width = 0;
    assert_eq!(
        renderer.bind(&mut target).submit(&invalid),
        Ok(RenderDisposition::Skipped)
    );
    assert_eq!(fingerprint(&target), old);
    assert_eq!(
        renderer.bind(&mut target).submit(&first),
        Ok(RenderDisposition::Presented)
    );
    assert_ne!(fingerprint(&target), old);
    let stable = fingerprint(&target);
    // A second submission replaces the scene instead of appending another frame.
    assert_eq!(
        renderer.bind(&mut target).submit(&first),
        Ok(RenderDisposition::Presented)
    );
    assert_eq!(fingerprint(&target), stable);
    let mut replacement = first.clone();
    replacement.session_id = first.session_id.next().unwrap();
    replacement.resources = ResourceSync::Snapshot(ResourceSnapshot::default());
    assert_eq!(
        renderer.bind(&mut target).submit(&replacement),
        Ok(RenderDisposition::Presented)
    );
    assert!(matches!(
        renderer.bind(&mut target).submit(&first),
        Err(RenderFailure::Fatal(_))
    ));
    assert_eq!(fingerprint(&target), stable);
}

#[test]
fn missing_resources_and_budget_failure_do_not_publish_partial_scenes() {
    let mut runtime = fixture();
    let mut renderer = VelloSceneRenderer::new();
    let mut target = seed();
    let first = submission(&mut runtime, &mut renderer, &mut target, 1.0);
    let old = fingerprint(&target);
    let mut missing = first.clone();
    missing.commands.push(RenderCommand {
        kind: RenderCommandKind::Image {
            image: ImageResourceRef::new(
                ResourceId::new(first.session_id.runtime_namespace(), 100),
                1,
                1,
                1,
                1.0,
            ),
            source_rect: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            dest_rect: Rectangle::new(10.0, 10.0, 1.0, 1.0),
            alpha: 1.0,
        },
    });
    assert_eq!(
        renderer.bind(&mut target).submit(&missing),
        Ok(RenderDisposition::Retry)
    );
    assert_eq!(fingerprint(&target), old);
    let mut off = first.clone();
    for command in &mut off.commands {
        if let RenderCommandKind::FillRect { antialias, .. } = &mut command.kind {
            *antialias = AntialiasMode::Off;
        }
    }
    let mut config = AntialiasConfig::default();
    config.frame.elements = 0;
    renderer.set_antialias_config(config);
    assert!(renderer.bind(&mut target).submit(&off).is_err());
    assert_eq!(fingerprint(&target), old);
    renderer.set_antialias_config(AntialiasConfig::default());
    assert_eq!(
        renderer.bind(&mut target).submit(&first),
        Ok(RenderDisposition::Presented)
    );
}

#[test]
fn replacing_backend_session_requires_a_snapshot() {
    let mut runtime = fixture();
    let mut renderer = VelloSceneRenderer::new();
    let mut target = seed();
    let first = submission(&mut runtime, &mut renderer, &mut target, 1.0);
    assert_eq!(
        renderer.bind(&mut target).submit(&first),
        Ok(RenderDisposition::Presented)
    );
    let old = fingerprint(&target);
    let mut replacement = first.clone();
    replacement.session_id = first.session_id.next().unwrap();
    replacement.resources = ResourceSync::default();
    assert!(renderer.bind(&mut target).submit(&replacement).is_err());
    assert_eq!(fingerprint(&target), old);
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn logical_scene_composition_matches_texture_pixels_at_multiple_dpi() {
    use novadraw_backend_vello::texture::VelloTextureRenderer;
    use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, wgpu};

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut gpu = Renderer::new(
        &device,
        RendererOptions {
            antialiasing_support: AaSupport::all(),
            ..Default::default()
        },
    )
    .unwrap();
    for scale in [1.0, 1.5, 2.0] {
        for clear in [None, Some(Color::BLUE)] {
            let mut runtime = fixture();
            let mut encoder = VelloSceneRenderer::new();
            let mut scene = Scene::new();
            let mut frame = submission(&mut runtime, &mut encoder, &mut scene, scale);
            frame.clear_color = clear;
            // An out-of-bounds draw must remain clipped after host composition.
            frame.commands.push(RenderCommand {
                kind: RenderCommandKind::FillRect {
                    antialias: AntialiasMode::Off,
                    rect: Rectangle::new(30.0, 12.0, 20.0, 20.0),
                    paint: Color::RED.into(),
                    rule: novadraw::graphics::FillRule::NonZero,
                },
            });
            let reference = texture(
                &device,
                frame.surface.pixel_width,
                frame.surface.pixel_height,
            );
            let mut texture_renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
            for generation in 0..2 {
                if generation == 1 {
                    // Reuse both targets; the full replacement must erase the old location.
                    frame.frame_id = frame.frame_id.next();
                    frame.resources = ResourceSync::default();
                    for command in &mut frame.commands {
                        if let RenderCommandKind::FillRect { rect, .. } = &mut command.kind {
                            rect.x += 8.0;
                        }
                    }
                }
                assert_eq!(
                    texture_renderer.bind(&reference).submit(&frame),
                    Ok(RenderDisposition::Presented)
                );
                assert_eq!(
                    encoder.bind(&mut scene).submit(&frame),
                    Ok(RenderDisposition::Presented)
                );
                let offset = 8;
                let width = frame.surface.pixel_width + offset * 2;
                let height = frame.surface.pixel_height + offset * 2;
                let composite = texture(&device, width, height);
                let mut window = Scene::new();
                window.append(
                    &scene,
                    Some(
                        Affine::translate((f64::from(offset), f64::from(offset)))
                            * Affine::scale(scale),
                    ),
                );
                gpu.render_to_texture(
                    &device,
                    &queue,
                    &window,
                    &composite.create_view(&Default::default()),
                    &RenderParams {
                        base_color: vello::peniko::Color::TRANSPARENT,
                        width,
                        height,
                        antialiasing_method: AaConfig::Msaa16,
                    },
                )
                .unwrap();
                let expected = pixels(&device, &queue, &reference);
                let actual = pixels(&device, &queue, &composite);
                for y in 0..height {
                    for x in 0..width {
                        let inside = x >= offset
                            && y >= offset
                            && x < offset + frame.surface.pixel_width
                            && y < offset + frame.surface.pixel_height;
                        let reference_pixel = if inside {
                            pixel(&expected, frame.surface.pixel_width, x - offset, y - offset)
                        } else {
                            [0; 4]
                        };
                        assert_eq!(
                            pixel(&actual, width, x, y),
                            reference_pixel,
                            "scale={scale}, clear={clear:?}, generation={generation}, x={x}, y={y}"
                        );
                    }
                }
            }
        }
    }
}

fn texture(device: &vello::wgpu::Device, width: u32, height: u32) -> vello::wgpu::Texture {
    use vello::wgpu;
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Scene consumer comparison"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn pixel(bytes: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    bytes[start..start + 4].try_into().unwrap()
}

fn pixels(
    device: &vello::wgpu::Device,
    queue: &vello::wgpu::Queue,
    texture: &vello::wgpu::Texture,
) -> Vec<u8> {
    use vello::wgpu;
    let stride = (texture.width() * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Scene consumer readback"),
        size: u64::from(stride) * u64::from(texture.height()),
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
                bytes_per_row: Some(stride),
                rows_per_image: Some(texture.height()),
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
    buffer
        .get_mapped_range(..)
        .chunks(stride as usize)
        .flat_map(|row| row[..texture.width() as usize * 4].iter().copied())
        .collect()
}
