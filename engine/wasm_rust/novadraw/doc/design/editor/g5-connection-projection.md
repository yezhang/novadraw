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
Model ordered connection snapshot
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

## 4. 模型快照契约

在 `ModelAdapter` 上增加有序连接描述：

```rust
pub struct ModelConnection<I> {
    id: I,
    source: I,
    target: I,
}

fn connections(
    &self,
) -> Result<Vec<ModelConnection<Self::ModelId>>, Self::Error>;
```

默认返回空列表，以保持不使用连接的应用兼容。返回顺序是 connection layer 的规范
Z-order；`source` 与 `target` 是 endpoint model identity。Viewer 从这一个事实源派生
outgoing/incoming 索引，不要求应用同时维护两份互为镜像的端点列表。

快照规则：

- connection ModelId 在列表中必须唯一；
- source/target 必须解析到同一快照中的 containment model；
- connection ModelId 不得同时作为 containment model 注册；
- source 与 target 可以是同一 Part；
- endpoint 变化本身不隐式改变顺序；只有 adapter 返回列表的位置变化才改变 Z-order；
- 同一模型快照重新加载必须产生相同的 connection layer 顺序。

这保留 GEF 的 source/target 关系语义，但不复制其“两侧分别枚举、依靠刷新顺序汇合”
机制。Novadraw 的 `ModelAdapter` 已代表整个应用模型，直接返回有序完整描述可消除
重复、遗漏和两侧顺序冲突，并保证保存加载后的视觉顺序一致。

## 5. Part 身份与关系

ConnectionPart 与普通 Part 共享 `EditPartId` namespace、model registry、visual
registry、selection、focus、policy 和 lifecycle 协议，但不共享 containment 边。

`PartTree` 增加独立关系：

```text
connection: ConnectionPartId -> { source: EditPartId, target: EditPartId }
outgoing: EditPartId -> ordered Vec<ConnectionPartId>
incoming: EditPartId -> ordered Vec<ConnectionPartId>
```

`EditPartId` 是唯一 controller identity。`ConnectionPartId` 是其透明、受检的角色
包装，不分配新 key 或 namespace：

```rust
pub struct ConnectionPartId(EditPartId);
```

包装值只能由 `PartTree::as_connection(EditPartId)` 在验证
`PartKind::Connection` 后产生，并可无损投影回通用 `EditPartId`。selection 和通用
registry 保存 `EditPartId`；connection 专用 API 接受 `ConnectionPartId`，在编译期
阻止普通 containment Part 被误传。

公开查询必须区分 containment 与 connection relation：

- `parent/children` 只返回 containment；
- `connection_endpoints` 只接受 ConnectionPart；
- `source_connections/target_connections` 只接受 endpoint Part；
- foreign、retired 或错误 kind 均返回结构化错误。

内部节点需要显式 `PartKind::{Root, Containment, Connection}`。禁止通过“是否有 parent”
或 Figure 类型猜测 kind，也不允许把 connection 塞入 synthetic root 的 children
来绕过关系建模。

## 6. Factory 与 Anchor 边界

Factory 增加显式 connection 创建入口；默认可委托现有 model-to-behavior factory，
但 Viewer 始终携带 `Connection` 创建上下文，不能依靠 downcast 判断：

```text
PartCreationContext::Containment { parent }
PartCreationContext::Connection { source, target }
```

G5.1 只使用 endpoint primary Figure 的 ChopboxAnchor。anchor 对象由 Runtime 注册，
Viewer 保存返回的 AnchorId，并负责随 ConnectionPart 生命周期释放。endpoint 未变化
时保留原 AnchorId；只替换发生变化的一端。

自定义 port/anchor 可能在 endpoint ModelId 不变时因连接属性变化而改变。若现在只暴露
返回 `Box<dyn ConnectionAnchor>` 的工厂，就无法可靠判断 anchor 是否仍等价，并会迫使
每次 refresh 重建 AnchorId。因此 G5.1 不提前稳定该扩展 API；G5.3 在真实 reconnect
与 port 用例下定义带稳定 value key 的 endpoint anchor descriptor。

G5.1 使用 connection layer 的 inherited router，不增加应用 router hook。Bendpoint
constraint 与显式 router binding 在后续 G5 切片加入，避免一次固定过多 API。

## 7. 投影事务

每次 refresh 的顺序固定为：

1. drain 并验证 model revision；
2. 构造 desired containment snapshot；
3. 读取有序 connection snapshot，并在 desired containment 上解析 endpoint；
4. 再次确认 snapshot revision 未漂移；
5. 完整校验后计算 containment 与 connection 的联合变更计划；
6. 清理受影响 connection 的 feedback、selection 和 focus；
7. 先解除将删除或 rebind 的旧 Runtime connection state 与 endpoint anchor；
8. 删除不再存在的 ConnectionPart；
9. 应用 containment remove/reparent/create/reorder；
10. 对保留项更新 endpoint indexes，并仅重建变化端的 anchor；
11. 创建新 ConnectionPart，注册 model/visual，挂入 connection layer；
12. 注册 Runtime connection state，使用 inherited router；
13. 激活 policy/subscription，统一解析 route；
14. 按模型列表顺序同步 connection layer；
15. 提交新的 Viewer revision。

所有扩展回调和可失败查询必须在可行范围内前置。计划阶段失败不得改变 Viewer；
提交阶段若发生无法补偿的 Runtime/扩展失败，Viewer 进入 faulted，拒绝后续编辑。

不能先完成 containment 删除再协调 connection。节点和关联 connection 在同一模型
事务中删除时，旧 connection 必须先释放 Runtime binding 和 anchor，随后 endpoint
Part 才能退休；reconnect 到新节点时则先解除旧端、应用 containment 计划，再绑定
desired endpoint。

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
- connection snapshot 重排：同步 connection layer、outgoing 和 incoming 查询顺序；
- connection visual 属性改变：调用 ConnectionPart behavior refresh，不重建 Part。

由于 `ModelAdapter::Event` 对框架不透明，首版在每个已接受 notification batch 后执行
一次 O(V + E) 的全局 connection reconciliation，并刷新 retained ConnectionPart 的
模型视觉。正确性优先于局部提示优化；只有基准证明该扫描成为瓶颈后，才考虑增加
可选 typed change hints。

## 10. 错误模型

至少区分：

- duplicate connection ModelId；
- missing source 或 missing target model；
- endpoint 指向未进入 desired containment snapshot 的 model；
- connection/containment model identity collision；
- factory 返回非 Connection Figure；
- 默认 anchor owner 无效或注册失败；
- connection layer 缺失；
- Runtime binding 或 cleanup 失败；
- extension panic。

查询错误、重复和悬空端点必须包含 connection ModelId 与相关 endpoint ModelId 的可诊断
表示，但公共错误不泄露内部 SlotMap key。

错误分级：

- adapter 查询失败、revision 漂移和快照结构不一致发生在计划阶段，不修改投影；
- revision 漂移、稳定模型无法投影、注册失败、cleanup 失败或扩展 panic 使 Viewer
  faulted；
- ConnectionRuntime 返回的 `UnresolvedConnection` 是可恢复路由状态，保留
  ConnectionPart 和 dependency observation，不升级为 Viewer fault；
- 非有限 route 等违反底层连接契约的错误按 Runtime 既有错误分类处理。

## 11. 与 GEF 的对应与差异

保留 GEF：

- source/target 作为独立关系；
- Viewer registry 按 connection 模型身份去重；
- ConnectionPart 独立于 containment；
- connection layer 挂载；
- anchor 归属 endpoint 语义，首个切片使用标准 Chopbox fallback；
- 生命周期与端点关系联动。

Novadraw 差异：

- 不允许刷新顺序产生可观察的半连接状态；
- 先构造全局稳定快照，再原子协调；
- 用单一有序 connection snapshot 代替两侧重复枚举；
- 使用 namespaced generational identity，不使用对象地址；
- `ConnectionPartId` 只是 `EditPartId` 的受检角色包装，不建立第二身份域；
- 连接关系使用显式索引，不复用不对称 parent 指针；
- G5.1 只固定 Chopbox fallback，不提前稳定缺少 value identity 的自定义 anchor API；
- route 和依赖由现有 ConnectionRuntime 原子提交；
- 确定性视觉顺序由模型 connection snapshot 显式定义。

## 12. 验证门禁

新增 `g5_connection_projection_contract.rs`，至少覆盖：

1. 初始有序 connection snapshot 为每个模型只创建一个 ConnectionPart；
2. `ConnectionPartId` 与 `EditPartId` 共用身份，但拒绝错误 PartKind；
3. self-loop 只创建一个 Part，并同时进入 incoming/outgoing；
4. connection layer 顺序稳定且不污染 containment children；
5. 增量新增、删除与重排；
6. reconnect 保留 Part/Figure identity、列表位置和未变化端 AnchorId；
7. endpoint geometry 变化触发现有 Runtime reroute；
8. duplicate connection、missing endpoint 和 revision drift 在提交前拒绝；
9. containment/connection ModelId 冲突拒绝；
10. 节点与关联 connection 同批删除时执行 connection-first cleanup；
11. 节点删除未处理关联 connection 时 Viewer fault；
12. ConnectionPart 删除清理 selection、focus、policy、anchors 和 Runtime state；
13. unresolved route 保留可恢复 ConnectionPart，不使 Viewer fault；
14. Viewer drop 使用 connection-first 顺序且旧身份全部失效；
15. factory/anchor/Runtime 失败不留下半注册对象；
16. workspace fmt/check/clippy/test。

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
