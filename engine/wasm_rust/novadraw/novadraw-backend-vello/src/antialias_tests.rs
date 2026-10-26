use super::*;
use novadraw::graphics::{AntialiasMode as Aa, Color, GradientStop, LinearGradient};
use novadraw::render::{
    BackendSessionId, DamageSet, FrameId, RenderCommandKind as Kind, RenderSubmission,
    ResourceSnapshot, ResourceSync, SurfaceInfo,
};
use vello::kurbo::{Affine, Point as KPoint, Rect, Shape, StrokeOpts};

fn submission(commands: Vec<RenderCommand>, scale: f64) -> RenderSubmission {
    let mut damage = DamageSet::default();
    damage.set_full();
    RenderSubmission {
        commands,
        damage,
        resources: ResourceSync::Snapshot(ResourceSnapshot::default()),
        retention: None,
        surface: SurfaceInfo {
            logical_width: 64.0 / scale,
            logical_height: 64.0 / scale,
            pixel_width: 64,
            pixel_height: 64,
            scale_factor: scale,
        },
        clear_color: None,
        background_input: None,
        session_id: BackendSessionId::default(),
        frame_id: FrameId::INITIAL,
    }
}

fn command(kind: Kind) -> RenderCommand {
    RenderCommand { kind }
}

fn curve() -> Path {
    let mut path = Path::new();
    path.move_to(5.17, 7.31);
    path.cubic_to(40.2, 1.7, 1.3, 46.1, 37.7, 38.9);
    path.line_to(3.2, 30.1);
    path.close();
    path
}

#[test]
fn whole_frame_admission_keeps_resources_and_completed_cache_unchanged() {
    let mut encoder = SceneEncoder::new(1.0);
    let first = command(Kind::FillPath {
        antialias: Aa::Off,
        path: curve(),
        paint: Color::WHITE.into(),
        rule: FillRule::NonZero,
    });
    let mut frame = submission(vec![first.clone()], 1.0);
    let prepared = antialias_frame::prepare(&encoder, &frame, false)
        .unwrap()
        .unwrap();
    let old = Arc::clone(&prepared.commands[0].1[0].path);
    encoder.encode(
        &frame.commands,
        Rectangle::new(0.0, 0.0, 64.0, 64.0),
        1.0,
        prepared,
    );
    let hit = antialias_frame::prepare(&encoder, &frame, true)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&old, &hit.commands[0].1[0].path));

    encoder.antialias_config.frame.rows = 50;
    frame.commands = vec![first.clone(); 4];
    let error = antialias_frame::prepare(&encoder, &frame, true)
        .err()
        .unwrap();
    let RenderFailure::InvalidInput(input) = error else {
        panic!("budget must be invalid input");
    };
    assert_eq!(input.command_index, 1);
    assert!(encoder.font_faces.is_empty());
    encoder.antialias_config = AntialiasConfig::default();
    frame.commands = vec![first];
    let retry = antialias_frame::prepare(&encoder, &frame, true)
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&old, &retry.commands[0].1[0].path));
    for change in 0..3 {
        let mut other = frame.clone();
        match change {
            0 => other.surface.scale_factor = 2.0,
            1 => other.surface.pixel_width = 96,
            _ => other.session_id = other.session_id.next().unwrap(),
        }
        let other = antialias_frame::prepare(&encoder, &other, true)
            .unwrap()
            .unwrap();
        assert!(!Arc::ptr_eq(&old, &other.commands[0].1[0].path));
    }
    encoder.antialias_config.cache_bytes = 0;
    let uncached = antialias_frame::prepare(&encoder, &frame, true)
        .unwrap()
        .unwrap();
    encoder.encode(
        &frame.commands,
        Rectangle::new(0.0, 0.0, 64.0, 64.0),
        1.0,
        uncached,
    );
    let next = antialias_frame::prepare(&encoder, &frame, true)
        .unwrap()
        .unwrap();
    assert!(!Arc::ptr_eq(&old, &next.commands[0].1[0].path));

    // Arbitrarily long borrowed dash input must be admitted before cache-key copying.
    encoder.antialias_config.operation.elements = 8;
    frame.commands = vec![command(Kind::StrokePath {
        antialias: Aa::Off,
        path: curve(),
        paint: Color::WHITE.into(),
        stroke: StrokeStyle::default().with_dash_pattern(novadraw::graphics::DashPattern::Custom(
            novadraw::graphics::CustomDash::try_new(&[1.0; 16]).unwrap(),
        )),
    })];
    assert!(matches!(
        antialias_frame::prepare(&encoder, &frame, true),
        Err(RenderFailure::InvalidInput(
            novadraw::render::InvalidGraphicsInput {
                command_index: 0,
                reason: novadraw::render::GraphicsInputError::AntialiasPreparationLimit {
                    kind: "elements",
                    ..
                },
            }
        ))
    ));
}

#[test]
fn stroke_and_fill_centers_match_independent_geometry_under_nonuniform_dpi() {
    let source = curve();
    let style = StrokeStyle::default()
        .with_width(1.7)
        .unwrap()
        .with_cap(LineCap::Round)
        .with_join(LineJoin::Round)
        .with_dash_pattern(novadraw::graphics::DashPattern::Custom(
            novadraw::graphics::CustomDash::try_new(&[4.7, 2.1]).unwrap(),
        ));
    for scale in [1.0, 1.5, 2.0] {
        let transform = Affine2D::new(0.65, 0.11, -0.17, 0.8, 3.27, 2.11);
        for rule in [FillRule::EvenOdd, FillRule::NonZero] {
            let frame = submission(
                vec![
                    command(Kind::SetTransform { matrix: transform }),
                    command(Kind::FillPath {
                        antialias: Aa::Off,
                        path: source.clone(),
                        paint: Color::WHITE.into(),
                        rule,
                    }),
                    command(Kind::StrokePath {
                        antialias: Aa::Off,
                        path: source.clone(),
                        paint: Color::WHITE.into(),
                        stroke: style.clone(),
                    }),
                ],
                scale,
            );
            let encoder = SceneEncoder::new(scale);
            let prepared = antialias_frame::prepare(&encoder, &frame, false)
                .unwrap()
                .unwrap();
            let device = Affine::scale(scale) * Affine::new(transform.coeffs());
            let source = path_to_vello(&source, 1.0);
            let reference_fill = device * source.clone();
            let reference_stroke = device
                * vello::kurbo::stroke(
                    source.iter(),
                    &vello_stroke(&style, 1.0),
                    &StrokeOpts::default(),
                    1e-6,
                );
            for y in 0..64 {
                for x in 0..64 {
                    let p = KPoint::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let winding = reference_fill.winding(p);
                    let expected = if rule == FillRule::EvenOdd {
                        winding & 1 != 0
                    } else {
                        winding != 0
                    };
                    assert_eq!(prepared.commands[0].1[0].path.winding(p) != 0, expected);
                    assert_eq!(
                        prepared.commands[1].1[0].path.winding(p) != 0,
                        reference_stroke.winding(p) != 0,
                        "stroke dpi={scale} {p:?}"
                    );
                }
            }
        }
    }
}

struct Gpu {
    device: vello::wgpu::Device,
    queue: vello::wgpu::Queue,
    renderer: Renderer,
    texture: vello::wgpu::Texture,
}

impl Gpu {
    fn new() -> Self {
        Self::sized(64)
    }

    fn sized(size: u32) -> Self {
        use vello::wgpu as w;
        let instance = w::Instance::new(w::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = Renderer::new(&device, vello::RendererOptions::default()).unwrap();
        let texture = device.create_texture(&w::TextureDescriptor {
            label: Some("per-operation AA production oracle"),
            size: w::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::STORAGE_BINDING | w::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Self {
            device,
            queue,
            renderer,
            texture,
        }
    }

    fn render(&mut self, scene: &vello::Scene) -> Vec<u8> {
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                &self.texture.create_view(&Default::default()),
                &vello::RenderParams {
                    base_color: VelloColor::TRANSPARENT,
                    width: 64,
                    height: 64,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .unwrap();
        self.read()
    }

    fn read(&self) -> Vec<u8> {
        use vello::wgpu as w;
        let buffer = self.device.create_buffer(&w::BufferDescriptor {
            label: Some("AA readback"),
            size: 64 * 64 * 4,
            usage: w::BufferUsages::MAP_READ | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            w::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: w::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(w::MapMode::Read, .., move |r| tx.send(r).unwrap());
        self.device
            .poll(w::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        buffer.get_mapped_range(..).to_vec()
    }
}

fn encoded(encoder: &mut SceneEncoder, frame: &RenderSubmission) {
    let prepared = antialias_frame::prepare(encoder, frame, true)
        .unwrap()
        .unwrap();
    encoder.encode(
        &frame.commands,
        Rectangle::new(
            0.0,
            0.0,
            frame.surface.logical_width,
            frame.surface.logical_height,
        ),
        frame.surface.scale_factor,
        prepared,
    );
}

#[test]
fn production_msaa_mask_alpha_gradient_clip_order_and_texture_rejection() {
    let mut gpu = Gpu::new();
    let mut encoder = SceneEncoder::new(1.0);
    let source = curve();
    let mut frame = submission(
        vec![command(Kind::FillPath {
            antialias: Aa::Off,
            path: source.clone(),
            paint: Color::WHITE.into(),
            rule: FillRule::NonZero,
        })],
        1.0,
    );
    encoded(&mut encoder, &frame);
    let off = gpu.render(&encoder.scene);
    let path = path_to_vello(&source, 1.0);
    for (index, rgba) in off.chunks_exact(4).enumerate() {
        let p = KPoint::new((index % 64) as f64 + 0.5, (index / 64) as f64 + 0.5);
        assert_eq!(rgba[3], if path.winding(p) != 0 { 255 } else { 0 });
    }
    let Kind::FillPath {
        antialias, paint, ..
    } = &mut frame.commands[0].kind
    else {
        unreachable!()
    };
    *antialias = Aa::On;
    *paint = Color::WHITE.into();
    encoded(&mut encoder, &frame);
    let on = gpu.render(&encoder.scene);
    assert!(on.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
    let Kind::FillPath { antialias, .. } = &mut frame.commands[0].kind else {
        unreachable!()
    };
    *antialias = Aa::Default;
    encoded(&mut encoder, &frame);
    assert_eq!(gpu.render(&encoder.scene), on);
    // Baseline is the unmodified ordinary lowering.
    let mut baseline = SceneEncoder::new(1.0);
    baseline.render_command(&frame.commands[0]);
    assert_eq!(gpu.render(&baseline.scene), on);
    let Kind::FillPath {
        antialias, paint, ..
    } = &mut frame.commands[0].kind
    else {
        unreachable!()
    };
    *antialias = Aa::Off;
    *paint = Color::WHITE.with_alpha(0.5).into();
    encoded(&mut encoder, &frame);
    let alpha = gpu.render(&encoder.scene);
    for (a, b) in off.chunks_exact(4).zip(alpha.chunks_exact(4)) {
        assert_eq!(b[3], if a[3] == 255 { 128 } else { 0 });
    }
    let gradient = LinearGradient::try_new(
        Point::new(0.0, 0.0),
        Point::new(64.0, 64.0),
        &[
            GradientStop::try_new(0.0, Color::BLACK.with_alpha(0.5)).unwrap(),
            GradientStop::try_new(1.0, Color::WHITE.with_alpha(0.5)).unwrap(),
        ],
    )
    .unwrap();
    let transform = Affine2D::new(1.0, 0.11, -0.07, 0.9, 3.25, 2.75);
    frame.commands = vec![
        command(Kind::FillRect {
            antialias: Aa::On,
            rect: Rectangle::new(0.2, 0.3, 63.2, 63.1),
            paint: Color::rgba(0.0, 0.0, 1.0, 0.5).into(),
            rule: FillRule::NonZero,
        }),
        command(Kind::ClipPath {
            clip: ClipPath::try_new(&source, FillRule::EvenOdd).unwrap(),
        }),
        command(Kind::SetTransform { matrix: transform }),
        command(Kind::FillPath {
            antialias: Aa::Off,
            path: source.clone(),
            paint: gradient.into(),
            rule: FillRule::NonZero,
        }),
        command(Kind::ResetClip),
        command(Kind::FillRect {
            antialias: Aa::On,
            rect: Rectangle::new(20.25, 20.75, 22.0, 22.0),
            paint: Color::rgba(0.0, 1.0, 0.0, 0.5).into(),
            rule: FillRule::NonZero,
        }),
    ];
    for scale in [1.0, 1.5, 2.0] {
        frame.surface.scale_factor = scale;
        frame.surface.logical_width = 64.0 / scale;
        frame.surface.logical_height = 64.0 / scale;
        encoded(&mut encoder, &frame);
        let actual = gpu.render(&encoder.scene);
        // Reference emits individual device-pixel cells directly into the same clip/order.
        let reference_path = Affine::scale(scale) * Affine::new(transform.coeffs()) * path.clone();
        let mut reference = SceneEncoder::new(scale);
        for (index, command) in frame.commands.iter().enumerate() {
            if index != 3 {
                reference.render_command(command);
                continue;
            }
            let paint = command.kind.paints().next().unwrap();
            for y in 0..64 {
                for x in 0..64 {
                    if reference_path.winding(KPoint::new(f64::from(x) + 0.5, f64::from(y) + 0.5))
                        != 0
                    {
                        reference.scene.fill(
                            vello::peniko::Fill::NonZero,
                            Affine::IDENTITY,
                            &vello_paint(paint, scale),
                            Some(SceneEncoder::transform_to_affine(&transform, scale)),
                            &Rect::new(x.into(), y.into(), f64::from(x + 1), f64::from(y + 1)),
                        );
                    }
                }
            }
        }
        let expected = gpu.render(&reference.scene);
        assert_eq!(actual, expected, "clip/order/gradient DPI={scale}");
    }
    // Exercise the public Texture backend and prove admission failure does not write pixels.
    frame = submission(
        vec![command(Kind::FillPath {
            antialias: Aa::Off,
            path: source,
            paint: Color::WHITE.into(),
            rule: FillRule::NonZero,
        })],
        1.0,
    );
    let mut texture = texture::VelloTextureRenderer::new(&gpu.device, &gpu.queue).unwrap();
    assert_eq!(
        texture.bind(&gpu.texture).submit(&frame).unwrap(),
        RenderDisposition::Presented
    );
    let before = gpu.read();
    texture.set_antialias_config(AntialiasConfig {
        operation: AntialiasWorkLimits {
            rows: 1,
            ..AntialiasConfig::default().operation
        },
        ..AntialiasConfig::default()
    });
    assert!(matches!(
        texture.bind(&gpu.texture).submit(&frame),
        Err(RenderFailure::InvalidInput(_))
    ));
    assert_eq!(before, gpu.read());
}

#[test]
fn text_off_is_independent_and_font_preparation_precedes_resource_sync() {
    use novadraw::render::{
        BuiltinFont, FontData, FontDescriptor, ParleyTextEngine, ResourceId, ResourceUpdate,
        TextConstraints,
    };
    let font_id = ResourceId::new(uuid::Uuid::nil(), 9);
    let mut text = ParleyTextEngine::new();
    text.register_font(font_id, 1, BuiltinFont::Inter.bytes())
        .unwrap();
    let layout = text
        .layout("Ab", &FontDescriptor::default(), TextConstraints::UNBOUNDED)
        .unwrap();
    let run = layout.glyph_runs()[0].clone();
    let mut frame = submission(
        vec![command(Kind::DrawGlyphRun {
            antialias: Aa::Off,
            run: run.clone(),
            origin: Point::new(3.25, 26.75),
            paint: GlyphPaint::Fill(Color::WHITE.into()),
        })],
        1.0,
    );
    let mut encoder = SceneEncoder::new(1.0);
    assert!(
        antialias_frame::prepare(&encoder, &frame, false)
            .unwrap()
            .is_none()
    );
    frame.resources = ResourceSync::Snapshot(ResourceSnapshot {
        ready: vec![ResourceUpdate {
            id: font_id,
            revision: 1,
            payload: ResourcePayload::Font(Arc::new(FontData::new(
                BuiltinFont::Inter.bytes().to_vec(),
            ))),
        }],
    });
    let prepared = antialias_frame::prepare(&encoder, &frame, false)
        .unwrap()
        .unwrap();
    assert!(
        encoder.font_faces.is_empty(),
        "preflight cannot synchronize fonts"
    );
    assert_eq!(prepared.commands[0].1.len(), run.glyphs.len());
    let font = vello::peniko::FontData::new(BuiltinFont::Inter.bytes().to_vec().into(), 0);
    for (glyph, draw) in run.glyphs.iter().zip(&prepared.commands[0].1) {
        let outline = glyph_outline::outline(&font, &run, glyph.id, 1.0).unwrap();
        let local = Affine::new([
            1.0,
            0.0,
            0.0,
            -1.0,
            3.25 + f64::from(glyph.x),
            26.75 + f64::from(glyph.y),
        ]);
        let reference = local * outline;
        for y in 0..64 {
            for x in 0..64 {
                let p = KPoint::new(f64::from(x) + 0.5, f64::from(y) + 0.5);
                assert_eq!(draw.path.winding(p) != 0, reference.winding(p) != 0);
            }
        }
    }
    let mut gate = BackendSessionGate::default();
    encoder.sync_resources(&mut gate, &frame).unwrap();
    encoded(&mut encoder, &frame);
    let mut gpu = Gpu::new();
    let off = gpu.render(&encoder.scene);
    assert!(off.chunks_exact(4).any(|p| p[3] == 255));
    assert!(off.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255));
    let Kind::DrawGlyphRun { antialias, .. } = &mut frame.commands[0].kind else {
        unreachable!()
    };
    *antialias = Aa::On;
    encoded(&mut encoder, &frame);
    let on = gpu.render(&encoder.scene);
    assert!(on.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
    let shape = command(Kind::Ellipse {
        antialias: Aa::Off,
        cx: 48.25,
        cy: 45.75,
        rx: 10.0,
        ry: 10.0,
        fill_paint: Some(Color::WHITE.into()),
        stroke_paint: None,
        stroke: StrokeStyle::default(),
        rule: FillRule::NonZero,
    });
    frame.commands.push(shape);
    encoded(&mut encoder, &frame);
    let shape_off = gpu.render(&encoder.scene);
    let Kind::Ellipse { antialias, .. } = &mut frame.commands[1].kind else {
        unreachable!()
    };
    *antialias = Aa::On;
    encoded(&mut encoder, &frame);
    let shape_on = gpu.render(&encoder.scene);
    for y in 0..32 {
        assert_eq!(
            &shape_off[y * 256..(y + 1) * 256],
            &shape_on[y * 256..(y + 1) * 256]
        );
    }
    let Kind::DrawGlyphRun {
        antialias, paint, ..
    } = &mut frame.commands[0].kind
    else {
        unreachable!()
    };
    *antialias = Aa::Off;
    *paint = GlyphPaint::Stroke {
        paint: Color::WHITE.into(),
        stroke: StrokeStyle::default()
            .with_width(1.25)
            .unwrap()
            .with_dash_pattern(novadraw::graphics::DashPattern::Dash),
    };
    encoded(&mut encoder, &frame);
    let _ = gpu.render(&encoder.scene);
    encoder.antialias_config.glyph_scratch_bytes = 0;
    assert!(matches!(
        antialias_frame::prepare(&encoder, &frame, true),
        Err(RenderFailure::InvalidInput(_))
    ));
}

#[test]
fn shape_operations_restore_transform_and_erase_after_retry_and_reset() {
    let mut gpu = Gpu::new();
    let mut encoder = SceneEncoder::new(1.0);
    let stroke = StrokeStyle::default()
        .with_width(1.7)
        .unwrap()
        .with_cap(LineCap::Round)
        .with_dash_pattern(novadraw::graphics::DashPattern::Dash);
    let shapes = vec![
        command(Kind::FillRect {
            antialias: Aa::Off,
            rect: Rectangle::new(3.25, 4.75, 30.0, 24.0),
            paint: Color::WHITE.into(),
            rule: FillRule::NonZero,
        }),
        command(Kind::StrokeRect {
            antialias: Aa::Off,
            rect: Rectangle::new(3.25, 4.75, 30.0, 24.0),
            paint: Color::WHITE.into(),
            stroke: stroke.clone(),
        }),
        command(Kind::Line {
            antialias: Aa::Off,
            p1: Point::new(2.25, 3.75),
            p2: Point::new(40.25, 42.75),
            paint: Color::WHITE.into(),
            stroke: stroke.clone(),
        }),
        command(Kind::Polyline {
            antialias: Aa::Off,
            points: vec![
                Point::new(20.0, 20.0),
                Point::new(50.0, 60.0),
                Point::new(10.0, 50.0),
            ]
            .into(),
            paint: Color::WHITE.into(),
            stroke: stroke.clone(),
        }),
        command(Kind::Ellipse {
            antialias: Aa::Off,
            cx: 24.25,
            cy: 23.75,
            rx: 17.0,
            ry: 14.0,
            fill_paint: None,
            stroke_paint: Some(Color::WHITE.into()),
            stroke,
            rule: FillRule::NonZero,
        }),
    ];
    for shape in shapes {
        let mut frame = submission(
            vec![
                command(Kind::PushState),
                command(Kind::ConcatTransform {
                    matrix: Affine2D::from_translation(11.0, 7.0),
                }),
                command(Kind::RestoreState),
                shape.clone(),
                command(Kind::PopState),
            ],
            1.0,
        );
        encoded(&mut encoder, &frame);
        let restored = gpu.render(&encoder.scene);
        assert!(restored.chunks_exact(4).any(|p| p[3] == 255));
        assert!(restored.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255));
        frame.commands = vec![shape];
        encoded(&mut encoder, &frame);
        assert_eq!(gpu.render(&encoder.scene), restored);
        // Integer damage clip uses the same global pixel phase and erases the old region.
        let prepared = antialias_frame::prepare(&encoder, &frame, true)
            .unwrap()
            .unwrap();
        encoder.encode(
            &frame.commands,
            Rectangle::new(8.0, 8.0, 32.0, 32.0),
            1.0,
            prepared,
        );
        let clipped = gpu.render(&encoder.scene);
        for y in 8..40 {
            for x in 8..40 {
                let index = (y * 64 + x) * 4;
                assert_eq!(&clipped[index..index + 4], &restored[index..index + 4]);
            }
        }
        encoder.antialias_config.operation.rows = 0;
        assert!(antialias_frame::prepare(&encoder, &frame, true).is_err());
        encoder.antialias_config = AntialiasConfig::default();
        frame.session_id = frame.session_id.next().unwrap();
        let mut gate = BackendSessionGate::default();
        let prepared = antialias_frame::prepare(&encoder, &frame, false)
            .unwrap()
            .unwrap();
        encoder.sync_resources(&mut gate, &frame).unwrap();
        encoder.encode(
            &frame.commands,
            Rectangle::new(0.0, 0.0, 64.0, 64.0),
            1.0,
            prepared,
        );
        assert_eq!(gpu.render(&encoder.scene), restored);
        frame.surface.scale_factor = 2.0;
        frame.surface.logical_width = 32.0;
        frame.surface.logical_height = 32.0;
        encoded(&mut encoder, &frame);
        let _ = gpu.render(&encoder.scene);
        frame.commands.clear();
        encoded(&mut encoder, &frame);
        assert!(gpu.render(&encoder.scene).iter().all(|byte| *byte == 0));
    }
}

#[test]
#[ignore = "release cost measurement; run through render.antialias-cost"]
fn antialias_release_cost() {
    use std::time::Instant;
    let mut gpu = Gpu::sized(1024);
    let mut rows = Vec::new();
    for (name, count, dashed, continuous, aa) in [
        ("default", 25, false, false, Aa::Default),
        ("off-cold-hot", 25, false, false, Aa::Off),
        ("dashed", 25, true, false, Aa::Off),
        ("continuous-dpi2", 25, true, true, Aa::Off),
    ] {
        let mut encoder = SceneEncoder::new(1.0);
        let shape = if dashed {
            command(Kind::StrokePath {
                antialias: aa,
                path: curve(),
                paint: Color::WHITE.into(),
                stroke: StrokeStyle::default()
                    .with_width(1.7)
                    .unwrap()
                    .with_cap(LineCap::Round)
                    .with_dash_pattern(novadraw::graphics::DashPattern::Dash),
            })
        } else {
            command(Kind::FillPath {
                antialias: aa,
                path: curve(),
                paint: Color::WHITE.into(),
                rule: FillRule::NonZero,
            })
        };
        let mut commands = Vec::new();
        for index in 0..count {
            commands.push(command(Kind::SetTransform {
                matrix: Affine2D::from_translation(
                    f64::from(index % 5) * 100.0,
                    f64::from(index / 5) * 90.0,
                ),
            }));
            commands.push(shape.clone());
        }
        let mut frame = submission(commands, 1.0);
        frame.surface.pixel_width = 1024;
        frame.surface.pixel_height = 1024;
        frame.surface.logical_width = 1024.0;
        frame.surface.logical_height = 1024.0;
        let mut samples = Vec::new();
        let mut storage = (0, 0);
        let mut peak_storage = 0;
        let mut prior_storage = 0;
        let mut cold = 0;
        let mut frames = Vec::new();
        let mut stream_bytes = 0;
        for iteration in 0..61 {
            if continuous {
                frame.surface.scale_factor = 2.0;
                frame.surface.logical_width = 512.0;
                frame.surface.logical_height = 512.0;
                for (index, pair) in frame.commands.chunks_exact_mut(2).enumerate() {
                    if let Kind::SetTransform { matrix } = &mut pair[0].kind {
                        *matrix = Affine2D::from_translation(
                            (index % 5) as f64 * 100.0 + iteration as f64 * 0.13,
                            (index / 5) as f64 * 90.0,
                        );
                    }
                }
            }
            let start = Instant::now();
            let prepared = antialias_frame::prepare(&encoder, &frame, true)
                .unwrap()
                .unwrap();
            storage = prepared.storage();
            peak_storage = peak_storage.max(prior_storage + storage.1);
            prior_storage = storage.1;
            encoder.encode(
                &frame.commands,
                Rectangle::new(
                    0.0,
                    0.0,
                    frame.surface.logical_width,
                    frame.surface.logical_height,
                ),
                frame.surface.scale_factor,
                prepared,
            );
            let micros = start.elapsed().as_micros();
            let encoding = encoder.scene.encoding();
            stream_bytes = std::mem::size_of_val(encoding.path_data.as_slice())
                + std::mem::size_of_val(encoding.path_tags.as_slice())
                + std::mem::size_of_val(encoding.draw_data.as_slice())
                + std::mem::size_of_val(encoding.draw_tags.as_slice())
                + std::mem::size_of_val(encoding.transforms.as_slice())
                + std::mem::size_of_val(encoding.styles.as_slice());
            gpu.renderer
                .render_to_texture(
                    &gpu.device,
                    &gpu.queue,
                    &encoder.scene,
                    &gpu.texture.create_view(&Default::default()),
                    &vello::RenderParams {
                        base_color: VelloColor::TRANSPARENT,
                        width: 1024,
                        height: 1024,
                        antialiasing_method: AaConfig::Msaa16,
                    },
                )
                .unwrap();
            gpu.device
                .poll(vello::wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .unwrap();
            if iteration == 0 {
                cold = micros;
            } else {
                samples.push(micros);
                frames.push(start.elapsed().as_micros());
            }
        }
        samples.sort_unstable();
        frames.sort_unstable();
        rows.push(format!(
            "{{\"scenario\":\"{name}\",\"cold_prepare_encode_us\":{cold},\
            \"hot_p50_us\":{},\"hot_p95_us\":{},\"output_elements\":{},\
            \"cache_payload_bytes\":{},\"old_new_payload_upper_bytes\":{peak_storage},\
            \"encoding_stream_bytes\":{stream_bytes},\"frame_wait_p95_us\":{}}}",
            samples[30], samples[57], storage.0, storage.1, frames[57]
        ));
    }
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/verification/per-operation-antialias");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("cost.json"), format!("[{}]\n", rows.join(","))).unwrap();
}
