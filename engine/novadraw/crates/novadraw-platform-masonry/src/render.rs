//! Full-frame publication to the upstream imaging recording protocol.
//!
//! The host owns rasterization and DPI. Explicit per-operation AA is rejected:
//! imaging 0.0.1 has no operation-level coverage switch.

use masonry::imaging::{ClipRef, Composite, GeometryRef, Painter, record::Scene};
use masonry::kurbo::{Affine, BezPath, Cap, Ellipse, Join, Rect, Shape, Stroke};
use masonry::peniko::{self, Brush, Color, Fill, Gradient};
use novadraw::geometry::{Affine2D, Rectangle};
use novadraw::render::backend_support::{NormalizedPathOp, for_each_normalized};
use novadraw::render::command::Path;
use novadraw::render::{
    BackendCapabilities, BackendSessionDecision, BackendSessionGate, DamageMode, LineCap, LineJoin,
    Paint, RenderBackend, RenderCommandKind as Kind, RenderDisposition, RenderFailure,
    RenderFailureDiagnostic, RenderResult, RenderSubmission, ResourceId, ResourceOp,
    ResourcePayload, ResourceSync, StrokeStyle,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(crate) struct ImagingSceneRenderer {
    gate: BackendSessionGate,
    resources: HashMap<ResourceId, CachedResource>,
}

#[derive(Clone)]
struct CachedResource {
    revision: u64,
    image: Option<peniko::ImageData>,
}

struct SharedPixels(Arc<novadraw::render::ImageData>);
impl AsRef<[u8]> for SharedPixels {
    fn as_ref(&self) -> &[u8] {
        &self.0.pixels
    }
}
impl CachedResource {
    fn new(update: &novadraw::render::ResourceUpdate) -> Self {
        let image = match &update.payload {
            ResourcePayload::Image(data) => Some(peniko::ImageData {
                data: peniko::Blob::new(Arc::new(SharedPixels(Arc::clone(data)))),
                format: peniko::ImageFormat::Rgba8,
                alpha_type: peniko::ImageAlphaType::Alpha,
                width: data.width,
                height: data.height,
            }),
            ResourcePayload::Font(_) => None,
        };
        Self {
            revision: update.revision,
            image,
        }
    }
}

impl ImagingSceneRenderer {
    pub fn bind<'a>(&'a mut self, scene: &'a mut Scene) -> SceneBackend<'a> {
        SceneBackend {
            renderer: self,
            scene,
        }
    }
}

pub(crate) struct SceneBackend<'a> {
    renderer: &'a mut ImagingSceneRenderer,
    scene: &'a mut Scene,
}

fn failure(message: impl Into<String>) -> RenderFailure {
    RenderFailure::Fatal(RenderFailureDiagnostic::new(message))
}

impl RenderBackend for SceneBackend<'_> {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FULL_FRAME_ONLY
            .with_image_resources()
            .with_custom_strokes()
            .with_path_clips()
            .with_linear_gradients()
            .with_host_background()
    }

    fn submit(&mut self, submission: &RenderSubmission) -> RenderResult {
        let capabilities = self.capabilities();
        submission.validate_composition(capabilities)?;
        capabilities
            .validate_capabilities(&submission.commands)
            .map_err(RenderFailure::Unsupported)?;
        novadraw::render::validate_graphics_input(
            &submission.commands,
            submission.surface.scale_factor,
        )
        .map_err(RenderFailure::InvalidInput)?;
        let surface = submission.surface;
        if surface.pixel_width == 0 || surface.pixel_height == 0 {
            return Ok(RenderDisposition::Skipped);
        }
        if !surface.is_renderable() || submission.damage.mode() != DamageMode::Full {
            return Err(failure(
                "imaging target requires full-frame damage and a valid surface",
            ));
        }
        // Stage resources and scene together. A rejected/retried publication changes neither.
        let mut gate = self.renderer.gate;
        let decision = gate.accept(submission.session_id, &submission.resources);
        if matches!(
            decision,
            BackendSessionDecision::RejectStale | BackendSessionDecision::RejectMissingSnapshot
        ) {
            return Err(failure("invalid imaging backend submission session"));
        }
        let mut resources = if decision == BackendSessionDecision::Continue
            && !matches!(submission.resources, ResourceSync::Snapshot(_))
        {
            self.renderer.resources.clone()
        } else {
            HashMap::new()
        };
        match &submission.resources {
            ResourceSync::Snapshot(snapshot) => {
                for update in &snapshot.ready {
                    resources.insert(update.id, CachedResource::new(update));
                }
            }
            ResourceSync::Delta(delta) => {
                for op in &delta.ops {
                    match op {
                        ResourceOp::Upsert(update) => {
                            resources.insert(update.id, CachedResource::new(update));
                        }
                        ResourceOp::Remove(id) => {
                            resources.remove(id);
                        }
                    }
                }
            }
        }
        for command in &submission.commands {
            if let Kind::Image { image, .. } = &command.kind {
                match resources.get(&image.resource_id()) {
                    Some(actual)
                        if actual.revision == image.revision() && actual.image.is_some() => {}
                    _ => return Ok(RenderDisposition::Retry),
                }
            }
        }
        let bounds = Rect::new(
            0.0,
            0.0,
            f64::from(surface.pixel_width) / surface.scale_factor,
            f64::from(surface.pixel_height) / surface.scale_factor,
        );
        let mut candidate = Scene::new();
        let mut painter: Painter<'_> = Painter::new(&mut candidate);
        painter.push_fill_clip(bounds);
        if let Some(clear) = submission.clear_color {
            painter.fill_rect(bounds, color(clear));
        }
        let mut stack = vec![State::default()];
        for command in &submission.commands {
            let transform = stack.last().expect("root state").transform;
            match &command.kind {
                Kind::PushState => stack.push(stack.last().expect("root state").clone()),
                Kind::RestoreState | Kind::PopState if stack.len() > 1 => {
                    let saved = stack[stack.len() - 2].clone();
                    restore_clips(
                        &mut painter,
                        &stack.last().expect("root state").clips,
                        &saved.clips,
                    );
                    if matches!(command.kind, Kind::PopState) {
                        stack.pop();
                    } else {
                        *stack.last_mut().expect("root state") = saved;
                    }
                }
                Kind::RestoreState | Kind::PopState => {}
                Kind::ConcatTransform { matrix } => {
                    stack.last_mut().expect("root state").transform = transform * affine(*matrix);
                }
                Kind::SetTransform { matrix } => {
                    stack.last_mut().expect("root state").transform = affine(*matrix)
                }
                Kind::ResetTransform => {
                    stack.last_mut().expect("root state").transform = Affine::IDENTITY
                }
                // Graphics already multiplied alpha into each immutable paint.
                Kind::SetGlobalAlpha { .. } => {}
                Kind::Clip { rect: area } => {
                    let clip = Clip {
                        transform,
                        shape: GeometryRef::Rect(rect(*area)).to_owned(),
                        rule: Fill::NonZero,
                    };
                    clip.push(&mut painter);
                    stack.last_mut().expect("root state").clips.push(clip);
                }
                Kind::ClipPath { clip } => {
                    let clip = Clip {
                        transform,
                        shape: GeometryRef::OwnedPath(path(clip.path())).to_owned(),
                        rule: fill(clip.rule()),
                    };
                    clip.push(&mut painter);
                    stack.last_mut().expect("root state").clips.push(clip);
                }
                Kind::ResetClip => {
                    for _ in &stack.last().expect("root state").clips {
                        painter.pop_clip();
                    }
                    stack.last_mut().expect("root state").clips.clear();
                }
                Kind::ClearRect {
                    rect: area,
                    color: clear,
                } => {
                    painter
                        .fill(rect(*area), color(*clear))
                        .transform(transform)
                        .draw();
                }
                Kind::FillRect {
                    rect: area,
                    paint,
                    rule,
                    ..
                } => {
                    painter
                        .fill(rect(*area), &brush(paint))
                        .transform(transform)
                        .fill_rule(fill(*rule))
                        .draw();
                }
                Kind::StrokeRect {
                    rect: area,
                    paint,
                    stroke: style,
                    ..
                } => {
                    painter
                        .stroke(rect(*area), &stroke(style), &brush(paint))
                        .transform(transform)
                        .draw();
                }
                Kind::Ellipse {
                    cx,
                    cy,
                    rx,
                    ry,
                    fill_paint,
                    stroke_paint,
                    stroke: style,
                    rule,
                    ..
                } => {
                    let ellipse = Ellipse::new((*cx, *cy), (*rx, *ry), 0.0);
                    let shape = ellipse.to_path(CURVE_TOLERANCE);
                    if let Some(paint) = fill_paint {
                        painter
                            .fill(&shape, &brush(paint))
                            .transform(transform)
                            .fill_rule(fill(*rule))
                            .draw();
                    }
                    if let Some(paint) = stroke_paint {
                        painter
                            .stroke(&shape, &stroke(style), &brush(paint))
                            .transform(transform)
                            .draw();
                    }
                }
                Kind::Line {
                    p1,
                    p2,
                    paint,
                    stroke: style,
                    ..
                } => {
                    let mut shape = BezPath::new();
                    shape.move_to((p1.x(), p1.y()));
                    shape.line_to((p2.x(), p2.y()));
                    painter
                        .stroke(&shape, &stroke(style), &brush(paint))
                        .transform(transform)
                        .draw();
                }
                Kind::Polyline {
                    points,
                    paint,
                    stroke: style,
                    ..
                } => {
                    let mut shape = BezPath::new();
                    for (index, point) in points.iter().enumerate() {
                        if index == 0 {
                            shape.move_to((point.x(), point.y()));
                        } else {
                            shape.line_to((point.x(), point.y()));
                        }
                    }
                    painter
                        .stroke(&shape, &stroke(style), &brush(paint))
                        .transform(transform)
                        .draw();
                }
                Kind::FillPath {
                    path: shape,
                    paint,
                    rule,
                    ..
                } => {
                    painter
                        .fill(path(shape), &brush(paint))
                        .transform(transform)
                        .fill_rule(fill(*rule))
                        .draw();
                }
                Kind::StrokePath {
                    path: shape,
                    paint,
                    stroke: style,
                    ..
                } => {
                    painter
                        .stroke(path(shape), &stroke(style), &brush(paint))
                        .transform(transform)
                        .draw();
                }
                // Default Core text uses prepared outline paths. Native glyph runs
                // are not advertised: upstream cannot preserve all glyph brush/dash semantics.
                Kind::DrawGlyphRun { .. } => {
                    return Err(failure("native glyph runs are unsupported"));
                }
                Kind::Image {
                    image,
                    source_rect,
                    dest_rect,
                    alpha,
                } => {
                    let data = resources[&image.resource_id()]
                        .image
                        .as_ref()
                        .expect("validated image resource");
                    let disposition =
                        novadraw::render::backend_support::validate_image_draw_geometry(
                            data.width,
                            data.height,
                            *source_rect,
                            *dest_rect,
                        )
                        .map_err(|error| failure(error.to_string()))?;
                    if disposition == novadraw::render::backend_support::ImageDrawDisposition::NoOp
                    {
                        continue;
                    }
                    let image_brush = peniko::ImageBrush::new(data.clone());
                    let image_transform = transform
                        * Affine::translate((dest_rect.x, dest_rect.y))
                        * Affine::scale_non_uniform(
                            dest_rect.width / source_rect.width,
                            dest_rect.height / source_rect.height,
                        )
                        * Affine::translate((-source_rect.x, -source_rect.y));
                    painter.push_fill_clip_transformed(rect(*dest_rect), transform);
                    painter
                        .fill(
                            Rect::new(0.0, 0.0, f64::from(data.width), f64::from(data.height)),
                            &image_brush,
                        )
                        .transform(image_transform)
                        .composite(Composite {
                            alpha: *alpha as f32,
                            ..Composite::default()
                        })
                        .draw();
                    painter.pop_clip();
                }
            }
        }
        for _ in &stack.last().expect("root state").clips {
            painter.pop_clip();
        }
        painter.pop_clip();
        candidate
            .validate()
            .map_err(|error| failure(error.to_string()))?;
        self.renderer.gate = gate;
        self.renderer.resources = resources;
        *self.scene = candidate;
        Ok(RenderDisposition::Presented)
    }

    fn resize(&mut self, _: u32, _: u32, _: f64) {}
}

const CURVE_TOLERANCE: f64 = 0.01;

#[derive(Clone)]
struct State {
    transform: Affine,
    clips: Vec<Clip>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            transform: Affine::IDENTITY,
            clips: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct Clip {
    transform: Affine,
    shape: masonry::imaging::record::Geometry,
    rule: Fill,
}
impl Clip {
    fn push(&self, painter: &mut Painter<'_>) {
        painter.push_clip(
            ClipRef::fill_with_rule(&self.shape, self.rule).with_transform(self.transform),
        );
    }
}

fn restore_clips(painter: &mut Painter<'_>, current: &[Clip], saved: &[Clip]) {
    let prefix = current
        .iter()
        .zip(saved)
        .take_while(|(a, b)| a == b)
        .count();
    for _ in prefix..current.len() {
        painter.pop_clip();
    }
    for clip in &saved[prefix..] {
        clip.push(painter);
    }
}

fn affine(matrix: Affine2D) -> Affine {
    Affine::new(matrix.coeffs())
}
fn rect(area: Rectangle) -> Rect {
    Rect::new(area.x, area.y, area.x + area.width, area.y + area.height)
}
fn color(value: novadraw::graphics::Color) -> Color {
    Color::new([
        value.red() as f32,
        value.green() as f32,
        value.blue() as f32,
        value.alpha() as f32,
    ])
}
fn fill(rule: novadraw::render::FillRule) -> Fill {
    match rule {
        novadraw::render::FillRule::NonZero => Fill::NonZero,
        novadraw::render::FillRule::EvenOdd => Fill::EvenOdd,
    }
}
fn brush(paint: &Paint) -> Brush {
    match paint {
        Paint::Solid(value) => color(*value).into(),
        Paint::LinearGradient(gradient) => Gradient::new_linear(
            (gradient.start().x(), gradient.start().y()),
            (gradient.end().x(), gradient.end().y()),
        )
        .with_interpolation_cs(peniko::color::ColorSpaceTag::Srgb)
        .with_interpolation_alpha_space(peniko::InterpolationAlphaSpace::Premultiplied)
        .with_stops(
            gradient
                .stops()
                .iter()
                .map(|stop| peniko::ColorStop {
                    offset: stop.offset() as f32,
                    color: color(stop.color()).into(),
                })
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .into(),
    }
}
fn stroke(style: &StrokeStyle) -> Stroke {
    Stroke::new(style.width())
        .with_caps(match style.cap() {
            LineCap::Butt => Cap::Butt,
            LineCap::Round => Cap::Round,
            LineCap::Square => Cap::Square,
        })
        .with_join(match style.effective_join() {
            LineJoin::Miter => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        })
        .with_miter_limit(style.miter_limit())
        .with_dashes(
            style.normalized_dash_offset(),
            style.dash_lengths().iter().copied(),
        )
}
fn path(value: &Path) -> BezPath {
    let mut result = BezPath::new();
    for_each_normalized(value.operations(), 1.0, |operation| match operation {
        NormalizedPathOp::MoveTo(p) => result.move_to((p.x(), p.y())),
        NormalizedPathOp::LineTo(p) => result.line_to((p.x(), p.y())),
        NormalizedPathOp::CubicTo(c) => result.curve_to(
            (c.control1.x(), c.control1.y()),
            (c.control2.x(), c.control2.y()),
            (c.end.x(), c.end.y()),
        ),
        NormalizedPathOp::QuadTo { control, end } => {
            result.quad_to((control.x(), control.y()), (end.x(), end.y()))
        }
        NormalizedPathOp::Close => result.close_path(),
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use masonry_imaging::{ImageRenderer, vello};
    use novadraw::graphics::{AntialiasMode, Color as NdColor};
    use novadraw::render::{
        DamageSet, FrameId, ImageData, ImageResourceRef, ResourceSnapshot, ResourceUpdate,
        SurfaceInfo,
    };
    use std::sync::Arc;

    fn frame(commands: Vec<Kind>) -> RenderSubmission {
        let mut damage = DamageSet::default();
        damage.set_full();
        RenderSubmission {
            commands: commands
                .into_iter()
                .map(|kind| novadraw::render::RenderCommand { kind })
                .collect(),
            damage,
            resources: ResourceSync::Snapshot(ResourceSnapshot::default()),
            retention: None,
            surface: SurfaceInfo {
                logical_width: 64.0,
                logical_height: 64.0,
                pixel_width: 64,
                pixel_height: 64,
                scale_factor: 1.0,
            },
            clear_color: Some(NdColor::WHITE),
            background_input: None,
            session_id: Default::default(),
            frame_id: FrameId::INITIAL,
        }
    }
    fn rectangle(area: Rectangle, paint: impl Into<Paint>) -> Kind {
        Kind::FillRect {
            antialias: AntialiasMode::Default,
            rect: area,
            paint: paint.into(),
            rule: novadraw::render::FillRule::NonZero,
        }
    }
    fn pixels(scene: &mut Scene) -> image_pixels::Image {
        let image = vello::new_headless_renderer()
            .expect("GPU required")
            .render_source(scene, 64, 64)
            .expect("GPU rendering");
        image_pixels::Image { data: image.data }
    }
    mod image_pixels {
        pub struct Image {
            pub data: Vec<u8>,
        }
        impl Image {
            pub fn at(&self, x: usize, y: usize) -> &[u8] {
                let index = (y * 64 + x) * 4;
                &self.data[index..index + 4]
            }
        }
    }

    #[test]
    fn captured_clip_transform_restore_and_default_coverage_reach_gpu() {
        let red = NdColor::rgba8(255, 0, 0, 255);
        let blue = NdColor::rgba8(0, 0, 255, 255);
        let submission = frame(vec![
            Kind::PushState,
            Kind::SetTransform {
                matrix: Affine2D::from_translation(10.0, 0.0),
            },
            Kind::Clip {
                rect: Rectangle::new(0.0, 0.0, 10.0, 20.0),
            },
            Kind::ResetTransform,
            rectangle(Rectangle::new(0.0, 0.0, 40.0, 40.0), red),
            Kind::RestoreState,
            rectangle(Rectangle::new(30.0, 0.0, 10.0, 10.0), blue),
            Kind::PopState,
        ]);
        let mut renderer = ImagingSceneRenderer::default();
        let mut scene = Scene::new();
        assert_eq!(
            renderer.bind(&mut scene).submit(&submission).unwrap(),
            RenderDisposition::Presented
        );
        let image = pixels(&mut scene);
        assert_eq!(image.at(15, 5), &[255, 0, 0, 255]);
        assert_eq!(image.at(5, 5), &[255, 255, 255, 255]);
        assert_eq!(image.at(35, 5), &[0, 0, 255, 255]);
    }

    #[test]
    fn unsupported_aa_retry_and_stale_session_preserve_published_target() {
        let mut renderer = ImagingSceneRenderer::default();
        let mut scene = Scene::new();
        let mut submission = frame(vec![rectangle(
            Rectangle::new(0.0, 0.0, 20.0, 20.0),
            NdColor::BLACK,
        )]);
        renderer.bind(&mut scene).submit(&submission).unwrap();
        let published = scene.clone();
        if let Kind::FillRect { antialias, .. } = &mut submission.commands[0].kind {
            *antialias = AntialiasMode::Off;
        }
        assert!(matches!(
            renderer.bind(&mut scene).submit(&submission),
            Err(RenderFailure::Unsupported(_))
        ));
        assert_eq!(scene, published);
        let id = ResourceId::new(submission.session_id.runtime_namespace(), 1);
        submission.commands = vec![novadraw::render::RenderCommand {
            kind: Kind::Image {
                image: ImageResourceRef::new(id, 2, 2, 1, 1.0),
                source_rect: Rectangle::new(0.0, 0.0, 2.0, 1.0),
                dest_rect: Rectangle::new(0.0, 0.0, 20.0, 10.0),
                alpha: 1.0,
            },
        }];
        assert_eq!(
            renderer.bind(&mut scene).submit(&submission).unwrap(),
            RenderDisposition::Retry
        );
        assert_eq!(scene, published);
        assert_eq!(renderer.gate.active_session(), Some(submission.session_id));
        assert!(renderer.resources.is_empty());
        let next_session = submission.session_id.next().unwrap();
        submission.session_id = next_session;
        submission.commands.clear();
        renderer.bind(&mut scene).submit(&submission).unwrap();
        let newer = scene.clone();
        submission.session_id = Default::default();
        assert!(renderer.bind(&mut scene).submit(&submission).is_err());
        assert_eq!(scene, newer);
        assert_eq!(renderer.gate.active_session(), Some(next_session));
    }

    #[test]
    fn image_revision_crop_and_delta_removal_preserve_gpu_pixels() {
        let mut renderer = ImagingSceneRenderer::default();
        let mut scene = Scene::new();
        let mut submission = frame(vec![]);
        let id = ResourceId::new(submission.session_id.runtime_namespace(), 2);
        submission.resources = ResourceSync::Snapshot(ResourceSnapshot {
            ready: vec![ResourceUpdate {
                id,
                revision: 1,
                payload: ResourcePayload::Image(Arc::new(ImageData::from_rgba(
                    2,
                    1,
                    vec![255, 0, 0, 255, 0, 0, 255, 255],
                    1.0,
                ))),
            }],
        });
        submission.commands.push(novadraw::render::RenderCommand {
            kind: Kind::Image {
                image: ImageResourceRef::new(id, 1, 2, 1, 1.0),
                source_rect: Rectangle::new(1.0, 0.0, 1.0, 1.0),
                dest_rect: Rectangle::new(10.0, 10.0, 20.0, 20.0),
                alpha: 1.0,
            },
        });
        renderer.bind(&mut scene).submit(&submission).unwrap();
        assert_eq!(pixels(&mut scene).at(25, 20), &[0, 0, 255, 255]);
        let published = scene.clone();
        submission.resources = ResourceSync::Delta(novadraw::render::ResourceDelta {
            ops: vec![ResourceOp::Remove(id)],
        });
        assert_eq!(
            renderer.bind(&mut scene).submit(&submission).unwrap(),
            RenderDisposition::Retry
        );
        assert_eq!(scene, published);
        // Removal is transactional while a frame still needs that resource.
        assert!(renderer.resources.contains_key(&id));
    }
}
