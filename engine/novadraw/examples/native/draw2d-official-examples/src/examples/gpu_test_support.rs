use novadraw::{
    render::{RenderBackend, RenderDisposition, SurfaceInfo},
    runtime::Runtime,
};
use novadraw_backend_vello::{texture::VelloTextureRenderer, wgpu};

pub(super) struct GpuCanvas {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    renderer: VelloTextureRenderer,
    surface: SurfaceInfo,
}

impl GpuCanvas {
    pub fn new(width: u32, height: u32, scale: f64) -> Self {
        assert!((width * 4).is_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = VelloTextureRenderer::new(&device, &queue).unwrap();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("official example retained comparison"),
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
        });
        Self {
            device,
            queue,
            texture,
            renderer,
            surface: SurfaceInfo {
                logical_width: f64::from(width) / scale,
                logical_height: f64::from(height) / scale,
                pixel_width: width,
                pixel_height: height,
                scale_factor: scale,
            },
        }
    }
    pub fn render(&mut self, runtime: &mut Runtime, full: bool) -> Vec<u8> {
        self.render_with_coverage_certificate(runtime, full, false)
    }
    /// Test-only certificate for finite unhinted fixtures. Production Vello remains uncertified.
    pub fn render_with_coverage_certificate(
        &mut self,
        runtime: &mut Runtime,
        full: bool,
        certified: bool,
    ) -> Vec<u8> {
        if full {
            runtime.frames().request_full_redraw();
        }
        let mut backend = self.renderer.bind(&self.texture);
        let capabilities = if certified {
            backend.capabilities().with_filled_path_coverage_outset(2)
        } else {
            backend.capabilities()
        };
        let frame = runtime
            .frames()
            .prepare_submission(self.surface, capabilities)
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
        let row = self.surface.pixel_width * 4;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("official example evidence readback"),
            size: u64::from(row * self.surface.pixel_height),
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
                    bytes_per_row: Some(row),
                    rows_per_image: Some(self.surface.pixel_height),
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            tx.send(result).unwrap()
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        buffer.get_mapped_range(..).to_vec()
    }
    pub fn save(&self, path: impl AsRef<std::path::Path>, pixels: &[u8]) {
        let path = path.as_ref();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::save_buffer(
            path,
            pixels,
            self.surface.pixel_width,
            self.surface.pixel_height,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
