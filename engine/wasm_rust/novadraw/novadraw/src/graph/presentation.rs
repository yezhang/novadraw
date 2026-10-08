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
            let visible = self.is_effectively_visible(id);
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

    pub fn child_clipping_strategy(&self, figure_id: FigureId) -> Option<ChildClippingStrategy> {
        self.blocks
            .get(figure_id)
            .map(FigureNode::child_clipping_strategy)
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

    pub(crate) fn commit_point_list(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        parent_points: Vec<Point>,
    ) -> Result<bool, ShapeMutationError> {
        if parent_points
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(point_list) = block.point_list() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old_points = self
            .point_list_points(id)
            .expect("validated point-list capability");
        if old_points == parent_points {
            return Ok(false);
        }
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let (new_bounds, local_points) = normalize_points(
            parent_points.clone(),
            point_list.stroke_style(),
            point_list.painted_minimum(),
        );
        if !finite_rectangle(new_bounds) {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }

        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let block = self
            .blocks
            .get_mut(id)
            .ok_or(ShapeMutationError::UnknownFigure(id))?;
        block.set_node_bounds(new_bounds);
        block
            .point_list_mut()
            .ok_or(ShapeMutationError::WrongCapability(id))?
            .commit_geometry(new_bounds, local_points);

        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id: id,
            old_bounds,
            new_bounds,
        });
        self.emit_typed_property_event(id, property::POINTS, old_points, parent_points);
        self.mark_invalid(update_manager, id);
        self.mark_freeform_ancestor_extents_dirty(id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn set_point_list_stroke_style(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        stroke: crate::render::StrokeStyle,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(point_list) = block.point_list() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old_stroke = point_list.stroke_style().clone();
        let old_stroke_width = old_stroke.width();
        let old_line_join = old_stroke.join();
        let stroke_width = stroke.width();
        let line_join = stroke.join();
        if old_stroke == stroke {
            return Ok(false);
        }

        let parent_points = self
            .point_list_points(id)
            .expect("validated point-list capability");
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let (new_bounds, local_points) =
            normalize_points(parent_points, &stroke, point_list.painted_minimum());
        if !finite_rectangle(new_bounds) {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }

        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let block = self
            .blocks
            .get_mut(id)
            .ok_or(ShapeMutationError::UnknownFigure(id))?;
        block.set_node_bounds(new_bounds);
        let mut point_list = block
            .point_list_mut()
            .ok_or(ShapeMutationError::WrongCapability(id))?;
        point_list.commit_stroke_style(stroke.clone());
        point_list.commit_geometry(new_bounds, local_points);

        self.notify_block_changed(id);
        if old_bounds != new_bounds {
            self.emit_figure_event(FigureEvent::FigureMoved {
                figure_id: id,
                old_bounds,
                new_bounds,
            });
            self.mark_freeform_ancestor_extents_dirty(id);
        }
        if old_stroke_width != stroke_width {
            self.emit_typed_property_event(
                id,
                property::STROKE_WIDTH,
                old_stroke_width,
                stroke_width,
            );
        }
        if old_line_join != line_join {
            self.emit_typed_property_event(id, property::LINE_JOIN, old_line_join, line_join);
        }
        self.emit_typed_property_event(id, property::STROKE_STYLE, old_stroke, stroke);
        self.mark_invalid(update_manager, id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn is_scalable_polygon(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.scalable_polygon().is_some())
    }

    pub(crate) fn replace_scalable_polygon_template(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        template: PointList,
    ) -> Result<bool, ShapeMutationError> {
        if template
            .iter()
            .any(|point| !point.x().is_finite() || !point.y().is_finite())
        {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(scalable) = block.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        if scalable.template() == &template {
            return Ok(false);
        }
        let old_template = scalable.template().as_slice().to_vec();
        self.blocks
            .get_mut(id)
            .and_then(FigureNode::scalable_polygon_mut)
            .expect("validated scalable polygon capability")
            .replace_template(template.clone());
        self.notify_block_changed(id);
        self.emit_typed_property_event(
            id,
            property::POLYGON_TEMPLATE,
            old_template,
            template.as_slice().to_vec(),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_scalable_polygon_scale_mode(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        mode: crate::PolygonScaleMode,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(scalable) = block.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = scalable.scale_mode();
        if old == mode {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(FigureNode::scalable_polygon_mut)
            .expect("validated scalable polygon capability")
            .replace_scale_mode(mode);
        self.notify_block_changed(id);
        self.emit_typed_property_event(id, property::POLYGON_SCALE_MODE, old, mode);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_scalable_polygon_alignment(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        horizontal: crate::Alignment,
        vertical: crate::Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(scalable) = block.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = scalable.alignment();
        if old == (horizontal, vertical) {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(FigureNode::scalable_polygon_mut)
            .expect("validated scalable polygon capability")
            .replace_alignment(horizontal, vertical);
        self.notify_block_changed(id);
        self.emit_typed_property_event(
            id,
            property::POLYGON_ALIGNMENT,
            old,
            (horizontal, vertical),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn replace_text_flow_page(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        page: crate::FlowPage,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(flow) = block.text_flow() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        if flow.page() == &page {
            return Ok(false);
        }
        let old = flow.page().clone();
        self.blocks
            .get_mut(id)
            .and_then(FigureNode::text_flow_mut)
            .expect("validated TextFlow capability")
            .replace_page(page.clone());
        self.notify_block_changed(id);
        self.emit_typed_property_event(id, property::FLOW_PAGE, old, page);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_text_flow_wrapping(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        wrapping: crate::FlowWrapping,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(flow) = block.text_flow() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = flow.wrapping();
        if old == wrapping {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(FigureNode::text_flow_mut)
            .expect("validated TextFlow capability")
            .replace_wrapping(wrapping);
        self.notify_block_changed(id);
        self.emit_typed_property_event(id, property::FLOW_WRAPPING, old, wrapping);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn has_border_capability(&mut self, id: FigureId) -> bool {
        self.blocks
            .get_mut(id)
            .is_some_and(|block| block.bordered_mut().is_some())
    }

    pub(crate) fn is_rounded_rectangle(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| {
            block
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<RoundedRectangleFigure>()
                .is_some()
        })
    }

    pub(crate) fn is_triangle(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| {
            block
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<TriangleFigure>()
                .is_some()
        })
    }

    pub(crate) fn replace_border(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        border: Option<Arc<dyn Border>>,
    ) -> Result<bool, ShapeMutationError> {
        let metrics = border.as_deref().map(|border| {
            let insets = border.get_insets();
            let preferred = border.preferred_size();
            (insets, preferred)
        });
        if metrics.is_some_and(|(insets, preferred)| {
            [
                insets.top,
                insets.left,
                insets.bottom,
                insets.right,
                preferred.width,
                preferred.height,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        }) {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let (supports_border, same_border) =
            self.blocks
                .get_mut(id)
                .map_or((false, false), |block| match block.bordered() {
                    Some(bordered) => {
                        let same = match (bordered.border(), border.as_ref()) {
                            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
                            (None, None) => true,
                            _ => false,
                        };
                        (true, same)
                    }
                    None => (false, false),
                });
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        if same_border {
            return Ok(false);
        }
        if !supports_border {
            return Err(ShapeMutationError::WrongCapability(id));
        }
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let had_border = block.figure.get_border().is_some();

        if visible {
            self.erase(update_manager, id, old_visual_bounds, parent_id);
        }
        let has_border = {
            let block = self
                .blocks
                .get_mut(id)
                .ok_or(ShapeMutationError::UnknownFigure(id))?;
            block
                .bordered_mut()
                .ok_or(ShapeMutationError::WrongCapability(id))?
                .replace_border(border);
            block.border_snapshot = None;
            block.insets = block
                .figure
                .get_border()
                .map(Border::get_insets)
                .unwrap_or_default();
            block.figure.get_border().is_some()
        };

        self.record_property_change(id, property::BORDER, had_border, has_border);
        self.mark_invalid(update_manager, id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn set_corner_dimensions_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        dimensions: Dimension,
    ) -> Result<bool, ShapeMutationError> {
        if !dimensions.width.is_finite() || !dimensions.height.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if dimensions.width < 0.0 || dimensions.height < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(rounded) = block
            .figure
            .as_ref()
            .as_any()
            .downcast_ref::<RoundedRectangleFigure>()
        else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = rounded.corner_dimensions();
        if old == dimensions {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<RoundedRectangleFigure>()
            .expect("validated rounded rectangle capability")
            .set_corner_dimensions(dimensions);
        self.record_property_change(id, property::CORNER_DIMENSIONS, old, dimensions);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_triangle_direction_with_update(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        direction: Direction,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(triangle) = block
            .figure
            .as_ref()
            .as_any()
            .downcast_ref::<TriangleFigure>()
        else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = triangle.direction;
        if old == direction {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<TriangleFigure>()
            .expect("validated triangle capability")
            .set_direction(direction);
        self.record_property_change(id, property::DIRECTION, old, direction);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
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
                if self.is_effectively_visible(id) {
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
                if self.is_effectively_visible(id) {
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

    pub(crate) fn set_image_figure(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        image: crate::ImageId,
    ) -> Result<bool, ShapeMutationError> {
        let Some(previous_image) = self.image_figure(id).map(ImageFigure::image) else {
            return Err(if self.blocks.contains_key(id) {
                ShapeMutationError::WrongCapability(id)
            } else {
                ShapeMutationError::UnknownFigure(id)
            });
        };
        if previous_image == image {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<ImageFigure>()
            .expect("image type checked before mutation")
            .set_image(image);
        self.record_property_change(id, property::IMAGE, previous_image, image);
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_image_alignment(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        alignment: crate::Alignment,
    ) -> Result<bool, ShapeMutationError> {
        let Some(previous) = self.image_figure(id).map(ImageFigure::alignment) else {
            return Err(if self.blocks.contains_key(id) {
                ShapeMutationError::WrongCapability(id)
            } else {
                ShapeMutationError::UnknownFigure(id)
            });
        };
        if previous == alignment {
            return Ok(false);
        }
        self.blocks[id]
            .figure
            .as_mut()
            .as_any_mut()
            .downcast_mut::<ImageFigure>()
            .expect("image type checked before mutation")
            .set_alignment(alignment);
        self.record_property_change(id, property::IMAGE_ALIGNMENT, previous, alignment);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mutate_label<V>(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        property: PropertyKey<V>,
        old_value: V,
        new_value: V,
        mutate: impl FnOnce(&mut LabelFigure),
        revalidate: bool,
    ) -> Result<bool, ShapeMutationError>
    where
        V: PropertyValueType,
    {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        if block.label().is_none() {
            return Err(ShapeMutationError::WrongCapability(id));
        }
        if old_value == new_value {
            return Ok(false);
        }
        let label = self.blocks[id]
            .label_mut()
            .expect("label capability checked before mutation");
        mutate(label);
        self.record_property_change(id, property, old_value, new_value);
        if revalidate {
            self.mark_invalid(update_manager, id);
        }
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn activate_clickable(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
    ) -> bool {
        if !self.is_effectively_enabled(id) {
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

    pub(crate) fn set_clickable_selected(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        selected: bool,
    ) -> Result<bool, WidgetError> {
        let Some(block) = self.blocks.get_mut(id) else {
            return Err(WidgetError::UnknownFigure(id));
        };
        let Some(clickable) = block.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().is_selected();
        if !crate::figure::widget::set_selected(clickable.clickable_model_mut(), selected) {
            return Ok(false);
        }
        self.record_property_change(id, property::SELECTED, old, selected);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn set_clickable_rollover_enabled(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        enabled: bool,
    ) -> Result<bool, WidgetError> {
        let Some(block) = self.blocks.get_mut(id) else {
            return Err(WidgetError::UnknownFigure(id));
        };
        let Some(clickable) = block.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().rollover_enabled();
        if !crate::figure::widget::set_rollover_enabled(clickable.clickable_model_mut(), enabled) {
            return Ok(false);
        }
        self.record_property_change(id, property::ROLLOVER_ENABLED, old, enabled);
        self.repaint(update_manager, id, None);
        Ok(true)
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
