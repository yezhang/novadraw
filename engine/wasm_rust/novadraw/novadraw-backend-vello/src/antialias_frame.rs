//! Whole-frame, fallible preparation. Publication follows resource acceptance.

use super::{
    SceneEncoder,
    antialias::{AntialiasConfig, Work, limit_failure, reserve, sample},
    antialias_stroke::{self, Style},
    glyph_outline,
};
use novadraw::{
    geometry::Affine2D,
    graphics::{AntialiasMode, FillRule, StrokeStyle},
    render::{
        BackendSessionId, GlyphPaint, RenderCommandKind as Kind, RenderFailure, RenderSubmission,
        ResourceOp, ResourcePayload, ResourceSync,
        backend_support::{NormalizedPathOp, for_each_normalized},
    },
};
use std::{collections::HashMap, sync::Arc};
use vello::kurbo::{Affine, BezPath, Cap, Ellipse, Join, PathEl, Shape};

#[derive(Clone, Copy, PartialEq)]
struct Domain {
    session: BackendSessionId,
    width: u32,
    height: u32,
    scale: f64,
}

struct Entry {
    path: Arc<BezPath>,
    work: Work,
    storage_bytes: usize,
}

#[derive(Default)]
pub(super) struct Cache {
    domain: Option<Domain>,
    entries: HashMap<Vec<u64>, Entry>,
}

pub(super) struct Draw {
    pub path: Arc<BezPath>,
    pub paint_slot: usize,
    pub brush_transform: Affine,
}

#[derive(Default)]
pub(super) struct PreparedFrame {
    pub commands: Vec<(usize, Vec<Draw>)>,
    cache: Cache,
}

impl PreparedFrame {
    pub fn publish(self, encoder: &mut SceneEncoder) -> Vec<(usize, Vec<Draw>)> {
        encoder.antialias_cache = self.cache;
        self.commands
    }

    #[cfg(test)]
    pub fn storage(&self) -> (usize, usize) {
        let elements = self
            .commands
            .iter()
            .flat_map(|(_, draws)| draws)
            .map(|draw| draw.path.elements().len())
            .sum();
        let bytes = self
            .cache
            .entries
            .iter()
            .map(|(key, entry)| key.capacity() * std::mem::size_of::<u64>() + entry.storage_bytes)
            .sum();
        (elements, bytes)
    }
}

struct Preparation<'a> {
    domain: Domain,
    config: AntialiasConfig,
    old: &'a Cache,
    next: Cache,
    cache_bytes: usize,
    operation: Work,
    frame: Work,
    index: usize,
}

impl Preparation<'_> {
    fn remaining(&self) -> super::antialias::AntialiasWorkLimits {
        let a = self.operation.remaining(self.config.operation);
        let b = self.frame.remaining(self.config.frame);
        super::antialias::AntialiasWorkLimits {
            rows: a.rows.min(b.rows),
            segments: a.segments.min(b.segments),
            crossings: a.crossings.min(b.crossings),
            runs: a.runs.min(b.runs),
            elements: a.elements.min(b.elements),
        }
    }

    fn charge(&mut self, amount: Work) -> Result<(), RenderFailure> {
        self.operation
            .add(amount, self.config.operation, self.index)?;
        self.frame.add(amount, self.config.frame, self.index)
    }

    fn input(&mut self, iter: impl IntoIterator<Item = PathEl>) -> Result<BezPath, RenderFailure> {
        let mut elements = Vec::new();
        for element in iter {
            self.charge(Work {
                elements: 1,
                ..Work::default()
            })?;
            reserve(&mut elements, 1)?;
            elements.push(element);
        }
        Ok(BezPath::from_vec(elements))
    }

    fn path(&mut self, path: &novadraw::graphics::Path) -> Result<BezPath, RenderFailure> {
        let remaining = self.remaining().elements;
        if path.operations().len() > remaining {
            return Err(limit_failure(
                self.index,
                "input path elements",
                path.operations().len(),
                remaining,
            ));
        }
        let mut elements = Vec::new();
        let mut failure = None;
        for_each_normalized(path.operations(), 1.0, |op| {
            if failure.is_some() {
                return;
            }
            let point = |p: novadraw::geometry::Point| (p.x(), p.y()).into();
            let element = match op {
                NormalizedPathOp::MoveTo(p) => PathEl::MoveTo(point(p)),
                NormalizedPathOp::LineTo(p) => PathEl::LineTo(point(p)),
                NormalizedPathOp::CubicTo(c) => {
                    PathEl::CurveTo(point(c.control1), point(c.control2), point(c.end))
                }
                NormalizedPathOp::QuadTo { control, end } => {
                    PathEl::QuadTo(point(control), point(end))
                }
                NormalizedPathOp::Close => PathEl::ClosePath,
            };
            match self
                .charge(Work {
                    elements: 1,
                    ..Work::default()
                })
                .and_then(|()| reserve(&mut elements, 1))
            {
                Ok(()) => elements.push(element),
                Err(error) => failure = Some(error),
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(BezPath::from_vec(elements))
    }

    fn draw(
        &mut self,
        mut path: BezPath,
        rule: FillRule,
        stroke: Option<&StrokeStyle>,
        device: Affine,
        brush_transform: Affine,
        paint_slot: usize,
    ) -> Result<Draw, RenderFailure> {
        let mut key = Vec::new();
        // Exact bit identities are verified by HashMap equality, including collisions.
        reserve(
            &mut key,
            path.elements().len().saturating_mul(7).saturating_add(16),
        )?;
        key.extend(device.as_coeffs().map(f64::to_bits));
        key.push(u64::from(rule == FillRule::EvenOdd));
        for el in path.elements() {
            match el {
                PathEl::MoveTo(p) => key.extend([0, p.x.to_bits(), p.y.to_bits()]),
                PathEl::LineTo(p) => key.extend([1, p.x.to_bits(), p.y.to_bits()]),
                PathEl::QuadTo(p, q) => key.extend([
                    2,
                    p.x.to_bits(),
                    p.y.to_bits(),
                    q.x.to_bits(),
                    q.y.to_bits(),
                ]),
                PathEl::CurveTo(p, q, r) => key.extend([
                    3,
                    p.x.to_bits(),
                    p.y.to_bits(),
                    q.x.to_bits(),
                    q.y.to_bits(),
                    r.x.to_bits(),
                    r.y.to_bits(),
                ]),
                PathEl::ClosePath => key.push(4),
            }
        }
        key.push(5);
        if let Some(stroke) = stroke {
            let dashes = stroke.dash_lengths();
            // Custom patterns are borrowed input of arbitrary length. Admit them
            // before copying into the cache identity or expanding any outline.
            self.charge(Work {
                elements: dashes.len(),
                ..Work::default()
            })?;
            reserve(&mut key, dashes.len().saturating_add(5))?;
            key.extend([
                stroke.width().to_bits(),
                stroke.miter_limit().to_bits(),
                stroke.cap() as u64,
                stroke.join() as u64,
                stroke.normalized_dash_offset().to_bits(),
            ]);
            key.extend(dashes.iter().map(|value| value.to_bits()));
        }
        let cached = self.next.entries.get(&key).or_else(|| {
            (self.old.domain == Some(self.domain))
                .then(|| self.old.entries.get(&key))
                .flatten()
        });
        let (mask, work, storage_bytes) = if let Some(entry) = cached {
            (Arc::clone(&entry.path), entry.work, entry.storage_bytes)
        } else {
            let mut work = Work::default();
            let limits = self.remaining();
            if let Some(stroke) = stroke {
                let cap = match stroke.cap() {
                    novadraw::graphics::LineCap::Butt => Cap::Butt,
                    novadraw::graphics::LineCap::Round => Cap::Round,
                    novadraw::graphics::LineCap::Square => Cap::Square,
                };
                let dashes = stroke.dash_lengths();
                let style = Style {
                    width: stroke.width(),
                    miter_limit: stroke.miter_limit(),
                    join: match stroke.effective_join() {
                        novadraw::graphics::LineJoin::Miter => Join::Miter,
                        novadraw::graphics::LineJoin::Round => Join::Round,
                        novadraw::graphics::LineJoin::Bevel => Join::Bevel,
                    },
                    start_cap: cap,
                    end_cap: cap,
                    dash_pattern: &dashes,
                    dash_offset: stroke.normalized_dash_offset(),
                };
                let c = device.as_coeffs();
                // Frobenius norm bounds amplification, including skew/nonuniform scale.
                let bound = c[..4].iter().map(|v| v * v).sum::<f64>().sqrt();
                let (outline, amount) =
                    antialias_stroke::expand(&path, &style, bound, limits, self.index)?;
                work.add(amount, limits, self.index)?;
                path = outline;
            }
            path.apply_affine(device);
            if !path.is_finite() {
                return Err(RenderFailure::InvalidInput(
                    novadraw::render::InvalidGraphicsInput {
                        command_index: self.index,
                        reason: novadraw::render::GraphicsInputError::DeviceValueOverflow {
                            field: "antialias outline",
                        },
                    },
                ));
            }
            let (mask, amount) = sample(
                &path,
                if stroke.is_some() {
                    FillRule::NonZero
                } else {
                    rule
                },
                self.domain.width,
                self.domain.height,
                work.remaining(limits),
                self.index,
            )?;
            work.add(amount, limits, self.index)?;
            let elements = mask.into_elements();
            let storage_bytes = elements
                .capacity()
                .saturating_mul(std::mem::size_of::<PathEl>());
            (Arc::new(BezPath::from_vec(elements)), work, storage_bytes)
        };
        // Cache hits still consume admission budgets.
        self.charge(work)?;
        let bytes = key
            .capacity()
            .saturating_mul(std::mem::size_of::<u64>())
            .saturating_add(storage_bytes)
            // Conservative bucket/control/Arc allowance, including the first allocation.
            .saturating_add(
                4 * (std::mem::size_of::<Entry>() + std::mem::size_of::<Vec<u64>>() + 16),
            );
        if !self.next.entries.contains_key(&key)
            && bytes <= self.config.cache_bytes.saturating_sub(self.cache_bytes)
        {
            self.next.entries.try_reserve(1).map_err(|error| {
                RenderFailure::OutOfMemory(novadraw::render::RenderFailureDiagnostic::from_error(
                    &error,
                ))
            })?;
            self.next.entries.insert(
                key,
                Entry {
                    path: Arc::clone(&mask),
                    work,
                    storage_bytes,
                },
            );
            self.cache_bytes += bytes;
        }
        Ok(Draw {
            path: mask,
            paint_slot,
            brush_transform,
        })
    }
}

fn font_for(
    encoder: &SceneEncoder,
    submission: &RenderSubmission,
    font: &novadraw::render::FontFaceRef,
    same_session: bool,
) -> Option<vello::peniko::FontData> {
    let update = match &submission.resources {
        ResourceSync::Snapshot(snapshot) => {
            let update = snapshot
                .ready
                .iter()
                .rev()
                .find(|item| item.id == font.resource_id())?;
            Some(update)
        }
        ResourceSync::Delta(delta) => match delta.ops.iter().rev().find(|op| match op {
            ResourceOp::Upsert(item) => item.id == font.resource_id(),
            ResourceOp::Remove(id) => *id == font.resource_id(),
        }) {
            Some(ResourceOp::Upsert(item)) => Some(item),
            Some(ResourceOp::Remove(_)) => return None,
            None => None,
        },
    };
    let blob = if let Some(update) = update {
        let ResourcePayload::Font(data) = &update.payload else {
            return None;
        };
        if update.revision != font.revision() {
            return None;
        }
        vello::peniko::Blob::new(data.shared_bytes())
    } else {
        if !same_session {
            return None;
        }
        encoder
            .font_faces
            .get(&(font.resource_id(), font.revision()))?
            .clone()
    };
    Some(vello::peniko::FontData::new(blob, font.collection_index()))
}

pub(super) fn prepare(
    encoder: &SceneEncoder,
    submission: &RenderSubmission,
    same_session: bool,
) -> Result<Option<PreparedFrame>, RenderFailure> {
    if !submission
        .commands
        .iter()
        .any(|cmd| cmd.kind.antialias() == Some(AntialiasMode::Off))
    {
        return Ok(Some(PreparedFrame::default()));
    }
    if submission.commands.len() > encoder.antialias_config.frame.elements {
        return Err(limit_failure(
            0,
            "frame commands",
            submission.commands.len(),
            encoder.antialias_config.frame.elements,
        ));
    }
    let surface = submission.surface;
    let domain = Domain {
        session: submission.session_id,
        width: surface.pixel_width,
        height: surface.pixel_height,
        scale: surface.scale_factor,
    };
    let mut prep = Preparation {
        domain,
        config: encoder.antialias_config,
        old: &encoder.antialias_cache,
        next: Cache {
            domain: Some(domain),
            entries: HashMap::new(),
        },
        cache_bytes: 0,
        operation: Work::default(),
        frame: Work::default(),
        index: 0,
    };
    let mut result = Vec::new();
    let mut transforms = Vec::new();
    reserve(&mut transforms, 1)?;
    transforms.push(Affine2D::IDENTITY);
    for (index, cmd) in submission.commands.iter().enumerate() {
        let current = *transforms.last().expect("initial transform");
        match &cmd.kind {
            Kind::PushState => {
                reserve(&mut transforms, 1)?;
                transforms.push(current);
            }
            Kind::RestoreState if transforms.len() > 1 => {
                let saved = transforms[transforms.len() - 2];
                *transforms.last_mut().expect("transform") = saved;
            }
            Kind::PopState if transforms.len() > 1 => {
                transforms.pop();
            }
            Kind::SetTransform { matrix } => *transforms.last_mut().expect("transform") = *matrix,
            Kind::ConcatTransform { matrix } => {
                *transforms.last_mut().expect("transform") = current.post_concat(*matrix)
            }
            Kind::ResetTransform => *transforms.last_mut().expect("transform") = Affine2D::IDENTITY,
            _ => {}
        }
        if cmd.kind.antialias() != Some(AntialiasMode::Off) {
            continue;
        }
        prep.index = index;
        prep.operation = Work::default();
        let device = Affine::scale(surface.scale_factor) * Affine::new(current.coeffs());
        // vello_paint already scales gradient coordinates by DPI.
        let brush = SceneEncoder::transform_to_affine(&current, surface.scale_factor);
        let mut draws = Vec::new();
        // Construct each source with fallible allocation before expanding/sampling.
        match &cmd.kind {
            Kind::FillRect { rect, .. } | Kind::StrokeRect { rect, .. } => {
                let rect_path = vello::kurbo::Rect::new(
                    rect.x,
                    rect.y,
                    rect.x + rect.width,
                    rect.y + rect.height,
                );
                // Fixed five-element rectangle; no allocation inside the iterator.
                let path =
                    prep.input(rect_path.path_elements(antialias_stroke::DEVICE_PRECISION))?;
                let stroke = cmd.kind.stroke();
                let rule = if let Kind::FillRect { rule, .. } = &cmd.kind {
                    *rule
                } else {
                    FillRule::NonZero
                };
                let draw = prep.draw(path, rule, stroke, device, brush, 0)?;
                reserve(&mut draws, 1)?;
                draws.push(draw);
            }
            Kind::FillPath { path, .. } | Kind::StrokePath { path, .. } => {
                let path = prep.path(path)?;
                let rule = if let Kind::FillPath { rule, .. } = &cmd.kind {
                    *rule
                } else {
                    FillRule::NonZero
                };
                let draw = prep.draw(path, rule, cmd.kind.stroke(), device, brush, 0)?;
                reserve(&mut draws, 1)?;
                draws.push(draw);
            }
            Kind::Line { p1, p2, stroke, .. } => {
                let path = prep.input([
                    PathEl::MoveTo((p1.x(), p1.y()).into()),
                    PathEl::LineTo((p2.x(), p2.y()).into()),
                ])?;
                let draw = prep.draw(path, FillRule::NonZero, Some(stroke), device, brush, 0)?;
                reserve(&mut draws, 1)?;
                draws.push(draw);
            }
            Kind::Polyline { points, stroke, .. } => {
                let path = prep.input(points.iter().enumerate().map(|(i, p)| {
                    if i == 0 {
                        PathEl::MoveTo((p.x(), p.y()).into())
                    } else {
                        PathEl::LineTo((p.x(), p.y()).into())
                    }
                }))?;
                let draw = prep.draw(path, FillRule::NonZero, Some(stroke), device, brush, 0)?;
                reserve(&mut draws, 1)?;
                draws.push(draw);
            }
            Kind::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill_paint,
                stroke_paint,
                stroke,
                rule,
                ..
            } => {
                // Preserve the ellipse geometry with fixed preparation precision.
                let ellipse = Ellipse::new((*cx, *cy), (*rx, *ry), 0.0);
                if fill_paint.is_some() {
                    let path =
                        prep.input(ellipse.path_elements(antialias_stroke::DEVICE_PRECISION))?;
                    let draw = prep.draw(path, *rule, None, device, brush, 0)?;
                    reserve(&mut draws, 1)?;
                    draws.push(draw);
                }
                if stroke_paint.is_some() && stroke.width() > 0.0 {
                    let path =
                        prep.input(ellipse.path_elements(antialias_stroke::DEVICE_PRECISION))?;
                    let draw =
                        prep.draw(path, FillRule::NonZero, Some(stroke), device, brush, 1)?;
                    reserve(&mut draws, 1)?;
                    draws.push(draw);
                }
            }
            Kind::DrawGlyphRun {
                run, origin, paint, ..
            } => {
                let Some(font) = font_for(encoder, submission, &run.font, same_session) else {
                    return Ok(None);
                };
                for glyph in &run.glyphs {
                    prep.charge(Work {
                        segments: 1,
                        ..Work::default()
                    })?;
                    let config = AntialiasConfig {
                        operation: prep.remaining(),
                        ..prep.config
                    };
                    let (path, work) =
                        glyph_outline::bounded_outline(&font, run, glyph.id, config, index)?;
                    prep.charge(work)?;
                    let local = Affine::new([
                        1.0,
                        0.0,
                        0.0,
                        -1.0,
                        origin.x() + f64::from(glyph.x),
                        origin.y() + f64::from(glyph.y),
                    ]) * run.skew_degrees.map_or(Affine::IDENTITY, |angle| {
                        Affine::skew(f64::from(angle.to_radians().tan()), 0.0)
                    });
                    let stroke = match paint {
                        GlyphPaint::Fill(_) => None,
                        GlyphPaint::Stroke { stroke, .. } => Some(stroke),
                    };
                    let draw =
                        prep.draw(path, FillRule::NonZero, stroke, device * local, brush, 0)?;
                    reserve(&mut draws, 1)?;
                    draws.push(draw);
                }
            }
            _ => unreachable!("all operations with AA hints are prepared"),
        }
        reserve(&mut result, 1)?;
        result.push((index, draws));
    }
    Ok(Some(PreparedFrame {
        commands: result,
        cache: prep.next,
    }))
}
