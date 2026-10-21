use super::*;

impl FigureTree {
    pub fn freeform_extent(&self, figure_id: FigureId) -> Result<Rectangle, FreeformError> {
        self.validate_identity(figure_id)?;
        let block = self
            .blocks
            .get(figure_id)
            .ok_or(FreeformError::UnknownFigure(figure_id))?;
        let state = block
            .layout
            .freeform
            .as_ref()
            .ok_or(FreeformError::NotFreeform(figure_id))?;
        if state.extent_generation.is_none() {
            return Err(FreeformError::Unvalidated(figure_id));
        }
        Ok(state.cached_extent)
    }

    /// Derives a freeform extent while omitting complete child subtrees.
    ///
    /// This is a read-only projection over the current validated geometry. It is useful when a
    /// transient visual replaces, rather than supplements, an existing subtree.
    pub fn freeform_extent_excluding(
        &self,
        figure_id: FigureId,
        excluded: &[FigureId],
    ) -> Result<Rectangle, FreeformError> {
        self.validate_identity(figure_id)?;
        for &id in excluded {
            self.validate_identity(id)?;
        }
        let block = self
            .blocks
            .get(figure_id)
            .ok_or(FreeformError::UnknownFigure(figure_id))?;
        let state = block
            .layout
            .freeform
            .as_ref()
            .ok_or(FreeformError::NotFreeform(figure_id))?;
        if state.extent_generation.is_none() {
            return Err(FreeformError::Unvalidated(figure_id));
        }
        Ok(self
            .derive_freeform_extent_excluding(figure_id, excluded)?
            .unwrap_or(Rectangle::ZERO))
    }

    fn derive_freeform_extent_excluding(
        &self,
        figure_id: FigureId,
        excluded: &[FigureId],
    ) -> Result<Option<Rectangle>, FreeformError> {
        let block = self.node(figure_id)?;
        let mut extent: Option<Rectangle> = None;
        for child_id in block.children.iter().copied() {
            if excluded.contains(&child_id) {
                continue;
            }
            let child = &self.blocks[child_id];
            let contribution = if child.layout.freeform.is_some() {
                let child_extent = self
                    .derive_freeform_extent_excluding(child_id, excluded)?
                    .unwrap_or(Rectangle::ZERO);
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * child.child_transform().affine();
                transform_rectangle(transform, child_extent)
                    .ok_or(FreeformError::NonFiniteGeometry(child_id))?
            } else {
                child.figure_bounds()
            };
            if !finite_rectangle(contribution) {
                return Err(FreeformError::NonFiniteGeometry(child_id));
            }
            let combined = match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            };
            if !finite_rectangle(combined) {
                return Err(FreeformError::NonFiniteGeometry(figure_id));
            }
            extent = Some(combined);
        }
        Ok(extent)
    }

    fn recompute_freeform_extent(&mut self, figure_id: FigureId) -> Result<(), LayoutError> {
        let dirty = self
            .blocks
            .get(figure_id)
            .and_then(|block| block.layout.freeform.as_ref())
            .is_some_and(|state| state.dirty);
        if !dirty {
            return Ok(());
        }

        let children = self.blocks[figure_id].children.clone();
        let mut extent: Option<Rectangle> = None;
        for child_id in children {
            if self.blocks[child_id].layout.freeform.is_some() {
                self.recompute_freeform_extent(child_id)?;
            }
            let child = &self.blocks[child_id];
            let contribution = if let Some(state) = child.layout.freeform.as_ref() {
                let child_extent = state.cached_extent;
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * child.child_transform().affine();
                transform_rectangle(transform, child_extent)
                    .ok_or(LayoutError::NonFiniteGeometry { figure: child_id })?
            } else {
                child.figure_bounds()
            };
            if !finite_rectangle(contribution) {
                return Err(LayoutError::NonFiniteGeometry { figure: child_id });
            }
            extent = Some(match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            });
        }

        let new_extent = extent.unwrap_or(Rectangle::ZERO);
        if !finite_rectangle(new_extent) {
            return Err(LayoutError::NonFiniteGeometry { figure: figure_id });
        }
        let (old_extent, changed) = {
            let block = &mut self.blocks[figure_id];
            let generation = block.layout.generation();
            let state = block
                .layout
                .freeform
                .as_mut()
                .expect("dirty freeform state must exist");
            let old_extent = state.cached_extent;
            state.cached_extent = new_extent;
            state.extent_generation = Some(generation);
            state.dirty = false;
            (old_extent, old_extent != new_extent)
        };
        if changed {
            self.record_property_change(
                figure_id,
                FREEFORM_EXTENT_PROPERTY,
                old_extent,
                new_extent,
            );
        }
        Ok(())
    }

    /// Executes the graph-owned validation phase while UpdateManager owns only
    /// the pending work queue and phase trigger.
    pub(crate) fn perform_validation_cycle(
        &mut self,
        update_manager: &mut UpdateManager,
    ) -> Result<(), ValidationError> {
        self.perform_validation_cycle_with_budget(update_manager, DEFAULT_VALIDATION_BUDGET)
    }

    pub(crate) fn perform_validation_cycle_with_budget(
        &mut self,
        update_manager: &mut UpdateManager,
        budget: usize,
    ) -> Result<(), ValidationError> {
        let mut processed = 0;
        let mut invalidation_chain = Vec::new();
        loop {
            let figure_ids = update_manager.drain_invalid_figures();
            if figure_ids.is_empty() {
                return Ok(());
            }
            let remaining = budget.saturating_sub(processed);
            if figure_ids.len() > remaining {
                invalidation_chain.extend(figure_ids.iter().take(remaining).copied());
                for figure_id in &figure_ids {
                    update_manager.add_invalid_figure(*figure_id);
                }
                return Err(ValidationError::NonConvergingValidation {
                    budget,
                    invalidation_chain,
                });
            }
            processed += figure_ids.len();
            invalidation_chain.extend(figure_ids.iter().copied());

            for figure_id in &figure_ids {
                self.mark_validation_path_invalid(*figure_id);
            }

            let mut validation_roots: Vec<FigureId> = figure_ids
                .into_iter()
                .filter_map(|figure_id| self.validation_root(figure_id))
                .collect();
            validation_roots.sort_by_key(|id| self.depth(*id).unwrap_or(usize::MAX));
            validation_roots.dedup();

            for root_id in validation_roots {
                if let Err(error) = self.revalidate_with_update(update_manager, root_id) {
                    update_manager.add_invalid_figure(root_id);
                    return Err(error.into());
                }
            }
        }
    }

    fn validation_root(&self, figure_id: FigureId) -> Option<FigureId> {
        let mut current = figure_id;
        let mut root = figure_id;
        loop {
            let block = self.blocks.get(current)?;
            let Some(parent_id) = block.parent else {
                return Some(root);
            };
            let parent = self.blocks.get(parent_id)?;
            if parent.is_valid {
                return Some(root);
            }
            root = parent_id;
            current = parent_id;
        }
    }

    fn revalidate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
    ) -> Result<(), LayoutError> {
        let ancestors_visible = match self.parent_id(container_id)? {
            Some(parent) => self.is_effectively_visible(parent)?,
            None => true,
        };
        self.revalidate_with_update_inner(update_manager, container_id, ancestors_visible, 0)
    }

    fn revalidate_with_update_inner(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(
                RECURSIVE_VALIDATION_STACK_RED_ZONE,
                RECURSIVE_VALIDATION_STACK_GROWTH,
                || {
                    self.revalidate_with_update_body(
                        update_manager,
                        container_id,
                        ancestors_visible,
                        depth,
                    )
                },
            );
        }
        self.revalidate_with_update_body(update_manager, container_id, ancestors_visible, depth)
    }

    fn revalidate_with_update_body(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !ancestors_visible || !self.blocks[container_id].is_visible {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            self.revalidate_children_with_update(update_manager, container_id, depth)?;
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let snapshot = LayoutSceneQuery::new(self);
                layout_manager.layout(container_id, &snapshot, &mut output)
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            match result {
                Ok(result) => result?,
                Err(payload) => std::panic::resume_unwind(payload),
            }
            self.apply_layout_output(update_manager, container_id, output)?;
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        self.revalidate_children_with_update(update_manager, container_id, depth)?;
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    fn revalidate_children_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        parent_id: FigureId,
        parent_depth: usize,
    ) -> Result<(), LayoutError> {
        let children = self
            .blocks
            .get(parent_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.revalidate_with_update_inner(update_manager, child_id, true, parent_depth + 1)?;
        }
        Ok(())
    }

    pub(crate) fn validate_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
    ) -> Result<(), LayoutError> {
        self.revalidate_with_update(update_manager, container_id)
    }

    #[cfg(test)]
    pub(crate) fn revalidate(&mut self, container_id: FigureId) {
        self.try_revalidate(container_id)
            .expect("layout validation failed");
    }

    pub(crate) fn try_revalidate(&mut self, container_id: FigureId) -> Result<(), LayoutError> {
        let ancestors_visible = match self.parent_id(container_id)? {
            Some(parent) => self.is_effectively_visible(parent)?,
            None => true,
        };
        self.try_revalidate_inner(container_id, ancestors_visible, 0)
    }

    fn try_revalidate_inner(
        &mut self,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            return stacker::maybe_grow(
                RECURSIVE_VALIDATION_STACK_RED_ZONE,
                RECURSIVE_VALIDATION_STACK_GROWTH,
                || self.try_revalidate_body(container_id, ancestors_visible, depth),
            );
        }
        self.try_revalidate_body(container_id, ancestors_visible, depth)
    }

    fn try_revalidate_body(
        &mut self,
        container_id: FigureId,
        ancestors_visible: bool,
        depth: usize,
    ) -> Result<(), LayoutError> {
        if self
            .blocks
            .get(container_id)
            .is_none_or(|block| block.is_valid)
        {
            return Ok(());
        }
        if !ancestors_visible || !self.blocks[container_id].is_visible {
            return Ok(());
        }

        let prevalidate_children = self.blocks[container_id]
            .layout
            .manager
            .as_deref()
            .is_some_and(LayoutManager::requires_valid_children_before_layout);
        if prevalidate_children {
            let children = self.blocks[container_id].children.clone();
            for child_id in children {
                self.try_revalidate_inner(child_id, true, depth + 1)?;
            }
        }

        let layout_manager = self
            .blocks
            .get_mut(container_id)
            .and_then(|block| block.layout.manager.take());
        if let Some(mut layout_manager) = layout_manager {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Started,
                container_id,
                child_id: None,
            });
            let mut output = LayoutOutput::new();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let snapshot = LayoutSceneQuery::new(self);
                layout_manager.layout(container_id, &snapshot, &mut output)
            }));
            if let Some(block) = self.blocks.get_mut(container_id) {
                block.layout.manager = Some(layout_manager);
            }
            match result {
                Ok(result) => result?,
                Err(payload) => std::panic::resume_unwind(payload),
            }
            self.apply_layout_output_without_update(container_id, output)?;
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Finished,
                container_id,
                child_id: None,
            });
        }

        let children = self
            .blocks
            .get(container_id)
            .map(|block| block.children.clone())
            .unwrap_or_default();
        for child_id in children {
            self.try_revalidate_inner(child_id, true, depth + 1)?;
        }
        self.recompute_freeform_extent(container_id)?;
        if let Some(block) = self.blocks.get_mut(container_id) {
            let bounds = block.figure_bounds();
            if let Some(lifecycle) = block.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    pub(crate) fn validate_layout_output(
        &self,
        container_id: FigureId,
        output: &LayoutOutput,
    ) -> Result<(), LayoutError> {
        for change in &output.changes {
            let child_id = match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    if !finite_rectangle(*bounds) || bounds.width < 0.0 || bounds.height < 0.0 {
                        return Err(LayoutError::NonFiniteGeometry { figure: *child_id });
                    }
                    Some(*child_id)
                }
                LayoutChange::Visibility(child_id, _) | LayoutChange::Invalidate(child_id) => {
                    Some(*child_id)
                }
                LayoutChange::Property(event) => {
                    if event.figure_id() != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: event.figure_id(),
                        });
                    }
                    None
                }
                LayoutChange::CoordinateSystemChanged(figure)
                | LayoutChange::Repaint(figure)
                | LayoutChange::RepaintParent(figure) => {
                    if *figure != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: *figure,
                        });
                    }
                    None
                }
                LayoutChange::ViewportEffect(effect) => {
                    if effect.viewport() != container_id {
                        return Err(LayoutError::InvalidChild {
                            container: container_id,
                            child: effect.viewport(),
                        });
                    }
                    None
                }
            };
            if let Some(child_id) = child_id
                && self
                    .blocks
                    .get(child_id)
                    .is_none_or(|child| child.parent != Some(container_id))
            {
                return Err(LayoutError::InvalidChild {
                    container: container_id,
                    child: child_id,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn apply_layout_output(
        &mut self,
        update_manager: &mut UpdateManager,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    self.set_bounds_with_update(
                        update_manager,
                        child_id,
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                    );
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible_with_update(update_manager, child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_invalid(update_manager, child_id);
                }
                LayoutChange::Property(event) => {
                    self.record_property_event(event);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(figure) => {
                    self.repaint(update_manager, figure, None);
                }
                LayoutChange::RepaintParent(figure) => {
                    if let Some(parent) = self.parent_id(figure)? {
                        self.repaint(update_manager, parent, None);
                    }
                }
                LayoutChange::ViewportEffect(effect) => effect.commit()?,
            }
        }
        Ok(())
    }

    fn apply_layout_output_without_update(
        &mut self,
        container_id: FigureId,
        output: LayoutOutput,
    ) -> Result<(), LayoutError> {
        self.validate_layout_output(container_id, &output)?;
        for change in output.changes {
            match change {
                LayoutChange::Bounds(child_id, bounds) => {
                    let old_bounds = self.figure_bounds(child_id);
                    self.set_bounds(child_id, bounds.x, bounds.y, bounds.width, bounds.height);
                    if old_bounds
                        .is_ok_and(|old| old.width != bounds.width || old.height != bounds.height)
                        && let Some(child) = self.blocks.get_mut(child_id)
                    {
                        child.is_valid = false;
                    }
                }
                LayoutChange::Visibility(child_id, visible) => {
                    self.set_visible(child_id, visible);
                }
                LayoutChange::Invalidate(child_id) => {
                    self.mark_validation_path_invalid(child_id);
                }
                LayoutChange::Property(event) => {
                    self.record_property_event(event);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(_) | LayoutChange::RepaintParent(_) => {}
                LayoutChange::ViewportEffect(effect) => effect.commit()?,
            }
        }
        Ok(())
    }

    pub fn is_layout_valid(&self) -> bool {
        self.blocks
            .get(self.contents.unwrap_or(self.root))
            .map(|block| block.is_valid)
            .unwrap_or(true)
    }

    pub fn is_valid(&self, figure_id: FigureId) -> Result<bool, TreeQueryError> {
        Ok(self.node(figure_id)?.is_valid)
    }

    pub fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        if let Some(size) = block.preferred_size {
            let raw = crate::layout::validate_measurement(
                figure_id,
                FigureMeasurement::new(size.width, size.height, None),
            )?;
            return validate_projected_measurement(
                figure_id,
                block.project_preferred_measurement(raw),
            );
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let constraints = block.layout_content_constraints(figure_id, constraints)?;
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().preferred
                && cached.matches(generation, constraints)
            {
                return Ok(cached.measurement);
            }
            let snapshot = LayoutSceneQuery::new(self);
            let raw = crate::layout::validate_measurement(
                figure_id,
                layout.preferred_measurement(figure_id, constraints, &snapshot)?,
            )?;
            let measurement = block.project_layout_preferred_measurement(figure_id, raw)?;
            block.layout.cache.borrow_mut().preferred = Some(CachedMeasurement {
                generation,
                constraints,
                measurement,
            });
            return Ok(measurement);
        }
        let constraints = block.layout_constraints(constraints);
        let measurement = if let Some(prepared) = &block.prepared {
            prepared.presentation.measurement()
        } else if block.border_snapshot.is_some() {
            block.figure.intrinsic_content_measurement(constraints)
        } else {
            block.figure.intrinsic_measurement(constraints)
        };
        let measurement = crate::layout::validate_measurement(figure_id, measurement)?;
        validate_projected_measurement(
            figure_id,
            owner_scoped_border_measurement(
                figure_id,
                measurement,
                block.border_snapshot.as_ref(),
            )?,
        )
    }

    pub fn minimum_size(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        if let Some(size) = block.minimum_size {
            crate::layout::validate_measurement_size(figure_id, size, false)?;
            return validate_projected_size(figure_id, block.project_minimum_size(size));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let constraints = block.layout_content_constraints(figure_id, constraints)?;
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().minimum
                && cached.matches(generation, constraints)
            {
                return Ok(cached.measurement.size());
            }
            let snapshot = LayoutSceneQuery::new(self);
            let raw = layout.minimum_size(figure_id, constraints, &snapshot)?;
            crate::layout::validate_measurement_size(figure_id, raw, false)?;
            let size = block.project_layout_minimum_size(figure_id, raw)?;
            block.layout.cache.borrow_mut().minimum = Some(CachedMeasurement {
                generation,
                constraints,
                measurement: FigureMeasurement::new(size.width, size.height, None),
            });
            return Ok(size);
        }
        let constraints = block.layout_constraints(constraints);
        let measurement = if let Some(prepared) = &block.prepared {
            let size = prepared.presentation.minimum_size();
            FigureMeasurement::new(size.width, size.height, None)
        } else if block.border_snapshot.is_some() {
            block
                .figure
                .intrinsic_content_minimum_measurement(constraints)
        } else {
            block.figure.intrinsic_minimum_measurement(constraints)
        };
        let measurement = crate::layout::validate_measurement(figure_id, measurement)?;
        let projected = validate_projected_measurement(
            figure_id,
            owner_scoped_border_measurement(
                figure_id,
                measurement,
                block.border_snapshot.as_ref(),
            )?,
        )?;
        Ok(projected.size())
    }

    pub fn maximum_size(&self, figure_id: FigureId) -> Result<Dimension, LayoutError> {
        self.validate_identity(figure_id)?;
        let block = &self.blocks[figure_id];
        crate::layout::validate_measurement_size(
            figure_id,
            block
                .maximum_size
                .unwrap_or(Dimension::new(f64::INFINITY, f64::INFINITY)),
            true,
        )
    }

    pub(crate) fn set_preferred_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.preferred_size == size {
            return false;
        }
        block.preferred_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn set_minimum_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.minimum_size == size {
            return false;
        }
        block.minimum_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn set_maximum_size(
        &mut self,
        figure_id: FigureId,
        size: Option<Dimension>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.maximum_size == size {
            return false;
        }
        block.maximum_size = size;
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::ExplicitSize);
        true
    }

    pub(crate) fn replace_layout_manager(
        &mut self,
        container: FigureId,
        layout_manager: Option<Box<dyn LayoutManager>>,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(container) else {
            return false;
        };
        if block.layout.manager.is_none() && layout_manager.is_none() {
            return false;
        }
        block.layout.manager = layout_manager;
        self.mark_validation_path_invalid_for(container, LayoutInvalidation::Structure);
        true
    }

    pub(crate) fn validate_layout_manager_constraints(
        &self,
        container: FigureId,
        layout_manager: &dyn LayoutManager,
    ) -> Result<(), LayoutError> {
        self.validate_identity(container)?;
        let block = &self.blocks[container];
        for (child, constraint) in &block.layout.constraints {
            layout_manager.validate_constraint(container, *child, constraint.as_ref())?;
        }
        Ok(())
    }

    pub fn layout_manager(
        &self,
        container: FigureId,
    ) -> Result<Option<&dyn LayoutManager>, TreeQueryError> {
        Ok(self.node(container)?.layout.manager.as_deref())
    }

    pub(super) fn validate_layout_child(&self, child: FigureId) -> Result<FigureId, LayoutError> {
        self.validate_identity(child)?;
        let child_node = &self.blocks[child];
        child_node.parent.ok_or(LayoutError::InvalidChild {
            container: self.root,
            child,
        })
    }

    pub(super) fn validate_layout_constraint(
        &self,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        let parent = self.validate_layout_child(child)?;
        if let Some(manager) = self.layout_manager(parent)? {
            manager.validate_constraint(parent, child, constraint)?;
        }
        Ok(())
    }

    pub(crate) fn set_boxed_constraint(
        &mut self,
        child_id: FigureId,
        constraint: Box<dyn LayoutConstraint>,
    ) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let Some(parent) = self.blocks.get_mut(parent_id) else {
            return false;
        };
        parent.layout.constraints.insert(child_id, constraint);
        self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
        self.emit_layout_event(LayoutEvent {
            kind: LayoutEventKind::ConstraintChanged,
            container_id: parent_id,
            child_id: Some(child_id),
        });
        true
    }

    pub fn layout_constraint<C>(&self, child_id: FigureId) -> Result<Option<&C>, LayoutError>
    where
        C: LayoutConstraint,
    {
        self.validate_identity(child_id)?;
        let Some(constraint) = self.constraint(child_id) else {
            return Ok(None);
        };
        constraint.as_any().downcast_ref::<C>().map(Some).ok_or(
            LayoutError::ConstraintTypeMismatch {
                container: self.blocks[child_id]
                    .parent
                    .expect("stored constraint has a parent"),
                child: child_id,
                expected: std::any::type_name::<C>(),
                actual: constraint.type_name(),
            },
        )
    }

    pub(crate) fn remove_constraint(&mut self, child_id: FigureId) -> bool {
        let Some(parent_id) = self.blocks.get(child_id).and_then(|child| child.parent) else {
            return false;
        };
        let removed = self
            .blocks
            .get_mut(parent_id)
            .and_then(|parent| parent.layout.constraints.remove(&child_id))
            .is_some();
        if removed {
            self.mark_validation_path_invalid_for(parent_id, LayoutInvalidation::Constraint);
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::ConstraintChanged,
                container_id: parent_id,
                child_id: Some(child_id),
            });
        }
        removed
    }

    fn constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        let parent_id = self.blocks.get(child_id)?.parent?;
        self.blocks
            .get(parent_id)?
            .layout
            .constraints
            .get(&child_id)
            .map(Box::as_ref)
    }

    pub(super) fn nearest_freeform_ancestor(&self, figure_id: FigureId) -> Option<FigureId> {
        let parent = self.blocks.get(figure_id)?.parent?;
        self.blocks[parent].layout.freeform.as_ref()?;
        Some(parent)
    }

    pub(super) fn mark_freeform_ancestor_extents_dirty(&mut self, figure_id: FigureId) {
        let mut current = self.blocks.get(figure_id).and_then(|block| block.parent);
        while let Some(id) = current {
            let Some(block) = self.blocks.get_mut(id) else {
                break;
            };
            if block.layout.freeform.is_none() {
                break;
            }
            block.is_valid = false;
            block.layout.invalidate(LayoutInvalidation::Geometry);
            current = block.parent;
        }
    }

    pub(super) fn mark_validation_path_invalid(&mut self, figure_id: FigureId) {
        self.mark_validation_path_invalid_for(figure_id, LayoutInvalidation::Geometry);
    }

    pub(super) fn mark_validation_path_invalid_for(
        &mut self,
        mut figure_id: FigureId,
        reason: LayoutInvalidation,
    ) {
        let mut invalidated = Vec::new();
        loop {
            let (parent, was_valid) = if let Some(block) = self.blocks.get_mut(figure_id) {
                let was_valid = block.is_valid;
                block.is_valid = false;
                block.layout.invalidate(reason);
                if was_valid && let Some(lifecycle) = block.lifecycle() {
                    lifecycle.invalidate();
                }
                (block.parent, was_valid)
            } else {
                (None, false)
            };

            if was_valid {
                invalidated.push(figure_id);
            }
            match parent {
                Some(parent_id) => figure_id = parent_id,
                None => break,
            }
        }
        for container_id in invalidated {
            self.emit_layout_event(LayoutEvent {
                kind: LayoutEventKind::Invalidated,
                container_id,
                child_id: None,
            });
        }
    }

    pub(crate) fn invalid_figure_ids(&self) -> Vec<FigureId> {
        self.blocks
            .iter()
            .filter_map(|(id, block)| (!block.is_valid).then_some(id))
            .collect()
    }

    pub(crate) fn invalid_validation_roots(&self) -> Vec<FigureId> {
        let mut roots = self
            .blocks
            .iter()
            .filter_map(|(id, block)| {
                if block.is_valid {
                    return None;
                }
                let parent_is_valid = block
                    .parent
                    .and_then(|parent| self.blocks.get(parent))
                    .is_some_and(|parent| parent.is_valid);
                (block.parent.is_none() || parent_is_valid).then_some(id)
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|id| self.depth(*id).unwrap_or(usize::MAX));
        roots
    }
}

impl crate::layout::LayoutSceneRead for FigureTree {
    fn get_children(&self, parent_id: FigureId) -> Result<Vec<(FigureId, Rectangle)>, LayoutError> {
        self.validate_identity(parent_id)?;
        let block = &self.blocks[parent_id];
        Ok(block
            .children
            .iter()
            .map(|&child_id| (child_id, self.blocks[child_id].figure_bounds()))
            .collect())
    }

    fn get_constraint(
        &self,
        child_id: FigureId,
    ) -> Result<Option<&dyn LayoutConstraint>, LayoutError> {
        self.validate_identity(child_id)?;
        Ok(self.constraint(child_id))
    }

    fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<FigureMeasurement, LayoutError> {
        self.preferred_measurement(figure_id, constraints)
    }

    fn minimum_size(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Result<Dimension, LayoutError> {
        self.minimum_size(figure_id, constraints)
    }

    fn maximum_size(&self, figure_id: FigureId) -> Result<Dimension, LayoutError> {
        self.maximum_size(figure_id)
    }

    fn get_container_bounds(&self, container_id: FigureId) -> Result<Rectangle, LayoutError> {
        self.validate_identity(container_id)?;
        let area = self.blocks[container_id].child_layout_area();
        if !finite_rectangle(area) {
            return Err(LayoutError::NonFiniteGeometry {
                figure: container_id,
            });
        }
        Ok(area)
    }

    fn get_freeform_extent(&self, figure_id: FigureId) -> Result<Option<Rectangle>, LayoutError> {
        self.validate_identity(figure_id)?;
        match self.freeform_extent(figure_id) {
            Ok(extent) => Ok(Some(extent)),
            Err(FreeformError::NotFreeform(_)) => Ok(None),
            Err(error) => Err(LayoutError::Freeform(error)),
        }
    }

    fn get_content_scale(&self, figure_id: FigureId) -> Result<Option<f64>, LayoutError> {
        self.validate_identity(figure_id)?;
        Ok(self
            .capability(figure_id, SCALE)
            .map_err(LayoutError::Capability)?
            .map(|capability| capability.model().scale()))
    }
}

pub(super) fn validate_projected_measurement(
    figure: FigureId,
    measurement: FigureMeasurement,
) -> Result<FigureMeasurement, LayoutError> {
    if !measurement.width.is_finite()
        || !measurement.height.is_finite()
        || measurement
            .baseline
            .is_some_and(|baseline| !baseline.is_finite())
    {
        return Err(LayoutError::InvalidMeasurement {
            figure,
            reason: crate::layout::MeasurementFailure::DerivedOverflow,
        });
    }
    crate::layout::validate_measurement(figure, measurement)
}

pub(super) fn validate_projected_size(
    figure: FigureId,
    size: Dimension,
) -> Result<Dimension, LayoutError> {
    validate_projected_measurement(
        figure,
        FigureMeasurement::new(size.width, size.height, None),
    )
    .map(FigureMeasurement::size)
}

#[cfg(test)]
mod s07_query_failure_tests {
    use super::*;
    use crate::RectangleFigure;
    use crate::identity::RuntimeHandle;
    use crate::layout::MeasurementFailure;

    fn subject() -> (FigureTree, FigureId) {
        let mut tree = FigureTree::new();
        let id = tree.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0)));
        (tree, id)
    }

    #[test]
    fn same_namespace_missing_identity_is_rejected_by_every_layout_query() {
        let (tree, root) = subject();
        let missing = FigureId::from_local(root.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        let error = LayoutError::Identity(TreeQueryError::UnknownFigure(missing));
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(query.children(missing), Err(error));
        assert_eq!(query.constraint(missing).err(), Some(error));
        assert_eq!(query.container_bounds(missing), Err(error));
        assert_eq!(
            query.preferred_measurement(missing, MeasureConstraints::UNBOUNDED),
            Err(error)
        );
        assert_eq!(
            query.minimum_size(missing, MeasureConstraints::UNBOUNDED),
            Err(error)
        );
        assert_eq!(query.maximum_size(missing), Err(error));
        assert_eq!(query.freeform_extent(missing), Err(error));
        assert_eq!(query.content_scale(missing), Err(error));
    }

    #[test]
    fn override_admission_distinguishes_bounded_and_unbounded_measurement() {
        let (mut tree, root) = subject();
        for axis in [f64::NAN, -1.0, f64::INFINITY, f64::NEG_INFINITY] {
            for size in [Dimension::new(axis, 0.0), Dimension::new(0.0, axis)] {
                tree.set_preferred_size(root, Some(size));
                tree.set_minimum_size(root, Some(size));
                let error = LayoutError::InvalidMeasurement {
                    figure: root,
                    reason: MeasurementFailure::InvalidSize,
                };
                assert_eq!(
                    tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
                    Err(error)
                );
                assert_eq!(
                    tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
                    Err(error)
                );
                tree.set_maximum_size(root, Some(size));
                if axis == f64::INFINITY {
                    assert_eq!(tree.maximum_size(root), Ok(size));
                } else {
                    assert_eq!(tree.maximum_size(root), Err(error));
                }
            }
        }
        tree.set_preferred_size(root, Some(Dimension::ZERO));
        tree.set_minimum_size(root, Some(Dimension::ZERO));
        tree.set_maximum_size(root, Some(Dimension::ZERO));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Ok(FigureMeasurement::default())
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::ZERO)
        );
        assert_eq!(tree.maximum_size(root), Ok(Dimension::ZERO));
    }

    #[test]
    fn layout_relationship_and_optional_absence_have_separate_results() {
        let (mut tree, root) = subject();
        let child = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0)));
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(query.children(child), Ok(Vec::new()));
        assert!(query.constraint(child).unwrap().is_none());
        assert_eq!(query.constraint_as::<Rectangle>(root, child), Ok(None));
        assert_eq!(
            query.constraint_as::<Rectangle>(child, root),
            Err(LayoutError::InvalidChild {
                container: child,
                child: root
            })
        );
        assert_eq!(query.freeform_extent(child), Ok(None));
        assert_eq!(query.content_scale(child), Ok(None));
    }

    #[test]
    fn layout_constraint_type_and_unvalidated_freeform_are_not_absence() {
        let (mut tree, root) = subject();
        let child = tree.add_child_to(root, Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0)));
        tree.set_boxed_constraint(child, Box::new(Rectangle::ZERO));
        let query = LayoutSceneQuery::new(&tree);
        assert_eq!(
            query.constraint_as::<Rectangle>(root, child),
            Ok(Some(&Rectangle::ZERO))
        );
        assert!(matches!(
            query.constraint_as::<crate::layout::XYConstraint>(root, child),
            Err(LayoutError::ConstraintTypeMismatch { container, child: subject, .. })
                if container == root && subject == child
        ));
        let mut freeform_tree = FigureTree::new();
        let freeform = freeform_tree.set_contents(Box::new(
            crate::container::FreeformLayerFigure::new(0.0, 0.0, 10.0, 10.0),
        ));
        assert_eq!(
            LayoutSceneQuery::new(&freeform_tree).freeform_extent(freeform),
            Err(LayoutError::Freeform(FreeformError::Unvalidated(freeform)))
        );
        let mut other = FigureTree::new();
        let foreign = other.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 0.0, 0.0)));
        assert_eq!(
            query.constraint_as::<Rectangle>(foreign, child),
            Err(LayoutError::Identity(TreeQueryError::ForeignFigure(
                foreign
            )))
        );
    }

    #[test]
    fn inset_overflow_fails_before_constraint_reduction_and_retries_after_invalidation() {
        let (mut tree, root) = subject();
        tree.replace_layout_manager(root, Some(Box::new(crate::layout::StackLayout::new())));
        tree.blocks[root].state.insets = Insets::uniform(f64::MAX);
        let error = LayoutError::InvalidMeasurement {
            figure: root,
            reason: MeasurementFailure::DerivedOverflow,
        };
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(error)
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Err(error)
        );
        assert!(tree.blocks[root].layout.cache.borrow().preferred.is_none());
        assert!(tree.blocks[root].layout.cache.borrow().minimum.is_none());
        tree.blocks[root].state.insets = Insets::uniform(1.0);
        tree.mark_validation_path_invalid(root);
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED)
                .unwrap()
                .size(),
            Dimension::new(2.0, 2.0)
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::new(2.0, 2.0))
        );
    }

    #[test]
    fn prepared_and_border_composition_checks_each_selected_stage() {
        use crate::figure::preparation::{PreparationKey, PreparedFigure};
        struct Drawing;
        impl crate::figure::FigureDrawing for Drawing {
            fn paint(
                &self,
                _: &mut crate::graphics::PaintContext<'_>,
            ) -> Result<(), crate::graphics::GraphicsError> {
                Ok(())
            }
        }
        let drawing = std::sync::Arc::new(Drawing);
        for measurement in [
            FigureMeasurement::new(f64::NAN, 1.0, None),
            FigureMeasurement::new(1.0, 1.0, Some(2.0)),
            FigureMeasurement::new(1.0, 1.0, Some(f64::INFINITY)),
        ] {
            assert!(matches!(
                crate::figure::FigurePresentation::new(
                    measurement,
                    Dimension::ZERO,
                    Rectangle::ZERO,
                    drawing.clone()
                ),
                Err(TextError::InvalidMetric)
            ));
        }
        let (mut tree, root) = subject();
        tree.blocks[root].prepared = Some(PreparedFigure {
            owner: root,
            key: PreparationKey {
                component: 0,
                provider: 0,
                text: 0,
                font: String::new(),
                bounds: Rectangle::ZERO,
            },
            presentation: crate::figure::FigurePresentation::new(
                FigureMeasurement::new(4.0, 8.0, Some(6.0)),
                Dimension::new(2.0, 3.0),
                Rectangle::ZERO,
                drawing,
            )
            .unwrap(),
        });
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::uniform(1.0),
            Dimension::ZERO,
        ));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Ok(FigureMeasurement::new(6.0, 10.0, Some(7.0)))
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Ok(Dimension::new(4.0, 5.0))
        );
        // A finite negative top inset makes the composed baseline invalid.
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::new(-7.0, 0.0, 7.0, 0.0),
            Dimension::ZERO,
        ));
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(LayoutError::InvalidMeasurement {
                figure: root,
                reason: MeasurementFailure::InvalidBaseline
            })
        );
        tree.blocks[root].border_snapshot = Some(BorderSnapshot::compound(
            None,
            None,
            Insets::uniform(f64::MAX),
            Dimension::ZERO,
        ));
        let overflow = LayoutError::InvalidMeasurement {
            figure: root,
            reason: MeasurementFailure::DerivedOverflow,
        };
        assert_eq!(
            tree.preferred_measurement(root, MeasureConstraints::UNBOUNDED),
            Err(overflow)
        );
        assert_eq!(
            tree.minimum_size(root, MeasureConstraints::UNBOUNDED),
            Err(overflow)
        );
    }

    #[test]
    fn failed_candidate_publishes_no_geometry_notifications_or_damage() {
        struct FailingLayout {
            survivor: FigureId,
            missing: FigureId,
        }
        impl LayoutManager for FailingLayout {
            fn preferred_measurement(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<FigureMeasurement, LayoutError> {
                Ok(FigureMeasurement::default())
            }
            fn minimum_size(
                &self,
                _: FigureId,
                _: MeasureConstraints,
                _: &LayoutSceneQuery<'_>,
            ) -> Result<Dimension, LayoutError> {
                Ok(Dimension::ZERO)
            }
            fn layout(
                &mut self,
                _: FigureId,
                query: &LayoutSceneQuery<'_>,
                output: &mut LayoutOutput,
            ) -> Result<(), LayoutError> {
                output.set_child_bounds(self.survivor, Rectangle::new(100.0, 200.0, 30.0, 40.0));
                output.repaint(self.survivor);
                query.container_bounds(self.missing)?;
                Ok(())
            }
        }
        let (mut tree, root) = subject();
        let survivor = tree.add_child_to(root, Box::new(RectangleFigure::new(1.0, 2.0, 3.0, 4.0)));
        let missing = FigureId::from_local(root.namespace(), slotmap::KeyData::from_ffi(u64::MAX));
        tree.replace_layout_manager(root, Some(Box::new(FailingLayout { survivor, missing })));
        tree.drain_notification_effects();
        let before = tree.figure_bounds(survivor);
        let mut updates = UpdateManager::with_namespace(tree.namespace());
        updates.add_invalid_figure(root);
        assert_eq!(
            tree.perform_validation_cycle(&mut updates),
            Err(ValidationError::Layout(LayoutError::Identity(
                TreeQueryError::UnknownFigure(missing)
            )))
        );
        assert_eq!(tree.figure_bounds(survivor), before);
        assert!(updates.dirty_regions.is_empty());
        assert!(tree.notification_effects().iter().all(|effect| matches!(
            effect,
            NotificationEffect::EmitLayout(LayoutEvent {
                kind: LayoutEventKind::Started,
                ..
            })
        )));
    }
}
