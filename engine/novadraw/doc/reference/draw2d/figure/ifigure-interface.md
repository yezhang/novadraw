# Eclipse Draw2D IFigure 接口分析

类型：`reference-analysis`

> 基于 Eclipse GEF Classic 源码分析
> 来源: org.eclipse.draw2d.IFigure

## 概述

IFigure 是 Draw2D 的核心接口，集中定义树、几何、布局、事件、样式、绘制与更新
协议。称它为“宽接口”是源码事实；称它为“上帝接口”属于架构评价，不是 Draw2D
源码声明，也不能简单归因于 Java 单一继承。

## 接口方法分类

### 1. 层次结构管理（Tree Management）

| 方法签名 | 作用 |
|---------|------|
| `void add(IFigure figure)` | 添加子图形 |
| `void add(IFigure figure, int index)` | 添加到指定位置 |
| `void add(IFigure figure, Object constraint)` | 添加带约束的子图形 |
| `void add(IFigure figure, Object constraint, int index)` | 添加带约束到指定位置 |
| `void remove(IFigure figure)` | 移除子图形 |
| `List<? extends IFigure> getChildren()` | 获取子图形列表（调用方不得修改） |
| `IFigure getParent()` | 获取父图形 |
| `void setParent(IFigure parent)` | 设置父图形 |
| `void addNotify()` | 添加通知 |
| `void removeNotify()` | 移除通知 |

**设计意图**：
- 支持 Z-order（通过 index 控制绘制顺序）
- 支持布局约束（Object constraint）
- 循环检测（防止父子关系循环）

### 2. 几何属性（Geometry）

| 方法签名 | 作用 |
|---------|------|
| `Rectangle getBounds()` | 获取外接矩形 |
| `void setBounds(Rectangle rect)` | 设置位置和大小 |
| `Point getLocation()` | 获取位置 |
| `void setLocation(Point p)` | 设置位置 |
| `Dimension getSize()` | 获取尺寸 |
| `void setSize(Dimension d)` | 设置尺寸 |
| `void setSize(int w, int h)` | 设置尺寸 |
| `Rectangle getClientArea()` | 获取客户区域（子元素布局区域） |
| `Rectangle getClientArea(Rectangle rect)` | 获取客户区域（带缓存） |
| `Insets getInsets()` | 获取内边距 |

**设计意图**：
- bounds 是 Figure 的核心几何属性
- clientArea = bounds - insets，用于子元素布局
- 使用 Rectangle/Dimension 而非分离的 x/y/width/height

### 3. 坐标变换（Transformation）

| 方法签名 | 作用 |
|---------|------|
| `void translate(int x, int y)` | 平移图形 |
| `void translateToParent(Translatable t)` | 当前 Figure 坐标 → 父坐标 |
| `void translateFromParent(Translatable t)` | 父坐标 → 当前 Figure 坐标 |
| `void translateToAbsolute(Translatable t)` | 相对坐标 → 绝对坐标 |
| `void translateToRelative(Translatable t)` | 绝对坐标 → 相对坐标 |
| `boolean isCoordinateSystem()` | 是否使用本地坐标系统 |

**设计意图**：
- 支持嵌套的坐标系统
- 本地坐标模式下，子元素使用相对于父图形的位置
- 坐标变换是双向的

### 4. 可见性与状态（Visibility & State）

| 方法签名 | 作用 |
|---------|------|
| `boolean isVisible()` | 是否可见（自身标志） |
| `boolean isShowing()` | 是否显示（自身+父可见） |
| `void setVisible(boolean visible)` | 设置可见性 |
| `boolean isEnabled()` | 是否启用 |
| `void setEnabled(boolean value)` | 设置启用状态 |
| `boolean isOpaque()` | 是否不透明 |
| `void setOpaque(boolean isOpaque)` | 设置不透明 |
| `boolean hasFocus()` | 是否有焦点 |
| `void requestFocus()` | 请求焦点 |
| `boolean isRequestFocusEnabled()` | 是否可以请求焦点 |
| `boolean isFocusTraversable()` | 焦点是否可遍历 |
| `void setFocusTraversable(boolean value)` | 设置焦点可遍历 |
| `boolean isMirrored()` | 是否镜像（RTL 语言支持） |

**设计意图**：
- `isVisible()` vs `isShowing()` 区分：
  - `isVisible()`：自身可见性标志
  - `isShowing()`：递归检查父级可见性
- 状态变化自动触发重绘

### 5. 渲染（Rendering）

| 方法签名 | 作用 |
|---------|------|
| `void paint(Graphics graphics)` | 绘制自身和子元素 |
| `boolean containsPoint(int x, int y)` | 命中测试（基于 bounds） |
| `boolean containsPoint(Point p)` | 命中测试（点参数） |
| `IFigure findFigureAt(int x, int y)` | 查找指定位置的图形 |
| `IFigure findFigureAt(int x, int y, TreeSearch search)` | 条件查找 |
| `IFigure findFigureAt(Point p)` | 查找（点参数） |
| `IFigure findFigureAtExcluding(int x, int y, Collection<IFigure> collection)` | 排除查找 |
| `IFigure findMouseEventTargetAt(int x, int y)` | 查找鼠标事件目标 |
| `boolean intersects(Rectangle rect)` | 矩形相交检测 |
| `void erase()` | 擦除图形 |

**设计意图**：
- `containsPoint()` 默认使用 AABB 检测，子类可覆盖
- `findFigureAt()` 用于事件派发
- `intersects()` 用于视锥裁剪

### 6. 布局（Layout）

| 方法签名 | 作用 |
|---------|------|
| `LayoutManager getLayoutManager()` | 获取布局管理器 |
| `void setLayoutManager(LayoutManager lm)` | 设置布局管理器 |
| `Dimension getPreferredSize()` | 获取首选尺寸 |
| `Dimension getPreferredSize(int wHint, int hHint)` | 带提示的首选尺寸 |
| `Dimension getMinimumSize()` | 获取最小尺寸 |
| `Dimension getMinimumSize(int wHint, int hHint)` | 带提示的最小尺寸 |
| `Dimension getMaximumSize()` | 获取最大尺寸 |
| `void setPreferredSize(Dimension size)` | 设置首选尺寸 |
| `void setMinimumSize(Dimension size)` | 设置最小尺寸 |
| `void setMaximumSize(Dimension size)` | 设置最大尺寸 |
| `void setConstraint(IFigure child, Object constraint)` | 设置布局约束 |
| `void revalidate()` | 重新验证 |
| `void invalidate()` | 使布局无效 |
| `void invalidateTree()` | 使整棵树无效 |
| `void validate()` | 执行验证 |

**设计意图**：
- 布局管理器模式，支持多种布局算法
- 尺寸提示（hint）允许约束计算
- 延迟布局（revalidate）优化性能

### 7. 样式属性（Style）

| 方法签名 | 作用 |
|---------|------|
| `Color getBackgroundColor()` | 获取背景色（可继承） |
| `Color getLocalBackgroundColor()` | 获取本地背景色 |
| `void setBackgroundColor(Color c)` | 设置背景色 |
| `Color getForegroundColor()` | 获取前景色（可继承） |
| `Color getLocalForegroundColor()` | 获取本地前景色 |
| `void setForegroundColor(Color c)` | 设置前景色 |
| `Font getFont()` | 获取字体（可继承） |
| `void setFont(Font f)` | 设置字体 |
| `Border getBorder()` | 获取边框 |
| `void setBorder(Border b)` | 设置边框 |
| `Cursor getCursor()` | 获取光标 |
| `void setCursor(Cursor cursor)` | 设置光标 |
| `IFigure getToolTip()` | 获取提示 |
| `void setToolTip(IFigure figure)` | 设置提示 |

**设计意图**：
- 颜色/字体支持继承机制
- Border 接口支持装饰性边框
- ToolTip 可以是任意 IFigure（支持丰富内容）

### 8. 事件监听（Event Listeners）

| 方法签名 | 作用 |
|---------|------|
| `void addMouseListener(MouseListener listener)` | 鼠标点击 |
| `void removeMouseListener(MouseListener listener)` | 移除 |
| `void addMouseMotionListener(MouseMotionListener listener)` | 鼠标移动 |
| `void removeMouseMotionListener(MouseMotionListener listener)` | 移除 |
| `void addKeyListener(KeyListener listener)` | 键盘事件 |
| `void removeKeyListener(KeyListener listener)` | 移除 |
| `void addFocusListener(FocusListener listener)` | 焦点事件 |
| `void removeFocusListener(FocusListener listener)` | 移除 |
| `void addFigureListener(FigureListener listener)` | 图形事件 |
| `void removeFigureListener(FigureListener listener)` | 移除 |
| `void addMouseWheelListener(MouseWheelListener listener)` | 鼠标滚轮 |
| `void removeMouseWheelListener(MouseWheelListener listener)` | 移除 |
| `void addPropertyChangeListener(PropertyChangeListener listener)` | 属性变化 |
| `void addPropertyChangeListener(String property, PropertyChangeListener listener)` | 指定属性 |
| `void removePropertyChangeListener(PropertyChangeListener listener)` | 移除 |
| `void removePropertyChangeListener(String property, PropertyChangeListener listener)` | 移除指定属性 |
| `void addAncestorListener(AncestorListener listener)` | 祖先变化 |
| `void removeAncestorListener(AncestorListener listener)` | 移除 |
| `void addCoordinateListener(CoordinateListener listener)` | 坐标变化 |
| `void removeCoordinateListener(CoordinateListener listener)` | 移除 |
| `void addLayoutListener(LayoutListener listener)` | 布局变化 |
| `void removeLayoutListener(LayoutListener listener)` | 移除 |

**设计意图**：
- 观察者模式，解耦事件和生产者
- 支持特定属性监听（String property 参数）
- 事件处理使用 handle* 方法而非直接监听

### 9. 更新管理（Update Management）

| 方法签名 | 作用 |
|---------|------|
| `void repaint()` | 重绘 |
| `void repaint(int x, int y, int w, int h)` | 局部重绘 |
| `void repaint(Rectangle rect)` | 局部重绘（矩形参数） |
| `UpdateManager getUpdateManager()` | 获取更新管理器 |

**设计意图**：
- 局部重绘支持增量更新
- UpdateManager 调度重绘请求

### 10. 内部方法（Internal）

| 方法签名 | 作用 |
|---------|------|
| `EventDispatcher internalGetEventDispatcher()` | 获取事件分发器 |

## 设计意图总结

### 1. 宽接口与可替换性

IFigure 包含 100+ 方法，涉及：
- 树结构管理
- 几何属性
- 布局系统
- 事件系统
- 样式系统
- 渲染系统

这让所有 Figure 实现都能参与同一套树、绘制、布局、命中和事件协议，代价是实现面
较大。是否在 Rust 版本中拆分，应作为 Novadraw 的设计选择单独论证。

### 2. 继承 vs 组合

颜色、字体等使用**继承机制**：
```java
getBackgroundColor() {
    if (localBgColor != null) return localBgColor;
    if (parent != null) return parent.getBackgroundColor();
    return null;
}
```

### 3. 尺寸回退与布局缓存

`bounds` 是直接保存的字段，不是懒计算值。`Figure.getPreferredSize()` 按显式
preferred size、LayoutManager 计算值、当前 size 的顺序回退；常用
`AbstractLayout` 实现可以缓存 preferred size，并在 `invalidate()` 时清空缓存。

```java
getPreferredSize(wHint, hHint) {
    if (prefSize != null) return prefSize;
    if (getLayoutManager() != null) {
        Dimension d = getLayoutManager().getPreferredSize(this, wHint, hHint);
        if (d != null) return d;
    }
    return getSize();
}
```

### 4. 模板方法

paint() 方法使用模板方法模式：
```java
paint(graphics) {
    initProperties();
    paintFigure();
    paintClientArea();
    paintBorder();
}
```

### 5. 验证机制

布局使用延迟验证：
```java
invalidate()   // 标记无效
revalidate()   // 请求重新验证
validate()     // 执行验证
```

## 与其他类的关系

```
IFigure (接口)
    │
    └── Figure (基类实现)
            │
            ├── Shape (添加线宽/样式)
            │       │
            │       ├── RectangleFigure
            │       ├── Ellipse
            │       └── AbstractPointListShape
            │               ├── Polyline
            │               └── Polygon
            │
            └── 其他容器 Figure
```

## 参考

- 源码位置: `org.eclipse.draw2d.IFigure`
- 实现类: `org.eclipse.draw2d.Figure`
- 继承层次: Figure → Shape → 具体图形
- 源码基线: eclipse/gef-classic commit `4463d9d0c`（2026-01-01）
