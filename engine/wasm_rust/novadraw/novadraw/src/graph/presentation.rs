use super::*;
use crate::runtime::ResourceRegistry;
use crate::runtime::update::property::standard as property;

impl FigureTree {
    pub(crate) fn refresh_prepared_figures(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        updates: &mut UpdateManager,
    ) -> Result<bool, TextError> {
        use crate::figure::preparation::{PreparationKey, PreparedFigure};
        let (figures, _) = self.resolved_styles_for(|node| node.preparation().is_some());
        let mut candidates = Vec::new();
        for (id, style) in figures {
            let node = &self.blocks[id];
            let preparation = node.preparation().expect("capability selected");
            let bounds = node.client_area();
            let key = PreparationKey {
                component: node.component_revision,
                provider: preparation.revision(),
                text: text.revision(),
                font: style.font.clone(),
                bounds,
            };
            if node.prepared.as_ref().is_some_and(|p| p.key == key) {
                continue;
            }
            let mut context = crate::text::MeasureContext::new(
                text,
                crate::text::FontDescriptor::parse(&style.font)?,
            );
            let presentation = preparation.prepare(&mut context, bounds)?;
            candidates.push((id, PreparedFigure { key, presentation }));
        }
        let mut metrics_changed = false;
        for (id, candidate) in candidates {
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
                metrics_changed = true;
                self.mark_invalid(updates, id);
            }
            self.mark_freeform_ancestor_extents_dirty(id);
            if visible {
                self.repaint(updates, id, None);
            }
        }
        Ok(metrics_changed)
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
