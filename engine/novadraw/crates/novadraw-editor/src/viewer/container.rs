use super::*;
use crate::container::{ContainerEditError, PolicyGeneration};
use novadraw::geometry::Affine2D;

impl<A: ModelAdapter, F: EditPartFactory<A>> GraphicalViewer<A, F> {
    /// Returns the actual content-pane child-domain map, including insets, scroll, and scale.
    pub fn child_content_to_surface(&self, part: EditPartId) -> Result<Affine2D, ViewerError> {
        self.validate_selectable(part)?;
        let node = self.parts.get(part).ok_or(ViewerError::InvalidPart(part))?;
        if node.kind() != PartKind::Containment {
            return Err(ViewerError::InvalidPart(part));
        }
        self.runtime
            .tree()
            .child_content_to_surface_transform(node.content_pane())
            .map_err(ViewerError::Coordinate)
    }

    /// Returns `None` only for mathematical singularity of the content-pane map.
    pub fn surface_to_child_content(
        &self,
        part: EditPartId,
    ) -> Result<Option<Affine2D>, ContainerEditError> {
        self.child_content_to_surface(part)?
            .checked_inverse()
            .map_err(ContainerEditError::Inverse)
    }

    /// Explicitly invalidates gesture permissions after application policy configuration changes.
    pub fn invalidate_container_policy(&mut self, host: EditPartId) -> Result<(), ViewerError> {
        self.validate_selectable(host)?;
        self.policies.invalidate(host);
        Ok(())
    }

    /// Returns the current version of the checked host's policy configuration.
    pub fn container_policy_generation(
        &self,
        host: EditPartId,
    ) -> Result<PolicyGeneration, ViewerError> {
        self.validate_selectable(host)?;
        Ok(self.policies.generation(host))
    }

    /// Resolves containment hits while pruning whole source visual subtrees and overlay layers.
    pub fn container_target_at(
        &self,
        location: Point,
        sources: &[EditPartId],
    ) -> Result<EditPartId, ContainerEditError> {
        if !location.x().is_finite() || !location.y().is_finite() {
            return Err(ContainerEditError::Geometry("non-finite container pointer"));
        }
        let mut excluded = vec![
            self.root_layers.handles(),
            self.root_layers.feedback(),
            self.root_layers.scaled_feedback(),
            self.root_layers.connection(),
        ];
        for &source in sources {
            self.validate_selectable(source)?;
            let node = self
                .parts
                .get(source)
                .ok_or(ContainerEditError::Identity(source))?;
            if node.kind() != PartKind::Containment {
                return Err(ContainerEditError::Identity(source));
            }
            excluded.extend_from_slice(node.visuals());
        }
        loop {
            let Some((figure, _)) = self
                .runtime
                .tree()
                .hit_test_excluding((location.x(), location.y()), excluded.iter().copied())
            else {
                return Ok(self.contents());
            };
            match self.visual_owner_or_ancestor(figure) {
                Some((owner_figure, owner)) => {
                    if owner.is_part()
                        && let Some(part) = owner.owner_part()
                        && self.parts.get(part).is_some_and(|node| {
                            node.kind() == PartKind::Containment && node.is_active()
                        })
                    {
                        if part == self.root() {
                            return Ok(self.contents());
                        }
                        return Ok(part);
                    }
                    excluded.push(owner_figure);
                }
                None => return Ok(self.contents()),
            }
        }
    }
}
