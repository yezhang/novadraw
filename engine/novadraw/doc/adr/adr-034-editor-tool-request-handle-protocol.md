# ADR-034：Editor Tool / Request / Handle 统一开放协议

类型：`architecture-decision`

## 状态

已实现，修订 1（2026-10-12）。P2-S04 已完成，实施与验证证据见
[P2-S04 完成记录](../verification/reviews/p2-s04-editor-open-protocol-completion-2026-10-10.md)。
本决策关闭
[TC-06](../verification/reviews/temporary-concept-design-audit-2026-10-08.md#36-tc-06editor-扩展只开放了-policy-实现)。

## 背景

[API 与架构 Review R05](../verification/reviews/api-architecture-review-2026-10-09.md#r05editor-扩展协议闭集复核-tc-06)
确认 Editor 的主要扩展入口仍由 Core 已知类型集合定义：

- `ActiveTool<A>` 固定列举 Selection、ConnectionCreation、ConnectionEndpoint 和
  ConnectionBendpoint；
- `EditorRequest` 固定列举 6 种请求；
- `HandleRole` 固定列举 5 种 handle action；
- `PolicyRole` 通过 `Custom` 变体为外部代码留洞，但内置 role 仍是特权枚举；
- `EditPolicy` 直接提供 `start_connection`、`start_reconnection` 和
  `start_direct_text_edit` 等内置 request typed hook。

这会形成两层 API：内置功能有强类型方法，外部功能只能使用 custom role、类型擦除或
等待 Core 新增枚举。该模型违反项目的基本原则：

> 内置功能只是提前实现的代码。内置与外部 Tool、Request、Handle 和 Policy 必须使用
> 同一 typed identity、payload、registration、dispatch、错误和生命周期协议。

GEF 使用 `Request` 与 role map 提供开放性，但其 `Object` key/payload 把类型错误推迟到
运行期。Novadraw 保留开放分派语义，同时用 typed protocol 和私有 erased registry
提供 Rust 类型安全。

ADR-032 已经收口 CommandStack、ViewerEditSession 和 ViewerDrive。本决策不改变这些
提交边界，只统一产生和处理编辑意图的协议。

## 决策

### 1. 公开协议始终类型化，擦除仅存在于 registry 内部

每个 request family 由一个公开 marker type 实现 `RequestProtocol<A>`：

```rust,ignore
pub trait RequestProtocol<A: ModelAdapter>: 'static {
    type Payload: RequestPayload;
    type Contribution: 'static;

    fn key() -> RequestKey<Self>;
    fn reduce(
        contributions: Vec<Self::Contribution>,
    ) -> Result<RequestResolution<Self::Contribution>, PolicyError>;
}

pub struct RequestKey<R>(/* opaque identity + diagnostic name */)
where
    R: ?Sized;
```

调用者通过 protocol marker 获得完整类型关系：

```rust,ignore
let resolution = context.dispatch::<ChangeBounds>(
    RequestMetadata::new(revision, source_parts),
    ChangeBoundsPayload::new(...),
)?;
```

`ChangeBounds`、`CreateConnection` 等内置 marker 与第三方 `AlignToGuide`、
`CreateDomainLink` 等 marker 使用完全相同的 trait 和入口。

Runtime 内部可以保存：

```text
ErasedRequestKey + erased payload + erased contribution adapter
```

但 `ErasedRequest`、`Any`、`TypeId` 和 downcast 不进入公共 API。公开调用方不能构造
“key 与 payload 类型不匹配”的 request。

### 2. Request metadata 与 payload 分离

所有 request 共有的因果身份进入统一 metadata：

```rust,ignore
pub struct RequestMetadata {
    revision: InteractionRevision,
    source_parts: Arc<[EditPartId]>,
}
```

- revision、source parts 属于 request envelope，不由每个 payload 重复实现；
- diagnostic label 属于 `RequestKey` descriptor；
- pointer、modifiers、bounds、endpoint、bendpoint index 等领域数据属于 payload；
- `Option` 只表达协议定义的合法缺失，不表达未知 request 或类型不匹配。

因此不再需要 Core 对 `EditorRequest` 做 `label()`、`revision()` 或
`source_parts()` 的 variant match。

ADR-055 补充 `GraphicalViewer::aggregate_at<R>` 与 `ToolContext::aggregate_at<R>`：
在显式 host 上聚合，不运行普通 dispatch 的 target callback，真实 source 保留在
metadata 中。host 与全部 source 均须通过活动身份检查。
`ContributionLimit::AtMostOne` 在 reducer 前后检查数量；`All` 保留原聚合行为。
所有 request family 共用这一入口，容器协议不享有特殊分派路径。

### 3. EditPolicy 在安装时注册 typed handler

`EditPolicy<A>` 只保留统一生命周期：

```rust,ignore
pub trait EditPolicy<A: ModelAdapter>: 'static {
    fn register_handlers(
        builder: &mut PolicyHandlerBuilder<'_, A, Self>,
    )
    where
        Self: Sized;

    fn activate(&mut self, host: PolicyHost<A::ModelId>, model: &A)
        -> Result<(), PolicyError>;
    fn deactivate(&mut self, host: PolicyHost<A::ModelId>, model: &A);
}
```

安装具体 policy 时，builder 记录 typed adapter：

```rust,ignore
builder.handle::<ChangeBounds>(Self::change_bounds);
builder.handle::<CreateConnection>(Self::start_connection);
builder.handle::<MyExternalRequest>(Self::handle_external);
```

adapter 内部负责把 erased storage 还原为注册时已证明匹配的 policy、payload 和
contribution 类型。公共 handler 始终得到 `&R::Payload` 并返回
`R::Contribution`。

`understands` 不再需要单独调用：存在对应 handler 就表示 policy understands request。
无 handler 表示无 contribution，不是类型错误。

以下内置特权 hook 直接删除：

- `start_connection(&CreateConnectionRequest, ...)`；
- `start_reconnection(&ReconnectConnectionRequest, ...)`；
- `start_direct_text_edit(&DirectTextEditRequest, ...)`。

它们改为普通 `RequestProtocol` handler。第三方可以定义具有任意 typed contribution
和 reducer 的 request family，不修改 `EditPolicy` trait。

### 4. Policy slot 使用开放 typed key

删除 `PolicyRole` 枚举及 `Custom` 变体，改为：

```rust,ignore
pub trait PolicySlot: 'static {}
pub struct PolicyKey<S: PolicySlot>(/* opaque identity */);
```

内置 Component、Layout、PrimaryDrag、SelectionFeedback、ConnectionCreation、
ConnectionReconnect、DirectTextEdit 都只是预定义 `PolicyKey<S>`。第三方声明自己的
slot marker 和 key，并通过同一个 `PolicyInstallation::new(key, policy)` 安装。

同一 host 上 duplicate key 在安装阶段结构化拒绝；不存在字符串 role 或整数 custom
slot。

### 5. Tool 使用统一 object-safe 生命周期

```rust,ignore
pub trait Tool<A: ModelAdapter>: 'static {
    fn key(&self) -> ErasedToolKey;
    fn activate(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError>;
    fn deactivate(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError>;
    fn cancel(&mut self, context: &mut ToolContext<'_, A>) -> Result<(), ToolError>;

    fn mouse_pressed(&mut self, context: &mut ToolContext<'_, A>, event: MouseEvent)
        -> Result<ToolDirective, ToolError>;
    fn mouse_moved(&mut self, context: &mut ToolContext<'_, A>, event: MouseEvent)
        -> Result<ToolDirective, ToolError>;
    fn mouse_released(&mut self, context: &mut ToolContext<'_, A>, event: MouseEvent)
        -> Result<ToolDirective, ToolError>;
    fn key_event(&mut self, context: &mut ToolContext<'_, A>, event: KeyEvent)
        -> Result<ToolDirective, ToolError>;
    fn tick(&mut self, context: &mut ToolContext<'_, A>, now: MonotonicTime)
        -> Result<ToolDirective, ToolError>;
}
```

- EditorDomain 只保存 `Box<dyn Tool<A>>`；
- built-in Tool 与 external Tool 都通过 `ToolKey<T>` 构造 typed registration，再擦除为
  `ErasedToolKey` 存储；
- `ToolContext` 提供 typed `dispatch<R: RequestProtocol<A>>()`，不暴露完整 Viewer、
  Runtime 或 CommandStack；
- `ToolDirective` 只描述引擎固定控制流，例如 Continue、ReleaseCapture、Deactivate；
  它不列举业务 Tool 或 Request；
- Tool 不能在 callback 中同步替换自己。切换由 EditorDomain 在 callback 返回并释放
  Tool 借用后执行，避免重入和所有权不确定性。

Selection、ConnectionCreation、ConnectionEndpoint 和 ConnectionBendpoint 全部迁为
普通 `Tool<A>` 实现；删除 `ActiveTool`。

### 6. Handle action 使用开放 typed protocol

```rust,ignore
pub trait HandleActionProtocol: 'static {
    type Payload: 'static;
    fn key() -> HandleActionKey<Self>;
}
```

Handle registration 接收 typed action：

```rust,ignore
viewer.add_handle(
    owner,
    ResizeAction::key(),
    ResizePayload::new(direction),
    figure,
)?;
```

内部 registry 可以保存 erased action，ToolContext 通过
`action.get::<ResizeAction>()` 等 typed API 读取。内置 Selection、Resize、
ConnectionEndpoint、BendpointMove 和 BendpointCreate 与第三方 action 共用协议。

`HandleId` 是 overlay instance identity；`HandleActionKey` 是行为协议 identity。
一个 action 可以对应多个 HandleId，一个 HandleId 最多绑定一个 action。

删除 `HandleRole`。`VisualOwner` 不再公开其 registry 字段；R08 的 overlay 命名空间
收口与本批次一并完成。

### 7. 内置实现没有 fast path

Core 中禁止：

- match 内置 Request/Tool/Handle/Policy variant；
- 按内置 concrete type 绕过 registry；
- 为内置 request 调用专用 hook，而外部 request 走 erased fallback；
- 为内置 handler 建立不同的错误、reducer 或生命周期通道。

性能优化只能作用于统一 registry，例如 key lookup cache；同样惠及内置和外部实现。

### 8. 直接 breaking

项目仍为 `0.1.0`。删除：

- `EditorRequest`；
- `HandleRole`；
- `ActiveTool`；
- `PolicyRole`；
- `EditPolicy::understands`；
- 三个内置 `start_*` hook；
- 字符串、整数或 concrete variant request dispatch。

不保留 deprecated alias 或双入口。

## 错误处理

- duplicate PolicyKey、RequestKey handler 或 HandleActionKey：安装失败，不发布部分状态；
- request 没有 handler：合法无 contribution；
- typed handler 拒绝：返回 `PolicyError`，不创建 Command/plan；
- registry 内部 descriptor mismatch：结构化一致性错误，使 Editor faulted，不伪装成
  no command；
- Tool callback panic：按 ADR-032 fault Editor/Viewer，不继续使用当前 Tool；
- Handler 返回可恢复拒绝时必须保持 model revision 和 notification 前缀不变。

## 外部消费者证据

必须提供独立 workspace consumer，而不是 `novadraw-editor` crate 内 unit test：

1. 外部 Marquee Tool 定义自己的 `ToolKey<T>`，完成 activate/input/cancel 生命周期；
2. 外部 AlignToGuide request 定义自己的 `RequestProtocol<A>`、payload、contribution
   和 reducer；
3. 外部 EditPolicy 通过 `PolicyHandlerBuilder` 注册该 handler；
4. 外部 GuideHandle 定义自己的 `HandleActionProtocol`；
5. 全流程不修改 Editor Core，不使用字符串、公开 Any 或 downcast；
6. 外部实现与内置实现通过同一 command/session/error/cleanup 门禁。

## 验证

1. API snapshot：旧四个闭合枚举和三个 `start_*` hook 退出；
2. API snapshot：Tool、RequestProtocol、RequestKey、RequestMetadata、
   PolicyHandlerBuilder、PolicyKey、HandleActionProtocol 与 HandleActionKey 冻结；
3. source gate：Editor Core 不 match 内置 Request/Tool/Handle/Policy；
4. external consumer：Tool + Request + Policy handler + Handle action 全链路；
5. G1、G2、G3、G4、G5、P2-E02 定向 suite 全部通过；
6. fault injection：duplicate key、missing handler、handler rejection、descriptor mismatch、
   callback panic；
7. `cargo xtask docs`、workspace fmt/check，完成边界执行 workspace Clippy/test。

## 实施与回滚

按可独立回退的 commit 推进：

1. 引入 typed key、protocol 和私有 erased registry；
2. 迁移一个内置 request family，证明统一模型；
3. 迁移其余 request 与 Policy handler，删除专用 `start_*`；
4. 迁移 Tool，删除 `ActiveTool`；
5. 迁移 Handle 与 Policy slot，删除 `HandleRole` / `PolicyRole`；
6. 加入外部 consumer、source gate 和 API snapshot；
7. 写完成记录并关闭 TC-06/R05。

旧 enum/hook 只在所有调用点迁移完成后删除。任何阶段失败均回退当前原子 commit，不在
同一 commit 同时引入 registry、迁移全部消费者并删除旧协议。

## 关系

- 落实 [R05](../verification/reviews/api-architecture-review-2026-10-09.md#r05editor-扩展协议闭集复核-tc-06)；
- 关闭 [TC-06](../verification/reviews/temporary-concept-design-audit-2026-10-08.md#36-tc-06editor-扩展只开放了-policy-实现)；
- 继承 ADR-015 的 Editor 边界和 ADR-032 的 ViewerEditSession；
- 复用 [ADR-027](adr-027-open-figure-capability-registry.md) 的 typed identity、
  attach/install-time descriptor、私有擦除与外部 consumer 方法；
- 为 TC-08/R06 的投影失败隔离与临时视觉清理复用统一 Tool/Handle 所有权协议；
  不要求新增 OverlayBatch 或 ProjectionPlan，当前范围见
  [P2-S06 队列](../roadmap/p2-delta-backlog.md)。

## 日期

2026-10-09
