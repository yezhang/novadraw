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
            prepared: node.prepared.clone(),
            route: node
                .connection()
                .map(|connection| connection.route_points().clone()),
            decoration: node
                .connection_decoration()
                .map(|decoration| decoration.decoration_geometry()),
        }
    }
    fn rebase(&mut self, before: &Self, after: &Self) {
        if before.bounds != after.bounds {
            self.bounds = after.bounds;
        }
        if before.insets != after.insets {
            self.insets = after.insets;
        }
        if before.border != after.border {
            self.border.clone_from(&after.border);
        }
        if before.visible != after.visible {
            self.visible = after.visible;
        }
        if before.measurement_generation != after.measurement_generation {
            self.measurement_generation = after.measurement_generation;
            self.freeform = after.freeform;
        }
        if before.route != after.route {
            self.route.clone_from(&after.route);
        }
        if before.decoration != after.decoration {
            self.decoration.clone_from(&after.decoration);
        }
    }
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
            .blocks
            .iter()
            .any(|(_, node)| node.preparation().is_some())
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

    pub(crate) fn capture_preparation_sources(&mut self) -> Option<DerivedCheckpoint> {
        let before = self
            .derived_checkpoint
            .as_ref()
            .map(|_| self.capture_derived_prefix());
        if let Some(prefix) = self.derived_checkpoint.as_mut() {
            prefix.source_region = true;
        }
        before
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
    pub(crate) fn rebase_preparation_sources(&mut self, before: Option<DerivedCheckpoint>) {
        let Some(before) = before else { return };
        let after = self.capture_derived_prefix();
        let prefix = self
            .derived_checkpoint
            .as_mut()
            .expect("active preparation");
        prefix.source_region = false;
        prefix.nodes.retain(|id, _| after.nodes.contains_key(id));
        for (id, node) in after.nodes {
            match (prefix.nodes.get_mut(&id), before.nodes.get(&id)) {
                (Some(saved), Some(old)) => saved.rebase(old, &node),
                _ => {
                    prefix.nodes.insert(id, node);
                }
            }
        }
        prefix.effects.extend(
            after.effects.effects()[before.effects.len()..]
                .iter()
                .cloned(),
        );
        for (id, current) in after.viewports {
            if let (Some(saved), Some(old)) =
                (prefix.viewports.get_mut(&id), before.viewports.get(&id))
            {
                saved.rebase(old, &current);
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
