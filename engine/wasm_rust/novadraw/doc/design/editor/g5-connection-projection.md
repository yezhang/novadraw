# G5.1 Connection Projection 契约提案

类型：`proposal`

状态：`review_required`

适用范围：G5 的模型连接发现、ConnectionPart 投影、关系索引、Connection Runtime
绑定与生命周期。连接创建、重连、bendpoint 和 auto-expose 只定义依赖边界，不在
G5.1 实现。

外部事实见
[`../../reference/gef/connection-editing.md`](../../reference/gef/connection-editing.md)；
Editor 总体约束见 [`architecture.md`](architecture.md)；底层连接契约见
[`../architecture/connection-routing.md`](../architecture/connection-routing.md)。

## 1. 目标

G5.1 建立从应用连接模型到可路由 Connection Figure 的稳定投影：

```text
Model source/target discovery
-> validated connection snapshot
-> one ConnectionPart per connection ModelId
-> source/target relation indexes
-> Connection Figure in connection layer
-> Runtime anchor/router binding
-> route, damage, hit-test
```

完成后，Viewer 能加载并增量刷新已有连接，但尚不提供用户创建、重连或编辑折点的
手势。

## 2. 非目标

- 不把 connection 当作普通模型 child；
- 不让 ConnectionPart 或 Figure 修改业务模型；
- 不复制 GEF 的 Java 对象地址身份或 `Object` request payload；
- 不在 app 层直接操作 FigureTree 或 ConnectionRuntime；
- 不在 G5.1 引入 connection-to-connection、connection child label 或
  ShortestPath router；
- 不提前实现 G5.2 之后的 create/reconnect/bendpoint/auto-expose。

## 3. 核心不变量

1. 一个 Viewer 内，一个 connection ModelId 最多对应一个 live ConnectionPart。
2. 每个已提交连接恰有一个 source Part 和一个 target Part；self-loop 合法。
3. source/target 关系不是 containment，不能出现在普通 `children()` 结果中。
4. Connection Figure 只挂到 root 的 connection layer。
5. ConnectionPart、Figure、Runtime connection state 和两个 endpoint anchor
   必须共同创建、共同退休。
6. Command 只保存 ModelId 和业务数据，不保存 EditPartId、FigureId、ConnectionId
   或 AnchorId。
7. 同一稳定模型 revision 的连接快照必须先完整校验，再改变 Viewer/Runtime。
8. endpoint 改变但 connection ModelId 不变时，保留 ConnectionPartId 和 FigureId，
   只替换关系和 Runtime binding。
9. 连接删除由模型事实和 Command 决定，不能由节点 Figure dispose 推断业务级联。

## 4. 模型发现契约

在 `ModelAdapter` 上增加带默认空实现的查询：

```rust
fn source_connections(
    &self,
    model: Self::ModelId,
) -> Result<Vec<Self::ModelId>, Self::Error>;

fn target_connections(
    &self,
    model: Self::ModelId,
) -> Result<Vec<Self::ModelId>, Self::Error>;
```

默认返回空列表，以保持不使用连接的应用兼容。列表顺序必须稳定，元素是 connection
ModelId。每次 Viewer refresh 在 containment 更新完成后，对全部 active containment
parts 构造一个稳定连接快照。

快照规则：

- 同一节点同一方向内禁止重复 connection ModelId；
- 每个 connection ModelId 必须在全局恰好出现一次 source 和一次 target；
- source/target 必须解析到当前 Viewer 的 active containment Part；
- connection ModelId 不得同时作为 containment model 注册；
- source 与 target 可以是同一 Part；
- connection layer 的确定顺序使用 source Part 的 containment preorder，再使用
  source list 内顺序，不依赖 HashMap 迭代或 target refresh 先后。

应用可以从集中式 edge store 计算这两个列表，不要求业务模型实际保存双向数组。

## 5. Part 身份与关系

ConnectionPart 与普通 Part 共享 `EditPartId` namespace、model registry、visual
registry、selection、focus、policy 和 lifecycle 协议，但不共享 containment 边。

`PartTree` 增加独立关系：

```text
connection: ConnectionPartId -> { source: EditPartId, target: EditPartId }
outgoing: NodePartId -> ordered Vec<ConnectionPartId>
incoming: NodePartId -> ordered Vec<ConnectionPartId>
```

`ConnectionPartId` 首版继续使用 `EditPartId`，不增加第二套 controller identity。
公开查询必须区分 containment 与 connection relation：

- `parent/children` 只返回 containment；
- `connection_endpoints` 只接受 ConnectionPart；
- `source_connections/target_connections` 只接受 endpoint Part；
- foreign、retired 或错误 kind 均返回结构化错误。

内部节点需要显式 `PartKind::{Root, Containment, Connection}`。禁止通过“是否有 parent”
或 Figure 类型猜测 kind，也不允许把 connection 塞入 synthetic root 的 children
来绕过关系建模。

## 6. Factory 与 Anchor 扩展点

Factory 增加显式 connection 创建入口；默认可委托现有 model-to-behavior factory，
但 Viewer 始终携带 `Connection` 创建上下文，不能依靠 downcast 判断：

```text
PartCreationContext::Containment { parent }
PartCreationContext::Connection { source, target }
```

端点 Part 提供可选 anchor capability。输入至少包含：

- endpoint Part host；
- connection ModelId；
- `ConnectionEnd::{Source, Target}`；
- endpoint primary Figure；
- 当前稳定模型只读引用。

未提供自定义 anchor 时使用 endpoint primary Figure 的 ChopboxAnchor。anchor 对象由
Runtime 注册，Viewer 只保存返回的 AnchorId，并负责随 ConnectionPart 生命周期释放。

G5.1 使用 connection layer 的 inherited router，不增加应用 router hook。Bendpoint
constraint 与显式 router binding 在后续 G5 切片加入，避免一次固定过多 API。

## 7. 投影事务

每次 refresh 的顺序固定为：

1. drain 并验证 model revision；
2. 完成 containment create/remove/reparent；
3. 枚举所有 active containment parts 的 source/target connections；
4. 构造、去重并完整验证 connection snapshot；
5. 计算 remove、retain/rebind、create 和 reorder plan；
6. 清理待删除 connection 的 feedback、selection 和 focus；
7. 解除并删除旧 Runtime connection state、anchors 和 Figure；
8. 对保留项原子更新 endpoint indexes 与 Runtime bindings；
9. 创建新 ConnectionPart，注册 model/visual，挂入 connection layer；
10. 注册 anchors 和 connection state，使用 inherited router；
11. 激活 policy/subscription，统一解析 route；
12. 提交新的 Viewer revision。

所有扩展回调和可失败查询必须在可行范围内前置。计划阶段失败不得改变 Viewer；
提交阶段若发生无法补偿的 Runtime/扩展失败，Viewer 进入 faulted，拒绝后续编辑。

## 8. 生命周期

创建顺序：

```text
factory creates connection behavior
-> allocate ConnectionPartId
-> create Connection Figure
-> register model and visual
-> install source/target relations
-> attach Figure to connection layer
-> register endpoint anchors
-> register Runtime connection state
-> activate policies/subscriptions
-> resolve initial route
```

删除使用严格逆序，并先清理交互状态。若 endpoint Part 被删除，而稳定模型快照仍引用
它，refresh 必须报告 dangling endpoint 并 fault，不能静默删除业务连接。合法的节点
删除 Command 必须在同一模型事务中删除或重连关联 connection。

Viewer drop 必须先退休全部 ConnectionPart，再退休 containment Part，以保证 anchor
owner 和 layer 在 connection cleanup 期间仍有效。

## 9. 增量刷新

- connection 新增：创建一个 Part 和一套 Runtime binding；
- connection 删除：完整退休，不保留旧运行时身份；
- endpoint 改变：保留 Part/Figure 身份，原子替换一端关系和 anchor；
- endpoint geometry 改变：由现有 ConnectionRuntime dependency invalidation reroute；
- source list 重排：只改变 connection layer 顺序；
- target list 重排：只改变 incoming 查询顺序，不改变视觉 Z-order；
- connection visual 属性改变：调用 ConnectionPart behavior refresh，不重建 Part。

由于 `ModelAdapter::Event` 对框架不透明，首版在每个已接受 notification batch 后执行
一次全局 connection reconciliation。正确性优先于局部提示优化；只有基准证明该扫描
成为瓶颈后，才考虑增加可选 typed change hints。

## 10. 错误模型

至少区分：

- duplicate connection in one endpoint list；
- missing source 或 missing target；
- multiple sources 或 multiple targets；
- endpoint 指向 foreign、retired 或非 containment Part；
- connection/containment model identity collision；
- factory 返回非 Connection Figure；
- anchor owner 无效或注册失败；
- connection layer 缺失；
- Runtime binding、route 或 cleanup 失败；
- extension panic。

查询错误、重复和悬空端点必须包含 connection ModelId 与相关 endpoint ModelId 的可诊断
表示，但公共错误不泄露内部 SlotMap key。

## 11. 与 GEF 的对应与差异

保留 GEF：

- source/target 两侧发现；
- Viewer registry 按模型身份去重；
- ConnectionPart 独立于 containment；
- connection layer 挂载；
- endpoint Part 提供 anchor；
- 生命周期与端点关系联动。

Novadraw 差异：

- 不允许刷新顺序产生可观察的半连接状态；
- 先构造全局稳定快照，再原子协调；
- 使用 namespaced generational identity，不使用对象地址；
- 连接关系使用显式索引，不复用不对称 parent 指针；
- route 和依赖由现有 ConnectionRuntime 原子提交；
- 确定性视觉顺序由 source preorder 和 source-list order 定义。

## 12. 验证门禁

新增 `g5_connection_projection_contract.rs`，至少覆盖：

1. 初始 source/target discovery 只创建一个 ConnectionPart；
2. source-first 与 target-first 枚举得到相同结果；
3. self-loop 只创建一个 Part，并同时进入 incoming/outgoing；
4. connection layer 顺序稳定且不污染 containment children；
5. 增量新增、删除与重排；
6. reconnect 保留 Part/Figure identity 并替换 Runtime endpoint；
7. endpoint geometry 变化触发现有 Runtime reroute；
8. duplicate、missing、multiple endpoint 在提交前拒绝；
9. containment/connection ModelId 冲突拒绝；
10. 节点删除未处理关联 connection 时 Viewer fault；
11. ConnectionPart 删除清理 selection、focus、policy、anchors 和 Runtime state；
12. Viewer drop 使用 connection-first 顺序且旧身份全部失效；
13. factory/anchor/Runtime 失败不留下半注册对象；
14. workspace fmt/check/clippy/test。

通过上述门禁后：

- `part.tree`、`part.refresh`、`connection.part` 可提升到 `verified`；
- `interaction.delete` 仅在节点删除级联的模型 Command 用例通过后提升；
- `connection.create`、`connection.reconnect`、`viewport.autoexpose` 保持
  `specified`，进入后续 G5 切片。

## 13. 后续切片

```text
G5.1 existing connection projection
-> G5.2 create connection request/tool/policy/command
-> G5.3 reconnect + endpoint handles
-> G5.4 bendpoint handles and constraints
-> G5.5 viewport/zoom feedback + drag auto-expose
-> Checkpoint C
```

G5.1 不开放人工检查点；检查点 C 只在完整 G5 自动门禁通过后开放。
