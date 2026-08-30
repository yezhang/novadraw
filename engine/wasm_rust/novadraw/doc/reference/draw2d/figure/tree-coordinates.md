# Draw2d Figure 树父子节点位置关系处理

类型：`reference-analysis`

## 概述

本文档整理 Eclipse Draw2d 图形引擎在渲染过程中对 Figure 树父子节点位置关系的关键处理逻辑。

---

## 1. 核心概念

### 1.1 坐标系层级

Draw2d 通过 Figure 父链形成多个可嵌套的坐标域，常用术语包括：

- **局部坐标 (Local/Relative)**：相对于某个 Figure client area 的坐标
- **父坐标 (Parent)**：当前 Figure 所属的直接父坐标域
- **绝对坐标 (Absolute)**：沿 parent 链应用全部 `translateToParent()` 后的根域坐标

`useLocalCoordinates()` 决定当前 Figure 是否为 children 建立局部坐标域；它不改变
当前 Figure 自己的 bounds 所属域。

### 1.2 关键方法

| 方法                                | 位置             | 说明                                |
| ----------------------------------- | ---------------- | ----------------------------------- |
| `translateToParent(Translatable)`   | IFigure:1073     | 将坐标从当前 Figure 转换到父 Figure |
| `translateFromParent(Translatable)` | IFigure:1057     | 将坐标从父 Figure 转换到当前 Figure |
| `translateToAbsolute(Translatable)` | IFigure:1065     | 将坐标转换到绝对坐标系              |
| `translateToRelative(Translatable)` | IFigure:1081     | 将绝对坐标转换到相对坐标            |
| `useLocalCoordinates()`             | Figure.java:2149 | 决定是否使用局部坐标系              |

---

## 2. 坐标转换机制

### 2.1 基础 Figure 的坐标转换

```java
// Figure.java:2068-2072
public void translateToParent(Translatable t) {
    if (useLocalCoordinates()) {
        t.performTranslate(getBounds().x + getInsets().left,
                          getBounds().y + getInsets().top);
    }
}

// Figure.java:2045-2049
public void translateFromParent(Translatable t) {
    if (useLocalCoordinates()) {
        t.performTranslate(-getBounds().x - getInsets().left,
                          -getBounds().y - getInsets().top);
    }
}
```

**关键点**:

- `useLocalCoordinates()` 返回 `false` 时，不进行任何转换
- 转换时需要考虑 `bounds` 位置和 `insets` 偏移

### 2.2 绝对坐标转换

```java
// Figure.java:2055-2062
public final void translateToAbsolute(Translatable t) {
    if (getParent() != null) {
        Translatable tPrecise = toPreciseShape(t);
        getParent().translateToParent(tPrecise);
        getParent().translateToAbsolute(tPrecise);
        fromPreciseShape(tPrecise, t);
    }
}
```

**关键点**:

- 递归向上遍历父节点链
- 支持双精度浮点数转换 (`PrecisionPoint`, `PrecisionRectangle` 等)

---

## 3. 位置传播机制

### 3.1 位置变化传播

当 Figure 的位置发生变化时，通过 `primTranslate` 方法传播到所有子节点：

```java
// Figure.java:1390-1398
protected void primTranslate(int dx, int dy) {
    bounds.x += dx;
    bounds.y += dy;
    if (useLocalCoordinates()) {
        fireCoordinateSystemChanged();
        return;
    }
    children.forEach(child -> child.translate(dx, dy));
}
```

`child.translate` 方法的实现：

```java
// Figure.java:2035-2039
@Override
public final void translate(int x, int y) {
    primTranslate(x, y);
    fireFigureMoved();
}
```

**关键点**:

- `translate` 是一个 final 方法，调用 `primTranslate` 执行实际的位移，然后触发 `figureMoved` 事件
- 如果当前 Figure 为 children 提供局部坐标系，只触发坐标系统变化事件，不修改
  children 的 bounds；children 的视觉绝对位置仍会随该坐标根移动
- 否则，递归调用子节点的 `translate` 方法，形成位置传播链

### 3.2 setBounds 中的处理

```java
// Figure.java:1675-1700
public void setBounds(Rectangle rect) {
    int x = bounds.x, y = bounds.y;
    boolean resize = (rect.width != bounds.width) || (rect.height != bounds.height);
    boolean translate = (rect.x != x) || (rect.y != y);

    if ((resize || translate) && isVisible()) {
        erase();
    }
    if (translate) {
        int dx = rect.x - x;
        int dy = rect.y - y;
        primTranslate(dx, dy);
    }

    bounds.width = rect.width;
    bounds.height = rect.height;

    if (translate || resize) {
        if (resize) {
            invalidate();
        }
        fireFigureMoved();
        repaint();
    }
}
```

**关键点**:

- 区分移动(translate)和调整大小(resize)
- 移动时调用 `primTranslate` 传播位置变化
- 触发 `figureMoved` 和重绘事件

---

## 4. 缩放(Scalable) Figure 的处理

### 4.1 IScalablePane 的坐标转换

对于实现 `ScalableFigure` 接口的容器，坐标转换需要额外考虑缩放因子：

```java
// IScalablePane.java:114-120
static void translateToParent(IScalablePane figurePane, Translatable t) {
    t.performScale(figurePane.getScale());
}

static void translateFromParent(IScalablePane figurePane, Translatable t) {
    t.performScale(1 / figurePane.getScale());
}
```

**关键点**:

- 缩放变换与位移变换叠加
- `translateToParent` 放大子节点坐标
- `translateFromParent` 缩小父节点坐标

### 4.2 Client Area 计算

```java
// IScalablePane.java:72-75
static Rectangle getClientArea(IScalablePane figurePane,
                                Function<Rectangle, Rectangle> superMethod,
                                Rectangle rect) {
    return figurePane.getScaledRect(superMethod.apply(rect));
}

// IScalablePane.java:44-48
public default Rectangle getScaledRect(Rectangle rect) {
    double scale = getScale();
    rect.scale(1 / scale);
    return rect;
}
```

### 4.3 Scale、Preferred Size 与 Bounds

`ScalableLayeredPane.setScale()` 只保存 scale，并触发 `fireMoved()`、`revalidate()`
和 `repaint()`，不会按比例修改当前 bounds。

`IScalablePaneHelper.getPreferredSize/getMinimumSize` 先用 `hint / scale` 查询
未缩放布局结果，再将结果乘 scale。`ViewportLayout` 使用这个 scaled preferred
size 分配 contents bounds；`Viewport.validate()` 最后依据 contents bounds 重算
RangeModel。

因此不能用“旧 bounds × scale ratio”作为下一次缩放尺寸。完整对标见
[scalable-zoom.md](scalable-zoom.md)。

---

## 5. 视口(Viewport) 的处理

### 5.1 视口坐标转换

Viewport 通过 `RangeModel` 管理滚动位置，并在坐标转换中体现：

```java
// Viewport.java:357-362
public void translateToParent(Translatable t) {
    if (useTranslate) {
        t.performTranslate(-getHorizontalRangeModel().getValue(),
                          -getVerticalRangeModel().getValue());
    }
    super.translateToParent(t);
}

// Viewport.java:346-351
public void translateFromParent(Translatable t) {
    if (useTranslate) {
        t.performTranslate(getHorizontalRangeModel().getValue(),
                          getVerticalRangeModel().getValue());
    }
    super.translateFromParent(t);
}
```

**关键点**:

- Viewport 的滚动偏移量作为额外的坐标变换
- 基础 Figure 的变换仍然保留

---

## 6. 自由形式(Freeform) Figure 的处理

### 6.1 FreeformHelper 的职责

FreeformFigure 允许子元素具有任意位置，其辅助类处理范围计算：

```java
// FreeformHelper.java:88-97
public void setFreeformBounds(Rectangle bounds) {
    host.setBounds(bounds);
    bounds = bounds.getCopy();
    host.translateFromParent(bounds);
    for (IFigure child : host.getChildren()) {
        if (child instanceof FreeformFigure freeFormFig) {
            freeFormFig.setFreeformBounds(bounds);
        }
    }
}
```

**关键点**:

- 递归设置子元素的自由形式边界
- 使用 `translateFromParent` 转换坐标

---

## 7. 验证与重绘机制

### 7.1 Revalidate 流程

```java
// Figure.java:1615-1622
public void revalidate() {
    invalidate();
    if (getParent() == null || isValidationRoot()) {
        getUpdateManager().addInvalidFigure(this);
    } else {
        getParent().revalidate();
    }
}

// Figure.java:1118-1125
public void invalidateTree() {
    invalidate();
    children.forEach(IFigure::invalidateTree);
}
```

**关键点**:

- 验证请求向上传递给父节点
- `invalidateTree()` 递归使整棵树失效

### 7.2 Validation

```java
// Figure.java:2174-2181
public void validate() {
    if (isValid()) {
        return;
    }
    setValid(true);
    layout();
    children.forEach(IFigure::validate);
}
```

**关键点**:

- 先布局自身，再验证子节点（前序遍历）

---

## 8. 事件监听机制

### 8.1 坐标系统变化通知

```java
// Figure.java:529-534
protected void fireCoordinateSystemChanged() {
    if (!eventListeners.containsListener(CoordinateListener.class)) {
        return;
    }
    eventListeners.getListenersIterable(CoordinateListener.class)
        .forEach(lst -> lst.coordinateSystemChanged(this));
}
```

### 8.2 Figure 移动通知

```java
// Figure.java:542-547
protected void fireFigureMoved() {
    if (!eventListeners.containsListener(FigureListener.class)) {
        return;
    }
    eventListeners.getListenersIterable(FigureListener.class)
        .forEach(lst -> lst.figureMoved(this));
}
```

---

## 9. 渲染时的坐标处理

### 9.1 paintClientArea 中的坐标变换

```java
// Figure.java:1328-1353
protected void paintClientArea(Graphics graphics) {
    if (children.isEmpty()) {
        return;
    }

    if (useLocalCoordinates()) {
        graphics.translate(getBounds().x + getInsets().left,
                          getBounds().y + getInsets().top);
        if (!optimizeClip()) {
            graphics.clipRect(getClientArea(PRIVATE_RECT));
        }
        graphics.pushState();
        paintChildren(graphics);
        graphics.popState();
        graphics.restoreState();
    } else {
        if (optimizeClip()) {
            paintChildren(graphics);
        } else {
            graphics.clipRect(getClientArea(PRIVATE_RECT));
            graphics.pushState();
            paintChildren(graphics);
            graphics.popState();
            graphics.restoreState();
        }
    }
}
```

**关键点**:

- 使用局部坐标时，Graphics 需要先 translate 到 Figure 内部
- 子节点在局部坐标系中绘制

---

## 10. 关键设计模式总结

### 10.1 转换链

```
绝对坐标 ←→ 父 Figure ←→ 子 Figure ←→ ...
           translateToParent/FromParent
```

### 10.2 useLocalCoordinates 的影响

| 场景              | useLocalCoordinates=true | useLocalCoordinates=false |
| ----------------- | ------------------------ | ------------------------- |
| 子节点位置        | 相对于父节点 (0,0)       | 绝对位置                  |
| translateToParent | 执行坐标变换             | 无操作                    |
| 位置传播          | 不传播，触发事件         | 传播给所有子节点          |

### 10.3 变换类型

1. **位移变换**: `performTranslate(dx, dy)` - 基础位置移动
2. **缩放变换**: `performScale(factor)` - 用于缩放容器
3. **滚动变换**: RangeModel 值 - 用于 Viewport

---

## 参考源码

- Figure.java: 核心实现
- IFigure.java: 接口定义
- ScalableLayeredPane.java: 缩放容器实现
- Viewport.java: 视口实现
- FreeformHelper.java: 自由形式辅助类
- 源码基线: eclipse/gef-classic commit `4463d9d0c`（2026-01-01）
