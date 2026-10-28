# Eclipse Draw2D Figure 核心概念全景图

类型：`reference-analysis`

本文档系统整理 Eclipse Draw2D Figure 系统的核心概念，帮助理解 Figure 的设计原理和实现要点。

## 1. 核心概念全景图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Figure 核心概念                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │  树状结构       │    │  Bounds 系统    │    │  坐标系统       │     │
│  │  Tree Structure │    │  Bounds System  │    │  Coordinate     │     │
│  ├─────────────────┤    ├─────────────────┤    ├─────────────────┤     │
│  │ • parent/child  │    │ • bounds (Rect) │    │ • local         │     │
│  │ • sibling       │    │ • setBounds()   │    │ • parent        │     │
│  │ • Z-order       │    │ • insets        │    │ • absolute      │     │
│  │ • add/remove    │    │ • clientArea    │    │ • coordinate    │     │
│  │                 │    │                 │    │   root          │     │
│  └────────┬────────┘    └────────┬────────┘    └────────┬────────┘     │
│           │                      │                      │               │
│           └──────────────────────┼──────────────────────┘               │
│                                  ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    Figure.paint 模板流程                         │   │
│  │  Apply local style → pushState → paintFigure → restoreState     │   │
│  │                    → paintClientArea → paintBorder → popState    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                  │                                      │
│           ┌──────────────────────┼──────────────────────┐               │
│           ▼                      ▼                      ▼               │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │  布局管理器     │    │  命中测试       │    │  事件通知       │     │
│  │  LayoutManager  │    │  Hit Test       │    │  Event System   │     │
│  ├─────────────────┤    ├─────────────────┤    ├─────────────────┤     │
│  │ • layout()      │    │ • containsPoint │    │ • figureMoved   │     │
│  │ • invalidate()  │    │ • findFigureAt  │    │ • coordinate    │     │
│  │ • revalidate()  │    │ • TreeSearch    │    │   changed       │     │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## 2. 树状结构 (Tree Structure)

### 2.1 基本概念

Figure 系统采用树形层次结构，每个 Figure 可以有零个或多个子节点：

```java
// Draw2D 源码结构
public class Figure implements IFigure {
    private IFigure parent;
    private List<IFigure> children = Collections.emptyList();
}
```

### 2.2 Z-order 机制

后添加的子节点在视觉上位于上层（遮挡先添加的）：

```java
// 添加顺序：A → B → C
// 渲染顺序：A → B → C（先添加的先渲染，在下层）
// 视觉层级：C 在最上层（遮挡 B 和 A）

// 命中测试时逆序遍历，后添加的优先命中
for (IFigure fig : getChildrenRevIterable()) {
    if (fig.containsPoint(x, y)) {
        return fig.findMouseEventTargetAt(x, y);
    }
}
```

### 2.3 层次遍历

Draw2D 对兄弟节点使用循环或迭代器，但核心树操作仍通过子节点方法调用形成递归：

- 绘制：`paint()` → `paintClientArea()` → `paintChildren()` → `child.paint()`
- 验证：`validate()` → `children.forEach(IFigure::validate)`
- 命中：`findFigureAt()` 与 descendant search 相互递归
- 平移：`primTranslate()` → `child.translate()`

因此，“使用迭代器”只描述同层枚举方式，不能推出 Draw2D 避免了递归栈增长。

## 3. Bounds 系统 (Bounds System)

### 3.1 Bounds 的定义与含义

```java
// Figure.java:77
protected Rectangle bounds = new Rectangle(0, 0, 0, 0);
```

Bounds 是一个 `Rectangle`，存储 Figure 的位置和尺寸：(x, y, width, height)。

**关键理解**：bounds 的 (x, y) 是**绝对坐标**，但这个"绝对"是相对于**坐标根**的。

### 3.2 两种坐标模式

| 当前 Figure 的模式 | children 的坐标域 | 当前 Figure 平移时的传播 |
|------|----------------------|-------------|
| `useLocalCoordinates() == false` | 与当前 Figure 共享所属坐标域 | 递归修改后代 bounds |
| `useLocalCoordinates() == true` | 以当前 Figure 的 client area 为新局部域 | 不修改后代 bounds，发送坐标系统变化通知 |

`useLocalCoordinates()` 描述的是当前 Figure **为 children 提供的坐标系**，不是把
当前 Figure 自己的 bounds 改成另一种存储格式。当前 Figure 的 bounds 始终位于其
parent 提供的坐标域中。

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

### 3.3 Bounds 的具体作用

| 用途 | 代码示例 | 说明 |
|------|----------|------|
| 命中测试 | `containsPoint(x, y) → bounds.contains(x, y)` | 判断点是否在图形内 |
| 绘制位置 | `graphics.fillRectangle(getBounds())` | 绘制图形的背景 |
| 裁剪区域 | `clippingStrategy.getClip(child)` | 确定子节点的绘制区域 |
| 重绘区域 | `repaint(getBounds())` | 需要重绘的区域 |
| 坐标转换 | `translateFromParent/ToParent` | 父子坐标系转换 |
| 布局计算 | `layout.setConstraints(this, constraint)` | 布局管理器计算位置 |

### 3.4 setBounds() 完整语义

```java
// Figure.java:1674-1698
@Override
public void setBounds(Rectangle rect) {
    int x = bounds.x, y = bounds.y;

    boolean resize = (rect.width != bounds.width) || (rect.height != bounds.height);
    boolean translate = (rect.x != x) || (rect.y != y);

    // 1. 擦除旧位置（如果可见且位置/大小变化）
    if ((resize || translate) && isVisible()) {
        erase();
    }

    // 2. 移动 bounds 并传播到子节点
    if (translate) {
        int dx = rect.x - x;
        int dy = rect.y - y;
        primTranslate(dx, dy);
    }

    // 3. 更新宽高
    bounds.width = rect.width;
    bounds.height = rect.height;

    // 4. 布局失效和重绘
    if (translate || resize) {
        if (resize) {
            invalidate();  // 使布局无效，需要重新布局
        }
        fireFigureMoved();
        repaint();  // 在新位置重绘
    }
}
```

### 3.5 clientArea 与 insets

```java
// clientArea = bounds - insets
public Rectangle getClientArea(Rectangle rect) {
    rect.setBounds(bounds);
    rect.x += insets.left;
    rect.y += insets.top;
    rect.width -= insets.left + insets.right;
    rect.height -= insets.top + insets.bottom;
    return rect;
}
```

## 4. 坐标系统 (Coordinate System)

### 4.1 坐标层级

```
画布原点 (0,0)
    │
    ├── ScalableFreeformLayeredPane (isCoordinateSystem() = true) ──┐
    │     │                                                     │
    │     ├── Panel A (useLocalCoordinates = false) ──────┐     │
    │     │     │                                           │     │
    │     │     └── Button X (bounds: 10,10,100,50) ───────┼─────┘
    │     │                                                   │
    │     └── Panel B (useLocalCoordinates = true) ─────────┼──┐
    │           │                                            │  │
    │           └── Button Y (bounds: 20,20,100,50) ─────────┼──┤
    │                                                            │
    └── Viewport (isCoordinateSystem() = true) ────────────────┘  │
                                                                │
    Button X 的绝对坐标 = (10, 10)                               │
    Button Y 的绝对坐标 = (20, 20) 相对于 Panel B，而非画布原点  │
```

### 4.2 坐标转换方法

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

### 4.3 坐标根 (Coordinate Root)

**坐标系统边界**由 `isCoordinateSystem()` 与
`translateToParent/translateFromParent` 协议共同表达。基础 `Figure` 默认让
`isCoordinateSystem()` 等于 `useLocalCoordinates()`；scalable pane 等子类可以
直接覆盖公开坐标协议。

形式化定义：
```
absolute_bounds(F) =
    如果 P = 最近的使用 isCoordinateSystem() = true 的祖先(F)
    则 absolute_bounds(F) = relative_bounds(F) 相对于 P
    否则 absolute_bounds(F) = relative_bounds(F) 相对于画布原点
```

明确覆盖 `isCoordinateSystem()` 的常见容器：

| 类名 | 用途 |
|------|------|
| `ScalableFreeformLayeredPane` | 可缩放的自由表单层叠面板 |
| `Viewport` | 视口，支持滚动 |
| `ScalableLayeredPane` | 可缩放层叠面板 |

```java
// ScalableFreeformLayeredPane.java
@Override
public boolean isCoordinateSystem() {
    return true;  // 总是作为坐标根
}

// Viewport.java
@Override
public boolean isCoordinateSystem() {
    return useGraphicsTranslate() || super.isCoordinateSystem();
}
```

`FreeformLayer` 和 `FreeformLayeredPane` 本身不覆盖
`isCoordinateSystem()`，不能仅因其为 freeform 容器就列为坐标根。

## 5. Figure.paint 模板流程

### 5.1 流程图

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Figure.paint 模板流程                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Apply local style → 设置本地背景色、前景色和字体                  │
│     │                                                               │
│     ▼                                                               │
│  2. pushState         → 保存完整 Graphics 状态                        │
│     │                                                               │
│     ▼                                                               │
│  3. paintFigure       → 绘制主体或背景                                │
│     │                                                               │
│     ▼                                                               │
│  4. restoreState      → 恢复进入 paintFigure 前的状态                 │
│     │                                                               │
│     ▼                                                               │
│  5. paintClientArea   → 设置内容区坐标/裁剪并递归绘制子节点           │
│     │                                                               │
│     ▼                                                               │
│  6. paintBorder       → 绘制边框                                     │
│     │                                                               │
│     ▼                                                               │
│  7. popState          → 恢复调用 paint 前的完整状态                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 关键点

1. **Figure 负责自身及子树编排**：`Figure.paint()` 直接调用
   `paintClientArea()`，后者调用 `paintChildren()` 和 `child.paint()`。
2. **状态管理**：`pushState/restoreState/popState` 隔离自身、子节点和调用者状态。
3. **PaintBorder 在子节点之后**：边框最后绘制。

### 5.3 Draw2D 源码实现

```java
// Figure.java
public void paint(Graphics graphics) {
    // 先应用本地颜色和字体
    graphics.pushState();
    try {
        paintFigure(graphics);
        graphics.restoreState();
        paintClientArea(graphics);
        paintBorder(graphics);
    } finally {
        graphics.popState();
    }
}
```

`paint()` 不是 `final`。`LightweightSystem.paint(GC)` 只把 SWT paint
入口委托给 `UpdateManager.paint(GC)`；真正的 Figure 子树协议仍由
`root.paint(graphics)` 启动。

## 6. 布局管理器 (LayoutManager)

### 6.1 LayoutManager 接口

```java
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

### 6.2 布局失效机制

```java
public void invalidate() {
    if (layoutManager != null) {
        layoutManager.invalidate();
    }
    setValid(false);
}

public void revalidate() {
    invalidate();
    if (getParent() == null || isValidationRoot()) {
        getUpdateManager().addInvalidFigure(this);
    } else {
        getParent().revalidate();
    }
}
```

`validate()` 才会把自身置为 valid、执行 `layout()`，再递归验证 children。

### 6.3 布局管理器类型

| 类型 | 用途 |
|------|------|
| `StackLayout` | 所有可见子元素填充 client area |
| `XYLayout` | 按每个子元素的 `Rectangle` 约束定位 |
| `FlowLayout` | 水平流式排列 |
| `GridLayout` | 网格排列 |
| `BorderLayout` | 东南西北中布局 |

## 7. 命中测试 (Hit Test)

### 7.1 完整流程

```
点击点 (global_x, global_y)
        │
        ▼
┌─────────────────────────────────────┐
│ findMouseEventTargetAt              │
│ 坐标转换: global → parent           │
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

### 7.2 关键点

1. **逆序遍历**：`getChildrenRevIterable()` 确保后添加的节点（视觉上层）先被检测
2. **坐标转换**：使用 `translateFromParent()` 在父子坐标域之间切换
3. **剪枝**：`!getClientArea().contains(point)` 跳过不在父节点内的整个子树
4. **递归**：找到最深层的命中节点即返回

### 7.3 Draw2D 源码

```java
// Figure.java:500-517 - findMouseEventTargetInDescendantsAt
protected IFigure findMouseEventTargetInDescendantsAt(int x, int y) {
    PRIVATE_POINT.setLocation(x, y);
    translateFromParent(PRIVATE_POINT);  // 转换到父节点坐标系

    if (!getClientArea(Rectangle.SINGLETON).contains(PRIVATE_POINT)) {
        return null;  // 剪枝：不在父节点内，跳过
    }

    x = PRIVATE_POINT.x;
    y = PRIVATE_POINT.y;
    for (IFigure fig : getChildrenRevIterable()) {  // 逆序遍历
        if (fig.isVisible() && fig.isEnabled() && fig.containsPoint(x, y)) {
            fig = fig.findMouseEventTargetAt(x, y);
            if (fig != null) return fig;
        }
    }
    return null;
}
```

## 8. 事件通知 (Event System)

### 8.1 Figure 事件

| 事件 | 触发条件 | 用途 |
|------|----------|------|
| `fireFigureMoved()` | bounds 的位置或大小变化 | 通知已注册的 `FigureListener` |
| `fireCoordinateSystemChanged()` | 局部坐标系统变化并影响后代绝对边界 | 通知已注册的 `CoordinateListener` |

布局请求走 `invalidate/revalidate/UpdateManager`，Draw2D Figure 没有
`fireRequestLayout()` API。

### 8.2 FigureListener 接口

```java
public interface FigureListener {
    void figureMoved(IFigure source);
}
```

Draw2D 用同一个 `figureMoved` 表达位置和尺寸变化，没有独立的
`figureResized` 回调。

## 9. 核心概念依赖关系

```
                    ┌─────────────────────────────────────┐
                    │          Tree Structure             │
                    │     (parent/children Z-order)       │
                    └──────────────────┬──────────────────┘
                                       │
                    ┌──────────────────▼──────────────────┐
                    │          Bounds System              │
                    │ (bounds = 相对最近坐标根的绝对值)   │
                    │    • setBounds()                    │
                    │    • erase/repaint                  │
                    └──────────────────┬──────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         │                             │                             │
┌────────▼────────┐        ┌───────────▼──────────┐        ┌────────▼────────┐
│  Coordinate      │        │  Layout Manager      │        │  Event System   │
│  System          │        │  • invalidate()      │        │  • moved event  │
│  • primTranslate │        │  • revalidate()      │        │  • coord change │
│  • translate*()  │        │  • layout()          │        │                 │
└────────┬─────────┘        └───────────────────────┘        └─────────────────┘
         │                             │
         │                             ▼
         │              ┌───────────────────────────────┐
         │              │      Figure.paint Template    │
         │              │ (self → client/children → border)│
         │              └───────────────────────────────┘
         │                             │
         │                             ▼
         │              ┌───────────────────────────────┐
         └─────────────►│         Hit Test              │
                        │  • containsPoint()            │
                        │  • findFigureAt()             │
                        │  • TreeSearch 过滤            │
                        └───────────────────────────────┘
```

## 10. 实现边界

- tree、bounds、coordinate conversion、paint、hit-test、validation 和 damage
  是互相约束的协议，不能按彼此无关的功能实现。
- 手动 `setBounds()` 与 LayoutManager 都是 Draw2D 的正式能力；采用哪一种由
  container 策略决定，不能从源码推出“编辑器应优先手动布局”。
- Novadraw 的实现优先级和完成状态以 `doc/parity/draw2d/api-coverage.md` 与
  `doc/roadmap/` 为准，不在本源码概念文档中重复维护。

## 11. 参考源码

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
