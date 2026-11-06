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
        self.render_with_aa(scene, AaConfig::Msaa16)
    }

    fn render_with_aa(&mut self, scene: &vello::Scene, antialiasing_method: AaConfig) -> Vec<u8> {
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
                    antialiasing_method,
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
        buffer.get_mapped_range(..).expect("mapped pixels").to_vec()
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

fn damage_test_run(id: u32) -> GlyphRun {
    use novadraw::render::{
        BuiltinFont, FontDescriptor, ParleyTextEngine, PositionedGlyph, ResourceId, TextConstraints,
    };
    use novadraw::text::shaping::GlyphRange;
    let mut text = ParleyTextEngine::new();
    text.register_font(
        ResourceId::new(uuid::Uuid::nil(), 199),
        1,
        BuiltinFont::Inter.bytes(),
    )
    .unwrap();
    let layout = text
        .layout("A", &FontDescriptor::default(), TextConstraints::UNBOUNDED)
        .unwrap();
    let mut run = layout.glyph_runs()[0].clone();
    run.font_size = 18.0;
    run.glyphs = GlyphRange::new(vec![
        PositionedGlyph { id, x: 0.0, y: 0.0 },
        PositionedGlyph {
            id,
            x: 400.0,
            y: 400.0,
        },
    ])
    .unwrap();
    run
}

#[test]
fn core_outline_damage_selection_preserves_repaired_pixels() {
    use novadraw::figure::Figure;
    use novadraw::graphics::{GraphicsError, PaintContext};
    use novadraw::runtime::Runtime;
    use novadraw::text::{
        BuiltinFont, FontDescriptor, TextConstraints, TextLayout,
        outline::{OutlineTextEngine, SkrifaOutlineProvider},
        shaping::{ParleyTextEngine, TextLayoutEngine},
    };
    struct TextFigure(TextLayout);
    impl Figure for TextFigure {
        fn name(&self) -> &'static str {
            "LocalOutlineDamageOracle"
        }
        fn initial_bounds(&self) -> Rectangle {
            Rectangle::new(0.0, 0.0, 64.0, 64.0)
        }
        fn paint(&self, paint: &mut PaintContext<'_>, _: Rectangle) -> Result<(), GraphicsError> {
            paint.set_fill_paint(Color::WHITE);
            paint.fill_text(&self.0, Point::ZERO)
        }
    }
    let mut gpu = Gpu::new();
    for scale in [1.0, 1.5, 2.0] {
        let mut runtime = Runtime::empty();
        let id = runtime
            .resources()
            .register_builtin_font(BuiltinFont::Inter)
            .unwrap();
        let mut text = OutlineTextEngine::new(
            Box::new(ParleyTextEngine::new()),
            Box::new(SkrifaOutlineProvider),
        );
        text.register_font(id.resource_id(), 1, BuiltinFont::Inter.bytes())
            .unwrap();
        let layout = text
            .layout(
                &"AB CD EF\n".repeat(100),
                &FontDescriptor::new(BuiltinFont::Inter.family(), 16.0).unwrap(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let root = runtime.set_contents(Box::new(TextFigure(layout))).unwrap();
        let surface = SurfaceInfo {
            logical_width: 64.0 / scale,
            logical_height: 64.0 / scale,
            pixel_width: 64,
            pixel_height: 64,
            scale_factor: scale,
        };
        let caps = BackendCapabilities::RETAINED_PARTIAL.with_filled_path_coverage_outset(2);
        let full = runtime
            .frames()
            .prepare_submission(surface, caps)
            .into_ready()
            .unwrap();
        let mut encoder = SceneEncoder::new(scale);
        encoded(&mut encoder, &full);
        let expected = gpu.render(&encoder.scene);
        let acknowledgement = runtime
            .frames()
            .complete_submission(
                full.session_id,
                full.frame_id,
                Ok(RenderDisposition::Presented),
            )
            .unwrap();
        assert_eq!(
            acknowledgement,
            novadraw::runtime::SubmissionAcknowledgement::Presented
        );
        let damage = Rectangle::new(4.3, 8.2, 5.2, 12.4);
        runtime.figure(root).unwrap().repaint(Some(damage)).unwrap();
        let partial = runtime
            .frames()
            .prepare_submission(surface, caps)
            .into_ready()
            .unwrap();
        assert!(!partial.damage.is_full());
        encoded(&mut encoder, &partial);
        let actual = gpu.render(&encoder.scene);
        let operations = |frame: &RenderSubmission| -> usize {
            frame
                .commands
                .iter()
                .map(|c| match &c.kind {
                    Kind::FillPath { path, .. } => path.operations().len(),
                    _ => 0,
                })
                .sum()
        };
        assert!(operations(&partial) < operations(&full));
        let (x, y, w, h) = damage_rect_to_copy_region(damage, 64, 64, scale).unwrap();
        let mut nonempty = false;
        for py in y..y + h {
            for px in x..x + w {
                let offset = ((py * 64 + px) * 4) as usize;
                nonempty |= expected[offset + 3] > 0;
                for channel in 0..4 {
                    assert!(
                        expected[offset + channel].abs_diff(actual[offset + channel]) <= 1,
                        "dpi={scale} pixel=({px},{py}) channel={channel}"
                    );
                }
            }
        }
        assert!(nonempty);
    }
}

#[test]
fn damage_selection_honors_clip_restore_reset_and_unknown_stroke() {
    use novadraw::render::{BuiltinFont, PositionedGlyph};
    use novadraw::text::shaping::GlyphRange;
    use skrifa::MetadataProvider;
    let bytes = BuiltinFont::Inter.bytes();
    let font = skrifa::FontRef::from_index(bytes, 0).unwrap();
    let id = font.charmap().map('A').unwrap().to_u32();
    let mut run = damage_test_run(id);
    run.glyphs = GlyphRange::new(vec![PositionedGlyph { id, x: 0.0, y: 0.0 }]).unwrap();
    let draw = || {
        command(Kind::DrawGlyphRun {
            antialias: Aa::Default,
            run: run.clone(),
            origin: Point::new(20.0, 30.0),
            paint: GlyphPaint::Fill(Color::WHITE.into()),
        })
    };
    let mut encoder = SceneEncoder::new(1.0);
    encoder.font_faces.insert(
        (run.font.resource_id(), run.font.revision()),
        bytes.to_vec().into(),
    );
    let frame = submission(
        vec![
            command(Kind::Clip {
                rect: Rectangle::new(0.0, 0.0, 4.0, 4.0),
            }),
            command(Kind::PushState),
            command(Kind::ResetClip),
            draw(),
            command(Kind::RestoreState),
            draw(),
            command(Kind::ResetClip),
            draw(),
            command(Kind::PopState),
            command(Kind::ResetClip),
        ],
        1.0,
    );
    encoded(&mut encoder, &frame);
    assert_eq!(encoder.scene.encoding().resources.glyphs.len(), 2);
    let prepared = antialias_frame::prepare(&encoder, &frame, true)
        .unwrap()
        .unwrap();
    encoder.encode(
        &frame.commands,
        Rectangle::new(0.0, 0.0, 4.0, 4.0),
        1.0,
        prepared,
    );
    assert!(
        encoder.scene.encoding().resources.glyphs.is_empty(),
        "ResetClip must retain damage restriction"
    );

    let data = vello::peniko::FontData::new(bytes.to_vec().into(), 0);
    let selected = encoder.ink_cache.select(
        &data,
        &run,
        (Point::new(300.0, 300.0), Affine::IDENTITY, 1.0),
        &GlyphPaint::Stroke {
            paint: Color::WHITE.into(),
            stroke: StrokeStyle::default(),
        },
        Rect::new(0.0, 0.0, 4.0, 4.0),
    );
    assert_eq!(
        selected.len(),
        1,
        "uncertified stroke must remain conservative"
    );
}

impl Gpu {
    fn render_damage_target(&mut self, scene: &vello::Scene, width: u32, height: u32) -> Vec<u8> {
        use vello::wgpu as w;
        let texture = self.device.create_texture(&w::TextureDescriptor {
            label: Some("local damage pixel oracle"),
            size: w::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format: w::TextureFormat::Rgba8Unorm,
            usage: w::TextureUsages::STORAGE_BINDING | w::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                &texture.create_view(&Default::default()),
                &vello::RenderParams {
                    base_color: VelloColor::TRANSPARENT,
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .unwrap();
        let stride =
            (width * 4).div_ceil(w::COPY_BYTES_PER_ROW_ALIGNMENT) * w::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&w::BufferDescriptor {
            label: Some("local damage readback"),
            size: u64::from(stride * height),
            usage: w::BufferUsages::MAP_READ | w::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            w::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: w::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
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
        let mapped = buffer.get_mapped_range(..).unwrap();
        mapped
            .chunks_exact(stride as usize)
            .flat_map(|row| row[..(width * 4) as usize].iter().copied())
            .collect()
    }
}

#[test]
fn local_damage_ink_and_tile_translation_match_unfiltered_pixels() {
    use novadraw::render::BuiltinFont;
    use skrifa::MetadataProvider;
    let fonts = vec![BuiltinFont::Inter.bytes().to_vec()];
    #[cfg(target_os = "macos")]
    let fonts = {
        let mut fonts = fonts;
        fonts.push(std::fs::read("/System/Library/Fonts/Apple Color Emoji.ttc").unwrap());
        fonts
    };
    let mut gpu = Gpu::new();
    for (font_index, bytes) in fonts.iter().enumerate() {
        let face = skrifa::FontRef::from_index(bytes, 0).unwrap();
        let id = face
            .charmap()
            .map(if font_index == 0 { 'f' } else { '😀' })
            .unwrap()
            .to_u32();
        let data = vello::peniko::FontData::new(bytes.clone().into(), 0);
        for scale in [1.0, 1.5, 2.0] {
            for matrix in [
                Affine2D::IDENTITY,
                Affine2D::new(0.85, 0.12, -0.18, 0.9, 1.27, 0.31),
            ] {
                let mut run = damage_test_run(id);
                run.skew_degrees = Some(17.0);
                let origin = Point::new(19.23 / scale, 33.41 / scale);
                let paint = GlyphPaint::Fill(Color::WHITE.into());
                let mut reference = vello::Scene::new();
                append_glyph_run(&mut reference, &run, &data, origin, &paint, &matrix, scale);
                let full = gpu.render_damage_target(&reference, 64, 64);
                let mut encoder = SceneEncoder::new(scale);
                encoder.font_faces.insert(
                    (run.font.resource_id(), run.font.revision()),
                    bytes.clone().into(),
                );
                let frame = submission(
                    vec![
                        command(Kind::SetTransform { matrix }),
                        command(Kind::DrawGlyphRun {
                            antialias: Aa::Default,
                            run: run.clone(),
                            origin,
                            paint,
                        }),
                    ],
                    scale,
                );
                for device_clip in [
                    Rectangle::new(17.2, 14.1, 7.6, 24.3),
                    Rectangle::new(31.1, 20.2, 6.5, 22.3),
                    Rectangle::new(1.1, 1.2, 3.2, 3.3),
                ] {
                    let clip = damage_rect_to_aligned_clip(
                        Rectangle::new(
                            device_clip.x / scale,
                            device_clip.y / scale,
                            device_clip.width / scale,
                            device_clip.height / scale,
                        ),
                        64,
                        64,
                        scale,
                    )
                    .unwrap();
                    let prepared = antialias_frame::prepare(&encoder, &frame, true)
                        .unwrap()
                        .unwrap();
                    encoder.encode(&frame.commands, clip, scale, prepared);
                    if font_index == 1 && device_clip.x < 2.0 {
                        assert!(
                            encoder.scene.encoding().resources.patches.is_empty(),
                            "remote bitmap must not reach image encoding"
                        );
                    }
                    let repair = RepairTarget::new(clip, 64, 64, scale);
                    repair.translate(&mut encoder.scene);
                    let partial =
                        gpu.render_damage_target(&encoder.scene, repair.width, repair.height);
                    let (x, y, w, h) = damage_rect_to_copy_region(clip, 64, 64, scale).unwrap();
                    for py in y..y + h {
                        for px in x..x + w {
                            let a = ((py * 64 + px) * 4) as usize;
                            let b = (((py - repair.y) * repair.width + px - repair.x) * 4) as usize;
                            // Image sampling uses f32 transforms. Rebasing a rotated bitmap
                            // can change the final 8-bit rounding by one unit.
                            let tolerance =
                                u8::from(font_index == 1 && matrix != Affine2D::IDENTITY);
                            for channel in 0..4 {
                                assert!(
                                    full[a + channel].abs_diff(partial[b + channel]) <= tolerance,
                                    "font={font_index} dpi={scale} pixel=({px},{py}) matrix={matrix:?} full={:?} partial={:?}",
                                    &full[a..a + 4],
                                    &partial[b..b + 4]
                                );
                            }
                        }
                    }
                }
            }
        }
    }
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

// Proof-domain oracle only. No production capability or recording filter uses this helper.
fn structurally_empty_native_glyf(font: &skrifa::FontRef<'_>, glyph: u32) -> bool {
    use skrifa::{MetadataProvider, raw::TableProvider};
    if (font.colr().is_ok() && font.cpal().is_ok()) || !font.bitmap_strikes().is_empty() {
        return false;
    }
    let id = skrifa::GlyphId::new(glyph);
    let Some(outline) = font.outline_glyphs().get(id) else {
        return false;
    };
    if outline.format() != skrifa::outline::OutlineGlyphFormat::Glyf {
        return false;
    }
    let Ok(head) = font.head() else {
        return false;
    };
    let (Ok(loca), Ok(glyf)) = (
        font.loca(Some(head.index_to_loc_format() == 1)),
        font.glyf(),
    ) else {
        return false;
    };
    matches!(loca.get_glyf(id, &glyf), Ok(None))
}

#[test]
fn empty_native_glyf_domain_preserves_gpu_pixels_without_fragmenting_runs() {
    // Vello writes straight-alpha RGBA. Quantized alpha-zero pixels can retain tiny
    // RGB residuals after unpremultiplication; those channels have no contribution
    // when composited. Compare a canonical transparent representation while keeping
    // alpha and every nontransparent channel byte-exact.
    fn render(gpu: &mut Gpu, scene: &vello::Scene, aa: AaConfig) -> Vec<u8> {
        let mut pixels = gpu.render_with_aa(scene, aa);
        for pixel in pixels.chunks_exact_mut(4) {
            if pixel[3] == 0 {
                pixel[..3].fill(0);
            }
        }
        pixels
    }
    use novadraw::render::{
        BuiltinFont, FontDescriptor, ParleyTextEngine, ResourceId, TextConstraints,
    };
    use novadraw::text::shaping::GlyphRange;
    use skrifa::{
        MetadataProvider,
        instance::{NormalizedCoord, Size},
        outline::{DrawSettings, OutlinePen},
    };
    struct CountPen(usize);
    impl OutlinePen for CountPen {
        fn move_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn line_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn close(&mut self) {
            self.0 += 1;
        }
    }
    let bytes = BuiltinFont::Inter.bytes();
    let face = skrifa::FontRef::from_index(bytes, 0).unwrap();
    let font_id = ResourceId::new(uuid::Uuid::nil(), 91);
    let mut text = ParleyTextEngine::new();
    text.register_font(font_id, 1, bytes).unwrap();
    let layout = text
        .layout(
            "A A  B",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    assert_eq!(layout.glyph_runs().len(), 1);
    let original = &layout.glyph_runs()[0];
    let empty: Vec<_> = original
        .glyphs
        .iter()
        .filter(|glyph| structurally_empty_native_glyf(&face, glyph.id))
        .collect();
    assert!(!empty.is_empty(), "the oracle must exercise deletions");
    assert!(!structurally_empty_native_glyf(&face, u32::MAX));
    for glyph in &empty {
        let outline = face
            .outline_glyphs()
            .get(skrifa::GlyphId::new(glyph.id))
            .unwrap();
        for size in [0.125, 12.0, 72.0, 4096.0] {
            for bits in [0, -16384, 16384] {
                let coords = [NormalizedCoord::from_bits(bits)];
                let mut pen = CountPen(0);
                outline
                    .draw(
                        DrawSettings::unhinted(Size::new(size), coords.as_slice()),
                        &mut pen,
                    )
                    .unwrap();
                assert_eq!(pen.0, 0);
            }
        }
    }
    let mut compact = original.clone();
    compact.glyphs = GlyphRange::new(
        original
            .glyphs
            .iter()
            .copied()
            .filter(|glyph| !structurally_empty_native_glyf(&face, glyph.id))
            .collect(),
    )
    .unwrap();
    assert!(!compact.glyphs.is_empty());
    assert_eq!(compact.glyphs.len() + empty.len(), original.glyphs.len());
    let font = vello::peniko::FontData::new(bytes.to_vec().into(), 0);
    let mask = native_empty::EmptyGlyphs::new(bytes, 0);
    assert!(!mask.is_empty());
    assert!(mask.storage_bytes() <= 8192);
    assert!(!mask.contains(u32::MAX));
    assert!(native_empty::EmptyGlyphs::new(bytes, u32::MAX).is_empty());
    assert!(native_empty::EmptyGlyphs::new(&[0, 1, 2], 0).is_empty());
    for glyph in &original.glyphs {
        assert_eq!(
            mask.contains(glyph.id),
            structurally_empty_native_glyf(&face, glyph.id)
        );
    }
    let mut all_empty = original.clone();
    all_empty.glyphs = GlyphRange::new(empty.iter().map(|glyph| **glyph).collect()).unwrap();
    let paint = GlyphPaint::Fill(Color::WHITE.into());
    let mut gpu = Gpu::new();
    for scale in [0.75, 1.0, 2.0] {
        for skew in [None, Some(15.0)] {
            let mut original = original.clone();
            let mut compact = compact.clone();
            original.skew_degrees = skew;
            compact.skew_degrees = skew;
            for transform in [Affine2D::IDENTITY, Affine2D::from_translation(3.25, -2.75)] {
                let mut full = vello::Scene::new();
                let mut filtered = vello::Scene::new();
                let mut masked = vello::Scene::new();
                append_glyph_run(
                    &mut full,
                    &original,
                    &font,
                    Point::new(3.5, 16.25),
                    &paint,
                    &transform,
                    scale,
                );
                append_glyph_run(
                    &mut filtered,
                    &compact,
                    &font,
                    Point::new(3.5, 16.25),
                    &paint,
                    &transform,
                    scale,
                );
                append_filtered_glyph_run(
                    &mut masked,
                    &original,
                    (&font, Some(&mask), None),
                    Point::new(3.5, 16.25),
                    &paint,
                    &transform,
                    scale,
                );
                let mut empty_reference = vello::Scene::new();
                let mut empty_masked = vello::Scene::new();
                append_glyph_run(
                    &mut empty_reference,
                    &all_empty,
                    &font,
                    Point::new(3.5, 16.25),
                    &paint,
                    &transform,
                    scale,
                );
                append_filtered_glyph_run(
                    &mut empty_masked,
                    &all_empty,
                    (&font, Some(&mask), None),
                    Point::new(3.5, 16.25),
                    &paint,
                    &transform,
                    scale,
                );
                assert_eq!(
                    masked.encoding().resources.glyphs.len(),
                    compact.glyphs.len()
                );
                assert_eq!(masked.encoding().resources.glyph_runs.len(), 1);
                assert!(empty_masked.encoding().resources.glyphs.is_empty());
                for aa in [AaConfig::Area, AaConfig::Msaa8, AaConfig::Msaa16] {
                    let expected = render(&mut gpu, &full, aa);
                    assert!(expected.chunks_exact(4).any(|pixel| pixel[3] != 0));
                    let filtered_pixels = render(&mut gpu, &filtered, aa);
                    if filtered_pixels != expected {
                        let repeated_full = render(&mut gpu, &full, aa);
                        let repeated_filtered = render(&mut gpu, &filtered, aa);
                        let differences: Vec<_> = expected
                            .chunks_exact(4)
                            .zip(filtered_pixels.chunks_exact(4))
                            .enumerate()
                            .filter(|(_, (a, b))| a != b)
                            .map(|(index, (a, b))| (index % 64, index / 64, a, b))
                            .collect();
                        panic!(
                            "compacted-scene pixels differ: scale={scale}, skew={skew:?}, transform={transform:?}, aa={aa:?}; full_repeat={}, filtered_repeat={}, repeated_pair={}; pixels={differences:?}",
                            repeated_full == expected,
                            repeated_filtered == filtered_pixels,
                            repeated_full == repeated_filtered,
                        );
                    }
                    assert_eq!(render(&mut gpu, &masked, aa), expected);
                    let empty_pixels = render(&mut gpu, &empty_reference, aa);
                    assert!(empty_pixels.iter().all(|byte| *byte == 0));
                    assert_eq!(render(&mut gpu, &empty_masked, aa), empty_pixels);
                }
            }
        }
    }
}

#[test]
fn empty_native_glyf_cache_follows_resource_replacement_and_session_snapshot() {
    use novadraw::render::{
        BuiltinFont, FontData, FontDescriptor, ParleyTextEngine, ResourceDelta, ResourceId,
        ResourceOp, ResourceUpdate, TextConstraints,
    };
    let bytes = BuiltinFont::Inter.bytes();
    let font_id = ResourceId::new(uuid::Uuid::nil(), 93);
    let mut text = ParleyTextEngine::new();
    text.register_font(font_id, 1, bytes).unwrap();
    let layout = text
        .layout(
            "A A",
            &FontDescriptor::default(),
            TextConstraints::UNBOUNDED,
        )
        .unwrap();
    let run = layout.glyph_runs()[0].clone();
    let mut frame = submission(
        vec![command(Kind::DrawGlyphRun {
            antialias: Aa::Default,
            run,
            origin: Point::new(3.5, 16.25),
            paint: GlyphPaint::Fill(Color::WHITE.into()),
        })],
        1.0,
    );
    let update = ResourceUpdate {
        id: font_id,
        revision: 1,
        payload: ResourcePayload::Font(Arc::new(FontData::new(bytes.to_vec()))),
    };
    frame.resources = ResourceSync::Snapshot(ResourceSnapshot {
        ready: vec![update.clone()],
    });
    let mut encoder = SceneEncoder::new(1.0);
    let mut gate = BackendSessionGate::default();
    encoder.sync_resources(&mut gate, &frame).unwrap();
    encoded(&mut encoder, &frame);
    assert_eq!(encoder.empty_glyphs.len(), 1);
    assert!(encoder.ink_cache.len() > 0);
    let storage: usize = encoder
        .empty_glyphs
        .values()
        .map(|mask| mask.storage_bytes())
        .sum();
    assert!(storage > 0 && storage <= 8192);
    frame.resources = ResourceSync::Delta(ResourceDelta {
        ops: vec![ResourceOp::Upsert(update.clone())],
    });
    encoder.sync_resources(&mut gate, &frame).unwrap();
    assert!(encoder.empty_glyphs.is_empty());
    assert_eq!(encoder.ink_cache.len(), 0);
    encoded(&mut encoder, &frame);
    assert_eq!(encoder.empty_glyphs.len(), 1);
    frame.resources = ResourceSync::Snapshot(ResourceSnapshot {
        ready: vec![update.clone()],
    });
    encoder.sync_resources(&mut gate, &frame).unwrap();
    assert!(encoder.empty_glyphs.is_empty());
    assert_eq!(encoder.ink_cache.len(), 0);
    encoded(&mut encoder, &frame);
    frame.session_id = frame.session_id.next().unwrap();
    encoder.sync_resources(&mut gate, &frame).unwrap();
    assert!(encoder.empty_glyphs.is_empty());
    assert_eq!(encoder.ink_cache.len(), 0);
    encoded(&mut encoder, &frame);
    frame.resources = ResourceSync::Delta(ResourceDelta {
        ops: vec![ResourceOp::Remove(font_id)],
    });
    encoder.sync_resources(&mut gate, &frame).unwrap();
    assert!(encoder.empty_glyphs.is_empty());
    assert_eq!(encoder.ink_cache.len(), 0);
    assert!(encoder.font_faces.is_empty());
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
        .join("../../target/verification/per-operation-antialias");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(output.join("cost.json"), format!("[{}]\n", rows.join(","))).unwrap();
}
