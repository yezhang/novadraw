//! 递归渲染实现
//!
//! 直接递归实现 Figure 树的渲染遍历，参考 Eclipse Draw2D 的 paint() 方法。

use crate::render::NdCanvas;

use super::FigureId;
use crate::{ChildClippingStrategy, ResolvedStyle};

const RECURSIVE_STACK_CHECK_INTERVAL: usize = 16;
const RECURSIVE_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_STACK_GROWTH: usize = 4 * 1024 * 1024;

/// 场景图引用（用于渲染）
pub(super) struct FigureTreeRenderRef<'a> {
    pub(crate) blocks: &'a crate::identity::RuntimeArena<FigureId, super::FigureNode>,
    pub(crate) presentation: Option<&'a crate::animation::PresentationSnapshot>,
}

impl<'a> FigureTreeRenderRef<'a> {
    /// 获取块
    pub(super) fn get(&self, id: FigureId) -> Option<&super::FigureNode> {
        self.blocks.get(id)
    }

    fn presentation(&self, id: FigureId) -> Option<crate::animation::FigurePresentationEffect> {
        self.presentation.and_then(|snapshot| snapshot.figure(id))
    }

    fn visible(&self, id: FigureId) -> bool {
        if let Some(geometry) = self.geometry(id) {
            return geometry.effective_visible;
        }
        self.get(id).is_some_and(|node| {
            self.presentation(id)
                .and_then(|effect| effect.geometry.visible)
                .unwrap_or(node.is_visible)
        })
    }

    fn bounds(&self, id: FigureId, node: &super::FigureNode) -> crate::Rectangle {
        if let Some(geometry) = self.geometry(id) {
            return geometry.bounds;
        }
        self.presentation(id)
            .and_then(|effect| effect.geometry.bounds)
            .unwrap_or_else(|| node.figure_bounds())
    }

    fn geometry(&self, id: FigureId) -> Option<&crate::animation::PresentedFigureGeometry> {
        self.presentation?.geometry.as_ref()?.data.nodes.get(&id)
    }

    fn frozen(&self, id: FigureId) -> Option<&crate::animation::FrozenGeometryNode> {
        self.presentation?.geometry.as_ref()?.frozen_node(id)
    }
}

impl<'a> Clone for FigureTreeRenderRef<'a> {
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks,
            presentation: self.presentation,
        }
    }
}

/// Figure 渲染器（递归模式）
///
/// 直接递归实现，简洁直观。
pub(super) struct FigureRenderer<'a> {
    scene: FigureTreeRenderRef<'a>,
    gc: &'a mut NdCanvas,
}

impl<'a> FigureRenderer<'a> {
    /// 创建渲染器
    pub(super) fn new(scene: &FigureTreeRenderRef<'a>, gc: &'a mut NdCanvas) -> Self {
        Self {
            scene: FigureTreeRenderRef {
                blocks: scene.blocks,
                presentation: scene.presentation,
            },
            gc,
        }
    }

    /// 递归渲染
    ///
    /// 对应 draw2d Figure.paint() final。
    pub(super) fn render(&mut self, root_id: FigureId) {
        let defaults = ResolvedStyle::default();
        self.gc.set_foreground_color(defaults.foreground);
        self.gc.set_background_color(defaults.background);
        self.paint(root_id, 0);
    }

    /// Seeds inheritance without applying the capture root's external placement.
    pub(super) fn capture(
        &mut self,
        root: FigureId,
        rect: crate::Rectangle,
        style: &ResolvedStyle,
        font: crate::text::FontDescriptor,
    ) {
        let Some(node) = self.scene.get(root) else {
            return;
        };
        let bounds = node.figure_bounds();
        self.gc.set_foreground_color(style.foreground);
        self.gc.set_background_color(style.background);
        self.gc.global_alpha(style.alpha);
        self.gc.set_font(font);
        self.gc.clip_rect(0.0, 0.0, rect.width, rect.height);
        self.gc.translate(-rect.x - bounds.x, -rect.y - bounds.y);
        self.gc
            .set_transform_floor(crate::Affine2D::from_translation(-rect.x, -rect.y));
        self.paint(root, 0);
    }

    /// 绘制 Figure
    ///
    /// 对应 draw2d Figure.paint()：
    /// ```text
    /// paint(Graphics)
    ///   ├─> setLocalBackgroundColor()
    ///   ├─> setLocalForegroundColor()
    ///   ├─> setLocalFont()
    ///   └─> pushState()
    ///         ├─> paintFigure()
    ///         ├─> restoreState()
    ///         ├─> paintClientArea()
    ///         │     └─> paintChildren() + restoreState()
    ///         ├─> paintBorder()
    ///         └─> popState()
    /// ```
    fn paint(&mut self, figure_id: FigureId, depth: usize) {
        if depth.is_multiple_of(RECURSIVE_STACK_CHECK_INTERVAL) {
            stacker::maybe_grow(RECURSIVE_STACK_RED_ZONE, RECURSIVE_STACK_GROWTH, || {
                self.paint_inner(figure_id, depth);
            });
        } else {
            self.paint_inner(figure_id, depth);
        }
    }

    fn paint_inner(&mut self, figure_id: FigureId, depth: usize) {
        // 获取 block
        let block = match self.scene.get(figure_id) {
            Some(b) if self.scene.visible(figure_id) => b,
            _ => return,
        };

        let bounds = self.scene.bounds(figure_id, block);

        // 1. 保存 parent state，并设置当前节点的 local state。
        self.gc.push_state();
        let frozen = self.scene.frozen(figure_id);
        let style = frozen.map_or(&block.style, |node| &node.style);
        if let Some(foreground) = style.foreground {
            self.gc.set_foreground_color(foreground);
        }
        if let Some(background) = style.background {
            self.gc.set_background_color(background);
        }
        if let Some(alpha) = style.alpha {
            self.gc.global_alpha(alpha);
        }
        let presentation = self.scene.presentation(figure_id);
        if let Some(alpha) = presentation.as_ref().and_then(|effect| effect.opacity) {
            self.gc.global_alpha(alpha);
        }
        self.gc.translate(bounds.x, bounds.y);
        let local_transform = self
            .scene
            .geometry(figure_id)
            .map(|geometry| geometry.local_transform)
            .or_else(|| presentation.as_ref().and_then(|effect| effect.transform));
        if let Some(transform) = local_transform {
            let [a, b, c, d, e, f] = transform.coeffs();
            self.gc.transform(a, b, c, d, e, f);
        }
        if self.scene.geometry(figure_id).is_none()
            && let Some(transform) = presentation
                .as_ref()
                .and_then(|effect| effect.geometry.local_transform)
        {
            let [a, b, c, d, e, f] = transform.coeffs();
            self.gc.transform(a, b, c, d, e, f);
        }

        // 2. Figure paint 允许临时修改 graphics state，但不能泄漏到 children。
        self.gc.push_state();
        if frozen.map_or_else(|| block.state().is_opaque(), |node| node.opaque)
            && presentation
                .as_ref()
                .is_none_or(|effect| effect.content.is_none())
        {
            self.gc
                .fill_rectangle(0.0, 0.0, bounds.width, bounds.height);
        }
        if let Some(content) = self
            .scene
            .geometry(figure_id)
            .and_then(|geometry| geometry.content.as_ref())
            .or_else(|| {
                presentation
                    .as_ref()
                    .and_then(|effect| effect.content.as_ref())
            })
        {
            let mut context = crate::graphics::PaintContext::for_figure(self.gc);
            if let Err(error) = content.paint(&mut context) {
                context.canvas.reject_recording(error);
            }
            drop(context);
        } else {
            let mut context = crate::graphics::PaintContext::for_figure(self.gc);
            if let Some(prepared) = &block.prepared {
                if let Err(error) = prepared.presentation.paint(&mut context) {
                    context.canvas.reject_recording(error);
                }
            } else if let Err(error) = block.figure.paint(
                &mut context,
                crate::geometry::Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
            ) {
                context.canvas.reject_recording(error);
            }
            drop(context);
        }
        self.gc.pop_state();

        // 3. 绘制子元素区域。
        self.paint_client_area(figure_id, depth);

        // 4. 绘制边框
        // 注意：block 借用在此结束，可以安全重新获取
        let block = match self.scene.get(figure_id) {
            Some(b) if self.scene.visible(figure_id) => b,
            _ => return,
        };
        let mut context = crate::graphics::PaintContext::for_figure(self.gc);
        let local_bounds = crate::geometry::Rectangle::new(0.0, 0.0, bounds.width, bounds.height);
        let result = if let Some(border) = self
            .scene
            .geometry(figure_id)
            .and_then(|geometry| geometry.border.as_ref())
        {
            border.paint(&mut context)
        } else {
            block.figure.paint_border_snapshot_in_bounds(
                &mut context,
                local_bounds,
                block.border_snapshot.as_ref(),
            )
        };
        if let Err(error) = result {
            context.canvas.reject_recording(error);
        }
        drop(context);

        // 5. 恢复 parent state。
        self.gc.pop_state();
    }

    /// 绘制子元素区域
    ///
    /// 对应 draw2d Figure.paintClientArea()：
    /// ```text
    /// paintClientArea(Graphics)
    ///   if (useLocalCoordinates) {
    ///     translate(x + left, y + top);
    ///     clipRect(0, 0, w - left - right, h - top - bottom);
    ///   } else {
    ///     clipRect(clientArea);
    ///   }
    ///   paintChildren(graphics);
    /// ```
    fn paint_client_area(&mut self, figure_id: FigureId, depth: usize) {
        let block = match self.scene.get(figure_id) {
            Some(b) if self.scene.visible(figure_id) => b,
            _ => return,
        };

        let geometry = self
            .scene
            .presentation(figure_id)
            .map(|effect| effect.geometry);
        let transform = self
            .scene
            .geometry(figure_id)
            .map(|geometry| geometry.child_transform)
            .or_else(|| {
                geometry
                    .and_then(|patch| patch.child_transform)
                    .map(|transform| {
                        let insets = block.state().insets;
                        crate::Affine2D::from_translation(insets.left, insets.top) * transform
                    })
            })
            .unwrap_or_else(|| block.child_transform().affine());
        let bounds = self.scene.bounds(figure_id, block);
        let insets = block.state().insets;
        let client_area = self
            .scene
            .geometry(figure_id)
            .map(|geometry| geometry.client_box)
            .unwrap_or_else(|| {
                crate::Rectangle::new(
                    insets.left,
                    insets.top,
                    (bounds.width - insets.width()).max(0.0),
                    (bounds.height - insets.height()).max(0.0),
                )
            });
        let clipping_strategy = self
            .scene
            .frozen(figure_id)
            .map_or_else(|| block.child_clipping_strategy(), |node| node.clipping);
        let [a, b, c, d, e, f] = transform.coeffs();
        self.gc.push_state();
        if clipping_strategy != ChildClippingStrategy::OverflowVisible {
            self.gc.clip_rect(
                client_area.x,
                client_area.y,
                client_area.width,
                client_area.height,
            );
        }
        self.gc.transform(a, b, c, d, e, f);

        self.paint_children(figure_id, depth);
        self.gc.pop_state();
    }

    /// 绘制子元素
    ///
    /// 对应 draw2d Figure.paintChildren()。
    /// 为每个子节点设置裁剪 + 绘制 + 恢复。
    ///
    /// draw2d 逻辑：
    /// ```text
    /// for (IFigure child : children) {
    ///   if (child.isVisible()) {
    ///     Rectangle[] clipping = new Rectangle[] { child.getBounds() };
    ///     for (Rectangle element : clipping) {
    ///       if (element.intersects(graphics.getClip())) {
    ///         graphics.clipRect(element);
    ///         child.paint(graphics);
    ///         graphics.restoreState();
    ///       }
    ///     }
    ///   }
    /// }
    /// ```
    fn paint_children(&mut self, figure_id: FigureId, depth: usize) {
        let children: Vec<FigureId> = {
            let block = match self.scene.get(figure_id) {
                Some(b) if self.scene.visible(figure_id) => b,
                _ => return,
            };
            self.scene.geometry(figure_id).map_or_else(
                || block.children.to_vec(),
                |geometry| geometry.children.clone(),
            )
        };
        let clipping_strategy = self
            .scene
            .frozen(figure_id)
            .map(|node| node.clipping)
            .or_else(|| {
                self.scene
                    .get(figure_id)
                    .map(super::FigureNode::child_clipping_strategy)
            })
            .unwrap_or(ChildClippingStrategy::ClipToChildBounds);

        // 正序遍历（与 draw2d 一致）
        for &child_id in &children {
            let child_block = match self.scene.get(child_id) {
                Some(b) if self.scene.visible(child_id) => b,
                _ => continue,
            };

            self.gc.push_state();
            let child_overflow = self.scene.frozen(child_id).map_or_else(
                || child_block.child_clipping_strategy(),
                |node| node.clipping,
            ) == ChildClippingStrategy::OverflowVisible
                || self
                    .scene
                    .presentation(child_id)
                    .and_then(|effect| effect.transform)
                    .is_some_and(|transform| transform != crate::Affine2D::IDENTITY);
            match (clipping_strategy, child_overflow) {
                (ChildClippingStrategy::ClipToChildBounds, false) => {
                    let child_bounds = self.scene.bounds(child_id, child_block);
                    self.gc.clip_rect(
                        child_bounds.x,
                        child_bounds.y,
                        child_bounds.width,
                        child_bounds.height,
                    );
                    self.paint(child_id, depth + 1);
                }
                _ => {
                    self.paint(child_id, depth + 1);
                }
            }
            self.gc.pop_state();
        }
    }
}
