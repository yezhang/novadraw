# Draw2d Graphics API 参考手册

类型：`reference-analysis`

## 概述

Draw2d 的 `Graphics` 抽象类定义了一套跨平台的 2D 绘图 API，实际实现由 `SWTGraphics` 支撑 SWT 的 `GC` 类。

---

## 1. 状态管理 API

### 1.0 Graphics 抽象架构

```text
┌─────────────────────────────────────────────────────────────────┐
│                     draw2d Graphics (抽象类)                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────┐    │
│  │  状态属性   │  │  变换属性   │  │     裁剪属性         │    │
│  ├─────────────┤  ├─────────────┤  ├──────────────────────┤    │
│  │ foreground  │  │ translate   │  │  currentClip         │    │
│  │ background  │  │ scale       │  │  clipRect            │    │
│  │ font        │  │ rotate      │  │  setClip             │    │
│  │ lineWidth   │  │ shear       │  │                      │    │
│  │ lineStyle   │  │             │  │                      │    │
│  └─────────────┘  └─────────────┘  └──────────────────────┘    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    状态栈 (栈顶 = 当前状态)                │    │
│  │  ┌─────────────────────────────────────────────────┐  │    │
│  │  │  Stack[0]: appliedState (最底层)                 │  │    │
│  │  │  Stack[1]: currentState                         │  │    │
│  │  │  Stack[2]: ...                                  │  │    │
│  │  │  Stack[n]: 栈顶 (由 pushState 创建)            │  │    │
│  │  └─────────────────────────────────────────────────┘  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    绘制 API                              │    │
│  │  drawLine / drawRectangle / drawOval / drawPolygon      │    │
│  │  fillRectangle / fillOval / fillPolygon                  │    │
│  │  drawImage / drawText / drawPath                        │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    SWTGraphics (实现类)                   │    │
│  │  将 draw2d Graphics 调用转换为 SWT GC                    │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.1 状态栈操作

```java
// 保存当前状态到栈
abstract void pushState();

// 恢复之前的状态
abstract void popState();

// 恢复到之前推送的状态
abstract void restoreState();
```

**使用模式**:
```java
graphics.pushState();
try {
    // 修改状态
    graphics.translate(x, y);
    graphics.setBackgroundColor(c);
    // 绘制...
} finally {
    graphics.popState();
}
```

---

## 2. 坐标变换 API

### 2.1 平移变换

```java
// 整数平移
abstract void translate(int dx, int dy);

// 浮点数平移
void translate(float dx, float dy);

// 点平移
final void translate(Point pt) { translate(pt.x, pt.y); }
```

### 2.2 缩放变换

```java
// 统一缩放
abstract void scale(double amount);

// 分别缩放水平和垂直
void scale(float horizontal, float vertical);
```

### 2.3 旋转变换

```java
// 逆时针旋转角度
void rotate(float degrees);
```

### 2.4 剪切变换

```java
// 水平/垂直剪切
void shear(float horz, float vert);
```

---

## 3. 裁剪区域 API

### 3.1 矩形裁剪

```java
// 设置裁剪矩形，超出部分不绘制
abstract void clipRect(Rectangle r);

// 获取当前裁剪区域
abstract Rectangle getClip(Rectangle rect);
```

### 3.2 路径裁剪

```java
// 设置裁剪路径
void setClip(Path path);

// 路径裁剪
void clipPath(Path path);
```

---

## 4. 颜色与样式 API

### 4.1 颜色设置

```java
// 设置背景色（用于填充）
abstract void setBackgroundColor(Color rgb);

// 设置前景色（用于线条和文字）
abstract void setForegroundColor(Color rgb);

// 获取背景色
abstract Color getBackgroundColor();

// 获取前景色
abstract Color getForegroundColor();
```

### 4.2 渐变填充

```java
// 线性渐变填充
abstract void fillGradient(int x, int y, int w, int h, boolean vertical);
```

### 4.3 图案填充

```java
// 背景填充图案
void setBackgroundPattern(Pattern pattern);

// 前景绘制图案
void setForegroundPattern(Pattern pattern);
```

---

## 5. 线条样式 API

### 5.1 线宽

```java
abstract void setLineWidth(int width);
abstract void setLineWidthFloat(float width);
abstract int getLineWidth();
abstract float getLineWidthFloat();
```

### 5.2 线型

```java
// SOLID, DASH, DOT, DASHDOT, DASHDOTDOT, CUSTOM
abstract void setLineStyle(int style);
abstract int getLineStyle();
```

### 5.3 线帽样式

```java
// CAP_FLAT, CAP_ROUND, CAP_SQUARE
void setLineCap(int cap);
int getLineCap();
```

### 5.4 线连接样式

```java
// JOIN_MITER, JOIN_ROUND, JOIN_BEVEL
void setLineJoin(int join);
int getLineJoin();
```

### 5.5 虚线模式

```java
// 整数数组表示虚线/实心长度
void setLineDash(int dash[]);

// 浮点数数组
void setLineDash(float[] value);

// 虚线偏移
void setLineDashOffset(float value);

// 斜接限制
abstract void setLineMiterLimit(float miterLimit);
```

### 5.6 批量设置线条属性

```java
void setLineAttributes(LineAttributes attributes);
LineAttributes getLineAttributes();
```

---

## 6. 绘制形状 API

### 6.1 矩形

```java
// 绘制矩形边框
abstract void drawRectangle(int x, int y, int w, int h);
final void drawRectangle(Rectangle r);

// 填充矩形
abstract void fillRectangle(int x, int y, int w, int h);
final void fillRectangle(Rectangle r);
```

### 6.2 圆角矩形

```java
// 带圆角的矩形边框
abstract void drawRoundRectangle(Rectangle r, int arcWidth, int arcHeight);

// 填充圆角矩形
abstract void fillRoundRectangle(Rectangle r, int arcWidth, int arcHeight);
```

### 6.3 椭圆

```java
abstract void drawOval(int x, int y, int w, int h);
final void drawOval(Rectangle r);
abstract void fillOval(int x, int y, int w, int h);
final void fillOval(Rectangle r);
```

### 6.4 弧形

```java
// offset: 起始角度（度），length: 弧长（度）
abstract void drawArc(int x, int y, int w, int h, int offset, int length);
final void drawArc(Rectangle r, int offset, int length);
abstract void fillArc(int x, int y, int w, int h, int offset, int length);
final void fillArc(Rectangle r, int offset, int length);
```

### 6.5 多边形

```java
// 绘制多边形（首尾相连）
abstract void drawPolygon(PointList points);
void drawPolygon(int[] points);

// 填充多边形
abstract void fillPolygon(PointList points);
void fillPolygon(int[] points);
```

### 6.6 折线

```java
// 绘制折线（首尾不相连）
abstract void drawPolyline(PointList points);
void drawPolyline(int[] points);
```

### 6.7 线条

```java
abstract void drawLine(int x1, int y1, int x2, int y2);
final void drawLine(Point p1, Point p2);

// 单个像素点
void drawPoint(int x, int y);
```

### 6.8 路径

```java
void drawPath(Path path);
void fillPath(Path path);
```

### 6.9 焦点框

```java
abstract void drawFocus(int x, int y, int w, int h);
final void drawFocus(Rectangle r);
```

---

## 7. 图像 API

### 7.1 绘制图像

```java
// 在指定位置绘制完整图像
abstract void drawImage(Image srcImage, int x, int y);
final void drawImage(Image image, Point p);

// 缩放绘制图像区域
abstract void drawImage(Image srcImage, int x1, int y1, int w1, int h1,
                        int x2, int y2, int w2, int h2);
final void drawImage(Image srcImage, Rectangle src, Rectangle dest);
```

---

## 8. 文字 API

### 8.1 简单文字

```java
// 无格式文字（背景透明）
abstract void drawString(String s, int x, int y);
final void drawString(String s, Point p);

// 有格式文字（背景填充）
abstract void fillString(String s, int x, int y);
final void fillString(String s, Point p);
```

### 8.2 格式文字

```java
// 支持制表符和换行处理
abstract void drawText(String s, int x, int y);
final void drawText(String s, Point p);

// 带样式标志
void drawText(String s, int x, int y, int style);
final void drawText(String s, Point p, int style);
```

### 8.3 高级文字布局

```java
// 完整文字布局控制
void drawTextLayout(TextLayout layout, int x, int y);
void drawTextLayout(TextLayout layout, int x, int y,
                    int selectionStart, int selectionEnd,
                    Color selectionForeground, Color selectionBackground);
```

### 8.4 字体管理

```java
abstract void setFont(Font f);
abstract Font getFont();
abstract FontMetrics getFontMetrics();
```

---

## 9. 渲染质量 API

### 9.1 抗锯齿

```java
// 非文字抗锯齿
void setAntialias(int value);  // SWT.DEFAULT, SWT.OFF, SWT.ON
int getAntialias();

// 文字抗锯齿
void setTextAntialias(int value);
int getTextAntialias();
```

### 9.2 透明度

```java
// 0-255，0 完全透明
void setAlpha(int alpha);
int getAlpha();
```

### 9.3 插值（图像缩放）

```java
void setInterpolation(int interpolation);  // NONE, LOW, HIGH
int getInterpolation();
```

### 9.4 填充规则

```java
// 多边形填充规则
void setFillRule(int rule);  // FILL_EVEN_ODD, FILL_WINDING
int getFillRule();
```

### 9.5 高级图形模式

```java
void setAdvanced(boolean advanced);
boolean getAdvanced();
```

---

## 10. 混合模式 API

### 10.1 XOR 模式

```java
abstract void setXORMode(boolean b);
abstract boolean getXORMode();
```

> **注意**: XOR 模式已被标记为 deprecated，且在 macOS 上不支持。

---

## 11. 字体与测量 API

```java
abstract Font getFont();
abstract FontMetrics getFontMetrics();
```

**FontMetrics 提供**:
- `getAscent()` / `getDescent()`
- `getHeight()`
- `getAverageCharWidth()`

---

## 12. 使用示例

### 12.1 基本绘制流程

```java
public void paintFigure(Graphics graphics) {
    // 1. 设置样式
    graphics.setForegroundColor(ColorConstants.blue);
    graphics.setBackgroundColor(ColorConstants.lightBlue);
    graphics.setLineWidth(2);

    // 2. 绘制形状
    graphics.drawRectangle(getBounds());
    graphics.fillRectangle(getBounds());

    // 3. 绘制文字
    graphics.drawText("Hello", 10, 10);
}
```

### 12.2 状态保存模式

```java
public void paintFigure(Graphics graphics) {
    graphics.pushState();
    try {
        graphics.translate(10, 10);
        graphics.setBackgroundColor(ColorConstants.red);
        // 绘制...
    } finally {
        graphics.popState();
    }
}
```

### 12.3 裁剪使用

```java
graphics.clipRect(getClientArea());
// 只在这个区域内绘制
```

---

## 13. API 分类速查

| 分类 | 关键方法 |
|------|---------|
| 状态管理 | `pushState`, `popState`, `restoreState` |
| 坐标变换 | `translate`, `scale`, `rotate`, `shear` |
| 裁剪 | `clipRect`, `setClip`, `clipPath` |
| 颜色 | `setForegroundColor`, `setBackgroundColor` |
| 线条 | `setLineWidth`, `setLineStyle`, `setLineDash` |
| 形状 | `drawRectangle`, `drawOval`, `drawPolygon`, `drawLine` |
| 填充 | `fillRectangle`, `fillOval`, `fillPolygon` |
| 图像 | `drawImage` |
| 文字 | `drawString`, `drawText`, `drawTextLayout` |
| 质量 | `setAntialias`, `setAlpha`, `setAdvanced` |

---

## 14. 与 SWT GC 的对应关系

| draw2d Graphics | SWT GC |
|----------------|--------|
| `setForegroundColor` | `GC.setForeground` |
| `setBackgroundColor` | `GC.setBackground` |
| `setFont` | `GC.setFont` |
| `drawLine` | `GC.drawLine` |
| `drawRectangle` | `GC.drawRectangle` |
| `fillRectangle` | `GC.fillRectangle` |
| `drawText` | `GC.drawText` |
| `translate` | `SWTGraphics` 维护逻辑平移，并在绘制/clip 同步时折算到 GC 坐标或 Transform |
| `clipRect` | `GC.setClipping` |
| `setAlpha` | `GC.setAlpha` |

`IClippingStrategy` 属于 `Figure.paintChildren()` 的 child clip 策略，不是
`Graphics` 状态属性。

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
