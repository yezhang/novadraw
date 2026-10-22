//! Feasible ranks, a tight spanning tree, then deterministic network-simplex pivots.
//! Tree cuts are recomputed rather than exposing Draw2D's mutable tree bookkeeping.

use super::{GraphLayoutError, GraphSolveContext, GraphStage};

#[derive(Clone, Copy, Debug)]
pub(super) struct RankEdge {
    pub source: usize,
    pub target: usize,
    pub span: f64,
    pub weight: u32,
}

fn arithmetic(value: f64) -> Result<f64, GraphLayoutError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(GraphLayoutError::Overflow(GraphStage::Ranking))
    }
}

fn slack(edge: RankEdge, ranks: &[f64]) -> Result<f64, GraphLayoutError> {
    arithmetic(ranks[edge.target] - ranks[edge.source] - edge.span)
}

/// The input is weakly connected and acyclic; spans may be zero.
pub(super) fn solve(
    count: usize,
    edges: &[RankEdge],
    context: &mut GraphSolveContext,
) -> Result<Vec<f64>, GraphLayoutError> {
    context.allocate(GraphStage::Ranking, (count * 3 + edges.len()) as u64)?;
    let mut ranks = vec![0.0_f64; count];
    if count == 0 {
        return Ok(ranks);
    }
    let mut ranked = vec![false; count];
    for _ in 0..count {
        let mut progress = false;
        for node in 0..count {
            context.consume(GraphStage::Ranking, 1)?;
            if ranked[node] {
                continue;
            }
            let mut ready = true;
            let mut minimum = 0.0_f64;
            for edge in edges {
                context.consume(GraphStage::Ranking, 1)?;
                if edge.target == node {
                    ready &= ranked[edge.source];
                    minimum = minimum.max(arithmetic(ranks[edge.source] + edge.span)?);
                }
            }
            if ready {
                ranks[node] = minimum;
                ranked[node] = true;
                progress = true;
            }
        }
        if ranked.iter().all(|value| *value) {
            break;
        }
        if !progress {
            return Err(GraphLayoutError::CapabilityRejected("acyclic rank graph"));
        }
    }

    let mut tree = vec![false; edges.len()];
    let mut members = vec![false; count];
    members[0] = true;
    for _ in 1..count {
        let mut selected = None;
        let mut minimum = f64::INFINITY;
        for (index, edge) in edges.iter().copied().enumerate() {
            context.consume(GraphStage::Ranking, 1)?;
            if members[edge.source] != members[edge.target] {
                let value = slack(edge, &ranks)?;
                if value < minimum {
                    minimum = value;
                    selected = Some(index);
                }
            }
        }
        let index = selected.ok_or(GraphLayoutError::Disconnected)?;
        let edge = edges[index];
        let shift = if members[edge.source] {
            minimum
        } else {
            -minimum
        };
        shift_members(&mut ranks, &members, shift, context)?;
        members[edge.source] = true;
        members[edge.target] = true;
        tree[index] = true;
    }

    // Bland's deterministic index rule handles zero-slack (degenerate) pivots.
    // No iteration cap silently returns an unoptimized result: the shared budget bounds work.
    loop {
        let mut pivot = None;
        for (leave, edge) in edges.iter().copied().enumerate() {
            context.consume(GraphStage::Ranking, 1)?;
            if !tree[leave] {
                continue;
            }
            let branch = component(count, edges, &tree, leave, edge.target, context)?;
            let mut cut = 0_i128;
            for other in edges {
                context.consume(GraphStage::Ranking, 1)?;
                let contribution = match (branch[other.source], branch[other.target]) {
                    (false, true) => i128::from(other.weight),
                    (true, false) => -i128::from(other.weight),
                    _ => 0,
                };
                cut = cut
                    .checked_add(contribution)
                    .ok_or(GraphLayoutError::Overflow(GraphStage::Ranking))?;
            }
            // The target-side component may increase without relaxing the leaving edge.
            // Its outgoing constraints bound that increase. Negative cut improves the objective.
            if cut >= 0 {
                continue;
            }
            let mut enter = None;
            let mut minimum = f64::INFINITY;
            for (index, other) in edges.iter().copied().enumerate() {
                context.consume(GraphStage::Ranking, 1)?;
                if !tree[index] && branch[other.source] && !branch[other.target] {
                    let value = slack(other, &ranks)?;
                    if value < minimum {
                        minimum = value;
                        enter = Some(index);
                    }
                }
            }
            let enter = enter.ok_or(GraphLayoutError::CapabilityRejected(
                "bounded weighted rank objective",
            ))?;
            pivot = Some((leave, enter, minimum, branch));
            break;
        }
        let Some((leave, enter, shift, branch)) = pivot else {
            break;
        };
        shift_members(&mut ranks, &branch, shift, context)?;
        tree[leave] = false;
        tree[enter] = true;
    }
    let minimum = ranks.iter().copied().fold(f64::INFINITY, f64::min);
    for rank in &mut ranks {
        context.consume(GraphStage::Ranking, 1)?;
        *rank = arithmetic(*rank - minimum)?;
    }
    Ok(ranks)
}

fn shift_members(
    ranks: &mut [f64],
    members: &[bool],
    shift: f64,
    context: &mut GraphSolveContext,
) -> Result<(), GraphLayoutError> {
    for (rank, member) in ranks.iter_mut().zip(members) {
        context.consume(GraphStage::Ranking, 1)?;
        if *member {
            *rank = arithmetic(*rank + shift)?;
        }
    }
    Ok(())
}

fn component(
    count: usize,
    edges: &[RankEdge],
    tree: &[bool],
    omitted: usize,
    root: usize,
    context: &mut GraphSolveContext,
) -> Result<Vec<bool>, GraphLayoutError> {
    context.allocate(GraphStage::Ranking, (count * 2) as u64)?;
    let mut members = vec![false; count];
    let mut queue = Vec::with_capacity(count);
    members[root] = true;
    queue.push(root);
    let mut cursor = 0;
    while cursor < queue.len() {
        let node = queue[cursor];
        cursor += 1;
        for (index, edge) in edges.iter().enumerate() {
            context.consume(GraphStage::Ranking, 1)?;
            if !tree[index] || index == omitted {
                continue;
            }
            let neighbor = if edge.source == node {
                edge.target
            } else if edge.target == node {
                edge.source
            } else {
                continue;
            };
            if !members[neighbor] {
                members[neighbor] = true;
                queue.push(neighbor);
            }
        }
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::graph::GraphBudget;

    fn context() -> GraphSolveContext {
        GraphSolveContext::new(GraphBudget::new(10_000_000, 1_000_000))
    }

    fn objective(edges: &[RankEdge], ranks: &[f64]) -> f64 {
        edges
            .iter()
            .map(|edge| f64::from(edge.weight) * (ranks[edge.target] - ranks[edge.source]))
            .sum()
    }

    #[test]
    fn weighted_rank_matches_exhaustive_feasible_assignments() {
        // Connected four-node DAGs include zero span, zero weight, diamonds,
        // parallel edges, and tradeoffs where longest-path ranks are not optimal.
        for weights in [[0, 1, 9, 1, 2], [9, 1, 1, 3, 0], [1, 9, 3, 0, 1]] {
            for spans in [[1, 1, 1, 1, 1], [0, 2, 1, 0, 1], [2, 0, 1, 2, 0]] {
                let pairs = [(0, 1), (0, 2), (1, 3), (2, 3), (0, 3)];
                let edges: Vec<_> = pairs
                    .into_iter()
                    .zip(spans)
                    .zip(weights)
                    .map(|(((source, target), span), weight)| RankEdge {
                        source,
                        target,
                        span: f64::from(span),
                        weight,
                    })
                    .collect();
                let ranks = solve(4, &edges, &mut context()).unwrap();
                let mut best = f64::INFINITY;
                for a in 0..=6 {
                    for b in 0..=6 {
                        for c in 0..=6 {
                            for d in 0..=6 {
                                let candidate = [a, b, c, d].map(f64::from);
                                if edges
                                    .iter()
                                    .all(|edge| slack(*edge, &candidate).unwrap() >= 0.0)
                                {
                                    best = best.min(objective(&edges, &candidate));
                                }
                            }
                        }
                    }
                }
                assert_eq!(objective(&edges, &ranks), best, "{edges:?} {ranks:?}");
            }
        }
    }

    #[test]
    fn arbitrary_node_order_and_parallel_edges_preserve_feasibility() {
        let edges = [
            RankEdge {
                source: 2,
                target: 0,
                span: 2.0,
                weight: 0,
            },
            RankEdge {
                source: 0,
                target: 1,
                span: 0.0,
                weight: 4,
            },
            RankEdge {
                source: 2,
                target: 1,
                span: 1.0,
                weight: 9,
            },
            RankEdge {
                source: 2,
                target: 0,
                span: 3.0,
                weight: 2,
            },
        ];
        let ranks = solve(3, &edges, &mut context()).unwrap();
        assert!(
            edges
                .iter()
                .all(|edge| slack(*edge, &ranks).unwrap() >= 0.0)
        );
        assert_eq!(ranks, vec![3.0, 3.0, 0.0]);
    }

    #[test]
    fn ranking_uses_shared_budget_and_rejects_nonfinite_arithmetic() {
        let edges = [RankEdge {
            source: 0,
            target: 1,
            span: 1.0,
            weight: 1,
        }];
        let mut tiny = GraphSolveContext::new(GraphBudget::new(1, 100));
        assert!(matches!(
            solve(2, &edges, &mut tiny),
            Err(GraphLayoutError::BudgetExceeded {
                stage: GraphStage::Ranking,
                ..
            })
        ));
        let huge = [
            RankEdge {
                span: f64::MAX,
                ..edges[0]
            },
            RankEdge {
                source: 1,
                target: 2,
                span: f64::MAX,
                weight: 1,
            },
        ];
        assert!(matches!(
            solve(3, &huge, &mut context()),
            Err(GraphLayoutError::Overflow(GraphStage::Ranking))
        ));
    }
}
