# ADR-033：Runtime 子系统 Facade 与统一 Figure Mutation

类型：`architecture-decision`

## 状态

已接受并验证；2026-10-12 同步完成态和服务边界。P2-S03 已完成，证据见
[完成记录](../verification/reviews/p2-s03-runtime-facade-completion-2026-10-10.md)。

## 背景

[API 与架构 Review 2026-10-09](../verification/reviews/api-architecture-review-2026-10-09.md)
发现项 R01：`novadraw::Runtime` 已成为跨领域的大型公开对象。

- [runtime.rs](../../novadraw/src/runtime/runtime.rs) 单文件超过 6000 行；
- Runtime 的直接和拆分 `impl` 合计承载约 248 个公开方法；
- frame、resource、connection、input、focus、tooltip、accessibility、animation、
  observation、text 以及具体 Figure mutation 全部平铺在同一类型上。

ADR-019 已确认 Runtime 是挂载后 mutation、派生状态、通知、damage 和 frame publication
的唯一提交权威。唯一提交权威不等于所有领域动词都必须成为 `Runtime::*` 方法。

原草案还保留 `label(id)`、`image(id)`、`viewport(id)` 等内置类型专用 facade。
这会让内置 Figure 获得外部 Figure 无法使用的快捷路径，违反“内置与扩展同模型”原则。
内置类型只是预先实现的代码；其挂载后 mutation 必须和外部 Figure 一样使用 typed
component/capability 协议。

本决策只重排公开调用面，不拆分 Runtime 的状态所有权，也不引入第二个提交者。

## 决策

### 1. Runtime 只保留组合根职责

Runtime 本体的公开方法只包含：

1. 构造、contents replacement、subtree disposal；
2. tree 查询入口与通用 `FigureMut` 获取；
3. fault/session 状态；
4.各子系统短借用 facade 的获取。

目标形态：

```rust,ignore
impl Runtime {
    pub fn new(tree: FigureTree) -> Self;
    pub fn with_text_layout_engine(...) -> Self;
    pub fn empty() -> Self;

    pub fn tree(&self) -> &FigureTree;
    pub fn figure(&mut self, id: FigureId) -> Result<FigureMut<'_>, RuntimeMutationError>;
    pub fn set_contents(&mut self, id: FigureId) -> Result<bool, RuntimeMutationError>;
    pub fn dispose_subtree(&mut self, id: FigureId) -> Result<bool, RuntimeMutationError>;

    pub fn is_faulted(&self) -> bool;
    pub fn take_validation_error(&mut self) -> Option<ValidationError>;
    pub fn take_deferred_mutation_errors(&mut self) -> Vec<RuntimeMutationError>;

    pub fn frames(&mut self) -> RuntimeFrames<'_>;
    pub fn resources(&mut self) -> RuntimeResources<'_>;
    pub fn connections(&mut self) -> RuntimeConnections<'_>;
    pub fn input(&mut self) -> RuntimeInput<'_>;
    pub fn focus(&mut self) -> RuntimeFocus<'_>;
    pub fn tooltip(&mut self) -> RuntimeTooltip<'_>;
    pub fn accessibility(&mut self) -> RuntimeAccessibility<'_>;
    pub fn animation(&mut self) -> RuntimeAnimation<'_>;
    pub fn observations(&mut self) -> RuntimeObservations<'_>;
    pub fn text(&mut self) -> RuntimeText<'_>;
    pub fn viewports(&mut self) -> RuntimeViewports<'_>;
    pub fn layers(&mut self) -> RuntimeLayers<'_>;
}
```

Runtime 本体公开方法预算为 **不超过 25 个**。新增方法必须先证明其属于组合根，而不是
既有子系统或具体内置类型。

### 2. 子系统 facade 是无状态短借用窗口

| Facade | 职责 |
|---|---|
| `RuntimeFrames` | frame preparation/completion、backend session、viewport resize、full redraw |
| `RuntimeResources` | image/font request、completion、status、dependency 与 registry query |
| `RuntimeConnections` | anchor、router、locator、connection state、route query 与 invalidation |
| `RuntimeInput` | mouse、keyboard、scroll、zoom、gesture cancellation 与 pointer exit |
| `RuntimeFocus` | focus request/release/traversal、policy 与 cursor |
| `RuntimeTooltip` | timing、visible snapshot、placement 与 update drain |
| `RuntimeAccessibility` | snapshot、action、update drain |
| `RuntimeAnimation` | clocks、timeline mutation、time advance 与 wake deadline |
| `RuntimeObservations` | listener registration/removal、scoped subscription 与 stable observation |
| `RuntimeText` | text layout、interaction geometry、revision 与 measurement statistics |
| `RuntimeViewports` | viewport 关系、range/origin/zoom 协调 |
| `RuntimeLayers` | keyed layer 关系及其生命周期协调 |

每个 facade 只保存 `&mut Runtime`：

```rust,ignore
pub struct RuntimeFrames<'a> {
    runtime: &'a mut Runtime,
}
```

Facade 不保存缓存、自有状态或待提交操作；Drop 不执行逻辑。所有方法继续委托同一
Runtime primitive，并保留原有 Result、幂等和 fault 语义。

Connection 不归 Resource facade。两者可以共享 Runtime 内部 dependency primitive，
但公开职责分别是图连接协调和资源生命周期。
Viewports/Layers 是领域关系与服务视图，不是被删除的 `viewport(id)`、
`layered_pane(id)` concrete Figure facade 的别名。它们不能靠内置类型闭集决定权限。

### 3. 内置与外部 Figure 使用统一 mutation 模型

Runtime 只提供通用：

```rust,ignore
runtime.figure(id)?.set_bounds(bounds)?;
runtime.figure(id)?.update_component(SetLabelText::new(text))?;
runtime.figure(id)?.update_capability(CONTAINER, AddChild::new(child))?;
```

规则：

- concrete Figure 私有状态使用 `FigureComponentUpdate<U>`；
- 独立跨类型消费者使用 `CapabilityKey<C>` 与 typed capability update；
- 内置和外部实现使用相同的 identity、prepare、commit、revision、invalidation、
  damage、notification 和错误通道；
- 删除 `label(id)`、`image(id)`、`text_flow(id)`、`viewport(id)`、
  `scalable_polygon(id)` 等仅面向内置 concrete type 的公开 facade；
- 不增加以 concrete enum、downcast 或中心 match 识别内置 Figure 的替代入口。
- prepare 读取当前提交前缀；NoChange 不执行 commit、不提升 revision、不产生
  work/journal/damage；资源/结构 effect 在 commit 前完整校验 scope 与原子性，
  详见 [组件更新协议](../design/architecture/component-update.md)。

标准 capability 可以提供预定义 key 和 typed payload；第三方 capability 可以定义
自己的 key 和 payload。二者只在代码交付时间上不同。

本节修订 ADR-019 中“新容器能力优先增加专用 facade”以及 ADR-027 中允许公开
concrete-type 专用 facade 的部分：detached concrete API 可以保留，attached mutation
必须统一进入 `FigureMut` 的 typed component/capability 协议。

### 4. 内部 primitive 不构成第二套公开 API

迁移期间，原 Runtime 方法可以临时降为 `pub(crate)`，用于当前 crate 内部调用。
最终代码可以保留命名内部 primitive，但不得重新公开，也不得由其他 crate 绕过 facade。

Core 内部同样应逐步调用领域 primitive，而不是为了减少迁移工作长期保留一套平铺
Runtime API。

### 5. ViewerDrive 是宿主受限视图

ADR-032 的 `ViewerDrive` 继续是 Editor 对宿主暴露的受限能力集。它可以在内部委托
`RuntimeFrames`、`RuntimeResources` 等 facade，但不得返回这些 facade，也不得让宿主
取得完整 Runtime。

因此：

- Runtime 子 facade 是 Core 的领域公开 API；
- ViewerDrive 是 Editor 对 host 的 capability projection；
- 二者共享底层语义，不形成两个提交权威。

### 6. 直接 breaking

项目仍为 `0.1.0`。不保留：

- Runtime 本体上已迁入子 facade 的公开方法；
- 内置 concrete Figure 的公开 scoped facade；
- deprecated alias、compatibility trait 或跨 crate pass-through。

## 错误处理

- facade 获取不修改状态；
- facade Drop 不提交、不清理、不 panic；
- foreign/stale Figure、capability absent、prepare rejection 和 commit fault 继续返回
  ADR-019/027 已定义的结构化错误；
- 子系统 facade 不吞掉 Runtime fault；
- 内部 descriptor mismatch 是一致性错误，不伪装为 capability absent。

## 外部消费者证据

必须提供 workspace 外部语义的测试 crate，证明：

1. 外部 Figure 可以定义 `FigureComponentUpdate<U>` 并通过
   `runtime.figure(id)?.update_component(...)` 修改；
2. 外部 Figure 可以注册自定义 `CapabilityKey<C>`，并通过与内置 capability 相同的
   Runtime commit 路径查询和更新；
3. 测试不调用任何内置 concrete Figure facade，也不修改 Core match。

## 验证

1. API snapshot：Runtime 本体公开方法数不超过 25；
2. API snapshot：每个子 facade 独立冻结，root 不平铺这些类型；
3. compile-fail：`runtime.prepare_submission(...)` 等旧入口不可用；
4. compile-fail：`runtime.label(id)` 等 concrete facade 不可用；
5. external consumer：内置与外部 component/capability 共用协议；
6. editor、platform-winit、platform-web、examples 与相关 Core suite 通过；
7. `cargo xtask docs`、workspace fmt/check，完成边界执行 workspace Clippy/test。

## 实施与回滚

已确认 frame/input/resource 三类调用点合计超过 700 处，完整调用面以 Execute 阶段
机器统计为准。迁移按可独立回退的 commit 进行：

1. 引入无状态 facade 与 API 门禁，不删除旧入口；
2. 逐领域迁移 Frames、Resources、Connections、Input；
3. 迁移 Focus、Tooltip、Accessibility、Animation、Observations、Text；
4. 把内置 concrete facade 调用迁为 typed component/capability update；
5. 删除旧公开入口并更新 API snapshot；
6. 写完成记录并关闭 R01。

任一中间 commit 可通过 `git revert` 回退。删除旧入口只能发生在所有 workspace
消费者完成迁移后，不与调用点迁移混在同一个 commit。

## 关系

- 落实 [R01](../verification/reviews/api-architecture-review-2026-10-09.md#r01runtime-公开面已达大上帝对象量级)；
- 继承 ADR-009、ADR-014、ADR-018 的提交、fault 和 frame 合同；
- 修订 [ADR-019](adr-019-composable-api-and-scoped-editors.md) 与
  [ADR-027](adr-027-open-figure-capability-registry.md) 的 concrete facade 例外；
- 不改变 [ADR-032](adr-032-viewer-edit-session-and-drive-boundary.md) 的
  ViewerDrive 边界；
- 与 [ADR-035](adr-035-explicit-domain-imports-and-root-boundary.md) 一起收敛公开
  命名空间。

## 日期

2026-10-09
