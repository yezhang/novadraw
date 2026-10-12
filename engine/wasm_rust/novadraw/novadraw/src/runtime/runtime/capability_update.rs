use crate::runtime::FigureUpdateEffectContext;
use crate::{
    CapabilityUpdateError, CapabilityUpdateReceipt, FigureCapabilityContext,
    FigureCapabilityUpdate, FigureId,
};

use super::Runtime;
use super::component_update::FigureUpdateRevisionContext;

impl Runtime {
    /// Applies a typed update through an attached Figure capability descriptor.
    pub fn update_capability<U>(
        &mut self,
        figure: FigureId,
        update: U,
    ) -> Result<CapabilityUpdateReceipt, CapabilityUpdateError<U::Error>>
    where
        U: FigureCapabilityUpdate,
    {
        self.validate_attached_figure(figure)?;
        let (previous_revision, bounds, old_visual_bounds, visible) = {
            let node = self
                .tree
                .node(figure)
                .expect("attached Figure must have a node");
            node.capabilities
                .get(U::KEY)
                .map_err(CapabilityUpdateError::Query)?
                .ok_or(CapabilityUpdateError::Missing {
                    figure,
                    capability: U::KEY.name(),
                })?;
            (
                node.component_revision,
                node.figure_bounds(),
                node.visual_bounds(),
                self.tree.is_effectively_visible(figure),
            )
        };
        let context = FigureCapabilityContext {
            figure_id: figure,
            revision: previous_revision,
            bounds,
        };

        self.guarded(move |runtime| {
            let prepared = {
                let node = runtime
                    .tree
                    .node(figure)
                    .expect("validated Figure must remain attached during update");
                let capability = node
                    .capabilities
                    .get(U::KEY)
                    .map_err(CapabilityUpdateError::Query)?
                    .expect("Figure capability registry cannot change after attach");
                update
                    .prepare(capability, node.figure.as_ref(), context)
                    .map_err(CapabilityUpdateError::Rejected)?
            };
            prepared
                .validate_effects(FigureUpdateEffectContext {
                    runtime,
                    figure_id: figure,
                })
                .map_err(CapabilityUpdateError::Effect)?;
            let revision = if prepared.changed() {
                previous_revision
                    .checked_add(1)
                    .ok_or(CapabilityUpdateError::RevisionExhausted(figure))?
            } else {
                previous_revision
            };

            let receipt = runtime.commit_prepared_figure_update(
                figure,
                FigureUpdateRevisionContext {
                    previous_revision,
                    revision,
                    old_visual_bounds,
                    visible,
                },
                prepared,
                |value, node| {
                    let capability = node
                        .capabilities
                        .get(U::KEY)
                        .expect("validated capability descriptor must remain valid")
                        .expect("Figure capability registry cannot change after attach");
                    U::commit(capability, value, node.figure.as_mut());
                },
            );
            Ok(CapabilityUpdateReceipt {
                figure: receipt.figure,
                previous_revision: receipt.previous_revision,
                revision: receipt.revision,
                invalidation: receipt.invalidation,
                changed: receipt.changed,
            })
        })
    }
}
