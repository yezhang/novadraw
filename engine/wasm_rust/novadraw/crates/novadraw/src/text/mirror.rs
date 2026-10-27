//! Piecewise translations preserve glyph orientation while reversing line fragments.
use std::{ops::Range, sync::Arc};

use crate::{
    geometry::{Point, Rectangle},
    text::{
        CaretGeometry, SelectionQuad, TextAffinity, TextInteractionError, TextMovement,
        TextPosition, TextRange, shaping::TextInteractionProvider,
    },
};

#[derive(Clone)]
pub(crate) struct FragmentTranslation {
    pub source: Range<usize>,
    pub line: usize,
    pub bounds: Rectangle,
    pub dx: f64,
}

impl FragmentTranslation {
    fn visual_bounds(&self) -> Rectangle {
        Rectangle::new(
            self.bounds.x + self.dx,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        )
    }
    fn owns(&self, position: TextPosition) -> bool {
        let offset = position.byte_offset();
        (offset > self.source.start && offset < self.source.end)
            || (offset == self.source.start && position.affinity() == TextAffinity::Downstream)
            || (offset == self.source.end && position.affinity() == TextAffinity::Upstream)
    }
}

pub(crate) struct TranslatedInteraction {
    pub original: Arc<dyn TextInteractionProvider>,
    pub fragments: Vec<FragmentTranslation>,
}

impl TranslatedInteraction {
    fn fragment(&self, position: TextPosition, line: usize) -> Option<&FragmentTranslation> {
        self.fragments
            .iter()
            .find(|fragment| fragment.line == line && fragment.owns(position))
            .or_else(|| {
                self.fragments.iter().find(|fragment| {
                    fragment.line == line
                        && fragment.source.start <= position.byte_offset()
                        && position.byte_offset() <= fragment.source.end
                })
            })
    }
    fn nearest(&self, point: Point) -> Option<&FragmentTranslation> {
        self.fragments.iter().min_by(|a, b| {
            let distance = |fragment: &FragmentTranslation| {
                let bounds = fragment.visual_bounds();
                let dy = (bounds.y - point.y()).max(0.0)
                    + (point.y() - bounds.y - bounds.height).max(0.0);
                let dx = (bounds.x - point.x()).max(0.0)
                    + (point.x() - bounds.x - bounds.width).max(0.0);
                (dy, dx)
            };
            let (ay, ax) = distance(a);
            let (by, bx) = distance(b);
            ay.total_cmp(&by).then(ax.total_cmp(&bx)).then_with(|| {
                let a = a.visual_bounds();
                let b = b.visual_bounds();
                if point.x() > a.x + a.width && point.x() > b.x + b.width {
                    (b.x + b.width).total_cmp(&(a.x + a.width))
                } else {
                    a.x.total_cmp(&b.x)
                }
            })
        })
    }
}

impl TextInteractionProvider for TranslatedInteraction {
    fn text_len(&self) -> usize {
        self.original.text_len()
    }
    fn hit_test(&self, point: Point) -> Result<TextPosition, TextInteractionError> {
        let Some(fragment) = self.nearest(point) else {
            return self.original.hit_test(point);
        };
        let boundaries = [
            TextPosition::new(fragment.source.start, TextAffinity::Downstream),
            TextPosition::new(fragment.source.end, TextAffinity::Upstream),
        ];
        let boundary_x = [
            self.original.caret_geometry(boundaries[0])?.bounds().x + fragment.dx,
            self.original.caret_geometry(boundaries[1])?.bounds().x + fragment.dx,
        ];
        for (boundary, x) in boundaries.into_iter().zip(boundary_x) {
            let distance = (x - point.x()).abs();
            if distance <= f64::from(f32::EPSILON) * (point.x().abs() + 1.0) {
                return Ok(boundary);
            }
        }
        let visual = fragment.visual_bounds();
        if point.x() < visual.x || point.x() > visual.x + visual.width {
            let choose_start = if point.x() < visual.x {
                boundary_x[0] <= boundary_x[1]
            } else {
                boundary_x[0] >= boundary_x[1]
            };
            return Ok(boundaries[if choose_start { 0 } else { 1 }]);
        }
        let position = self
            .original
            .hit_test(Point::new(point.x() - fragment.dx, point.y()))?;
        let offset = position
            .byte_offset()
            .clamp(fragment.source.start, fragment.source.end);
        Ok(TextPosition::new(
            offset,
            if offset == fragment.source.start {
                TextAffinity::Downstream
            } else if offset == fragment.source.end {
                TextAffinity::Upstream
            } else {
                position.affinity()
            },
        ))
    }
    fn caret_geometry(
        &self,
        position: TextPosition,
    ) -> Result<CaretGeometry, TextInteractionError> {
        let caret = self.original.caret_geometry(position)?;
        let mut bounds = caret.bounds();
        bounds.x += self
            .fragment(position, caret.line_index())
            .map_or(0.0, |f| f.dx);
        Ok(CaretGeometry::new(bounds, caret.line_index()))
    }
    fn selection_geometry(
        &self,
        range: TextRange,
    ) -> Result<Vec<SelectionQuad>, TextInteractionError> {
        let mut output = Vec::new();
        for quad in self.original.selection_geometry(range)? {
            let bounds = quad.bounds();
            for fragment in self
                .fragments
                .iter()
                .filter(|f| f.line == quad.line_index())
            {
                let left = bounds.x.max(fragment.bounds.x);
                let right =
                    (bounds.x + bounds.width).min(fragment.bounds.x + fragment.bounds.width);
                if right > left {
                    output.push(SelectionQuad::new(
                        Rectangle::new(left + fragment.dx, bounds.y, right - left, bounds.height),
                        quad.line_index(),
                    ));
                }
            }
        }
        Ok(output)
    }
    fn move_position(
        &self,
        position: TextPosition,
        movement: TextMovement,
    ) -> Result<TextPosition, TextInteractionError> {
        let moved = self.original.move_position(position, movement)?;
        let current = self.caret_geometry(position)?;
        let next = self.caret_geometry(moved)?;
        let left = matches!(
            movement,
            TextMovement::PreviousVisual | TextMovement::PreviousWord
        );
        let right = matches!(movement, TextMovement::NextVisual | TextMovement::NextWord);
        if left || right {
            let x = current.bounds().x;
            let current_fragment = self.fragment(position, current.line_index());
            if next.line_index() == current.line_index()
                && current_fragment.is_some_and(|fragment| fragment.owns(moved))
                && ((left && next.bounds().x < x) || (right && next.bounds().x > x))
            {
                return Ok(moved);
            }
            // At a fragment boundary, the next visual fragment is not the next source run.
            let adjacent = self
                .fragments
                .iter()
                .filter(|fragment| {
                    let bounds = fragment.visual_bounds();
                    fragment.line == current.line_index()
                        && if left {
                            bounds.x + bounds.width <= x + f64::from(f32::EPSILON) * (x.abs() + 1.0)
                        } else {
                            bounds.x >= x - f64::from(f32::EPSILON) * (x.abs() + 1.0)
                        }
                        && !fragment.owns(position)
                })
                .min_by(|a, b| {
                    let distance = |fragment: &FragmentTranslation| {
                        let bounds = fragment.visual_bounds();
                        if left {
                            x - bounds.x - bounds.width
                        } else {
                            bounds.x - x
                        }
                    };
                    distance(a).total_cmp(&distance(b))
                })
                .or_else(|| {
                    let line = if left {
                        current.line_index().checked_sub(1)
                    } else {
                        current.line_index().checked_add(1)
                    }?;
                    self.fragments
                        .iter()
                        .filter(|fragment| fragment.line == line)
                        .min_by(|a, b| {
                            let edge = |fragment: &FragmentTranslation| {
                                let bounds = fragment.visual_bounds();
                                if left {
                                    -(bounds.x + bounds.width)
                                } else {
                                    bounds.x
                                }
                            };
                            edge(a).total_cmp(&edge(b))
                        })
                });
            if let Some(fragment) = adjacent {
                let start = TextPosition::new(fragment.source.start, TextAffinity::Downstream);
                let end = TextPosition::new(fragment.source.end, TextAffinity::Upstream);
                let start_x = self.original.caret_geometry(start)?.bounds().x;
                let end_x = self.original.caret_geometry(end)?.bounds().x;
                let boundary = if if left {
                    start_x > end_x
                } else {
                    start_x < end_x
                } {
                    start
                } else {
                    end
                };
                let advanced = self.original.move_position(boundary, movement)?;
                return Ok(if fragment.owns(advanced) {
                    advanced
                } else {
                    boundary
                });
            }
            return Ok(position);
        } else if matches!(
            movement,
            TextMovement::PreviousLine | TextMovement::NextLine
        ) {
            return self.hit_test(Point::new(
                current.bounds().x,
                next.bounds().y + next.bounds().height * 0.5,
            ));
        } else if matches!(movement, TextMovement::LineStart | TextMovement::LineEnd) {
            let x = if movement == TextMovement::LineStart {
                0.0
            } else {
                f64::from(f32::MAX)
            };
            return self.hit_test(Point::new(
                x,
                current.bounds().y + current.bounds().height * 0.5,
            ));
        }
        Ok(moved)
    }
}
