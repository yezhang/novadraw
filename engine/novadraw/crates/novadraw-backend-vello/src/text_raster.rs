//! Opt-in, conservative command-derived raster experiment. No Figure identity is assumed.
use super::{ClipGeometry, RenderClip, RenderState, SceneEncoder, diagnostics};
use novadraw::{
    geometry::{Affine2D, Rectangle},
    graphics::AntialiasMode,
    render::{BackendSessionId, RenderCommand, RenderCommandKind as Kind, RenderFailure},
};
use std::{collections::BTreeMap, ops::Range};
use vello::{AaConfig, RenderParams, Renderer, Scene, peniko};

const TILE_HEIGHT: u32 = 512;
const BYTE_BUDGET: u64 = 16 * 1024 * 1024;
const MAX_COMMANDS: usize = 16_384;
const BYTES_PER_PIXEL: u64 = 4;

#[derive(Clone, Debug, PartialEq)]
struct Content {
    commands: Vec<RenderCommand>,
    scale: f64,
    raster_x: f64,
    width: u32,
    session: BackendSessionId,
}

pub(super) struct Candidate {
    content: Content,
    range: Range<usize>,
    origin: [f64; 2],
    visible: Rectangle,
}

/// The range contains all intervening draws, so replacing it cannot reorder them.
pub(super) fn candidate(
    commands: &[RenderCommand],
    scale: f64,
    width: u32,
    height: u32,
    session: BackendSessionId,
) -> Option<Candidate> {
    if commands.len() > MAX_COMMANDS {
        return None;
    }
    let start = commands
        .iter()
        .position(|c| matches!(c.kind, Kind::DrawGlyphRun { .. }))?;
    let end = commands
        .iter()
        .rposition(|c| matches!(c.kind, Kind::DrawGlyphRun { .. }))?
        + 1;
    let mut stack = vec![RenderState::default()];
    let mut origin = None;
    let mut clips = None;
    let mut flattened = Vec::new();
    for (index, command) in commands.iter().enumerate().take(end) {
        match &command.kind {
            Kind::PushState => stack.push(stack.last()?.clone()),
            Kind::PopState => {
                if stack.len() <= 1 {
                    return None;
                }
                stack.pop();
            }
            Kind::RestoreState => {
                let saved = stack.get(stack.len().checked_sub(2)?)?.clone();
                *stack.last_mut()? = saved;
            }
            Kind::ConcatTransform { matrix } => {
                let state = stack.last_mut()?;
                state.transform = state.transform.post_concat(*matrix);
            }
            Kind::SetTransform { matrix } => stack.last_mut()?.transform = *matrix,
            Kind::ResetTransform => stack.last_mut()?.transform = Affine2D::IDENTITY,
            Kind::Clip { rect } => {
                let state = stack.last_mut()?;
                state.clips.push(RenderClip {
                    transform: state.transform,
                    geometry: ClipGeometry::Rectangle(*rect),
                });
            }
            Kind::ResetClip => stack.last_mut()?.clips.clear(),
            Kind::ClipPath { .. } => return None,
            Kind::SetGlobalAlpha { .. } => {}
            kind if index < start => {
                let _ = kind;
            }
            kind => {
                if !matches!(kind, Kind::DrawGlyphRun { .. } | Kind::FillRect { .. })
                    || kind.antialias() == Some(AntialiasMode::Off)
                {
                    return None;
                }
                let state = stack.last()?;
                let [a, b, c, d, x, y] = state.transform.coeffs();
                if [a, b, c, d] != [1.0, 0.0, 0.0, 1.0] {
                    return None;
                }
                let base = *origin.get_or_insert([x, y]);
                if clips.as_ref().is_some_and(|saved| saved != &state.clips) {
                    return None;
                }
                clips.get_or_insert_with(|| state.clips.clone());
                flattened.push(RenderCommand {
                    kind: Kind::SetTransform {
                        matrix: Affine2D::from_translation(x - base[0], y - base[1]),
                    },
                });
                flattened.push(command.clone());
            }
        }
    }
    let origin = origin?;
    // Resampling at fractional device positions changes text coverage.
    if origin.iter().any(|v| (v * scale).fract() != 0.0) {
        return None;
    }
    let clips = clips?;
    let mut visible = Rectangle::new(
        0.0,
        0.0,
        f64::from(width) / scale,
        f64::from(height) / scale,
    );
    for clip in &clips {
        let ClipGeometry::Rectangle(rect) = clip.geometry else {
            return None;
        };
        let [a, b, c, d, x, y] = clip.transform.coeffs();
        if [a, b, c, d] != [1.0, 0.0, 0.0, 1.0] {
            return None;
        }
        let rect = Rectangle::new(rect.x + x, rect.y + y, rect.width, rect.height);
        visible = visible.intersection(rect)?;
    }
    if visible.width <= 0.0 || visible.height <= 0.0 {
        return None;
    }
    // Only effective visible clip edges matter; offscreen fractional edges cannot affect coverage.
    if [
        visible.x,
        visible.y,
        visible.x + visible.width,
        visible.y + visible.height,
    ]
    .iter()
    .any(|edge| (edge * scale).fract() != 0.0)
    {
        return None;
    }
    Some(Candidate {
        content: Content {
            commands: flattened,
            scale,
            raster_x: origin[0],
            width,
            session,
        },
        range: start..end,
        origin,
        visible,
    })
}

#[derive(Clone)]
pub(super) struct Composite {
    pub range: Range<usize>,
    pub tiles: Vec<(peniko::ImageData, [f64; 2])>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextRasterStatistics {
    pub hits: usize,
    pub misses: usize,
    pub resident_bytes: u64,
    pub eligible: bool,
    /// Glyphs in the final composite Scene; cold tile Scenes are counted by misses.
    pub encoded_glyphs: usize,
}

#[derive(Default)]
pub(super) struct Cache {
    content: Option<Content>,
    tiles: BTreeMap<i64, peniko::ImageData>,
    pub statistics: TextRasterStatistics,
}

impl Cache {
    pub fn clear(&mut self, renderer: &mut Renderer) {
        for (_, image) in std::mem::take(&mut self.tiles) {
            renderer.unregister_texture(image);
        }
        self.content = None;
        self.statistics = TextRasterStatistics::default();
    }

    pub fn prepare(
        &mut self,
        candidate: Candidate,
        encoder: &mut SceneEncoder,
        renderer: &mut Renderer,
        device: &vello::wgpu::Device,
        queue: &vello::wgpu::Queue,
    ) -> Result<Option<Composite>, RenderFailure> {
        let _scope = diagnostics::scope(c"backend-text-raster-cache");
        let scale = candidate.content.scale;
        let width = candidate.content.width;
        let bytes = u64::from(width) * u64::from(TILE_HEIGHT) * BYTES_PER_PIXEL;
        let first = ((candidate.visible.y - candidate.origin[1]) * scale / f64::from(TILE_HEIGHT))
            .floor() as i64;
        let last = (((candidate.visible.y + candidate.visible.height - candidate.origin[1])
            * scale)
            .ceil()
            - 1.0)
            / f64::from(TILE_HEIGHT);
        let last = last.floor() as i64;
        let count = last.saturating_sub(first).saturating_add(1) as u64;
        if bytes == 0
            || count.saturating_mul(bytes) > BYTE_BUDGET
            || width > device.limits().max_texture_dimension_2d
        {
            self.clear(renderer);
            return Ok(None);
        }
        if self.content.as_ref() != Some(&candidate.content) {
            self.clear(renderer);
            self.content = Some(candidate.content);
        }
        self.statistics = TextRasterStatistics {
            eligible: true,
            ..Default::default()
        };
        // Evict nonvisible registrations before allocating. Queued GPU work may retain old textures.
        while (self.tiles.len() as u64
            + (first..=last)
                .filter(|key| !self.tiles.contains_key(key))
                .count() as u64)
            * bytes
            > BYTE_BUDGET
        {
            let Some(key) = self
                .tiles
                .keys()
                .copied()
                .find(|key| *key < first || *key > last)
            else {
                break;
            };
            renderer.unregister_texture(self.tiles.remove(&key).expect("resident key"));
        }
        let mut tiles = Vec::new();
        for key in first..=last {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.tiles.entry(key) {
                let _raster = diagnostics::scope(c"backend-text-raster-miss");
                let texture = device.create_texture(&vello::wgpu::TextureDescriptor {
                    label: Some("Text raster tile"),
                    size: vello::wgpu::Extent3d {
                        width,
                        height: TILE_HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: vello::wgpu::TextureDimension::D2,
                    format: vello::wgpu::TextureFormat::Rgba8Unorm,
                    usage: vello::wgpu::TextureUsages::STORAGE_BINDING
                        | vello::wgpu::TextureUsages::TEXTURE_BINDING
                        | vello::wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let mut scene = Scene::new();
                encoder.scene = Scene::new();
                encoder.scale_factor = scale;
                encoder.state_stack = vec![RenderState::default()];
                for command in &self.content.as_ref().expect("content installed").commands {
                    encoder.render_command(command);
                }
                scene.append(
                    &encoder.scene,
                    Some(vello::kurbo::Affine::translate((
                        candidate.origin[0] * scale,
                        -(key as f64) * f64::from(TILE_HEIGHT),
                    ))),
                );
                let view = texture.create_view(&Default::default());
                renderer
                    .render_to_texture(
                        device,
                        queue,
                        &scene,
                        &view,
                        &RenderParams {
                            base_color: peniko::Color::TRANSPARENT,
                            width,
                            height: TILE_HEIGHT,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .map_err(super::render_failure_from_vello)?;
                entry.insert(renderer.register_texture(texture));
                self.statistics.misses += 1;
            } else {
                self.statistics.hits += 1;
            }
            tiles.push((
                self.tiles[&key].clone(),
                [
                    0.0,
                    candidate.origin[1] * scale + (key as f64) * f64::from(TILE_HEIGHT),
                ],
            ));
        }
        self.statistics.resident_bytes = self.tiles.len() as u64 * bytes;
        Ok(Some(Composite {
            range: candidate.range,
            tiles,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novadraw::{
        geometry::Point,
        graphics::Color,
        render::{GlyphPaint, ResourceId, TextEngine},
        text::{BuiltinFont, FontDescriptor, TextConstraints},
    };

    fn drawing() -> Vec<RenderCommand> {
        let mut text = TextEngine::new();
        text.register_font(
            ResourceId::new(uuid::Uuid::nil(), 1),
            1,
            BuiltinFont::Inter.bytes(),
        )
        .unwrap();
        let layout = text
            .layout(
                "cache",
                &FontDescriptor::default(),
                TextConstraints::UNBOUNDED,
            )
            .unwrap();
        let commands: Vec<_> = layout
            .glyph_runs()
            .iter()
            .map(|run| RenderCommand {
                kind: Kind::DrawGlyphRun {
                    run: run.clone(),
                    origin: Point::new(8.0, 24.0),
                    antialias: AntialiasMode::Default,
                    paint: GlyphPaint::Fill(Color::BLACK.into()),
                },
            })
            .collect();
        assert!(!commands.is_empty());
        commands
    }

    fn scroll(commands: &[RenderCommand], y: f64) -> Vec<RenderCommand> {
        let mut result = vec![
            RenderCommand {
                kind: Kind::Clip {
                    rect: Rectangle::new(0.0, 0.0, 200.0, 100.0),
                },
            },
            RenderCommand {
                kind: Kind::ConcatTransform {
                    matrix: Affine2D::from_translation(0.0, y),
                },
            },
        ];
        result.extend_from_slice(commands);
        result
    }

    #[test]
    fn integer_scroll_reuses_complete_content_but_not_changed_paint() {
        let commands = drawing();
        let session = BackendSessionId::initial(uuid::Uuid::nil());
        let first = candidate(&scroll(&commands, 0.0), 2.0, 400, 200, session)
            .unwrap_or_else(|| panic!("unexpectedly ineligible: {commands:?}"));
        let second = candidate(&scroll(&commands, -40.0), 2.0, 400, 200, session).unwrap();
        assert_eq!(first.content, second.content);
        assert_ne!(first.origin, second.origin);
        let mut changed = commands.clone();
        for command in &mut changed {
            if let Kind::DrawGlyphRun { paint, .. } = &mut command.kind {
                *paint =
                    novadraw::render::GlyphPaint::Fill(novadraw::graphics::Color::WHITE.into());
            }
        }
        assert_ne!(
            first.content,
            candidate(&scroll(&changed, 0.0), 2.0, 400, 200, session)
                .unwrap()
                .content
        );
    }

    #[test]
    fn fractional_scroll_and_intervening_unsupported_draw_fall_back() {
        let commands = drawing();
        let session = BackendSessionId::initial(uuid::Uuid::nil());
        assert!(candidate(&scroll(&commands, -0.25), 2.0, 400, 200, session).is_none());
        let mut interrupted = commands.clone();
        interrupted.push(RenderCommand {
            kind: Kind::ClearRect {
                rect: Rectangle::new(0.0, 0.0, 8.0, 8.0),
                color: novadraw::graphics::Color::WHITE,
            },
        });
        interrupted.extend_from_slice(&commands);
        assert!(candidate(&scroll(&interrupted, 0.0), 2.0, 400, 200, session).is_none());
        let mut clipped = commands.clone();
        clipped.push(RenderCommand {
            kind: Kind::Clip {
                rect: Rectangle::new(0.0, 0.0, 8.0, 8.0),
            },
        });
        clipped.extend_from_slice(&commands);
        assert!(candidate(&scroll(&clipped, 0.0), 2.0, 400, 200, session).is_none());
        let mut fractional_clip = scroll(&commands, 0.0);
        fractional_clip[0].kind = Kind::Clip {
            rect: Rectangle::new(0.25, 0.0, 199.75, 100.0),
        };
        assert!(candidate(&fractional_clip, 2.0, 400, 200, session).is_none());
        let mut offscreen_edge = scroll(&commands, 0.0);
        offscreen_edge.insert(
            2,
            RenderCommand {
                kind: Kind::Clip {
                    rect: Rectangle::new(0.0, 0.0, 200.0, 500.25),
                },
            },
        );
        assert!(candidate(&offscreen_edge, 2.0, 400, 200, session).is_some());
    }
}
