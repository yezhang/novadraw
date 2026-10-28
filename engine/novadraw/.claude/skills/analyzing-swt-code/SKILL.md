---
name: analyzing-swt-code
description: 分析 SWT (Standard Widget Toolkit) 源码，重点关注 GC 底层绘制 API 和平台交互机制。为设计 NdCanvas 隔离层提供参考。
---

# SWT 框架源码分析

## 概述

SWT (Standard Widget Toolkit) 是 Eclipse 的原生 UI 框架，其 GC (Graphics Context) 类是核心绘制抽象。分析 SWT 源码可为设计 NdCanvas 隔离层提供参考。

## 源码位置

| 文件 | 说明 |
|------|------|
| `eclipse.platform.swt/bundles/org.eclipse.swt/Eclipse SWT/win32/org/eclipse/swt/graphics/GC.java` | Win32 实现 |
| `eclipse.platform.swt/bundles/org.eclipse.swt/Eclipse SWT/gtk/org/eclipse/swt/graphics/GC.java` | GTK 实现 |
| `eclipse.platform.swt/bundles/org.eclipse.swt/Eclipse SWT/cocoa/org/eclipse/swt/graphics/GC.java` | Cocoa 实现 |
| `eclipse.platform.swt/bundles/org.eclipse.swt/Eclipse SWT/win32/org/eclipse/swt/graphics/GCData.java` | 平台相关数据 |

## GC 核心设计

### 职责

- 在 Image、Control 或 Display 上绘制
- 管理绘制状态（颜色、字体、线条样式等）
- 封装平台特定的设备上下文 (Device Context)

### 构造函数

```java
public GC(Drawable drawable)  // 默认样式
public GC(Drawable drawable, int style)  // 指定样式 (LEFT_TO_RIGHT, RIGHT_TO_LEFT)
```

Drawable 可为：Image、Control、Printer

### 资源管理

必须显式调用 `dispose()` 释放资源：

```java
GC gc = new GC(drawable);
try {
    // 绘制操作
} finally {
    gc.dispose();
}
```

## 绘制 API 分类

### 线条绘制 (draw*)

| 方法 | 说明 |
|------|------|
| `drawArc()` | 绘制圆弧 |
| `drawFocus()` | 绘制焦点矩形 |
| `drawLine()` | 绘制直线 |
| `drawOval()` | 绘制椭圆轮廓 |
| `drawPath()` | 绘制路径轮廓 |
| `drawPoint()` | 绘制点 |
| `drawPolygon()` | 绘制多边形轮廓 |
| `drawPolyline()` | 绘制折线 |
| `drawRectangle()` | 绘制矩形轮廓 |
| `drawRoundRectangle()` | 绘制圆角矩形轮廓 |
| `drawString()` / `drawText()` | 绘制文本 |

### 填充绘制 (fill*)

| 方法 | 说明 |
|------|------|
| `fillArc()` | 填充圆弧 |
| `fillGradientRectangle()` | 填充渐变矩形 |
| `fillOval()` | 填充椭圆 |
| `fillPath()` | 填充路径 |
| `fillPolygon()` | 填充多边形 |
| `fillRectangle()` | 填充矩形 |
| `fillRoundRectangle()` | 填充圆角矩形 |

### 图像绘制

```java
// 简单绘制
drawImage(Image image, int x, int y)

// 缩放绘制
drawImage(Image image, int destX, int destY, int destWidth, int destHeight)

// 裁剪绘制
drawImage(Image image, int srcX, int srcY, int srcWidth, int srcHeight,
          int destX, int destY, int destWidth, int destHeight)
```

## 绘制状态管理 (GCData)

GCData 存储所有绘制状态：

### 颜色相关

```java
public int foreground;      // 前景色 (ARGB)
public int background;      // 背景色 (ARGB)
public Pattern foregroundPattern;  // 前景渐变
public Pattern backgroundPattern;  // 背景渐变
public int alpha;           // 全局透明度 (0-255)
```

### 线条相关

```java
public int lineStyle;       // LINE_SOLID, LINE_DASH, LINE_DOT, LINE_DASHDOT, LINE_DASHDOTDOT
public float lineWidth;     // 线条宽度
public int lineCap;         // CAP_FLAT, CAP_ROUND, CAP_SQUARE
public int lineJoin;        // JOIN_MITER, JOIN_ROUND, JOIN_BEVEL
public float[] lineDashes;  // 自定义虚线模式
public float lineMiterLimit;// 斜接限制
```

### 字体

```java
public Font font;
```

### 平台句柄

```java
public long hPen;           // GDI 画笔
public long hBrush;         // GDI 画刷
public long gdipGraphics;   // GDI+ Graphics 对象
public long gdipPen;        // GDI+ 画笔
public long gdipBrush;      // GDI+ 画刷
public long hwnd;           // 窗口句柄
```

### 状态追踪

```java
public int state = -1;      // 脏状态标记，用于延迟更新
```

状态位定义：

```java
static final int FOREGROUND = 1 << 0;
static final int BACKGROUND = 1 << 1;
static final int FONT = 1 << 2;
static final int LINE_STYLE = 1 << 3;
static final int LINE_WIDTH = 1 << 4;
static final int LINE_CAP = 1 << 5;
static final int LINE_JOIN = 1 << 6;
static final int LINE_MITERLIMIT = 1 << 7;
static final int FOREGROUND_TEXT = 1 << 8;
static final int BACKGROUND_TEXT = 1 << 9;
static final int BRUSH = 1 << 10;
static final int PEN = 1 << 11;
```

## 变换管理

### 坐标系统

- 原点 (0,0) 在绘制区域左上角
- X 向右递增，Y 向下递增

### 变换方法

```java
// 获取/设置变换
public Matrix getTransform()
public void setTransform(Matrix matrix)

// 平移
public void translate(float dx, float dy)

// 旋转 (角度)
public void rotate(float degrees)

// 缩放
public void scale(float scaleX, float scaleY)
```

## 裁剪管理

```java
// 裁剪区域
public Rectangle getClipping()
public void setClipping(Rectangle rect)
public void setClipping(Path path)

// 排除裁剪
public void addClipping(Rectangle rect)
```

## 高级功能

### 图案填充

```java
// 设置渐变前景
public void setForegroundPattern(Pattern pattern)

// 设置渐变背景
public void setBackgroundPattern(Pattern pattern)
```

### 路径操作

```java
// 绘制路径轮廓
public void drawPath(Path path)

// 填充路径
public void fillPath(Path path)

// 获取当前剪裁路径
public Path getClipping(Path path)
```

## 平台适配模式

SWT 通过 GCData 隔离平台差异：

1. **通用层**：GC.java 定义统一接口
2. **平台层**：GCData 包含平台特定句柄
3. **Drawable 接口**：各平台实现 `internal_new_GC()` 创建 GC

```java
// Drawable 接口
public interface Drawable {
    long internal_new_GC(GCData data);
    void internal_dispose_GC(long handle, GCData data);
}
```

## 对比分析：SWT GC vs NdCanvas

| 特性 | SWT GC | NdCanvas (目标) |
|------|--------|-----------------|
| 绘制目标 | Image/Control/Printer | 待定 |
| 状态管理 | GCData (可变) | 分离 Figure + FigureBlock |
| 变换 | Matrix 对象 | Transform |
| 裁剪 | Path/Rectangle | Clip |
| 资源释放 | 显式 dispose() | 自动引用计数 |
| 平台抽象 | GCData + JNI | 直接使用 WebGPU |

## 分析要点

分析 SWT 源码时关注：

1. **状态变更检测**：GC 如何追踪脏状态
2. **批量绘制**：如何减少平台调用
3. **延迟初始化**：GCData 字段何时初始化
4. **线程安全**：Drawable 线程亲和性
5. **坐标变换**：矩阵堆栈管理方式
