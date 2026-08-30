//! 递归渲染实现
//!
//! 直接递归实现 Figure 树的渲染遍历，参考 Eclipse Draw2D 的 paint() 方法。

use novadraw_render::NdCanvas;

use super::BlockId;
use crate::ChildClippingStrategy;
use crate::debug_render;

const RECURSIVE_STACK_RED_ZONE: usize = 128 * 1024;
const RECURSIVE_STACK_GROWTH: usize = 2 * 1024 * 1024;

/// 场景图引用（用于渲染）
pub struct FigureGraphRenderRef<'a> {
    pub(crate) blocks: &'a slotmap::SlotMap<BlockId, super::FigureBlock>,
}

impl<'a> FigureGraphRenderRef<'a> {
    /// 获取块
    pub fn get(&self, id: BlockId) -> Option<&super::FigureBlock> {
        self.blocks.get(id)
    }
}

impl<'a> Clone for FigureGraphRenderRef<'a> {
    fn clone(&self) -> Self {
        Self {
            blocks: self.blocks,
        }
    }
}

/// Figure 渲染器（递归模式）
///
/// 直接递归实现，简洁直观。
pub struct FigureRenderer<'a> {
    scene: FigureGraphRenderRef<'a>,
    gc: &'a mut NdCanvas,
    /// 调试计数器
    counter: usize,
}

impl<'a> FigureRenderer<'a> {
    /// 创建渲染器
    pub fn new(scene: &FigureGraphRenderRef<'a>, gc: &'a mut NdCanvas) -> Self {
        Self {
            scene: FigureGraphRenderRef {
                blocks: scene.blocks,
            },
            gc,
            counter: 0,
        }
    }

    /// 递归渲染
    ///
    /// 对应 draw2d Figure.paint() final。
    pub fn render(&mut self, root_id: BlockId) {
        self.paint(root_id);
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
    fn paint(&mut self, block_id: BlockId) {
        stacker::maybe_grow(RECURSIVE_STACK_RED_ZONE, RECURSIVE_STACK_GROWTH, || {
            self.paint_inner(block_id);
        });
    }

    fn paint_inner(&mut self, block_id: BlockId) {
        // 获取 block
        let block = match self.scene.get(block_id) {
            Some(b) if b.is_visible => b,
            _ => return,
        };

        self.counter += 1;
        let id = self.counter;
        let bounds = block.figure_bounds();
        debug_render!("[RECUR] #{:02} paint bounds={:?}", id, bounds);

        // 1. 设置本地属性
        block.figure.init_properties(self.gc);

        // 2. 保存状态 → 直接调用 gc
        self.gc.push_state();

        // 3. 绘制自身
        block.figure.paint_figure(self.gc);

        // 4. 恢复上下文状态 → 直接调用 gc
        debug_render!("[RECUR] #{:02}   paint_figure done, restore_state", id);
        self.gc.restore_state();

        // 5. 绘制子元素区域（paintClientArea 负责 translate + clip）
        self.paint_client_area(block_id);

        // 6. 绘制边框
        // 注意：block 借用在此结束，可以安全重新获取
        let block = match self.scene.get(block_id) {
            Some(b) if b.is_visible => b,
            _ => return,
        };
        block.figure.paint_border(self.gc);
        super::paint_selection_overlay(block, self.gc);

        // 7. 恢复初始状态 → 直接调用 gc
        debug_render!("[RECUR] #{:02}   pop_state", id);
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
    fn paint_client_area(&mut self, block_id: BlockId) {
        let block = match self.scene.get(block_id) {
            Some(b) if b.is_visible => b,
            _ => return,
        };

        self.counter += 1;
        let id = self.counter;

        if block.figure.use_local_coordinates() {
            let transform = block.figure.child_transform();
            let client_area = block.figure.client_area();
            debug_render!(
                "[RECUR] #{:02} paintClientArea use_local=true, scale({}) translate({}, {}) clip({},{},{},{})",
                id,
                transform.scale,
                transform.translate_x,
                transform.translate_y,
                client_area.x,
                client_area.y,
                client_area.width,
                client_area.height
            );
            self.gc.scale(transform.scale, transform.scale);
            self.gc
                .translate(transform.translate_x, transform.translate_y);
            self.gc.clip_rect(
                client_area.x,
                client_area.y,
                client_area.width,
                client_area.height,
            );
        } else {
            let client_area = block.figure.client_area();
            debug_render!(
                "[RECUR] #{:02} paintClientArea use_local=false, clip({},{},{},{})",
                id,
                client_area.x,
                client_area.y,
                client_area.width,
                client_area.height
            );
            self.gc.clip_rect(
                client_area.x,
                client_area.y,
                client_area.width,
                client_area.height,
            );
        }

        self.gc.push_state();
        self.paint_children(block_id);
        self.gc.pop_state();

        // 恢复 paintClientArea 设置的裁剪区域
        debug_render!("[RECUR] #{:02}   restore_state (client area)", id);
        self.gc.restore_state();
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
    fn paint_children(&mut self, block_id: BlockId) {
        let children: Vec<BlockId> = {
            let block = match self.scene.get(block_id) {
                Some(b) if b.is_visible => b,
                _ => return,
            };
            block.children.to_vec()
        };
        let clipping_strategy = self
            .scene
            .get(block_id)
            .map(|block| block.figure.child_clipping_strategy())
            .unwrap_or(ChildClippingStrategy::ClipToChildBounds);

        debug_render!(
            "[RECUR]     paint_children, children count: {}",
            children.len()
        );

        // 正序遍历（与 draw2d 一致）
        for &child_id in &children {
            let child_block = match self.scene.get(child_id) {
                Some(b) if b.is_visible => b,
                _ => continue,
            };

            match clipping_strategy {
                ChildClippingStrategy::ClipToChildBounds => {
                    let child_bounds = child_block.figure_bounds();
                    debug_render!("[RECUR]     -> clip to child bounds={:?}", child_bounds);
                    self.gc.clip_rect(
                        child_bounds.x,
                        child_bounds.y,
                        child_bounds.width,
                        child_bounds.height,
                    );
                    self.paint(child_id);
                }
                ChildClippingStrategy::DoNotClipChildBounds => {
                    debug_render!("[RECUR]     -> paint child without child bounds clip");
                    self.paint(child_id);
                }
            }
            self.gc.restore_state();
        }
    }
}
