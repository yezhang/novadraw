# ADR-029：结构化 RenderBackend 失败与恢复确认

类型：`architecture-decision`

状态：`accepted`

日期：2026-10-08

## 背景

`RenderBackend::submit` 目前返回扁平 `RenderOutcome`。`Presented`、`Skipped` 和
`Retry` 与 `Unsupported`、`InvalidGraphicsInput` 混在同一枚举中，且无法表达
surface lost、device lost、out of memory 或 backend 编码失败。Vello 因而在
`render_to_texture` 上使用 `expect`，Host 也无法可靠判断应请求下一帧、重配 surface、
重建 backend 还是终止当前渲染会话。

ADR-012 已规定 backend 重建后必须递增 session，并以 `Snapshot + Full` 恢复基线。
本决策补齐提交失败到该恢复协议之间的结构化边界。

## 决策

### 1. 成功处置与失败正交

规范提交签名为：

```text
RenderResult = Result<RenderDisposition, RenderFailure>

RenderDisposition = Presented | Skipped | Retry
RenderFailure =
    Unsupported
  | InvalidInput
  | SurfaceLost
  | DeviceLost
  | OutOfMemory
  | Fatal
```

`Skipped` 表示当前 surface 不可见或暂停，本帧未发布；`Retry` 表示同一 backend 与
surface session 上的瞬时条件。二者都不等于成功提交，Runtime 必须保留完整重绘需求。

`Unsupported` 和 `InvalidInput` 是确定性拒绝，不得自动循环重试。Runtime preparation
仍负责正常路径的 capability 和 graphics input 预检；backend 保留防御性验证。

### 2. Backend-neutral 诊断

运行失败携带 backend-neutral `RenderFailureDiagnostic`。它保存稳定摘要和 source chain
文本，不暴露 Vello、WGPU 或平台类型。具体 backend 可以另外保留原始错误供自身诊断，
Core 不把第三方错误加入公共签名。

失败分类由 backend 完成：

- surface acquire 的 lost/outdated -> `SurfaceLost`；
- device lost callback -> `DeviceLost`；
- WGPU out-of-memory -> `OutOfMemory`；
- Vello encoding、WGPU validation/internal 或未知错误 -> `Fatal`。

不得通过字符串匹配决定分类。

### 3. Runtime 完成确认是恢复权威

Host 必须把完整 `RenderResult` 交给
`Runtime::complete_submission`。Runtime 返回结构化 acknowledgement：

| 提交结果 | Runtime 动作 | acknowledgement |
|---|---|---|
| `Presented` | 提交资源 cursor；Snapshot 建立 session 基线 | `Presented` |
| `Skipped` | 恢复 in-flight 资源；保留 Full | `Skipped` |
| `Retry` | 恢复 in-flight 资源；保留 Full | `Retry` |
| `SurfaceLost` | 恢复资源；保留当前 backend session 与 Full | `SurfaceInvalidated` |
| `DeviceLost` | 递增 backend session；丢弃旧 in-flight；下帧 Snapshot + Full | `BackendSessionInvalidated` |
| `Unsupported` / `InvalidInput` | 恢复资源与 Full，不自动重试 | `Rejected` |
| `OutOfMemory` / `Fatal` | 恢复资源与 Full，不自动重试 | `Terminated` |

只有 Runtime 可以递增 `BackendSessionId`。Host 收到
`BackendSessionInvalidated` 后重建 backend，但不得再次调用
`reset_backend_session`。

completion 对无 in-flight、session/frame 不匹配、Runtime faulted 和 session id
耗尽返回结构化错误，不再以 `false` 折叠。

### 4. Host 调度

Native 与 Web Host 使用同一恢复规则：

- `Retry`、`Skipped`：按平台可见性和 wakeup 规则请求后续 redraw；
- `SurfaceInvalidated`：让现有 backend 完成 surface reconfigure，并请求 redraw；
- `BackendSessionInvalidated`：异步或同步重建 backend，成功后请求 redraw；
- `Rejected`、`Terminated`：停止自动提交并向应用暴露失败；
- `Presented`：仅在 Runtime 仍有 pending work 时继续调度。

Demo、benchmark 和验证器可以选择 panic/返回 JS error，但必须先完成 Runtime
acknowledgement，不能留下永久 in-flight frame。

### 5. Vello 边界

Vello backend 必须：

- 把 `render_to_texture` 错误转换成 `RenderFailure`，不得 panic；
- 安装 device-lost 与 uncaptured-error callback，并在下一次提交边界发布故障；
- surface lost/outdated 返回 `SurfaceLost`，同时准备同一 backend 内的 reconfigure；
- renderer 初始化失败通过 `VelloInitializationError` 返回；
- 测试使用 backend-local fault injection，不要求真实破坏 GPU device。

纹理创建和 WGPU queue submission 中仍可能存在 API 自身的异步错误；它们必须通过
上述 callback 在后续提交边界变成结构化失败。

## 兼容与后果

- 本次属于 0.1 breaking migration，不保留 `RenderOutcome` 别名或旧 submit 签名；
- `UpdateEvent::Submitted` 保存完整 `RenderResult`，观察者能区分 disposition 与 failure；
- surface recovery 不清空 backend resource cache；device recovery 必须建立新 session；
- OOM 不自动重试，避免无界提交循环；
- Core 不负责创建 backend 或 surface，Host 仍是 composition root。

## 验证

- Core fault-injection contract 覆盖所有 disposition/failure 到 acknowledgement 的映射；
- DeviceLost 后旧 completion 被拒绝，下一帧为新 session 的 Snapshot + Full；
- SurfaceLost 保持 session，下一帧仍恢复资源与 Full；
- Vello 纯分类测试覆盖 surface、device、OOM 和 encoding failure；
- Native/Web host consumer 编译并穷尽处理 acknowledgement；
- `core.tc12-render-failure`、docs、Clippy、quick 与 full gate。

## 关系

- 延续 ADR-012 的资源因果日志与 backend session；
- 延续 ADR-017 的结构化公开失败合同；
- 延续 ADR-022 的第三方类型隔离；
- 关闭临时概念设计审计 TC-12。
