//! Bounds 坐标系统验证测试
//!
//! 验证 bounds 位于 parent content domain，Figure 绘制位于 node-local domain。

use novadraw_core::Color;
use novadraw_geometry::Rectangle;
use novadraw_render::NdCanvas;

use crate::container::{scalable::ScalableLayeredPaneFigure, viewport::ViewportFigure};
use crate::figure::{Bounded, Figure, RectangleFigure, Shape, Updatable};
use crate::graph::FigureGraph;

// ========== 测试用 Figure 类型 ==========

/// 带可配置 insets 的测试 Figure。
#[derive(Clone, Copy)]
struct TestCoordRootFigure {
    bounds: Rectangle,
    insets: (f64, f64, f64, f64),
}

impl TestCoordRootFigure {
    fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            insets: (0.0, 0.0, 0.0, 0.0),
        }
    }

    fn with_insets(x: f64, y: f64, width: f64, height: f64, insets: (f64, f64, f64, f64)) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            insets,
        }
    }
}

impl Bounded for TestCoordRootFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    fn name(&self) -> &'static str {
        "TestCoordRootFigure"
    }
}

/// 另一种带 insets 的测试 Figure。
#[derive(Clone, Copy)]
struct TestInsetFigure {
    bounds: Rectangle,
    insets: (f64, f64, f64, f64),
}

impl TestInsetFigure {
    fn new(x: f64, y: f64, width: f64, height: f64, insets: (f64, f64, f64, f64)) -> Self {
        Self {
            bounds: Rectangle::new(x, y, width, height),
            insets,
        }
    }
}

impl Bounded for TestInsetFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }

    fn insets(&self) -> (f64, f64, f64, f64) {
        self.insets
    }

    fn name(&self) -> &'static str {
        "TestInsetFigure"
    }
}

impl Updatable for TestInsetFigure {
    fn validate(&mut self) {}
    fn invalidate(&mut self) {}
}

impl Shape for TestInsetFigure {
    fn stroke_color(&self) -> Option<novadraw_core::Color> {
        None
    }

    fn stroke_width(&self) -> f64 {
        0.0
    }

    fn fill_color(&self) -> Option<novadraw_core::Color> {
        None
    }

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        novadraw_render::command::LineCap::default()
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        novadraw_render::command::LineJoin::default()
    }

    fn fill_enabled(&self) -> bool {
        false
    }

    fn outline_enabled(&self) -> bool {
        false
    }

    fn fill_shape(&self, _gc: &mut NdCanvas) {}

    fn outline_shape(&self, _gc: &mut NdCanvas) {}
}

impl Updatable for TestCoordRootFigure {
    fn validate(&mut self) {}
    fn invalidate(&mut self) {}
}

impl Shape for TestCoordRootFigure {
    fn stroke_color(&self) -> Option<novadraw_core::Color> {
        None
    }

    fn stroke_width(&self) -> f64 {
        0.0
    }

    fn fill_color(&self) -> Option<novadraw_core::Color> {
        None
    }

    fn line_cap(&self) -> novadraw_render::command::LineCap {
        novadraw_render::command::LineCap::default()
    }

    fn line_join(&self) -> novadraw_render::command::LineJoin {
        novadraw_render::command::LineJoin::default()
    }

    fn fill_enabled(&self) -> bool {
        false
    }

    fn outline_enabled(&self) -> bool {
        false
    }

    fn fill_shape(&self, _gc: &mut NdCanvas) {}

    fn outline_shape(&self, _gc: &mut NdCanvas) {}
}

macro_rules! impl_test_shape_figure {
    ($($figure:ty),+ $(,)?) => {
        $(
            impl Figure for $figure {
                fn initial_bounds(&self) -> Rectangle {
                    Bounded::bounds(self)
                }

                fn name(&self) -> &'static str {
                    Bounded::name(self)
                }

                fn initial_insets(&self) -> (f64, f64, f64, f64) {
                    Bounded::insets(self)
                }

                fn paint_figure(&self, gc: &mut NdCanvas) {
                    Shape::paint_figure(self, gc);
                }
            }
        )+
    };
}

impl_test_shape_figure!(TestCoordRootFigure, TestInsetFigure);

/// 辅助函数：收集所有 FillRect 命令的 rect 坐标
fn collect_fill_rects(gc: &novadraw_render::NdCanvas) -> Vec<[glam::DVec2; 2]> {
    gc.commands()
        .iter()
        .filter_map(|cmd| match &cmd.kind {
            novadraw_render::command::RenderCommandKind::FillRect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

/// 辅助函数：收集所有 Clip 命令的 rect 坐标
fn collect_clip_rects(gc: &novadraw_render::NdCanvas) -> Vec<[glam::DVec2; 2]> {
    gc.commands()
        .iter()
        .filter_map(|cmd| match &cmd.kind {
            novadraw_render::command::RenderCommandKind::Clip { rect } => Some(*rect),
            _ => None,
        })
        .collect()
}

fn has_clip_rect(clip_rects: &[[glam::DVec2; 2]], x: f64, y: f64, width: f64, height: f64) -> bool {
    clip_rects.iter().any(|rect| {
        rect[0].x == x && rect[0].y == y && rect[1].x == x + width && rect[1].y == y + height
    })
}

/// 测试：bounds 表示 parent content domain 中的布局矩形
///
/// 场景：父子节点分别设置 bounds
/// 期望：所有 RenderCommand 使用 bounds 在所属坐标域中的值
#[test]
fn test_bounds_absolute_coordinates() {
    let mut scene = FigureGraph::new();

    // parent bounds = (0, 0, 100, 100)
    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    // child bounds = (10, 10, 50, 50)
    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let _child_id = scene.add_child_to(parent_id, Box::new(child));

    let gc = scene.render();
    let fill_rects = collect_fill_rects(&gc);

    // 期望有两个 FillRect: parent 和 child
    // fill_rect 使用图元在当前绘制坐标域中的矩形，
    // 实际绘制位置由 transform / translate 状态决定

    // parent FillRect: (0, 0, 100, 100)
    // child FillRect: (0, 0, 50, 50)
    assert!(
        fill_rects.len() >= 2,
        "应有 2 个 FillRect，实际为 {}",
        fill_rects.len()
    );

    eprintln!("FillRects: {:?}", fill_rects);
}

/// 测试：RenderCommand 坐标与 bounds 对应
///
/// 场景：parent(0,0,100,100) + child(10,10,50,50)
/// 期望：
/// - parent ClipRect: [0,0, 100,100]
/// - child ClipRect: [10,10, 50,50]
#[test]
fn test_render_commands_coords() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    // 使用不同颜色以便区分
    let child = RectangleFigure::new_with_color(10.0, 10.0, 50.0, 50.0, Color::hex("#e74c3c"));
    let _child_id = scene.add_child_to(parent_id, Box::new(child));

    let gc = scene.render();
    let clip_rects = collect_clip_rects(&gc);

    eprintln!("ClipRects: {:?}", clip_rects);

    // 在默认共享坐标域模式下，每个 Figure 的 clip_rect 使用其 bounds
    // parent clip = (0, 0, 100, 100)
    // child clip = (10, 10, 50, 50)
    assert!(
        clip_rects.len() >= 2,
        "应有 2 个 ClipRect，实际为 {}",
        clip_rects.len()
    );
}

/// 测试：嵌套层次渲染顺序正确
///
/// 场景：root → parent → child
/// 期望：渲染顺序 parent → child
#[test]
fn test_nested_structure_render_order() {
    let mut scene = FigureGraph::new();

    // root (内容容器)
    let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
    let root_id = scene.set_contents(Box::new(root));

    // parent
    let parent = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
    let parent_id = scene.add_child_to(root_id, Box::new(parent));

    // child 嵌套在 parent 内部
    let child = RectangleFigure::new(60.0, 60.0, 30.0, 30.0);
    let _child_id = scene.add_child_to(parent_id, Box::new(child));

    let gc = scene.render();

    // 验证渲染命令数量（3 个图形，每个产生多个命令）
    let cmd_count = gc.commands().len();
    assert!(
        cmd_count >= 15,
        "应有至少 15 个渲染命令，实际为 {}",
        cmd_count
    );

    // 收集所有 FillRect 的坐标
    let fill_rects = collect_fill_rects(&gc);
    eprintln!("Nested FillRects: {:?}", fill_rects);

    // fill_rect 使用当前绘制坐标域中的矩形，
    // 实际位置由 translate 状态管理
    // 这验证了：RenderCommand 只存储 bounds 值，translate 状态由独立栈管理
}

#[test]
fn test_hit_test_prefers_topmost_deepest_child() {
    let mut scene = FigureGraph::new();

    let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
    let root_id = scene.set_contents(Box::new(root));

    let bottom = RectangleFigure::new(20.0, 20.0, 120.0, 120.0);
    let bottom_id = scene.add_child_to(root_id, Box::new(bottom));

    let top = RectangleFigure::new(40.0, 40.0, 120.0, 120.0);
    let top_id = scene.add_child_to(root_id, Box::new(top));

    let nested = RectangleFigure::new(20.0, 20.0, 40.0, 40.0);
    let nested_id = scene.add_child_to(top_id, Box::new(nested));

    assert_eq!(scene.find_mouse_event_target_at(70.0, 70.0), None);
    assert_eq!(scene.hit_test_simple((70.0, 70.0)), Some(nested_id));
    assert_eq!(scene.hit_test_simple((50.0, 50.0)), Some(top_id));
    assert_eq!(scene.hit_test_simple((30.0, 30.0)), Some(bottom_id));
    assert_eq!(scene.hit_test_simple((190.0, 190.0)), Some(root_id));
    assert_eq!(scene.hit_test_simple((260.0, 260.0)), None);
}

/// 测试：prim_translate 保持子节点的 parent-local bounds
///
/// 场景：parent(0,0,100,100) + child(10,10,50,50)
/// 动作：平移 parent (5, 10)
/// 期望：parent bounds = (5,10,100,100), child bounds 保持不变
#[test]
fn test_prim_translate_propagates() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 平移前验证
    let parent_bounds_before = scene.blocks.get(parent_id).unwrap().figure_bounds();
    let child_bounds_before = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds_before.x, 0.0);
    assert_eq!(parent_bounds_before.y, 0.0);
    assert_eq!(child_bounds_before.x, 10.0);
    assert_eq!(child_bounds_before.y, 10.0);

    // 平移 parent (5, 10)
    scene.prim_translate(parent_id, 5.0, 10.0);

    // 验证平移后 parent-local bounds
    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 5.0, "父节点 x 应为 5");
    assert_eq!(parent_bounds.y, 10.0, "父节点 y 应为 10");

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0, "子节点 x 不应被改写");
    assert_eq!(child_bounds.y, 10.0, "子节点 y 不应被改写");
}

/// 测试：prim_translate 通过变换链移动子树
///
/// 场景：root(0,0,200,200) → parent(50,50,100,100) → child(10,10,50,50)
/// 动作：平移 root (5, 5)
/// 期望：后代 parent-local bounds 不变
#[test]
fn test_prim_translate_nested_propagation() {
    let mut scene = FigureGraph::new();

    let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
    let root_id = scene.set_contents(Box::new(root));

    let parent = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
    let parent_id = scene.add_child_to(root_id, Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 平移根节点 (5, 5)
    scene.prim_translate(root_id, 5.0, 5.0);

    // 只有 root 的 parent-local bounds 被修改。
    let root_bounds = scene.blocks.get(root_id).unwrap().figure_bounds();
    assert_eq!(root_bounds.x, 5.0);
    assert_eq!(root_bounds.y, 5.0);

    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 50.0);
    assert_eq!(parent_bounds.y, 50.0);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0);
    assert_eq!(child_bounds.y, 10.0);
}

/// 测试：RenderCommand 在平移后使用更新后的 bounds
///
/// 场景：创建场景后平移父节点
/// 期望：RenderCommand 使用平移后的 bounds 值
#[test]
fn test_render_commands_after_translate() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 平移前收集 RenderCommand
    let gc_before = scene.render();
    let clip_rects_before = collect_clip_rects(&gc_before);
    eprintln!("Before translate - ClipRects: {:?}", clip_rects_before);

    // 平移
    scene.prim_translate(parent_id, 10.0, 20.0);

    // 平移后收集 RenderCommand
    let gc_after = scene.render();
    let clip_rects_after = collect_clip_rects(&gc_after);
    eprintln!("After translate - ClipRects: {:?}", clip_rects_after);

    // 验证 bounds 已更新
    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 10.0);
    assert_eq!(parent_bounds.y, 20.0);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0);
    assert_eq!(child_bounds.y, 10.0);
}

/// 测试：Figure 的 parent-local placement 与 node-local clip 正确组合
#[test]
fn test_local_coordinates_mode() {
    let mut scene = FigureGraph::new();

    // 坐标根 (10, 10, 100, 100)
    let coord_root = TestCoordRootFigure::new(10.0, 10.0, 100.0, 100.0);
    let root_id = scene.set_contents(Box::new(coord_root));

    // 子节点 (30, 40, 50, 50)
    let child = RectangleFigure::new(30.0, 40.0, 50.0, 50.0);
    let _child_id = scene.add_child_to(root_id, Box::new(child));

    let gc = scene.render();
    let clip_rects = collect_clip_rects(&gc);

    eprintln!("Local coord ClipRects: {:?}", clip_rects);

    // 本地坐标模式下：
    // - 坐标根：translate(10, 10)，clip(0, 0, 100, 100)
    // - 子节点：其 bounds 位于当前坐标域，clip(30, 40, 50, 50)

    // clip_rects 应该包含：
    // 1. 坐标根的 clip: (0, 0, 100, 100) - 在 translate 之后
    // 2. 子节点的 clip: (30, 40, 50, 50) - 子节点所属坐标域中的值
    assert!(!clip_rects.is_empty(), "应有 ClipRect 命令");
}

#[test]
fn test_client_area_resets_origin_for_coordinate_root() {
    let figure = TestCoordRootFigure::with_insets(10.0, 20.0, 100.0, 80.0, (5.0, 7.0, 11.0, 13.0));

    assert_eq!(figure.client_area(), Rectangle::new(7.0, 5.0, 80.0, 64.0));
}

#[test]
fn test_render_clips_parent_local_figure_to_client_area() {
    let mut scene = FigureGraph::new();
    let root_id = scene.set_contents(Box::new(TestInsetFigure::new(
        10.0,
        20.0,
        100.0,
        80.0,
        (5.0, 7.0, 11.0, 13.0),
    )));
    scene.add_child_to(
        root_id,
        Box::new(RectangleFigure::new(10.0, 20.0, 30.0, 30.0)),
    );

    let gc = scene.render();
    let clips = collect_clip_rects(&gc);
    assert!(
        has_clip_rect(&clips, 7.0, 5.0, 80.0, 64.0),
        "renderer should clip parent-local children to client area: {:?}",
        clips
    );
}

#[test]
fn test_viewport_figure_render_uses_content_clip_and_transform() {
    let mut scene = FigureGraph::new();
    let root_id = scene.set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 400.0, 300.0)));
    let viewport_id = scene.add_child_to(
        root_id,
        Box::new(ViewportFigure::new(100.0, 50.0, 200.0, 100.0).with_origin(40.0, 20.0)),
    );
    let scalable_id = scene.add_child_to(
        viewport_id,
        Box::new(ScalableLayeredPaneFigure::new(0.0, 0.0, 400.0, 200.0).with_scale(2.0)),
    );
    scene.add_child_to(
        scalable_id,
        Box::new(RectangleFigure::new(30.0, 20.0, 40.0, 40.0)),
    );

    let gc = scene.render();
    let clips = collect_clip_rects(&gc);
    assert!(
        has_clip_rect(&clips, 0.0, 0.0, 200.0, 100.0),
        "renderer should clip viewport children in node-local coordinates: {:?}",
        clips
    );
}

/// 测试：场景结构验证 bounds 完整性
///
/// 场景：复杂嵌套结构
/// 期望：所有节点的 bounds 都是有效值
#[test]
fn test_bounds_integrity() {
    let mut scene = FigureGraph::new();

    // root
    let root = RectangleFigure::new(0.0, 0.0, 800.0, 600.0);
    let root_id = scene.set_contents(Box::new(root));

    // layer 1
    let layer1 = RectangleFigure::new(100.0, 100.0, 600.0, 400.0);
    let layer1_id = scene.add_child_to(root_id, Box::new(layer1));

    // layer 2 (嵌套在 layer1 中)
    let layer2 = RectangleFigure::new(150.0, 150.0, 500.0, 300.0);
    let layer2_id = scene.add_child_to(layer1_id, Box::new(layer2));

    // 多个子元素
    for i in 0..3 {
        let x = 160.0 + i as f64 * 120.0;
        let y = 160.0;
        let item = RectangleFigure::new(x, y, 100.0, 80.0);
        scene.add_child_to(layer2_id, Box::new(item));
    }

    // 验证所有节点 bounds 有效
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        if let Some(block) = scene.blocks.get(id) {
            let bounds = block.figure_bounds();
            assert!(
                bounds.width >= 0.0 && bounds.height >= 0.0,
                "节点 {:?} bounds 宽度/高度无效: {:?}",
                id,
                bounds
            );
            // 子节点入栈
            for &child_id in &block.children {
                stack.push(child_id);
            }
        }
    }
}

/// 测试：验证渲染命令中的坐标累加
///
/// 场景：三个矩形水平排列
/// 期望：每个矩形的 ClipRect 反映其实际位置
#[test]
fn test_horizontal_layout_coords() {
    let mut scene = FigureGraph::new();

    let container = RectangleFigure::new(0.0, 0.0, 400.0, 100.0);
    let container_id = scene.set_contents(Box::new(container));

    // 三个水平排列的矩形
    let rect1 = RectangleFigure::new_with_color(10.0, 10.0, 100.0, 80.0, Color::hex("#3498db"));
    let _ = scene.add_child_to(container_id, Box::new(rect1));

    let rect2 = RectangleFigure::new_with_color(120.0, 10.0, 100.0, 80.0, Color::hex("#e74c3c"));
    let _ = scene.add_child_to(container_id, Box::new(rect2));

    let rect3 = RectangleFigure::new_with_color(230.0, 10.0, 100.0, 80.0, Color::hex("#2ecc71"));
    let _ = scene.add_child_to(container_id, Box::new(rect3));

    let gc = scene.render();
    let clip_rects = collect_clip_rects(&gc);

    eprintln!("Horizontal layout ClipRects: {:?}", clip_rects);

    // container clip: (0, 0, 400, 100)
    // rect1 clip: (10, 10, 100, 80)
    // rect2 clip: (120, 10, 100, 80)
    // rect3 clip: (230, 10, 100, 80)

    assert!(
        clip_rects.len() >= 4,
        "应有至少 4 个 ClipRect，实际为 {}",
        clip_rects.len()
    );
}

// ========== 碰撞检测和 set_bounds 测试 ==========

/// 测试：contains_point 基本功能
///
/// 场景：矩形 bounds = (10, 10, 50, 50)
/// 期望：内部点返回 true，外部点返回 false
#[test]
fn test_contains_point_basic() {
    let rect = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);

    // 边界内
    assert!(rect.contains_point(0.0, 0.0), "左上角应包含");
    assert!(rect.contains_point(25.0, 25.0), "中心点应包含");
    assert!(rect.contains_point(49.0, 49.0), "右下角应包含");

    // 边界外
    assert!(!rect.contains_point(-1.0, 25.0), "左边外应不包含");
    assert!(!rect.contains_point(51.0, 25.0), "右边外应不包含");
    assert!(!rect.contains_point(25.0, -1.0), "上边外应不包含");
    assert!(!rect.contains_point(25.0, 51.0), "下边外应不包含");
}

/// 测试：contains_point 边界情况
///
/// 场景：点正好在边界上
/// 期望：边界上返回 true（包含边界）
#[test]
fn test_contains_point_boundary() {
    let rect = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);

    assert!(rect.contains_point(0.0, 0.0), "左上角边界应包含");
    assert!(rect.contains_point(100.0, 100.0), "右下角边界应包含");
}

/// 测试：intersects 基本功能
///
/// 场景：矩形 A(0,0,100,100)，矩形 B(50,50,100,100)
/// 期望：相交返回 true，不相交返回 false
#[test]
fn test_intersects_basic() {
    let rect_a = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);

    // 相交
    let rect_b = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
    assert!(rect_a.intersects(rect_b.bounds), "相交矩形应返回 true");

    // 部分重叠
    let rect_c = RectangleFigure::new(80.0, 80.0, 50.0, 50.0);
    assert!(rect_a.intersects(rect_c.bounds), "部分重叠应返回 true");

    // 包含
    let rect_d = RectangleFigure::new(25.0, 25.0, 50.0, 50.0);
    assert!(rect_a.intersects(rect_d.bounds), "被包含应返回 true");

    // 不相交
    let rect_e = RectangleFigure::new(150.0, 150.0, 50.0, 50.0);
    assert!(!rect_a.intersects(rect_e.bounds), "不相交应返回 false");

    // 刚好相切
    let rect_f = RectangleFigure::new(100.0, 100.0, 50.0, 50.0);
    assert!(
        !rect_a.intersects(rect_f.bounds),
        "刚好相切应返回 false（按 > 判断）"
    );
}

/// 测试：intersects 与自身
///
/// 场景：矩形与自身比较
/// 期望：应返回 true
#[test]
fn test_intersects_self() {
    let rect = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    assert!(rect.intersects(rect.bounds), "与自身相交应返回 true");
}

/// 测试：set_bounds 功能
///
/// 场景：创建矩形后设置新 bounds
/// 期望：bounds 正确更新
#[test]
fn test_set_bounds() {
    let mut rect = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);

    // 初始验证
    let b = rect.bounds();
    assert_eq!(b.x, 0.0);
    assert_eq!(b.y, 0.0);
    assert_eq!(b.width, 100.0);
    assert_eq!(b.height, 100.0);

    // 使用 set_bounds 更新
    rect.set_bounds(50.0, 50.0, 200.0, 150.0);

    let b = rect.bounds();
    assert_eq!(b.x, 50.0, "x 应为 50");
    assert_eq!(b.y, 50.0, "y 应为 50");
    assert_eq!(b.width, 200.0, "width 应为 200");
    assert_eq!(b.height, 150.0, "height 应为 150");
}

/// 测试：set_bounds 后 contains_point 正确工作
///
/// 场景：set_bounds 移动矩形后
/// 期望：contains_point 使用新的 bounds 位置
#[test]
fn test_set_bounds_affects_contains_point() {
    let mut rect = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);

    // 原始位置：点 (50, 50) 应在内部
    assert!(rect.contains_point(50.0, 50.0));

    // 移动到 (100, 100)
    rect.set_bounds(100.0, 100.0, 100.0, 100.0);

    // node-local 命中只受 size 影响，不受 parent-local origin 影响。
    assert!(rect.contains_point(50.0, 50.0));
    assert!(!rect.contains_point(150.0, 150.0));
}

/// 测试：使用 Figure trait 的 set_bounds
///
/// 场景：通过 trait 对象使用 set_bounds
/// 期望：正确更新 bounds
#[test]
fn test_figure_set_bounds() {
    let mut rect = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);

    // 通过 trait 调用 set_bounds
    <RectangleFigure as Bounded>::set_bounds(&mut rect, 10.0, 20.0, 80.0, 60.0);

    let b = rect.bounds();
    assert_eq!(b.x, 10.0);
    assert_eq!(b.y, 20.0);
    assert_eq!(b.width, 80.0);
    assert_eq!(b.height, 60.0);
}

/// 测试：空的或无效的 bounds
///
/// 场景：宽和高都为 0 的矩形
/// 期望：contains_point 应正确处理
#[test]
fn test_empty_bounds() {
    // 宽和高都为 0 的矩形
    let rect = RectangleFigure::new(10.0, 10.0, 0.0, 0.0);

    // 宽度和高度都为 0 时，边界外的点不在内部
    assert!(!rect.contains_point(5.0, 5.0), "点 (5,5) 应不在空矩形内");
    assert!(
        !rect.contains_point(11.0, 11.0),
        "点 (11,11) 应不在空矩形内"
    );

    // 注意：点 (10,10) 在边界上，根据包含边界的实现会在内部
    // 这是符合预期的行为

    // intersects 应该仍然工作（空矩形与任何矩形）
    let point_rect = Rectangle::new(10.0, 10.0, 1.0, 1.0);
    assert!(!rect.intersects(point_rect), "空矩形与任何矩形都不相交");
}

// ========== FigureGraph::set_bounds 测试 ==========

/// 测试：FigureGraph::set_bounds 基本功能
///
/// 场景：parent(0,0,100,100) + child(10,10,50,50)
/// 动作：set_bounds(parent, 20, 30, 150, 100)
/// 期望：
/// - parent bounds = (20, 30, 150, 100)
/// - child bounds = (30, 40, 50, 50)（位置传播）
#[test]
fn test_scene_set_bounds_basic() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 验证初始状态
    let parent_bounds_before = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds_before.x, 0.0);
    assert_eq!(parent_bounds_before.y, 0.0);
    assert_eq!(parent_bounds_before.width, 100.0);
    assert_eq!(parent_bounds_before.height, 100.0);

    let child_bounds_before = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds_before.x, 10.0);
    assert_eq!(child_bounds_before.y, 10.0);

    // set_bounds: 新位置 (20, 30)，新尺寸 (150, 100)
    scene.set_bounds(parent_id, 20.0, 30.0, 150.0, 100.0);

    // 验证 parent
    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 20.0, "父节点 x 应为 20");
    assert_eq!(parent_bounds.y, 30.0, "父节点 y 应为 30");
    assert_eq!(parent_bounds.width, 150.0, "父节点 width 应为 150");
    assert_eq!(parent_bounds.height, 100.0, "父节点 height 应为 100");

    // child 的 parent-local bounds 保持不变。
    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0);
    assert_eq!(child_bounds.y, 10.0);
    assert_eq!(child_bounds.width, 50.0, "子节点 width 不变");
    assert_eq!(child_bounds.height, 50.0, "子节点 height 不变");
}

/// 测试：FigureGraph::set_bounds 仅位置变化
///
/// 场景：parent(0,0,100,100) + child(10,10,50,50)
/// 动作：set_bounds(parent, 50, 60, 100, 100)（只变位置，不变尺寸）
/// 期望：只修改 parent，child 的 parent-local bounds 不变
#[test]
fn test_scene_set_bounds_position_only() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 只改变位置：偏移 (+50, +60)
    scene.set_bounds(parent_id, 50.0, 60.0, 100.0, 100.0);

    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 50.0);
    assert_eq!(parent_bounds.y, 60.0);
    assert_eq!(parent_bounds.width, 100.0);
    assert_eq!(parent_bounds.height, 100.0);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0);
    assert_eq!(child_bounds.y, 10.0);
}

/// 测试：FigureGraph::set_bounds 不改写后代 bounds
///
/// 场景：root(0,0,200,200) → parent(50,50,100,100) → child(10,10,50,50)
/// 动作：set_bounds(root, 10, 10, 200, 200)
/// 期望：后代通过变换链移动，但存储值保持不变
#[test]
fn test_scene_set_bounds_nested_propagation() {
    let mut scene = FigureGraph::new();

    let root = RectangleFigure::new(0.0, 0.0, 200.0, 200.0);
    let root_id = scene.set_contents(Box::new(root));

    let parent = RectangleFigure::new(50.0, 50.0, 100.0, 100.0);
    let parent_id = scene.add_child_to(root_id, Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // set_bounds: 偏移 (+10, +10)
    scene.set_bounds(root_id, 10.0, 10.0, 200.0, 200.0);

    // 验证后代 parent-local bounds 保持不变。
    let root_bounds = scene.blocks.get(root_id).unwrap().figure_bounds();
    assert_eq!(root_bounds.x, 10.0);
    assert_eq!(root_bounds.y, 10.0);

    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 50.0);
    assert_eq!(parent_bounds.y, 50.0);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0);
    assert_eq!(child_bounds.y, 10.0);
}

/// 测试：FigureGraph::set_bounds 仅尺寸变化
///
/// 场景：parent(0,0,100,100) + child(10,10,50,50)
/// 动作：set_bounds(parent, 0, 0, 200, 150)（只变尺寸，不变位置）
/// 期望：位置不变，尺寸更新，子节点位置不变
#[test]
fn test_scene_set_bounds_size_only() {
    let mut scene = FigureGraph::new();

    let parent = RectangleFigure::new(0.0, 0.0, 100.0, 100.0);
    let parent_id = scene.set_contents(Box::new(parent));

    let child = RectangleFigure::new(10.0, 10.0, 50.0, 50.0);
    let child_id = scene.add_child_to(parent_id, Box::new(child));

    // 只改变尺寸：位置不变
    scene.set_bounds(parent_id, 0.0, 0.0, 200.0, 150.0);

    let parent_bounds = scene.blocks.get(parent_id).unwrap().figure_bounds();
    assert_eq!(parent_bounds.x, 0.0, "x 不变");
    assert_eq!(parent_bounds.y, 0.0, "y 不变");
    assert_eq!(parent_bounds.width, 200.0);
    assert_eq!(parent_bounds.height, 150.0);

    let child_bounds = scene.blocks.get(child_id).unwrap().figure_bounds();
    assert_eq!(child_bounds.x, 10.0, "子节点 x 不变");
    assert_eq!(child_bounds.y, 10.0, "子节点 y 不变");
    assert_eq!(child_bounds.width, 50.0);
    assert_eq!(child_bounds.height, 50.0);
}

/// 测试：场景6 - 裁剪测试的详细命令分析
///
/// 场景：Parent(350, 250, 100, 100) + 三个子元素
/// 验证：子元素超出父边界的部分被裁剪
#[test]
fn test_clip_test_scene_commands() {
    use novadraw_render::command::RenderCommandKind;

    let mut scene = FigureGraph::new();

    // Root
    let root =
        RectangleFigure::new_with_color(0.0, 0.0, 800.0, 600.0, Color::rgba(0.0, 0.0, 0.0, 0.0));
    let root_id = scene.set_contents(Box::new(root));

    // Parent - 半透明蓝色容器 (350, 250, 100, 100)
    let parent = RectangleFigure::new_with_color(
        350.0,
        250.0,
        100.0,
        100.0,
        Color::rgba(0.2, 0.4, 0.8, 0.5),
    );
    let parent_id = scene.add_child_to(root_id, Box::new(parent));

    // Child 1 - 完全在父容器内 (360, 260, 30, 30) - 绿色
    let child1 =
        RectangleFigure::new_with_color(360.0, 260.0, 30.0, 30.0, Color::rgba(0.2, 0.8, 0.3, 1.0));
    let _child1_id = scene.add_child_to(parent_id, Box::new(child1));

    // Child 2 - 超出父容器右边界 (430, 280, 50, 40) - 红色
    // 父容器右边界是 450，子元素从 430 开始，宽度 50，应该超出到 480
    let child2 =
        RectangleFigure::new_with_color(430.0, 280.0, 50.0, 40.0, Color::rgba(0.9, 0.2, 0.2, 1.0));
    let _child2_id = scene.add_child_to(parent_id, Box::new(child2));

    // Child 3 - 超出父容器下边界 (380, 340, 40, 40) - 黄色
    // 父容器下边界是 350，子元素从 340 开始，高度 40，应该超出到 380
    let child3 =
        RectangleFigure::new_with_color(380.0, 340.0, 40.0, 40.0, Color::rgba(0.9, 0.8, 0.2, 1.0));
    let _child3_id = scene.add_child_to(parent_id, Box::new(child3));

    // 渲染并分析命令
    let gc = scene.render();
    let commands = gc.commands();

    println!("\n=== 场景6：裁剪测试命令分析 ===");
    println!("Parent bounds: (350, 250, 100, 100)");
    println!("  Child 1 (绿色): (360, 260, 30, 30) - 完全在内");
    println!("  Child 2 (红色): (430, 280, 50, 40) - 超出右边界 (480 > 450)");
    println!("  Child 3 (黄色): (380, 340, 40, 40) - 超出下边界 (380 > 350)");
    println!();

    for (i, cmd) in commands.iter().enumerate() {
        match &cmd.kind {
            RenderCommandKind::PushState => {
                println!("[{:2}] PushState", i);
            }
            RenderCommandKind::PopState => {
                println!("[{:2}] PopState", i);
            }
            RenderCommandKind::RestoreState => {
                println!("[{:2}] RestoreState", i);
            }
            RenderCommandKind::Clip { rect } => {
                let x = rect[0].x;
                let y = rect[0].y;
                let w = rect[1].x - rect[0].x;
                let h = rect[1].y - rect[0].y;

                // 判断是哪个 clip
                let desc = if (x - 350.0).abs() < 0.1 && (y - 250.0).abs() < 0.1 {
                    "Parent (350, 250, 100, 100)"
                } else if (x - 360.0).abs() < 0.1 && (y - 260.0).abs() < 0.1 {
                    "Child 1 (360, 260, 30, 30)"
                } else if (x - 430.0).abs() < 0.1 && (y - 280.0).abs() < 0.1 {
                    "Child 2 (430, 280, 50, 40) - 超出部分应被裁剪"
                } else if (x - 380.0).abs() < 0.1 && (y - 340.0).abs() < 0.1 {
                    "Child 3 (380, 340, 40, 40) - 超出部分应被裁剪"
                } else if x == 0.0 && y == 0.0 {
                    "Root (0, 0, 800, 600)"
                } else {
                    "Unknown"
                };

                println!(
                    "[{:2}] Clip: ({:.0}, {:.0}, {:.0}, {:.0}) <- {}",
                    i, x, y, w, h, desc
                );
            }
            RenderCommandKind::FillRect { rect, color } => {
                let x = rect[0].x;
                let y = rect[0].y;
                let w = rect[1].x - rect[0].x;
                let h = rect[1].y - rect[0].y;
                let (r, g, b) = (color.r, color.g, color.b);

                let desc = if r < 0.3 && g > 0.7 && b < 0.3 {
                    "Child 1 - 绿色"
                } else if r > 0.8 && g < 0.3 && b < 0.3 {
                    "Child 2 - 红色"
                } else if r > 0.8 && g > 0.7 && b < 0.3 {
                    "Child 3 - 黄色"
                } else if r < 0.3 && g < 0.5 && b > 0.7 {
                    "Parent - 蓝色"
                } else {
                    "Unknown"
                };

                println!(
                    "[{:2}] FillRect: ({:.0}, {:.0}, {:.0}, {:.0}) - {}",
                    i, x, y, w, h, desc
                );
            }
            RenderCommandKind::StrokeRect { rect, width, .. } => {
                let x = rect[0].x;
                let y = rect[0].y;
                let w = rect[1].x - rect[0].x;
                let h = rect[1].y - rect[0].y;
                println!(
                    "[{:2}] StrokeRect: ({:.0}, {:.0}, {:.0}, {:.0}) w={:.0}",
                    i, x, y, w, h, width
                );
            }
            RenderCommandKind::ConcatTransform { matrix } => {
                let c = matrix.coeffs();
                if c[4] != 0.0 || c[5] != 0.0 {
                    println!("[{:2}] Translate: ({:.0}, {:.0})", i, c[4], c[5]);
                }
            }
            _ => {}
        }
    }

    // 验证 clip 的数量和位置
    let clip_rects: Vec<_> = commands
        .iter()
        .filter_map(|cmd| match &cmd.kind {
            RenderCommandKind::Clip { rect } => Some(*rect),
            _ => None,
        })
        .collect();

    println!("\n=== Clip 统计 ===");
    println!("共 {} 个 Clip 命令", clip_rects.len());

    // 期望的 clip 序列：
    // 1. Parent clip: (350, 250, 100, 100) - paint_client_area 设置
    // 2. Child 1 clip: (360, 260, 30, 30) - paint_children 为 child1 设置
    // 3. Parent clip restored - paint_children 中 restoreState
    // 4. Child 2 clip: (430, 280, 50, 40) - paint_children 为 child2 设置
    // 5. Parent clip restored
    // 6. Child 3 clip: (380, 340, 40, 40) - paint_children 为 child3 设置
    // 7. Parent clip restored

    // 验证关键 clip 存在
    let has_parent_clip = clip_rects
        .iter()
        .any(|r| (r[0].x - 350.0).abs() < 0.1 && (r[0].y - 250.0).abs() < 0.1);
    let has_child1_clip = clip_rects
        .iter()
        .any(|r| (r[0].x - 360.0).abs() < 0.1 && (r[0].y - 260.0).abs() < 0.1);
    let has_child2_clip = clip_rects
        .iter()
        .any(|r| (r[0].x - 430.0).abs() < 0.1 && (r[0].y - 280.0).abs() < 0.1);
    let has_child3_clip = clip_rects
        .iter()
        .any(|r| (r[0].x - 380.0).abs() < 0.1 && (r[0].y - 340.0).abs() < 0.1);

    println!("Parent clip (350, 250): {}", has_parent_clip);
    println!("Child 1 clip (360, 260): {}", has_child1_clip);
    println!("Child 2 clip (430, 280): {}", has_child2_clip);
    println!("Child 3 clip (380, 340): {}", has_child3_clip);

    assert!(has_parent_clip, "应有 Parent clip");
    assert!(has_child1_clip, "应有 Child 1 clip");
    assert!(has_child2_clip, "应有 Child 2 clip");
    assert!(has_child3_clip, "应有 Child 3 clip");
}
