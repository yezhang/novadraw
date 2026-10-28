# Eclipse Draw2D Clip 原理分析

类型：`reference-analysis`

本文档分析 Eclipse Draw2D 中裁剪（Clip）机制的设计原理和实现细节。

## 1. 概述

Clip 机制用于限制 Figure 的绘制区域，确保子节点只在其父节点的指定区域内渲染。

## 2. 架构层次

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Clip 架构层次                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐       │
│  │  Figure         │     │  Graphics       │     │  SWT GC         │       │
│  │  paintChildren  │────►│  clipRect()     │────►│  setClipping()  │       │
│  │  IClippingStrategy│   │  LazyState      │     │  OS/GDI         │       │
│  └─────────────────┘     └─────────────────┘     └─────────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 3. 核心组件

### 3.1 Figure 层

**Clip 设置入口**：`Figure.java:87, 1768-1770`

```java
// 成员变量
private IClippingStrategy clippingStrategy = null;

// 设置裁剪策略
public void setClippingStrategy(IClippingStrategy clippingStrategy) {
    this.clippingStrategy = clippingStrategy;
}

// 获取裁剪策略
public IClippingStrategy getClippingStrategy() {
    return clippingStrategy;
}
```

**Clip 应用位置**：`Figure.java:1296-1317`

```java
protected void paintChildren(Graphics graphics) {
    for (IFigure child : children) {
        if (child.isVisible()) {
            Rectangle[] clipping = null;
            if (clippingStrategy != null) {
                // 使用自定义裁剪策略
                clipping = clippingStrategy.getClip(child);
            } else {
                // 默认使用 child 的 bounds 作为裁剪区域
                clipping = new Rectangle[] { child.getBounds() };
            }
            // 为每个裁剪区域应用裁剪并绘制
            for (Rectangle element : clipping) {
                if (element.intersects(graphics.getClip(Rectangle.SINGLETON))) {
                    graphics.clipRect(element);  // 应用裁剪
                    child.paint(graphics);       // 绘制子节点
                    graphics.restoreState();     // 恢复状态
                }
            }
        }
    }
}
```

### 3.2 Graphics 抽象层

**Clip 接口定义**：`Graphics.java:76`

```java
public abstract void clipRect(Rectangle r);
```

### 3.3 SWTGraphics 实现层

**状态管理**：`SWTGraphics.java:83-90`

```java
// 延迟状态类
static class LazyState {
    // ...
    Clipping relativeClip;  // 当前裁剪区域
}

interface Clipping {
    Rectangle getBoundingBox(Rectangle rect);
    Clipping getCopy();
    void intersect(int left, int top, int right, int bottom);
    void scale(float horizontal, float vertical);
    void setOn(GC gc, int translateX, int translateY);
    void translate(float dx, float dy);
}
```

**Clip 核心实现**：`SWTGraphics.java:373-382`

```java
@Override
public void clipRect(Rectangle rect) {
    if (currentState.relativeClip == null) {
        throw new IllegalStateException("The current clipping area does not " +
                "support intersection.");
    }

    checkSharedClipping();  // 复制 clip 避免共享修改
    currentState.relativeClip.intersect(rect.x, rect.y, rect.right(), rect.bottom());
    appliedState.relativeClip = null;  // 标记需要重新应用
}
```

**底层执行**：`SWTGraphics.java:156-161`（RectangleClipping）

```java
@Override
public void setOn(GC gc, int translateX, int translateY) {
    int xInt = (int) Math.floor(left);
    int yInt = (int) Math.floor(top);
    gc.setClipping(
        xInt + translateX,
        yInt + translateY,
        (int) Math.ceil(right) - xInt,
        (int) Math.ceil(bottom) - yInt
    );
}
```

## 4. 延迟同步机制（checkGC）

### 4.1 设计背景

每次绘图操作都直接调用 `gc.setClipping()` 会导致：
- 频繁的 JNI 调用（Java → native SWT）
- 大量系统调用开销

### 4.2 解决方案：状态缓存 + 按需同步

```java
// 状态分为两部分
class GraphicsState {
    GraphicsState appliedState;  // 已应用到 GC 的状态
    GraphicsState currentState;  // 当前内存中的状态
}
```

**设计原则**：
1. 状态修改在内存中完成（`currentState`），**不立即同步到 GC**
2. 在真正需要绘图前，**按需同步**到 GC

### 4.3 checkGC 实现

```java
// SWTGraphics.java:294-304
protected final void checkGC() {
    // 裁剪区域同步
    if (appliedState.relativeClip != currentState.relativeClip) {
        appliedState.relativeClip = currentState.relativeClip;
        currentState.relativeClip.setOn(gc, translateX, translateY);
    }

    // 渲染属性同步
    if (appliedState.graphicHints != currentState.graphicHints) {
        reconcileHints(gc, appliedState.graphicHints, currentState.graphicHints);
        appliedState.graphicHints = currentState.graphicHints;
    }
}
```

### 4.4 同步触发流程

```
修改操作（clipRect, setForeground...）
        ↓
    更新 currentState（内存操作，快）
        ↓
绘图操作（drawLine, fillOval...）
        ↓
    checkGC() 检查并同步到 GC（只同步变化的属性）
        ↓
    gc.setClipping(...)  // 最终系统调用
```

### 4.5 性能对比

| 方案 | 每帧 JNI 调用次数 |
|------|-------------------|
| 每次修改直接同步 GC | O(n)，n = 属性修改次数 |
| checkGC 延迟同步 | 只同步绘制前实际变化的状态；最坏仍可随“修改后立即绘制”的次数增长 |

## 5. 默认行为

当 Figure 没有设置 `IClippingStrategy` 时：

```java
// 默认使用 child 的 bounds 作为裁剪区域
clipping = new Rectangle[] { child.getBounds() };
```

这意味着子节点默认只能在自身的 bounds 区域内绘制。

## 6. 自定义裁剪策略

用户可以实现 `IClippingStrategy` 接口自定义裁剪行为：

```java
public interface IClippingStrategy {
    Rectangle[] getClip(IFigure child);
}
```

应用场景：
- 为一个 child 返回多个矩形可见区
- 调整或扩大默认 child-bounds 矩形裁剪
- 预计算矩形裁剪区域

接口返回 `Rectangle[]`，本身不提供任意路径或圆形 clip。

## 7. 完整调用链

```
用户调用: figure.setClippingStrategy(strategy)
        ↓
渲染阶段: LightweightSystem.paint()
        ↓
UpdateManager.paint(GC) / performUpdate(...)
        ↓
root.paint(graphics)
        ↓
Figure.paintChildren(graphics)
        ↓
graphics.clipRect(rect)  // 设置裁剪区域
        ↓
SWTGraphics.clipRect()   // 更新内存状态
        ↓
绘图操作（如 drawLine）
        ↓
checkGC() → setOn(gc)    // 按需同步到 GC
        ↓
gc.setClipping()         // SWT 底层调用
        ↓
操作系统/GDI 执行实际裁剪
```

## 8. 设计模式

Clip 机制使用了以下设计模式：

| 模式 | 应用场景 |
|------|----------|
| Strategy 模式 | `IClippingStrategy` 支持自定义裁剪策略 |
| State 模式 | `LazyState` 管理 Graphics 状态 |
| 值对象/策略实现 | `RectangleClipping` 实现内部 `Clipping` 协议 |
| Lazy Evaluation | `checkGC` 延迟同步优化性能 |

## 9. 参考源码

| 文件 | 主要内容 |
|------|----------|
| `Figure.java:1296-1317` | paintChildren 中应用 clip |
| `Figure.java:1768-1770` | setClippingStrategy 方法 |
| `SWTGraphics.java:294-304` | checkGC 延迟同步 |
| `SWTGraphics.java:373-382` | clipRect 实现 |
| `SWTGraphics.java:156-161` | RectangleClipping.setOn |
| `Graphics.java:76` | clipRect 抽象接口 |

---

*源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。*
