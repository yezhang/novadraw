//! Identical baseline/candidate consumer for release artifact-size measurement.
use novadraw::{
    figure::RectangleFigure,
    graphics::Color,
    render::{RenderBackend, SurfaceInfo},
    runtime::Runtime,
};
use novadraw_backend_vello::texture::VelloTextureRenderer;
use vello::wgpu;

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("artifact probe"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let mut runtime = Runtime::empty();
    runtime
        .set_contents(Box::new(RectangleFigure::new_with_color(
            0.0,
            0.0,
            64.0,
            64.0,
            Color::WHITE,
        )))
        .unwrap();
    let mut backend = renderer.bind(&target);
    let frame = runtime
        .frames()
        .prepare_submission(
            SurfaceInfo {
                logical_width: 64.0,
                logical_height: 64.0,
                pixel_width: 64,
                pixel_height: 64,
                scale_factor: 1.0,
            },
            backend.capabilities(),
        )
        .into_ready()
        .unwrap();
    backend.submit(&frame).unwrap();
}
