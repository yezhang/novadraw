use std::collections::{HashMap, HashSet};

use crate::FigureId;
use crate::connection::{ConnectionId, CoordinateSpace, DependencySubject};
use crate::graph::FigureTree;
use crate::text::TextError;

use super::{PreparationDependencyError, scene_error};

/// Calculation edges point from a consumer to the output it reads.
/// Required route publication is deliberately absent from this graph.
#[derive(Default)]
pub(crate) struct PreparationDependencies {
    edges: HashMap<DependencySubject, Vec<DependencySubject>>,
}

impl PreparationDependencies {
    pub(crate) fn replace(
        &mut self,
        consumer: DependencySubject,
        suppliers: impl IntoIterator<Item = DependencySubject>,
        budget: &super::PreparationBudgetHandle,
    ) -> Result<(), TextError> {
        budget.allocate("dependency graph", 1)?;
        let mut collected = Vec::new();
        for supplier in suppliers {
            budget.allocate("dependency graph", 1)?;
            collected.push(supplier);
        }
        let suppliers = collected;
        for supplier in &suppliers {
            if matches!(
                supplier,
                DependencySubject::NamedAnchorRegion(..) | DependencySubject::RelativeTransform(..)
            ) {
                budget.allocate("dependency alias", 3)?;
                self.edges.entry(supplier.clone()).or_insert_with(|| {
                    output_figures(supplier)
                        .into_iter()
                        .flatten()
                        .map(DependencySubject::FigureGeometry)
                        .collect()
                });
            }
        }
        self.edges.insert(consumer, suppliers);
        Ok(())
    }

    pub(crate) fn retain_tree(
        &mut self,
        tree: &FigureTree,
        figures: &[FigureId],
        budget: &super::PreparationBudgetHandle,
    ) -> Result<(), TextError> {
        for &id in figures {
            budget.consume("retained preparation dependencies", 1)?;
            let node = tree.node(id).map_err(scene_error)?;
            if let Some(prepared) = &node.prepared
                && prepared.owner == id
                && prepared.valid
            {
                self.replace(
                    DependencySubject::PreparedScene(id),
                    prepared
                        .observations
                        .iter()
                        .map(|item| item.subject.clone()),
                    budget,
                )?;
            }
        }
        // Route geometry and all post-route child outputs depend on the route's reads.
        // Source topology and visibility never acquire derived-output edges.
        for &id in figures {
            let mut cursor = id;
            loop {
                budget.consume("route output ancestry", 1)?;
                let node = tree.node(cursor).map_err(scene_error)?;
                if node.connection().is_some() {
                    if cursor != id {
                        let route = DependencySubject::FigureGeometry(cursor);
                        for subject in [
                            DependencySubject::FigureGeometry(id),
                            DependencySubject::FigureMeasurement(id),
                            DependencySubject::FigureMeasurementSource(id),
                        ] {
                            self.replace(subject, [route.clone()], budget)?;
                        }
                    } else {
                        for subject in [
                            DependencySubject::FigureMeasurement(id),
                            DependencySubject::FigureMeasurementSource(id),
                        ] {
                            self.replace(subject, [DependencySubject::FigureGeometry(id)], budget)?;
                        }
                    }
                    break;
                }
                let Some(parent) = node.parent else {
                    break;
                };
                cursor = parent;
            }
        }
        Ok(())
    }

    pub(crate) fn validate(
        &self,
        tree: &FigureTree,
        owner: FigureId,
        required: impl IntoIterator<Item = ConnectionId>,
        budget: &super::PreparationBudgetHandle,
    ) -> Result<(), TextError> {
        let mut collected = Vec::new();
        for connection in required {
            budget.allocate("required dependency members", 1)?;
            collected.push(connection);
        }
        let required = collected;
        budget.allocate("dependency validation", 3)?;
        let root = DependencySubject::PreparedScene(owner);
        let mut path = vec![root.clone()];
        let mut active = HashSet::from([root.clone()]);
        let mut complete = HashSet::new();
        let mut pending = vec![(root, 0)];
        while let Some((subject, next)) = pending.last_mut() {
            budget.consume("dependency traversal", 1)?;
            let suppliers = self.edges.get(subject).map(Vec::as_slice).unwrap_or(&[]);
            let Some(supplier) = suppliers.get(*next) else {
                budget.allocate("completed dependency", 1)?;
                complete.insert(subject.clone());
                active.remove(subject);
                pending.pop();
                path.pop();
                continue;
            };
            *next += 1;
            for connection in &required {
                for figure in output_figures(supplier).into_iter().flatten() {
                    let mut cursor = Some(figure);
                    let mut reads_route = false;
                    while let Some(id) = cursor {
                        budget.consume("required route ancestry", 1)?;
                        if id == connection.figure() {
                            reads_route = true;
                            break;
                        }
                        cursor = tree.node(id).map_err(scene_error)?.parent;
                    }
                    if reads_route {
                        return Err(TextError::PreparationDependency(Box::new(
                            PreparationDependencyError::ForbiddenRouteOutputDependency {
                                owner,
                                connection: *connection,
                                subject: supplier.clone(),
                            },
                        )));
                    }
                }
            }
            if active.contains(supplier) {
                let start = path.iter().position(|item| item == supplier).unwrap();
                budget.allocate("dependency cycle path", (path.len() - start + 1) as u64)?;
                let mut cycle = path[start..].to_vec();
                cycle.push(supplier.clone());
                return Err(TextError::PreparationDependency(Box::new(
                    PreparationDependencyError::DependencyCycle { path: cycle },
                )));
            }
            if complete.contains(supplier) {
                continue;
            }
            if path.len() >= crate::graph::MAX_TREE_DEPTH {
                return Err(TextError::PreparationDependency(Box::new(
                    PreparationDependencyError::DepthExceeded {
                        limit: crate::graph::MAX_TREE_DEPTH,
                    },
                )));
            }
            budget.allocate("dependency traversal", 3)?;
            active.insert(supplier.clone());
            path.push(supplier.clone());
            pending.push((supplier.clone(), 0));
        }
        Ok(())
    }
}

fn output_figures(subject: &DependencySubject) -> [Option<FigureId>; 2] {
    match subject {
        DependencySubject::FigureGeometry(id)
        | DependencySubject::FigureMeasurement(id)
        | DependencySubject::FigureMeasurementSource(id)
        | DependencySubject::NamedAnchorRegion(id, _) => [Some(*id), None],
        DependencySubject::RelativeTransform(from, to) => [from, to].map(|space| match space {
            CoordinateSpace::FigureLocal(id) | CoordinateSpace::ChildContent(id) => Some(*id),
            CoordinateSpace::LogicalSurface => None,
        }),
        _ => [None, None],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{PreparationBudget, PreparationBudgetHandle};

    #[test]
    fn unfinished_reachability_reports_budget_not_a_proven_cycle() {
        let tree = FigureTree::new();
        let owner = tree.synthetic_root();
        let subject = DependencySubject::PreparedScene(owner);
        let mut graph = PreparationDependencies::default();
        graph
            .replace(
                subject.clone(),
                [subject.clone()],
                &PreparationBudgetHandle::default(),
            )
            .unwrap();
        let budget = PreparationBudgetHandle::new(PreparationBudget::new(3, 3));
        assert!(matches!(
            graph.validate(&tree, owner, [], &budget),
            Err(TextError::PreparationDependency(error))
                if matches!(error.as_ref(), PreparationDependencyError::Budget(_))
        ));
        assert!(matches!(
            graph.validate(&tree, owner, [], &PreparationBudgetHandle::default()),
            Err(TextError::PreparationDependency(error))
                if matches!(error.as_ref(), PreparationDependencyError::DependencyCycle { path }
                    if *path == vec![subject.clone(), subject.clone()])
        ));
    }
}
