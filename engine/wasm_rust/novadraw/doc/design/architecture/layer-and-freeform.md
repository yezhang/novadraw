# Layer 与 Freeform 契约

类型：`normative-design`

状态：`approved`

本文定义 D2 的规范架构，由
[`ADR-004`](../../adr/adr-004-layer-and-freeform-contract.md) 接受。本文只定义
Draw2D 行为语义到 Novadraw 的映射，不复制 Java 继承结构。

## 1. 目标与边界

D2 提供：

- 默认透明的 `LayerFigure`；
- 由稳定 key 管理有序 layer 的 `LayeredPane`；
- 可包含负坐标内容的 `FreeformLayerFigure`；
- 组合 layer lookup 与 freeform extent 的 `FreeformLayeredPane`；
- 组合现有 scale 状态的 `ScalableFreeformLayeredPane` 便利类型；
- 保留 child 显式坐标的 `FreeformLayout`；
- 可缓存、可失效、可通知的 freeform extent；
- Freeform、Viewport、Zoom、paint、hit-test 和 damage 的组合契约。

D2 不提供 Connection、Anchor、Router、GEF Viewer/EditPart、selection、无限画布
分块存储或空间索引。ConnectionLayer 在 M9 基于本契约实现。

## 2. Draw2D 基线与 Novadraw 变体

参考仓库：

- `https://github.com/eclipse-gef/gef-classic.git`
- 基线提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

方法级证据：

- `Layer.containsPoint/findFigureAt`：透明 layer 只允许 descendants 命中；
- `LayeredPane.add/getLayer/removeLayer`：key 与 children 顺序同步；
- `FreeformHelper.getFreeformExtent/invalidate`：范围由 children 派生并向上传播失效；
- `FreeformLayout.getOrigin`：布局 origin 可根据最小 child 坐标调整；
- `FreeformViewport.readjustScrollBars`：freeform extent 与 viewport client area
  合并后更新 range model。

Novadraw 保留上述行为目标，并采用以下变体：

- `FigureId` 替代对象引用，layer key 使用强类型值；
- FigureTree 仍是 topology 与 Z-order 的唯一真源；
- freeform extent 从后代几何派生，不递归改写普通内容节点的 bounds；
- nested freeform 容器通过派生 extent/envelope 传播范围变化，不建立第二套几何真源；
- child bounds 始终保留在 parent content domain，允许负坐标；
- Runtime 在稳定事务边界统一提交 extent、range、damage 与通知；
- 不使用 listener 对象链维护缓存依赖，依赖关系由 Runtime 根据树关系传播。

## 3. Layer

### 3.1 LayerFigure

`LayerFigure` 是无自身视觉内容的容器 Figure：

- 默认不绘制背景；
- 默认使用 `HitParticipation::DescendantsOnly`，不把自身作为 hit-test 结果；
- descendants 仍按普通规则参与命中；
- children 的 paint 顺序使用 FigureTree 正序；
- hit-test 使用同一 children 列表的逆序；
- visibility、enabled、clip、transform 和 damage 不获得旁路。

`DescendantsOnly` 使通用命中遍历跳过当前节点的 self geometry/accept，但仍在有效
clip 内下降到 children；它不能实现为 `precise_hit = false` 后剪掉整棵子树。
`Figure::hit_participation()` 默认返回 `SelfAndDescendants`，内置 LayerFigure 固定返回
`DescendantsOnly`。需要可命中背景时添加显式 background Figure，或由自定义 layer
类型返回 `SelfAndDescendants`。命中参与方式与 `NodeState.opaque` 解耦：opaque 是
绘制覆盖承诺，不再隐式控制输入语义。

`TreeSearch::prune`、effective visibility/enabled 和坐标可逆性仍先于
`HitParticipation`；`DescendantsOnly` 不能绕过搜索剪枝或有效状态门禁。

Layer capability 是无平台依赖的类型标记，用于 LayeredPane child policy。
`LayerFigure`、`LayeredPane`、`FreeformLayerFigure`、`FreeformLayeredPane` 和
`ScalableFreeformLayeredPane` 均具备该 capability，因此 pane 可以作为另一个 pane
中的 layer。它不增加空 paint/event callback。

Freeform 容器使用显式 `ChildClippingStrategy::OverflowVisible`：

- paint children 时继承进入容器的有效 ancestor clip，但不再与容器自身 client box
  相交；
- parent 绘制该 Freeform child 时不得再追加 child presentation bounds clip；
- hit-test descent 使用完全相同的有效 clip；
- damage 投影也使用同一 clip 链；
- 外层 Viewport 仍以自己的 client box 截断最终可见区域。

若只放宽 paint 或 hit-test 中的一条路径，负坐标内容会出现“看得见但点不到”或
“点得到但不重绘”的错误。普通 LayeredPane 继续使用默认 client clip。

### 3.2 LayerKey

候选公共类型：

```rust
pub struct LayerKey(Arc<str>);
```

要求：

- 可克隆、可比较、可哈希；
- 内容非空，按字符串值相等；
- 不携带 `FigureId`；
- 不使用全局注册表或进程级 intern pool；
- 同一个 LayeredPane 内唯一；
- 不要求不同 LayeredPane 之间全局唯一。

引擎可以提供常用 key 常量，但应用可创建领域专用 key。公开 API 不接受任意
`Any` 作为 key。

### 3.3 LayeredPane

LayeredPane 是带关系元数据的容器协议，不建立第二棵 layer 树：

```text
FigureTree.children(pane) = paint/Z-order 真源
LayeredPaneState.by_key   = LayerKey -> FigureId
LayeredPaneState.by_child = FigureId -> LayerKey
```

`LayeredPaneState` 由 Runtime 按 pane `FigureId` 持有，只保存双向 lookup，不复制
children 顺序。Layer key 是 parent-child 关系身份，不是布局约束，替换 LayoutManager
不得删除或改变 key。

`LayeredPane` 是内置容器 Figure，Runtime 通过短生命周期
`LayeredPaneMut` 提供命名操作：

```rust
runtime.layered_pane(pane_id)?.add_layer(layer, key, placement)
runtime.layered_pane(pane_id)?.remove_layer(key)
runtime.layered_pane(pane_id)?.move_layer(key, placement)
runtime.layered_pane(pane_id)?.layer(key)
runtime.layered_pane(pane_id)?.layer_key(layer)
runtime.layered_pane(pane_id)?.layer_ids()
```

`placement` 支持末尾、指定 key 之前和指定 key 之后。操作必须原子验证：

- pane 具备 LayeredPane 能力；
- child 具备 Layer 能力；
- key 不重复；
- reference key 存在；
- topology 操作合法。

`Before(key)` 插入到目标下方，`After(key)` 插入到目标上方；children 从前到后绘制，
因此后一项视觉上位于前一项之上。

成功后只修改一次 children 顺序和对应 membership。失败时 topology、membership、
validation、damage 和通知均保持不变。remove、reparent 和 pane detach 必须原子清理
双向 lookup。

已是该 pane direct child 的 layer 再次执行 `add_layer` 返回 `AlreadyLayerMember`；
调用方使用 `move_layer` 只改变 Z-order。跨 parent 添加按 Runtime 既有 reparent
语义执行，旧 membership 清理与新 membership 建立属于同一个原子 mutation。

LayeredPane 使用与 Viewport 单 child policy 相同层级的
`ChildPolicy::Layered`。所有 add/reparent 入口都必须执行该 policy；缺少 LayerKey
的泛型 topology 操作必须返回错误，不能绕过 LayeredPane 不变量。

由于 membership 状态属于 Runtime，LayeredPane 写操作必须经过 Runtime mutation
入口。普通场景的 pre-Runtime 批量构建使用显式 FigureTreeBuilder；FigureTree 的底层
add/remove/reparent primitive 只能作为 crate 内部步骤。callback effect 中若请求把
child 移入 LayeredPane，也必须携带 LayerKey 与 placement，否则整项 mutation 被拒绝。

### 3.4 普通与 Freeform LayeredPane

普通 `LayeredPane` 使用 StackLayout 语义：每个 layer 的 bounds 等于 pane client
box，所有 layer 共享相同有限区域。

`FreeformLayeredPane` 同时具备 LayeredPane 与 Freeform capability：

- 每个 layer 的 placement origin 固定为 `(0, 0)`，不使用 `extent.min`；
- 所有 layer extent 通过统一 edge transform 映射到 pane child content domain；
- layer 的 presentation size 由 parent layout 分配，通常覆盖当前 viewport
  envelope，但该尺寸不参与 freeform extent 计算；
- pane extent 只由各 layer 的内容贡献合并；
- 未启用 scale 时 child transform 保持 identity。

FreeformLayeredPane 仍接受任何具备 Layer capability 的 child：freeform layer 贡献其
派生 extent，普通 bounded layer 贡献自身 border-box。

内置 `FreeformLayerFigure` 默认使用 zero insets、identity local/child transform，
因此多个默认 layer 的 child 坐标数值直接对齐。自定义 layer 可以使用 insets 或
transform，但 extent、paint、hit-test、event point 和 damage 都必须映射，调用方
不得假定不同 layer 的 local point 数值天然相同。

Viewport 只查询 direct contents 的 capability，因此 scale 不能通过一个不转发
Freeform capability 的外层 wrapper 接入。`ScalableFreeformLayeredPane` 是同一节点
同时暴露 LayeredPane、Freeform 与 `Figure::scale_model` capability 的便利类型；它复用
共享 `ScaleModel` 和统一 child transform，不保存第二份 scale 或 extent。

## 4. Freeform Extent

### 4.1 坐标域

Freeform capability 只声明节点具有派生内容范围；实际缓存和重算由 Runtime 与
LayoutState 承担，具体 Figure 不通过遍历 children 自行计算。

`freeform_extent(host)` 位于 host 的 child content domain。它描述 host children
占用的逻辑布局范围，不是：

- host 在 parent 中的 `NodeState.bounds`；
- logical surface domain 的 projected bounds；
- 包含阴影或滤镜的 visual bounds；
- viewport 当前可见矩形。

host 自身的 insets 不混入该值；从 child content domain 映射到 host local 或 parent
domain 时，由统一 edge transform 应用 client origin 与 insets。

空 freeform 容器的 extent 固定为：

```text
Rect(0, 0, 0, 0)
```

direct child 的贡献按以下规则计算：

```text
普通 child:
    child layout border-box in host child content domain

freeform child:
    child freeform extent mapped through the complete child-content-to-host-content transform

host extent:
    union(all direct child contributions)
```

与 Draw2D `FreeformHelper` 一致，具备 Freeform capability 的 child 贡献自身派生
extent，而不是 presentation border-box 与 extent 的 union。这样 viewport 为 layer
分配的展示尺寸不会反向污染内容范围或阻止范围收缩。D2 内置 freeform 类型自身不绘制
需要纳入滚动范围的实体；未来带自身视觉的 freeform 类型必须显式提供额外的
`self_extent`，不能隐式复用 NodeState.bounds。

Extent 默认包含所有 attached children，不因 visible 或 enabled 改变。隐藏只影响当前
绘制和命中，不应导致画布范围、滚动位置或 sibling 坐标跳变。未来若布局器需要忽略
隐藏 child，必须作为显式 layout policy，并由 LayoutOutput 改变对应布局几何。

所有 rectangle 映射使用现有 `Affine2D` 协议。非有限 bounds 或 transform 是结构化
错误，不得污染 extent 缓存。公共几何 mutation 应在入口拒绝非有限值；若派生阶段
仍检测到非法值，则中止当前事务并保留旧缓存。不可逆 transform 不阻止正向 extent
投影，但该分支仍按坐标契约不可命中。

### 4.2 单一真源

`NodeState.bounds` 继续是节点几何真源；freeform extent 只是由 topology、bounds、
insets 和 edge transform 派生的缓存结果。

Novadraw 不公开 Draw2D 风格的递归 `setFreeformBounds`。Viewport 消费 extent 后更新
RangeModel 和派生的 viewport envelope，但不得改写普通 descendants bounds、把
`extent.min` 写入 contents bounds，或将负坐标归一化为正坐标。否则 contents placement
与 child 的负坐标会产生双重平移。

Nested freeform 容器仍必须传播 extent 失效并派生各自 envelope。该传播只同步范围，
不把 envelope 写回普通 child 的 `NodeState.bounds`。

### 4.3 缓存与失效

Freeform state 至少保存：

```text
cached_extent
extent_generation
dirty
```

以下变化使最近 freeform ancestor 的 extent 失效，并继续传播到连续的 freeform
ancestor：

- add、remove 或 reparent；
- direct child bounds 变化；
- child placement/local transform 变化；
- nested freeform child 的 extent 变化；
- attach/detach。

纯 reorder 不改变 extent，也不使 extent cache 失效；它只使 Z-order 相关
paint/hit-test 状态失效。若同一操作还改变 membership、bounds 或 transform，再按对应
原因失效 extent。

host 自身 insets 或 child transform 不改变其 child content domain 内的 extent，
但会改变该 extent 向 parent 的投影，因此必须失效上层 freeform ancestor 的缓存。

同一 Runtime 事务内的多次变化合并为一次 extent 重算。只有 old/new extent 不同才
产生 typed `PropertyChange::FreeformExtent { old, new }`，通知在新 extent、
RangeModel 和 damage 已稳定后 flush。它复用现有 property notification，不新增
FreeformListener 或独立 listener 总线。

公开 `freeform_extent(id)` 只返回最近一次稳定事务提交的值。事务内部由 validation
阶段读取正在构造的 generation；查询 API 不通过 interior mutation 临时重算缓存。
unknown ID 与不具备 Freeform capability 必须返回不同错误。

相同输入 generation 中每个 dirty freeform host 合并重复工作；同一 source epoch 内
若 route/layout 改变输入，必须以新 generation 再次入队。Nested freeform 按深度
从深到浅归并，禁止每个 host 各自重新扫描完整后代树而退化为 O(n²)。

## 5. FreeformLayout

`FreeformLayout` 使用 typed rectangle constraint，并保留 child 在 host child content
domain 中的显式位置：

```rust
pub struct FreeformConstraint {
    origin: Point,
    width: Option<f64>,
    height: Option<f64>,
}
```

`origin` 可为负；`None` 表示使用对应轴的 intrinsic size，`Some(0.0)` 是合法显式
尺寸。约束通过校验构造器创建并以只读 accessor 访问，不得使用结构体字面量绕过
校验。不得使用 `-1` 或 zero 作为 fallback sentinel；显式负尺寸和非有限值在构造时
拒绝。

默认策略不平移 children，不因新增更靠左或更靠上的 child 改写已有 child bounds。
这保证持久化坐标、anchor 和事件坐标稳定。

Draw2D 的 `positiveCoordinates` 选项不进入 D2 默认契约。若未来需要导出或兼容只接受
非负坐标的数据格式，应在导入/导出边界显式归一化，而不是改变运行时布局坐标。

布局计算继续遵守 `LayoutSnapshot -> LayoutOutput`：layout 不能重入访问或修改
FigureTree，输出在 Runtime 边界原子提交。

`measure` 必须从 LayoutSnapshot 中的 constraint 和 child measurement 计算
prospective extent，再与 origin point 做 union，最后加 host insets 与 border
preferred size。它不能反向读取尚待本轮 LayoutOutput 提交的 extent cache，否则会
形成旧缓存依赖或 validation 循环。Size 只能表达宽高，不能替代包含负 origin 的
`freeform_extent`；FreeformViewport 必须查询提交后的完整 Rect。

## 6. Viewport 与 Zoom

Freeform contents 的滚动范围来自：

```text
viewport_baseline =
    Rect(0, 0, client_width / scale, client_height / scale)

content_envelope =
    union(freeform_extent, viewport_baseline)
```

因此：

- RangeModel 的 minimum、maximum、extent 和 value 均使用 content domain；
- `content_envelope.x/y` 成为 RangeModel minimum；
- `content_envelope.right/bottom` 成为 RangeModel maximum；
- `client_width / scale`、`client_height / scale` 成为 RangeModel extent；
- contents 小于 viewport 时仍覆盖 viewport client rect；
- baseline 固定在内容坐标原点，不使用当前 RangeModel value，范围因此可以收缩；
- ZoomManager 按既有顺序执行 anchor 捕获、scale 更新、extent validation 和 origin
  clamp；
- origin 变化与 scale 变化在同一 Runtime 事务中提交。

这里只允许 ScalablePane 已定义的有限正 scale。RangeModel 不保存缩放后的第二份
坐标；最终映射继续遵守坐标 SSOT：

```text
viewport_point = (content_point - origin) * scale
```

普通 Viewport 继续使用普通 contents extent。Freeform 规则必须由显式 contents
capability 触发，不能隐式改变所有 Viewport。

Direct contents 同时具备 Freeform 与 ScaleModel capability 时，Viewport 使用
上述 content-domain 公式。仅有外层 scalable wrapper、但未显式转发 Freeform
capability 时，Viewport 不得越过 wrapper 猜测后代 extent。

ViewportLayout 必须声明 child-first validation 依赖：先使 direct contents 的
freeform extent 稳定，再计算本轮 envelope。若范围收缩使 RangeModel value 被 clamp，
layout commit 必须同时提交 `viewLocation` property、coordinate-system change 以及
viewport 和其父容器的 repaint；不能只修改 RangeModel 而遗漏变换失效与 damage。

## 7. 更新、Damage 与通知

Freeform extent 重算进入既有 validation 收敛循环：

```text
capture changed figures' old projected visual bounds and old viewport origin
→ apply topology/property/layout mutations
→ repeat:
     calculate and atomically commit LayoutOutput
     recompute dirty extents bottom-up
     update dependent viewport envelope and RangeModel
     collect newly generated invalidations
  until layout、extent 与 range 同时稳定
→ enqueue changed figures' old/new projected visual damage
→ if RangeModel clamp changed origin, damage the viewport client
→ flush typed notifications
```

extent 变化不能直接触发同步递归 layout。UpdateManager 的收敛保护继续适用；若
FreeformLayout 与 Viewport 相互失效而不能收敛，应返回现有 validation 诊断错误。
Extent rectangle 本身不是视觉内容，单纯 extent 变化不得把整个 old/new extent
加入 damage。

稳定后通知顺序固定为：child geometry/layout 通知 →
`PropertyChange::FreeformExtent` → RangeModel bounds 通知 → clamp 导致的
view-location 通知。所有回调观察到的查询值都必须来自同一个已提交 generation。

## 8. 对现有规范的增量

ADR-004 已接受第 12 节决策，并已同步更新以下现有规范：

- `tree-search-and-focus.md`：把 branch containment、child descent 与 self accept
  分开。顺序调整为 effective state → coordinate inverse → policy-aware branch
  containment → TreeSearch prune → effective child clip → children reverse Z-order
  → self participation/hit/accept。默认 branch containment 仍要求 self hit 与
  client clip；`ChildClippingStrategy::OverflowVisible` 改用进入容器的 ancestor
  clip，只有 Freeform 容器能越过自身 border-box 搜索 descendants。
- `coordinate-system.md`：将 `ChildClippingStrategy::OverflowVisible` 纳入 edge clip
  协议，paint、hit-test 和 damage 必须共享。
- `static-architecture.md`：记录 Runtime 私有的 LayeredPaneState registry，以及
  LayoutState 中的可选 FreeformState；不得把 child IDs 放进具体 Figure；公开拓扑
  写入由 Runtime 统一执行。
- `update-manager.md`：把 dirty freeform extent 和 dependent RangeModel 纳入
  validation 收敛条件。

`HitParticipation`、`LayeredPaneState` 和
`ChildClippingStrategy::OverflowVisible` 是规范契约名称；具体 Rust 模块路径可在
不改变语义的前提下调整。

## 9. 所有权与扩展点

- `LayerFigure`、`FreeformLayerFigure`：具体 Figure 行为；
- `HitParticipation`：Figure 的只读类型行为，默认允许 self 与 descendants；
- `LayeredPane`：Runtime 管理的容器 capability 与命名操作；
- `LayeredPaneState`：Runtime 内按 pane 保存 key/member 双向 lookup，不保存顺序；
- `FreeformState`：`LayoutState` 中的可选派生缓存，不进入具体 Figure；
- `FreeformLayout`：可替换布局策略；
- `PropertyChange::FreeformExtent`：复用现有 typed property notification；
- `ConnectionLayer`：M9 基于 Layer capability 扩展，不反向修改 D2 基础协议。

任何对象都不得长期持有 `&mut FigureTree`、UpdateManager 或 PlatformHost。不得增加
singleton、全局 layer registry 或 app 层 freeform 状态镜像。

## 10. 失败模式

公开操作返回结构化错误，至少区分：

- unknown/stale FigureId；
- target 不是 LayeredPane；
- child 不是 Layer；
- duplicate LayerKey；
- unknown LayerKey；
- already layer member；
- missing placement reference；
- illegal topology mutation；
- invalid/empty LayerKey；
- inconsistent LayeredPaneState（内部不变量错误）；
- wrong layout constraint type；
- non-finite geometry；
- validation 不收敛。

失败必须原子：不得留下只有 key 没有 child、只有 child 没有 membership、部分更新的
extent cache 或提前发送的通知。

## 11. 验证入口

契约测试至少覆盖：

- transparent layer 自身不命中、descendant 可命中；
- paint 正序与 hit-test 逆序使用同一 topology；
- key 唯一性、before/after、remove/reparent/detach 原子性；
- 泛型 Runtime mutation 与 callback effect 不能绕过 LayeredPane child policy；
- LayoutManager 替换不改变 layer membership；
- 正坐标、负坐标、空容器和 nested freeform extent；
- 默认 layer 坐标对齐与自定义 layer transform 映射；
- presentation bounds 变化不污染 freeform extent，内容删除后范围可以收缩；
- add/remove/move/resize/transform/insets 的 extent 失效；
- hidden/disabled child 不改变 extent；
- `ChildClippingStrategy::OverflowVisible` 在 paint、hit-test 和 damage 中使用同一
  有效 clip；
- 普通 LayeredPane 保持默认 client clip；
- 同事务多次变化只发送一次有效通知；
- extent、range bounds、clamped origin 的通知顺序稳定；
- 稳定事务外查询不触发 lazy mutation；
- FreeformLayout 保留负坐标且不移动无关 siblings，显式 zero 不触发 intrinsic fallback；
- viewport 四边可达、范围可扩张也可收缩、origin/range 同帧一致；
- viewport + scale + partial damage；
- extent-only 变化不产生虚假的整块 damage，origin clamp 覆盖完整 viewport；
- 10,000 层深度边界、单 generation 线性重算与大规模 layer 查询基线；
- macOS、Web 和 Headless 组合验证。

## 12. 已接受决策

### 12.1 分类总览

| 决策 | 与 Draw2D 的关系 | 是否完全等价 |
|---|---|---|
| 1. extent 不写回 bounds | 目标等价，API 与可观察几何语义调整 | 否 |
| 2. LayerKey 唯一 | 收窄含糊自由度，增强确定性 | 否 |
| 3. 不提供 positive-coordinates | 明确减少可选功能 | 否 |
| 4. LayeredPaneState 归 Runtime | Rust 所有权实现迁移 | 基本是 |
| 5. FreeformState 归 LayoutState | 缓存与失效实现迁移 | 基本是 |
| 6. OverflowVisible | 目标等价，裁剪机制显式化并加强一致性 | 结果应等价 |
| 7. RangeModel 使用 content domain | 用户效果等价，公开数值表示不同 | 否 |
| 8. Scalable capability 组合 | 继承改组合 | 基本是 |
| 9. Builder/Runtime topology mutation | 收窄修改入口，增强事务语义 | 否 |

“语义收窄”需要区分：

- 删除含糊或可破坏不变量的自由度，如重复 LayerKey、绕过 Runtime 直接写树。这类
  收窄通常提升正确性，但减少底层控制能力。
- 不交付 Draw2D 已有的明确能力，如 positive-coordinates。这类收窄属于真实功能
  减配，必须提供替代路径并记录未来恢复成本。

### 12.2 决策 1：extent 不递归写回 bounds

**Draw2D：**

- `getFreeformExtent()` 从 children 派生范围；
- `setFreeformBounds()` 修改 host bounds，并递归同步 nested FreeformFigure bounds；
- 普通内容 child bounds 不会被递归改写。

**Novadraw：**

- extent 同样从后代几何派生；
- nested freeform 继续传播 extent/envelope；
- envelope 不写回任何 freeform 容器或普通 child 的 `NodeState.bounds`。

**分类：**目标语义等价，但 API 和可观察几何语义发生变化。依赖
`freeform.getBounds() == extent` 的 Draw2D 调用方不能直接迁移。

**收益：**

- bounds 始终只有 parent-local layout geometry 一种含义；
- 避免 extent → bounds → extent 的反馈循环；
- 新增负坐标内容不会隐式移动已有 Figure 或破坏 anchor/reference point；
- layout、hit-test、event point 和 damage 继续共享同一坐标真源。

**代价：**

- 删除公开 `setFreeformBounds`，降低 Draw2D API 兼容度；
- 调用方不能再通过写 host bounds 强制指定 freeform envelope；
- 迁移旧场景时需要适配器把 bounds-based 逻辑改为 extent/envelope 查询。

### 12.3 决策 2：LayerKey 在单 pane 内唯一

**Draw2D：**使用平行的 `List<Object> layerKeys`，技术上允许重复 key；
`getLayer/removeLayer` 操作第一个匹配项。

**Novadraw：**重复 key 返回结构化错误，`LayerKey -> FigureId` 保持一一对应。

**分类：**语义收窄，同时是确定性增强。

**收益：**

- 查询和删除没有“第一个匹配项”的顺序歧义；
- 双向 lookup、原子 reparent 和不变量检查更简单；
- key 可以稳定承担 ConnectionLayer、FeedbackLayer 等语义身份；
- 错误在插入时暴露，而不是在后续查询时表现为错误 layer。

**代价：**

- 不能有意使用同一个逻辑 key 堆叠多个 layer；
- 迁移重复 key 的 Draw2D 场景时必须生成复合 key 或增加子容器；
- key rename 和跨 pane 搬迁必须经过显式事务。

### 12.4 决策 3：不提供运行时 positive-coordinates

**Draw2D：**`FreeformLayout` 默认保留原坐标，但可以开启
`setPositiveCoordinates(true)`，根据最小 child 坐标平移 layout origin。

**Novadraw：**D2 只提供保留原始正负坐标的模式；非负归一化放到导入、导出或算法
适配边界。

**分类：**明确的功能减配。默认行为与 Draw2D 等价，可选模式未交付。

**收益：**

- 新增更靠左或更靠上的 child 不会导致所有既有 Figure 跳动；
- 持久化坐标、Anchor、Router constraint 和事件坐标保持稳定；
- 避免一次插入触发整层 bounds、transform、damage 和通知风暴；
- Runtime 不需要维护“原始坐标”和“正坐标投影”两套状态。

**代价：**

- 只接受非负坐标的导出格式或第三方算法不能直接消费 Runtime 坐标；
- 调用方需要显式计算 normalization transform，并维护结果到原坐标的逆映射；
- 某些只关心正象限的简单应用使用成本提高；
- 未来若恢复该能力，必须设计独立 view/export transform，不能重新引入批量改写
  child bounds。

### 12.5 决策 4：LayeredPaneState 属于 Runtime

**Draw2D：**LayeredPane 对象内部保存 key list，并与自身 children list 同步。

**Novadraw：**FigureTree 保存唯一 children/Z-order，Runtime 私有
`LayeredPaneState` 只保存 key/member 双向 lookup。

**分类：**主要是 Rust 所有权和事务实现手段，外部 add/get/remove/before/after
语义基本不变。

**收益：**

- 具体 Figure 不持有 child IDs；
- topology 与 membership 可以在一个 Runtime mutation 中原子提交；
- LayoutManager 替换不会误删 layer key；
- pane 删除时可统一清理 stale FigureId。

**代价：**

- Runtime 多一份需要与 FigureTree 同步维护的关系状态；
- 所有 topology 写入口都必须经过 Runtime；
- 实现需要内部不变量检查，避免 tree 与 lookup 分叉。

### 12.6 决策 5：FreeformState 属于 LayoutState

**Draw2D：**每个 FreeformFigure 持有 `FreeformHelper`，通过 FigureListener 和
FreeformListener 跟踪 child 变化并缓存 extent。

**Novadraw：**缓存位于 LayoutState，由 Runtime 根据 topology、geometry 和 generation
统一失效并 bottom-up 重算。

**分类：**实现手段迁移。extent 派生与失效目标语义保持，bounds 差异来自决策 1。

**收益：**

- 不建立 listener 对象链和回调重入；
- 相同输入 generation 合并重算；输入改变可在同一 source epoch 再次计算；
- 更容易检测 O(n²) 和 non-converging validation；
- 具体 Figure 继续只保存类型专属行为。

**代价：**

- LayoutState 同时承担布局缓存和 freeform 派生缓存，职责需要明确分区；
- Runtime validation 复杂度增加；
- 脱离 Runtime 单独使用具体 Figure 时无法自行计算 extent。

### 12.7 决策 6：显式 OverflowVisible

**Draw2D：**通常通过 `setFreeformBounds()` 扩大 freeform 容器，使标准 client clip
覆盖所有内容。

**Novadraw：**不扩大 bounds，改由
`ChildClippingStrategy::OverflowVisible` 跳过当前 freeform client clip，并继承最近
有效 ancestor/Viewport clip。

**分类：**目标结果应等价，机制不同；同时增强 paint、hit-test、damage 三条路径的
一致性约束。

**收益：**

- 无需为了裁剪而伪造或扩大 layout bounds；
- 负坐标内容在 paint、hit-test、event point 和 damage 中共享同一规则；
- Viewport 仍是最终有限裁剪边界；
- 自定义 layer transform 不需要改写后代几何。

**代价：**

- clipping strategy 成为核心正确性协议，遗漏任一路径都会产生明显错误；
- 不在 Viewport 下使用 freeform 时，可能绘制到更远的 ancestor/root clip；
- 调试时不能只看 container bounds 判断实际可见区域。

### 12.8 决策 7：RangeModel 使用 content domain

**Draw2D：**FreeformViewport 把 freeform extent 直接写入 RangeModel；在 scalable
pane 下，数值受 Draw2D 缩放后 bounds 和坐标转换语义影响。

**Novadraw：**minimum、maximum、extent 和 value 全部使用未缩放 content domain，
scale 只存在于 ScalablePane：

```text
viewport_point = (content_point - origin) * scale
```

**分类：**视觉和交互目标等价，但公开 RangeModel 数值表示不同。

**收益：**

- RangeModel 与 scale 各自只有一个真源；
- zoom 不需要重写所有 range 值为另一单位；
- anchor zoom、事件点和内容坐标可以直接使用同一 domain；
- 避免 logical surface units 与 content units 混用。

**代价：**

- scale 不为 1 时，RangeModel snapshot 数值不能直接与 Draw2D 比较；
- scrollbar、line/page increment 必须明确执行 content/viewport 单位换算；
- 平台或应用若把 RangeModel value 当屏幕像素使用，需要迁移。

### 12.9 决策 8：ScalableFreeformLayeredPane 使用 capability 组合

**Draw2D：**通过 Java 继承
`FreeformLayeredPane` 并实现 `IScalablePane`，对象自身保存 scale。

**Novadraw：**同一节点组合 LayeredPane、Freeform 和 `Figure::scale_model`
capability，共享可克隆 `ScaleModel`，不通过不透明 wrapper 查找后代 extent，也不
downcast 具体 Scalable pane 类型。

**分类：**继承到组合的实现迁移，目标行为基本等价。

**收益：**

- Viewport 可以直接查询 contents 的 Freeform 与 Scalable capability；
- 不复制 scale、extent 或 child transform；
- 后续可分别替换 layer、freeform 和 zoom 策略。

**代价：**

- capability 组合和具体便利类型之间必须有清晰构造规则；
- 错误 wrapper 若不转发 Freeform capability，会退回普通 Viewport 语义；
- API 类型关系不再与 Draw2D 类继承一一对应。

### 12.10 决策 9：构建期 Builder，运行期 Runtime

**Draw2D：**调用方可以直接执行 `Figure.add/remove`，Figure 自己同步 parent、
listeners、layout 和 UpdateManager。

**Novadraw：**pre-Runtime 批量构建通过 FigureTreeBuilder；进入 Runtime 后公开写入
只能通过 Runtime mutation。FigureTree 保留公开只读查询和 crate-private mutation
primitive，callback 只能记录 effect。

**分类：**修改入口与时序的语义收窄，同时显著增强事务保证。最终稳定树结构可以
等价，但中间状态、重入能力和通知时序不等价。

**收益：**

- topology、LayeredPane membership、extent、interaction cleanup 和 damage 原子提交；
- callback 不会在持有 Figure 借用时重入修改树；
- mutation FIFO 和通知因果顺序可测试；
- 失败不会留下半条 parent/child 关系或 stale key。

**代价：**

- 调用简单 add/remove 的样板代码和概念成本增加；
- callback 修改不会立即在当前调用栈中可见；
- 某些底层批量算法需要专用复合 mutation，不能直接循环操作 FigureTree；
- 运行期写入强制依赖 Runtime；独立批量构建必须显式使用 builder；
- 事务队列和校验会引入少量管理开销。
