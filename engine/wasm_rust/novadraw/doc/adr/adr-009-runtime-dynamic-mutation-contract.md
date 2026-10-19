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
`Runtime::figure(...)`、`Runtime::container(...)` 等短生命周期 scoped mutable facade。
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

2026-10-02 补充：P2-S01 在 ADR-019 的命名复合操作边界内增加
Builder `insert_child/insert_child_with_constraint` 与 ContainerMut
`insert/insert_with_constraint`。索引、constraint 与新增节点一次预检/发布，
规范见 [动态协议 §5.1](../design/architecture/dynamic-architecture.md#51-原子-child-insertionp2-s01)；
不改变既有 reorder、reparent 或 callback FIFO 语义。

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

2026-10-05 补充：`EventContext::set_bounds_later` 将 parent-content-domain bounds
排入同一 FIFO，提交时复用 `FigureMut::set_bounds` 的 attached/finite/non-negative
校验、old/new damage、Figure moved 通知和 Connection dependency 失效语义。该入口用于
pointer capture 下的轻量 Figure 直接操纵；需要 Command/undo、policy 或 selection 的
编辑仍属于 Editor framework。

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

## K01 事件期样式写入增量

2026-10-19 按用户接受的官方示例 Core 计划实施。`EventSceneQuery::figure_style`
返回当前 committed 局部样式副本；`EventEffects::set_style_later` 记录目标身份和完整
`FigureStyle`，复用 `FigureMut::set_style` 的 Runtime 提交路径。外部实现和内置 Figure
均使用同一入口，无具体 Figure 分支。

`None` 属性恢复父级继承，cursor 清除与 foreground 清除同样适用。事件内查询仍看不到
待提交写入；多个 replacement 按 FIFO 应用。非法身份或非有限 alpha 结构化拒绝，
失败不写样式，也不使健康 Runtime fault；有限 alpha 继续沿既有范围收敛语义。
属性通知、字体失效与 descendant repaint 由现有 update-aware style mutation 处理。

外部消费者证据：`novadraw/tests/p2_deferred_style.rs` 使用第三方 Figure 验证延迟写入、
清除继承、FIFO、foreign 目标、同步/延迟非法 alpha 拒绝；官方 InternalFrame 通过同一
入口更新边缘/角落 resize cursor，并在 pointer exit 清除。本切片不关闭完整 K01
subtree 构造或事件结果 receipt，也不引入 Editor API。

`api_semantics`：`figure.properties`、`event.input_listeners`、`runtime.identity`、
`damage.repaint`。

## K01 完整子树与挂载回执增量

2026-10-19 继续按已批准的 Core 计划实施。`FigureSubtree` 拥有 detached Figure、
局部 style、LayoutManager、父级 constraint 和显式 resource dependencies。
`SubtreeNodeId` 使用描述实例身份与序号，不能作为 `FigureId` 使用。
`FigureMut::add_subtree` 与 `EventEffects::add_subtree_later` 复用同一 Runtime 提交机制。

准入先验证目标，再按父先子后验证 bounds、style、capability、资源、child policy 和
深度，并保留节点身份；最后验证全部布局 constraint。保留节点尚未进入父级 children、UUID lookup、
通知和 damage。验证拒绝或 panic 时清理保留节点；验证器见过的 FigureId 永久退休。
全部通过后发布拓扑，注册全部节点依赖和失效，再按父先子后执行 attachment callback。
callback panic 沿既有边界 fault，不回滚真实发布状态或外部副作用。

资源由 `RESOURCE_DEPENDENCIES` typed capability 和显式依赖共同声明。Label、Image、
Button、Toggle 与外部 Figure 均使用 `FigureResourceDependencies`，不按具体 Figure
类型分支。准入消费本次已构建的 capability registry，不重复注册；外部或已删除的资源
以带节点序号的 `Resource` 错误拒绝，pending 资源允许挂载，重复声明只注册一次。
Graph 只承载构造与发布，资源检查和错误归因由 Runtime 提供，不依赖 Runtime 错误类型。
本增量覆盖完整子树入口；旧单节点、Builder 与 attached 资源更新的统一准入仍需核对。

同步返回 `SubtreeReceipt`；延迟入口回调接收
`Result<SubtreeReceipt, &RuntimeMutationError>`。成功映射在全部 attachment callback
完成后返回；拒绝先写入 deferred mutation errors，再借用同一错误调用回调。即使拒绝
回调 panic，原错误仍可读取。回调没有 Runtime、FigureTree 或同步写权限；外部 model
只在成功后发布节点。receipt 表示 source 挂载成功，不表示布局稳定或帧已呈现。

请求与回调按 FIFO 执行，普通拒绝不阻止后续请求。成功或拒绝回调 panic 时 Runtime
fault，后续请求停止，保留真实已发布拓扑和外部副作用。attachment panic 沿原生命周期
异常通道传播，不伪造完整子树成功回执。

外部消费者证据：`novadraw/tests/p2_subtree.rs` 验证外部 Figure、Label/Border/Layout、
顺序与继承、foreign 构造身份、末项失败、Single child policy、constraint 拒绝与身份
退休、事件内不可见、foreign Runtime、资源、10,000 深度限制和 attachment panic；
同时验证主动消费拒绝、错误队列保留、FIFO 继续提交和两种结果回调 panic。
资源测试覆盖内置图元、复合按钮和外部实现的 foreign/retired 资源，以及 pending 与重复依赖。
官方 TreeExample 使用同一入口挂载 TreeBranch → AnimatingLayer / PageNode → Label，
在成功回调更新 TreeModel，并保留父分支样式。

配对 Connection binding 的准入与发布见下节；K01 重入验收仍待完成。

### 完整 contents 替换

`Runtime::set_contents_subtree` 与 `RuntimeViewports::set_contents_subtree` 接收同一
`FigureSubtree`，返回同一 `SubtreeReceipt`。原单 Figure `set_contents` 也复用整树准入，
保留单项错误的原有类型。`EventEffects::set_contents_subtree_later` 在事件分发结束后
替换 Runtime contents，沿已有 FIFO、结果回调与错误队列提交。
`set_viewport_contents_subtree_later` 使用同一请求与回执机制替换指定 Viewport 的 contents。

替换先验证旧树释放的 connection revision 预算，再预检整棵新树。Single child policy
只排除本次即将替换的直接 child，普通 append 仍拒绝第二个 child。末项 bounds、资源、
capability 或 constraint 拒绝时，新节点不发布，旧树保留。通过后先发布新拓扑与全部
资源、失效和 pane 状态；Runtime 先切换 contents，再释放旧树，最后激活新树。
旧树的 Focus、Animation、Connection、资源依赖和订阅沿原统一释放路径退休。

生命周期 callback panic 会 fault：旧树释放回调 panic 时，新 Runtime contents 已
指定、资源已登记，不返回成功回执；新树 attachment panic 同样保留真实发布状态。
不承诺扩展副作用回滚，也不允许 fault 后继续公共资源写入。

外部消费者证据：`p2_subtree` 验证末项资源拒绝保留旧树、布局与样式映射、旧树先释放、
释放 panic、单 Figure 资源准入，以及事件内不可见和替换后旧目标请求按 FIFO 拒绝。
Viewport 测试证明非法输入不清空旧视图，成功替换后只有一个 contents child；
事件期替换保留外层场景，并按 FIFO 拒绝已退休的旧目标。
内部 `contents_retirement_fault_keeps_new_scene_resource_dependencies` 检查 fault 后
registry 中的新树依赖，公共写入入口继续按 fault 契约拒绝。
此入口提供 graph fixture 场景重建的 Core 基础；尚未移植的 graph 算法不因此关闭差额。

### 构造期 Connection binding

`FigureSubtree::set_connection_binding` 以 scoped `SubtreeNodeId` 标识连接节点，
持有公开 `SubtreeConnectionBinding` payload。外部 struct 或 owned closure 都使用同一
入口，返回 `SubtreeConnection`，不匹配具体 Figure、Anchor 或 Router 类型。
`SubtreeBindingNodes` 只提供保留身份映射，与挂载成功的 `SubtreeReceipt` 分开；
prepare 不获得 Runtime 或可变树，也不能把保留身份当成成功回执发布外部模型。

拓扑发布前先检查所有布局约束，再依次 prepare binding：目标必须提供 Connection
capability；owner-backed Anchor 的主 owner 和 inherited Router 的 layer 必须属于
同一构造描述；显式 Router 必须已在接收 Runtime 注册，constraint 使用原 registry
受检类型协议。此入口的 owned Anchor 不支持引用描述外节点；已有跨树连接仍使用原
attached Connection facade。Anchor 的坐标几何和其他依赖继续在 route query 中受检，
挂载准入不承诺路由一定成功。

全部 binding 通过后，一次检查现有连接的 revision 预算，覆盖批量注册、旧场景释放
和 Runtime contents 的后续失效次数。普通拒绝不写 topology、Anchor registry、
Connection state 或 dirty revision；prepare panic 清理保留节点并 fault。随后先发布
全部拓扑，再注册 owned Anchor 和 Connection state、资源及失效，最后执行 attachment
callback。单节点注册保持原行为；新批量注册仅使原有连接失效一次，不重复运行扩展
constraint validator。同步与延迟 add/contents/viewport replacement 共用这一路径。

构造 binding 创建的 Anchor 由 Connection registry 追踪；最后一个绑定释放后，删除
Anchor 身份并释放 payload。手工注册的 Anchor 保留原显式释放语义。如果用户通过
attached facade 共享构造 Anchor，则一直保留到最后一个 source/target 引用消失。
Router 由接收 Runtime 原 registry 持有，不随子树释放。

外部消费者证据：`p2_subtree` 使用第三方 Anchor、公开 closure payload 和内置 Chopbox
验证配对挂载、结果回执后可查 Connection state、释放 owned Anchor、末项 foreign
Router 拒绝与保留身份退休、错误 capability/owner/constraint 保留旧场景和 prepare
panic。内部测试注入 `u64::MAX - 2` revision，验证 Runtime replacement 的三次预算
在写入前拒绝。ConnectionAndAnchorExample 的两组端点和连接使用同一整树构造入口，
保留已移植的 child 顺序、拖动、locator 与 route 行为；其 GroupBox/EndpointLocator
差额仍由 K02/K03 跟踪，不因此宣称与官方完全对等。

性能边界：身份反查、Anchor owner 检查与未引用 owned Anchor 清理使用 HashMap/
HashSet，避免逐 binding 扫描全部节点或逐 Anchor 扫描全部 Connection。
没有变更渲染主循环，也没有在热路径增加日志。

`api_semantics`：`figure.tree`、`figure.properties`、`layout.manager`、
`runtime.identity`、`validation.protocol`、`damage.repaint`、`connection.figure`、
`connection.anchor`。
