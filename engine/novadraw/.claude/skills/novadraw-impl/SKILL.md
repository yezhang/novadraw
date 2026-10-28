---
name: novadraw-impl
description: 指导 novadraw 项目特定模块的实现模式，包括 Figure 类型、布局算法、渲染后端等。
---

# Novadraw 实现指南

## 模块结构

```text
novadraw-core/     - 核心数据类型
novadraw-geometry/ - 几何计算
novadraw-math/     - 数学运算
novadraw-scene/    - 场景图和 Figure
novadraw-render/   - 渲染后端抽象
```

## Figure 实现模式

### 1. 创建新 Figure 类型

参考 `doc/figure_box_model.md` 中的 g2 盒模型实现。

```rust
// 1. 在 figure/mod.rs 添加模块
mod my_figure;
pub use my_figure::MyFigure;

// 2. 实现 Figure trait
pub struct MyFigure {
    bounds: Rectangle,
    // ... 其他属性
}

impl Figure for MyFigure {
    fn bounds(&self) -> Rectangle {
        self.bounds
    }

    fn paint_figure(&self, gc: &mut NdCanvas) {
        // 绘制逻辑
    }

    fn set_bounds(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.bounds = Rectangle::new(x, y, width, height);
    }
}

// 3. 如果需要描边/填充，实现 Shape trait
impl Shape for MyFigure {
    fn fill_color(&self) -> Option<Color> { ... }
    fn stroke_color(&self) -> Option<Color> { ... }
    // ...
}
```

### 2. 坐标计算要点

- **bounds**: 图形占用的完整矩形区域
- **outline**: 描边在 bounds 内部，向内收缩 `lineWidth / 2`
- **border**: 装饰边框，由 Border 类型处理
- **clientArea**: `bounds - insets`，子元素布局区域

### 3. 命中测试实现

```rust
impl Figure for RectangleFigure {
    fn contains_point(&self, x: f64, y: f64) -> bool {
        let b = self.bounds();
        x >= b.x && x <= b.x + b.width
            && y >= b.y && y <= b.y + b.height
    }
}
```

## 布局算法实现

### 实现 LayoutManager

```rust
pub trait LayoutManager: Send + Sync {
    fn layout(&self, container: &mut Figure, children: &[BlockId]);

    fn get_constraint(&self, child: BlockId) -> Option<Box<dyn Any>>;

    fn set_constraint(&mut self, child: BlockId, constraint: Box<dyn Any>);
}
```

### 注意事项

- 布局计算在 FigureGraph 层完成
- 布局结果更新 Figure 的 bounds
- 使用迭代而非递归处理子节点

## 渲染后端实现

### NdCanvas 命令模式

```rust
// novadraw-render/src/context.rs
pub struct NdCanvas {
    commands: Vec<RenderCommand>,
}

// 添加新命令类型
// 1. 在 command.rs 添加 RenderCommandKind
// 2. 在 context.rs 实现对应方法
// 3. 在后端实现中处理命令
```

### 后端替换流程

1. 实现 `RenderBackend` trait
2. 添加到后端注册表
3. 测试现有功能正常

## 事件处理

### 添加新事件类型

1. 在 `novadraw-scene/src/event.rs` 定义事件
2. 在 `FigureGraph` 中添加事件队列
3. 在渲染循环中分发事件

## 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_calculation() {
        let figure = RectangleFigure::new(0.0, 0.0, 100.0, 50.0);
        let bounds = figure.bounds();
        assert_eq!(bounds.width, 100.0);
    }

    #[test]
    fn test_contains_point() {
        let figure = RectangleFigure::new(0.0, 0.0, 100.0, 50.0);
        assert!(figure.contains_point(50.0, 25.0));
        assert!(!figure.contains_point(150.0, 25.0));
    }
}
```

### 集成测试

在 `apps/` 目录下创建测试应用验证功能。

## 文档更新

新增功能需要更新对应文档：

| 功能 | 文档位置 |
|------|----------|
| Figure 类型 | `doc/figure_*.md` |
| 布局算法 | `doc/layout_*.md` |
| 渲染流程 | `doc/rendering_*.md` |
| API 对照 | `doc/api-comparison.md` |
