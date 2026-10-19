# Eclipse Draw2D Figure 类实现分析

类型：`reference-analysis`

> 基于 Eclipse GEF Classic 源码分析
> 来源: org.eclipse.draw2d.Figure

## 概述

Figure 是 IFigure 接口的基类实现，包含了图形组件的核心逻辑实现。

## 核心数据结构

### 1. 标志位系统（Flags）

```java
private static final int FLAG_VALID = 1;
private static final int FLAG_OPAQUE = 1 << 1;
private static final int FLAG_VISIBLE = 1 << 2;
private static final int FLAG_FOCUSABLE = 1 << 3;
private static final int FLAG_ENABLED = 1 << 4;
private static final int FLAG_FOCUS_TRAVERSABLE = 1 << 5;
```

**设计意图**：
- 使用位运算存储多个布尔状态，节省内存
- 支持快速的状态检查：`getFlag(FLAG_VISIBLE)`
- 子类可以扩展标志位

### 2. 字段定义

```java
protected Rectangle bounds = new Rectangle(0, 0, 0, 0);  // 图形边界
private IFigure parent;                                    // 父图形
private List<IFigure> children = Collections.emptyList(); // 子图形列表
private LayoutManager layoutManager;                       // 布局管理器
private IClippingStrategy clippingStrategy = null;         // 裁剪策略
private Cursor cursor;                                     // 光标

// 样式属性
// 当前基线仍为 protected，但已标记 deprecated-for-removal，
// 客户端应改用 local property accessor
protected Color bgColor;   // 背景色
protected Color fgColor;   // 前景色
protected Font font;       // 字体
protected Border border;   // 边框

// 尺寸约束
protected Dimension prefSize;  // 首选尺寸
protected Dimension minSize;    // 最小尺寸
protected Dimension maxSize;     // 最大尺寸

// 工具
private PropertyChangeSupport propertyListeners;
private final EventListenerList eventListeners = new EventListenerList();
```

**设计意图**：
- 使用 `Collections.emptyList()` 避免 null 检查
- 子类可访问的 protected 字段允许扩展

## 核心方法实现

### 1. paint() - 模板方法模式

```java
public void paint(Graphics graphics) {
    // 1. 设置属性
    if (getLocalBackgroundColor() != null) {
        graphics.setBackgroundColor(getLocalBackgroundColor());
    }
    if (getLocalForegroundColor() != null) {
        graphics.setForegroundColor(getLocalForegroundColor());
    }
    if (getLocalFont() != null) {
        graphics.setFont(getLocalFont());
    }

    // 2. 保存图形状态
    graphics.pushState();
    try {
        // 3. 绘制自身
        paintFigure(graphics);
        graphics.restoreState();

        // 4. 绘制子元素
        paintClientArea(graphics);

        // 5. 绘制边框
        paintBorder(graphics);
    } finally {
        graphics.popState();
    }
}
```

**绘制顺序**：

```text
paint(Graphics)
    │
    ├── 1. 设置本地属性
    │       setBackgroundColor()
    │       setForegroundColor()
    │       setFont()
    │
    ├── 2. pushState()          ── 保存完整状态
    │
    ├── 3. paintFigure()        ── 绘制自身（子类覆盖）
    │       │
    │       └── fillShape() / outlineShape()
    │
    ├── 4. restoreState()       ── 恢复颜色（保留变换）
    │
    ├── 5. paintClientArea()   ── 绘制子元素
    │       ├── translate(bounds + insets)
    │       ├── clipRect(clientArea)
    │       ├── pushState()
    │       └── for child in children
    │               child.paint()  ── 递归
    │
    ├── 6. paintBorder()       ── 绘制装饰边框
    │
    └── 7. popState()          ── 恢复完整状态
```

**状态保护示意**:

```text
pushState()         popState()
    │                   │
    ▼                   ▲
┌──────────────────────────────────────────┐
│  ┌────────────────────────────────────┐  │
│  │  完整状态 (变换+裁剪+颜色)          │  │  ← pop 恢复
│  │  ┌──────────────────────────────┐  │  │
│  │  │  应用状态 (当前生效)         │  │  │  ← restore 恢复
│  │  │  ┌────────────────────────┐  │  │  │
│  │  │  │  绘制操作              │  │  │  │
│  │  │  └────────────────────────┘  │  │  │
│  │  └──────────────────────────────┘  │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
        ▲ push 压入          ▼ pop 弹出
```

**设计意图**：
- 模板方法模式：统一流程，子类只需实现特定阶段
- 使用 pushState/popState 保护图形状态
- 使用 restoreState 在 paintFigure 后恢复，再绘制子元素

### 2. setBounds() - 核心属性设置

```java
public void setBounds(Rectangle rect) {
    int x = bounds.x, y = bounds.y;
    boolean resize = (rect.width != bounds.width) || (rect.height != bounds.height);
    boolean translate = (rect.x != x) || (rect.y != y);

    // 如果大小或位置变化且可见，先擦除
    if ((resize || translate) && isVisible()) {
        erase();
    }

    // 平移处理
    if (translate) {
        int dx = rect.x - x;
        int dy = rect.y - y;
        primTranslate(dx, dy);  // 默认递归平移；useLocalCoordinates() 处停止修改后代 bounds
    }

    // 更新尺寸
    bounds.width = rect.width;
    bounds.height = rect.height;

    // 触发更新
    if (translate || resize) {
        if (resize) {
            invalidate();  // 布局失效
        }
        fireFigureMoved();  // 触发移动事件
        repaint();         // 重绘
    }
}
```

**设计意图**：
- 分离平移和缩放处理
- 先擦除旧位置，再移动，最后重绘
- 调用 `primTranslate` 更新自身位置；仅在 `useLocalCoordinates() == false`
  时递归传播到子元素
- 自动触发事件和重绘

### 3. containsPoint() - 命中测试

```java
public boolean containsPoint(int x, int y) {
    return getBounds().contains(x, y);
}
```

**设计意图**：
- 默认使用 AABB（轴对齐包围盒）检测
- 子类可以覆盖实现精确检测（如 Polyline）
- 简单高效

### 4. 子元素管理 add()

```java
public void add(IFigure figure, Object constraint, int index) {
    // 1. 延迟初始化 children 列表
    if (children.equals(Collections.emptyList())) {
        children = new ArrayList<>(2);
    }

    // 2. 循环检测
    for (IFigure f = this; f != null; f = f.getParent()) {
        if (figure == f) {
            throw new IllegalArgumentException("Figure being added introduces cycle");
        }
    }

    // 3. 从原父节点移除
    if (figure.getParent() != null) {
        figure.getParent().remove(figure);
    }

    // 4. 添加到列表
    if (index == -1) {
        children.add(figure);
    } else {
        children.add(index, figure);
    }

    // 5. 设置父子关系
    figure.setParent(this);

    // 6. 设置布局约束
    if (layoutManager != null) {
        layoutManager.setConstraint(figure, constraint);
    }

    // 7. 触发更新
    revalidate();
    if (getFlag(FLAG_REALIZED)) {
        figure.addNotify();
    }
    figure.repaint();
}
```

**设计意图**：
- 循环检测防止无限递归
- 自动处理父子关系
- 自动触发重绘和布局更新

### 5. paintFigure() - 默认实现

```java
protected void paintFigure(Graphics graphics) {
    if (isOpaque()) {
        graphics.fillRectangle(getBounds());
    }
    if (getBorder() instanceof AbstractBackground abstractBackground) {
        abstractBackground.paintBackground(this, graphics, NO_INSETS);
    }
}
```

**设计意图**：
- 不透明图形填充背景
- 支持背景边框绘制

### 6. paintClientArea() - 子元素绘制

```java
protected void paintClientArea(Graphics graphics) {
    if (children.isEmpty()) {
        return;
    }

    if (useLocalCoordinates()) {
        graphics.translate(getBounds().x + getInsets().left, getBounds().y + getInsets().top);
        if (!optimizeClip()) {
            graphics.clipRect(getClientArea(PRIVATE_RECT));
        }
        graphics.pushState();
        paintChildren(graphics);
        graphics.popState();
        graphics.restoreState();
    } else if (optimizeClip()) {
        paintChildren(graphics);
    } else {
        graphics.clipRect(getClientArea(PRIVATE_RECT));
        graphics.pushState();
        paintChildren(graphics);
        graphics.popState();
        graphics.restoreState();
    }
}
```

**设计意图**：
- 使用本地坐标时平移到客户区起点
- 非优化路径裁剪到 client area
- `paintChildren()` 再按 clipping strategy 或 child bounds 逐个裁剪并递归调用
  `child.paint()`

### 7. erase() - 擦除

```java
public void erase() {
    if (getParent() == null || !isVisible()) {
        return;
    }
    Rectangle r = new Rectangle(getBounds());
    getParent().translateToParent(r);
    getParent().repaint(r.x, r.y, r.width, r.height);
}
```

**设计意图**：
- 只处理可见且有父节点的图形
- 将旧 bounds 转到父坐标域后，请求父节点重绘该区域；它不是立即清屏

## 关键设计模式

### 1. 模板方法模式

```java
// 父类定义骨架
paint(Graphics) {
    initProperties();
    paintFigure();     // 子类实现
    paintChildren();   // 父类实现
    paintBorder();     // 子类实现
}
```

### 2. 观察者模式

```java
// 几何变化使用专门的 FigureListener
addFigureListener(listener);
fireFigureMoved();

// parent 等命名属性使用 PropertyChangeSupport
firePropertyChange("parent", oldParent, newParent);
```

### 3. 策略模式

```java
// 布局策略
layoutManager = new FlowLayout();
layoutManager.layout(container);

// 裁剪策略
clippingStrategy = new RectangleClippingStrategy();
```

### 4. 延迟初始化

```java
// 延迟创建 children 列表
if (children.equals(Collections.emptyList())) {
    children = new ArrayList<>(2);
}
```

### 5. 位标志

```java
// 高效的状态存储
flags = FLAG_VISIBLE | FLAG_ENABLED;
if (getFlag(FLAG_VISIBLE)) { ... }
```

## 事件触发机制

### 1. fireFigureMoved()

```java
protected void fireFigureMoved() {
    eventListeners.getListenersIterable(FigureListener.class)
        .forEach(listener -> listener.figureMoved(this));
}
```

`setBounds()` 不会为 bounds 额外发送 JavaBeans property change；几何通知走
`FigureListener`。

### 2. invalidate()

```java
public void invalidate() {
    setFlag(FLAG_VALID, false);
    if (layoutManager != null) {
        layoutManager.invalidate();
    }
}
```

### 3. revalidate()

```java
public void revalidate() {
    invalidate();
    if (getParent() == null || isValidationRoot()) {
        getUpdateManager().addInvalidFigure(this);
    } else {
        getParent().revalidate();
    }
}
```

## 与 Shape 的关系

```text
IFigure (接口, 1089行)
    │
    ▼
Figure (基类, ~2000行)
    │
    ├── 字段
    │   ├── bounds: Rectangle          // 边界
    │   ├── parent: IFigure            // 父节点
    │   ├── children: List<IFigure>   // 子节点
    │   ├── layoutManager: LayoutManager
    │   ├── border: Border
    │   ├── bgColor, fgColor, font
    │   └── prefSize, minSize, maxSize
    │
    ├── 模板方法
    │   ├── paint(Graphics)            // 固定流程
    │   ├── paintFigure()              // 子类覆盖
    │   ├── paintClientArea()          // 绘制子节点
    │   └── paintBorder()              // 绘制边框
    │
    ├── containsPoint()               // 子类可覆盖
    ├── setBounds()                   // 位置变更+传播
    ├── add() / remove()              // 树管理
    └── invalidate() / revalidate()   // 验证
            │
            ▼
    Shape (extends Figure)
            │
            ├── 字段
            │   ├── lineWidth
            │   ├── lineStyle
            │   ├── lineCap, lineJoin
            │   └── fillColor, strokeColor
            │
            └── 方法
                ├── fillShape()      ← RectangleFigure 等实现
                └── outlineShape()   ← RectangleFigure 等实现
                        │
                        ▼
                ┌─────────────────┬──────────────────┐
                │ RectangleFigure  │ EllipseFigure     │
                │ PolylineFigure   │ PolygonFigure     │
                │ RoundedRect...   │ TriangleFigure    │
                └─────────────────┴──────────────────┘
```

## 性能优化

### 1. 状态缓存

```java
private boolean isValid = false;
public void validate() {
    if (!isValid) {
        // 计算...
        isValid = true;
    }
}
```

### 2. 对象复用

```java
private static final Rectangle PRIVATE_RECT = new Rectangle();
public Rectangle getClientArea(Rectangle rect) {
    rect.setBounds(...);  // 复用对象
    return rect;
}
```

### 3. 懒创建

```java
private List<IFigure> children = Collections.emptyList();
// 首次添加时才创建 ArrayList
```

## 子类扩展方式

### 1. 覆盖 paintFigure()

```java
class MyFigure extends Figure {
    @Override
    protected void paintFigure(Graphics g) {
        g.drawOval(getBounds());
    }
}
```

### 2. 覆盖 containsPoint()

```java
class MyFigure extends Figure {
    @Override
    public boolean containsPoint(int x, int y) {
        // 精确检测
        return geometry.contains(x, y);
    }
}
```

### 3. 扩展标志位

```java
class MyFigure extends Figure {
    private static final int FLAG_CUSTOM = 1 << 6;
    protected static int MAX_FLAG = FLAG_CUSTOM;
}
```

## 与 Novadraw 对比

| 方面 | g2 Figure | Novadraw 实现 |
|------|-----------|--------------|
| 接口 | IFigure 宽接口 | Figure trait |
| 实现 | Figure 基类 | 多个 struct + impl |
| 继承 | extends | Trait |
| 状态 | 位标志 | 布尔字段 |
| 事件 | 观察者模式 | 简化实现 |
| 布局 | LayoutManager | 基础实现 |

## 参考

- 源码位置: `org.eclipse.draw2d.Figure`
- 源码基线: eclipse/gef-classic commit `4463d9d0c`（2026-01-01）
- 主要方法: paint, setBounds, add, remove, containsPoint
