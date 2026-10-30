# ADR-010: Runtime Listener 生命周期

类型：`architecture-decision`

## 状态

已通过

2026-09-10 按 [ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md) 修订；
Runtime/Figure scope 已实现注册时声明与 dispose 清理，历史事件上下文和完整 fault
契约尚待实现验证。

## 背景

Novadraw 的 `UpdateManager` 已分别存储并分发 Update、Figure、Coordinate、
Ancestor、Property、Action 和 Layout listener，所有通知也已通过 effect queue
延迟到稳定事务边界。

但是 Runtime 只暴露 Update、Property 和 Action 的部分注册入口，其中
`add_update_listener` 不返回 `ListenerId`。Figure、Coordinate、Ancestor 和 Layout
listener 对 Runtime 用户不可达，已实现的 M7 内部能力因此尚未形成完整公共面。

此外，listener callback 执行时 `UpdateManager` 正在遍历 listener 集合。允许 callback
直接取得 Runtime 并立即注销会造成重入可变借用；完全禁止 callback 注销又无法支持
一次性订阅等常见生命周期。

## 决策

### 1. Runtime 暴露完整注册面

Runtime 提供以下入口，全部返回带 Runtime namespace 和本地 generation 的 `ListenerId`，
不使用进程全局分配器：

```text
add_update_listener
add_figure_listener
add_coordinate_listener
add_ancestor_listener
add_property_listener
add_action_listener
add_layout_listener
```

`remove_listener(ListenerId)` 统一搜索全部 listener 类别。首次成功移除返回 `true`，
未知或已移除 ID 返回 `false`。

Listener 由 Runtime 统一存储，注册时声明 Runtime 或 Figure(owner) scope；
前者显式注销，后者在 owner dispose 时自动注销。不复制每节点 listener list，
也不能用 payload 过滤或 closure 捕获推断所有权。foreign ListenerId 不得注销本域对象。

### 2. Callback 使用返回值请求自注销

新增：

```text
ListenerDirective::Keep
ListenerDirective::Remove
```

每个 listener 的主 callback 返回 `ListenerDirective`。返回 `Remove` 表示：

1. 当前 callback 已完整执行；
2. 同一 effect 上的其他 listener 仍按注册顺序执行；
3. 当前 listener 在本 effect 分发完成后移除；
4. 当前 flush 中的后续 effect 不再调用该 listener。

该设计只支持 callback 自注销，不支持从 callback 中按 ID 删除其他 listener。
外部代码继续通过 `Runtime::remove_listener` 执行显式注销。

### 3. 分发使用 effect 快照和逐类 retain

一次 flush 先冻结 `NotificationEffect` 快照，再按 effect 顺序分发。每类 listener
集合在处理对应 effect 时使用稳定注册顺序，并根据 callback 返回值原地保留：

```text
drain effects
→ for each effect in FIFO order
  → invoke matching listeners in registration order
  → remove listeners returning Remove
```

listener 只能查询 flush 时最新 stable scene。每条 effect 携带发生时的 revision/epoch、
sequence 和必要 old/new 数据，不能把最新查询视为历史事件发生时的全树状态。
Validating/Painting 表示过去阶段事实，不提供事前拦截。注销只影响后续 effect。

### 4. UpdateListener 聚合回调保持完整

`UpdateListener` 同时接收 notify、FigureEvent 和 UpdateEvent。若
`on_update_event` 对 Validating/Validated 返回 `Remove`，同一 listener 的
`ValidatingListener` adapter 仍完成当前 event 对应 callback，然后再移除。

这保证“当前 effect 完整、后续 effect 停止”的一致语义。

### 5. 不向 listener 暴露 Runtime

Listener callback 不接收 `&mut Runtime`、`&mut FigureTree` 或
`&mut UpdateManager`。需要修改场景的业务继续通过事件输入 callback 的
`EventContext` 和 `PendingMutation` 完成，通知 listener 只观察已提交事实。

## 错误与边界

- `ListenerId` 分配溢出继续视为不可恢复的进程内 invariant violation；
- listener panic 不吞掉；恢复 phase guard 后 Runtime 默认 faulted，不再发布新提交；
  不承诺外部副作用回滚或未送达 journal 自动重放；
- 重复注销稳定返回 `false`；
- self-removal 不产生额外通知 effect；
- 注册新 listener、跨 listener 注销和 listener priority 不进入 D3.3。

## 验证

- 七类 listener 均可通过 Runtime 注册并获得 `ListenerId`；
- 每类 listener 可观察对应的提交后事件；
- `remove_listener` 对全部类别有效，重复注销返回 `false`；
- self-removal listener 收到当前 effect，但不收到同一 flush 的后续 effect；
- self-removal 不跳过其他 listener，也不改变注册顺序；
- listener callback 查询最新 stable scene，历史 old/new 只来自事件 payload；
- owner dispose 清理 Figure-scoped listener，但保留 Runtime-scoped listener；
- foreign ListenerId 不会注销本域订阅，panic 后不能假装 Runtime 可继续呈现；
- workspace、Clippy 和 WASM 门禁通过。

## 后果

### 正面

- M7 的内部 listener 能力形成完整 Runtime 公共面；
- self-removal 不需要全局状态、共享取消标志或 Runtime 重入；
- 当前 effect 与后续 effect 的边界明确且可测试；
- 所有 listener 类别共享相同 ID 和注销协议。

### 代价

- listener callback 返回类型从 `()` 调整为 `ListenerDirective`；
- 现有 listener 实现需要显式返回 `ListenerDirective::Keep`；
- callback 不能直接注销其他 listener。

## 不采用

### Listener callback 接收 `&mut Runtime`

不采用。它会在 UpdateManager 分发期间形成 Runtime 重入，并允许观察或制造部分提交。

### 使用全局取消表

不采用。它违反无全局状态约束，也无法自然限定 Runtime 生命周期。

### 使用共享原子取消 token

不采用。它增加跨线程语义和注册前 token 管理，但本项目 listener 当前只需要
同步事务边界上的确定性 self-removal。

## 关系

- 延续 ADR-002 的 effect queue 与稳定事务边界；
- 补齐 D3.3 和 M7 Runtime public surface；
- 对应 `api_semantics`：
  `notification.figure`、`notification.coordinate`、`notification.ancestor`、
  `notification.property`、`notification.action`、`notification.layout_update`。

## 日期

2026-09-08
