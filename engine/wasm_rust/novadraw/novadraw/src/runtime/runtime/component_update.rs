use crate::{
    ComponentInvalidation, ComponentUpdateError, ComponentUpdateReceipt, FigureComponentContext,
    FigureComponentUpdate, FigureId,
};

use super::Runtime;

impl Runtime {
    pub(crate) fn update_component<U>(
        &mut self,
        figure: FigureId,
        update: U,
    ) -> Result<ComponentUpdateReceipt, ComponentUpdateError<U::Error>>
    where
        U: FigureComponentUpdate,
    {
        self.validate_attached_figure(figure)?;
        let (previous_revision, revision, bounds, actual, old_visual_bounds, visible) = {
            let node = self
                .tree
                .node(figure)
                .expect("attached Figure must have a node");
            let actual = node.figure.as_ref().type_name();
            if node
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<U::Figure>()
                .is_none()
            {
                return Err(ComponentUpdateError::WrongFigureType {
                    figure,
                    expected: std::any::type_name::<U::Figure>(),
                    actual,
                });
            }
            let revision = node
                .component_revision
                .checked_add(1)
                .ok_or(ComponentUpdateError::RevisionExhausted(figure))?;
            (
                node.component_revision,
                revision,
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
                let target = node
                    .figure
                    .as_ref()
                    .as_any()
                    .downcast_ref::<U::Figure>()
                    .ok_or(ComponentUpdateError::WrongFigureType {
                        figure,
                        expected: std::any::type_name::<U::Figure>(),
                        actual,
                    })?;
                update
                    .prepare(target, context)
                    .map_err(ComponentUpdateError::Rejected)?
            };

            let invalidation = prepared.invalidation;
            if visible {
                runtime
                    .updates
                    .freeze_figure_damage(&runtime.tree, figure, old_visual_bounds);
            }

            let node = runtime
                .tree
                .node_mut(figure)
                .expect("validated Figure must remain attached during update");
            let target = node
                .figure
                .as_mut()
                .as_any_mut()
                .downcast_mut::<U::Figure>()
                .expect("Figure type cannot change during component update");
            U::commit(prepared.value, target);
            node.component_revision = revision;

            if invalidation == ComponentInvalidation::LayoutGeometryAndPaint {
                runtime.tree.mark_invalid(&mut runtime.updates, figure);
            }
            if invalidation != ComponentInvalidation::Paint {
                runtime.invalidate_connection_figure_change(figure, false);
            }
            runtime.tree.repaint(&mut runtime.updates, figure, None);

            Ok(ComponentUpdateReceipt {
                figure,
                previous_revision,
                revision,
                invalidation,
            })
        })
    }
}
