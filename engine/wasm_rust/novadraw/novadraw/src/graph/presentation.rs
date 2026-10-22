use super::*;
use crate::runtime::ResourceRegistry;
use crate::runtime::update::property::standard as property;

impl FigureTree {
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

    pub(crate) fn checked_prepared_scene(
        &self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, crate::connection::RouteSceneQueryError> {
        use crate::connection::{DependencySubject, RouteSceneQueryError, RouteSceneRead};
        let snapshot = self.source_checked_prepared_scene(owner)?;
        let geometries = std::collections::HashMap::new();
        let read = crate::connection::FigureTreeRouteSceneRead::new(self, &geometries);
        let mut pending = vec![snapshot.clone()];
        let mut visited = std::collections::HashSet::new();
        while let Some(value) = pending.pop() {
            if !visited.insert(value.identity()) {
                continue;
            }
            for observation in value.observations() {
                if read.dependency_generation(&observation.subject) != observation.generation {
                    return Err(RouteSceneQueryError::UnavailablePreparedScene(owner));
                }
                if let DependencySubject::PreparedScene(dependency) = observation.subject {
                    pending.push(self.source_checked_prepared_scene(dependency)?);
                }
            }
        }
        Ok(snapshot)
    }

    // Generation calculation must not recursively expand the computation graph.
    // Full authority checks are performed by checked_prepared_scene above.
    pub(crate) fn source_checked_prepared_scene(
        &self,
        owner: FigureId,
    ) -> Result<crate::figure::PreparedSceneSnapshot, crate::connection::RouteSceneQueryError> {
        use crate::connection::RouteSceneQueryError;
        self.validate_identity(owner)?;
        if !self.is_attached(owner)? {
            return Err(RouteSceneQueryError::DetachedFigure(owner));
        }
        let prepared = self.blocks[owner]
            .prepared
            .as_ref()
            .filter(|prepared| prepared.owner == owner)
            .ok_or(RouteSceneQueryError::UnavailablePreparedScene(owner))?;
        let (styles, _) = self.resolved_styles_for(|_| true);
        let styles = styles.into_iter().collect();
        let scene = crate::figure::PreparationScene::new(self, owner, &styles);
        if scene.source_revision().ok() != Some(prepared.source) {
            return Err(RouteSceneQueryError::UnavailablePreparedScene(owner));
        }
        Ok(prepared.snapshot())
    }

    pub(crate) fn refresh_prepared_figures(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        updates: &mut UpdateManager,
    ) -> Result<bool, TextError> {
        use crate::figure::PreparationScene;
        use crate::figure::preparation::{PreparationKey, PreparedFigure, scene_error};
        if !self
            .blocks
            .iter()
            .any(|(_, node)| node.preparation().is_some())
        {
            return Ok(false);
        }
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
        for id in figures {
            let style = &styles[&id];
            let node = &self.blocks[id];
            let preparation = node.preparation().expect("capability selected");
            let bounds = node.client_area();
            let scene = PreparationScene::new(self, id, &styles);
            let key = PreparationKey {
                component: scene.revision()?,
                provider: preparation.revision(),
                text: text.revision(),
                font: style.font.clone(),
                bounds,
            };
            let geometries = std::collections::HashMap::new();
            let read = crate::connection::FigureTreeRouteSceneRead::new(self, &geometries);
            if node.prepared.as_ref().is_some_and(|p| {
                use crate::connection::RouteSceneRead;
                p.key == key
                    && p.observations.iter().all(|observation| {
                        read.dependency_generation(&observation.subject) == observation.generation
                    })
            }) {
                continue;
            }
            let mut context = crate::text::MeasureContext::new(
                text,
                crate::text::FontDescriptor::parse(&style.font)?,
            );
            let source = scene.source_revision()?;
            let prepared_scene = preparation.prepare_scene(&mut context, bounds, &scene)?;
            prepared_scene.validate(&scene)?;
            if scene.revision()? != key.component || preparation.revision() != key.provider {
                return Err(TextError::InvalidPreparedScene(
                    "input revision changed during preparation".into(),
                ));
            }
            candidates.push((id, key, source, scene.observations(), prepared_scene));
        }
        let mut presented = std::collections::HashSet::new();
        for (id, key, _, _, candidate) in &candidates {
            for source in
                std::iter::once(id).chain(candidate.descendants.iter().map(|(source, _)| source))
            {
                if !presented.insert(*source) {
                    return Err(TextError::InvalidPreparedScene(
                        "overlapping presentation owners".into(),
                    ));
                }
            }
            if PreparationScene::new(self, *id, &styles).revision()? != key.component
                || text.revision() != key.text
            {
                return Err(TextError::InvalidPreparedScene(
                    "frozen preparation prefix is stale".into(),
                ));
            }
        }
        let mut metrics_changed = false;
        // No tree changes occur until every candidate and its layout output has been checked.
        let owners = candidates
            .iter()
            .map(|(id, _, _, _, _)| *id)
            .collect::<std::collections::HashSet<_>>();
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
            let unchanged = self.blocks[id].prepared.as_ref().filter(|old| {
                candidate.result_version.is_some()
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
                        key: key.clone(),
                        presentation,
                        identity,
                        source: input_source,
                        result_version: candidate.result_version,
                        observations: observations.clone(),
                        required_routes: Vec::new(),
                    },
                    updates,
                );
            }
            let changed = self.publish_prepared_figure(
                id,
                PreparedFigure {
                    owner: id,
                    key,
                    presentation: candidate.root,
                    identity,
                    source: input_source,
                    result_version: candidate.result_version,
                    observations,
                    required_routes: candidate.required_routes,
                },
                updates,
            );
            metrics_changed |= changed;
        }
        Ok(metrics_changed)
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
        let mut gc = NdCanvas::new();
        gc.damage_mut().set_full();
        self.render_to_with_presentation(&mut gc, presentation);
        gc
    }

    #[allow(dead_code)]
    pub(crate) fn render_to(&self, gc: &mut NdCanvas) {
        self.render_to_with_presentation(gc, None);
    }

    pub(crate) fn render_to_with_presentation(
        &self,
        gc: &mut NdCanvas,
        presentation: Option<&crate::animation::PresentationSnapshot>,
    ) {
        let start_id = self.contents.unwrap_or(self.root);
        let scene_ref = FigureTreeRenderRef {
            blocks: &self.blocks,
            presentation,
        };
        let mut renderer = FigureRenderer::new(&scene_ref, gc);
        renderer.render(start_id);
        if let Some(presentation) = presentation {
            presentation.paint_temporaries(gc);
        }
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
            let font = crate::render::FontDescriptor::parse(&style.font)?;
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

    pub(crate) fn refresh_text_flow_layouts(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        updates: &mut UpdateManager,
    ) -> Result<TextLayoutRefreshResult, TextError> {
        let flow_ids = self
            .blocks
            .iter()
            .filter_map(|(id, node)| node.text_flow().is_some().then_some(id))
            .collect::<Vec<_>>();
        if flow_ids.is_empty() {
            return Ok(TextLayoutRefreshResult {
                changed: Vec::new(),
                style_nodes_visited: 0,
                figures_refreshed: 0,
            });
        }
        let (flows, style_nodes_visited) =
            self.resolved_styles_for(|node| node.text_flow().is_some());
        let figures_refreshed = u64::try_from(flow_ids.len()).unwrap_or(u64::MAX);
        let mut changed = Vec::new();
        for (id, style) in flows {
            let font = crate::render::FontDescriptor::parse(&style.font)?;
            let bounds = self.blocks[id].client_area();
            let old_visual = self.blocks[id].visual_bounds();
            let parent = self.blocks[id].parent;
            let flow = self.blocks[id]
                .text_flow_mut()
                .expect("TextFlow type checked before refresh");
            if flow.refresh_layout(text, &font, bounds)? {
                if self.is_effectively_visible(id).expect("live text flow") {
                    self.erase(updates, id, old_visual, parent);
                }
                self.mark_freeform_ancestor_extents_dirty(id);
                changed.push(id);
            }
        }
        Ok(TextLayoutRefreshResult {
            changed,
            style_nodes_visited,
            figures_refreshed,
        })
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

    pub(crate) fn set_figure_style(&mut self, id: FigureId, mut style: FigureStyle) -> bool {
        let Some(old_style) = self.blocks.get(id).map(|block| block.style.clone()) else {
            return false;
        };
        style.alpha = style.alpha.map(|alpha| alpha.clamp(0.0, 1.0));
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
        let font_changed = self
            .blocks
            .get(id)
            .is_some_and(|block| block.style.font != style.font);
        if !self.set_figure_style(id, style) {
            return false;
        }

        let mut stack = vec![id];
        while let Some(node_id) = stack.pop() {
            let Some(node) = self.blocks.get(node_id) else {
                continue;
            };
            stack.extend(node.children.iter().copied());
            if font_changed {
                self.mark_invalid(update_manager, node_id);
            }
            self.repaint(update_manager, node_id, None);
        }
        true
    }

    fn emit_style_property_changes(&mut self, id: FigureId, old: &FigureStyle, new: &FigureStyle) {
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
