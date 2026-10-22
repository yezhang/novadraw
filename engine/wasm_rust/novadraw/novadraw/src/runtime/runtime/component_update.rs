use crate::render::ResourceId;
use crate::runtime::FigureUpdateEffectContext;
use crate::runtime::PreparedFigureUpdate;
use crate::runtime::mutation::ValidatedFigureEffect;
use crate::{
    ComponentInvalidation, ComponentUpdateError, ComponentUpdateReceipt, FigureComponentContext,
    FigureComponentUpdate, FigureId, PropertyKey, PropertyValueType, RuntimeMutationError,
};

use super::Runtime;
use super::{ConnectionId, CoordinateSpace, FigureTreeRouteSceneRead};

pub(super) struct FigureUpdateRevisionContext {
    pub(super) previous_revision: u64,
    pub(super) revision: u64,
    pub(super) old_visual_bounds: crate::Rectangle,
    pub(super) visible: bool,
}

impl Runtime {
    pub(super) fn commit_prepared_figure_update<T>(
        &mut self,
        figure: FigureId,
        context: FigureUpdateRevisionContext,
        prepared: PreparedFigureUpdate<T>,
        effects: Vec<ValidatedFigureEffect>,
        commit: impl FnOnce(T, &mut crate::FigureNode),
    ) -> ComponentUpdateReceipt {
        let (value, invalidation, changed, _) = prepared.into_parts();
        if !changed {
            return ComponentUpdateReceipt {
                figure,
                previous_revision: context.previous_revision,
                revision: context.previous_revision,
                invalidation,
                changed: false,
            };
        }
        if context.visible {
            self.updates
                .freeze_figure_damage(&self.tree, figure, context.old_visual_bounds);
        }

        {
            let node = self
                .tree
                .node_mut(figure)
                .expect("validated Figure must remain attached during update");
            commit(value, node);
            node.component_revision = context.revision;
        }

        for effect in effects {
            self.commit_figure_effect(figure, effect);
        }

        if invalidation == ComponentInvalidation::LayoutGeometryAndPaint {
            self.tree.mark_invalid(&mut self.updates, figure);
        }
        if invalidation != ComponentInvalidation::Paint {
            self.invalidate_connection_figure_change(figure, false);
        }
        self.tree.repaint(&mut self.updates, figure, None);

        ComponentUpdateReceipt {
            figure,
            previous_revision: context.previous_revision,
            revision: context.revision,
            invalidation,
            changed: true,
        }
    }

    pub(crate) fn update_component<U>(
        &mut self,
        figure: FigureId,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate,
    {
        self.validate_attached_figure(figure)?;
        let (previous_revision, bounds, actual, old_visual_bounds, visible) = {
            let node = self
                .tree
                .node(figure)
                .expect("attached Figure must have a node");
            let actual = node.figure.as_ref().type_name();
            if node.component::<U::Figure>().is_none() {
                return Err(ComponentUpdateError::WrongFigureType {
                    figure,
                    expected: std::any::type_name::<U::Figure>(),
                    actual,
                });
            }
            (
                node.component_revision,
                node.figure_bounds(),
                actual,
                node.visual_bounds(),
                self.tree
                    .is_effectively_visible(figure)
                    .expect("validated update target"),
            )
        };
        let context = FigureComponentContext {
            figure_id: figure,
            component_revision: previous_revision,
            bounds,
        };

        self.guarded(move |runtime| {
            let mut prepared = {
                let node = runtime
                    .tree
                    .node(figure)
                    .expect("validated Figure must remain attached during update");
                let target =
                    node.component::<U::Figure>()
                        .ok_or(ComponentUpdateError::WrongFigureType {
                            figure,
                            expected: std::any::type_name::<U::Figure>(),
                            actual,
                        })?;
                update
                    .prepare(target, context)
                    .map_err(ComponentUpdateError::Rejected)?
            };
            let effects = runtime
                .prepare_figure_effects(figure, &mut prepared)
                .map_err(ComponentUpdateError::Effect)?;
            let revision = if prepared.changed() {
                previous_revision
                    .checked_add(1)
                    .ok_or(ComponentUpdateError::RevisionExhausted(figure))?
            } else {
                previous_revision
            };

            Ok(runtime.commit_prepared_figure_update(
                figure,
                FigureUpdateRevisionContext {
                    previous_revision,
                    revision,
                    old_visual_bounds,
                    visible,
                },
                prepared,
                effects,
                |value, node| {
                    let target = node
                        .component_mut::<U::Figure>()
                        .expect("Figure type cannot change during component update");
                    U::commit(value, target);
                },
            ))
        })
    }

    pub(super) fn prepare_figure_effects<T>(
        &self,
        figure: FigureId,
        prepared: &mut PreparedFigureUpdate<T>,
    ) -> Result<Vec<ValidatedFigureEffect>, RuntimeMutationError> {
        if !prepared.changed() {
            return Ok(Vec::new());
        }
        let mut context = FigureUpdateEffectContext {
            runtime: self,
            figure_id: figure,
            parent: self
                .tree
                .parent_id(figure)
                .expect("validated update target")
                .ok_or(RuntimeMutationError::DetachedFigure(figure))?,
            effects: Vec::new(),
            rejected: false,
        };
        for effect in std::mem::take(&mut prepared.effects) {
            effect.prepare(&mut context)?;
            if context.rejected {
                return Err(RuntimeMutationError::Rejected);
            }
        }
        Ok(context.effects)
    }
}

impl FigureUpdateEffectContext<'_> {
    /// Declares a move of the update target. Other Figure writes are rejected.
    pub fn reparent(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<(), RuntimeMutationError> {
        let result = self.prepare_reparent(child, new_parent);
        self.rejected |= result.is_err();
        result
    }

    fn prepare_reparent(
        &mut self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<(), RuntimeMutationError> {
        self.runtime.validate_attached_figure(child)?;
        self.runtime.validate_attached_figure(new_parent)?;
        if child != self.figure_id {
            return Err(RuntimeMutationError::EffectTargetMismatch {
                target: self.figure_id,
                requested: child,
            });
        }
        if self.parent == new_parent {
            return Ok(());
        }
        self.runtime
            .tree
            .validate_effect_reparent(child, self.parent, new_parent)?;
        // A target-only move cannot change either legal parent's coordinate chain.
        // Remap the preceding prepared constraint, not the committed original.
        let scene =
            FigureTreeRouteSceneRead::new(&self.runtime.tree, &self.runtime.anchor_geometries);
        let connection = ConnectionId::from_figure(child);
        let from = CoordinateSpace::ChildContent(self.parent);
        let to = CoordinateSpace::ChildContent(new_parent);
        let previous = self.effects.iter().rev().find_map(|effect| match effect {
            ValidatedFigureEffect::Reparent {
                constraint: Some(value),
                ..
            } => Some(value.as_ref()),
            _ => None,
        });
        let constraint = if let Some(previous) = previous {
            self.runtime
                .connections
                .prepare_reparent_constraint_value(connection, previous, from, to, &scene)?
        } else {
            self.runtime
                .connections
                .prepare_reparent_constraint(connection, from, to, &scene)?
        };
        self.effects.push(ValidatedFigureEffect::Reparent {
            parent: new_parent,
            constraint,
        });
        self.parent = new_parent;
        Ok(())
    }

    pub fn resource_dependency_change(
        &mut self,
        old_resource: Option<ResourceId>,
        new_resource: Option<ResourceId>,
    ) -> Result<(), RuntimeMutationError> {
        let result = self.prepare_resource_dependency_change(old_resource, new_resource);
        self.rejected |= result.is_err();
        result
    }

    fn prepare_resource_dependency_change(
        &mut self,
        old_resource: Option<ResourceId>,
        new_resource: Option<ResourceId>,
    ) -> Result<(), RuntimeMutationError> {
        if let Some(resource) = old_resource {
            self.runtime.resources.status(resource)?;
        }
        if let Some(resource) = new_resource {
            self.runtime.resources.status(resource)?;
        }
        if old_resource != new_resource {
            self.effects.push(ValidatedFigureEffect::Resource {
                old: old_resource,
                new: new_resource,
            });
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, bounds: crate::Rectangle) -> Result<(), RuntimeMutationError> {
        let result = self.prepare_bounds(bounds);
        self.rejected |= result.is_err();
        result
    }

    fn prepare_bounds(&mut self, bounds: crate::Rectangle) -> Result<(), RuntimeMutationError> {
        if !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width < 0.0
            || bounds.height < 0.0
        {
            return Err(RuntimeMutationError::InvalidBounds {
                figure: self.figure_id,
                bounds,
            });
        }
        self.effects.push(ValidatedFigureEffect::Bounds(bounds));
        Ok(())
    }
    pub fn record_property_change<V>(
        &mut self,
        property: PropertyKey<V>,
        old_value: V,
        new_value: V,
    ) where
        V: PropertyValueType,
    {
        self.effects.push(ValidatedFigureEffect::Property(Box::new(
            move |tree, figure| {
                tree.record_property_change(figure, property, old_value, new_value);
            },
        )));
    }

    pub fn refresh_border_state(&mut self) {
        self.effects.push(ValidatedFigureEffect::Border);
    }
}

impl Runtime {
    fn commit_figure_effect(&mut self, figure: FigureId, effect: ValidatedFigureEffect) {
        match effect {
            ValidatedFigureEffect::Property(record) => record(&mut self.tree, figure),
            ValidatedFigureEffect::Resource { old, new } => {
                if let Some(resource) = new {
                    self.resources
                        .add_dependency(resource, figure)
                        .expect("validated resource dependency must commit");
                    self.record_resource_input(resource, figure);
                }
                if let Some(resource) = old {
                    self.resources
                        .remove_dependency(resource, figure)
                        .expect("validated resource dependency must commit");
                    if let Some(node) = self.tree.node_mut(figure) {
                        node.resource_inputs.retain(|(id, _)| *id != resource);
                    }
                }
            }
            ValidatedFigureEffect::Reparent { parent, constraint } => {
                let old_parent = self
                    .tree
                    .parent_id(figure)
                    .expect("live effect target")
                    .expect("attached effect target");
                self.tree
                    .commit_effect_reparent(&mut self.updates, figure, parent);
                if let Some(constraint) = constraint {
                    self.connections
                        .commit_reparent_constraint(ConnectionId::from_figure(figure), constraint);
                }
                self.invalidate_connection_figure_change(figure, true);
                self.retain_interactive_figures();
                self.tree.complete_detachment(figure, old_parent);
                self.tree.complete_attachment(figure, parent);
            }
            ValidatedFigureEffect::Border => {
                let node = self
                    .tree
                    .node_mut(figure)
                    .expect("validated Figure update target must remain attached");
                node.state.border_snapshot = None;
                node.state.insets = node
                    .figure
                    .get_border()
                    .map(crate::figure::Border::get_insets)
                    .unwrap_or_default();
            }
            ValidatedFigureEffect::Bounds(bounds) => {
                self.set_bounds_inner(figure, bounds);
            }
        }
    }
}
