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
            Some(b) if b.is_visible => b,
            _ => return,
        };

        let bounds = block.figure_bounds();

        // 1. 保存 parent state，并设置当前节点的 local state。
        self.gc.push_state();
        if let Some(foreground) = block.style.foreground {
            self.gc.set_foreground_color(foreground);
        }
        if let Some(background) = block.style.background {
            self.gc.set_background_color(background);
        }
        if let Some(alpha) = block.style.alpha {
            self.gc.set_alpha(alpha);
        }
        let presentation = self.scene.presentation(figure_id);
        if let Some(alpha) = presentation.as_ref().and_then(|effect| effect.opacity) {
            self.gc.set_alpha(alpha);
        }
        self.gc.translate(bounds.x, bounds.y);
        if let Some(transform) = presentation.as_ref().and_then(|effect| effect.transform) {
            let [a, b, c, d, e, f] = transform.coeffs();
            self.gc.transform(a, b, c, d, e, f);
        }

        // 2. Figure paint 允许临时修改 graphics state，但不能泄漏到 children。
        self.gc.push_state();
        if let Some(content) = presentation
            .as_ref()
            .and_then(|effect| effect.content.as_ref())
        {
            let mut context = crate::graphics::PaintContext::for_figure(self.gc);
            if let Err(error) = content.paint(&mut context) {
                context.canvas.reject_recording(error);
            }
            drop(context);
        } else {
            if block.state().is_opaque() {
                self.gc
                    .fill_rectangle(0.0, 0.0, bounds.width, bounds.height);
            }
            let mut context = crate::graphics::PaintContext::for_figure(self.gc);
            if let Some(prepared) = &block.prepared {
                if let Err(error) = prepared.presentation.paint(&mut context) {
                    context.canvas.reject_recording(error);
                }
            } else {
                block.figure.paint(
                    &mut context,
                    crate::geometry::Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
                );
            }
            drop(context);
        }
        self.gc.pop_state();

        // 3. 绘制子元素区域。
        self.paint_client_area(figure_id, depth);

        // 4. 绘制边框
        // 注意：block 借用在此结束，可以安全重新获取
        let block = match self.scene.get(figure_id) {
            Some(b) if b.is_visible => b,
            _ => return,
        };
        block.figure.paint_border_snapshot_in_bounds(
            self.gc,
            crate::geometry::Rectangle::new(0.0, 0.0, bounds.width, bounds.height),
            block.border_snapshot.as_ref(),
        );

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
            Some(b) if b.is_visible => b,
            _ => return,
        };

        let transform = block.child_transform();
        let client_area = block.client_area();
        let clipping_strategy = block.child_clipping_strategy();
        let [a, b, c, d, e, f] = transform.affine().coeffs();
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
                Some(b) if b.is_visible => b,
                _ => return,
            };
            block.children.to_vec()
        };
        let clipping_strategy = self
            .scene
            .get(figure_id)
            .map(super::FigureNode::child_clipping_strategy)
            .unwrap_or(ChildClippingStrategy::ClipToChildBounds);

        // 正序遍历（与 draw2d 一致）
        for &child_id in &children {
            let child_block = match self.scene.get(child_id) {
                Some(b) if b.is_visible => b,
                _ => continue,
            };

            self.gc.push_state();
            let child_overflow = child_block.child_clipping_strategy()
                == ChildClippingStrategy::OverflowVisible
                || self
                    .scene
                    .presentation(child_id)
                    .and_then(|effect| effect.transform)
                    .is_some_and(|transform| transform != crate::Affine2D::IDENTITY);
            match (clipping_strategy, child_overflow) {
                (ChildClippingStrategy::ClipToChildBounds, false) => {
                    let child_bounds = child_block.figure_bounds();
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
