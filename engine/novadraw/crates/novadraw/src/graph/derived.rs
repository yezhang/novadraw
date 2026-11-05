//! Recovery of unpublished derived geometry. Source components and topology are never cloned.
use super::*;
use crate::container::viewport::ViewportLayoutEffect;
use std::collections::HashMap;

#[derive(Clone)]
struct DerivedNode {
    bounds: Rectangle,
    insets: Insets,
    border: Option<BorderSnapshot>,
    visible: bool,
    measurement_generation: u64,
    freeform: Option<FreeformState>,
    layout_input_proofs: Vec<crate::figure::preparation::InputProof>,
    prepared: Option<crate::figure::preparation::PreparedFigure>,
    route: Option<PointList>,
    decoration: Option<crate::connection::PreparedDecorationGeometry>,
}
impl DerivedNode {
    fn capture(node: &FigureNode) -> Self {
        Self {
            bounds: node.bounds,
            insets: node.insets,
            border: node.border_snapshot.clone(),
            visible: node.is_visible,
            measurement_generation: node.layout.generation(),
            freeform: node.layout.freeform,
            layout_input_proofs: node.layout.input_proofs.clone(),
            prepared: node.prepared.clone(),
            route: node
                .connection()
                .map(|connection| connection.route_points().clone()),
            decoration: node
                .connection_decoration()
                .map(|decoration| decoration.decoration_geometry()),
        }
    }
    fn rebase(&mut self, before: &SourceNode, after: &FigureNode) {
        if before.bounds != after.bounds {
            self.bounds = after.bounds;
        }
        if before.insets != after.insets {
            self.insets = after.insets;
        }
        if before.border != after.border_snapshot {
            self.border.clone_from(&after.border_snapshot);
        }
        if before.visible != after.is_visible {
            self.visible = after.is_visible;
        }
        if before.measurement_generation != after.layout.generation() {
            self.measurement_generation = after.layout.generation();
            self.freeform = after.layout.freeform;
        }
        let route = after
            .connection()
            .map(|connection| connection.route_points());
        if before.route.as_ref() != route {
            self.route = route.cloned();
        }
        let decoration = after
            .connection_decoration()
            .map(|decoration| decoration.decoration_geometry());
        if before.decoration != decoration {
            self.decoration = decoration;
        }
    }
}

/// Activity comparison inputs have no authority to replace prepared assets or proofs.
struct SourceNode {
    bounds: Rectangle,
    insets: Insets,
    border: Option<BorderSnapshot>,
    visible: bool,
    measurement_generation: u64,
    route: Option<PointList>,
    decoration: Option<crate::connection::PreparedDecorationGeometry>,
}

impl SourceNode {
    fn capture(node: &FigureNode) -> Self {
        Self {
            bounds: node.bounds,
            insets: node.insets,
            border: node.border_snapshot.clone(),
            visible: node.is_visible,
            measurement_generation: node.layout.generation(),
            route: node
                .connection()
                .map(|connection| connection.route_points().clone()),
            decoration: node
                .connection_decoration()
                .map(|decoration| decoration.decoration_geometry()),
        }
    }
}

pub(crate) struct PreparationSourceCheckpoint {
    nodes: HashMap<FigureId, SourceNode>,
    effect_count: usize,
    viewports: HashMap<FigureId, ViewportLayoutEffect>,
}

pub(crate) struct DerivedCheckpoint {
    nodes: HashMap<FigureId, DerivedNode>,
    effects: NotificationQueue,
    viewports: HashMap<FigureId, ViewportLayoutEffect>,
    source_region: bool,
}

impl FigureTree {
    /// Only a preparation owner needs a rollback prefix across feedback rounds.
    pub(crate) fn begin_derived_preparation(&mut self) -> bool {
        if self
            .capability_ids(PREPARATION)
            .any(|id| self.blocks[id].preparation().is_some())
        {
            self.derived_checkpoint = Some(self.capture_derived_prefix());
        }
        self.derived_checkpoint.is_some()
    }

    fn capture_derived_prefix(&self) -> DerivedCheckpoint {
        DerivedCheckpoint {
            nodes: self
                .blocks
                .iter()
                .map(|(id, n)| (id, DerivedNode::capture(n)))
                .collect(),
            effects: self.notification_effects.clone(),
            source_region: false,
            viewports: self
                .derived_checkpoint
                .as_ref()
                .map_or_else(HashMap::new, |prefix| {
                    prefix
                        .viewports
                        .iter()
                        .map(|(id, effect)| (*id, effect.current()))
                        .collect()
                }),
        }
    }

    pub(crate) fn capture_preparation_sources(&mut self) -> Option<PreparationSourceCheckpoint> {
        let prefix = self.derived_checkpoint.as_mut()?;
        prefix.source_region = true;
        Some(PreparationSourceCheckpoint {
            nodes: self
                .blocks
                .iter()
                .map(|(id, node)| (id, SourceNode::capture(node)))
                .collect(),
            effect_count: self.notification_effects.len(),
            viewports: prefix
                .viewports
                .iter()
                .map(|(id, effect)| (*id, effect.current()))
                .collect(),
        })
    }

    pub(crate) fn record_preparation_source_bounds(&mut self, id: FigureId, bounds: Rectangle) {
        if self
            .derived_checkpoint
            .as_ref()
            .is_none_or(|prefix| prefix.source_region)
            && let Some(node) = self.blocks.get_mut(id)
        {
            node.source_bounds = bounds;
        }
        if let Some(prefix) = self.derived_checkpoint.as_mut()
            && prefix.source_region
            && let Some(node) = prefix.nodes.get_mut(&id)
        {
            node.bounds = bounds;
        }
    }

    /// Rebase only committed activity changes; keep the earlier derived prefix.
    pub(crate) fn rebase_preparation_sources(
        &mut self,
        before: Option<PreparationSourceCheckpoint>,
    ) {
        let Some(before) = before else { return };
        let prefix = self
            .derived_checkpoint
            .as_mut()
            .expect("active preparation");
        prefix.source_region = false;
        prefix.nodes.retain(|id, _| self.blocks.contains_key(*id));
        for (id, node) in self.blocks.iter() {
            match (prefix.nodes.get_mut(&id), before.nodes.get(&id)) {
                (Some(saved), Some(old)) => saved.rebase(old, node),
                _ => {
                    prefix.nodes.insert(id, DerivedNode::capture(node));
                }
            }
        }
        prefix.effects.extend(
            self.notification_effects.effects()[before.effect_count..]
                .iter()
                .cloned(),
        );
        for (id, saved) in &mut prefix.viewports {
            if let Some(old) = before.viewports.get(id) {
                saved.rebase(old, &saved.current());
            }
        }
    }

    pub(crate) fn record_derived_viewport(&mut self, effect: &ViewportLayoutEffect) {
        if let Some(prefix) = self.derived_checkpoint.as_mut() {
            prefix
                .viewports
                .entry(effect.viewport())
                .or_insert_with(|| effect.current());
        }
    }

    pub(crate) fn finish_derived_preparation(&mut self, success: bool) -> Result<(), LayoutError> {
        let Some(prefix) = self.derived_checkpoint.take() else {
            return Ok(());
        };
        if success {
            return Ok(());
        }
        for (id, mut saved) in prefix.nodes {
            saved.prepared = saved
                .prepared
                .filter(|p| p.owner == id || self.is_ancestor_of(p.owner, id) == Ok(true));
            if let Some(node) = self.blocks.get_mut(id) {
                node.bounds = saved.bounds;
                node.insets = saved.insets;
                node.border_snapshot = saved.border;
                node.is_visible = saved.visible;
                // A detached/moved source must not regain another owner's presentation.
                node.prepared = saved.prepared;
                if let Some(points) = saved.route
                    && let Some(connection) = node.connection_mut()
                {
                    connection.commit_route_points(points);
                }
                if let Some(geometry) = saved.decoration
                    && let Some(decoration) = node.connection_decoration_mut()
                {
                    decoration.commit_decoration_geometry(geometry);
                }
                // Discard caches computed for rejected geometry, while restoring the logical
                // measurement version of the rebased source checkpoint.
                node.is_valid = false;
                node.layout.invalidate(LayoutInvalidation::Geometry);
                node.layout.cache.get_mut().generation = saved.measurement_generation;
                node.layout.freeform = saved.freeform;
                node.layout.input_proofs = saved.layout_input_proofs;
            }
        }
        for (_, effect) in prefix.viewports {
            if self.blocks.contains_key(effect.viewport()) {
                effect.restore()?;
            }
        }
        self.notification_effects = prefix.effects;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::preparation::{
        FigureDrawing, FigurePresentation, PreparationKey, PreparationSource, PreparedFigure,
        PreparedSceneIdentity,
    };
    use crate::graphics::{GraphicsError, PaintContext};
    use std::sync::Arc;

    struct EmptyDrawing;
    impl FigureDrawing for EmptyDrawing {
        fn paint(&self, _: &mut PaintContext<'_>) -> Result<(), GraphicsError> {
            Ok(())
        }
    }

    fn install_asset(tree: &mut FigureTree, id: FigureId, fragments: usize) {
        let bounds = tree.blocks[id].client_area();
        let presentation = FigurePresentation::new(
            FigureMeasurement::new(bounds.width, bounds.height, None),
            Dimension::ZERO,
            bounds,
            Arc::new(EmptyDrawing),
        )
        .unwrap()
        .with_hit_regions(vec![bounds; fragments])
        .unwrap();
        tree.blocks[id].prepared = Some(PreparedFigure {
            owner: id,
            valid: true,
            key: PreparationKey {
                component: 0,
                provider: 0,
                text: 0,
                font: crate::text::FontDescriptor::default(),
                bounds,
            },
            presentation,
            identity: PreparedSceneIdentity::new(id, 1),
            source: PreparationSource::default(),
            result_version: None,
            observations: Arc::from([]),
            input_proofs: Arc::from([]),
            required_routes: vec![],
        });
    }

    #[test]
    fn source_handoff_retains_original_assets_and_only_committed_geometry() {
        let mut tree = FigureTree::new();
        let parent = tree.add_child_to(
            tree.root,
            Box::new(crate::RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap()),
        );
        let child = tree.add_child_to(
            parent,
            Box::new(crate::RectangleFigure::new(5.0, 6.0, 10.0, 10.0).unwrap()),
        );
        install_asset(&mut tree, child, 4);
        let identity = tree.blocks[child].prepared.as_ref().unwrap().identity;
        let proofs = tree.blocks[child]
            .prepared
            .as_ref()
            .unwrap()
            .input_proofs
            .clone();
        tree.derived_checkpoint = Some(tree.capture_derived_prefix());
        let retained = Arc::strong_count(&proofs);
        // A rejected derived candidate precedes an unrelated committed source move.
        tree.set_bounds(child, 5.0, 6.0, 40.0, 10.0);
        let before = tree.capture_preparation_sources();
        assert_eq!(Arc::strong_count(&proofs), retained);
        tree.set_bounds(parent, 30.0, 40.0, 100.0, 100.0);
        tree.rebase_preparation_sources(before);
        assert_eq!(Arc::strong_count(&proofs), retained);
        install_asset(&mut tree, child, 0);
        tree.blocks[child].prepared.as_mut().unwrap().identity =
            PreparedSceneIdentity::new(child, 2);
        tree.finish_derived_preparation(false).unwrap();
        assert_eq!(tree.blocks[parent].bounds.x, 30.0);
        assert_eq!(tree.blocks[parent].bounds.y, 40.0);
        assert_eq!(
            tree.blocks[child].bounds,
            Rectangle::new(5.0, 6.0, 10.0, 10.0)
        );
        assert_eq!(
            tree.blocks[child].prepared.as_ref().unwrap().identity,
            identity
        );
        assert!(Arc::ptr_eq(
            &tree.blocks[child].prepared.as_ref().unwrap().input_proofs,
            &proofs,
        ));
        let transform = tree.local_to_surface_transform(child).unwrap();
        assert_eq!(
            transform.transform_point(crate::Point::ZERO),
            crate::Point::new(35.0, 46.0),
        );
    }

    #[test]
    fn source_handoff_rebases_size_visibility_and_new_node_without_restoring_deleted_identity() {
        let mut tree = FigureTree::new();
        let parent = tree.add_child_to(
            tree.root,
            Box::new(crate::RectangleFigure::new(0.0, 0.0, 100.0, 100.0).unwrap()),
        );
        let deleted = tree.add_child_to(
            parent,
            Box::new(crate::RectangleFigure::new(0.0, 0.0, 10.0, 10.0).unwrap()),
        );
        tree.derived_checkpoint = Some(tree.capture_derived_prefix());
        let before = tree.capture_preparation_sources();
        tree.set_bounds(parent, 5.0, 6.0, 80.0, 90.0);
        tree.blocks[parent].is_visible = false;
        tree.mark_validation_path_invalid(parent);
        let generation = tree.blocks[parent].layout.generation();
        let mut updates = UpdateManager::new();
        let ids = tree.disposal_ids(deleted).unwrap();
        tree.extract_subtree(&ids, &mut updates).unwrap();
        let added = tree.add_child_to(
            parent,
            Box::new(crate::RectangleFigure::new(7.0, 8.0, 9.0, 10.0).unwrap()),
        );
        tree.rebase_preparation_sources(before);
        tree.set_bounds(added, 0.0, 0.0, 30.0, 30.0);
        tree.finish_derived_preparation(false).unwrap();
        assert!(!tree.blocks.contains_key(deleted));
        assert_eq!(
            tree.blocks[parent].bounds,
            Rectangle::new(5.0, 6.0, 80.0, 90.0)
        );
        assert!(!tree.blocks[parent].is_visible);
        // Insertion/removal may advance the source generation further.
        assert!(tree.blocks[parent].layout.generation() >= generation);
        assert_eq!(
            tree.blocks[added].bounds,
            Rectangle::new(7.0, 8.0, 9.0, 10.0)
        );
    }

    /// Isolates the activity handoff; it does not measure recording or GPU submission.
    #[test]
    #[ignore = "release cost probe; run explicitly with --ignored --nocapture"]
    fn projection_source_handoff_cost() {
        use std::hint::black_box;
        use std::time::Instant;
        const NODES: usize = 300;
        const SAMPLES: usize = 200;
        for fragments in [0, 1024] {
            let mut tree = FigureTree::new();
            let parent = tree.add_child_to(
                tree.root,
                Box::new(crate::RectangleFigure::new(0.0, 0.0, 200.0, 200.0).unwrap()),
            );
            for index in 0..NODES {
                let id = tree.add_child_to(
                    parent,
                    Box::new(crate::RectangleFigure::new(0.0, 0.0, 20.0, 20.0).unwrap()),
                );
                if index % 3 == 0 {
                    install_asset(&mut tree, id, fragments);
                }
            }
            tree.derived_checkpoint = Some(tree.capture_derived_prefix());
            let mut samples = Vec::with_capacity(SAMPLES);
            for index in 0..SAMPLES + 20 {
                let start = Instant::now();
                let before = tree.capture_preparation_sources();
                tree.set_bounds(parent, index as f64, 0.0, 200.0, 200.0);
                tree.rebase_preparation_sources(before);
                let elapsed = start.elapsed().as_nanos();
                tree.drain_notification_effects();
                black_box(&tree);
                if index >= 20 {
                    samples.push(elapsed);
                }
            }
            samples.sort_unstable();
            println!(
                "projection_source_handoff nodes={NODES} assets={} fragments={fragments} samples={SAMPLES} p50_ns={} p95_ns={} p99_ns={}",
                NODES / 3,
                samples[SAMPLES / 2],
                samples[SAMPLES * 95 / 100],
                samples[SAMPLES * 99 / 100],
            );
        }
    }
}
