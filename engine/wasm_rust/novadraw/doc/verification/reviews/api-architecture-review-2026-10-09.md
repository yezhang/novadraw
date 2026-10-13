# API 与架构 Review

类型：`verification`

日期：2026-10-09

状态：`findings_open`

## 1. 审计信息

- 代码基线：`d0da529` 加 2026-10-09 工作区（含 `book/internals/`、
  `doc/strategy/` 的未提交文档改动）；
- 审计范围：`novadraw` Core、`novadraw-editor`、`novadraw-inspector`、
  Vello backend 与 Winit/Web platform adapter 的公开 API 和架构边界；
- 审计方式：静态源码/文档交叉检查，未执行运行期、性能、平台或 full workspace
  门禁；
- 关联基线：
  - [临时概念设计审计 2026-10-08](temporary-concept-design-audit-2026-10-08.md)
  - [框架公共 API 复审 2026-10-07](framework-public-api-follow-up-audit-2026-10-07.md)
  - [Core 公开 API 审计 2026-09-22](core-public-api-audit-2026-09-22.md)
  - [ADR-021 公开 Facade 与 Feature 边界](../../adr/adr-021-public-facade-and-feature-boundary.md)
  - [ADR-032 Viewer 编辑会话与宿主驱动边界](../../adr/adr-032-viewer-edit-session-and-drive-boundary.md)

本报告是对临时概念设计审计 TC-01 ~ TC-14 之外的横向复查，用于补齐该审计未显式
列出但同等级的架构债，并给出下一个推进顺序建议。本报告是对应快照的历史证据，
不覆盖当前 `design/`、ADR、parity ledger 或 roadmap。

## 2. 总体结论

ViewerEditSession（ADR-032）、typed property（ADR-028）、render failure（ADR-029）、
resource provider（ADR-030）、Graphics 单入口（ADR-031）五条核心边界已经落位；
`&mut self.model` 全库仅两处合规入口（`synchronize_model` 回调与
`begin_edit_session`），模型可变逃逸已消除。TC-01、TC-02、TC-04、TC-05、TC-07、
TC-11、TC-12、TC-13、TC-14 的闭合证据已在对应完成记录中。

但当前仍有两条架构主线明显阻塞 Novadraw 作为"可扩展 GEF 风格框架"的产品定位：

1. **Core 面积过大**：`impl Runtime` 已经成为"大上帝对象"，合计约 248 个公开方法，
   覆盖 frame、resource、anchor/router、capability、submission、焦点、时间、
   tooltip、accessibility 等全部子系统；
2. **Editor 扩展协议闭合**：`EditorRequest`、`ActiveTool`、`HandleRole` 仍为
   crate 内部闭集，没有 `Tool<A>` / `ToolContext` / `RequestKey<T>` /
   `HandleActionKey<T>` 等扩展点，第三方 marquee、guide、palette、业务 tracker
   必须 patch Core。

另外，TC-03、TC-06、TC-08、TC-09 以及 TC-10 的 Animation / Inspector /
`register_*` 双入口 / error source 丢失子项，在当前基线上仍然打开。

本报告认定 **8 项新发现 + 复核 7 项仍打开的 TC**。所有发现仅为静态证据，未声称
现有运行期行为出现回归。

## 3. 发现项

### R01：Runtime 公开面已达"大上帝对象"量级

- 优先级：P0
- 临时概念：单一 `impl Runtime` 承接所有子系统的方法转发；
- 证据：
  - [runtime.rs](../../../novadraw/src/runtime/runtime.rs) 单文件 6094 行，
    `impl Runtime` 直接暴露约 97 个 `pub fn`；
  - 加上拆分出去的 `connection_service`（27）、`frame_resource`（19）、
    `frame_submission`（6）、`capability_update`（1）等子 impl，Runtime 的公开
    方法总数约 248 个；
  - 覆盖 frame 调度、资源注册、anchor/router、capability update、submission、
    焦点、时间、tooltip、accessibility、每一类 Figure 的 `*_mut()` 等全部子系统。
- 长期方向：按子系统拆分 facade：
  - `Runtime::frames() -> RuntimeFrames<'_>`：`prepare_submission` /
    `complete_submission` / `reset_backend_session` / 全量重绘；
  - `Runtime::focus() -> RuntimeFocus<'_>`：焦点、焦点遍历策略、tooltip；
  - `Runtime::resources() -> RuntimeResources<'_>`：image/font/token/`ResourceDelta`；
  - `Runtime::animation() -> RuntimeAnimation<'_>`：Animation 服务；
  - `Runtime::accessibility() -> RuntimeAccessibility<'_>`：snapshot / action /
    update；
  - `Runtime::input() -> RuntimeInput<'_>`：所有 `dispatch_*`；
  - `Runtime` 本体只保留 lifecycle / fault / session 顶层 API。
- 验证门禁建议：
  - API snapshot 把 Runtime 公开方法数列为单独指标，禁止 root `impl Runtime` 继续
    新增方法；
  - compile-fail 测试锁死 Runtime 本体不再暴露 `prepare_submission` 等已经迁入
    子 facade 的方法。

### R02：root `pub use` 与 prelude 重复暴露，违反 ADR-021

- 优先级：P0
- 临时概念：root 把具体 Figure、Layout、Border、几何、Graphics 全部 re-export，
  prelude 复刻同一组符号；
- 证据：
  - [lib.rs:49-70](../../../novadraw/src/lib.rs) root 直接 `pub use` 了 Color、
    几何、所有具体 Figure 类型、所有 LayoutManager 实现、Graphics、Runtime 等；
  - 审计时的 `novadraw/src/prelude.rs` 单文件 prelude 内容与 root 高度重叠；
  - [verification/public-api/novadraw-symbols.txt](../../../verification/public-api/novadraw-symbols.txt)
    中 `prelude` 与 `root` 两个分组互相复刻；
- 长期方向：
  - 删除库级 `novadraw::prelude`；
  - root 只公开领域模块，不平铺 struct、enum、trait、function 或 constant；
  - 每个公开符号只有一个规范领域路径；
  - API snapshot 按领域模块冻结，并显式拒绝 root 非 module 项；
  - 应用可在自己的 composition root 建立本地 prelude，但不能反向成为框架 API；
- 关联：R01 一并影响 Runtime facade 的 root 暴露面。

### R03：公开值类型仍是未受检数据袋（扩展 TC-03）

- 优先级：P0
- 临时概念：Figure、Border、Style 使用全 `pub` 字段 + 静默 clamp / panic alias，
  整治面比 TC-03 清单更广；
- 证据：
  - 矩形族 Figure 全 `pub` 字段并在 `outline_shape` 内 `.max(0.0)` 静默兜底：
    [rectangle.rs:18-35](../../../novadraw/src/figure/rectangle.rs)、
    `rounded_rectangle.rs:20-33`、`ellipse.rs:20-30`、`triangle.rs:65-73`；
  - Border 系整体使用 `assert!` panic 做输入校验：
    [line_border.rs:17-60](../../../novadraw/src/figure/border/line_border.rs)、
    `rectangle_border.rs:29,48`、`margin_border.rs:39,107`、`bevel_border.rs:25`、
    `title_bar_border.rs:47`；
  - [style.rs:20-69](../../../novadraw/src/style.rs) `FigureStyle` 全 `pub`，
    `alpha` 延后到 `apply_override` 的 `clamp`，`font` 为裸 `Option<String>`
    无语法校验；
  - 静默 clamp 热路径：[render/context.rs:L223,L967](../../../novadraw/src/render/context.rs)、
    [render/paint.rs:L112](../../../novadraw/src/render/paint.rs)、
    [graph/presentation.rs:L958](../../../novadraw/src/graph/presentation.rs)
    的 `alpha.clamp(0.0, 1.0)`；
- 长期方向：
  - 引入受检值 `Alpha01`、`NonNegativeF64`、`LogicalLength`、`FontFamily`；
  - Figure、Border、Style 字段改为私有 + 构造器 `Result`；
  - admission 只做跨对象校验，不吞领域不变量；
- 验证门禁：compile-fail 测试阻止 `FigureStyle { alpha: Some(2.0), .. }` 这样的
  字段级构造；Border 不再 panic。

### R04：查询面默认值吞无效身份（扩展 TC-09）

- 优先级：P1
- 临时概念：LayoutSceneQuery / FigureTree 查询对未知身份返回"看似合法"的默认值，
  各错误类型互不兼容；
- 证据：
  - [layout/mod.rs:105-158](../../../novadraw/src/layout/mod.rs) 对未知
    `FigureId` 返回 `Rectangle::ZERO` / `FigureMeasurement::default()` /
    `Dimension::INFINITY`；
  - [graph/layout_measurement.rs:980-1012](../../../novadraw/src/graph/layout_measurement.rs)
    显式 `.unwrap_or_default()` / `.ok()`；
  - [graph/query.rs:130-143](../../../novadraw/src/graph/query.rs)
    `is_visible/is_enabled` 对未知 id 退化为 `false`；
  - 错误类型碎片：`CapabilityQueryError`、`TreeQueryError`、`RouteSceneQueryError`、
    `StableQueryError`、`TextFlowQueryError` 各自为政；
- 长期方向：
  - 引入统一 `QueryError { UnknownFigure, Faulted, Detached, … }`；
  - 结构查询统一返回 `Result<Option<T>, QueryError>`；
  - `Option` 仅表示已验证对象上的合法缺失；
  - 几何变换的 `Option<Affine2D>` 继续表达不可逆或无路径，不机械改为 `Result`。

### R05：Editor 扩展协议闭集（复核 TC-06）

- 当前状态：`closed`，由 ADR-034 / P2-S04 完成；
- 优先级：P0（高于审计原列 P1）
- 临时概念：Editor 承载产品层扩展，但扩展点仍为 crate 内部闭枚举；
- 证据：
  - [domain.rs:113-118](../../../novadraw-editor/src/domain.rs) `ActiveTool<A>`
    为 crate 私有 4 变体；
  - [request/mod.rs:548-561](../../../novadraw-editor/src/request/mod.rs)
    `EditorRequest` 6 变体闭集；
  - [feedback/mod.rs:41-52](../../../novadraw-editor/src/feedback/mod.rs)
    `HandleRole` 5 变体闭集；
  - 全仓无 `Tool<A> trait`、`ToolContext`、`RequestKey<T>`、`HandleActionKey<T>`；
- 长期方向：
  - object-safe `Tool<A>`，通过窄 `ToolContext` 访问 targeting / feedback /
    command 提交；
  - typed `RequestKey<T>` + request envelope；
  - `HandleActionKey<T>`，VisualOwner 保存 opaque action identity；
  - EditorDomain 只持有单个 `Box<dyn Tool<A>>`；
  - built-in Selection / Connection / Bendpoint tools 迁为协议实现；
  - 需至少一个外部 Tool 消费者作为验收证据；
- 优先级上调原因：
  - Novadraw 产品定位要求支持业务层 Tool 扩展；
  - 当前 TC-08 (结构发布事务) 需要 Tool / Request / Handle 协议作为前置条件。

### R06：Editor 结构发布缺事务（复核 TC-08）

- 优先级：P1
- 临时概念：Part / Connection / Overlay 的创建依次调用 Figure、anchor、
  connection state、PartTree、registry、behavior、policy，失败再手工逆操作；
- 证据：
  - [viewer/projection.rs:446-477](../../../novadraw-editor/src/viewer/projection.rs)
    连接发布靠 `let _ = runtime.remove_connection_anchor(...)` 逐步手工回滚；
  - [viewer/mod.rs:1837-1867](../../../novadraw-editor/src/viewer/mod.rs)
    `remove_overlay_visual` / `remove_overlay_visuals` 直接写 registry，
    失败即 `faulted=true`；
  - 全仓无 `OverlayBatch` / `PartProjectionPlan` / `ConnectionProjectionPlan`
    雏形；
- 长期方向：
  - `OverlayBatch`：拥有 FeedbackId / FigureId 集合，Drop 做不可失败本地撤销，
    显式 commit 后转交 Viewer；
  - `PartProjectionPlan` / `ConnectionProjectionPlan`：先准备 Figure subtree、
    policy、anchor、registry delta，完整校验后按一个结构提交原语发布；
  - 外部副作用不承诺回滚，可回滚仅限引擎自身结构和所有权登记；
- 依赖：R05 TC-06 Tool / Request 协议先行。

### R07：失败语义仍跨服务不一致（复核 TC-10 剩余）

- 优先级：P1
- 临时概念：faulted、幂等未变化、缺失身份与扩展 panic 被折叠为同一个 `bool` /
  `None`；
- 证据：
  - [animation/service.rs:1493](../../../novadraw/src/animation/service.rs)
    `AnimationMut::set_mode(..) -> bool` 把 `faulted` 与"幂等未变化"折叠；
    周边 `set_surface_suspended / process_behaviors` 同模式；
  - [inspector/src/lib.rs:167-192](../../../novadraw-inspector/src/lib.rs)
    `events()` / `clear_events()` / `InspectorObserver::observed` 对锁 poison
    静默吞错，无结构化 fault / detach token；
  - [connection_service.rs:15-93](../../../novadraw/src/runtime/runtime/connection_service.rs)
    `register_*` / `try_register_*` 双入口仍存在，前者在 faulted 时
    `.expect(..)` panic；
  - [command/mod.rs:14-45](../../../novadraw-editor/src/command/mod.rs)
    `CommandError { message: String }` 与 `CommandStackError::Rejected/Panicked
    { label: String }` 丢失 error source；panic payload 直接丢弃；
  - [figure/label.rs:L518](../../../novadraw/src/figure/label.rs)、
    [figure/text_flow.rs:L619](../../../novadraw/src/figure/text_flow.rs)
    以 `source: &str` 传入；
- 长期方向：
  - `register_*` 单入口（fallible），废弃 `try_register_*` 别名；
  - AnimationMut / Inspector 返回 `Result<bool, _>` 或分离 fault 与无变化；
  - extension error 使用 `Box<dyn Error + Send + Sync>` 或泛型 facade；
  - Inspector 提供结构化 fault 与 subscription token，明确 detach 生命周期。

### R08：Editor crate 公共 re-export 过度暴露内部注册

- 当前状态：overlay 命名空间与字段暴露面已由 P2-S04 关闭；TC-08/R06 的
  prepared publication 仍由 P2-S06 承接；
- 优先级：P2
- 临时概念：overlay registry 的实现细节被 crate 根 re-export，变成稳定公开面；
- 证据：
  - [novadraw-editor/src/lib.rs:38](../../../novadraw-editor/src/lib.rs) 把
    `FeedbackId`、`HandleId`、`VisualOwner`（含公开 `Handle { id, owner, role }`
    字段）re-export 到 crate 根；
  - `InteractionRevision`、`BendpointHandleSite` 等交互内部数据同理随 `pub use`
    暴露；
- 长期方向：
  - 将 overlay 注册相关标识符移入 `novadraw_editor::overlay::*`；
  - 公共面只保留 `VisualOwner::Part(id)` 这样的 opaque constructor，
    不公开 handle 结构字段；
  - 与 R05 / R06 一起收口。

## 4. 复核仍打开的审计项

| 发现 | 原优先级 | 当前状态 | 建议 |
|---|---|---|---|
| TC-03 detached 值未受检 | P0 | 打开 | 并入 R03，整治面扩展 |
| TC-06 Editor Tool/Request/Handle 闭集 | P1 | 已关闭 | ADR-034 / P2-S04 完成 |
| TC-08 Editor 结构发布依赖手工补偿 | P0 | 打开 | 并入 R06，依赖 R05 先行 |
| TC-09 查询默认值吞无效身份 | P1 | 打开 | 并入 R04 |
| TC-10 剩余（Animation/Inspector/`register_*`/error source）| P1 | 部分打开 | 并入 R07 |

## 5. 建议的原子批次顺序

| 顺序 | 批次 | 覆盖 | 风险 | 依赖 |
|---|---|---|---|---|
| 1 | **Runtime facade 与统一 Figure Mutation**（R01） | Runtime 子系统 facade；内置/外部 Figure 共用 typed mutation | 高 | P2-S02 |
| 2 | **删除 Prelude 与 root 收口**（R02） | 显式领域导入；root 只公开模块 | 高（全仓 API 迁移） | 独立 |
| 3 | **TC-06 Editor 统一开放协议**（R05） | typed Tool/Request/Policy/Handle registry + 内置迁移 | 高 | 独立 |
| 4 | **TC-09 QueryError 统一**（R04） | Layout/graph 查询统一 `Result<Option<T>, QueryError>` | 中 | 独立 |
| 5 | **TC-03 受检值类型**（R03） | `Alpha01` / `NonNegativeF64` / Border 私有字段 | 中 | 独立 |
| 6 | **TC-10 剩余**（R07） | AnimationMut `Result`、Inspector 结构化 fault、`register_*` 单入口、error source | 中 | 可与 5 并行 |
| 7 | **TC-08 OverlayBatch / ProjectionPlan**（R06） | 事务化结构发布 | 高 | 依赖 R05 |
| 8 | **Editor overlay 命名空间**（R08） | 收口 overlay re-export | 低 | 依赖 R05 / R06 |

> **Design 已就绪**（2026-10-09）：R01、R02、R05 的 ADR 草案已就绪，分别为
> [ADR-033](../../adr/adr-033-runtime-subsystem-facade.md)、
> [ADR-035](../../adr/adr-035-explicit-domain-imports-and-root-boundary.md)、
> [ADR-034](../../adr/adr-034-editor-tool-request-handle-protocol.md)；
> Backlog 对应条目为 P2-S03、P2-S02、P2-S04，推荐 Execute 顺序：
> **P2-S02 → P2-S03 → P2-S04**（先建立唯一领域路径，再迁移 Runtime 与 Editor
> 公开协议）。

每个高风险批次均应先评审独立 ADR 或 normative design，再实施；不得合并为一次
全仓重写。

## 6. 验证门禁建议

1. **Runtime 公开方法预算**：API snapshot 独立列出 Runtime 本体与各子 facade
   的方法计数，禁止回归；
2. **compile-fail**：
   - root 不得暴露具体 Figure / LayoutManager 实现；
   - Runtime 本体不得暴露已迁入子 facade 的方法；
   - Border 构造器不得 panic；
3. **external consumer**：
   - 不修改 Editor crate 即可注册自定义 Tool、Request 与 Handle action；
   - 外部 Figure consumer 不修改 Core 即可提供 capability；
4. **结构事务**：Part / Connection / Overlay 发布失败前后 registry、Runtime 与
   PartTree 快照一致；
5. **查询负测**：foreign / stale identity 得到 `QueryError`，不得得到零值或
   `false`；
6. **错误 source 保留**：CommandError / CommandStackError 透出 source，panic
   payload 不被丢弃。

## 7. 不列为缺陷的阶段实现

- Runtime 的 `register_*` 旧别名在 R07 收口前保留，可用；
- `AnimationMut::set_mode -> bool` 在 R07 收口前保留，可用；
- Figure `pub` 字段在 R03 收口前保留，可用于读写；
- 完整 ModelSnapshot 继续作为正确性基线，不改为增量；
- 10,000 层递归合同按既有决策保留。

## 8. 审计置信度与限制

- 本审计基于 `d0da529` 和 2026-10-09 工作区静态证据；
- 未执行 GPU、窗口、Web、性能或 full workspace 门禁；
- 未声称上述问题导致任何现有运行期功能回归；
- Runtime 公开方法计数通过 `rg '^    pub fn ' novadraw/src/runtime/runtime.rs`
  在 `impl Runtime` 范围内统计得到；拆分子 impl 的方法数通过同样方式对
  `runtime/runtime/connection_service.rs`、
  `runtime/runtime/frame_resource.rs`、
  `runtime/runtime/frame_submission.rs`、
  `runtime/runtime/capability_update.rs` 统计。
