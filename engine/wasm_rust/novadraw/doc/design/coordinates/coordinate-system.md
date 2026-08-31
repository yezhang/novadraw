# Novadraw 坐标与变换契约

类型：`normative-design`

本文是 Novadraw 二维坐标、变换、命中和 damage 投影的 SSOT。Draw2D 的参考事实见
`doc/reference/draw2d/figure/tree-coordinates.md`。

## 1. 设计选择

Novadraw 保留 Draw2D 的核心行为：

- 坐标转换沿 Figure 父链组合；
- parent/child 转换必须双向；
- insets、scroll 和 scale 都是坐标协议的一部分；
- paint、hit-test、事件点和 damage 使用同一转换来源。

Novadraw 不复制 Draw2D 的物理存储方式。所有节点统一采用：

> `bounds` 存储在 parent content domain 中，节点自身在 local border-box domain 中绘制。

因此移动 parent 不改写后代 bounds，只使后代的 world transform 和 projected bounds
失效。

## 2. 坐标域

### 2.1 Physical Surface Domain

窗口表面或 canvas backing store 的物理像素。只允许出现在 PlatformHost 和
RenderBackend 边界。

### 2.2 Logical Surface Domain

设备无关的根逻辑坐标，也是平台输入进入 Runtime 后的 entry domain：

```text
logical = physical / scale_factor
physical = logical * scale_factor
```

### 2.3 Parent Content Domain

节点的 `bounds` 所属坐标域。原点是 parent client area 的左上角，并包含 parent 为
children 声明的 scroll、scale 或其他二维 child transform。

### 2.4 Node Local Domain

节点自身绘制和精确命中的坐标域：

```text
local border box = Rect(0, 0, bounds.width, bounds.height)
local client box = local border box inset by insets
```

Figure 不需要知道自己在父树中的绝对位置。

### 2.5 Child Content Domain

children bounds 所属域。它由当前节点的 client origin 和 `child_transform` 定义。
普通容器的 child transform 是 identity；Viewport、ScalablePane 等容器可以提供
scroll 或 scale。

## 3. 边变换

对节点 `N`，定义：

```text
node_local_to_parent_content(N)
    = Translate(N.bounds.origin) * N.local_transform

child_content_to_node_local(N)
    = Translate(N.insets.left, N.insets.top) * N.child_transform
```

完整的 child 到 parent 映射为：

```text
child_local
→ child node placement
→ parent child-content transform
→ parent local
→ parent placement
```

实现可以缓存组合矩阵，但所有消费者必须从同一 edge-transform 协议取得结果。

矩阵使用列向量语义。向已有 parent/world 变换追加 local 变换时必须执行：

```text
world = parent_world * local
```

对应 API 为 `parent_world.post_concat(local)`。不得使用
`local * parent_world`，否则父层 scale/rotation 会与子节点 placement 以错误顺序组合。

## 4. Affine2D

二维核心使用显式仿射变换：

```rust
pub struct Affine2D {
    // [a c tx]
    // [b d ty]
    // [0 0  1]
    coefficients: [f64; 6],
}
```

它支持：

- translate；
- scale；
- rotate；
- skew；
- composition；
- inverse；
- point、vector、rect 和 path 变换。

矩阵组合顺序必须由 API 文档定义，不能依赖调用者猜测。建议使用命名：

```text
then_translate
then_scale
pre_concat
post_concat
map_point
```

并通过非交换变换测试锁定语义。

## 5. World Transform

```text
local_to_surface(id)
    = compose every edge transform from id to root

surface_to_local(id)
    = inverse(local_to_surface(id))
```

缓存至少带有 topology/transform generation。以下变化必须使相关缓存失效：

- bounds origin；
- insets；
- child transform；
- ancestor reparent；
- viewport origin；
- scale；
- visual transform 变化。

不可逆变换不能产生伪造坐标。调用应返回 `None` 或结构化错误。

## 6. Bounds

`NodeState.bounds` 是 parent content domain 中的布局矩形：

- origin 由 parent layout 或显式操作确定；
- size 参与 layout、client area 和默认 hit-test；
- bounds 不是 world-space AABB；
- bounds 不包含 shadow、blur 等视觉外扩；
- bounds 不因 ancestor 移动而被批量改写。

另外定义：

```text
layout_bounds     // parent content domain
local_border_box  // node local domain
visual_bounds     // local effects 后的可见范围
projected_bounds  // logical surface domain 中的保守 AABB
```

这些概念不能共用一个含义模糊的 rectangle。

### 6.1 旧场景一次性迁移

旧版“相对最近坐标根的绝对 bounds”场景必须在交给 Runtime 前调用：

```rust
FigureTree::migrate_legacy_bounds_to_parent_local(&legacy_coordinate_roots)
```

调用方负责提供旧模型中重置子树坐标域的 FigureId 集合。转换会使用修改前的父
bounds 与 insets 计算所有普通父子边的 parent-local bounds；旧坐标根的直接
children 保持原值。

该操作不是幂等的，不得在正常帧循环、事件回调或已经迁移的场景上调用。引擎不会
保存 legacy 标记，也不会根据场景内容自动猜测旧坐标模式。

## 7. Client Area 与 Insets

```text
client_box = Rect(0, 0, width, height).inset(insets)
```

client box 同时用于：

- LayoutManager 的默认可用区域；
- children paint 的默认 clip；
- hit-test 是否继续下降到 children；
- scroll/viewport 的可视范围。

自定义 child clipping policy 可以放宽默认裁剪，但必须同时定义 paint 和 hit-test
行为，不能只修改其中一条路径。

## 8. Paint

每个节点的绘制按以下顺序应用坐标：

```text
push
→ concat node_local_to_parent
→ paint Figure in local coordinates
→ clip local client box
→ concat child_content_to_node_local
→ paint children
→ restore child state
→ paint border/foreground
→ pop
```

Figure 自身只使用 local coordinates。具体后端可以把 `Affine2D` 转换为 `Mat3`、
`Mat4` 或后端原生 affine。

## 9. Hit-test

从 logical surface point 开始：

```text
root
→ inverse edge transform into node local
→ visible / clip / local hit-shape check
→ inverse child-content transform
→ children in reverse Z-order
→ deepest eligible target
```

规则：

- invisible 节点及其默认可见子树不参与；
- disabled 节点不能成为默认输入 target，但其可见 children 可按容器策略继续搜索；
- Figure 精确 hit-test 接收 node local point；
- 不可逆 transform 的分支不可命中；
- event callback 接收 target local point，并保留原始 logical surface point。

## 10. 几何变更与 Damage

`set_bounds` 必须原子处理：

```text
old_projected_visual_bounds
→ change bounds once
→ invalidate transform cache
→ new_projected_visual_bounds
→ damage union(old, new)
→ one geometry notification
```

resize 还会使自身或 parent layout 失效。移动不能产生额外的 resize/move 重复通知。

局部 dirty region 的传播：

```text
local dirty
→ expand by stroke/filter/effect
→ map through local-to-surface
→ intersect effective ancestor clips
→ conservative logical-surface AABB
```

旋转或投影后的矩形必须取保守 AABB，不能只变换左上和右下两个点。

## 11. Viewport、Scroll 与 Scale

Viewport 的状态：

```text
viewport local client box
origin: content coordinate visible at viewport top-left
scale: owned by scalable content/pane
```

典型映射：

```text
viewport_point = (content_point - origin) * scale
content_point = viewport_point / scale + origin
```

该映射实现为 child transform，不在 hit-test、event 或 damage 中建立专用全局捷径。

RangeModel 是 scroll origin 的唯一真源；ScalablePane 是 scale 的唯一真源。
ZoomManager 只协调二者，不建立第二份状态。

## 12. DPI

Runtime、Figure、LayoutManager 和 InputEvent 统一使用 logical units。只有两个边界处理
physical pixels：

- PlatformInputAdapter：physical input 到 logical input；
- RenderBackend：logical submission 到 physical surface。

scale factor 变化必须作为 surface transaction 处理，不能在不同事件路径中各自换算。

## 13. 2.5D Projective Composition

2.5D 是二维 layout 之后的视觉合成：

```text
2D local geometry
→ Affine2D world transform
→ Projective3D visual transform
→ surface projection
```

它不改变二维 layout bounds。每个 projective effect 必须声明：

- projected visual bounds 的计算；
- clip 是投影前还是投影后；
- hit-test 是否支持从 screen point 逆投影；
- 矩阵不可逆或 `w <= 0` 时的处理；
- backend 不支持时的 fallback。

仅需视觉效果时，默认可将 hit-test 保持在二维布局几何上；若产品要求“看到哪里点到
哪里”，则 capability 必须提供投影命中。

## 14. 真正 3D

真正 3D 不复用二维 `Point/Rect/LayoutManager`：

```text
Scene3D
├── Vec3 / Transform3D
├── Bounds3D
├── Camera / Projection
├── RayHitTest
├── FrustumCulling
└── Depth / Material / Lighting
```

Scene3D 可通过以下方式接入：

- 3D viewport Figure；
- 离屏 texture；
- 独立 render layer；
- 与二维 overlay 的显式坐标桥。

这让新增 3D 数学和渲染能力不要求修改所有二维 Figure、布局和事件 API。

## 15. 数值与失败规则

- 几何输入必须有限；
- size 不得为负；
- inverse 失败必须显式返回；
- scale factor 必须有限且大于零；
- 极大坐标的裁剪和 AABB 计算不得溢出；
- 浮点比较使用领域容差，不把容差写成散落的 magic number；
- projective transform 的 near-plane 和 `w` 规则由合成协议统一定义。

## 16. 验证契约

至少覆盖：

1. 多层 translation/scale/rotation 的往返；
2. bounds 始终保持 parent-local；
3. parent 移动不改写 descendants bounds；
4. paint、hit-test、event point 和 damage 使用同一变换；
5. insets 同时影响 layout、clip 和 child descent；
6. viewport/scale 嵌套；
7. 不可逆 transform；
8. old/new projected damage；
9. DPI 改变；
10. projective effect 的 bounds、clip 和可选命中；
11. 10,000 层边界下的明确行为。
