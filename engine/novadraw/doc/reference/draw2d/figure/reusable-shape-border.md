# Draw2D Reusable Shape 与 Border 源码语义

类型：`reference-analysis`

本文只记录 Eclipse Draw2D 的源码事实，不直接定义 Novadraw 契约。Novadraw 的
M10.1 决策见
[`../../../design/architecture/reusable-shape-border.md`](../../../design/architecture/reusable-shape-border.md)。

参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`

参考提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 1. 源码范围

主要源码：

- `Shape.java`
- `RectangleFigure.java`
- `RoundedRectangle.java`
- `Ellipse.java`
- `AbstractPointListShape.java`
- `Polyline.java`
- `Polygon.java`
- `Triangle.java`
- `Border.java`
- `AbstractBorder.java`
- `LineBorder.java`
- `MarginBorder.java`
- `CompoundBorder.java`
- `SchemeBorder.java`
- `SimpleEtchedBorder.java`
- `SimpleRaisedBorder.java`
- `SimpleLoweredBorder.java`
- `AbstractLabeledBorder.java`
- `TitleBarBorder.java`
- `Figure.java`
- `AbstractLayout.java`

## 2. Shape 基类语义

`Shape` 是 `Figure` 的绘制模板，不是独立场景节点：

```text
Shape.paintFigure
→ optional antialias / alpha
→ disabled-state extra pass
→ paintFill
→ paintOutline
```

具体 Shape 不应覆盖 `paintFigure`，而应实现 `fillShape` 和 `outlineShape`。

默认状态：

- fill：启用；
- outline：启用；
- line width：1；
- alpha 和 antialias：继承 Graphics 当前状态；
- XOR fill/outline：关闭。

fill、outline、alpha、antialias 和 line attributes 改变时，Draw2D 只触发 repaint。
它们通常不改变 Figure 的布局尺寸。Polyline 是例外：line width 会影响其派生 bounds，
因此会先清除 bounds cache。

Draw2D 将 background 用作 fill color，将 foreground 用作 outline color。Shape
本身不额外保存一套 fill/stroke 颜色。

## 3. Bounded Shape

### 3.1 RectangleFigure

- fill 使用完整 bounds；
- outline 中心线向内收缩 `max(1, lineWidth) / 2`；
- 奇数像素线宽使用 floor/ceil 分配，保证 outline 留在 bounds 内。

### 3.2 RoundedRectangle

- fill 使用完整 bounds 和 corner width/height；
- outline 与 RectangleFigure 使用相同的向内收缩规则；
- outline 的 corner dimensions 同时减去 line inset，并截断到非负；
- corner 使用二维 `Dimension`，不是单一半径。

### 3.3 Ellipse

- `containsPoint` 先做 bounds 快速拒绝，再做椭圆方程判断；
- fill 和 outline 都使用按 line width 收缩后的 optimized bounds；
- 命中语义与 fill/outline 开关无关，仍按椭圆区域判断。

### 3.4 Triangle

- direction/orientation 改变会 revalidate 和 repaint；
- validate 阶段从 bounds 扣除 insets、再将宽高各缩减 1px 后计算三个顶点；
- 三角形保持 Draw2D 的 2:1 底边/高度关系，在剩余 client box 内按 direction 居中；
- 顶点是派生缓存，不是第二份独立布局真值；
- Draw2D Triangle 没有覆盖 `containsPoint`，因此沿用 Figure 的矩形命中。这是源码
  行为，不代表 Novadraw 必须复制其低精度命中。

## 4. Point-list Shape

`AbstractPointListShape` 持有 PointList，并提供 add/insert/remove/set 等 mutation：

- add/insert：修改 point list 后 repaint；
- remove/set：先 erase 旧区域，再修改并 repaint；
- `setPoints` 按引用保存传入列表，因此调用方直接修改后必须再次通知 Figure；
- `containsPoint` 同时考虑自身几何和 children。

`Polyline` 进一步规定：

- Figure 位置由 points 决定，不应通过 `setBounds` 定位；
- bounds 是 point bounds 按 line width 扩张后的派生缓存；
- repaint 会清除 bounds cache；
- hit-test tolerance 为 `max(lineWidth / 2, configuredTolerance)`；
- Polyline 永不 opaque，也不参与 fill。

`Polygon` 复用 Polyline 的 point storage，但：

- fill 和 outline 使用闭合路径；
- hit-test 使用 polygon interior；
- bounds 仍由 point list 派生。

空 PointList 是合法状态，`PointList.getBounds()` 返回原点处的零尺寸 Rectangle。
Draw2D 也允许只含一个点或不足以形成可见 polyline/polygon 的列表；这些状态不因
无法绘制而被视为 mutation 错误。

## 5. Border 基础协议

Draw2D `Border` 有四项核心语义：

```text
getInsets(figure)
getPreferredSize(figure)
isOpaque()
paint(figure, graphics, incomingInsets)
```

Border 实例允许被多个 Figure 共享。`Figure.setBorder` 在 identity 变化时同时触发：

```text
revalidate
repaint
```

原因是 Border 同时影响：

- client area；
- layout preferred size；
- 最终 border paint。

Border 在 Figure children 之后绘制。`paint` 的目标区域是：

```text
outer = figure.bounds inset incomingInsets
inner = outer inset border.getInsets(figure)
paint region = outer - inner
```

`Border.isOpaque` 描述的是 border ring 是否完全覆盖，不等价于 Figure 的整个
background 是否 opaque。

## 6. Border 实现语义

### 6.1 AbstractBorder

默认：

- preferred size 为 zero；
- opaque 为 false；
- `getPaintRectangle` 返回 Figure bounds 扣除调用方传入 insets 后的区域。

### 6.2 LineBorder

- 四边 insets 等于 line width；
- opaque 为 true；
- stroke centerline 按线宽向内收缩；
- 奇数像素线宽会额外缩减 width/height，以保持绘制在 bounds 内。

### 6.3 MarginBorder

- 只贡献 insets；
- paint 不输出命令；
- 继承 AbstractBorder 的 zero preferred size 和 non-opaque。

### 6.4 CompoundBorder

outer 和 inner 的组合规则：

```text
insets = outer.insets + inner.insets
preferred =
    max_componentwise(
        inner.preferred + outer.insets,
        outer.preferred
    )
opaque = outer.opaque && inner.opaque
```

绘制顺序：

1. push Graphics state；
2. 在 incoming insets 内绘制 outer；
3. pop Graphics state；
4. 将 incoming insets 加上 outer.insets；
5. 绘制 inner。

outer 的 Graphics 状态不能泄漏到 inner。

### 6.5 Scheme / Etched / Raised / Lowered

SchemeBorder 以 top-left highlight colors 和 bottom-right shadow colors 表达立体边框：

- 每个颜色对应一像素层；
- highlight 与 shadow 数量决定各边 inset；
- paint 固定使用 1px solid line；
- Raised 与 Lowered 只是 highlight/shadow 顺序不同；
- Etched 使用两组相反的明暗顺序。

这是一种边框策略，而不是新的 Graphics primitive。

当前基线中 `Scheme.calculateOpaque()` 会检查 highlight/shadow 是否包含 null，
但 `SchemeBorder.isOpaque()` 直接返回 true，没有读取该计算结果。这是源码中的
不一致；Novadraw 不应把可能透明的颜色错误声明为 opaque。

### 6.6 TitleBarBorder

TitleBarBorder 不是纯几何 Border：

- insets 依赖文字高度和 padding；
- preferred size 依赖文字测量；
- paint 依赖 Figure 的 font；
- label/font/padding 改变会清除文本和 insets cache；
- opaque 为 true。

因此 TitleBarBorder 与文本测量契约存在硬依赖，不能在文本系统之前独立完成。

## 7. Layout 对 Border 的消费

Draw2D Layout 先计算内容尺寸并加上 Figure insets，再与 Border preferred size
做 component-wise union。以 GridLayout 为例：

```text
contentPreferred
→ expand(borderInsets)
→ union(borderPreferredSize)
```

Border preferred size 不是 insets 的别名。普通 Line/Margin Border 通常返回 zero，
TitleBarBorder 等带内容 Border 才会扩大最小尺寸。

## 8. 不应机械复制的实现

- Draw2D 的 mutable shared Border 没有自动向所有 owner 广播变化；Novadraw 不应复制
  这种隐式共享可变性。
- PointList 按引用暴露并要求调用方手工通知，容易产生 stale bounds；Rust API 应通过
  受控 mutation 原子更新 point list、bounds、damage 和通知。
- Triangle 的矩形命中精度低于其真实几何；Novadraw 的 reusable Figure 应提供精确
  shape hit-test。
- XOR 和 SWT antialias 是平台实现细节，不属于 M10.1 基础契约。
- SchemeBorder 的视觉结果可用普通线段命令表达，不需要扩张 Graphics 抽象。

## 9. 对 M10.1 的直接约束

1. 保留 Shape 的 fill-before-outline 模板语义。
2. Shape outline 和 Border 必须绘制在 bounds 内。
3. point-list bounds 是 geometry/stroke 的派生值，不能成为调用方独立维护的第二真值。
4. Border replacement 同时影响 validation 和 repaint。
5. CompoundBorder 必须保留 insets、preferred size、opacity 和隔离绘制公式。
6. TitleBarBorder 移至具备字体测量能力的 M10.2。
7. Graphics 只在现有 path/command 无法表达产品 Figure 时增加最小 primitive。
