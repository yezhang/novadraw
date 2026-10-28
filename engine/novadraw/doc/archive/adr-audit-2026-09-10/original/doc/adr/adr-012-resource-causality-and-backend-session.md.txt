# ADR-012: 资源因果日志与 Backend Session

类型：`architecture-decision`

## 状态

已通过

## 背景

长期架构审计复现两个相互独立的问题：

- `ResourceDelta` 将 upsert 与 remove 分装在两个列表中，丢失同一 ResourceId 的真实
  操作顺序；Ready -> Failed -> Ready 可能被 backend 最终解释为删除；
- 已确认的 Ready payload 不会随普通 full redraw 重发，新建或重建 backend 后缺少恢复
  CPU Registry 当前资源状态的协议。

surface resize、submission retry 和 backend/device 重建不是同一种事件：

- resize 只改变输出 surface；
- retry 重放一个未确认 submission 的因果前缀；
- backend 重建丢失全部 backend-local cache，需要新的资源基线和完整场景。

## 决策

### 1. ResourceDelta 使用有序 ResourceOp

增量资源更新使用单一有序操作序列：

```text
ResourceOp::Upsert(ResourceUpdate { id, revision, payload })
ResourceOp::Remove(ResourceId)
```

Runtime 按资源状态转换的实际提交顺序 append，backend 严格顺序应用。`Upsert` 同
revision 幂等，`Remove` 对不存在的 cache entry 幂等。

不在 `ResourceDelta` 内隐式按 ResourceId 归约最终状态。未来若增加归约优化，必须作为
独立、可证明保持命令引用、retry 前缀和 revision 语义的优化。

### 2. retry 恢复精确因果前缀

Runtime 同时只允许一个 in-flight submission。增量 submission 未呈现时，将其原始
`ResourceOp` 序列恢复到当前 pending 序列之前：

```text
failed in-flight ops
-> newer pending ops
```

backend 必须在验证和编码 draw command 的资源引用前，顺序应用本 submission 的全部
resource ops。部分应用后的 Retry 仍然安全，因为 Upsert/Remove 都是幂等操作。

### 3. BackendSessionId 标识 cache 生命周期

`BackendSessionId` 由 Runtime namespace 与单调、非零 generation 组成。每个
`RenderSubmission` 携带 session id。namespace 防止同一 backend 先后消费两个 Runtime
时误复用 cache；generation 用于拒绝同一 Runtime 的旧 submission。Host 在
backend/device cache 建立或重建时调用显式 session reset；surface resize 不创建新 session。

backend 保存当前 session：

- 首次看到 session：建立空 cache；
- 相同 session：消费增量；
- 更大的 session：先清空 backend-local resource/retained-surface cache，再消费；
- 更小的 session：作为 stale submission 拒绝。

### 4. 新 session 首帧使用 Ready snapshot

新 session 在首个可渲染 submission 中发送当前 Registry 所有 Ready 资源的稳定快照，
并同时提交完整场景与 Full damage：

```text
begin/reset backend session
-> freeze current Ready resource snapshot
-> record full stable scene
-> submit ResourceSync::Snapshot + Full damage
-> Presented establishes the session baseline
```

snapshot 只包含 Ready payload；backend 在 session 切换时已经清空旧 cache，因此不需要
为 Pending/Failed/Removed 资源发送 remove。

snapshot in-flight 期间产生的新资源变化进入普通 pending op log。snapshot 失败时不恢复
旧 snapshot，而是保留 `session_sync_pending`，下一次从 Registry 当前状态重新冻结快照；
新快照取代此前尚未确认的资源基线。

每次应用 `ResourceSync::Snapshot` 都必须先清空该 session 的 resource cache，再安装
快照，即使 session id 没有变化。这样 partially-applied snapshot 的 Retry 不会残留已从
新快照中消失的资源。

### 5. session reset 使旧 in-flight 失效

session reset 原子执行：

- 增加 session id；
- 使旧 session 的 in-flight token 失效；
- 标记 Ready snapshot 与 full redraw pending；
- 保留 CPU Registry 当前状态；
- 旧 session 的迟到 completion 不得确认或恢复当前 session 的工作。

旧 in-flight delta 不需要恢复到新 session；新的 Ready snapshot 从 Registry 真值重建
完整基线。

### 6. 异步请求 token 只定义契约

未来异步 loader 的每次请求必须携带 resource-local request token。只有仍匹配当前请求
的完成消息才能进入 Registry，过期完成不得覆盖更新请求。

D4.2 仅固定 token 契约；在 worker/network loader 成为产品路径前，不引入线程池、
解码队列或资源预算实现。

## 后果

### 正面

- Ready -> Failed -> Ready 按 Remove -> Upsert 的真实顺序到达 backend；
- retry 精确恢复未确认的操作前缀；
- 同一 Runtime 可以无损重建一个 backend/device；
- full redraw 不再被误用为资源 cache 恢复协议；
- 新资源类型复用同一 op、snapshot 与 session 语义。

### 代价

- `ResourceDelta`、backend cache adapter 和相关测试需要迁移到 ordered ops；
- `RenderSubmission` 与 completion token 增加 session identity；
- Host/backend 重建路径必须显式通知 Runtime；
- session snapshot 与普通 delta 需要不同的 retry 处理。

## 不采用

### 保留 added/removed 双列表

两个列表无法表达跨列表顺序；改变 backend 的固定消费顺序只能修复一种转换，不能形成
通用因果协议。

### 默认按 ResourceId 归约最终状态

最终态归约在当前单消费者模型中可能成立，但必须证明 command revision、in-flight retry
和未来资源操作均可交换。当前没有必要把该证明和优化引入正确性主线。

### full redraw 时无条件重发全部资源

普通 resize 或 damage promotion 不代表 backend cache 丢失。每次 full redraw 重发资源
会把 surface 与 backend 生命周期混为一谈，并增加无谓 payload。

### backend 自行向 Runtime 拉取资源

这会反转 RenderSubmission 单向边界，使 backend 获得 Runtime/Registry 引用，并增加
重入、生命周期和 Web 平台适配复杂度。

### 同时支持多个 backend consumer

Core 1.0 只要求同一 Runtime 的单 backend 无损重建。多消费者需要独立 ack cursor、
snapshot 和资源保留策略，当前没有产品需求证据。

## 与 Draw2D / Vello 的关系

Draw2D 的 UpdateManager 不定义 GPU 资源同步；SWT Graphics/device 资源由平台对象模型
管理。Novadraw 因 RenderSubmission 与 WebGPU backend cache 分离，必须增加显式 session
与资源同步协议，这是保持平台/引擎隔离的合理变体。

Vello 内部 cache epoch 只管理 Vello 自身编码或 GPU 资源，不替代 Novadraw
ResourceId/revision、retry 或 Runtime/backend session 协议。

## 关系

- 延续 ADR-003 的 Runtime 组合根与单向 RenderSubmission；
- 延续 ADR-007 的字体资源 revision；
- 延续 ADR-011 的 stable frame preparation 与 full redraw 门禁；
- 对应 D4.2 与 `resource.lifecycle`、`render.backend_session`、
  `frame.preparation`、`graphics.context`。

规范细节见
[`../design/architecture/resource-lifecycle.md`](../design/architecture/resource-lifecycle.md)。

## 日期

2026-09-09
