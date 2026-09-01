//! 三角形图形
//!
//! 参考 Eclipse Draw2D 的 Triangle 设计。
//! 预定义的等腰三角形，主要用于箭头指向标（Connection Anchors）。
//!
//! # 设计原理
//!
//! ## 顶点计算时机
//!
//! 三角形的三个顶点通过 `validate()` 阶段计算并缓存，渲染时使用缓存的顶点。
//! 这种设计基于 draw2d 的延迟布局（Deferred Layout）模式：
//!
//! | 操作类型 | 处理方式 | 原因 |
//! |----------|----------|------|
//! | 平移 (translate) | 只平移缓存的点 | 顶点相对位置不变，无需重算 |
//! | 大小变化 (resize) | invalidate → validate 重算 | 顶点依赖 bounds 尺寸 |
//! | 方向变化 (direction) | invalidate → validate 重算 | 顶点分布方式改变 |
//!
//! ## validate() 设计理由
//!
//! 1. **避免重复计算**：如果每次 setBounds() 都同步计算顶点，而紧接着父容器又因为布局管理器调整了 bounds，就会产生冗余计算。validate() 机制确保 bounds 在布局完成后才计算顶点。
//!
//! 2. **布局批处理**：DeferredUpdateManager 在统一的验证阶段批量处理所有 invalid figures，避免多次同步调用。
//!
//! 3. **渲染与几何分离**：bounds 是逻辑状态，顶点是渲染产物。validate() 处于"逻辑状态已确定 → 即将渲染"的节点，此时 bounds 已稳定，计算顶点最合适。

use std::sync::Arc;

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use super::{Border, Bounded, ChildClippingStrategy, Figure, Shape, Updatable};

/// 三角形方向
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Direction {
    /// 指向上方
    North,
    /// 指向下方
    South,
    /// 指向左侧
    West,
    /// 指向右侧
    #[default]
    East,
}

/// 三角形图形
///
/// 参考 Eclipse Draw2D 的 Triangle 设计。
/// 预定义的等腰三角形，根据 bounds 和 direction 动态计算顶点。
/// 主要用于箭头指向标（Connection Anchors）。
///
/// # 顶点计算
///
/// 顶点在 `validate()` 阶段计算并缓存，参考 draw2d: Figure.validate()。
/// 渲染时使用缓存的顶点，避免重复计算。
#[derive(Clone)]
pub struct TriangleFigure {
    /// 边界矩形
    pub bounds: Rectangle,
    /// 填充颜色
    pub fill_color: Color,
    /// 描边颜色
    pub stroke_color: Color,
    /// 描边宽度
    pub stroke_width: f64,
    /// 方向
    pub direction: Direction,
    /// 线帽样式
    pub line_cap: novadraw_render::command::LineCap,
    /// 连接样式
    pub line_join: novadraw_render::command::LineJoin,
    /// 缓存的顶点（validate 后有效）
    cached_points: Option<[(f64, f64); 3]>,
    /// 缓存的 bounds（用于检测是否需要重新计算）
    cached_bounds: Option<Rectangle>,
    /// 绘制子节点时使用的裁剪策略
    child_clipping_strategy: ChildClippingStrategy,
    /// 边框装饰器
    border: Option<Arc<dyn Border>>,
}

impl TriangleFigure {
    /// 创建三角形
    ///
    /// # Arguments
    ///
    /// * `x` - 左上角 x 坐标
    /// * `y` - 左上角 y 坐标
    /// * `width` - 宽度
    /// * `height` - 高度
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: Color::hex("#e74c3c"),
            stroke_color: Color::hex("#c0392b"),
            stroke_width: 1.0,
            direction: Direction::North,
            line_cap: novadraw_render::command::LineCap::Butt,
            line_join: novadraw_render::command::LineJoin::Miter,
            cached_points: None,
            cached_bounds: None,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 从 Rectangle 创建三角形
    pub fn from_bounds(bounds: Rectangle) -> Self {
        Self::new(bounds.x, bounds.y, bounds.width, bounds.height)
    }

    /// 创建指定方向的三角形
    pub fn new_with_direction(
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        direction: Direction,
    ) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            fill_color: Color::hex("#e74c3c"),
            stroke_color: Color::hex("#c0392b"),
            stroke_width: 1.0,
            direction,
            line_cap: novadraw_render::command::LineCap::Butt,
            line_join: novadraw_render::command::LineJoin::Miter,
            cached_points: None,
            cached_bounds: None,
            child_clipping_strategy: ChildClippingStrategy::ClipToChildBounds,
            border: None,
        }
    }

    /// 设置填充颜色
    pub fn with_fill_color(mut self, color: Color) -> Self {
        self.fill_color = color;
        self
    }

    /// 设置描边颜色
    pub fn with_stroke_color(mut self, color: Color) -> Self {
        self.stroke_color = color;
        self
    }

    /// 设置描边宽度
    pub fn with_stroke_width(mut self, width: f64) -> Self {
        self.stroke_width = width;
        self
    }

    /// 设置方向
    ///
    /// 参考 draw2d: Triangle.setDirection(int)
    /// 方向变化需要重新计算顶点，因此清除缓存。
    pub fn set_direction(&mut self, direction: Direction) {
        if self.direction != direction {
            self.direction = direction;
            self.invalidate();
        }
    }

    /// 设置方向（builder 风格）
    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// 平移三角形（仅位置变化）
    ///
    /// 参考 draw2d: Triangle.primTranslate(int, int)
    /// 平移时直接平移缓存的顶点，不需要重新计算。
    /// 这是性能优化，避免在纯位置变化时触发 validate 重算。
    pub fn prim_translate(&mut self, dx: f64, dy: f64) {
        self.bounds.x += dx;
        self.bounds.y += dy;

        // 平移缓存的顶点
        if let Some(ref mut points) = self.cached_points {
            for point in points.iter_mut() {
                point.0 += dx;
                point.1 += dy;
            }
        }

        // 更新缓存的 bounds
        if let Some(ref mut cached) = self.cached_bounds {
            cached.x += dx;
            cached.y += dy;
        }
    }

    /// 设置线条样式
    pub fn with_style(mut self, fill: Color, stroke: Color, stroke_width: f64) -> Self {
        self.fill_color = fill;
        self.stroke_color = stroke;
        self.stroke_width = stroke_width;
        self
    }

    /// 设置子节点绘制裁剪策略。
    pub fn with_child_clipping_strategy(mut self, strategy: ChildClippingStrategy) -> Self {
        self.child_clipping_strategy = strategy;
        self
    }

    /// 添加边框装饰器。
    pub fn with_border(mut self, border: impl Border + 'static) -> Self {
        self.border = Some(Arc::new(border));
        self
    }

    /// 计算三角形的三个顶点
    ///
    /// 参考 draw2d Triangle.validate():
    /// - r.shrink(getInsets()) - 收缩 insets (novadraw 暂不支持 insets)
    /// - r.resize(-1, -1) - 向内收缩，为描边预留空间
    /// - r.y/r.x += (height/width - size) / 2 - 居中调整
    ///
    /// 关键点：
    /// 1. 描边向内收缩，填充区域缩小
    /// 2. 主尖角完整，不被裁剪（在收缩后的边界内，考虑描边向外扩展）
    /// 3. 底部两角的裁剪由渲染器的 line join 行为自然产生
    fn compute_points(&self) -> [(f64, f64); 3] {
        let mut r = Rectangle::new(0.0, 0.0, self.bounds.width, self.bounds.height);

        if r.width <= 0.0 || r.height <= 0.0 {
            return [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)];
        }

        // 向内收缩 1px (对应 draw2d 的 r.resize(-1, -1))
        r.x += 1.0;
        r.y += 1.0;
        r.width = (r.width - 2.0).max(0.0);
        r.height = (r.height - 2.0).max(0.0);

        if r.width <= 0.0 || r.height <= 0.0 {
            return [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)];
        }

        // 计算 size 并居中 (对应 draw2d 的逻辑)
        // 使用 f64::min 明确指定类型
        let size: f64 = match self.direction {
            Direction::North | Direction::South => {
                // 垂直方向：size = min(height, width / 2)
                let half_width = r.width / 2.0;
                let size = if r.height < half_width {
                    r.height
                } else {
                    half_width
                };
                // 垂直居中：r.y += (r.height - size) / 2
                r.y += (r.height - size) / 2.0;
                size
            }
            Direction::West | Direction::East => {
                // 水平方向：size = min(height / 2, width)
                let half_height = r.height / 2.0;
                let size = if half_height < r.width {
                    half_height
                } else {
                    r.width
                };
                // 水平居中：r.x += (r.width - size) / 2
                r.x += (r.width - size) / 2.0;
                size
            }
        };

        // 最小尺寸保护 (对应 draw2d 的 Math.max(size, 1))
        let size = size.max(1.0);

        match self.direction {
            Direction::North => {
                // 顶点朝上
                let head_x = r.x + r.width / 2.0;
                let head_y = r.y;
                let p2_x = head_x - size;
                let p3_x = head_x + size;
                let bottom_y = head_y + size;

                [(head_x, head_y), (p2_x, bottom_y), (p3_x, bottom_y)]
            }
            Direction::South => {
                // 顶点朝下
                let head_x = r.x + r.width / 2.0;
                let head_y = r.y + size;
                let p2_x = head_x - size;
                let p3_x = head_x + size;
                let bottom_y = head_y - size;

                [(head_x, head_y), (p2_x, bottom_y), (p3_x, bottom_y)]
            }
            Direction::West => {
                // 顶点朝左
                let head_x = r.x;
                let head_y = r.y + r.height / 2.0;
                let right_x = head_x + size;
                let p2_y = head_y - size;
                let p3_y = head_y + size;

                [(head_x, head_y), (right_x, p2_y), (right_x, p3_y)]
            }
            Direction::East => {
                // 顶点朝右
                let head_x = r.x + size;
                let head_y = r.y + r.height / 2.0;
                let left_x = head_x - size;
                let p2_y = head_y - size;
                let p3_y = head_y + size;

                [(head_x, head_y), (left_x, p2_y), (left_x, p3_y)]
            }
        }
    }
}

impl Bounded for TriangleFigure {
    fn bounds(&self) -> Rectangle {
        // 返回原始 bounds，与 draw2d 保持一致
        // draw2d 中 Figure.bounds 保持为用户设置的原始值，不受 validate() 中收缩的影响
        // paintChildren() 使用原始 bounds 作为裁剪区域
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let new_bounds = Rectangle::new(x, y, width, height);

        // 参考 draw2d: setBounds() 只有在大小变化时才调用 invalidate()
        // 纯位置变化通过 prim_translate() 处理，不需要重新计算顶点
        if self.bounds.width != width || self.bounds.height != height {
            // 大小变化：清除缓存，触发 validate 时重新计算
            self.invalidate();
        }

        self.bounds = new_bounds;
    }

    fn name(&self) -> &'static str {
        "TriangleFigure"
    }

    fn child_clipping_strategy(&self) -> ChildClippingStrategy {
        self.child_clipping_strategy
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.border
            .as_ref()
            .map(|border| border.get_insets())
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }
}

// 实现 Updatable trait：验证钩子
impl Updatable for TriangleFigure {
    /// 布局验证：计算并缓存顶点
    ///
    /// 对应 draw2d: Triangle.validate()
    /// 在布局完成后被调用，预计算三角形的顶点位置。
    fn validate(&mut self) {
        // 检查 bounds 是否变化，变化则重新计算顶点
        if self.cached_bounds != Some(self.bounds) {
            self.cached_points = Some(self.compute_points());
            self.cached_bounds = Some(self.bounds);
        }
    }

    fn invalidate(&mut self) {
        // 清除缓存，强制重新计算
        self.cached_points = None;
        self.cached_bounds = None;
    }
}

impl Figure for TriangleFigure {
    fn paint_figure(&self, gc: &mut NdCanvas) {
        Shape::paint_figure(self, gc);
    }

    fn get_border(&self) -> Option<&dyn Border> {
        Shape::get_border(self)
    }
}

impl Shape for TriangleFigure {
    fn stroke_color(&self) -> Option<Color> {
        if self.stroke_color.a > 0.0 {
            Some(self.stroke_color)
        } else {
            None
        }
    }

    fn stroke_width(&self) -> f64 {
        self.stroke_width
    }

    fn fill_color(&self) -> Option<Color> {
        if self.fill_color.a > 0.0 {
            Some(self.fill_color)
        } else {
            None
        }
    }

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        self.line_cap
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        self.line_join
    }

    fn get_border(&self) -> Option<&dyn Border> {
        self.border.as_deref()
    }

    fn fill_enabled(&self) -> bool {
        self.fill_color.a > 0.0
    }

    fn outline_enabled(&self) -> bool {
        self.stroke_color.a > 0.0 && self.stroke_width > 0.0
    }

    fn fill_shape(&self, gc: &mut NdCanvas) {
        // 使用缓存的顶点，如果没有缓存则计算
        let points = self.cached_points.unwrap_or_else(|| self.compute_points());

        gc.begin_path();
        gc.move_to(points[0].0, points[0].1);
        gc.line_to(points[1].0, points[1].1);
        gc.line_to(points[2].0, points[2].1);
        gc.close_path();

        gc.fill_style(self.fill_color);
        gc.fill();
    }

    fn outline_shape(&self, gc: &mut NdCanvas) {
        // 使用缓存的顶点，如果没有缓存则计算
        let points = self.cached_points.unwrap_or_else(|| self.compute_points());

        gc.begin_path();
        gc.move_to(points[0].0, points[0].1);
        gc.line_to(points[1].0, points[1].1);
        gc.line_to(points[2].0, points[2].1);
        gc.close_path();

        gc.stroke_style(self.stroke_color);
        gc.line_width(self.stroke_width);
        gc.line_cap(self.line_cap);
        gc.line_join(self.line_join);
        gc.stroke();
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：validate 计算并缓存顶点
    #[test]
    fn test_validate_computes_and_caches_points() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);

        // 初始没有缓存
        assert!(triangle.cached_points.is_none());

        // 调用 validate
        triangle.validate();

        // 现在有缓存
        assert!(triangle.cached_points.is_some());

        // 验证缓存的 bounds
        assert!(triangle.cached_bounds.is_some());
        assert_eq!(triangle.cached_bounds.unwrap(), triangle.bounds);
    }

    /// 测试：bounds 变化后 validate 重新计算顶点
    #[test]
    fn test_validate_recomputes_when_bounds_change() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 获取原始缓存的顶点
        let original_points = triangle.cached_points.unwrap();

        // 改变大小
        triangle.set_bounds(0.0, 0.0, 40.0, 40.0);

        // 再次 validate
        triangle.validate();

        // 获取新的顶点
        let new_points = triangle.cached_points.unwrap();

        // 顶点应该不同（因为大小变了）
        // North 方向的顶点应该在新的位置
        assert_ne!(original_points[0].0, new_points[0].0); // head x 应该变化
    }

    /// 测试：大小不变只平移时缓存被保留
    #[test]
    fn test_prim_translate_preserves_cache() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 获取原始缓存
        let original_points = triangle.cached_points.unwrap();

        // 平移（不改变大小）
        triangle.prim_translate(10.0, 10.0);

        // 缓存应该仍然存在且平移了
        let translated_points = triangle.cached_points.unwrap();
        assert_eq!(translated_points[0].0, original_points[0].0 + 10.0);
        assert_eq!(translated_points[0].1, original_points[0].1 + 10.0);
    }

    /// 测试：set_direction 清除缓存
    #[test]
    fn test_set_direction_clears_cache() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 缓存存在
        assert!(triangle.cached_points.is_some());

        // 改变方向
        triangle.set_direction(Direction::South);

        // 缓存被清除
        assert!(triangle.cached_points.is_none());
    }

    /// 测试：set_bounds 大小变化时清除缓存
    #[test]
    fn test_set_bounds_size_change_clears_cache() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 缓存存在
        assert!(triangle.cached_points.is_some());

        // 改变大小
        triangle.set_bounds(0.0, 0.0, 30.0, 30.0);

        // 缓存被清除
        assert!(triangle.cached_points.is_none());
    }

    /// 测试：set_bounds 只改变位置不清除缓存
    #[test]
    fn test_set_bounds_position_only_preserves_cache() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 缓存存在
        let original_points = triangle.cached_points.unwrap();

        // 只改变位置（大小不变）
        triangle.set_bounds(10.0, 10.0, 20.0, 20.0);

        // 注意：当前实现中 set_bounds 不清除缓存，
        // 因为只检查了大小变化。但如果需要更严格的优化，
        // 可以让 set_bounds 也清除缓存，让 FigureGraph 用 prim_translate 处理纯平移。
        // 目前缓存被保留，validate 会比较 bounds 发现没变，直接用缓存。
        assert!(triangle.cached_points.is_some());
        assert_eq!(triangle.cached_points.unwrap(), original_points);
    }

    /// 测试：invalidate 清除缓存
    #[test]
    fn test_invalidate_clears_cache() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);
        triangle.validate();

        // 缓存存在
        assert!(triangle.cached_points.is_some());

        // 显式 invalidate
        triangle.invalidate();

        // 缓存被清除
        assert!(triangle.cached_points.is_none());
    }

    /// 测试：不同方向的顶点计算
    #[test]
    fn test_different_directions() {
        let mut triangle = TriangleFigure::new(0.0, 0.0, 20.0, 20.0);

        // North 方向
        triangle.direction = Direction::North;
        let north_points = triangle.compute_points();
        // North: 顶点在上方 (y 最小)
        assert!(north_points[0].1 < north_points[1].1);
        assert!(north_points[0].1 < north_points[2].1);

        // South 方向
        triangle.direction = Direction::South;
        let south_points = triangle.compute_points();
        // South: 顶点在下方 (y 最大)
        assert!(south_points[0].1 > south_points[1].1);
        assert!(south_points[0].1 > south_points[2].1);

        // East 方向
        triangle.direction = Direction::East;
        let east_points = triangle.compute_points();
        // East: 顶点在右侧 (x 最大)
        assert!(east_points[0].0 > east_points[1].0);
        assert!(east_points[0].0 > east_points[2].0);

        // West 方向
        triangle.direction = Direction::West;
        let west_points = triangle.compute_points();
        // West: 顶点在左侧 (x 最小)
        assert!(west_points[0].0 < west_points[1].0);
        assert!(west_points[0].0 < west_points[2].0);
    }
}
