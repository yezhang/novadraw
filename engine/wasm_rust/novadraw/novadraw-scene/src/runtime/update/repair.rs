//! Damage repair 语义实现。
//!
//! # 坐标模型契约
//!
//! `bounds` 处于 parent content domain。`repaint()` 产生的 dirty rect
//! 处于对应 Figure 的 node-local domain。
//!
//! 因此 damage 传播必须遵循 Draw2D 的 repairDamage 协议：
//!
//! - 先与当前 Figure 的 local border box 求交；
//! - 沿父链组合 node placement 与 parent child transform；
//! - 每到一层 parent local domain，与 parent client area 求交；
//! - 最终得到 logical surface domain 中的保守 AABB。

use std::collections::HashMap;

use novadraw_geometry::{Affine2D, Rectangle, Translatable};
use novadraw_render::NdCanvas;

use crate::graph::{BlockId, FigureGraph};

const DAMAGE_REGION_MERGE_AREA_THRESHOLD: f64 = 9.0;
const DAMAGE_REGION_MAX_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DamagePropagationStep {
    pub transform: Affine2D,
    pub clip: Option<Rectangle>,
}

pub(crate) fn merge_dirty_region(
    dirty_regions: &mut HashMap<BlockId, Rectangle>,
    block_id: BlockId,
    rect: Rectangle,
) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }

    if let Some(existing) = dirty_regions.get_mut(&block_id) {
        *existing = union_rectangles(*existing, rect);
    } else {
        dirty_regions.insert(block_id, rect);
    }

    true
}

pub(crate) fn compute_damage_union<'a>(
    rects: impl IntoIterator<Item = &'a Rectangle>,
) -> Rectangle {
    let mut rects = rects.into_iter();
    let Some(first) = rects.next() else {
        return Rectangle::new(0.0, 0.0, 0.0, 0.0);
    };

    rects.fold(*first, |acc, rect| union_rectangles(acc, *rect))
}

pub(crate) fn propagate_damage_through_parent_chain(
    mut contribution: Rectangle,
    steps: &[DamagePropagationStep],
) -> Option<Rectangle> {
    for step in steps {
        contribution.transform(step.transform);
        if let Some(clip) = step.clip {
            contribution = contribution.intersection(clip)?;
        }
    }

    Some(contribution)
}

pub(crate) fn propagate_damage_to_root(
    graph: &FigureGraph,
    block_id: BlockId,
    contribution: Rectangle,
) -> Option<Rectangle> {
    let steps = collect_parent_chain_steps(graph, block_id)?;
    propagate_damage_through_parent_chain(contribution, &steps)
}

pub(crate) fn write_damage_set(canvas: &mut NdCanvas, rects: Vec<Rectangle>) -> Option<Rectangle> {
    let rects = normalize_damage_regions(rects);
    if rects.is_empty() {
        canvas.damage_mut().clear();
        return None;
    }

    canvas.damage_mut().set_regions(rects);
    canvas.damage().union()
}

pub(crate) fn prepare_damage_set<'a>(
    graph: &FigureGraph,
    canvas: &mut NdCanvas,
    dirty_regions: impl IntoIterator<Item = (&'a BlockId, &'a Rectangle)>,
) -> Option<Rectangle> {
    let propagated_regions: Vec<Rectangle> = dirty_regions
        .into_iter()
        .filter_map(|(block_id, rect)| propagate_damage_to_root(graph, *block_id, *rect))
        .collect();
    write_damage_set(canvas, propagated_regions)
}

fn collect_parent_chain_steps(
    graph: &FigureGraph,
    block_id: BlockId,
) -> Option<Vec<DamagePropagationStep>> {
    let mut steps = Vec::new();
    let current = graph.get_block(block_id)?;
    let contents_id = graph.get_contents();
    steps.push(DamagePropagationStep {
        transform: Affine2D::IDENTITY,
        clip: Some(current.figure.visual_bounds()),
    });
    if Some(block_id) == contents_id {
        let bounds = current.figure_bounds();
        steps.push(DamagePropagationStep {
            transform: Affine2D::from_translation(bounds.x, bounds.y),
            clip: None,
        });
        return Some(steps);
    }
    let mut walker_id = block_id;

    loop {
        let walker = graph.get_block(walker_id)?;
        let bounds = walker.figure_bounds();
        let Some(parent_id) = walker.parent else {
            steps.push(DamagePropagationStep {
                transform: Affine2D::from_translation(bounds.x, bounds.y),
                clip: None,
            });
            break;
        };
        let parent = graph.get_block(parent_id)?;
        steps.push(DamagePropagationStep {
            transform: parent.figure.child_transform().affine()
                * Affine2D::from_translation(bounds.x, bounds.y),
            clip: Some(parent.figure.client_area()),
        });
        if Some(parent_id) == contents_id {
            let parent_bounds = parent.figure_bounds();
            steps.push(DamagePropagationStep {
                transform: Affine2D::from_translation(parent_bounds.x, parent_bounds.y),
                clip: None,
            });
            break;
        }
        walker_id = parent_id;
    }

    Some(steps)
}

fn normalize_damage_regions(mut rects: Vec<Rectangle>) -> Vec<Rectangle> {
    rects.retain(|rect| rect.width > 0.0 && rect.height > 0.0);
    rects.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| {
                a.width
                    .partial_cmp(&b.width)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.height
                    .partial_cmp(&b.height)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut normalized: Vec<Rectangle> = Vec::new();
    'outer: for rect in rects {
        for existing in &mut normalized {
            if can_merge_regions(*existing, rect) {
                *existing = existing.union(rect);
                continue 'outer;
            }
        }
        normalized.push(rect);
    }

    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < normalized.len() {
            let mut j = i + 1;
            while j < normalized.len() {
                if can_merge_regions(normalized[i], normalized[j]) {
                    let merged = normalized[i].union(normalized[j]);
                    normalized[i] = merged;
                    normalized.remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    normalized = merge_small_regions(normalized);
    if normalized.len() > DAMAGE_REGION_MAX_COUNT {
        return vec![compute_damage_union(normalized.iter())];
    }

    normalized
}

fn can_merge_regions(a: Rectangle, b: Rectangle) -> bool {
    a.intersects(b)
        || a.intersection(b).is_some()
        || horizontally_touching_with_vertical_overlap(a, b)
        || vertically_touching_with_horizontal_overlap(a, b)
}

fn merge_small_regions(mut regions: Vec<Rectangle>) -> Vec<Rectangle> {
    let mut index = 0;
    while index < regions.len() {
        if regions[index].width * regions[index].height > DAMAGE_REGION_MERGE_AREA_THRESHOLD {
            index += 1;
            continue;
        }

        let mut best_idx = None;
        let mut best_union_area = f64::MAX;
        for candidate_idx in 0..regions.len() {
            if candidate_idx == index {
                continue;
            }
            let union = regions[index].union(regions[candidate_idx]);
            let union_area = union.width * union.height;
            if union_area < best_union_area {
                best_union_area = union_area;
                best_idx = Some(candidate_idx);
            }
        }

        if let Some(candidate_idx) = best_idx {
            let merged = regions[index].union(regions[candidate_idx]);
            regions[index] = merged;
            regions.remove(candidate_idx);
            if candidate_idx < index {
                index = index.saturating_sub(1);
            }
        } else {
            index += 1;
        }
    }

    regions
}

fn horizontally_touching_with_vertical_overlap(a: Rectangle, b: Rectangle) -> bool {
    let a_right = a.x + a.width;
    let b_right = b.x + b.width;
    let touch = a_right == b.x || b_right == a.x;
    let overlap = a.y < b.y + b.height && a.y + a.height > b.y;
    touch && overlap
}

fn vertically_touching_with_horizontal_overlap(a: Rectangle, b: Rectangle) -> bool {
    let a_bottom = a.y + a.height;
    let b_bottom = b.y + b.height;
    let touch = a_bottom == b.y || b_bottom == a.y;
    let overlap = a.x < b.x + b.width && a.x + a.width > b.x;
    touch && overlap
}

fn union_rectangles(a: Rectangle, b: Rectangle) -> Rectangle {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = (a.x + a.width).max(b.x + b.width);
    let max_y = (a.y + a.height).max(b.y + b.height);
    Rectangle::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RectangleFigure;
    use novadraw_core::Color;
    use slotmap::KeyData;

    fn create_test_key(data: u64) -> BlockId {
        BlockId::from(KeyData::from_ffi(data))
    }

    #[test]
    fn test_merge_dirty_region_rejects_invalid_rectangles() {
        let mut dirty_regions = HashMap::new();

        let merged = merge_dirty_region(
            &mut dirty_regions,
            create_test_key(1),
            Rectangle::new(0.0, 0.0, 0.0, 10.0),
        );

        assert!(!merged);
        assert!(dirty_regions.is_empty());
    }

    #[test]
    fn test_compute_damage_union_merges_multiple_rectangles() {
        let rects = vec![
            Rectangle::new(10.0, 20.0, 30.0, 40.0),
            Rectangle::new(25.0, 5.0, 10.0, 15.0),
            Rectangle::new(-5.0, 18.0, 8.0, 10.0),
        ];

        let union = compute_damage_union(rects.iter());

        assert_eq!(union, Rectangle::new(-5.0, 5.0, 45.0, 55.0));
    }

    #[test]
    fn test_write_damage_set_clears_canvas_for_empty_damage() {
        let mut canvas = NdCanvas::new();
        canvas
            .damage_mut()
            .set_union(Rectangle::new(1.0, 2.0, 3.0, 4.0));

        let union = write_damage_set(&mut canvas, Vec::new());

        assert_eq!(union, None);
        assert!(canvas.damage().is_empty());
    }

    #[test]
    fn test_write_damage_set_preserves_regions_and_union() {
        let mut canvas = NdCanvas::new();
        let regions = vec![
            Rectangle::new(10.0, 20.0, 5.0, 5.0),
            Rectangle::new(40.0, 50.0, 10.0, 8.0),
        ];

        let union = write_damage_set(&mut canvas, regions.clone());

        assert_eq!(union, Some(Rectangle::new(10.0, 20.0, 40.0, 38.0)));
        assert_eq!(canvas.damage().regions(), regions);
        assert_eq!(canvas.damage().union(), union);
    }

    #[test]
    fn test_normalize_damage_regions_merges_overlapping_and_touching_rects() {
        let normalized = normalize_damage_regions(vec![
            Rectangle::new(10.0, 10.0, 10.0, 10.0),
            Rectangle::new(18.0, 12.0, 8.0, 8.0),
            Rectangle::new(26.0, 12.0, 4.0, 8.0),
            Rectangle::new(100.0, 100.0, 5.0, 5.0),
        ]);

        assert_eq!(
            normalized,
            vec![
                Rectangle::new(10.0, 10.0, 20.0, 10.0),
                Rectangle::new(100.0, 100.0, 5.0, 5.0),
            ]
        );
    }

    #[test]
    fn test_normalize_damage_regions_keeps_separated_rects() {
        let normalized = normalize_damage_regions(vec![
            Rectangle::new(0.0, 0.0, 12.0, 12.0),
            Rectangle::new(24.0, 0.0, 12.0, 12.0),
        ]);

        assert_eq!(
            normalized,
            vec![
                Rectangle::new(0.0, 0.0, 12.0, 12.0),
                Rectangle::new(24.0, 0.0, 12.0, 12.0),
            ]
        );
    }

    #[test]
    fn test_normalize_damage_regions_merges_small_fragments_into_nearest_neighbor() {
        let normalized = normalize_damage_regions(vec![
            Rectangle::new(0.0, 0.0, 20.0, 20.0),
            Rectangle::new(30.0, 0.0, 20.0, 20.0),
            Rectangle::new(21.0, 8.0, 2.0, 2.0),
        ]);

        assert_eq!(
            normalized,
            vec![
                Rectangle::new(0.0, 0.0, 23.0, 20.0),
                Rectangle::new(30.0, 0.0, 20.0, 20.0),
            ]
        );
    }

    #[test]
    fn test_normalize_damage_regions_falls_back_to_union_when_region_count_exceeds_limit() {
        let rects: Vec<Rectangle> = (0..10)
            .map(|index| Rectangle::new(index as f64 * 20.0, 0.0, 10.0, 10.0))
            .collect();

        let normalized = normalize_damage_regions(rects);

        assert_eq!(normalized, vec![Rectangle::new(0.0, 0.0, 190.0, 10.0)]);
    }

    /// 当 dirty rect 与当前 Figure 的 bounds 同域时，
    /// 传播遇到坐标根父节点需要应用 offset 平移。
    #[test]
    fn test_parent_chain_single_level_propagation() {
        let local_damage = Rectangle::new(10.0, 20.0, 30.0, 40.0);
        let steps = [DamagePropagationStep {
            transform: Affine2D::from_translation(100.0, 50.0),
            clip: None,
        }];
        let expected_root_damage = Rectangle::new(110.0, 70.0, 30.0, 40.0);

        let actual = propagate_damage_through_parent_chain(local_damage, &steps);

        assert_eq!(actual, Some(expected_root_damage));
    }

    /// 多层坐标根时应逐层累加 offset。
    #[test]
    fn test_parent_chain_multi_level_propagation() {
        let local_damage = Rectangle::new(5.0, 6.0, 20.0, 10.0);
        let steps = [
            DamagePropagationStep {
                transform: Affine2D::from_translation(30.0, 40.0),
                clip: None,
            },
            DamagePropagationStep {
                transform: Affine2D::from_translation(100.0, 200.0),
                clip: None,
            },
        ];
        let expected_root_damage = Rectangle::new(135.0, 246.0, 20.0, 10.0);

        let actual = propagate_damage_through_parent_chain(local_damage, &steps);

        assert_eq!(actual, Some(expected_root_damage));
    }

    /// offset 与 clip 需要按父链协议组合。
    #[test]
    fn test_parent_chain_propagation_intersects_clip_regions() {
        let local_damage = Rectangle::new(10.0, 10.0, 80.0, 60.0);
        let steps = [
            DamagePropagationStep {
                transform: Affine2D::from_translation(20.0, 30.0),
                clip: Some(Rectangle::new(40.0, 50.0, 30.0, 20.0)),
            },
            DamagePropagationStep {
                transform: Affine2D::from_translation(100.0, 0.0),
                clip: Some(Rectangle::new(150.0, 40.0, 20.0, 20.0)),
            },
        ];
        let expected_root_damage = Some(Rectangle::new(150.0, 50.0, 20.0, 10.0));

        let actual = propagate_damage_through_parent_chain(local_damage, &steps);

        assert_eq!(actual, expected_root_damage);
    }

    #[test]
    fn test_propagate_damage_to_root_uses_figure_local_coordinate_roots() {
        let mut graph = FigureGraph::new();
        let root = RectangleFigure::new_with_color(0.0, 0.0, 500.0, 400.0, Color::BLACK);
        let root_id = graph.set_contents(Box::new(root));
        let parent = RectangleFigure::new_with_color(100.0, 50.0, 200.0, 150.0, Color::WHITE);
        let parent_id = graph.add_child_to(root_id, Box::new(parent));
        let child = RectangleFigure::new_with_color(20.0, 30.0, 80.0, 60.0, Color::WHITE);
        let child_id = graph.add_child_to(parent_id, Box::new(child));

        let actual =
            propagate_damage_to_root(&graph, child_id, Rectangle::new(0.0, 0.0, 20.0, 10.0));

        assert_eq!(actual, Some(Rectangle::new(120.0, 80.0, 20.0, 10.0)));
    }

    #[test]
    fn test_propagate_damage_to_root_translates_through_parent_local_ancestors() {
        let mut graph = FigureGraph::new();
        let root = RectangleFigure::new_with_color(0.0, 0.0, 500.0, 400.0, Color::BLACK);
        let root_id = graph.set_contents(Box::new(root));
        let parent = RectangleFigure::new_with_color(100.0, 50.0, 200.0, 150.0, Color::WHITE);
        let parent_id = graph.add_child_to(root_id, Box::new(parent));
        let child = RectangleFigure::new_with_color(120.0, 80.0, 80.0, 60.0, Color::WHITE);
        let child_id = graph.add_child_to(parent_id, Box::new(child));

        let actual =
            propagate_damage_to_root(&graph, child_id, Rectangle::new(0.0, 0.0, 20.0, 10.0));

        assert_eq!(actual, Some(Rectangle::new(220.0, 130.0, 20.0, 10.0)));
    }

    #[test]
    fn test_propagate_damage_to_root_applies_root_clip() {
        let mut graph = FigureGraph::new();
        let root = RectangleFigure::new_with_color(0.0, 0.0, 100.0, 80.0, Color::BLACK);
        let root_id = graph.set_contents(Box::new(root));
        let parent = RectangleFigure::new_with_color(70.0, 50.0, 40.0, 40.0, Color::WHITE);
        let parent_id = graph.add_child_to(root_id, Box::new(parent));
        let child = RectangleFigure::new_with_color(20.0, 20.0, 30.0, 30.0, Color::WHITE);
        let child_id = graph.add_child_to(parent_id, Box::new(child));

        let actual =
            propagate_damage_to_root(&graph, child_id, Rectangle::new(0.0, 0.0, 30.0, 30.0));

        assert_eq!(actual, Some(Rectangle::new(90.0, 70.0, 10.0, 10.0)));
    }
}
