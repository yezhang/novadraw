# Reusable Shape 与 Border 产品化契约

类型：`normative-design`

状态：`accepted`

范围：M10.1

Draw2D 源码事实见
[`../../reference/draw2d/figure/reusable-shape-border.md`](../../reference/draw2d/figure/reusable-shape-border.md)。

## 1. 目标

M10.1 将已有基础图元提升为可复用产品能力，并补齐文本无关 Border：

- RectangleFigure 作为行为 baseline；
- EllipseFigure；
- RoundedRectangleFigure；
- PolylineFigure；
- PolygonFigure；
- TriangleFigure；
- LineBorder；
- MarginBorder；
- CompoundBorder；
- EtchedBorder；
- BevelBorder。

本阶段闭合：

- 绘制；
- 精确命中；
- visual bounds；
- intrinsic/preferred size；
- 样式消费；
- 运行期几何变化；
- validation、damage 和通知；
- Border insets、preferred size、opacity 和组合语义。

## 2. 非目标

M10.1 不包含：

- 重新设计 M1 Graphics 状态栈、坐标、裁剪或基础命令；
- clipPath、shear、gradient、XOR、打印和平台专属 Graphics；
- line dash、可配置 miter limit 等高级 stroke 扩展；
- Label、TitleBarBorder 或任何依赖文字测量的 Border；
- ImageFigure；
- Button、Toggle、Tooltip 和 Accessibility bridge；
- 富文本 Flow。

TitleBarBorder 移入 M10.2，与字体测量、Label 和 text cache 同批设计。

## 3. 核心原则

### 3.1 Graphics 冻结

M1 已验证的 Graphics 基础契约在 M10.1 保持冻结。Reusable Figure 优先组合现有
path、fill、stroke 和基础图元命令。

只有同时满足以下条件才允许新增 Graphics primitive：

1. 至少一个 M10.1 产品 Figure 无法基于现有 command 正确表达；
2. 新 primitive 具有跨 Figure 或跨 backend 的稳定语义；
3. command、Vello backend、Headless snapshot 和退化输入测试能同批闭合；
4. 不引入平台专属状态或静默 no-op。

RoundedRectangleFigure 当前可由 path 表达，因此
`draw_rounded_rectangle/fill_rounded_rectangle` 不是预设交付项。

### 3.2 单一状态真值

- NodeState bounds 是 bounded Figure 的布局几何真值；
- FigureStyle / ResolvedStyle 是 foreground、background、alpha 等通用样式真值；
- 具体 Figure 只保存自身几何参数和 Shape 专属开关；
- point-list Figure 的 points 是路径真值，NodeState bounds 是其派生布局包围盒；
- Border 是 Figure 的装饰策略，不拥有 owner、Runtime 或 FigureTree 引用。

不得为方便绘制复制第二份可独立修改的 bounds、颜色或 inherited style。

### 3.3 运行期修改经过 Runtime

pre-Runtime 构建可使用 Figure 构造器和 FigureTreeBuilder。Figure 进入 Runtime 后：

- 通用 style 使用既有 Runtime/FigureTree style mutation；
- bounds 使用既有 geometry mutation；
- point list、corner dimensions 和 triangle direction 使用命名的 typed mutation；
- Border replacement 使用 Runtime mutation。

运行期不得向调用方暴露可变 PointList、可变 Border 引用或可绕过
validation/damage 的具体 Figure 可变引用。

### 3.4 Shape 是绘制辅助，不是第二棵对象树

Shape 保留小型模板语义：

```text
paint shape
→ fill when enabled
→ outline when enabled
```

它不拥有：

- topology；
- bounds；
- inherited style；
- UpdateManager；
- Runtime；
- backend。

新 Shape 类型通过实现 Figure 和 Shape 绘制行为扩展，不需要注册全局类型或修改
渲染主循环。

## 4. Shape 样式契约

通用颜色和 alpha 继续由 FigureStyle/ResolvedStyle 提供：

- background：fill color；
- foreground：outline color；
- alpha：整个 Figure 的绘制 alpha。

Shape 专属状态只包含：

- fill enabled；
- outline enabled；
- stroke width；
- line cap；
- line join。

M10.1 不增加宽的统一 Shape setter 接口。构建期由具体 Figure builder 设置 Shape
参数；运行期只有出现真实产品 mutation 时才增加对应 typed Runtime operation。

变化分类：

| 变化 | validation | damage | 通知 |
|---|---|---|---|
| fill/outline/color/alpha | 否 | repaint old/new visual | typed property |
| cap/join | 否 | repaint old/new visual | typed property |
| stroke width | point-list Figure 需要重算派生 bounds；bounded Figure 不改变 layout bounds | old/new visual | typed property |
| point list/corner/direction | intrinsic size 变化时需要 | old/new visual | typed property |
| Border replacement | 是 | old/new visual | typed property |

相同值写入不得产生 validation、damage 或通知。

## 5. 几何与命中

### 5.1 Bounded Figure

Rectangle、Ellipse、RoundedRectangle 和 Triangle 使用 NodeState 的 local border box：

```text
border_box = Rect(0, 0, node.width, node.height)
client_box = border_box.inset(border.insets)
```

Rectangle、Ellipse 和 RoundedRectangle 使用 border box；Triangle 与 Draw2D 一致，
从 client box 派生顶点，避免覆盖 Border 占用区域。Border 仍在 children 之后绘制。
Shape outline 必须落在 bounds 内，不扩大布局 bounds。

具体 paint geometry 保留 Draw2D 语义：

- Rectangle fill 使用完整 border box，outline 按线宽向内收缩；
- RoundedRectangle fill 使用完整 border box，outline 按线宽向内收缩，同时缩减
  corner dimensions；
- Ellipse fill 与 outline 都使用按线宽向内收缩后的 optimized bounds；
- Triangle 从 client box 宽高各缩减 1px 后，按 direction 生成居中的 2:1
  底边/高度三角形。

命中规则：

- Rectangle：矩形区域；
- Ellipse：椭圆内部；
- RoundedRectangle：裁去四角椭圆外部后的区域；
- Triangle：实际三角形内部，不复制 Draw2D 的矩形退化行为。

命中采用几何区域，不因 fill/outline 是否启用而改变。是否参与 self hit-test 由既有
HitParticipation 决定。

### 5.2 Corner geometry

RoundedRectangle 的规范输入是二维 corner dimensions：

```text
corner.width >= 0
corner.height >= 0
```

每个维度在使用前限制到对应 bounds 尺寸。已有单值 radius 可以保留为 convenience，
但必须明确映射到相等的 width/height，不能形成第二套几何语义。

### 5.3 Point-list Figure

Polyline 和 Polygon 的规范路径由有序 PointList 表达：

- mutation 输入使用 parent child-content domain；
- Runtime 计算 path/stroke envelope；
- Runtime 将 points 规范化为 Figure local domain；
- NodeState bounds 与局部 points 在同一事务提交；
- 空列表或不足绘制点数时不输出绘制命令。

空列表、单点 Polyline、少于三个点的 Polygon 都是合法几何状态：

- 空列表提交为空 local points，并使用 parent domain 原点处的零尺寸 bounds；
- 非空但不足绘制点数时，bounds 由已有点的几何范围派生，不增加不存在的 stroke
  envelope；
- 只有形成实际绘制路径时，派生 bounds 才包含对应 cap、join 和 stroke width；
- `clear` 必须成功并 damage 旧 visual bounds，不能因新几何不可绘制而失败。

Polyline hit-test：

```text
distance(point, any segment)
<= max(stroke_width / 2, hit_tolerance)
```

Polygon hit-test 使用闭合 polygon interior；边界点必须命中。children 仍由树遍历独立
命中，不由 Shape 自行递归。

point mutation 至少覆盖：

- replace all；
- insert；
- replace one；
- remove one；
- clear。

非法索引、非有限坐标或算术溢出时，整个 mutation 失败且不得部分提交。空或不足绘制
点数不是错误。

### 5.4 Visual bounds

- bounded Shape 和 M10.1 Border 都必须绘制在 NodeState bounds 内；
- point-list visual bounds 必须覆盖 cap、join 和 stroke width；
- fill-only Polygon 使用 polygon bounds；
- geometry 退化为点或零长度 segment 时不得产生 NaN；
- visual bounds 必须与 render command 和 precise hit-test 使用同一几何来源。

若现有 stroke API 无法给出精确 join envelope，可以保守扩大，但扩大规则必须来自
stroke style，而不是 magic number。

## 6. Border 契约

### 6.1 不可变、可复用策略

Border 在附加后视为不可变配置。需要改变颜色、宽度、padding 或组合关系时，构造新
Border 并通过 Runtime 替换。

这保留 Draw2D 的可复用性，同时避免 mutable shared Border 修改后无法通知所有 owner。
禁止 Border 持有 owner Figure、Runtime、FigureTree 或全局注册表。

### 6.2 指标

Border 对外提供三个独立指标：

```text
insets
preferred_size
is_opaque
```

- insets 收缩 client area；
- preferred size 表示 Border 自身正确显示所需的最小外部尺寸；
- is_opaque 只声明 Border 配置能否覆盖 border ring，不等价于 NodeState.opaque；
- 最终 effective opacity 还必须要求实际颜色和 ResolvedStyle alpha 均为完全不透明。

默认值：

- preferred size：zero；
- is_opaque：false。

布局尺寸组合：

```text
content_with_insets = content_preferred + border.insets
figure_preferred =
    max_componentwise(content_with_insets, border.preferred_size)
```

### 6.3 绘制

Border 接收只读 paint input：

- local border-box bounds；
- 调用方累计 insets；
- 当前已解析的 Figure style；
- NdCanvas。

Border 不查询或修改 FigureTree。M10.2 若需要文字测量，在同一只读上下文边界扩展
measurement capability，不向 Border 暴露 Runtime。

所有 M10.1 Border 在 bounds 内绘制，不改变 visual bounds。

### 6.4 CompoundBorder

CompoundBorder 包含 outer 和 inner 两个可选 Border：

```text
insets = outer.insets + inner.insets
preferred =
    max_componentwise(
        inner.preferred + outer.insets,
        outer.preferred
    )
opaque = outer.is_opaque && inner.is_opaque
```

缺失的 outer/inner 在 insets、preferred size 和 paint 中按空 Border 处理；任一侧
缺失时 CompoundBorder 为 non-opaque。该规则保留 Draw2D 的 opacity 语义，同时避免
其默认构造后查询 preferred size 时的空引用风险。

绘制时 outer 和 inner 使用隔离的 Graphics state。inner 的累计 inset 等于调用方
inset 加 outer.insets。

### 6.5 产品 Border

- LineBorder：等宽四边，stroke 完全位于 bounds 内；
- MarginBorder：只贡献 insets，不绘制，non-opaque；
- CompoundBorder：组合任意两个 Border；
- EtchedBorder：两层相反明暗线；
- BevelBorder：通过 `Raised | Lowered` 模式选择 top-left/bottom-right 颜色顺序。

Etched/Bevel 使用普通 line command，不增加 Graphics primitive。

TitleBarBorder 延后到 M10.2，因为其 insets、preferred size 和 paint 都依赖字体与文字
测量。ButtonBorder 延后到 M10.4，因为它依赖 ButtonModel 状态。

## 7. 事务与失败模型

### 7.1 原子更新

geometry/style/Border mutation 遵循：

```text
validate input
→ capture old visual bounds and layout contribution
→ apply one logical change
→ recompute derived geometry
→ enqueue validation when required
→ damage old and new visual bounds
→ emit typed notification
```

任一步验证失败时：

- Figure 状态不变；
- NodeState bounds 不变；
- 不产生通知；
- 不留下 partial damage 或 invalid cache。

### 7.2 错误

至少区分：

- unknown/detached FigureId；
- wrong Figure capability；
- non-finite geometry；
- negative stroke width/corner/insets；
- point index out of range；
- invalid Border metrics；
- unsupported operation。

不得静默 clamp 非有限值、吞掉非法索引或返回占位成功。

## 8. 扩展点

### 新 Shape

新 Shape：

1. 定义自身几何参数；
2. 实现 fill/outline；
3. 实现 precise hit；
4. 声明 bounded 或 point-derived bounds；
5. 复用现有 Runtime mutation 和样式协议。

不修改 FigureTree traversal 或 RenderBackend 类型分派。

### 新 Border

新 Border：

1. 纯计算 metrics；
2. 在只读上下文中 paint；
3. 声明 opacity；
4. 不持有 owner 或隐藏可变状态。

文本、图像或控件相关 Border 在对应 M10 子阶段扩展上下文能力。

### 新 RenderBackend

Backend 只实现既有 RenderCommand。M10.1 Figure 和 Border 不直接依赖 Vello，也不向
backend 添加 Shape/Border 类型识别。

## 9. Draw2D 对应与合理变体

| 语义 | Draw2D | Novadraw |
|---|---|---|
| Shape paint | fill 后 outline | 相同 |
| 通用颜色 | Graphics foreground/background | ResolvedStyle 写入 NdCanvas，语义相同 |
| PointList mutation | 暴露可变列表并手工通知 | Runtime typed mutation 原子提交 |
| Polyline bounds | points + stroke 派生 cache | points + stroke 派生 NodeState bounds |
| Triangle hit | 默认矩形 | 精确三角形 |
| Border 复用 | 可共享且可变 | 不可变策略，可安全复用 |
| Border owner 输入 | 直接传 IFigure | 只读 Border paint/measure context |
| Scheme opacity | public API 恒报 opaque，和内部颜色检查不一致 | 配置 opacity 再结合实际颜色与 resolved alpha |
| CompoundBorder | outer/inner 嵌套 | 相同公式与状态隔离 |
| TitleBarBorder | Border 内直接测量文本 | 移至 M10.2，依赖统一文本测量 |
| Graphics 扩展 | 宽 SWT 风格 API | 仅增加产品 Figure 证明需要的最小 primitive |

## 10. 验证门禁

### 10.1 Contract tests

- 六类 Figure 的 fill/outline render command；
- bounded Shape 的 stroke 完全位于 bounds 内；
- Ellipse/RoundedRectangle/Polygon/Triangle 精确命中；
- Polyline segment tolerance 和 stroke-aware bounds；
- point insert/replace/remove/clear 的原子 bounds、damage 和通知；
- 非有限输入、非法 index、退化几何错误；
- Border insets、preferred size、opacity；
- CompoundBorder 指标公式、outer/inner 顺序和 Graphics state 隔离；
- Border replacement 同时触发 validation 与 old/new damage；
- inherited FigureStyle 与 Shape fill/outline 的消费一致。

### 10.2 Demo

`shape-app` 至少覆盖：

- fill-only、outline-only、fill+outline；
- 1px、奇数和偶数宽 stroke；
- Rectangle、Ellipse、RoundedRectangle、Polyline、Polygon、Triangle；
- 精确命中可视反馈；
- 运行期 point/corner/direction 更新。

`border-app` 至少覆盖：

- Line、Margin、Compound、Etched、Raised、Lowered；
- insets 对 client area/layout 的影响；
- preferred size 下限；
- opaque 与 transparent border；
- nested CompoundBorder 绘制顺序。

### 10.3 Graphics 守门

- M10.1 默认不得修改 Graphics 状态模型或渲染主循环；
- 若新增 primitive，必须单独列出受影响的 `graphics.context` API 行；
- Native、Web、Headless command snapshot 必须语义一致；
- 不允许为了 API 对称添加 backend 静默忽略的命令。

## 11. 实施顺序

1. 审查并批准本契约。
2. 建立 M10.1 contract tests，先固定 Shape/Border 行为。
3. 完成 bounded Shape 命中与 visual bounds。
4. 完成 point-list mutation、派生 bounds 和 damage。
5. 补齐 Border metrics、Compound、Etched 和 Bevel。
6. 更新 shape-app 和 border-app。
7. 完成 Native 截图、Headless snapshot 与人工验收。
8. 更新 API 账本并将 M10.1 标记为 complete。

实现期间若发现必须修改 M1 Graphics 稳定契约，应停止当前实现，提交独立
architecture delta 说明原因、兼容性和 backend 影响，经批准后再继续。
