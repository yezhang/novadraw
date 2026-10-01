use super::*;

impl FigureTree {
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

    pub(crate) fn render(&self) -> NdCanvas {
        let mut gc = NdCanvas::new();
        gc.damage_mut().set_full();
        self.render_to(&mut gc);
        gc
    }

    pub(crate) fn render_to(&self, gc: &mut NdCanvas) {
        let start_id = self.contents.unwrap_or(self.root);
        let scene_ref = FigureTreeRenderRef {
            blocks: &self.blocks,
        };
        let mut renderer = FigureRenderer::new(&scene_ref, gc);
        renderer.render(start_id);
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
        let Some(point_list) = block.figure.point_list() else {
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
            point_list.stroke_width(),
            point_list.line_join(),
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
            .figure
            .point_list_mut()
            .ok_or(ShapeMutationError::WrongCapability(id))?
            .commit_geometry(new_bounds, local_points);

        self.notify_block_changed(id);
        self.emit_figure_event(FigureEvent::FigureMoved {
            figure_id: id,
            old_bounds,
            new_bounds,
        });
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "points",
            old_value: PropertyValue::PointList(old_points),
            new_value: PropertyValue::PointList(parent_points),
        });
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
        stroke_width: f64,
        line_join: crate::render::command::LineJoin,
    ) -> Result<bool, ShapeMutationError> {
        if !stroke_width.is_finite() {
            return Err(ShapeMutationError::NonFiniteGeometry);
        }
        if stroke_width < 0.0 {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        let Some(point_list) = block.figure.point_list() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old_stroke_width = point_list.stroke_width();
        let old_line_join = point_list.line_join();
        if old_stroke_width == stroke_width && old_line_join == line_join {
            return Ok(false);
        }

        let parent_points = self
            .point_list_points(id)
            .expect("validated point-list capability");
        let old_bounds = block.figure_bounds();
        let old_visual_bounds = block.visual_bounds();
        let parent_id = block.parent;
        let visible = self.is_effectively_visible(id);
        let (new_bounds, local_points) = normalize_points(
            parent_points,
            stroke_width,
            line_join,
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
        let point_list = block
            .figure
            .point_list_mut()
            .ok_or(ShapeMutationError::WrongCapability(id))?;
        point_list.commit_stroke_style(stroke_width, line_join);
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
            self.emit_property_event(PropertyChangeEvent {
                figure_id: id,
                property: "stroke_width",
                old_value: PropertyValue::Number(old_stroke_width),
                new_value: PropertyValue::Number(stroke_width),
            });
        }
        if old_line_join != line_join {
            self.emit_property_event(PropertyChangeEvent {
                figure_id: id,
                property: "line_join",
                old_value: PropertyValue::Text(format!("{old_line_join:?}")),
                new_value: PropertyValue::Text(format!("{line_join:?}")),
            });
        }
        self.mark_invalid(update_manager, id);
        if visible {
            self.repaint(update_manager, id, None);
        }
        Ok(true)
    }

    pub(crate) fn has_scalable_polygon_capability(&self, id: FigureId) -> bool {
        self.blocks
            .get(id)
            .is_some_and(|block| block.figure.scalable_polygon().is_some())
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
        let Some(scalable) = block.figure.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        if scalable.template() == &template {
            return Ok(false);
        }
        let old_template = scalable.template().as_slice().to_vec();
        self.blocks
            .get_mut(id)
            .and_then(|block| block.figure.scalable_polygon_mut())
            .expect("validated scalable polygon capability")
            .replace_template(template.clone());
        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "polygon_template",
            old_value: PropertyValue::PointList(old_template),
            new_value: PropertyValue::PointList(template.as_slice().to_vec()),
        });
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
        let Some(scalable) = block.figure.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = scalable.scale_mode();
        if old == mode {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(|block| block.figure.scalable_polygon_mut())
            .expect("validated scalable polygon capability")
            .replace_scale_mode(mode);
        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "polygon_scale_mode",
            old_value: PropertyValue::Text(format!("{old:?}")),
            new_value: PropertyValue::Text(format!("{mode:?}")),
        });
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
        let Some(scalable) = block.figure.scalable_polygon() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = scalable.alignment();
        if old == (horizontal, vertical) {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(|block| block.figure.scalable_polygon_mut())
            .expect("validated scalable polygon capability")
            .replace_alignment(horizontal, vertical);
        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "polygon_alignment",
            old_value: PropertyValue::Text(format!("{old:?}")),
            new_value: PropertyValue::Text(format!("{:?}", (horizontal, vertical))),
        });
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
        let Some(flow) = block.figure.text_flow() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        if flow.page() == &page {
            return Ok(false);
        }
        let old = format!("{:?}", flow.page());
        self.blocks
            .get_mut(id)
            .and_then(|block| block.figure.text_flow_mut())
            .expect("validated TextFlow capability")
            .replace_page(page.clone());
        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "flow_page",
            old_value: PropertyValue::Text(old),
            new_value: PropertyValue::Text(format!("{page:?}")),
        });
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
        let Some(flow) = block.figure.text_flow() else {
            return Err(ShapeMutationError::WrongCapability(id));
        };
        let old = flow.wrapping();
        if old == wrapping {
            return Ok(false);
        }
        self.blocks
            .get_mut(id)
            .and_then(|block| block.figure.text_flow_mut())
            .expect("validated TextFlow capability")
            .replace_wrapping(wrapping);
        self.notify_block_changed(id);
        self.emit_property_event(PropertyChangeEvent {
            figure_id: id,
            property: "flow_wrapping",
            old_value: PropertyValue::Text(format!("{old:?}")),
            new_value: PropertyValue::Text(format!("{wrapping:?}")),
        });
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn has_border_capability(&mut self, id: FigureId) -> bool {
        self.blocks
            .get_mut(id)
            .is_some_and(|block| block.figure.bordered_mut().is_some())
    }

    pub(crate) fn has_rounded_rectangle_capability(&self, id: FigureId) -> bool {
        self.blocks.get(id).is_some_and(|block| {
            block
                .figure
                .as_ref()
                .as_any()
                .downcast_ref::<RoundedRectangleFigure>()
                .is_some()
        })
    }

    pub(crate) fn has_triangle_capability(&self, id: FigureId) -> bool {
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
                insets.0,
                insets.1,
                insets.2,
                insets.3,
                preferred.0,
                preferred.1,
            ]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        }) {
            return Err(ShapeMutationError::NegativeMetric);
        }
        let (supports_border, same_border) =
            self.blocks.get_mut(id).map_or((false, false), |block| {
                match block.figure.bordered_mut() {
                    Some(bordered) => {
                        let same = match (bordered.border(), border.as_ref()) {
                            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
                            (None, None) => true,
                            _ => false,
                        };
                        (true, same)
                    }
                    None => (false, false),
                }
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
                .figure
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

        self.record_property_change(
            id,
            "border",
            PropertyValue::Bool(had_border),
            PropertyValue::Bool(has_border),
        );
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
        self.record_property_change(
            id,
            "corner_dimensions",
            PropertyValue::Size(old),
            PropertyValue::Size(dimensions),
        );
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
        self.record_property_change(
            id,
            "direction",
            PropertyValue::Text(format!("{old:?}")),
            PropertyValue::Text(format!("{direction:?}")),
        );
        self.mark_invalid(update_manager, id);
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn refresh_label_intrinsic_layouts(
        &mut self,
        text: &mut dyn TextLayoutEngine,
        resources: &crate::ResourceRegistry,
    ) -> Result<TextLayoutRefreshResult, TextError> {
        let label_ids = self
            .blocks
            .iter()
            .filter_map(|(id, node)| node.figure.label().is_some().then_some(id))
            .collect::<Vec<_>>();
        if label_ids.is_empty() {
            return Ok(TextLayoutRefreshResult {
                changed: Vec::new(),
                style_nodes_visited: 0,
                figures_refreshed: 0,
            });
        }
        let (labels, style_nodes_visited) =
            self.resolved_styles_for(|node| node.figure.label().is_some());
        let figures_refreshed = u64::try_from(label_ids.len()).unwrap_or(u64::MAX);
        let mut changed = Vec::new();
        for (id, style) in labels {
            let font = crate::render::FontDescriptor::parse(&style.font)?;
            let label = self.blocks[id]
                .figure
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
    ) -> Result<Vec<FigureId>, TextError> {
        let labels = self
            .blocks
            .iter()
            .filter_map(|(id, block)| block.figure.label().is_some().then_some(id))
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for id in labels {
            let bounds = self.blocks[id].client_area();
            let label = self.blocks[id]
                .figure
                .label_mut()
                .expect("label capability checked before mutable borrow");
            if label.refresh_presentation(text, bounds)? {
                changed.push(id);
            }
        }
        Ok(changed)
    }

    pub(crate) fn refresh_text_flow_layouts(
        &mut self,
        text: &mut dyn TextLayoutEngine,
    ) -> Result<TextLayoutRefreshResult, TextError> {
        let flow_ids = self
            .blocks
            .iter()
            .filter_map(|(id, node)| node.figure.text_flow().is_some().then_some(id))
            .collect::<Vec<_>>();
        if flow_ids.is_empty() {
            return Ok(TextLayoutRefreshResult {
                changed: Vec::new(),
                style_nodes_visited: 0,
                figures_refreshed: 0,
            });
        }
        let (flows, style_nodes_visited) =
            self.resolved_styles_for(|node| node.figure.text_flow().is_some());
        let figures_refreshed = u64::try_from(flow_ids.len()).unwrap_or(u64::MAX);
        let mut changed = Vec::new();
        for (id, style) in flows {
            let font = crate::render::FontDescriptor::parse(&style.font)?;
            let bounds = self.blocks[id].client_area();
            let flow = self.blocks[id]
                .figure
                .as_any_mut()
                .downcast_mut::<crate::TextFlowFigure>()
                .expect("TextFlow capability belongs to TextFlowFigure");
            if flow.refresh_layout(text, &font, bounds)? {
                changed.push(id);
            }
        }
        Ok(TextLayoutRefreshResult {
            changed,
            style_nodes_visited,
            figures_refreshed,
        })
    }

    pub(crate) fn refresh_image_figures(
        &mut self,
        resources: &crate::ResourceRegistry,
    ) -> Vec<FigureId> {
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
        self.record_property_change(
            id,
            "image",
            PropertyValue::Text(format!("{previous_image:?}")),
            PropertyValue::Text(format!("{image:?}")),
        );
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
        self.record_property_change(
            id,
            "image_alignment",
            PropertyValue::Text(format!("{previous:?}")),
            PropertyValue::Text(format!("{alignment:?}")),
        );
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mutate_label(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        property: &'static str,
        old_value: PropertyValue,
        new_value: PropertyValue,
        mutate: impl FnOnce(&mut LabelFigure),
        revalidate: bool,
    ) -> Result<bool, ShapeMutationError> {
        let Some(block) = self.blocks.get(id) else {
            return Err(ShapeMutationError::UnknownFigure(id));
        };
        if block.figure.label().is_none() {
            return Err(ShapeMutationError::WrongCapability(id));
        }
        if old_value == new_value {
            return Ok(false);
        }
        let label = self.blocks[id]
            .figure
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
        let Some(clickable) = self
            .blocks
            .get_mut(id)
            .and_then(|block| block.figure.clickable_mut())
        else {
            return false;
        };
        let (selection_change, revision) =
            crate::figure::widget::activate(clickable.clickable_model_mut());
        self.notify_block_changed(id);
        if let Some((old, new)) = selection_change {
            self.emit_property_event(PropertyChangeEvent {
                figure_id: id,
                property: "selected",
                old_value: PropertyValue::Bool(old),
                new_value: PropertyValue::Bool(new),
            });
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
        let Some(clickable) = block.figure.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().is_selected();
        if !crate::figure::widget::set_selected(clickable.clickable_model_mut(), selected) {
            return Ok(false);
        }
        self.record_property_change(
            id,
            "selected",
            PropertyValue::Bool(old),
            PropertyValue::Bool(selected),
        );
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
        let Some(clickable) = block.figure.clickable_mut() else {
            return Err(WidgetError::WrongCapability(id));
        };
        let old = clickable.clickable_model().rollover_enabled();
        if !crate::figure::widget::set_rollover_enabled(clickable.clickable_model_mut(), enabled) {
            return Ok(false);
        }
        self.record_property_change(
            id,
            "rollover_enabled",
            PropertyValue::Bool(old),
            PropertyValue::Bool(enabled),
        );
        self.repaint(update_manager, id, None);
        Ok(true)
    }

    pub(crate) fn sync_clickable_visual(
        &mut self,
        update_manager: &mut UpdateManager,
        id: FigureId,
        visual: ClickableVisualState,
    ) -> bool {
        let Some(clickable) = self
            .blocks
            .get_mut(id)
            .and_then(|block| block.figure.clickable_mut())
        else {
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
            self.record_property_change(
                id,
                "foreground",
                old.foreground
                    .map_or(PropertyValue::None, PropertyValue::Color),
                new.foreground
                    .map_or(PropertyValue::None, PropertyValue::Color),
            );
        }
        if old.background != new.background {
            self.record_property_change(
                id,
                "background",
                old.background
                    .map_or(PropertyValue::None, PropertyValue::Color),
                new.background
                    .map_or(PropertyValue::None, PropertyValue::Color),
            );
        }
        if old.alpha != new.alpha {
            self.record_property_change(
                id,
                "alpha",
                old.alpha.map_or(PropertyValue::None, PropertyValue::Number),
                new.alpha.map_or(PropertyValue::None, PropertyValue::Number),
            );
        }
        if old.font != new.font {
            self.record_property_change(
                id,
                "font",
                old.font
                    .clone()
                    .map_or(PropertyValue::None, PropertyValue::Text),
                new.font
                    .clone()
                    .map_or(PropertyValue::None, PropertyValue::Text),
            );
        }
        if old.cursor != new.cursor {
            self.record_property_change(
                id,
                "cursor",
                old.cursor
                    .map_or(PropertyValue::None, PropertyValue::Cursor),
                new.cursor
                    .map_or(PropertyValue::None, PropertyValue::Cursor),
            );
        }
        if old.tooltip != new.tooltip {
            self.record_property_change(
                id,
                "tooltip",
                old.tooltip
                    .clone()
                    .flatten()
                    .map_or(PropertyValue::None, PropertyValue::Text),
                new.tooltip
                    .clone()
                    .flatten()
                    .map_or(PropertyValue::None, PropertyValue::Text),
            );
        }
    }
}
