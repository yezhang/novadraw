use super::*;

impl FigureTree {
    pub fn freeform_extent(&self, figure_id: FigureId) -> Result<Rectangle, FreeformError> {
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
            .derive_freeform_extent_excluding(figure_id, excluded)
            .unwrap_or(Rectangle::ZERO))
    }

    fn derive_freeform_extent_excluding(
        &self,
        figure_id: FigureId,
        excluded: &[FigureId],
    ) -> Option<Rectangle> {
        let block = self.blocks.get(figure_id)?;
        let mut extent: Option<Rectangle> = None;
        for child_id in block.children.iter().copied() {
            if excluded.contains(&child_id) {
                continue;
            }
            let child = &self.blocks[child_id];
            let contribution = if child.layout.freeform.is_some() {
                let child_extent = self
                    .derive_freeform_extent_excluding(child_id, excluded)
                    .unwrap_or(Rectangle::ZERO);
                let bounds = child.figure_bounds();
                let transform = Affine2D::from_translation(bounds.x, bounds.y)
                    * child.child_transform().affine();
                transform_rectangle(transform, child_extent)?
            } else {
                child.figure_bounds()
            };
            extent = Some(match extent {
                Some(current) => current.union(contribution),
                None => contribution,
            });
        }
        extent
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
                PropertyValue::Rectangle(old_extent),
                PropertyValue::Rectangle(new_extent),
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
        let ancestors_visible = self
            .parent_id(container_id)
            .is_none_or(|parent| self.is_effectively_visible(parent));
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
                let snapshot = LayoutSnapshot::new(self);
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
            if let Some(lifecycle) = block.figure.lifecycle() {
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
        let ancestors_visible = self
            .parent_id(container_id)
            .is_none_or(|parent| self.is_effectively_visible(parent));
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
                let snapshot = LayoutSnapshot::new(self);
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
            if let Some(lifecycle) = block.figure.lifecycle() {
                lifecycle.validate(bounds);
            }
            block.is_valid = true;
            block.layout.mark_validated();
        }
        Ok(())
    }

    fn validate_layout_output(
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
                LayoutChange::Property { figure, .. }
                | LayoutChange::CoordinateSystemChanged(figure)
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

    fn apply_layout_output(
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
                LayoutChange::Property {
                    figure,
                    property,
                    old_value,
                    new_value,
                } => {
                    self.record_property_change(figure, property, old_value, new_value);
                }
                LayoutChange::CoordinateSystemChanged(figure) => {
                    self.record_coordinate_system_changed(figure);
                }
                LayoutChange::Repaint(figure) => {
                    self.repaint(update_manager, figure, None);
                }
                LayoutChange::RepaintParent(figure) => {
                    if let Some(parent) = self.parent_id(figure) {
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
                        .is_some_and(|old| old.width != bounds.width || old.height != bounds.height)
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
                LayoutChange::Property {
                    figure,
                    property,
                    old_value,
                    new_value,
                } => {
                    self.record_property_change(figure, property, old_value, new_value);
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

    pub fn is_valid(&self, figure_id: FigureId) -> bool {
        self.blocks
            .get(figure_id)
            .is_some_and(|block| block.is_valid)
    }

    pub fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Option<FigureMeasurement> {
        let block = self.blocks.get(figure_id)?;
        let constraints = block.layout_constraints(constraints);
        if let Some(size) = block.preferred_size {
            return Some(
                block.project_preferred_measurement(FigureMeasurement::new(size.0, size.1, None)),
            );
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().preferred
                && cached.matches(generation, constraints)
            {
                return Some(cached.measurement);
            }
            let snapshot = LayoutSnapshot::new(self);
            let measurement = block.project_preferred_measurement(layout.preferred_measurement(
                figure_id,
                constraints,
                &snapshot,
            ));
            block.layout.cache.borrow_mut().preferred = Some(CachedMeasurement {
                generation,
                constraints,
                measurement,
            });
            return Some(measurement);
        }
        let measurement = if let Some(prepared) = &block.prepared {
            prepared.presentation.measurement()
        } else if block.border_snapshot.is_some() {
            block.figure.intrinsic_content_measurement(constraints)
        } else {
            block.figure.intrinsic_measurement(constraints)
        };
        Some(owner_scoped_border_measurement(
            measurement,
            block.border_snapshot.as_ref(),
        ))
    }

    pub fn minimum_size(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> Option<Dimension> {
        let block = self.blocks.get(figure_id)?;
        let constraints = block.layout_constraints(constraints);
        if let Some(size) = block.minimum_size {
            return Some(block.project_minimum_size(size.into()));
        }
        if let Some(layout) = block.layout.manager.as_deref() {
            let generation = block.layout.generation();
            if let Some(cached) = block.layout.cache.borrow().minimum
                && cached.matches(generation, constraints)
            {
                return Some(cached.measurement.size());
            }
            let snapshot = LayoutSnapshot::new(self);
            let size =
                block.project_minimum_size(layout.minimum_size(figure_id, constraints, &snapshot));
            block.layout.cache.borrow_mut().minimum = Some(CachedMeasurement {
                generation,
                constraints,
                measurement: FigureMeasurement::new(size.width, size.height, None),
            });
            return Some(size);
        }
        let content = if let Some(prepared) = &block.prepared {
            prepared.presentation.minimum_size()
        } else if block.border_snapshot.is_some() {
            block
                .figure
                .intrinsic_content_minimum_measurement(constraints)
                .size()
        } else {
            block
                .figure
                .intrinsic_minimum_measurement(constraints)
                .size()
        };
        Some(owner_scoped_border_size(
            content,
            block.border_snapshot.as_ref(),
        ))
    }

    pub fn maximum_size(&self, figure_id: FigureId) -> Option<Dimension> {
        let block = self.blocks.get(figure_id)?;
        Some(
            block
                .maximum_size
                .unwrap_or((f64::INFINITY, f64::INFINITY))
                .into(),
        )
    }

    pub(crate) fn set_preferred_size(
        &mut self,
        figure_id: FigureId,
        size: Option<(f64, f64)>,
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
        size: Option<(f64, f64)>,
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
        size: Option<(f64, f64)>,
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
        let block = self
            .blocks
            .get(container)
            .ok_or(LayoutError::UnknownFigure { figure: container })?;
        for (child, constraint) in &block.layout.constraints {
            layout_manager.validate_constraint(container, *child, constraint.as_ref())?;
        }
        Ok(())
    }

    pub fn layout_manager(&self, container: FigureId) -> Option<&dyn LayoutManager> {
        self.blocks
            .get(container)
            .and_then(|block| block.layout.manager.as_deref())
    }

    pub(super) fn validate_layout_child(&self, child: FigureId) -> Result<FigureId, LayoutError> {
        let child_node = self
            .blocks
            .get(child)
            .ok_or(LayoutError::UnknownFigure { figure: child })?;
        child_node
            .parent
            .ok_or(LayoutError::UnknownFigure { figure: child })
    }

    pub(super) fn validate_layout_constraint(
        &self,
        child: FigureId,
        constraint: &dyn LayoutConstraint,
    ) -> Result<(), LayoutError> {
        let parent = self.validate_layout_child(child)?;
        if let Some(manager) = self.layout_manager(parent) {
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

    pub fn layout_constraint<C>(&self, child_id: FigureId) -> Option<&C>
    where
        C: LayoutConstraint,
    {
        self.constraint(child_id)?.as_any().downcast_ref::<C>()
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
                if was_valid && let Some(lifecycle) = block.figure.lifecycle() {
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

impl crate::layout::LayoutContext for FigureTree {
    fn get_children(&self, parent_id: FigureId) -> Vec<(FigureId, Rectangle)> {
        if let Some(block) = self.blocks.get(parent_id) {
            block
                .children
                .iter()
                .filter_map(|&child_id| {
                    self.blocks
                        .get(child_id)
                        .map(|child| (child_id, child.figure_bounds()))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    fn get_constraint(&self, child_id: FigureId) -> Option<&dyn LayoutConstraint> {
        self.constraint(child_id)
    }

    fn preferred_measurement(
        &self,
        figure_id: FigureId,
        constraints: MeasureConstraints,
    ) -> FigureMeasurement {
        self.preferred_measurement(figure_id, constraints)
            .unwrap_or_default()
    }

    fn minimum_size(&self, figure_id: FigureId, constraints: MeasureConstraints) -> Dimension {
        self.minimum_size(figure_id, constraints)
            .unwrap_or(Dimension::ZERO)
    }

    fn maximum_size(&self, figure_id: FigureId) -> Dimension {
        self.maximum_size(figure_id)
            .unwrap_or(Dimension::new(f64::INFINITY, f64::INFINITY))
    }

    fn get_container_bounds(&self, container_id: FigureId) -> Rectangle {
        if let Some(block) = self.blocks.get(container_id) {
            block.child_layout_area()
        } else {
            Rectangle::ZERO
        }
    }

    fn get_freeform_extent(&self, figure_id: FigureId) -> Option<Rectangle> {
        self.freeform_extent(figure_id).ok()
    }

    fn get_content_scale(&self, figure_id: FigureId) -> Option<f64> {
        self.blocks
            .get(figure_id)
            .and_then(|block| block.figure.content_scale())
    }
}
