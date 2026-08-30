# Eclipse Draw2d Figure 树操作机制分析

类型：`reference-analysis`

> 基于 Eclipse GEF Classic Figure.java (2387 行) 源码分析
> 来源: org.eclipse.draw2d.Figure

## 概述

本文档分析 draw2d Figure 接口中所有涉及树遍历的方法，按照**传播方向**和**遍历机制**分类，并按设计意图重新分组。

---

## 1. 涉及树操作的方法总览

### 1.1 按传播方向分类

#### 下行传播（父 → 子）

| 方法 | 遍历机制 | 传播内容 |
|------|-----------|----------|
| `primTranslate(int, int)` | **递归** | 位置增量到所有后代 |
| `addNotify()` | `forEach` 递归 | 实体化标记 |
| `removeNotify()` | `forEach` 递归 | 去实体化标记 |
| `invalidateTree()` | `forEach` 递归 | 无效标记到整棵子树 |
| `validate()` | `forEach` 递归 | 布局执行到子节点 |
| `setChildrenDirection(int)` | `forEach` 迭代 | 方向属性 |
| `setChildrenEnabled(boolean)` | `forEach` 迭代 | 启用状态 |
| `setChildrenOrientation(int)` | `forEach` 迭代 | 方向属性 |
| `removeAll()` | `forEach` 迭代 | 逐个调用 `remove()` |

#### 上行传播（子 → 父）

| 方法 | 遍历机制 | 传播内容 |
|------|-----------|----------|
| `revalidate()` | **递归** 向上 | 无效标记冒泡至根节点或验证根 |
| `translateToAbsolute(Translatable)` | **递归** 向上 | 坐标转绝对坐标 |
| `translateToRelative(Translatable)` | **递归** 向上 | 绝对坐标转相对坐标 |
| `erase()` | 单跳父调用 | 重绘请求（仅父节点） |
| `isShowing()` | 递归条件判断 | 可见性级联检查 |
| `getBackgroundColor()` | 递归条件回退 | 背景色继承 |
| `getForegroundColor()` | 递归条件回退 | 前景色继承 |
| `getFont()` | 递归条件回退 | 字体继承 |
| `getCursor()` | 递归条件回退 | 光标继承 |
| `getUpdateManager()` | 递归向上 | 寻找根的 UpdateManager |
| `isMirrored()` | 递归向上 | 检查祖先是否镜像 |

#### 双向搜索

| 方法 | 遍历机制 | 用途 |
|------|-----------|------|
| `findFigureAt(int, int, TreeSearch)` | **互相递归** 向下 | 命中测试（最深节点优先） |
| `findDescendantAtExcluding(int, int, TreeSearch)` | 迭代 + **递归** | 命中测试（排除某节点） |
| `findMouseEventTargetAt(int, int)` | **互相递归** 向下 | 鼠标事件目标查找 |
| `findMouseEventTargetInDescendantsAt(int, int)` | 迭代 + **递归** | 子树鼠标目标 |
| `paintChildren(Graphics)` | **互相递归** 向下 | 绘制所有可见子节点 |

### 1.2 遍历机制说明

| 机制 | 含义 | 风险 |
|------|------|------|
| **递归** | 方法直接调用自身 | 深层层次结构可能栈溢出 |
| **`forEach` 递归** | `forEach` 迭代调用 child 的递归方法 | 与递归等价 |
| **迭代 + 递归** | 外层 `for` 循环遍历子节点，每个 child 递归调用 | 风险同递归 |
| **单跳父调用** | 仅调用父节点一次，不继续向上 | 无栈溢出风险 |
| **条件回退** | 属性为 null 时递归调用父节点 | 深父链同样增长调用栈 |

---

## 2. 关键树操作详解

### 2.1 primTranslate -- 下行递归传播

```java
// Figure.java:1390-1398
protected void primTranslate(int dx, int dy) {
    bounds.x += dx;
    bounds.y += dy;
    if (useLocalCoordinates()) {
        fireCoordinateSystemChanged();  // 停止传播，触发事件
        return;
    }
    children.forEach(child -> child.translate(dx, dy));  // 递归向下
}

// Figure.java:2035-2039
public final void translate(int x, int y) {
    primTranslate(x, y);
    fireFigureMoved();
}
```

**关键点**：

- 基础 Figure 的 `useLocalCoordinates()` 为 `true` 时，只触发
  `fireCoordinateSystemChanged()`，**不修改**子节点 bounds
- 默认 `false`，递归调用 `child.translate(dx, dy)` 传播到所有后代
- `translate` 是 `final` 方法，不可 override
- 这是 draw2d 中**最深的无界递归路径**之一

### 2.2 revalidate -- 上行递归冒泡

```java
// Figure.java:1615-1622
public void revalidate() {
    invalidate();
    if (getParent() == null || isValidationRoot()) {
        getUpdateManager().addInvalidFigure(this);
    } else {
        getParent().revalidate();  // 递归向上
    }
}
```

**关键点**：

- 自身标记无效后，若无父节点或自己是验证根，则注册到 UpdateManager
- 否则递归向父节点冒泡，直到根节点或验证根
- 是 `setBounds`、`add`、`remove` 等操作的最终副作用链终点

### 2.3 translateToAbsolute / translateToRelative -- 上行递归

```java
// Figure.java:2055-2062
public final void translateToAbsolute(Translatable t) {
    if (getParent() != null) {
        Translatable tPrecise = toPreciseShape(t);
        getParent().translateToParent(tPrecise);
        getParent().translateToAbsolute(tPrecise);  // 递归向上
        fromPreciseShape(tPrecise, t);
    }
}

// Figure.java:2078-2085
public final void translateToRelative(Translatable t) {
    if (getParent() != null) {
        Translatable tPrecise = toPreciseShape(t);
        getParent().translateToRelative(tPrecise);  // 递归向上
        getParent().translateFromParent(tPrecise);  // 调整回
        fromPreciseShape(tPrecise, t);
    }
}
```

**关键点**：

- `translateToAbsolute` 从当前节点向上遍历父链，每个父节点累加其 `bounds.x/y` 偏移
- `translateToRelative` 逆向操作，从根向下反向调整
- 支持双精度浮点数转换（`toPreciseShape` / `fromPreciseShape`）

### 2.4 invalidateTree / validate -- 整棵子树操作

```java
// Figure.java:1122-1125
public void invalidateTree() {
    invalidate();
    children.forEach(IFigure::invalidateTree);  // 递归向下
}

// Figure.java:2174-2181
public void validate() {
    if (isValid()) return;
    setValid(true);
    layout();
    children.forEach(IFigure::validate);  // 递归向下
}
```

**关键点**：

- `invalidateTree` 通过 `forEach` 递归使整棵子树无效
- `validate` 先执行自身布局，再递归验证子节点（前序遍历）
- 两者都通过 `forEach` 实现，存在递归风险

### 2.5 add / remove -- 树结构变更

```java
// Figure.java:161-200
public void add(IFigure figure, Object constraint, int index) {
    // 1. 循环检测：向上遍历父链
    for (IFigure f = this; f != null; f = f.getParent()) {
        if (figure == f) throw new IllegalArgumentException("...");
    }
    if (figure.getParent() != null) {
        figure.getParent().remove(figure);  // 从旧父节点移除
    }
    children.add(index, figure);
    figure.setParent(this);
    if (layoutManager != null) layoutManager.setConstraint(figure, constraint);
    revalidate();                          // 冒泡向上
    if (getFlag(FLAG_REALIZED)) figure.addNotify();  // 通知新子节点
    figure.repaint();
}

// Figure.java:1408-1425
public void remove(IFigure figure) {
    if (getFlag(FLAG_REALIZED)) figure.removeNotify();  // 递归向下
    if (layoutManager != null) layoutManager.remove(figure);
    figure.erase();             // 单跳向上重绘
    figure.setParent(null);
    children.remove(figure);
    revalidate();               // 冒泡向上
}

// Figure.java:1433-1435
public void removeAll() {
    List<? extends IFigure> list = new ArrayList<>(getChildren());
    list.forEach(this::remove);  // 迭代，每次 remove 触发一次递归冒泡
}
```

### 2.6 命中测试 -- 互相递归向下

```java
// Figure.java:437-456
public IFigure findFigureAt(int x, int y, TreeSearch search) {
    if (!containsPoint(x, y)) return null;
    if (search.prune(this)) return null;
    IFigure child = findDescendantAtExcluding(x, y, search);  // 递归
    if (child != null) return child;
    if (search.accept(this)) return this;
    return null;
}

// Figure.java:395-413
protected IFigure findDescendantAtExcluding(int x, int y, TreeSearch search) {
    PRIVATE_POINT.setLocation(x, y);
    translateFromParent(PRIVATE_POINT);
    if (!getClientArea(Rectangle.SINGLETON).contains(PRIVATE_POINT)) return null;
    x = PRIVATE_POINT.x; y = PRIVATE_POINT.y;
    for (IFigure fig : getChildrenRevIterable()) {         // 逆序迭代
        if (fig.isVisible()) {
            fig = fig.findFigureAt(x, y, search);          // 递归调用
            if (fig != null) return fig;
        }
    }
    return null;
}
```

**关键点**：

- `findFigureAt` 与 `findDescendantAtExcluding` **互相递归**
- 逆序遍历子节点（Z-order 上层优先）
- `TreeSearch` 接口提供剪枝和接受策略
- `Layer` 覆盖 `containsPoint` 和 `findFigureAt` 实现透明层穿透

### 2.7 paint -- 互相递归绘制

```java
// Figure.java:1250-1275
public void paint(Graphics graphics) {
    graphics.pushState();
    try {
        paintFigure(graphics);         // 绘制自身
        paintClientArea(graphics);     // 内部调用 paintChildren
        paintBorder(graphics);         // 边框覆盖在最上层
    } finally {
        graphics.popState();
    }
}

// Figure.java:1296-1314
protected void paintChildren(Graphics graphics) {
    for (IFigure child : children) {       // 迭代
        if (child.isVisible()) {
            // ... 裁剪处理
            child.paint(graphics);          // 递归调用 child.paint
        }
    }
}
```

**关键点**：

- `paint()` → `paintClientArea()` → `paintChildren()` → `child.paint()` 形成**互相递归链**
- 这是 draw2d 中**最深、最无界的递归路径** -- 任意深度的可见子树都会触发栈增长
- 裁剪策略 (`clippingStrategy`) 为每个子节点维护独立的裁剪区域

### 2.8 Layer 透明层覆盖

```java
// Layer.java:30-46
public boolean containsPoint(int x, int y) {
    if (isOpaque()) return super.containsPoint(x, y);  // 简单 bounds 检查
    Point pt = Point.SINGLETON;
    pt.setLocation(x, y);
    translateFromParent(pt);
    x = pt.x; y = pt.y;
    for (IFigure child : getChildren()) {              // 迭代，非递归
        if (child.containsPoint(x, y)) return true;
    }
    return false;
}

// Layer.java:53-62
public IFigure findFigureAt(int x, int y, TreeSearch search) {
    if (!isEnabled()) return null;
    IFigure f = super.findFigureAt(x, y, search);
    if (f == this) return null;  // 点击穿透，返回 null
    return f;
}
```

**关键点**：

- `Layer.containsPoint` 用**迭代**替代简单边界检查
- `Layer.findFigureAt` 若命中自身则返回 null（透明穿透）
- 每个子节点的 `containsPoint` 是简单 bounds 检查，不继续递归

---

## 3. 按设计意图分组的完整接口

### 3.1 渲染（Painting）

```java
paint(Graphics)                    // 编排器
paintFigure(Graphics)              // 自渲染（背景填充）
paintClientArea(Graphics)          // 自渲染 + 触发子绘制
paintChildren(Graphics)            // 迭代绘制子节点（互相递归）
paintBorder(Graphics)
optimizeClip()
```

> 树操作：`paintChildren()` 是互相递归链的入口，存在栈溢出风险。

### 3.2 几何 / 边界（Geometry / Bounds）

```java
getBounds() / setBounds(Rectangle)
getClientArea() / getClientArea(Rectangle)
getSize() / setSize(Dimension) / setSize(int, int)
getLocation() / setLocation(Point)
intersects(Rectangle)
```

> 树操作：`setBounds()` 通过 `primTranslate()` 触发向下递归传播。

### 3.3 坐标系统（Coordinate System）

```java
translate(int, int)                // 公开入口
primTranslate(int, int)            // 受保护，递归向下传播
translateToParent(Translatable)
translateFromParent(Translatable)
translateToAbsolute(Translatable)  // 递归向上
translateToRelative(Translatable)   // 递归向上
useLocalCoordinates()             // 基础 Figure 的 child-local 坐标控制
useDoublePrecision()
isCoordinateSystem()
toPreciseShape() / fromPreciseShape()
```

> 树操作：`primTranslate` 递归向下；`translateToAbsolute/Relative` 递归向上。

### 3.4 树结构 / 生命周期（Tree Structure / Lifecycle）

```java
add(IFigure) / add(IFigure, int) / add(IFigure, Object, int) / add(IFigure, Object)
remove(IFigure)
removeAll()
getChildren() / getChildrenRevIterable()
getParent() / setParent(IFigure)
addNotify() / removeNotify()
```

> 树操作：`add` 包含父链循环检测；`remove` 触发向下 `removeNotify` 和向上 `revalidate`；`addNotify/removeNotify` 通过 `forEach` 递归。

### 3.5 命中测试 / 包含检测（Containment / Hit Testing）

```java
containsPoint(Point) / containsPoint(int, int)
findFigureAt(int, int, TreeSearch)         // 互相递归
findDescendantAtExcluding(int, int, TreeSearch)
findFigureAtExcluding(int, int, Collection)
findMouseEventTargetAt(int, int)            // 互相递归
findMouseEventTargetInDescendantsAt(int, int)
getClippingStrategy() / setClippingStrategy()
```

> 树操作：`findFigureAt` 族全部涉及互相递归向下搜索。

### 3.6 验证与布局（Validation & Layout）

```java
invalidate()                       // 自操作
invalidateTree()                   // 递归向下（forEach）
validate()                         // 递归向下（forEach）
isValid() / setValid(boolean)
isValidationRoot()                 // override 点
layout()
revalidate()                       // 递归向上冒泡
getLayoutManager() / setLayoutManager()
getMinimumSize() / getMinimumSize(int, int)
getPreferredSize() / getPreferredSize(int, int)
getMaximumSize()
setMinimumSize() / setMaximumSize() / setPreferredSize()
setConstraint()
addLayoutListener() / removeLayoutListener()
```

> 树操作：`invalidateTree` 和 `validate` 通过 `forEach` 递归向下；`revalidate` 递归向上冒泡。

### 3.7 重绘与擦除（Repaint / Erase）

```java
repaint() / repaint(int, int, int, int) / repaint(Rectangle)
erase()
```

> 树操作：`erase()` 单跳向上调用父节点 `repaint()`；`repaint()` 无树遍历，直接注册到 UpdateManager。

### 3.8 可见性与启用状态（Visibility / Enabled）

```java
isVisible() / setVisible(boolean)   // erase() + revalidate() 链
isShowing()                          // 递归向上
isEnabled() / setEnabled(boolean)
isOpaque() / setOpaque(boolean)
setChildrenEnabled(boolean)         // 迭代传播
setChildrenDirection(int)           // 迭代传播
setChildrenOrientation(int)         // 迭代传播
```

> 树操作：`setVisible` 组合 `erase()` 和 `revalidate()` 链；批量操作方法通过 `forEach` 迭代。

### 3.9 样式 / 外观（Styling / Appearance）

```java
getBorder() / setBorder()
getForegroundColor() / setForegroundColor()
getLocalForegroundColor()
getBackgroundColor() / setBackgroundColor()
getLocalBackgroundColor()
getFont() / setFont() / getLocalFont()
getCursor() / setCursor()
getInsets()
```

> 树操作：属性 getter 在值为 null 时向上回退（条件调用，非递归）。

### 3.10 焦点管理（Focus）

```java
hasFocus()
requestFocus()
isFocusTraversable() / setFocusTraversable(boolean)
isRequestFocusEnabled() / setRequestFocusEnabled(boolean)
```

> 无树操作。

### 3.11 事件分发（Event Dispatch）

```java
handleFocusGained() / handleFocusLost()
handleKeyPressed() / handleKeyReleased()
handleMouseEntered() / handleMouseExited()
handleMouseMoved() / handleMouseDragged()
handleMouseHovered() / handleMousePressed() / handleMouseReleased()
handleMouseDoubleClicked()
handleMouseWheelScrolled()
internalGetEventDispatcher()
isMouseEventTarget()
```

> 无树操作。事件分发不遍历树，事件直接派发给当前 Figure。

### 3.12 监听器管理（Listener Management）

```java
add/removeAncestorListener
add/removeFigureListener
add/removeFocusListener
add/removeKeyListener
add/removeMouseListener
add/removeMouseMotionListener
add/removeMouseWheelListener
add/removeCoordinateListener
add/removePropertyChangeListener(...)
addListener(Class, Object) / removeListener(Class, Object)
getListeners(Class) / getListenersIterable(Class)
```

> 无树操作。所有监听器管理是节点本地的，不涉及树传播。

### 3.13 通知 / 事件触发（Fire / Notification）

```java
fireFigureMoved()
fireCoordinateSystemChanged()
fireMoved()               // deprecated
firePropertyChange(...)   // 3 个重载
```

> 无树操作。事件触发仅通知当前节点的监听器，不向上或向下传播。

### 3.14 工具提示（Tooltip）

```java
getToolTip() / setToolTip(IFigure)
```

> 无树操作。

### 3.15 内部基础设施（Internal Infrastructure）

```java
getUpdateManager()         // 递归向上寻找根的 UpdateManager
getFlag() / setFlag()
```

---

## 4. 关键设计观察

### 4.1 useLocalCoordinates 是传播的门控

```text
useLocalCoordinates = false（默认）:
    primTranslate     → 递归向下传播位置
    translateToParent → 无操作

useLocalCoordinates = true（基础 Figure 为 children 建立局部域）:
    primTranslate     → 触发事件，停止传播
    translateToParent → 执行坐标变换
```

这是 draw2d 区分"绝对坐标模式"和"相对坐标模式"的核心机制。

### 4.2 forEach 迭代本质是递归

`forEach(child -> child.method())` 在语义上等价于递归调用。
Java 的 `forEach` 底层通过 `Iterator` 实现，但 lambda 体内的方法调用本身是递归的。
如果每个 child 的 `method()` 本身递归，则整体仍然是递归。

`forEach` 本身只枚举同层 children。若 lambda 调用 `child.validate()`、
`child.invalidateTree()`、`child.addNotify()` 或 `child.removeNotify()`，整条调用链仍是
递归；而 `setChildrenDirection()` 这类只改单层 child 的方法则是纯同层迭代。

### 4.3 互相递归链的深度风险

draw2d 中存在两条互相递归链：

1. **绘制链**: `paint()` ↔ `paintChildren()` ↔ `child.paint()`
2. **命中测试链**: `findFigureAt()` ↔ `findDescendantAtExcluding()` ↔ `child.findFigureAt()`

两条链的深度取决于**可见子树的深度**，而非树的宽度。

### 4.4 revalidate 是上行传播的单一入口

所有导致布局失效的操作（`setBounds`、`add`、`remove`、`setBorder`、`setLayoutManager` 等）最终都通过 `revalidate()` 冒泡到根节点。UpdateManager 批量处理这些失效请求，避免每次变更立即触发完整重绘。

### 4.5 Layer 对树操作的特殊覆盖

`Layer` 通过覆盖 `containsPoint` 和 `findFigureAt`，将"透明容器"语义从 Figure 的默认行为中分离出来。这避免了为每个透明容器重新实现整棵树遍历逻辑。

### 4.6 树传播主要由 Figure 方法递归协作

Draw2D 没有独立的 SceneGraph 对象。树关系、绘制、命中、验证、坐标传播和样式继承
主要由 `Figure`/`IFigure` 方法沿 parent 或 children 链协作完成；
`UpdateManager` 只收集 invalid/dirty 请求并驱动 validation 与 repaint。

---

## 5. 源码位置参考

| 文件 | 核心内容 |
|------|----------|
| `Figure.java` | 核心实现，所有树操作方法 |
| `IFigure.java` | 接口定义 |
| `Layer.java` | 透明层覆盖 |
| `ScalableLayeredPane.java` | 缩放容器 |
| `FreeformLayer.java` | 自由表单层（覆盖 `primTranslate`） |
| `UpdateManager.java` | 更新管理器（不直接遍历树） |
| `FigureListener.java` | 移动事件接口 |

---

## 6. 与 Novadraw 设计的对应

| draw2d 机制 | Novadraw 对应 | 差异 |
|-------------|--------------|------|
| `primTranslate` 递归向下 | FigureGraph 坐标协议 | 保留 `useLocalCoordinates()` 处停止修改后代 bounds 的语义 |
| `revalidate` 递归向上 | FigureGraph + UpdateManager | 保留冒泡到 validation root 的语义 |
| `paintChildren` 互相递归 | 当前递归渲染主线 | 保留 Draw2D 的深度优先阶段顺序 |
| `findFigureAt` 互相递归 | FigureGraph 命中协议 | 保留逆序 Z-order 与坐标降域 |
| `forEach` 树操作 | FigureGraph 生命周期操作 | 区分同层迭代与递归传播 |
| `useLocalCoordinates` 门控 | Figure 的 child-local 坐标标记 | 对应 |
| `UpdateManager` 批量失效 | `SceneUpdateManager` | 对应 |

---

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
