---
name: analyzing-gef-code
description: 分析 GEF 和 draw2d 源码并深入理解核心数据结构和算法。当需要分析 draw2d 代码时调用。
---

# GEF/draw2d 框架源码分析

## 源码位置

| 组件 | 路径 |
|------|------|
| draw2d 核心 | `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/` |
| draw2d 示例 | `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d.examples/` |
| GEF 框架 | `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef/src/org/eclipse/gef/` |
| GEF 示例 | `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef.examples/` |

## 核心包结构 (draw2d)

| 包 | 职责 |
|----|------|
| `geometry` | 几何计算（Point, Rectangle, Transformations） |
| `graph` | 图形布局算法 |
| `text` | 文本渲染相关 |
| `parts` | 图形部件 |
| `widgets` | UI 小部件 |

## 分析步骤

### 1. 定位关键类

根据分析需求，定位对应的核心类：

- **Figure 体系**: `Figure.java` → `ShapeFigure` → `RectangleFigure`, `Ellipse`, `Polyline` 等
- **Graphics 绘制**: `Graphics.java` → `SWTGraphics`
- **坐标变换**: `geometry.Translatable`, `geometry.Transform`
- **布局管理**: `AbstractLayout`, `LayoutManager` → `FlowLayout`, `GridLayout`, `XYLayout`
- **连接线**: `Connection`, `ConnectionAnchor`, `AbstractRouter`

### 2. 数据结构分析

分析以下核心数据结构：

1. **Figure 类层次**
   - 继承关系（父类/接口）
   - 核心属性（bounds, background, border, layoutManager）
   - 事件机制（FigureListener, PropertyChange）

2. **Graphics 状态**
   - 绘制状态（fill, stroke, clip）
   - 坐标变换栈
   - 字体/颜色管理

3. **布局算法**
   - 约束系统（Constraint）
   - 布局接口（LayoutManager）
   - 嵌套布局处理

### 3. 算法分析

重点关注：

- **坐标变换**: `toLocal()`, `toParent()`, `translateToParent()` 的变换矩阵计算过程
- **绘制流程**: `paint()` → `paintFigure()` → `paintChildren()` 的调用链
- **布局更新**: `invalidate()`, `revalidate()`, `layout()` 的触发机制
- **命中测试**: `contains()`, `getBackgroundShape()` 的区域计算

### 4. 关键方法示例

分析坐标变换时，追踪以下调用链：

```text
Figure.translateToParent(dx, dy)
  → Figure.getParent()
  → Figure.getParent().translateToParent(dx, dy)
  → Bounds.add(x, y)
```

分析绘制流程时，追踪：

```text
Figure.paint(Graphics)
  → paintFigure(Graphics)
  → paintBorder(Graphics)
  → paintChildren(Graphics)
    → childFigure.paint(Graphics)
```

## 分析输出模板

分析完成后，提供以下信息：

### 核心数据结构

```text
类名: Figure
职责: ...
关键属性:
  - bounds: Rectangle - 图形边界
  - background: Color - 背景色
  - border: Border - 边框
  - layoutManager: LayoutManager - 布局管理器
关键方法:
  - contains(Point): boolean
  - paint(Graphics): void
  - translate(int, int): void
```

### 关键算法说明

```text
算法名: 坐标变换
输入: 子图形坐标 (x, y)
输出: 父图形坐标
过程:
  1. 获取当前图形的 bounds
  2. 加上偏移量 (dx, dy)
  3. 递归调用父图形的 translateToParent
  ...
```

### 架构设计要点

```text
1. 组合模式: Figure 支持嵌套形成树形结构
2. 观察者模式: FigureListener 监听图形变化
3. 策略模式: LayoutManager 支持多种布局算法
4. 责任链模式: Border 嵌套组合
```

## 注意事项

## 注意事项

- draw2d 使用 Java/SWT，Graphics 底层依赖 SWT GC
- 坐标系统：原点左上角，y 轴向下
- 绘制单位：像素
- 事件处理使用 SWT 标准的 Listener 机制
