use super::*;
use crate::runtime::ResourceRegistry;
use crate::runtime::update::property::standard as property;

impl FigureTree {
    pub(crate) fn synchronize_preparation_text_revision(&mut self, revision: u64) {
        self.preparation_text_revision = revision;
    }

    pub(crate) fn invalidate_prepared_owners(&mut self, owners: &[FigureId]) {
        let ids = self.blocks.iter().map(|(id, _)| id).collect::<Vec<_>>();
        for id in ids {
            let node = &mut self.blocks[id];
            if let Some(prepared) = &mut node.prepared
                && owners.contains(&prepared.owner)
            {
                prepared.valid = false;
            }
        }
    }

    pub(crate) fn prepared_scene_generation(&self) -> u64 {
        self.prepared_scene_serial
    }

    pub(crate) fn required_route_groups(
        &self,
    ) -> Vec<(FigureId, &crate::figure::preparation::RequiredRouteGroup)> {
        self.blocks
            .iter()
            .flat_map(|(id, node)| {
                node.prepared
                    .as_ref()
                    .into_iter()
                    .filter(move |prepared| prepared.owner == id)
                    .flat_map(move |prepared| {
                        prepared
                            .required_routes
                            .iter()
                            .map(move |group| (id, group))
                    })
            })
            .collect()
    }

    pub(crate) fn checked_prepared_scene_with_budget(
        &self,
        owner: FigureId,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<crate::figure::PreparedSceneSnapshot, crate::connection::RouteSceneQueryError> {
        use crate::connection::{DependencySubject, RouteSceneQueryError, RouteSceneRead};
        let snapshot = self.source_checked_prepared_scene_with_budget(owner, budget)?;
        let geometries = std::collections::HashMap::new();
        let read = crate::connection::FigureTreeRouteSceneRead::with_budget(
            self,
            &geometries,
            budget.clone(),
        );
        let mut pending = vec![snapshot.clone()];
        let mut visited = std::collections::HashSet::new();
        while let Some(value) = pending.pop() {
            budget
                .allocate("checked snapshot traversal", 1)
                .map_err(RouteSceneQueryError::PreparationBudget)?;
            if !visited.insert(value.identity()) {
                continue;
            }
            for observation in value.observations() {
                budget
                    .consume("checked snapshot observation", 1)
                    .map_err(RouteSceneQueryError::PreparationBudget)?;
                if read.dependency_generation(&observation.subject) != observation.generation {
                    budget
                        .check()
                        .map_err(RouteSceneQueryError::PreparationBudget)?;
                    return Err(RouteSceneQueryError::UnavailablePreparedScene(owner));
                }
                if let DependencySubject::PreparedScene(dependency) = observation.subject {
                    budget
                        .allocate("checked snapshot traversal", 1)
                        .map_err(RouteSceneQueryError::PreparationBudget)?;
                    pending
                        .push(self.source_checked_prepared_scene_with_budget(dependency, budget)?);
                }
            }
        }
        Ok(snapshot)
    }

    // Generation calculation must not recursively expand the computation graph.
    // Full authority checks are performed by checked_prepared_scene above.
    pub(crate) fn source_checked_prepared_scene_with_budget(
        &self,
        owner: FigureId,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<crate::figure::PreparedSceneSnapshot, crate::connection::RouteSceneQueryError> {
        use crate::connection::RouteSceneQueryError;
        let map_error = |_| {
            budget.check().err().map_or(
                RouteSceneQueryError::UnavailablePreparedScene(owner),
                RouteSceneQueryError::PreparationBudget,
            )
        };
        self.validate_identity(owner)?;
        if !self.is_attached(owner)? {
            return Err(RouteSceneQueryError::DetachedFigure(owner));
        }
        let prepared = self.blocks[owner]
            .prepared
            .as_ref()
            .filter(|prepared| prepared.owner == owner && prepared.valid)
            .ok_or(RouteSceneQueryError::UnavailablePreparedScene(owner))?;
        budget
            .allocate("checked snapshot styles", self.blocks.len() as u64)
            .map_err(RouteSceneQueryError::PreparationBudget)?;
        let (styles, _) = self.resolved_styles_for(|_| true);
        let styles = styles.into_iter().collect();
        let scene =
            crate::figure::PreparationScene::with_budget(self, owner, &styles, budget.clone())
                .map_err(map_error)?;
        if scene.source_prefix().map_err(map_error)? != prepared.source
            || prepared.key.text != self.preparation_text_revision
        {
            return Err(RouteSceneQueryError::UnavailablePreparedScene(owner));
        }
        scene
            .retain_input_proofs(&prepared.input_proofs)
            .and_then(|()| scene.validate_input_proofs("checked prepared snapshot"))
            .map_err(|error| RouteSceneQueryError::PreparationInput(Box::new(error)))?;
        budget
            .allocate("checked snapshot clone", 1)
            .map_err(RouteSceneQueryError::PreparationBudget)?;
        Ok(prepared.snapshot())
    }

    pub(crate) fn refresh_prepared_figures(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        updates: &mut UpdateManager,
        mut dependencies: crate::figure::preparation::PreparationDependencies,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<bool, TextError> {
        let _refresh_timing = budget.time("prepare-refresh");
        let inputs_timing = budget.time("prepare-inputs");
        use crate::figure::preparation::{PreparationKey, PreparedFigure, scene_error};
        self.synchronize_preparation_text_revision(text.revision());
        if !self
            .capability_ids(PREPARATION)
            .any(|id| self.blocks[id].preparation().is_some())
        {
            return Ok(false);
        }
        // Reserve collection-entry accounting before building the batch's style/list/map views.
        budget.allocate("preparation batch inputs", self.blocks.len() as u64 * 3)?;
        let (styles, _) = self.resolved_styles_for(|_| true);
        let figures = styles
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| self.blocks[*id].preparation().is_some())
            .collect::<Vec<_>>();
        let styles = styles
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        let mut candidates = Vec::new();
        let inputs =
            crate::figure::preparation::PreparationInputs::new(self, &styles, budget.clone())?;
        let mut batch_input_proofs = Vec::new();
        drop(inputs_timing);
        for id in figures {
            let signature_timing = budget.time("prepare-signature");
            let style = &styles[&id];
            let node = &self.blocks[id];
            let preparation = node.preparation().expect("capability selected");
            let bounds = node.client_area();
            let scene = inputs.scene(id)?;
            let source = scene.source_prefix()?;
            let key = PreparationKey {
                component: scene.revision()?,
                provider: preparation.revision(),
                text: text.revision(),
                font: style.font.clone(),
                bounds,
            };
            let geometries = std::collections::HashMap::new();
            let read = crate::connection::FigureTreeRouteSceneRead::with_budget(
                self,
                &geometries,
                budget.clone(),
            );
            let reusable = node.prepared.as_ref().is_some_and(|p| {
                use crate::connection::RouteSceneRead;
                p.valid
                    && p.key == key
                    && p.source == source
                    && p.observations.iter().all(|observation| {
                        read.dependency_generation(&observation.subject) == observation.generation
                    })
            });
            if reusable {
                let mut unchanged = true;
                for proof in node
                    .prepared
                    .as_ref()
                    .expect("reusable preparation")
                    .input_proofs
                    .iter()
                {
                    match proof.validate(&scene, "preparation cache") {
                        Ok(()) => (),
                        Err(TextError::PreparationInput(error))
                            if matches!(
                                error.kind,
                                crate::figure::PreparationInputErrorKind::Changed { .. }
                            ) =>
                        {
                            unchanged = false;
                            break;
                        }
                        Err(error) => return Err(error),
                    }
                }
                if unchanged {
                    crate::figure::preparation::merge_proofs(
                        &mut batch_input_proofs,
                        &node
                            .prepared
                            .as_ref()
                            .expect("reusable preparation")
                            .input_proofs,
                        budget,
                    )?;
                    continue;
                }
            }
            drop(signature_timing);
            budget.check()?;
            let callback_timing = budget.time("prepare-callback");
            let mut context = crate::text::MeasureContext::new(text, style.font.clone());
            let mut prepared_scene = preparation.prepare_scene(&mut context, bounds, &scene)?;
            drop(callback_timing);
            let _validation_timing = budget.time("prepare-candidate-validation");
            prepared_scene.validate(&scene)?;
            prepared_scene.input_proofs = scene.input_proofs()?;
            crate::figure::preparation::merge_proofs(
                &mut batch_input_proofs,
                &prepared_scene.input_proofs,
                budget,
            )?;
            if scene.revision()? != key.component || preparation.revision() != key.provider {
                return Err(TextError::InvalidPreparedScene(
                    "input revision changed during preparation".into(),
                ));
            }
            budget.allocate("preparation candidates", 1)?;
            candidates.push((id, key, source, scene.observations(), prepared_scene));
        }
        let batch_timing = budget.time("prepare-batch-validation");
        let mut presented = std::collections::HashSet::new();
        for (id, key, source, observations, candidate) in &candidates {
            for source in
                std::iter::once(id).chain(candidate.descendants.iter().map(|(source, _)| source))
            {
                budget.allocate("presentation membership", 1)?;
                if !presented.insert(*source) {
                    return Err(TextError::InvalidPreparedScene(
                        "overlapping presentation owners".into(),
                    ));
                }
            }
            let scene = inputs.scene(*id)?;
            if scene.revision()? != key.component
                || scene.source_prefix()? != *source
                || text.revision() != key.text
            {
                return Err(TextError::InvalidPreparedScene(
                    "frozen preparation prefix is stale".into(),
                ));
            }
            use crate::connection::RouteSceneRead;
            let geometries = std::collections::HashMap::new();
            let read = crate::connection::FigureTreeRouteSceneRead::with_budget(
                self,
                &geometries,
                budget.clone(),
            );
            if observations.iter().any(|observation| {
                read.dependency_generation(&observation.subject) != observation.generation
            }) {
                budget.check()?;
                return Err(TextError::InvalidPreparedScene(
                    "frozen preparation dependency is stale".into(),
                ));
            }
        }
        budget.allocate("retained dependency owners", self.blocks.len() as u64)?;
        dependencies.retain_tree(
            self,
            &self.blocks.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            budget,
        )?;
        for (id, _, _, observations, _) in &candidates {
            dependencies.replace(
                crate::connection::DependencySubject::PreparedScene(*id),
                observations
                    .iter()
                    .map(|observation| observation.subject.clone()),
                budget,
            )?;
        }
        budget.allocate("candidate owner index", candidates.len() as u64)?;
        let candidate_by_owner = candidates
            .iter()
            .map(|(id, _, _, _, candidate)| (*id, candidate))
            .collect::<std::collections::HashMap<_, _>>();
        for (id, node) in self.blocks.iter() {
            let required = if let Some(candidate) = candidate_by_owner.get(&id) {
                &candidate.required_routes
            } else if let Some(prepared) = &node.prepared
                && prepared.owner == id
            {
                &prepared.required_routes
            } else {
                continue;
            };
            dependencies.validate(
                self,
                id,
                required
                    .iter()
                    .flat_map(|group| group.connections.iter().copied()),
                budget,
            )?;
        }
        budget.check()?;
        // All extension-capable candidate and dependency checks have finished.
        // Recheck the first values immediately before releasing input authority.
        if !batch_input_proofs.is_empty() {
            let scene = inputs.scene(self.root_id())?;
            scene.retain_input_proofs(&batch_input_proofs)?;
            scene.validate_input_proofs("preparation batch publication")?;
        }
        drop(batch_timing);
        // Release immutable input authority before publishing any layout effects.
        drop(inputs);
        let _publication_timing = budget.time("prepare-publication");
        let mut metrics_changed = false;
        // No tree changes occur until every candidate and its layout output has been checked.
        budget.allocate("publication owner index", candidates.len() as u64)?;
        let owners = candidates
            .iter()
            .map(|(id, _, _, _, _)| *id)
            .collect::<std::collections::HashSet<_>>();
        budget.allocate("retired presentation scan", self.blocks.len() as u64)?;
        let retired = self
            .blocks
            .iter()
            .filter_map(|(source, node)| {
                node.prepared
                    .as_ref()
                    .filter(|prepared| {
                        owners.contains(&prepared.owner) && !presented.contains(&source)
                    })
                    .map(|_| source)
            })
            .collect::<Vec<_>>();
        for (_, _, _, observations, candidate) in &candidates {
            // Source inputs were allocated once while freezing the prefix. Each published
            // descendant shares that immutable prefix and the owner's observation slice.
            budget.allocate(
                "published preparation snapshots",
                (observations.len() as u64).saturating_add(candidate.descendants.len() as u64 + 1),
            )?;
        }
        for source in retired {
            let node = &self.blocks[source];
            let old_visual = node.visual_bounds();
            let parent = node.parent;
            let visible = self
                .is_effectively_visible(source)
                .expect("retired live source");
            if visible {
                self.erase(updates, source, old_visual, parent);
            }
            self.blocks[source].prepared = None;
            self.mark_invalid(updates, source);
            if visible {
                self.repaint(updates, source, None);
            }
            metrics_changed = true;
        }
        for (id, key, input_source, observations, candidate) in candidates {
            let observations: std::sync::Arc<[_]> = observations.into();
            let input_proofs: std::sync::Arc<[_]> = candidate.input_proofs.into();
            let unchanged = self.blocks[id].prepared.as_ref().filter(|old| {
                old.valid
                    && candidate.result_version.is_some()
                    && old.result_version == candidate.result_version
                    && old.source == input_source
                    && old.key.text == key.text
                    && old.key.font == key.font
                    && old.key.bounds == key.bounds
                    && old.observations.len() == observations.len()
                    && old
                        .observations
                        .iter()
                        .all(|observation| observations.contains(observation))
            });
            let identity = if let Some(old) = unchanged {
                old.identity
            } else {
                self.prepared_scene_serial =
                    self.prepared_scene_serial.checked_add(1).ok_or_else(|| {
                        TextError::InvalidPreparedScene("prepared scene identity exhausted".into())
                    })?;
                crate::figure::PreparedSceneIdentity::new(id, self.prepared_scene_serial)
            };
            for (parent, output) in candidate.layouts {
                self.apply_layout_output(updates, parent, output)
                    .map_err(scene_error)?;
            }
            for (source, presentation) in candidate.descendants {
                metrics_changed |= self.publish_prepared_figure(
                    source,
                    PreparedFigure {
                        owner: id,
                        valid: true,
                        key: key.clone(),
                        presentation,
                        identity,
                        source: input_source.clone(),
                        result_version: candidate.result_version,
                        observations: observations.clone(),
                        input_proofs: input_proofs.clone(),
                        required_routes: Vec::new(),
                    },
                    updates,
                );
            }
            let changed = self.publish_prepared_figure(
                id,
                PreparedFigure {
                    owner: id,
                    valid: true,
                    key,
                    presentation: candidate.root,
                    identity,
                    source: input_source,
                    result_version: candidate.result_version,
                    observations,
                    input_proofs,
                    required_routes: candidate.required_routes,
                },
                updates,
            );
            metrics_changed |= changed;
        }
        Ok(metrics_changed)
    }

    /// Reusing a layout requires its original inputs, even when refreshed
    /// presentations report identical dimensions. This runs before callbacks in
    /// a new stabilization; changes during that stabilization still fail the
    /// publication check rather than being rebased.
    pub(crate) fn invalidate_stale_layout_inputs(
        &mut self,
        updates: &mut UpdateManager,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<(), TextError> {
        if !self.blocks.iter().any(|(id, node)| {
            node.is_valid
                && !node.layout.input_proofs.is_empty()
                && self.is_effectively_visible(id) == Ok(true)
        }) {
            return Ok(());
        }
        let inputs = crate::figure::preparation::PreparationInputs::lazy(self, budget.clone())?;
        let scene = inputs.scene(self.root_id())?;
        let mut stale = Vec::new();
        for (id, node) in self.blocks.iter() {
            if !node.is_valid
                || node.layout.input_proofs.is_empty()
                || !scene.contains_frozen_member(id)?
                || self.is_effectively_visible(id) != Ok(true)
            {
                continue;
            }
            for proof in &node.layout.input_proofs {
                match proof.validate(&scene, "layout cache") {
                    Ok(()) => (),
                    Err(TextError::PreparationInput(error))
                        if matches!(
                            error.kind,
                            crate::figure::PreparationInputErrorKind::Changed { .. }
                        ) =>
                    {
                        budget.allocate("stale layout input owner", 1)?;
                        stale.push(id);
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        budget.check()?;
        drop(scene);
        drop(inputs);
        for id in stale {
            self.mark_invalid(updates, id);
        }
        Ok(())
    }

    pub(crate) fn validate_preparation_inputs(
        &self,
        budget: &crate::figure::PreparationBudgetHandle,
    ) -> Result<(), TextError> {
        if !self.blocks.iter().any(|(_, node)| {
            !node.layout.input_proofs.is_empty()
                || node
                    .prepared
                    .as_ref()
                    .is_some_and(|p| !p.input_proofs.is_empty())
        }) {
            return Ok(());
        }
        let inputs = crate::figure::preparation::PreparationInputs::lazy(self, budget.clone())?;
        let scene = inputs.scene(self.root_id())?;
        for (id, node) in self.blocks.iter() {
            let owns_prepared_inputs = node
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.owner == id && !prepared.input_proofs.is_empty());
            if node.layout.input_proofs.is_empty() && !owns_prepared_inputs {
                continue;
            }
            if !scene.contains_frozen_member(id)? {
                continue;
            }
            // Hidden placements are not consumed by this publication. If a
            // visible parent measures a hidden child, its own query retains
            // those proofs independently. Showing the owner checks its cache
            // inputs before any visible layout can reuse the placements.
            if node.is_valid && self.is_effectively_visible(id) == Ok(true) {
                scene.retain_input_proofs(&node.layout.input_proofs)?;
            }
            if let Some(prepared) = &node.prepared
                && prepared.owner == id
            {
                scene.retain_input_proofs(&prepared.input_proofs)?;
            }
        }
        scene.validate_input_proofs("runtime publication")
    }

    fn publish_prepared_figure(
        &mut self,
        id: FigureId,
        candidate: crate::figure::preparation::PreparedFigure,
        updates: &mut UpdateManager,
    ) -> bool {
        let node = &self.blocks[id];
        let changed = node.prepared.as_ref().is_none_or(|old| {
            old.presentation.measurement() != candidate.presentation.measurement()
                || old.presentation.minimum_size() != candidate.presentation.minimum_size()
        });
        let old_visual = node.visual_bounds();
        let parent = node.parent;
        let visible = self.is_effectively_visible(id).expect("prepared live node");
        if visible {
            self.erase(updates, id, old_visual, parent);
        }
        self.blocks[id].prepared = Some(candidate);
        if changed {
            self.mark_invalid(updates, id);
        }
        self.mark_freeform_ancestor_extents_dirty(id);
        if visible {
            self.repaint(updates, id, None);
        }
        changed
    }

    pub(crate) fn set_child_clipping_strategy(
        &mut self,
        figure_id: FigureId,
        strategy: ChildClippingStrategy,
    ) -> bool {
        let Some(block) = self.blocks.get_mut(figure_id) else {
            return false;
        };
        if block.child_clipping_strategy() == strategy {
            return false;
        }
        block.state.child_clipping_strategy = Some(strategy);
        self.notify_block_changed(figure_id);
        true
    }

    pub fn child_clipping_strategy(
        &self,
        figure_id: FigureId,
    ) -> Result<ChildClippingStrategy, TreeQueryError> {
        Ok(self.node(figure_id)?.child_clipping_strategy())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn render(&self) -> NdCanvas {
        self.render_with_presentation(None)
    }

    pub(crate) fn render_with_presentation(
        &self,
        presentation: Option<&crate::animation::PresentationSnapshot>,
    ) -> NdCanvas {
        self.render_with_paint_coverage(presentation, None)
    }

    pub(crate) fn render_with_paint_coverage(
        &self,
        presentation: Option<&crate::animation::PresentationSnapshot>,
        coverage_outset: Option<f64>,
    ) -> NdCanvas {
        self.try_render_with_paint_coverage(presentation, coverage_outset)
            .expect("test recording requires valid projection inputs")
    }

    pub(crate) fn try_render_with_paint_coverage(
        &self,
        presentation: Option<&crate::animation::PresentationSnapshot>,
        coverage_outset: Option<f64>,
    ) -> Result<NdCanvas, CoordinateQueryError> {
        let mut gc = NdCanvas::new();
        gc.set_coverage_outset(coverage_outset);
        gc.damage_mut().set_full();
        self.try_render_to_with_presentation(&mut gc, presentation)?;
        Ok(gc)
    }

    #[allow(dead_code)]
    pub(crate) fn render_to(&self, gc: &mut NdCanvas) {
        self.render_to_with_presentation(gc, None);
    }

    pub(crate) fn capture_to(
        &self,
        gc: &mut NdCanvas,
        root: FigureId,
        rect: Rectangle,
        style: &crate::ResolvedStyle,
        font: crate::text::FontDescriptor,
    ) -> Result<(), CoordinateQueryError> {
        let projection = self.begin_projection_scope();
        let snapshot = crate::animation::PresentationSnapshot::for_recording(self, root, None)?;
        let scene = FigureTreeRenderRef {
            blocks: &self.blocks,
            presentation: Some(&snapshot),
        };
        FigureRenderer::new(&scene, gc).capture(root, rect, style, font);
        projection.finish(self, "scene capture return")
    }

    pub(crate) fn render_to_with_presentation(
        &self,
        gc: &mut NdCanvas,
        presentation: Option<&crate::animation::PresentationSnapshot>,
    ) {
        self.try_render_to_with_presentation(gc, presentation)
            .expect("test recording requires valid projection inputs");
    }

    pub(crate) fn try_render_to_with_presentation(
        &self,
        gc: &mut NdCanvas,
        presentation: Option<&crate::animation::PresentationSnapshot>,
    ) -> Result<(), CoordinateQueryError> {
        let projection = self.begin_projection_scope();
        let start_id = self.contents.unwrap_or(self.root);
        let snapshot =
            crate::animation::PresentationSnapshot::for_recording(self, start_id, presentation)?;
        let scene_ref = FigureTreeRenderRef {
            blocks: &self.blocks,
            presentation: Some(&snapshot),
        };
        let mut renderer = FigureRenderer::new(&scene_ref, gc);
        renderer.render(start_id);
        if let Some(presentation) = presentation {
            presentation.paint_temporaries(gc);
        }
        projection.finish(self, "recording return")
    }

    pub(crate) fn refresh_label_intrinsic_layouts(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        resources: &ResourceRegistry,
    ) -> Result<TextLayoutRefreshResult, TextError> {
        let label_ids = self
            .blocks
            .iter()
            .filter_map(|(id, node)| node.label().is_some().then_some(id))
            .collect::<Vec<_>>();
        if label_ids.is_empty() {
            return Ok(TextLayoutRefreshResult {
                changed: Vec::new(),
                style_nodes_visited: 0,
                figures_refreshed: 0,
            });
        }
        let (labels, style_nodes_visited) = self.resolved_styles_for(|node| node.label().is_some());
        let figures_refreshed = u64::try_from(label_ids.len()).unwrap_or(u64::MAX);
        let mut changed = Vec::new();
        for (id, style) in labels {
            let font = style.font.clone();
            let label = self.blocks[id]
                .label_mut()
                .expect("label capability checked before mutable borrow");
            let icon = label.icon().and_then(|id| resources.image_ref(id));
            if label.refresh_intrinsic(text, &font, icon)? {
                changed.push(id);
            }
        }
        Ok(TextLayoutRefreshResult {
            changed,
            style_nodes_visited,
            figures_refreshed,
        })
    }

    pub(crate) fn refresh_label_presentations(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        updates: &mut UpdateManager,
    ) -> Result<Vec<FigureId>, TextError> {
        let labels = self
            .blocks
            .iter()
            .filter_map(|(id, block)| block.label().is_some().then_some(id))
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for id in labels {
            let bounds = self.blocks[id].client_area();
            let old_visual = self.blocks[id].visual_bounds();
            let parent = self.blocks[id].parent;
            let label = self.blocks[id]
                .label_mut()
                .expect("label capability checked before mutable borrow");
            if label.refresh_presentation(text, bounds)? {
                if self.is_effectively_visible(id).expect("live label") {
                    self.erase(updates, id, old_visual, parent);
                }
                self.mark_freeform_ancestor_extents_dirty(id);
                changed.push(id);
            }
        }
        Ok(changed)
    }

    pub(crate) fn refresh_image_figures(&mut self, resources: &ResourceRegistry) -> Vec<FigureId> {
        let images = self
            .blocks
            .iter()
            .filter_map(|(id, block)| {
                block
                    .figure
                    .as_ref()
                    .as_any()
                    .is::<ImageFigure>()
                    .then_some(id)
            })
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for id in images {
            let image = self.blocks[id]
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<ImageFigure>()
                .expect("image type checked before refresh")
                .image();
            let status = resources.status(image.resource_id()).ok();
            let image_ref = resources.image_ref(image);
            let figure = self.blocks[id]
                .figure
                .as_mut()
                .as_any_mut()
                .downcast_mut::<ImageFigure>()
                .expect("image type checked before refresh");
            if figure.refresh(status, image_ref) {
                changed.push(id);
            }
        }
        changed
    }

    pub(crate) fn activate_clickable(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
    ) -> bool {
        if self.is_effectively_enabled(id) != Ok(true) {
            return false;
        }
        let Some(clickable) = self.blocks.get_mut(id).and_then(FigureNode::clickable_mut) else {
            return false;
        };
        let (selection_change, revision) =
            crate::figure::widget::activate(clickable.clickable_model_mut());
        self.notify_block_changed(id);
        if let Some((old, new)) = selection_change {
            self.emit_typed_property_event(id, property::SELECTED, old, new);
        }
        self.notification_effects.emit_action(ActionEvent {
            figure_id: id,
            revision,
        });
        self.repaint(update_manager, id, None);
        true
    }

    pub(crate) fn sync_clickable_visual(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        visual: ClickableVisualState,
    ) -> bool {
        let Some(clickable) = self.blocks.get_mut(id).and_then(FigureNode::clickable_mut) else {
            return false;
        };
        if !crate::figure::widget::sync_visual(clickable.clickable_model_mut(), visual) {
            return false;
        }
        self.repaint(update_manager, id, None);
        true
    }

    pub(crate) fn set_border_snapshot(&mut self, id: FigureId, snapshot: BorderSnapshot) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.border_snapshot.as_ref() == Some(&snapshot) {
            return false;
        }
        block.insets = snapshot.insets();
        block.border_snapshot = Some(snapshot);
        true
    }

    pub(crate) fn set_opaque(&mut self, id: FigureId, opaque: bool) -> bool {
        let Some(block) = self.blocks.get_mut(id) else {
            return false;
        };
        if block.is_opaque == opaque {
            return false;
        }
        block.is_opaque = opaque;
        self.notify_block_changed(id);
        true
    }

    pub(crate) fn set_figure_style(&mut self, id: FigureId, style: FigureStyle) -> bool {
        let Some(old_style) = self.blocks.get(id).map(|block| block.style.clone()) else {
            return false;
        };
        if old_style == style {
            return false;
        }
        self.blocks[id].style = style.clone();
        self.notify_block_changed(id);
        self.emit_style_property_changes(id, &old_style, &style);
        true
    }

    pub(crate) fn set_figure_style_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        style: FigureStyle,
    ) -> bool {
        let layout_style_changed = self.blocks.get(id).is_some_and(|block| {
            block.style.font != style.font || block.style.layout_direction != style.layout_direction
        });
        if !self.set_figure_style(id, style) {
            return false;
        }

        let mut stack = vec![id];
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.blocks.get(node_id) else {
                continue;
            };
            stack.extend(node.children.iter().copied());
            if layout_style_changed {
                self.mark_invalid(update_manager, node_id);
            }
            self.repaint(update_manager, node_id, None);
        }
        true
    }

    fn emit_style_property_changes(&mut self, id: FigureId, old: &FigureStyle, new: &FigureStyle) {
        if old.layout_direction != new.layout_direction {
            self.record_property_change(
                id,
                property::LAYOUT_DIRECTION,
                old.layout_direction,
                new.layout_direction,
            );
        }
        if old.foreground != new.foreground {
            self.record_property_change(id, property::FOREGROUND, old.foreground, new.foreground);
        }
        if old.background != new.background {
            self.record_property_change(id, property::BACKGROUND, old.background, new.background);
        }
        if old.alpha != new.alpha {
            self.record_property_change(id, property::ALPHA, old.alpha, new.alpha);
        }
        if old.font != new.font {
            self.record_property_change(id, property::FONT, old.font.clone(), new.font.clone());
        }
        if old.cursor != new.cursor {
            self.record_property_change(id, property::CURSOR, old.cursor, new.cursor);
        }
        if old.tooltip != new.tooltip {
            self.record_property_change(
                id,
                property::TOOLTIP,
                old.tooltip.clone().flatten(),
                new.tooltip.clone().flatten(),
            );
        }
    }
}
