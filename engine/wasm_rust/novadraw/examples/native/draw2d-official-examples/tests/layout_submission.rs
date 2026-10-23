use novadraw::{
    geometry::Point,
    layout::{FlowDirection, FlowLayout, ToolbarLayout, ToolbarOrientation},
    render::{BackendCapabilities, DamageMode, RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::{Runtime, SubmissionAcknowledgement},
    tree::FigureId,
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use vello::wgpu;

fn scene(name: &str) -> (Runtime, FigureId) {
    let mut entry = draw2d_official_examples::panel_entries()
        .into_iter()
        .find(|entry| entry.0 == name)
        .unwrap();
    let (mut runtime, _) = entry.1();
    runtime
        .frames()
        .resize_logical_viewport(1200.0, 800.0)
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let root = runtime.tree().contents().unwrap();
    let content = runtime.tree().child_order(root).unwrap()[0];
    (runtime, content)
}

fn surface() -> SurfaceInfo {
    SurfaceInfo {
        logical_width: 1200.0,
        logical_height: 800.0,
        pixel_width: 1200,
        pixel_height: 800,
        scale_factor: 1.0,
    }
}

fn surface_center(runtime: &Runtime, figure: FigureId) -> Point {
    let bounds = runtime.tree().figure_bounds(figure).unwrap();
    runtime
        .tree()
        .local_to_surface_transform(figure)
        .unwrap()
        .transform_point(Point::new(bounds.width / 2.0, bounds.height / 2.0))
}

#[test]
fn canvas_minimum_extent_is_finite_and_resize_restores_all_child_geometry() {
    for (name, minimum_width) in [
        ("FlowLayoutExample", 300.0),
        ("ToolbarLayoutExample", 280.0),
        ("GridLayoutExample", 410.0),
    ] {
        let (mut runtime, content) = scene(name);
        let children = runtime.tree().child_order(content).unwrap();
        let initial: Vec<_> = children
            .iter()
            .map(|id| runtime.tree().figure_bounds(*id).unwrap())
            .collect();
        runtime
            .frames()
            .resize_logical_viewport(minimum_width, 40.0)
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let canvas = runtime.tree().figure_bounds(content).unwrap();
        assert_eq!((canvas.width, canvas.height), (0.0, 0.0));
        for id in &children {
            let bounds = runtime.tree().figure_bounds(*id).unwrap();
            assert!(bounds.x.is_finite() && bounds.y.is_finite());
            assert!(bounds.width.is_finite() && bounds.width >= 0.0);
            assert!(bounds.height.is_finite() && bounds.height >= 0.0);
        }
        runtime
            .frames()
            .resize_logical_viewport(1200.0, 800.0)
            .unwrap();
        runtime.frames().stabilize_for_query().unwrap();
        let restored: Vec<_> = children
            .iter()
            .map(|id| runtime.tree().figure_bounds(*id).unwrap())
            .collect();
        assert_eq!(
            initial, restored,
            "{name}: zero available extent cannot destroy preferred sizes"
        );
    }
}

fn orient(runtime: &mut Runtime, content: FigureId, name: &str, vertical: bool) {
    if name == "FlowLayoutExample" {
        runtime
            .figure(content)
            .unwrap()
            .set_layout_manager(Box::new(FlowLayout::with_direction(if vertical {
                FlowDirection::Vertical
            } else {
                FlowDirection::Horizontal
            })))
            .unwrap();
    } else {
        runtime
            .figure(content)
            .unwrap()
            .set_layout_manager(Box::new(
                ToolbarLayout::new()
                    .with_orientation(if vertical {
                        ToolbarOrientation::Vertical
                    } else {
                        ToolbarOrientation::Horizontal
                    })
                    .with_stretch_minor_axis(false),
            ))
            .unwrap();
    }
}

#[test]
fn layout_changes_damage_both_old_and_new_shape_positions() {
    for name in [
        "FlowLayoutExample",
        "ToolbarLayoutExample",
        "GridLayoutExample",
    ] {
        let (mut runtime, content) = scene(name);
        let capabilities = BackendCapabilities::RETAINED_PARTIAL;
        let initial = runtime
            .frames()
            .prepare_submission(surface(), capabilities)
            .into_ready()
            .unwrap();
        let acknowledgement = runtime
            .frames()
            .complete_submission(
                initial.session_id,
                initial.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        assert_eq!(acknowledgement, SubmissionAcknowledgement::Presented);
        let shapes = runtime.tree().child_order(content).unwrap();
        let old: Vec<_> = shapes
            .iter()
            .map(|id| surface_center(&runtime, *id))
            .collect();
        if name == "GridLayoutExample" {
            for id in shapes {
                runtime.figure(content).unwrap().remove(id).unwrap();
            }
        } else {
            orient(&mut runtime, content, name, true);
        }
        let next = runtime
            .frames()
            .prepare_submission(surface(), capabilities)
            .into_ready()
            .unwrap();
        let new: Vec<_> = runtime
            .tree()
            .child_order(content)
            .unwrap()
            .iter()
            .map(|id| surface_center(&runtime, *id))
            .collect();
        for point in old.into_iter().chain(new) {
            assert!(
                next.damage.mode() == DamageMode::Full
                    || next
                        .damage
                        .regions()
                        .iter()
                        .any(|region| region.contains(point)),
                "{name}: old and new geometry must both be repaired at {point:?}"
            );
        }
        let acknowledgement = runtime
            .frames()
            .complete_submission(
                next.session_id,
                next.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        assert_eq!(acknowledgement, SubmissionAcknowledgement::Presented);
    }
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn native_gpu_layout_changes_erase_old_pixels_and_restore_the_same_image() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("official layout evidence"),
        size: wgpu::Extent3d {
            width: 1200,
            height: 800,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    for name in [
        "FlowLayoutExample",
        "ToolbarLayoutExample",
        "GridLayoutExample",
    ] {
        let (mut runtime, content) = scene(name);
        let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
        let initial = submit(&mut runtime, &mut renderer, &device, &queue, &target);
        save(name, "initial", &initial);
        let shapes = runtime.tree().child_order(content).unwrap();
        let red = if name == "GridLayoutExample" {
            shapes[0]
        } else {
            shapes[1]
        };
        let point = surface_center(&runtime, red);
        let original_color = pixel(&initial, point);
        assert_ne!(original_color, [255, 255, 255, 255]);
        if name == "GridLayoutExample" {
            for id in shapes {
                runtime.figure(content).unwrap().remove(id).unwrap();
            }
        } else {
            orient(&mut runtime, content, name, true);
        }
        let changed = submit(&mut runtime, &mut renderer, &device, &queue, &target);
        save(name, "changed", &changed);
        assert_eq!(
            pixel(&changed, point),
            [255, 255, 255, 255],
            "{name}: stale shape pixel must be erased"
        );
        if name != "GridLayoutExample" {
            orient(&mut runtime, content, name, false);
            let restored = submit(&mut runtime, &mut renderer, &device, &queue, &target);
            save(name, "restored", &restored);
            assert_eq!(
                restored, initial,
                "{name}: geometry restore must restore all pixels"
            );
        } else {
            let origin = runtime
                .tree()
                .local_to_surface_transform(content)
                .unwrap()
                .transform_point(Point::new(0.0, 0.0));
            assert_eq!(origin, Point::new(390.0, 20.0));
        }
    }
}

fn submit(
    runtime: &mut Runtime,
    renderer: &mut VelloTextureRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
) -> Vec<u8> {
    let mut backend = renderer.bind(target);
    let frame = runtime
        .frames()
        .prepare_submission(surface(), backend.capabilities())
        .into_ready()
        .unwrap();
    let result = backend.submit(&frame);
    assert_eq!(result, Ok(RenderDisposition::Presented));
    let acknowledgement = runtime
        .frames()
        .complete_submission(frame.session_id, frame.frame_id, result)
        .unwrap();
    assert_eq!(acknowledgement, SubmissionAcknowledgement::Presented);
    let stride = (target.width() * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("layout readback"),
        size: u64::from(stride * target.height()),
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
                bytes_per_row: Some(stride),
                rows_per_image: Some(target.height()),
            },
        },
        target.size(),
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
    buffer
        .get_mapped_range(..)
        .chunks(stride as usize)
        .flat_map(|row| row[..target.width() as usize * 4].iter().copied())
        .collect()
}

fn pixel(bytes: &[u8], point: Point) -> [u8; 4] {
    let offset = (point.y() as usize * 1200 + point.x() as usize) * 4;
    bytes[offset..offset + 4].try_into().unwrap()
}

fn save(name: &str, stage: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("NOVADRAW_LAYOUT_EVIDENCE") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        image::save_buffer(
            directory.join(format!("{name}-{stage}.png")),
            bytes,
            1200,
            800,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
