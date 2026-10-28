use crate::{Affine2D, Point, Rectangle};

/// An order-preserving hierarchy. Bounds are unions, never sorted logical line intervals.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EnvelopeIndex {
    nodes: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
struct Node {
    bounds: Envelope,
    start: usize,
    end: usize,
    right: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Envelope {
    Empty,
    Known(Rectangle),
    Unknown,
}

impl Envelope {
    pub(crate) fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Empty, b) | (b, Self::Empty) => b,
            (Self::Known(a), Self::Known(b)) => {
                let bounds = a.union(b);
                if [
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                    bounds.x + bounds.width,
                    bounds.y + bounds.height,
                ]
                .iter()
                .all(|v| v.is_finite())
                {
                    Self::Known(bounds)
                } else {
                    Self::Unknown
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct QueryWork {
    pub nodes: usize,
    pub candidates: usize,
    pub glyph_checks: usize,
    pub visible: usize,
    pub fallback: usize,
}

pub(crate) fn transformed_bounds(bounds: Rectangle, transform: Affine2D) -> Option<Rectangle> {
    let corners = [
        Point::new(bounds.x, bounds.y),
        Point::new(bounds.x + bounds.width, bounds.y),
        Point::new(bounds.x, bounds.y + bounds.height),
        Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
    ]
    .map(|point| transform.transform_point(point));
    if corners
        .iter()
        .any(|point| !point.x().is_finite() || !point.y().is_finite())
    {
        return None;
    }
    let min_x = corners.iter().map(|p| p.x()).fold(f64::INFINITY, f64::min);
    let min_y = corners.iter().map(|p| p.y()).fold(f64::INFINITY, f64::min);
    let max_x = corners
        .iter()
        .map(|p| p.x())
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = corners
        .iter()
        .map(|p| p.y())
        .fold(f64::NEG_INFINITY, f64::max);
    let result = Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y);
    (result.width.is_finite() && result.height.is_finite()).then_some(result)
}

impl EnvelopeIndex {
    /// None denotes certified empty geometry. Unknown geometry never enters this index.
    pub(super) fn new(bounds: &[Option<Rectangle>]) -> Self {
        Self::from_envelopes(
            &bounds
                .iter()
                .map(|b| b.map_or(Envelope::Empty, Envelope::Known))
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn from_envelopes(bounds: &[Envelope]) -> Self {
        fn build(nodes: &mut Vec<Node>, bounds: &[Envelope], start: usize) -> usize {
            let index = nodes.len();
            nodes.push(Node {
                bounds: Envelope::Empty,
                start,
                end: start + bounds.len(),
                right: 0,
            });
            let envelope = if bounds.len() == 1 {
                bounds[0]
            } else {
                let middle = bounds.len() / 2;
                let left = build(nodes, &bounds[..middle], start);
                let right = build(nodes, &bounds[middle..], start + middle);
                nodes[index].right = right;
                nodes[left].bounds.union(nodes[right].bounds)
            };
            nodes[index].bounds = envelope;
            index
        }
        let mut nodes = Vec::new();
        if !bounds.is_empty() {
            build(&mut nodes, bounds, 0);
        }
        Self { nodes }
    }

    pub(crate) fn visit(&self, clip: Rectangle, mut emit: impl FnMut(usize)) -> QueryWork {
        fn visit(
            nodes: &[Node],
            index: usize,
            clip: Rectangle,
            work: &mut QueryWork,
            emit: &mut impl FnMut(usize),
        ) {
            let node = &nodes[index];
            work.nodes += 1;
            let bounds = match node.bounds {
                Envelope::Empty => return,
                Envelope::Known(bounds) => Some(bounds),
                Envelope::Unknown => None,
            };
            // Inclusive comparisons preserve geometry touching a coverage boundary.
            if bounds.is_some_and(|bounds| {
                bounds.x > clip.x + clip.width
                    || clip.x > bounds.x + bounds.width
                    || bounds.y > clip.y + clip.height
                    || clip.y > bounds.y + bounds.height
            }) {
                return;
            }
            if node.end - node.start == 1 {
                work.candidates += 1;
                work.glyph_checks += 1;
                work.visible += 1;
                emit(node.start);
            } else {
                visit(nodes, index + 1, clip, work, emit);
                visit(nodes, node.right, clip, work, emit);
            }
        }
        let mut work = QueryWork::default();
        if !self.nodes.is_empty() {
            visit(&self.nodes, 0, clip, &mut work, &mut emit);
        }
        work
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_and_remote_ink_keep_source_order() {
        let bounds = [
            Some(Rectangle::new(-100.0, -100.0, 110.0, 120.0)),
            Some(Rectangle::new(0.0, 1000.0, 10.0, 20.0)),
            Some(Rectangle::new(-10.0, 5.0, 30.0, 2000.0)),
            None,
        ];
        let mut hits = Vec::new();
        let work = EnvelopeIndex::new(&bounds)
            .visit(Rectangle::new(0.0, 10.0, 10.0, 10.0), |index| {
                hits.push(index)
            });
        assert_eq!(hits, vec![0, 2]);
        assert_eq!(
            (work.candidates, work.glyph_checks, work.visible),
            (2, 2, 2)
        );
    }

    #[test]
    fn disjoint_subtrees_are_pruned_but_full_overlap_is_linear() {
        let separated: Vec<_> = (0..1024)
            .map(|i| Some(Rectangle::new(0.0, f64::from(i) * 100.0, 10.0, 10.0)))
            .collect();
        let clip = Rectangle::new(0.0, 0.0, 10.0, 10.0);
        let work = EnvelopeIndex::new(&separated).visit(clip, |_| {});
        assert_eq!(work.visible, 1);
        assert!(work.nodes < 30);
        let overlapping = vec![Some(clip); 1024];
        let work = EnvelopeIndex::new(&overlapping).visit(clip, |_| {});
        assert_eq!(work.visible, 1024);
        assert_eq!(work.nodes, 2047);
    }

    #[test]
    fn unknown_leaf_survives_and_does_not_disable_known_sibling_pruning() {
        let bounds = [
            Envelope::Unknown,
            Envelope::Empty,
            Envelope::Known(Rectangle::new(0.0, 1000.0, 10.0, 10.0)),
        ];
        let mut hits = Vec::new();
        EnvelopeIndex::from_envelopes(&bounds).visit(Rectangle::ZERO, |i| hits.push(i));
        assert_eq!(hits, vec![0]);
    }
}
