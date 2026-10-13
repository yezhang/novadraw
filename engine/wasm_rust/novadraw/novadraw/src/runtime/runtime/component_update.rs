use crate::render::ResourceId;
use crate::runtime::PreparedFigureUpdate;
use crate::runtime::{FigureUpdateCommitContext, FigureUpdateEffectContext};
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
        commit: impl FnOnce(T, &mut crate::FigureNode),
    ) -> ComponentUpdateReceipt {
        let (value, invalidation, changed, effects) = prepared.into_parts();
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

        let mut effect_context = FigureUpdateCommitContext {
            runtime: self,
            figure_id: figure,
        };
        for effect in effects {
            effect.commit(&mut effect_context);
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
                self.tree.is_effectively_visible(figure),
            )
        };
        let context = FigureComponentContext {
            figure_id: figure,
            component_revision: previous_revision,
            bounds,
        };

        self.guarded(move |runtime| {
            let prepared = {
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
            prepared
                .validate_effects(FigureUpdateEffectContext {
                    runtime,
                    figure_id: figure,
                })
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
                |value, node| {
                    let target = node
                        .component_mut::<U::Figure>()
                        .expect("Figure type cannot change during component update");
                    U::commit(value, target);
                },
            ))
        })
    }

    fn validate_reparent_effect(
        &self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<(), RuntimeMutationError> {
        self.validate_attached_figure(child)?;
        self.validate_attached_figure(new_parent)?;
        let old_parent = self
            .tree
            .parent_id(child)
            .ok_or(RuntimeMutationError::DetachedFigure(child))?;
        if old_parent == new_parent {
            return Ok(());
        }
        if self.tree.is_layered_pane(old_parent) {
            return Err(RuntimeMutationError::LayeredParent(old_parent));
        }
        if self.tree.is_layered_pane(new_parent) {
            return Err(RuntimeMutationError::LayeredParent(new_parent));
        }
        self.tree.validate_attachment(new_parent, child)?;
        let scene = FigureTreeRouteSceneRead::new(&self.tree, &self.anchor_geometries);
        self.connections.prepare_reparent_constraint(
            ConnectionId::from_figure(child),
            CoordinateSpace::ChildContent(old_parent),
            CoordinateSpace::ChildContent(new_parent),
            &scene,
        )?;
        Ok(())
    }
}

impl FigureUpdateEffectContext<'_> {
    pub fn validate_reparent(
        self,
        child: FigureId,
        new_parent: FigureId,
    ) -> Result<(), RuntimeMutationError> {
        self.runtime.validate_reparent_effect(child, new_parent)
    }

    pub(crate) fn validate_resource_dependency_change(
        self,
        old_resource: Option<ResourceId>,
        new_resource: Option<ResourceId>,
    ) -> Result<(), RuntimeMutationError> {
        if old_resource == new_resource {
            return Ok(());
        }
        if let Some(resource) = old_resource {
            self.runtime.resources.status(resource)?;
        }
        if let Some(resource) = new_resource {
            self.runtime.resources.status(resource)?;
        }
        Ok(())
    }

    pub(crate) fn validate_node_bounds(
        self,
        bounds: crate::Rectangle,
    ) -> Result<(), RuntimeMutationError> {
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
        Ok(())
    }
}

impl FigureUpdateCommitContext<'_> {
    pub fn record_property_change<V>(
        &mut self,
        property: PropertyKey<V>,
        old_value: V,
        new_value: V,
    ) where
        V: PropertyValueType,
    {
        self.runtime
            .tree
            .record_property_change(self.figure_id, property, old_value, new_value);
    }

    pub fn reparent(&mut self, child: FigureId, new_parent: FigureId) -> bool {
        self.runtime
            .reparent_inner(child, new_parent)
            .expect("validated Figure update effect must commit")
    }

    pub(crate) fn commit_resource_dependency_change(
        &mut self,
        old_resource: Option<ResourceId>,
        new_resource: Option<ResourceId>,
    ) {
        if old_resource == new_resource {
            return;
        }
        if let Some(resource) = new_resource {
            self.runtime
                .resources
                .add_dependency(resource, self.figure_id)
                .expect("validated resource dependency must commit");
        }
        if let Some(resource) = old_resource {
            self.runtime
                .resources
                .remove_dependency(resource, self.figure_id)
                .expect("validated resource dependency must commit");
        }
    }

    pub(crate) fn refresh_border_state(&mut self) {
        let node = self
            .runtime
            .tree
            .node_mut(self.figure_id)
            .expect("validated Figure update target must remain attached");
        node.state.border_snapshot = None;
        node.state.insets = node
            .figure
            .get_border()
            .map(crate::figure::Border::get_insets)
            .unwrap_or_default();
    }

    pub(crate) fn commit_node_bounds(&mut self, bounds: crate::Rectangle) {
        self.runtime.set_bounds_inner(self.figure_id, bounds);
    }
}
