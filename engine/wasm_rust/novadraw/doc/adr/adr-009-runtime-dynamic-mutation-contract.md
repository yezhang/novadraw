# ADR-009: Runtime 动态 Mutation 事务

类型：`architecture-decision`

## 状态

已通过

## 背景

Draw2D 允许在运行期替换 `LayoutManager`、设置 child constraint、覆盖
preferred/minimum/maximum size、调整 child index，以及替换 clipping strategy。
这些操作都会进入 Figure 的 revalidate/repaint 语义。

Novadraw 的 `FigureTree` 已具备大部分底层状态操作，但 Runtime 只公开只读
`tree()`。应用若在 Runtime 建立后直接修改 FigureTree，将绕过 UpdateManager、
ConnectionRuntime、资源和交互状态清理；仅把现有底层方法公开也无法保证参数校验、
原子失败和 callback 非重入。

D3.2 因此需要建立统一的 Runtime mutation 事务，而不是开放裸
`&mut FigureTree`。

## 决策

### 1. Runtime 是唯一运行期提交边界

Runtime 事务提供以下 typed、update-aware 操作：

```text
set_layout_manager / clear_layout_manager
set_layout_constraint / remove_layout_constraint
set_preferred_size / clear_preferred_size
set_minimum_size / clear_minimum_size
set_maximum_size / clear_maximum_size
move_child_to_index / bring_child_to_front / send_child_to_back
set_child_clipping_strategy
```

除创建新 Figure 的操作外，成功 mutation 返回 `Ok(true)`，幂等操作返回
`Ok(false)`。参数或结构非法时返回 `RuntimeMutationError`，且不修改树、update
队列、通知队列或 Runtime 派生状态。

构建期 `FigureTreeBuilder` 继续提供低层批量组装入口；它不承担 Runtime
事务语义。

2026-09-10 补充：该清单不是自定义 Figure 的封闭写接口。外部组件采用 ADR-014
的 owned typed update / prepared value，通过统一提交与失效协议更新私有内容。
一般用户 callback 或 Drop 的 panic 不在可恢复参数错误的原子承诺内。

2026-09-28 补充：ADR-019 将上述公开调用面组织为
`Runtime::figure(...)`、`Runtime::container(...)` 等短生命周期 scoped editor。
Runtime 内部 mutation primitive 与本 ADR 的逐操作原子、FIFO、失效和错误语义保持
不变；该调整只分离“提交权威”与“API 命名空间”，不建立第二套写路径。

### 2. LayoutManager 在提交前校验 constraint

`LayoutManager` 增加只读 `validate_constraint` 扩展点。内置 layout manager
按实际支持的 constraint 类型实现该方法；自定义 manager 必须声明接受任意合法
constraint、仅接受指定类型或不接受 constraint。不能把未实现校验默认为验证成功。
兼容适配器只能显式声明宽松策略；完整 LayoutOutput 仍须校验有限性与拓扑。

设置 constraint 时：

1. 校验 child 存在且有直接 parent；
2. 若 parent 当前有 LayoutManager，由 manager 校验 constraint；
3. 校验全部成功后才替换 parent-owned constraint；
4. 使 parent 到 validation root 的路径失效，并排入 UpdateManager；
5. 记录 `ConstraintChanged`。

替换 LayoutManager 时，新 manager 必须先校验 parent 当前保存的全部 constraints。
任一 constraint 不兼容时，旧 manager 和全部 constraints 保持不变。

清除 LayoutManager 不隐式删除 constraints。constraint 仍由 parent 的
`LayoutState` 持有；后续 manager replacement 仍须完整预验证。这是 Novadraw
为 Rust 所有权采用的变体，避免替换策略时静默丢失用户配置。

### 3. Size override 是可清除的节点状态

preferred/minimum/maximum size override 继续存放在 `NodeState`，Runtime 提供
set 与 clear 操作。显式 size 的两个分量必须有限且非负；非法输入原子失败。

size 变化使目标到 validation root 的路径失效并排入 UpdateManager。布局产生的
old/new bounds 继续由既有 Validation -> Damage Repair 路径处理。D3.2 不额外强制
preferred/minimum/maximum 三者的大小关系，以保持 Draw2D 的独立属性语义。

### 4. Child order 使用 parent children 作为唯一真值

普通 child 的 z-order 由 parent `children` 顺序决定：

- index `0` 最先绘制且位于最低层；
- index `len - 1` 最后绘制且位于最高层；
- hit-test 使用相同顺序的逆序；
- index 越界、非直接 child 或未知 Figure 原子失败；
- `LayeredPane` 拒绝普通 child-order API，继续使用 keyed Layer API。

成功重排后 Runtime 请求完整 parent repaint，并使受影响 connection/routing group
失效。构建期与 Runtime 使用同一个底层 reorder primitive，避免两套顺序语义。

D3.2 不新增 `add(child, constraint, index)` 复合入口；新增 Figure 继续使用既有
Runtime add API，原子 indexed add 留待出现真实调用需求后单独设计。

### 5. Clipping 真值进入 NodeState

`NodeState` 保存 Runtime 显式设置的 clipping override。没有 override 时，仍委托
具体 Figure 的 `child_clipping_strategy()`，以保持 Viewport/Scalable 等共享状态
capability 的既有语义；Runtime 调用 `set_child_clipping_strategy` 后，节点 override
成为当前 clipping strategy 的权威值。

Core 1.0 只支持现有三种受控策略：

```text
ClipToChildBounds
DoNotClipChildBounds
OverflowVisible
```

不公开可变 provider，也不在 D3.2 引入任意多矩形 clipping。clipping 变化可能使
旧像素位于新 clip 之外，无法仅用变更后的 parent chain 重建旧 damage，因此成功
替换明确触发 full redraw。这是 correctness contract，不是临时回退。

### 6. Callback mutation 按 FIFO 延迟提交

`EventContext` 为上述操作提供对应的 `*_later` 方法。callback 只把拥有完整参数的
`PendingMutation` 追加到队尾，不取得 `&mut FigureTree`。

顶层 dispatch 释放 Figure 借用后，Runtime 按 FIFO 逐条执行：

- 每条 mutation 都基于前序已提交结果重新校验；
- 每条 mutation 独立原子；
- 某条失败不回滚已提交前缀，也不阻断后续 mutation；
- deferred failure 按发生顺序写入 Runtime error queue；
- `take_deferred_mutation_errors` 在稳定边界返回并清空错误。

该模型保留 callback 因果顺序，同时避免 Figure callback 重入树 mutation。

### 7. 清理误导性的 paint hook

删除公开但不参与真实 traversal 的 `Figure::paint_children` no-op。child traversal
继续只由受保护的递归 renderer 管理，Figure 不能绕过统一坐标、clip 和 paint
顺序协议。

## 错误模型

`RuntimeMutationError` 至少区分：

- unknown/detached Figure；
- parent/child 不是直接关系；
- layered parent 必须使用 Layer API；
- child index 越界；
- 非有限或负 size；
- LayoutManager constraint 校验失败。

错误不得以 `false` 混淆。`Ok(false)` 只表示目标状态已经满足。

## 验证

- 运行期替换和清除 LayoutManager；
- compatible constraint replacement，及 incompatible constraint/layout replacement
  原子失败；
- preferred/minimum/maximum override set/clear；
- 普通 child index、front/back 同步 paint order 与逆序 hit-test；
- LayeredPane 拒绝普通 reorder；
- clipping replacement 修改实际 paint/damage 行为；
- callback 中混合成功、失败 mutation 时保持 FIFO 和已提交前缀；
- deferred errors 顺序可观察；
- Runtime 公共路径与 FigureTreeBuilder 的最终 child order 一致；
- 删除 `Figure::paint_children` 后无实现或调用残留；
- Rust workspace 与 WASM validation 全部门禁通过。

## 后果

### 正面

- Runtime 用户无需访问内部 FigureTree/UpdateManager 即可完成动态布局和树操作；
- constraint 类型错误在状态提交前暴露；
- callback mutation 具有明确的因果、失败和恢复语义；
- clipping 从构造期选项提升为真正的运行期节点状态；
- paint traversal 只保留一个权威入口。

### 代价

- `LayoutManager` 增加一个可选校验扩展点；
- `PendingMutation` 需要持有 layout manager 和 constraint trait object；
- clipping replacement 使用 full redraw，优先保证旧像素清理正确；
- deferred mutation 的调用方需主动提取异步错误。

## 不采用

### Runtime 暴露 `tree_mut()`

不采用。它会绕过 update、resource、connection、interaction 和 notification
事务边界。

### 在 callback 内立即执行 mutation

不采用。Figure 正在被借用，立即写树既违反 Rust 所有权，也会破坏事件传播快照。

### 继续把 clipping strategy 保存在具体 Figure

不采用。构造对象不是树内运行时状态真源，且无法为所有 Figure 提供统一 setter。

### 在 D3.2 引入任意 ClippingStrategy provider

不采用。当前产品只需要三个稳定枚举策略；多矩形或 viewport-aware provider
等待独立需求和 ADR。

## 关系

- 补齐 ADR-003 的 Runtime 所有权边界；
- child-order mutation 必须满足 ADR-008 的 routing group 失效要求；
- 对应 `api_semantics`：
  `figure.tree`、`paint.protocol`、`clipping.strategy`、`layout.manager`、
  `validation.protocol`、`damage.repaint`。

## 日期

2026-09-08
