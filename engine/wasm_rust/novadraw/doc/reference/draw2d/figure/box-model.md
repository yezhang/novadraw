# Eclipse Draw2D Figure 盒模型分析

类型：`reference-analysis`

> 基于 Eclipse GEF Classic 源码分析

## 概述

本文用“盒模型”作类比来解释 Draw2D 的 `bounds / Border / Insets /
clientArea`；源码没有声明该设计源自 CSS，两者也不是逐项对应关系。

## 盒模型层次结构

```text
┌────────────────────────────────────────────────────────────────┐
│                        Figure Bounds                            │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                      Border (装饰边框)                    │ │
│  │  ┌────────────────────────────────────────────────────┐ │ │
│  │  │                                                    │ │ │
│  │  │  ┌────────────────────────────────────────────┐  │ │ │
│  │  │  │           Client Area (客户区域)            │  │ │ │
│  │  │  │                                            │  │ │ │
│  │  │  │     子元素在这里布局和绘制                   │  │ │ │
│  │  │  │                                            │  │ │ │
│  │  │  └────────────────────────────────────────────┘  │ │ │
│  │  │                                                    │ │ │
│  │  └────────────────────────────────────────────────────┘ │ │
│  │                                                            │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                │
└────────────────────────────────────────────────────────────────
```

## 核心概念定义

### 1. Bounds（边界）

Figure 占用的完整矩形区域，包括所有视觉元素。

```java
// Figure.java
protected Rectangle bounds = new Rectangle(0, 0, 0, 0);

public Rectangle getBounds() {
    return bounds;
}
```

**特点**：

- 图形占用的完整区域
- 是默认绘制、命中和裁剪共同使用的占用矩形；自定义 Figure 仍需遵守其祖先传入的
  Graphics clip
- 是图形定位的基础坐标

### 2. Insets（内边距）

Border 声明占用的空间，用于收缩 ClientArea。

```java
// Figure.java
public Insets getInsets() {
    if (getBorder() != null) {
        return getBorder().getInsets(this);
    }
    return NO_INSETS;
}
```

**计算公式**：

```java
// LineBorder.java
public Insets getInsets(IFigure figure) {
    return new Insets(getWidth());  // insets = border width
}
```

### 3. Client Area（客户区域）

子元素可以布局和绘制的区域。

```java
// Figure.java
public Rectangle getClientArea(Rectangle rect) {
    rect.setBounds(getBounds());      // 从 bounds 开始
    rect.shrink(getInsets());        // 向内收缩 insets
    if (useLocalCoordinates()) {
        rect.setLocation(0, 0);      // 本地坐标时置零
    }
    return rect;
}
```

**计算公式**：

```text
clientArea = bounds - insets
```

### 4. Border（装饰边框）

可选的装饰性边框，独立于 Shape 的 outline。

**内置 LineBorder 在 bounds 内侧描边。**

```java
// Border.java
public interface Border {
    Insets getInsets(IFigure figure);  // 返回占用的空间（影响 clientArea）
    void paint(IFigure figure, Graphics g, Insets insets);  // 绘制
}

// Figure.java - paintBorder()
protected void paintBorder(Graphics graphics) {
    if (getBorder() != null) {
        // 注意：传入的是 NO_INSETS (全0)！
        // Border 接口设计：insets 参数允许调用者控制绘制区域
        // 但 Figure 默认传入 NO_INSETS，表示在完整 bounds 内绘制
        getBorder().paint(this, graphics, NO_INSETS);
    }
}

// AbstractBorder.java - getPaintRectangle
protected Rectangle getPaintRectangle(IFigure figure, Insets insets) {
    tempRect.setBounds(figure.getBounds());
    return tempRect.shrink(insets);  // 根据传入的 insets 收缩
}

// LineBorder.java - paint
public void paint(IFigure figure, Graphics graphics, Insets insets) {
    // 1. 获取绘制区域（bounds 向内收缩传入的 insets）
    Rectangle r = getPaintRectangle(figure, insets);  // bounds - insets

    // 2. 奇数宽度修正
    if (getWidth() % 2 == 1) {
        r.width--;
        r.height--;
    }

    // 3. 向内收缩边框宽度的一半（描边居中绘制）
    r.shrink(getWidth() / 2, getWidth() / 2);

    graphics.setLineWidth(getWidth());
    graphics.setLineStyle(getStyle());
    if (getColor() != null) {
        graphics.setForegroundColor(getColor());
    }
    graphics.drawRectangle(r);
}
```

**关键发现**：

- `paintBorder()` 调用时传入的是 `NO_INSETS`（全是0）
- 所以 Border 绘制在 **bounds 内部**
- Border 和 Outline 一样，都是向内收缩绘制的
- Figure 入口传入 `NO_INSETS`；`CompoundBorder` 会把 outer border 的 insets
  累加后传给 inner border，因此该参数也用于组合边框的嵌套定位

### 5. Outline（轮廓描边）

Shape 自身的描边（与 Border 类似，都在 bounds 内部绘制）。

#### 5.1 RectangleFigure Outline 坐标计算

```java
// RectangleFigure.java
protected void outlineShape(Graphics graphics) {
    // lineInset = max(1.0, lineWidth) / 2.0
    // 确保最小为 1.0，即使 lineWidth=0 也有描边效果
    float lineInset = Math.max(1.0f, getLineWidthFloat()) / 2.0f;

    // 向下/向上取整，处理奇数线宽
    int inset1 = (int) Math.floor(lineInset);
    int inset2 = (int) Math.ceil(lineInset);

    Rectangle r = Rectangle.SINGLETON.setBounds(getBounds());

    // x, y 向内收缩 inset1
    r.x += inset1;
    r.y += inset1;

    // 宽高向内收缩 inset1 + inset2
    // 公式：width - inset1 - inset2
    r.width -= inset1 + inset2;
    r.height -= inset1 + inset2;

    graphics.drawRectangle(r);
}
```

**关键设计**：

- `Math.max(1.0f, getLineWidthFloat())` 确保即使 lineWidth=0 也至少有 1px 描边
- 使用 `floor` 和 `ceil` 分别处理奇数线宽，保证描边居中

**计算示例**：

```text
假设 bounds = (0, 0, 100, 50)，lineWidth = 2：
  lineInset = max(1.0, 2.0) / 2.0 = 1.0
  inset1 = floor(1.0) = 1
  inset2 = ceil(1.0) = 1

  r.x = 0 + 1 = 1
  r.y = 0 + 1 = 1
  r.width = 100 - 1 - 1 = 98
  r.height = 50 - 1 - 1 = 48

  绘制的矩形: (1, 1, 98, 48)

假设 lineWidth = 3（奇数）：
  lineInset = max(1.0, 3.0) / 2.0 = 1.5
  inset1 = floor(1.5) = 1
  inset2 = ceil(1.5) = 2

  r.x = 0 + 1 = 1
  r.y = 0 + 1 = 1
  r.width = 100 - 1 - 2 = 97
  r.height = 50 - 1 - 2 = 47

  绘制的矩形: (1, 1, 97, 47)
```

#### 5.2 Ellipse Outline 坐标计算

Ellipse 使用与 RectangleFigure 相同的内缩算法：

```java
// Ellipse.java
private Rectangle getOptimizedBounds() {
    float lineInset = Math.max(1.0f, getLineWidthFloat()) / 2.0f;
    int inset1 = (int) Math.floor(lineInset);
    int inset2 = (int) Math.ceil(lineInset);

    Rectangle r = Rectangle.SINGLETON.setBounds(getBounds());
    r.x += inset1;
    r.y += inset1;
    r.width -= inset1 + inset2;
    r.height -= inset1 + inset2;
    return r;
}

protected void outlineShape(Graphics graphics) {
    graphics.drawOval(getOptimizedBounds());
}
```

#### 5.3 Shape.fillShape 绘制位置

```java
// RectangleFigure.java
protected void fillShape(Graphics graphics) {
    // 填充整个 bounds 范围
    graphics.fillRectangle(getBounds());
}

// Ellipse.java
protected void fillShape(Graphics graphics) {
    graphics.fillOval(getOptimizedBounds());
}
```

**绘制顺序**：先 `fillShape` 填充整个 bounds，再 `outlineShape` 在内部描边，这样 outline 会覆盖 fill 的边缘，形成描边效果。

#### 5.4 Ellipse 精确命中测试

RectangleFigure 的 `containsPoint` 使用简单的 bounds 检测，但 Ellipse 实现了精确的椭圆检测：

```java
// Ellipse.java
public boolean containsPoint(int x, int y) {
    // 1. 首先进行 bounds 快速检测
    if (!super.containsPoint(x, y)) {
        return false;
    }

    // 2. 精确的椭圆检测
    Rectangle r = getBounds();
    // 将点平移到以椭圆中心为原点
    long ux = x - r.x - r.width / 2;
    long uy = y - r.y - r.height / 2;

    // 椭圆方程：(x/rx)^2 + (y/ry)^2 <= 1
    // 使用整数运算避免浮点精度问题
    // 乘以 256 是为了避免除法并允许一定容差
    return ((ux * ux) << 10) / (r.width * r.width)
         + ((uy * uy) << 10) / (r.height * r.height) <= 256;
}
```

**算法原理**：

- 椭圆方程：`x²/a² + y²/b² ≤ 1`（a, b 为半轴长度）
- 将点平移到以椭圆中心为原点
- 使用移位 `<< 10` 代替乘法 1024，避免除法

#### 5.5 Polyline 描边

```java
// PolylineShape.java
protected void outlineShape(Graphics graphics) {
    graphics.pushState();
    graphics.translate(getLocation());  // 移动到折线起点
    graphics.drawPolyline(points);      // 绘制折线
    graphics.popState();
}
```

**注意**：Polyline 的描边只绘制折线本身，不形成封闭区域。

## 绘制顺序

### Figure 基类绘制流程

```text
paint(Graphics)
    │
    ├── 1. 设置本地属性（颜色、字体）
    │       setBackgroundColor()
    │       setForegroundColor()
    │       setFont()
    │
    ├── 2. pushState()  ← 保存当前 Graphics 状态
    │
    ├── 3. paintFigure()
    │       │
    │       ├── isOpaque() ? fillRectangle(bounds)  ← 填充背景
    │       │
    │       └── border instanceof AbstractBackground ? paintBackground()
    │
    ├── 4. restoreState()  ← 恢复状态（清除裁剪区）
    │
    ├── 5. paintClientArea()
    │       │
    │       ├── translate(bounds.x + insets.left, bounds.y + insets.top)
    │       ├── clipRect(clientArea)              ← 裁剪到客户区
    │       │
    │       └── for child : children
    │               child.paint()
    │
    ├── 6. paintBorder()
    │       │
    │       └── border.paint(figure, g, NO_INSETS)  ← 绘制装饰边框
    │
    └── 7. popState()  ← 恢复原始 Graphics 状态
```

### Shape 子类绘制流程

```java
// Shape.java - paintFigure()
public void paintFigure(Graphics graphics) {
    // 1. 设置抗锯齿和透明度
    if (antialias != null) {
        graphics.setAntialias(antialias.intValue());
    }
    if (alpha != null) {
        graphics.setAlpha(alpha.intValue());
    }

    // 2. 禁用状态特殊处理（偏移绘制）
    if (!isEnabled()) {
        graphics.translate(1, 1);
        // 使用较浅的颜色绘制
        if (fill) paintFill(graphics);
        if (outline) paintOutline(graphics);
        graphics.translate(-1, -1);
    }

    // 3. 正常绘制
    if (fill) {
        paintFill(graphics);    // → fillShape()
    }

    if (outline) {
        paintOutline(graphics); // → outlineShape()
    }
}

// Shape 内部方法
private void paintOutline(Graphics graphics) {
    // 同步线宽和样式属性
    lineAttributes.width = getLineWidthFloat();
    lineAttributes.style = getLineStyle();
    graphics.setLineAttributes(lineAttributes);
    outlineShape(graphics);  // 调用子类实现
}

private void paintFill(Graphics graphics) {
    fillShape(graphics);  // 调用子类实现
}
```

**Shape 绘制顺序**：

1. `paintFill()` → `fillShape()` - 由具体 Shape 决定填充区域；
   `RectangleFigure` 使用 bounds，当前 `Ellipse` 使用内缩后的 optimized bounds
2. `paintOutline()` → `outlineShape()` - 在内部描边（覆盖 fill 边缘）

### useLocalCoordinates 行为

当 Figure 使用本地坐标时（默认 `false`）：

```java
protected void paintClientArea(Graphics graphics) {
    if (useLocalCoordinates()) {
        // 子元素使用相对于 bounds 左上角 + insets 的本地坐标
        graphics.translate(
            getBounds().x + getInsets().left,
            getBounds().y + getInsets().top
        );

        // 裁剪到 client area
        if (!optimizeClip()) {
            graphics.clipRect(getClientArea(PRIVATE_RECT));
        }

        paintChildren(graphics);
        graphics.restoreState();
    } else {
        // 默认：子元素使用父图形坐标（无需变换）
        paintChildren(graphics);
    }
}
```

**本地坐标 vs 父坐标**：

- `useLocalCoordinates() = false`（默认）：子元素使用父图形坐标系
- `useLocalCoordinates() = true`：子元素使用相对于 `bounds.left + insets.left` 的本地坐标

## Border vs Outline vs Fill

| 概念 | 来源 | 作用 | 绘制位置 |
|------|------|------|----------|
| **fillShape** | Shape | 填充图形内部 | bounds **整个范围** |
| **outlineShape** | Shape | 描边图形轮廓 | bounds 向内收缩 `max(1, lineWidth)/2` |
| **border** | Figure (setBorder) | 装饰性边框 | bounds 向内收缩 `borderWidth/2` |

**绘制顺序**：Shape 的 fillShape → outlineShape，随后 Figure 在 children 之后绘制
border。outline 与 border 可能重叠，不能把 border 一概描述为“最内层”。

### 关键发现：Border 和 Outline 都在 bounds 内部绘制

```text
┌─────────────────────────────────────────────────────────┐
│                    Figure Bounds                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │                                                 │   │
│  │  ┌─────────────────────────────────────────┐  │   │
│  │  │                                         │  │   │
│  │  │  ┌─────────────────────────────────┐  │  │   │
│  │  │  │                                 │  │  │   │
│  │  │  │  ┌───────────────────────┐   │  │  │   │
│  │  │  │  │                       │   │  │  │   │
│  │  │  │  │      FILL            │   │  │  │   │ ← bounds 整个范围
│  │  │  │  │  ┌─────────────────┐ │   │  │  │   │
│  │  │  │  │  │               │ │   │  │  │   │
│  │  │  │  │  │   OUTLINE     │ │   │  │  │   │ ← 向内收缩
│  │  │  │  │  │               │ │   │  │  │   │
│  │  │  │  │  └─────────────────┘ │   │  │  │   │
│  │  │  │  │                       │   │  │  │   │
│  │  │  │  └───────────────────────┘   │  │  │   │
│  │  │  │                                 │  │  │   │
│  │  │  └─────────────────────────────────┘  │  │   │
│  │  │                                         │  │   │
│  │  └─────────────────────────────────────────┘  │   │
│  │                                             │   │
│  │              BORDER (装饰)                    │ ← 向内收缩
│  │                                             │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Border vs Outline 区别

| | Border | Outline |
|---|--------|---------|
| 来源 | `setBorder(Border)` | Shape 属性 (fill/outline) |
| 绘制位置 | bounds 内，向内收缩 | bounds 内，向内收缩 |
| insets 作用 | **不决定** Border 绘制位置 | 不适用 |
| 影响 clientArea | 是（收缩子元素布局区域） | 否 |

### 重要：Insets 的真正作用

insets 用于收缩 **ClientArea**（子元素布局区域），但 **不决定** Border 的绘制位置：

```java
// Border 绘制：使用传入的 insets 参数（通常为 NO_INSETS）
getBorder().paint(this, graphics, NO_INSETS);

// ClientArea 计算：使用 figure 的 insets
public Rectangle getClientArea() {
    rect.shrink(getInsets());  // insets 收缩客户区
}
```

## 实际案例：带边框的矩形

```java
RectangleFigure rect = new RectangleFigure();
rect.setBounds(new Rectangle(10, 10, 100, 50));
rect.setBorder(new LineBorder(Color.BLACK, 2));

// 几何数据：
// bounds      = (10, 10, 100, 50)
// border insets = 2 (border width)
// clientArea  = bounds - insets = (12, 12, 96, 46)
```

### 绘制结果

```text
┌──────────────────────────────┐
│ ■■■■■■■■■■■■ │  ← Border (2px，在 bounds 内)
│ ■                          ■ │
│ ■  ┌────────────────────┐  ■ │  ← fillShape + outlineShape
│ ■  │                    │  ■ │
│ ■  │   Client Area     │  ■ │     (子元素布局区域)
│ ■  │                    │  ■ │
│ ■  └────────────────────┘  ■ │
│ ■                          ■ │
│ ■■■■■■■■■■■■■■■■■■■■ │  ← Border
└──────────────────────────────┘
```

## 与 CSS 盒模型对比

| CSS 盒模型 | g2 Figure | 说明 |
|------------|-----------|------|
| content | clientArea | 内容区域 |
| padding | - | g2 无对应 |
| border | Border + outline | 边框区域（在 bounds 内） |
| margin | - | g2 无对应 |
| box-sizing | - | g2 固定为 border-box 模式 |

**关键区别**：

- g2 使用 border-box 模式（bounds 包含所有）
- g2 没有 margin（外边距由父容器管理）
- Border 和 Outline 都在 bounds 内部绘制
- insets 只用于收缩 ClientArea，不决定 Border 绘制位置

## 关键设计意图

### 1. Border 与 Shape 分离

Border 是**装饰性**的，独立于图形几何：

- 一个 Border 可以用于任何 Figure
- Border 改变不影响图形几何（只影响 clientArea）

### 2. Outline 是 Shape 属性

Outline（描边）是**图形几何**的一部分：

- 与 fill（填充）成对出现
- 在 bounds 内部绘制

### 3. Border 和 Outline 都在 bounds 内部绘制

两者都向内收缩，不占用额外空间：

- 内置 `LineBorder` 的描边中心线按线宽的一半向内收缩，并对奇数线宽修正
- Outline 向内收缩 line-width/2

### 4. Insets 的真正作用

insets 的作用是**收缩 ClientArea**（子元素布局区域），而不是决定 Border 绘制位置。

```text
// Border 绘制位置 = bounds 内部（由传入的 insets 决定，通常为 NO_INSETS）
// ClientArea 收缩 = 使用 getInsets()
```

## 图示总结

```text
┌─────────────────────────────────────────────────────────────────┐
│                         BOUNDS                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                        INSETS                             │  │
│  │     (收缩 ClientArea，不决定 Border 绘制位置)              │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                                                     │  │  │
│  │  │  ┌───────────────────────────────────────────────┐  │  │ │
│  │  │  │                                               │  │  │ │
│  │  │  │              CLIENT AREA                        │  │  │ │
│  │  │  │         (子元素布局区域)                        │  │  │ │
│  │  │  │         = bounds - insets                       │  │  │ │
│  │  │  │                                               │  │  │ │
│  │  │  │  ┌─────────────────────────────────────────┐  │  │  │ │
│  │  │  │  │                                         │  │  │  │ │
│  │  │  │  │              FILL                        │  │  │  │ │
│  │  │  │  │  ┌───────────────────────────────────┐  │  │  │  │ │
│  │  │  │  │  │                                   │  │  │  │  │ │
│  │  │  │  │  │           OUTLINE                 │  │  │  │  │ │
│  │  │  │  │  │                                   │  │  │  │  │ │
│  │  │  │  │  └───────────────────────────────────┘  │  │  │  │ │
│  │  │  │  │                                         │  │  │  │ │
│  │  │  │  └─────────────────────────────────────────┘  │  │  │ │
│  │  │  │                                                 │  │  │ │
│  │  │  └───────────────────────────────────────────────┘  │  │ │
│  │  │                                                     │  │ │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                                                           │  │
│  │              BORDER (在 bounds 内绘制)                    │  │
│  │              = bounds - NO_INSETS - borderWidth/2        │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## 参考

- 源码: `org.eclipse.draw2d.Figure`
- 源码: `org.eclipse.draw2d.Shape`
- 源码: `org.eclipse.draw2d.Border`
- 源码: `org.eclipse.draw2d.LineBorder`

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
