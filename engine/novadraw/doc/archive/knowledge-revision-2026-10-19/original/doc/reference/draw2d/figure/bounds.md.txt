# Eclipse Draw2D Figure Bounds 分析

类型：`reference-analysis`

本文档分析 Eclipse Draw2D 中 Figure 的 bounds 概念，包括其含义、作用、坐标系统以及在布局和绘制中的应用。

## 1. Bounds 的定义与含义

### 1.1 基本定义

```java
// Figure.java:77
protected Rectangle bounds = new Rectangle(0, 0, 0, 0);
```

Bounds 是一个 `Rectangle`，存储 Figure 的**位置和尺寸**：(x, y, width, height)。

### 1.2 准确含义：绝对坐标

Bounds 的 (x, y) 是**绝对坐标**，但这个"绝对"是相对于**坐标根**的：

- **默认模式**（`useLocalCoordinates() = false`）：当前 Figure 与 children
  共享所属坐标域；当前 Figure 平移时递归修改后代 bounds
- **本地坐标模式**（`useLocalCoordinates() = true`）：当前 Figure 为 children
  建立以 client area 为原点的局部坐标域；当前 Figure 平移时不修改后代 bounds

```java
// Figure.java:1390-1397 - primTranslate
protected void primTranslate(int dx, int dy) {
    bounds.x += dx;
    bounds.y += dy;

    if (useLocalCoordinates()) {
        // 本地坐标模式：不传播偏移到子节点
        fireCoordinateSystemChanged();
        return;
    }
    // 默认模式：递归传播偏移到所有子节点
    children.forEach(child -> child.translate(dx, dy));
}
```

`useLocalCoordinates()` 控制的是当前 Figure **如何解释 children 的坐标**。
当前 Figure 自己的 bounds 始终位于 parent 提供的坐标域中；不能把
`useLocalCoordinates() == true` 简化成“当前 Figure 的 bounds 改为 parent-local”。

## 2. Bounds 的具体作用

Bounds 在 Draw2D 中有多种关键用途：

| 用途 | 代码示例 | 说明 |
|------|----------|------|
| **命中测试** | `containsPoint(x, y) → bounds.contains(x, y)` | 判断点是否在图形内 |
| **绘制位置** | `graphics.fillRectangle(getBounds())` | 绘制图形的背景 |
| **裁剪区域** | `clippingStrategy.getClip(child)` | 确定子节点的绘制区域 |
| **重绘区域** | `repaint(getBounds())` | 需要重绘的区域 |
| **坐标转换** | `translateFromParent/ToParent` | 父子坐标系转换 |
| **布局计算** | `layout.setConstraints(this, constraint)` | 布局管理器计算位置 |

### 2.1 绘制中的使用

```java
// Figure.java:1373-1375 - paintFigure
protected void paintFigure(Graphics graphics) {
    if (isOpaque()) {
        graphics.fillRectangle(getBounds());  // 使用绝对坐标绘制
    }
}

// Figure.java - paintClientArea (子节点绘制)
if (useLocalCoordinates()) {
    graphics.translate(getBounds().x + getInsets().left,
                       getBounds().y + getInsets().top);
    // ... 绘制子节点
}
```

### 2.2 命中测试中的使用

```java
// Figure.java:367-368 - containsPoint
public boolean containsPoint(int x, int y) {
    return getBounds().contains(x, y);  // x, y 是绝对坐标
}

// Figure.java:500-517 - findMouseEventTargetInDescendantsAt
protected IFigure findMouseEventTargetInDescendantsAt(int x, int y) {
    PRIVATE_POINT.setLocation(x, y);
    translateFromParent(PRIVATE_POINT);  // 转换到父节点坐标系

    if (!getClientArea(Rectangle.SINGLETON).contains(PRIVATE_POINT)) {
        return null;  // 剪枝：不在父节点内，跳过
    }

    for (IFigure fig : getChildrenRevIterable()) {  // 逆序遍历
        if (fig.containsPoint(x, y)) {  // 检查子节点 bounds
            fig = fig.findMouseEventTargetAt(x, y);
            if (fig != null) return fig;
        }
    }
    return this;
}
```

### 2.3 坐标转换

```java
// 父节点坐标域 → 当前 Figure 的局部坐标域
public void translateFromParent(Translatable t) {
    if (useLocalCoordinates()) {
        t.performTranslate(-bounds.x - insets.left, -bounds.y - insets.top);
    }
}

// 当前 Figure 的局部坐标域 → 父节点坐标域
public void translateToParent(Translatable t) {
    if (useLocalCoordinates()) {
        t.performTranslate(bounds.x + insets.left, bounds.y + insets.top);
    }
}

// 相对坐标 → 绝对坐标
public final void translateToAbsolute(Translatable t) {
    if (getParent() != null) {
        Translatable tPrecise = toPreciseShape(t);
        getParent().translateToParent(tPrecise);
        getParent().translateToAbsolute(tPrecise);
        fromPreciseShape(tPrecise, t);
    }
}
```

## 3. 坐标根（Coordinate Root）

### 3.1 定义

**坐标系统边界**是父链中 `isCoordinateSystem() = true` 并通过
`translateToParent/translateFromParent` 定义变换的 Figure。基础 `Figure`
的默认实现把它等同于 `useLocalCoordinates()`；scalable pane、Viewport 等子类
可以直接覆盖公开坐标协议。

### 3.2 形式化定义

```
absolute_bounds(F) =
    如果 P = 最近的使用 isCoordinateSystem() = true 的祖先(F)
    则 absolute_bounds(F) = relative_bounds(F) 相对于 P
    否则 absolute_bounds(F) = relative_bounds(F) 相对于画布原点
```

其中：
- `F` 是当前 Figure
- `P` 是 `F` 的祖先中最近的使用 `isCoordinateSystem() = true` 的节点
- 如果没有这样的祖先 `P`，则相对于画布原点

### 3.3 关键源码

```java
// IFigure.java:662-670
/**
 * Returns <code>true</code> if this figure is capable of applying a local
 * coordinate system which affects its children.
 *
 * @since 3.1
 * @return <code>true</code> if this figure provides local coordinates to
 *         children
 */
boolean isCoordinateSystem();

// Figure.java:1128-1133
@Override
public boolean isCoordinateSystem() {
    return useLocalCoordinates();
}

// Figure.java:2045-2048
@Override
public void translateFromParent(Translatable t) {
    if (useLocalCoordinates()) {
        t.performTranslate(-getBounds().x - getInsets().left,
                           -getBounds().y - getInsets().top);
    }
}
```

### 3.4 常见坐标根

在当前源码中，以下容器明确覆盖了 `isCoordinateSystem()`：

| 类名 | 用途 |
|------|------|
| `ScalableFreeformLayeredPane` | 可缩放的自由表单层叠面板 |
| `Viewport` | 视口，支持滚动 |
| `ScalableLayeredPane` | 可缩放层叠面板 |

```java
// ScalableFreeformLayeredPane.java:67-72
@Override
public boolean isCoordinateSystem() {
    return true;  // 总是作为坐标根
}

// Viewport.java:172-177
@Override
public boolean isCoordinateSystem() {
    return useGraphicsTranslate() || super.isCoordinateSystem();
}
```

`Viewport` 只有在 `useGraphicsTranslate()` 为真，或父类通过
`useLocalCoordinates()` 提供局部坐标时才是坐标系统。
`FreeformLayer` 与 `FreeformLayeredPane` 本身没有覆盖该方法，不能仅凭类型名称
将它们视为坐标根。

### 3.5 图示

```
画布原点 (0,0)
    │
    ├── ScalableFreeformLayeredPane (isCoordinateSystem() = true) ─┐
    │     │                                                     │
    │     ├── Panel A (useLocalCoordinates = false) ──────┐     │
    │     │     │                                           │     │
    │     │     └── Button X (bounds: 10,10,100,50) ───────┼─────┘
    │     │                                                   │
    │     └── Panel B (useLocalCoordinates = true) ─────────┼──┐
    │           │                                            │  │
    │           └── Button Y (bounds: 20,20,100,50) ─────────┼──┤
    │                                                          │  │
    └── Viewport (isCoordinateSystem() = true) ────────────────┘  │
                                                                │
    Button X 的绝对坐标 = (10, 10)                               │
    Button Y 的绝对坐标 = (20, 20) 相对于 Panel B，而非画布原点  │
```

### 3.6 总结表

| 问题 | 答案 |
|------|------|
| bounds 存储什么？ | (x, y, width, height) - 位置和尺寸 |
| x, y 的含义是什么？ | **绝对坐标**（相对于坐标根） |
| 什么是坐标根？ | `isCoordinateSystem() = true` 且定义 parent/local 变换边界的 Figure |
| 默认模式下如何变成绝对坐标？ | 父节点移动时自动 `translate()` 传播到子节点 |
| `useLocalCoordinates()` 的作用？ | 在基础 Figure 中，设为 `true` 时为 children 建立 client-area 局部域 |
| 命中测试的坐标？ | 输入点始于入口坐标域，递归下降时通过 `translateFromParent()` 切换到子坐标域 |
| 绘制时如何使用 bounds？ | `graphics.fillRectangle(getBounds())` - 绝对坐标 |
| 子节点如何定位？ | 遍历时通过 `translateFromParent()` 转换坐标 |
| 常见坐标根有哪些？ | ScalableFreeformLayeredPane、ScalableLayeredPane，以及启用 graphics translate 的 Viewport |
| 如果没有坐标根？ | 相对于画布原点 |

## 4. Bounds 与布局的关系

### 4.1 布局过程中的 bounds 设置

```java
// Figure.java:1674-1698 - setBounds
@Override
public void setBounds(Rectangle rect) {
    int x = bounds.x, y = bounds.y;

    boolean resize = (rect.width != bounds.width) || (rect.height != bounds.height);
    boolean translate = (rect.x != x) || (rect.y != y);

    if ((resize || translate) && isVisible()) {
        erase();  // 擦除旧位置
    }
    if (translate) {
        int dx = rect.x - x;
        int dy = rect.y - y;
        primTranslate(dx, dy);  // 移动 bounds 并传播到子节点
    }

    bounds.width = rect.width;
    bounds.height = rect.height;

    if (translate || resize) {
        if (resize) {
            invalidate();  // 使布局无效，需要重新布局
        }
        fireFigureMoved();
        repaint();  // 在新位置重绘
    }
}
```

### 4.2 布局管理器的作用

布局管理器负责计算子 Figure 的 bounds：

```java
// LayoutManager 接口
public interface LayoutManager {
    Object getConstraint(IFigure child);
    Dimension getMinimumSize(IFigure container, int wHint, int hHint);
    Dimension getPreferredSize(IFigure container, int wHint, int hHint);
    void invalidate();
    void layout(IFigure container);
    void remove(IFigure child);
    void setConstraint(IFigure child, Object constraint);
}
```

布局计算完成后，LayoutManager 把 child bounds 写入 parent 为 children 提供的
坐标域：普通 Figure 与 children 共享当前域；local-coordinate Figure 的 children
使用以 parent client area 为原点的局部域。

## 5. Bounds 与命中的测试流程

### 5.1 命中测试的完整流程

```
点击点 (入口坐标域 x, y)
        │
        ▼
┌─────────────────────────────────────┐
│ findMouseEventTargetAt              │
│ 坐标转换: 父坐标域 → 当前坐标域      │
│ containsPoint(parent)?              │
└─────────────────────────────────────┘
        │
        ▼
    逆序遍历 children
        │
        ├─→ Child1 (后添加，在上层)
        │       │
        │       ▼
        │   containsPoint(child1)?
        │       │
        │       ├──→ 是 → 递归检测 Child1 的子节点
        │       └──→ 否 → 继续下一个
        │
        └─→ Child2 (先添加，在下层)
                │
                ▼
            containsPoint(child2)?
                │
                └──→ 否 → 继续

返回最深层的命中节点
```

### 5.2 关键点

1. **逆序遍历**：`getChildrenRevIterable()` 确保后添加的节点（视觉上层）先被检测
2. **坐标转换**：使用 `translateFromParent()` 在父子坐标域之间切换
3. **剪枝**：`!getClientArea().contains(point)` 跳过不在父节点内的整个子树
4. **递归**：找到最深层的命中节点即返回

## 6. 参考源码

| 文件 | 主要内容 |
|------|----------|
| `Figure.java` | 核心 Figure 类，包含 bounds 操作 |
| `IFigure.java` | Figure 接口，定义坐标相关方法 |
| `ScalableFreeformLayeredPane.java` | 可缩放自由表单层叠面板 |
| `Viewport.java` | 视口实现 |
| `FreeformLayer.java` | 自由表单层 |
| `LayoutManager.java` | 布局管理器接口 |

---

*源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。*
