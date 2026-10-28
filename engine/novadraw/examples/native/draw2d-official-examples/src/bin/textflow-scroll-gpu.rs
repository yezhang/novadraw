//! Original Large factory: CPU preparation, Vello encoding and synchronous GPU evidence.
use novadraw::{
    geometry::Rectangle,
    render::{RenderBackend, RenderCommandKind, RenderDisposition, SurfaceInfo},
    runtime::{Runtime, SubmissionAcknowledgement},
};
use novadraw_backend_vello::scene::VelloSceneRenderer;
use std::time::{Duration, Instant};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene, wgpu};

fn viewport(runtime: &Runtime) -> novadraw::tree::FigureId {
    let mut pending = vec![runtime.tree().contents().unwrap()];
    while let Some(id) = pending.pop() {
        if runtime.tree().node(id).unwrap().figure_name() == "ViewportFigure" {
            return id;
        }
        pending.extend(runtime.tree().child_order(id).unwrap());
    }
    panic!("Large viewport");
}

fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "30".into())
        .parse()
        .unwrap();
    let scale: f64 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "2".into())
        .parse()
        .unwrap();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    eprintln!("adapter: {:?}", adapter.get_info());
    let mut renderer = Renderer::new(&device, RendererOptions::default()).unwrap();
    let surface = SurfaceInfo {
        logical_width: 1200.0,
        logical_height: 800.0,
        pixel_width: (1200.0 * scale) as u32,
        pixel_height: (800.0 * scale) as u32,
        scale_factor: scale,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("original Large GPU scroll"),
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
    let view = texture.create_view(&Default::default());
    let (_, mut factory) = draw2d_official_examples::runnable_entries()
        .into_iter()
        .find(|entry| entry.0 == "TextFlowLargeExample")
        .unwrap();
    let mut runtime = factory();
    let root = runtime.tree().contents().unwrap();
    runtime
        .figure(root)
        .unwrap()
        .set_bounds(Rectangle::new(0.0, 0.0, 1200.0, 800.0))
        .unwrap();
    runtime.frames().stabilize_for_query().unwrap();
    let viewport = viewport(&runtime);
    let mut encoder = VelloSceneRenderer::new();
    let mut scene = Scene::new();
    for sample in 0..count + 5 {
        let start = Instant::now();
        runtime
            .viewports()
            .scroll_by(viewport, 0.0, if sample % 20 < 10 { 40.25 } else { -40.25 })
            .unwrap();
        runtime.frames().request_full_redraw();
        let mut backend = encoder.bind(&mut scene);
        let frame = runtime
            .frames()
            .prepare_submission(surface, backend.capabilities())
            .into_ready()
            .unwrap();
        let prepare_ns = start.elapsed().as_nanos();
        let glyphs: usize = frame
            .commands
            .iter()
            .map(|c| match &c.kind {
                RenderCommandKind::DrawGlyphRun { run, .. } => run.glyphs.len(),
                _ => 0,
            })
            .sum();
        let start = Instant::now();
        assert_eq!(backend.submit(&frame), Ok(RenderDisposition::Presented));
        let encode_ns = start.elapsed().as_nanos();
        // Scene uses logical coordinates; the GPU applies DPI exactly once.
        let mut device_scene = Scene::new();
        device_scene.append(&scene, Some(vello::kurbo::Affine::scale(scale)));
        let start = Instant::now();
        renderer
            .render_to_texture(
                &device,
                &queue,
                &device_scene,
                &view,
                &RenderParams {
                    base_color: vello::peniko::Color::WHITE,
                    width: surface.pixel_width,
                    height: surface.pixel_height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .unwrap();
        let render_ns = start.elapsed().as_nanos();
        let start = Instant::now();
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(30)),
            })
            .unwrap();
        let wait_ns = start.elapsed().as_nanos();
        assert_eq!(
            runtime
                .frames()
                .complete_submission(
                    frame.session_id,
                    frame.frame_id,
                    Ok(RenderDisposition::Presented)
                )
                .unwrap(),
            SubmissionAcknowledgement::Presented
        );
        println!(
            "{{\"sample\":{sample},\"warmup\":{},\"scale\":{scale},\"commands\":{},\"glyphs\":{glyphs},\"prepare_ns\":{prepare_ns},\"encode_ns\":{encode_ns},\"render_ns\":{render_ns},\"wait_ns\":{wait_ns}}}",
            sample < 5,
            frame.commands.len()
        );
    }
}
