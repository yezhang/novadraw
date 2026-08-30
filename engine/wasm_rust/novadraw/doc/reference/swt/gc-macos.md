# SWT GC macOS 平台实现分析

类型：`reference-analysis`

> 基于 Eclipse SWT 源码分析，版本 2025-01

## 概述

SWT GC (Graphics Context) 是 Eclipse SWT 的核心绘制抽象。分析 macOS (Cocoa) 平台的实现，可为设计 NdCanvas 提供参考。

## 源码位置

```text
eclipse.platform SWT/
└── bundles/org.eclipse.swt/Eclipse SWT/cocoa/
    └── org/eclipse/swt/graphics/
        ├── GC.java      (4239 行)
        └── GCData.java  (平台相关数据)
```

## 核心架构

### GC 初始化流程

```java
GC(Drawable drawable)
  → 创建 GCData 对象
  → drawable.internal_new_GC(data)  // 获取平台句柄
  → init(drawable, data, context)
      → 创建 NSGraphicsContext
      → handle.saveGraphicsState()    // 保存初始状态
      → 创建 NSBezierPath (用于路径绘制)
```

## GCData 核心字段

```java
public final class GCData {
    // 颜色
    public double[] foreground, background;  // ARGB 颜色数组 [r, g, b, a]
    public NSColor fg, bg;                   // Cocoa NSColor 对象

    // 线条样式
    public float lineWidth;
    public int lineStyle;      // LINE_SOLID, LINE_DASH, etc.
    public int lineCap;        // CAP_FLAT, CAP_ROUND, CAP_SQUARE
    public int lineJoin;       // JOIN_MITER, JOIN_ROUND, JOIN_BEVEL
    public float[] lineDashes;
    public float lineMiterLimit;

    // 变换
    public NSAffineTransform transform;       // 正向变换
    public NSAffineTransform inverseTransform; // 逆变换

    // 裁剪
    public NSBezierPath clipPath;
    public NSBezierPath visiblePath;

    // 路径（复用）
    public NSBezierPath path;

    // 脏状态标记
    public int state = -1;
}
```

## 状态管理机制

### 状态位定义

```java
static final int FOREGROUND       = 1 << 0;
static final int BACKGROUND       = 1 << 1;
static final int FONT             = 1 << 2;
static final int LINE_STYLE       = 1 << 3;
static final int LINE_CAP         = 1 << 4;
static final int LINE_JOIN        = 1 << 5;
static final int LINE_WIDTH       = 1 << 6;
static final int LINE_MITERLIMIT = 1 << 7;
static final int FOREGROUND_FILL  = 1 << 8;
static final int DRAW_OFFSET      = 1 << 9;
static final int CLIPPING         = 1 << 10;
static final int TRANSFORM        = 1 << 11;
static final int VISIBLE_REGION   = 1 << 12;

// 绘制操作需要的完整状态
static final int DRAW = CLIPPING | TRANSFORM | FOREGROUND | LINE_WIDTH | ...;
static final int FILL = CLIPPING | TRANSFORM | BACKGROUND;
```

### checkGC() - 延迟状态更新

**核心设计**：使用脏标记延迟更新平台状态：

```java
NSAutoreleasePool checkGC(int mask) {
    // 1. 如果需要裁剪/变换，更新它们
    if ((mask & (CLIPPING | TRANSFORM)) != 0) {
        handle.restoreGraphicsState();  // 恢复 Cocoa 状态
        handle.saveGraphicsState();     // 重新保存（创建新状态）
        // 应用裁剪和变换
        if (data.clipPath != null) data.clipPath.addClip();
        if (data.transform != null) data.transform.concat();
    }

    // 2. 只更新需要的状态位
    int state = data.state;
    if ((state & mask) == mask) return pool;  // 状态已同步，无需更新
    state = (state ^ mask) & mask;
    data.state |= mask;

    // 3. 根据状态位更新平台资源
    if ((state & FOREGROUND) != 0) {
        // 更新前景色
        NSColor fg = NSColor.colorWithDeviceRed(...);
        fg.setStroke();
    }
    // ... 其他状态更新
}
```

**设计要点**：

- 每次绘制前调用 `checkGC(DRAW)` 或 `checkGC(FILL)`
- 只更新标记为"脏"的状态
- 复用 `NSBezierPath` 避免重复创建

## 统一 Path 绘制模式

### 核心发现

**macOS 平台所有图形绘制都统一使用 NSBezierPath**：

| 方法 | 底层实现 |
|------|----------|
| `drawRectangle` | `path.appendBezierPathWithRect()` + `path.stroke()` |
| `fillRectangle` | `path.appendBezierPathWithRect()` + `path.fill()` |
| `drawOval` | `path.appendBezierPathWithOvalInRect()` + `path.stroke()` |
| `fillOval` | `path.appendBezierPathWithOvalInRect()` + `path.fill()` |
| `drawRoundRectangle` | `path.appendBezierPathWithRoundedRect()` + `path.stroke()` |
| `drawLine` | `path.moveToPoint()` + `path.lineToPoint()` + `path.stroke()` |
| `drawPolygon` | `path.moveToPoint()` + `path.lineToPoint()` + `path.closePath()` + `path.stroke()` |
| `drawPolyline` | `path.moveToPoint()` + `path.lineToPoint()` + `path.stroke()` |

### 统一绘制模式

```java
// 典型的绘制方法结构
public void drawXxx(...) {
    NSAutoreleasePool pool = checkGC(DRAW);  // 1. 检查/更新状态
    try {
        NSBezierPath path = data.path;       // 2. 获取复用的 path 对象

        // 3. 构建路径
        path.appendBezierPathWithXxx(...);
        // 或
        path.moveToPoint(...);
        path.lineToPoint(...);

        // 4. 描边或填充
        if (hasPattern) {
            strokePattern(path, pattern);     // 渐变
        } else {
            path.stroke();                    // 或 path.fill()
        }

        // 5. 清理路径（复用！）
        path.removeAllPoints();
    } finally {
        uncheckGC(pool);
    }
}
```

### drawRectangle 示例

```java
public void drawRectangle(int x, int y, int width, int height) {
    NSAutoreleasePool pool = checkGC(DRAW);
    try {
        // 处理负数宽高
        if (width < 0) { x += width; width = -width; }
        if (height < 0) { y += height; height = -height; }

        // 创建 NSRect
        NSRect rect = new NSRect();
        rect.x = x + data.drawXOffset;
        rect.y = y + data.drawYOffset;
        rect.width = width;
        rect.height = height;

        // 使用 Path 绘制
        NSBezierPath path = data.path;
        path.appendBezierPathWithRect(rect);

        // 描边
        if (pattern != null && pattern.gradient != null) {
            strokePattern(path, pattern);
        } else {
            path.stroke();
        }

        // 清理（复用 path）
        path.removeAllPoints();
    } finally {
        uncheckGC(pool);
    }
}
```

## 坐标变换

### setTransform 实现

```java
public void setTransform(Transform transform) {
    if (transform != null) {
        // 创建正向变换
        data.transform = new NSAffineTransform(transform.handle);

        // 创建逆变换（用于坐标转换）
        data.inverseTransform = new NSAffineTransform(transform.handle);
        data.inverseTransform.invert();
    } else {
        data.transform = data.inverseTransform = null;
    }

    // 标记变换状态为脏
    data.state &= ~(TRANSFORM | DRAW_OFFSET);
}
```

变换在 `checkGC()` 中应用：调用 `data.transform.concat()` 左乘变换矩阵。

## 裁剪管理

### setClipping 实现

```java
public void setClipping(int x, int y, int width, int height) {
    NSRect rect = new NSRect(x, y, width, height);
    NSBezierPath path = NSBezierPath.bezierPathWithRect(rect);
    setClipping(path);
}

public void setClipping(Path path) {
    // 释放旧裁剪路径
    if (data.clipPath != null) data.clipPath.release();

    // 设置新裁剪路径
    data.clipPath = path;
    data.clipPath.retain();

    // 标记裁剪状态为脏
    data.state &= ~CLIPPING;
}
```

裁剪在 `checkGC()` 中应用：调用 `data.clipPath.addClip()`。

## 状态栈管理

**重要发现**：macOS GC 没有 `pushState()/popState()` 方法！

而是通过 `NSGraphicsContext` 的状态栈实现：

```java
// GC 初始化时
handle.saveGraphicsState();

// checkGC() 中
handle.restoreGraphicsState();  // 恢复
handle.saveGraphicsState();     // 创建新状态
```

## 关键设计模式总结

| 设计模式 | 应用场景 |
|----------|----------|
| **脏标记延迟更新** | `checkGC()` 只更新变化的状态位 |
| **对象复用** | 复用 `NSBezierPath` 避免 GC |
| **状态位掩码** | 用位运算管理多种状态 |
| **平台抽象** | `GCData` 隔离平台差异 |
| **统一 Path 抽象** | 所有图形都使用 NSBezierPath |

## 对 NdCanvas 设计的启示

### 1. 统一 Path 抽象

```text
NdCanvas.rect(x, y, w, h)     → Path.rect() + fill/stroke
NdCanvas.ellipse(cx, cy, rx, ry) → Path.ellipse() + fill/stroke
NdCanvas.line(p1, p2)         → Path.moveTo() + lineTo() + stroke
NdCanvas.polyline(points)     → Path.moveTo() + lineTo()... + stroke
```

**优势**：

- 统一的底层表示
- 便于实现裁剪、渐变等高级功能
- 易于扩展（贝塞尔曲线、路径组合等）

### 2. 命令队列模式

可以收集绘制命令，最后批量执行：

```rust
struct NdCanvas {
    commands: Vec<RenderCommand>,
}
```

### 3. 状态跟踪

使用状态位跟踪变化的属性：

```rust
struct CanvasState {
    foreground: Color,
    background: Color,
    line_width: f64,
    // 脏标记
    dirty: u32,
}
```

### 4. 延迟初始化

只初始化需要的平台资源，避免不必要的开销。

### 5. 对象池

复用路径、变换等对象减少分配：

```rust
struct PathPool {
    paths: Vec<Path>,
}
```

## 参考

- 源码: `org.eclipse.swt.graphics.GC`
- 源码: `org.eclipse.swt.graphics.GCData`
- Cocoa Path API: `NSBezierPath`
- vello 渲染库: 类似的统一路径抽象
