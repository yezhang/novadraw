//! Optional WGPU render targets and premultiplied-alpha window composition.
use crate::geometry::{CanvasError, CanvasRect, WindowMetrics};
use wgpu::util::DeviceExt;

pub const CANVAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct CanvasTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}
impl CanvasTarget {
    /// Returns None for an empty canvas. Allocation limits are checked before creation.
    pub fn new(
        device: &wgpu::Device,
        rect: CanvasRect,
        metrics: WindowMetrics,
    ) -> Result<Option<Self>, CanvasError> {
        if rect.is_empty() {
            return Ok(None);
        }
        let size = metrics.texture_size(rect)?;
        if size.width > device.limits().max_texture_dimension_2d
            || size.height > device.limits().max_texture_dimension_2d
        {
            return Err(CanvasError::TextureTooLarge);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Canvas target"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: CANVAS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(Some(Self {
            texture,
            view,
            size: [size.width, size.height],
        }))
    }
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
}

pub struct CanvasLayer<'a> {
    pub target: &'a CanvasTarget,
    pub rect: CanvasRect,
}

pub struct CanvasCompositor {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

impl CanvasCompositor {
    /// The output must be a render attachment with this format, on the same device.
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Canvas composition bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Canvas compositor"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compositor.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Canvas compositor layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Canvas compositor"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Canvas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout,
            pipeline,
            sampler,
        }
    }
    /// Clears and recomposes the complete window, erasing old positions of moved/removed canvases.
    pub fn compose(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        metrics: WindowMetrics,
        background: wgpu::Color,
        layers: &[CanvasLayer<'_>],
    ) -> Result<(), CanvasError> {
        let size = metrics.pixels();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let scale = metrics.scale_factor();
        let mut prepared = Vec::new();
        for layer in layers {
            if layer.rect.is_empty() {
                continue;
            }
            let origin = layer.rect.origin();
            let x = origin.x * scale;
            let y = origin.y * scale;
            let width = layer.rect.width() * scale;
            let height = layer.rect.height() * scale;
            if ![x, y, width, height]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
            {
                return Err(CanvasError::InvalidGeometry);
            }
            // A stale smaller texture would sample beyond its contents after resize.
            if width.ceil() > f64::from(layer.target.size[0])
                || height.ceil() > f64::from(layer.target.size[1])
            {
                return Err(CanvasError::TextureTooLarge);
            }
            let left = x.floor().clamp(0.0, f64::from(size.width)) as u32;
            let top = y.floor().clamp(0.0, f64::from(size.height)) as u32;
            let right = (x + width).ceil().clamp(0.0, f64::from(size.width)) as u32;
            let bottom = (y + height).ceil().clamp(0.0, f64::from(size.height)) as u32;
            if right <= left || bottom <= top {
                continue;
            }
            let values = [
                x as f32,
                y as f32,
                width as f32,
                height as f32,
                size.width as f32,
                size.height as f32,
                (width / f64::from(layer.target.size[0])) as f32,
                (height / f64::from(layer.target.size[1])) as f32,
            ];
            let bytes: Vec<_> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Canvas composition geometry"),
                contents: &bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Canvas layer"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(layer.target.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            prepared.push((bindings, [left, top, right - left, bottom - top]));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Canvas window composition"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        for (bindings, [x, y, width, height]) in &prepared {
            pass.set_bind_group(0, bindings, &[]);
            pass.set_scissor_rect(*x, *y, *width, *height);
            pass.draw(0..6, 0..1);
        }
        Ok(())
    }
}
