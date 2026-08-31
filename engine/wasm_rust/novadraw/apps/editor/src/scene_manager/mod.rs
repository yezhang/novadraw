use novadraw::{Color, EllipseFigure, FigureGraph, PolylineFigure, Rectangle, RectangleFigure};

mod interactive_figure;
pub mod scene_host;

use interactive_figure::InteractiveRectFigure;

pub struct SceneManager {
    pub scene: FigureGraph,
    /// 当前激活的场景类型
    pub current_scene: SceneType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneType {
    BasicAnchors,    // 场景0：基础四个定位点
    Nested,          // 场景1：嵌套父子结构
    NestedWithRoot,  // 场景2：嵌套场景（含透明根节点）
    ZOrder,          // 场景3：Z-order 叠加
    Visibility,      // 场景4：不可见节点过滤
    BoundsTranslate, // 场景5：prim_translate 平移传播
    ClipTest,        // 场景6：裁剪测试（子元素超出父边界）
    EllipseTest,     // 场景7：椭圆图形测试
    LineTest,        // 场景8：直线图形测试
    DpiTest,         // 场景9：DPI 坐标验证
}

pub const DPI_TEST_PROBE_BOUNDS: Rectangle = Rectangle {
    x: 100.0,
    y: 100.0,
    width: 100.0,
    height: 100.0,
};

impl SceneManager {
    /// 创建默认场景（场景9：DPI 坐标验证）
    pub fn new() -> Self {
        Self::with_scene(SceneType::DpiTest)
    }

    /// 根据场景类型创建场景
    pub fn with_scene(scene_type: SceneType) -> Self {
        let mut scene = FigureGraph::new();

        match scene_type {
            SceneType::BasicAnchors => Self::create_basic_anchors_scene(&mut scene),
            SceneType::Nested => Self::create_nested_scene(&mut scene),
            SceneType::NestedWithRoot => Self::create_nested_with_root_scene(&mut scene),
            SceneType::ZOrder => Self::create_zorder_scene(&mut scene),
            SceneType::Visibility => Self::create_visibility_scene(&mut scene),
            SceneType::BoundsTranslate => Self::create_bounds_translate_scene(&mut scene),
            SceneType::ClipTest => Self::create_clip_test_scene(&mut scene),
            SceneType::EllipseTest => Self::create_ellipse_test_scene(&mut scene),
            SceneType::LineTest => Self::create_line_test_scene(&mut scene),
            SceneType::DpiTest => Self::create_dpi_test_scene(&mut scene),
        }

        Self {
            scene,
            current_scene: scene_type,
        }
    }

    /// 场景 0：基础四个定位点（只有四个小正方形）
    ///
    /// 用于验证最基本的渲染逻辑
    fn create_basic_anchors_scene(scene: &mut FigureGraph) {
        // 创建一个基准矩形作为父容器（灰色边框）
        let root_fig = RectangleFigure::new_with_color(
            100.0,
            100.0,
            600.0,
            400.0,
            Color::rgba(0.3, 0.3, 0.3, 1.0),
        );
        let root_bounds = root_fig.bounds;
        let parent_id = scene.set_contents(Box::new(root_fig));

        // 角点手柄大小
        let handle_size = 20.0;

        // 左上角 - 红色小正方形
        let rect_tl = RectangleFigure::new_with_color(
            root_bounds.x,
            root_bounds.y,
            handle_size,
            handle_size,
            Color::rgba(0.9, 0.2, 0.2, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(rect_tl));

        // 右上角 - 绿色小正方形
        let rect_tr = RectangleFigure::new_with_color(
            root_bounds.x + root_bounds.width - handle_size,
            root_bounds.y,
            handle_size,
            handle_size,
            Color::rgba(0.2, 0.8, 0.3, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(rect_tr));

        // 左下角 - 蓝色小正方形
        let rect_bl = RectangleFigure::new_with_color(
            root_bounds.x,
            root_bounds.y + root_bounds.height - handle_size,
            handle_size,
            handle_size,
            Color::rgba(0.2, 0.4, 0.9, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(rect_bl));

        // 右下角 - 黄色小正方形
        let rect_br = RectangleFigure::new_with_color(
            root_bounds.x + root_bounds.width - handle_size,
            root_bounds.y + root_bounds.height - handle_size,
            handle_size,
            handle_size,
            Color::rgba(0.9, 0.8, 0.2, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(rect_br));
    }

    /// 场景 4：prim_translate 平移传播测试
    ///
    /// 验证：`prim_translate` 平移操作会传播到所有子节点
    fn create_bounds_translate_scene(scene: &mut FigureGraph) {
        // Parent - 深紫容器
        let parent = RectangleFigure::new_with_color(
            200.0,
            150.0,
            300.0,
            200.0,
            Color::rgba(0.4, 0.2, 0.5, 1.0),
        );
        let parent_id = scene.set_contents(Box::new(parent));

        // Child - 橙色（相对于父节点）
        let child = RectangleFigure::new_with_color(
            30.0,
            30.0,
            100.0,
            80.0,
            Color::rgba(0.9, 0.5, 0.1, 1.0),
        );
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // Grandchild - 青色（选中状态）
        let grandchild = RectangleFigure::new_with_color(
            20.0,
            20.0,
            40.0,
            30.0,
            Color::rgba(0.1, 0.8, 0.8, 1.0),
        );
        let gc_id = scene.add_child_to(child_id, Box::new(grandchild));

        // 选中 Grandchild
        scene.set_selected(Some(gc_id));
    }

    /// 场景 3：不可见节点过滤测试
    ///
    /// 验证：`is_visible = false` 的节点不产生任何渲染命令
    fn create_visibility_scene(scene: &mut FigureGraph) {
        // Visible A - 红色 (contents)
        let rect_a = RectangleFigure::new_with_color(
            50.0,
            50.0,
            150.0,
            60.0,
            Color::rgba(0.9, 0.2, 0.2, 1.0),
        );
        let root_id = scene.set_contents(Box::new(rect_a));

        // Hidden B - 蓝色 (不可见)
        let rect_b = RectangleFigure::new_with_color(
            50.0,
            150.0,
            150.0,
            60.0,
            Color::rgba(0.2, 0.4, 0.9, 1.0),
        );
        let id_b = scene.add_child_to(root_id, Box::new(rect_b));

        // 设置不可见
        scene.set_visible(id_b, false);

        // Visible C - 绿色
        let rect_c = RectangleFigure::new_with_color(
            50.0,
            250.0,
            150.0,
            60.0,
            Color::rgba(0.2, 0.8, 0.3, 1.0),
        );
        scene.add_child_to(root_id, Box::new(rect_c));
    }

    /// 场景 2：Z-order 叠加测试
    ///
    /// 验证：后添加的节点视觉上在上层（遮挡先添加的）
    fn create_zorder_scene(scene: &mut FigureGraph) {
        // Z-Order Parent - 灰色容器
        let z_parent = RectangleFigure::new_with_color(
            50.0,
            50.0,
            200.0,
            200.0,
            Color::rgba(0.3, 0.3, 0.3, 1.0),
        );
        let z_parent_id = scene.set_contents(Box::new(z_parent));

        // A - 先添加，红色
        let rect_a = RectangleFigure::new_with_color(
            0.0,
            0.0,
            100.0,
            100.0,
            Color::rgba(0.9, 0.2, 0.2, 1.0),
        );
        scene.add_child_to(z_parent_id, Box::new(rect_a));

        // B - 后添加，蓝色，会覆盖 A 的右下角
        let rect_b = RectangleFigure::new_with_color(
            50.0,
            50.0,
            100.0,
            100.0,
            Color::rgba(0.2, 0.4, 0.9, 1.0),
        );
        scene.add_child_to(z_parent_id, Box::new(rect_b));
    }

    /// 场景 1：嵌套父子结构测试
    ///
    /// 验证：`parent.PaintBorder` 在所有子节点完成后执行
    fn create_nested_scene(scene: &mut FigureGraph) {
        // Parent - 深紫容器
        let parent = RectangleFigure::new_with_color(
            150.0,
            50.0,
            250.0,
            200.0,
            Color::rgba(0.4, 0.2, 0.5, 1.0),
        );
        let parent_id = scene.set_contents(Box::new(parent));

        // Child - 橙色
        let child = RectangleFigure::new_with_color(
            30.0,
            30.0,
            150.0,
            100.0,
            Color::rgba(0.9, 0.5, 0.1, 1.0),
        );
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // Grandchild - 青色（选中）
        let grandchild = RectangleFigure::new_with_color(
            20.0,
            20.0,
            80.0,
            40.0,
            Color::rgba(0.1, 0.8, 0.8, 1.0),
        );
        let gc_id = scene.add_child_to(child_id, Box::new(grandchild));

        // 选中 Grandchild
        scene.set_selected(Some(gc_id));
    }

    /// 场景 2：嵌套场景（含透明根节点）
    ///
    /// contents 根节点下包含一个相对坐标模式的子树，
    /// 验证局部坐标模式下子元素坐标的累积效果。
    /// 注意：与场景 1 保持相同的矩形尺寸，方便比较。
    fn create_nested_with_root_scene(scene: &mut FigureGraph) {
        // 创建透明背景作为根容器
        let root = RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::rgba(0.0, 0.0, 0.0, 0.0),
        );
        let root_id = scene.set_contents(Box::new(root));

        // Parent - 深紫色（与场景 1 相同尺寸：250x200）
        let parent = RectangleFigure::new_with_color(
            350.0,
            50.0,
            250.0,
            200.0,
            Color::rgba(0.4, 0.2, 0.5, 1.0),
        );
        let parent_id = scene.add_child_to(root_id, Box::new(parent));

        // Child - 橙色（与场景 1 相同尺寸：150x100）
        let child = RectangleFigure::new_with_color(
            30.0,
            30.0,
            150.0,
            100.0,
            Color::rgba(0.9, 0.5, 0.1, 1.0),
        );
        let child_id = scene.add_child_to(parent_id, Box::new(child));

        // Grandchild - 青色（选中状态，与场景 1 相同尺寸：80x40）
        // 实际位置 = parent(350,50) + child(30,30) + gc(20,20) = (400, 100)
        let gc = RectangleFigure::new_with_color(
            20.0,
            20.0,
            80.0,
            40.0,
            Color::rgba(0.1, 0.8, 0.8, 1.0),
        );
        let gc_id = scene.add_child_to(child_id, Box::new(gc));
        scene.set_selected(Some(gc_id));
    }

    /// 场景 6：裁剪测试
    ///
    /// 验证：子元素超出父边界时被正确裁剪
    fn create_clip_test_scene(scene: &mut FigureGraph) {
        // 创建透明背景作为根容器
        let root = RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::rgba(0.0, 0.0, 0.0, 0.0),
        );
        let root_id = scene.set_contents(Box::new(root));

        // Parent - 半透明蓝色容器 (100x100)
        let parent = RectangleFigure::new_with_color(
            350.0,
            250.0,
            100.0,
            100.0,
            Color::rgba(0.2, 0.4, 0.8, 0.5),
        );
        let parent_id = scene.add_child_to(root_id, Box::new(parent));

        // Child 1 - 完全在父容器内 (绿色)
        let child1 = RectangleFigure::new_with_color(
            360.0,
            260.0,
            30.0,
            30.0,
            Color::rgba(0.2, 0.8, 0.3, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(child1));

        // Child 2 - 超出父容器右边界 (红色)
        // 父容器 (350, 250, 100, 100)，右边界是 450
        // 子元素从 430 开始，宽度 50，所以超出 450 的部分应该被裁剪
        let child2 = RectangleFigure::new_with_color(
            430.0,
            280.0,
            50.0,
            40.0,
            Color::rgba(0.9, 0.2, 0.2, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(child2));

        // Child 3 - 超出父容器下边界 (黄色)
        // 父容器下边界是 350
        // 子元素从 340 开始，高度 40，所以超出 350 的部分应该被裁剪
        let child3 = RectangleFigure::new_with_color(
            380.0,
            340.0,
            40.0,
            40.0,
            Color::rgba(0.9, 0.8, 0.2, 1.0),
        );
        scene.add_child_to(parent_id, Box::new(child3));
    }

    /// 场景 7：椭圆图形测试
    ///
    /// 验证 EllipseFigure 渲染正确
    fn create_ellipse_test_scene(scene: &mut FigureGraph) {
        // 创建透明背景
        let root = RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::rgba(0.1, 0.1, 0.1, 1.0),
        );
        let root_id = scene.set_contents(Box::new(root));

        // 椭圆 1 - 红色填充，带白色边框
        let ellipse1 = EllipseFigure::new(150.0, 150.0, 100.0, 80.0).with_stroke(Color::WHITE, 2.0);
        scene.add_child_to(root_id, Box::new(ellipse1));

        // 椭圆 2 - 蓝色填充，无边框
        let ellipse2 = EllipseFigure::new_with_color(
            350.0,
            200.0,
            120.0,
            120.0,
            Color::rgba(0.2, 0.6, 0.9, 1.0),
        );
        scene.add_child_to(root_id, Box::new(ellipse2));

        // 椭圆 3 - 绿色描边，无填充
        let ellipse3 = EllipseFigure::new_with_color(
            550.0,
            150.0,
            80.0,
            150.0,
            Color::rgba(0.0, 0.0, 0.0, 0.0),
        )
        .with_stroke(Color::rgba(0.2, 0.8, 0.3, 1.0), 3.0);
        scene.add_child_to(root_id, Box::new(ellipse3));

        // 圆形 - 正椭圆
        let circle = EllipseFigure::new_with_color(
            400.0,
            400.0,
            100.0,
            100.0,
            Color::rgba(0.9, 0.6, 0.2, 1.0),
        )
        .with_stroke(Color::WHITE, 2.0);
        scene.add_child_to(root_id, Box::new(circle));
    }

    /// 场景 8：直线图形测试
    ///
    /// 验证 LineFigure 渲染正确
    fn create_line_test_scene(scene: &mut FigureGraph) {
        // 创建透明背景
        let root = RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::rgba(0.1, 0.1, 0.1, 1.0),
        );
        let root_id = scene.set_contents(Box::new(root));

        // 水平线 - 红色
        let h_line = PolylineFigure::new_with_color(
            100.0,
            100.0,
            300.0,
            100.0,
            Color::rgba(0.9, 0.3, 0.3, 1.0),
        )
        .with_width(3.0);
        scene.add_child_to(root_id, Box::new(h_line));

        // 垂直线 - 蓝色
        let v_line = PolylineFigure::new_with_color(
            400.0,
            50.0,
            400.0,
            250.0,
            Color::rgba(0.3, 0.3, 0.9, 1.0),
        )
        .with_width(3.0);
        scene.add_child_to(root_id, Box::new(v_line));

        // 斜线 - 绿色
        let diag_line = PolylineFigure::new_with_color(
            100.0,
            200.0,
            300.0,
            400.0,
            Color::rgba(0.3, 0.9, 0.3, 1.0),
        )
        .with_width(3.0);
        scene.add_child_to(root_id, Box::new(diag_line));

        // 反向斜线 - 橙色（测试反向坐标）
        let diag_line2 = PolylineFigure::new_with_color(
            400.0,
            200.0,
            300.0,
            400.0,
            Color::rgba(0.9, 0.6, 0.2, 1.0),
        )
        .with_width(3.0);
        scene.add_child_to(root_id, Box::new(diag_line2));

        // 粗线 - 紫色
        let thick_line = PolylineFigure::new_with_color(
            100.0,
            450.0,
            700.0,
            450.0,
            Color::rgba(0.6, 0.3, 0.9, 1.0),
        )
        .with_width(8.0);
        scene.add_child_to(root_id, Box::new(thick_line));

        // 加粗白色细线
        let white_line = PolylineFigure::new_with_color(500.0, 300.0, 750.0, 550.0, Color::WHITE)
            .with_width(2.0);
        scene.add_child_to(root_id, Box::new(white_line));
    }

    fn create_dpi_test_scene(scene: &mut FigureGraph) {
        let root = RectangleFigure::new_with_color(
            0.0,
            0.0,
            800.0,
            600.0,
            Color::rgba(0.10, 0.12, 0.16, 1.0),
        );
        let root_id = scene.set_contents(Box::new(root));

        let outer = InteractiveRectFigure::with_palette(
            DPI_TEST_PROBE_BOUNDS.x,
            DPI_TEST_PROBE_BOUNDS.y,
            DPI_TEST_PROBE_BOUNDS.width,
            DPI_TEST_PROBE_BOUNDS.height,
            Color::rgba(0.20, 0.47, 0.86, 1.0),
            Color::rgba(0.18, 0.72, 0.64, 1.0),
            Color::rgba(0.95, 0.56, 0.18, 1.0),
            Color::rgba(0.10, 0.16, 0.24, 1.0),
            Color::rgba(0.98, 0.86, 0.22, 1.0),
        );
        scene.add_child_to(root_id, Box::new(outer));
    }

    /// 切换场景
    pub fn switch_scene(&mut self, scene_type: SceneType) {
        self.scene = FigureGraph::new();
        match scene_type {
            SceneType::BasicAnchors => Self::create_basic_anchors_scene(&mut self.scene),
            SceneType::Nested => Self::create_nested_scene(&mut self.scene),
            SceneType::NestedWithRoot => Self::create_nested_with_root_scene(&mut self.scene),
            SceneType::ZOrder => Self::create_zorder_scene(&mut self.scene),
            SceneType::Visibility => Self::create_visibility_scene(&mut self.scene),
            SceneType::BoundsTranslate => Self::create_bounds_translate_scene(&mut self.scene),
            SceneType::ClipTest => Self::create_clip_test_scene(&mut self.scene),
            SceneType::EllipseTest => Self::create_ellipse_test_scene(&mut self.scene),
            SceneType::LineTest => Self::create_line_test_scene(&mut self.scene),
            SceneType::DpiTest => Self::create_dpi_test_scene(&mut self.scene),
        }
        self.current_scene = scene_type;
        println!("[SceneManager] 切换到场景 {:?}", scene_type);
    }

    /// 获取场景图
    pub fn scene(&self) -> &FigureGraph {
        &self.scene
    }

    /// 获取场景图可变引用
    pub fn scene_mut(&mut self) -> &mut FigureGraph {
        &mut self.scene
    }
}

impl Default for SceneManager {
    fn default() -> Self {
        Self::new()
    }
}
