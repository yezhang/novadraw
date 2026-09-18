# 资源因果与 Backend Session

类型：`normative-design`

状态：`approved`

范围：D4.2

## 1. 目标

Runtime 到 RenderBackend 的资源协议必须保证：

- 同一 ResourceId 的状态转换按真实因果顺序到达 backend；
- submission Retry 不丢失、不倒置未确认操作；
- backend/device 重建后，从 CPU Registry 真值恢复全部 Ready 资源；
- surface resize、full damage 和 backend session 生命周期相互独立；
- Figure 继续只持有 typed resource ID，不持有 payload 或 backend 对象。

## 2. 身份

```text
ResourceId
├── runtime namespace
└── slot key (index + generation)

ImageId(ResourceId)
FontId(ResourceId)
```

namespace 隔离不同 Runtime；slot generation 防止删除后复用槽位时旧 handle 指向新资源。
Ready revision 在同一 ResourceId 内单调增加，并进入 ImageResourceRef/FontFaceRef。

```text
BackendSessionId
├── Runtime namespace
└── monotonic non-zero generation
```

BackendSessionId 标识一组 backend-local resource cache 与 retained surface 的生命周期，
namespace 防止复用同一 backend 的两个 Runtime 被误认为同一 session；generation 只在
同一 Runtime namespace 内比较。切换 namespace 无时间顺序，必须先由 Host 停止
旧 producer 并排空/撤销旧提交，再替换 cache；不能仅凭不同 namespace 防止迟到包。
2026-09-10 新接管契约见 ADR-014，尚需 D4.4 验证。

## 3. Registry 状态机

```text
register → Pending
Pending  → Ready(revision)
Pending  → Failed(reason)
Ready    → Ready(revision + 1)
Ready    → Failed(reason)
Failed   → Ready(revision + 1)
any      → Removed
```

- Pending/Failed 使用确定性 fallback；
- Ready 产生 Upsert；
- Ready -> Failed 与 Ready -> Removed 产生 Remove；
- Pending/Failed -> Removed 没有 backend op；
- 依赖已删除资源的 `ImageFigure` 在同一稳定化事务中进入 `Unavailable`，清除旧
  `ImageResourceRef` 且不再生成 Image command；该状态不同于仍可完成的 Pending；
- 同一资源的全部 Figure dependent 必须一起失效和刷新，不能留下引用旧 revision 的
  局部派生快照；
- unknown、namespace mismatch 和 kind mismatch 返回结构化错误；
- worker 不直接修改 Registry、FigureTree 或 backend。

Registry 是 CPU payload 和当前状态的 SSOT。pending op log 与 backend cache 都不是资源
真值。

## 4. 有序 ResourceDelta

```rust
pub enum ResourceOp {
    Upsert(ResourceUpdate),
    Remove(ResourceId),
}

pub struct ResourceDelta {
    ops: Vec<ResourceOp>,
}
```

要求：

- append 顺序等于 Runtime 已提交状态转换顺序；
- 不跨 Upsert/Remove 分桶；
- Upsert 携带 ResourceId、Ready revision 与 Arc payload；
- 同 revision Upsert 幂等；
- Remove 对缺失 cache entry 幂等；
- backend 在处理 draw command 前顺序应用全部 ops；
- backend cache key 至少包含 ResourceId，并保存已应用 revision。

示例：

```text
Ready(1) -> Failed -> Ready(2)
=> Upsert(1), Remove, Upsert(2)
```

若 Ready(1) 已在此前 frame 确认，则当前 pending 只包含：

```text
Remove, Upsert(2)
```

## 5. ResourceSync

RenderSubmission 携带两种互斥资源同步模式：

```rust
pub enum ResourceSync {
    Delta(ResourceDelta),
    Snapshot(ResourceSnapshot),
}

pub struct ResourceSnapshot {
    ready: Vec<ResourceUpdate>,
}
```

Delta 用于同一 backend session 的增量同步。Snapshot 只用于新 session 的基线建立：

- 从 Registry 当前 Ready 状态冻结；
- 按 ResourceId 稳定排序；
- 每个 ResourceId 最多一个最新 Ready revision；
- 不包含 Pending、Failed、Removed；
- backend 每次应用 Snapshot 前都清空当前 resource cache，即使 session id 未变化；
- 空 Snapshot 仍是 cache replacement，不得被当作无操作跳过；
- 必须与完整 scene commands 和 Full damage 同一 submission。

## 6. Backend session

### 6.1 初始与重建

Runtime 初始 session id 为非零值，并将 `session_sync_pending` 置为 true。Host 在以下事件
调用 `begin_backend_session`/`reset_backend_session`：

- 首次建立 RenderBackend；
- WebGPU device/queue 丢失后重建；
- 切换为新的 backend 实例；
- backend 明确报告其 retained/resource cache 已全部丢失。

以下事件不创建新 session：

- surface resize；
- scale factor 改变；
- partial damage 提升为 full damage；
- 普通 submission Retry；
- swapchain/surface 暂停后恢复，但 backend cache 仍有效。

### 6.2 Submission

```rust
pub struct RenderSubmission {
    session_id: BackendSessionId,
    frame_id: FrameId,
    resources: ResourceSync,
    commands: CommandStream,
    damage: Damage,
    surface: SurfaceInfo,
}
```

backend 处理规则：

```text
no active session
  -> require Snapshot + Full, initialize cache baseline
same session
  -> consume normally
same namespace, newer generation
  -> require Snapshot + Full, clear resource cache + retained surface
same namespace, older generation
  -> reject as stale
different namespace
  -> Host serialized handoff, require Snapshot + Full
```

同一 Runtime 仍只允许一个 in-flight submission。

Host 不允许旧 Runtime 在 handoff 后继续投递；切回原 Runtime 也 reset session 并发送
Snapshot。并发 handoff 需要额外的 activation token，当前 Core 不提供该能力。
缺 snapshot 基线时返回结构化错误，不先破坏当前可用 cache。

Backend 实现通过共享 `BackendSessionGate` 执行上述接收规则。首次 session、同
namespace 新 generation 或不同 namespace 接管若携带 Delta，返回
`RejectMissingSnapshot`，并保持原 active session 不变；同 namespace 旧 generation
返回 `RejectStale`。Vello 与 Web Canvas2D 使用同一 gate。

## 7. Freeze、ack 与 retry

### 7.1 Delta submission

prepare：

```text
freeze pending ResourceOp prefix
-> move prefix into in-flight
-> leave later mutations in pending log
```

Presented：

```text
discard in-flight prefix
```

Retry/Failed：

```text
restore exact in-flight prefix before newer pending ops
```

### 7.2 Snapshot submission

prepare：

```text
freeze current Ready snapshot
-> drain pending ops already represented by that snapshot
-> mark snapshot in-flight
```

Presented：

```text
session_sync_pending = false
-> later pending ops remain incremental
```

Retry/Failed：

```text
session_sync_pending remains true
-> do not restore old snapshot
-> next prepare freezes a fresh snapshot from current Registry
```

snapshot in-flight 后产生的新状态转换仍进入 pending log。新 snapshot 会覆盖这些操作的
最终 Registry 状态，因此准备 snapshot 时可一并 drain 已被新快照表示的 pending ops。
重新应用 Snapshot 时先清空 cache，避免此前 partially-applied snapshot 中已不再 Ready
的资源残留。

## 8. Session reset 与迟到 completion

session reset 原子执行：

```text
invalidate old in-flight token
-> increment BackendSessionId
-> session_sync_pending = true
-> full_redraw_pending = true
-> keep Registry state and CPU payloads
```

旧 session 的 in-flight delta 不恢复，因为新 snapshot 从 Registry 当前状态重建基线。
completion 必须同时匹配 `(session_id, frame_id)`；旧 session 的迟到 completion 返回
stale/false，不得确认、恢复或清除当前 session 工作。

session id 溢出返回结构化 `BackendSessionExhausted`，不能 wrap 后误接收旧 submission。

## 9. Backend 应用原子性

ResourceOp 对 backend cache 的重复应用必须幂等。backend 可逐项更新 cache，但只有在：

```text
resource sync applied
-> all command resource references validated
-> frame encoding accepted
```

后才能报告 Presented。若中途返回 Retry，Runtime 重放相同因果前缀。

永久 payload/command 不支持属于 D4.4 的不可恢复错误，不得无限伪装成 Retry。

## 10. 依赖 Figure

ResourceRegistry 维护：

```text
ResourceId -> dependent FigureId set
```

资源 Ready、Failed、更新或 Removed 时，仍附着的依赖 Figure 进入统一派生状态 worklist：

- image natural dimensions 可能改变 intrinsic layout；
- font fallback/metrics 可能改变 intrinsic text；
- presentation snapshot 必须匹配新的 resource revision；
- damage 在稳定派生状态后记录。

依赖 Figure 删除后，Runtime 在稳定事务边界清理失效引用。

## 11. 异步加载 token

未来平台 loader 使用：

```text
begin_load(ResourceId) -> ResourceRequestToken
complete(token, payload)
fail(token, reason)
```

每个 ResourceId 只有最新 token 可提交结果。旧 token 返回 `StaleResourceCompletion`，
不得改变 Registry revision、pending op log 或依赖 Figure。

D4.2 不实现线程池、Web Worker、网络 loader、解码尺寸或内存预算；这些在真实异步产品
路径出现前保持契约。

## 12. 扩展点

### 新资源类型

新增 payload variant、Registry kind 和 backend Upsert adapter；ResourceOp、snapshot、
session 与 retry 协议不变。

### 新 RenderBackend

维护自己的 active BackendSessionId 和 resource cache；只消费 RenderSubmission，不向
Runtime 拉取 payload。

### 多 backend

不在 Core 1.0 范围。未来需要 per-consumer session、ack cursor 与 payload retention，
不能复用当前单 in-flight 状态冒充多消费者协议。

## 13. 验证

- Ready -> Failed -> Ready 的 op 顺序为 Upsert -> Remove -> Upsert；
- backend 顺序应用后保留最新 Ready revision；
- delta Retry 恢复到 newer pending ops 之前；
- Presented 后已确认 ops 不重复提交；
- 新 backend session 首帧包含全部 Ready payload、完整 commands 与 Full damage；
- session snapshot in-flight 期间的新 mutation 不丢失；
- snapshot Retry 使用 Registry 当前状态重新冻结；
- reset 时旧 in-flight 不污染新 session；
- 旧 `(session_id, frame_id)` completion 被拒绝；
- surface resize 不创建 session 或重发 Ready snapshot；
- session reset 后缺失资源 command 不会进入 Presented；
- 不同 Runtime 的 ResourceId 与 BackendSessionId 不互相解释。

## 14. 非目标

- 多 backend 同时消费；
- 跨进程或持久化资源协议；
- 固定二进制 DisplayList ABI；
- worker/thread pool 实现；
- 解码、GPU cache 或 CPU payload 内存预算；
- D4.4 的完整 backend capability 与永久错误分类。
