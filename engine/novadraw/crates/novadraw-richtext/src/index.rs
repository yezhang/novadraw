use novadraw::text::TextError;
use std::{ops::Range, sync::Arc};

/// Persistent balanced height summary. Queries and single-height updates visit
/// only tree paths; old snapshots remain valid.
#[derive(Clone, Default)]
pub struct ParagraphHeightIndex(Option<Arc<Node>>);
struct Node {
    count: usize,
    height: f64,
    children: Option<(Arc<Node>, Arc<Node>)>,
}
impl ParagraphHeightIndex {
    pub fn new(heights: &[f64]) -> Result<Self, TextError> {
        if heights.iter().any(|h| !h.is_finite() || *h < 0.0) {
            return Err(TextError::InvalidMetric);
        }
        fn build(heights: &[f64]) -> Result<Arc<Node>, TextError> {
            let (height, children) = if heights.len() == 1 {
                (heights[0], None)
            } else {
                let middle = heights.len() / 2;
                let left = build(&heights[..middle])?;
                let right = build(&heights[middle..])?;
                (left.height + right.height, Some((left, right)))
            };
            if !height.is_finite() {
                return Err(TextError::InvalidMetric);
            }
            Ok(Arc::new(Node {
                count: heights.len(),
                height,
                children,
            }))
        }
        Ok(Self(if heights.is_empty() {
            None
        } else {
            Some(build(heights)?)
        }))
    }
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |n| n.count)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
    pub fn total_height(&self) -> f64 {
        self.0.as_ref().map_or(0.0, |n| n.height)
    }
    pub fn offset(&self, index: usize) -> Option<f64> {
        if index > self.len() {
            return None;
        }
        fn sum(node: &Node, index: usize) -> f64 {
            if index == 0 {
                return 0.0;
            }
            if index == node.count {
                return node.height;
            }
            let (left, right) = node.children.as_ref().expect("interior index is a branch");
            if index <= left.count {
                sum(left, index)
            } else {
                left.height + sum(right, index - left.count)
            }
        }
        Some(self.0.as_ref().map_or(0.0, |n| sum(n, index)))
    }
    /// Logical paragraph range, for scrolling and hit queries. This is not an
    /// ink coverage proof and must not be used alone to discard drawing.
    pub fn intersecting(&self, range: Range<f64>) -> Range<usize> {
        if !range.start.is_finite() || !range.end.is_finite() || range.start >= range.end {
            return 0..0;
        }
        fn boundary(node: &Node, y: f64, include_end: bool) -> usize {
            if y < 0.0 || (include_end && y == 0.0) {
                return 0;
            }
            if y >= node.height {
                return node.count;
            }
            let Some((left, right)) = &node.children else {
                return if include_end { 1 } else { 0 };
            };
            if y < left.height || (include_end && y == left.height) {
                boundary(left, y, include_end)
            } else {
                left.count + boundary(right, y - left.height, include_end)
            }
        }
        self.0.as_ref().map_or(0..0, |n| {
            boundary(n, range.start, false)..boundary(n, range.end, true)
        })
    }
    pub fn with_height(&self, index: usize, height: f64) -> Result<Self, TextError> {
        if index >= self.len() || !height.is_finite() || height < 0.0 {
            return Err(TextError::InvalidMetric);
        }
        fn update(node: &Arc<Node>, index: usize, height: f64) -> Result<Arc<Node>, TextError> {
            let Some((left, right)) = &node.children else {
                return Ok(Arc::new(Node {
                    count: 1,
                    height,
                    children: None,
                }));
            };
            let (left, right) = if index < left.count {
                (update(left, index, height)?, right.clone())
            } else {
                (left.clone(), update(right, index - left.count, height)?)
            };
            let height = left.height + right.height;
            if !height.is_finite() {
                return Err(TextError::InvalidMetric);
            }
            Ok(Arc::new(Node {
                count: node.count,
                height,
                children: Some((left, right)),
            }))
        }
        Ok(Self(Some(update(
            self.0.as_ref().expect("checked index"),
            index,
            height,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_queries_and_persistent_updates_match_linear_reference() {
        let heights: Vec<_> = (1..=79).map(f64::from).collect();
        let index = ParagraphHeightIndex::new(&heights).unwrap();
        let changed = index.with_height(31, 123.0).unwrap();
        assert_eq!(index.total_height(), heights.iter().sum::<f64>());
        assert_eq!(
            changed.total_height(),
            index.total_height() - heights[31] + 123.0
        );
        for start in (-1..=index.total_height() as i32).step_by(17) {
            let range = f64::from(start)..f64::from(start + 41);
            let mut y = 0.0;
            let expected: Vec<_> = heights
                .iter()
                .enumerate()
                .filter_map(|(i, h)| {
                    let hit = y < range.end && y + h > range.start;
                    y += h;
                    hit.then_some(i)
                })
                .collect();
            let actual: Vec<_> = index.intersecting(range).collect();
            assert_eq!(actual, expected);
        }
        assert_eq!(index.offset(79), Some(index.total_height()));
        assert!(index.with_height(0, f64::INFINITY).is_err());
    }
}
