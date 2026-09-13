# 07-GEF / Editor 路线图

类型：`roadmap`

状态：`in_progress`

本目录承载 Draw2D Core 1.0 之后的独立 Editor 框架路线图。`G0-G6` 只属于本目录，
不得与 Draw2D `M1-M10` 或历史 architecture delta `D0-D4` 混用。

## 文档边界

- 外部 GEF 源码事实：`doc/reference/gef/`
- Novadraw Editor 规范：`doc/design/editor/`
- 采用与差异：`doc/parity/gef/api-coverage.md`
- 关键架构决策：`doc/adr/adr-015-editor-framework-boundary.md`
- 实施顺序和状态：本目录
- 阶段验证：`doc/verification/`

## 状态

| 状态 | 含义 |
|---|---|
| `not_started` | 尚未进入当前开发主线 |
| `in_progress` | 正在补契约、实现或验证 |
| `contract_aligned` | 公开契约稳定，行为验证未闭合 |
| `behavior_verified` | 自动契约和 headless 行为闭合 |
| `complete` | 产品入口、跨平台验证和文档全部闭合 |

## 里程碑

| Milestone | 标题 | 状态 | 完成门禁 | 人工验收 |
|---|---|---|---|---|
| G0 | 架构与工程启动 | `complete` | ADR、规范、语义账本、单 crate 骨架、workspace 门禁 | 不需要 |
| G1 | Model Adapter 与 CommandStack | `complete` | 16 项契约覆盖 identity/revision、execute/undo/redo、compound、dirty/save 与 fault | 不需要 |
| G2 | EditPart Tree 与 Viewer 投影 | `complete` | factory、生命周期、registry、contents/root、增量 containment 同步 | 不需要 |
| G3 | Selection、Targeting 与输入仲裁 | `behavior_verified` | 多选/primary/focus、visual targeting、Figure/Editor fallback 消费与 capture | **检查点 A：待人工确认** |
| G4 | Tool / Request / EditPolicy 编辑闭环 | `not_started` | create、move、resize、delete、feedback、undo/redo | **检查点 B：首个可用编辑闭环** |
| G5 | Connection 编辑与 Viewport 协作 | `not_started` | create/reconnect/bendpoint、auto-expose、scroll/zoom 下反馈 | **检查点 C：图编辑主流程** |
| G6 | 产品化与跨平台毕业 | `not_started` | 保存加载、Native/Web/Headless 等价、节点编辑器毕业场景 | **最终验收** |

人工验收只在对应 milestone 的自动门禁通过后进行：

- 检查点 A：验证单选、多选、primary/focus、空白区回退和 widget 输入隔离；
- 检查点 B：验证 create/move/resize/delete、feedback 与 undo/redo 完整闭环；
- 检查点 C：验证连接创建/重连、bendpoint、zoom/scroll 与 auto-expose；
- 最终验收：验证保存加载及 Native/Web/Headless 等价。

## 固定执行顺序

```text
G0 architecture and evidence
-> G1 model transaction
-> G2 model-view projection
-> G3 interaction ownership
-> G4 basic editing
-> G5 connection editing
-> G6 product qualification
```

不得先做拖拽 demo，再从 app 代码反推 Command、PartTree 或 Viewer 契约。

## G0 门禁

- [x] 记录 Draw2D/GEF 源码分析和当前实现证据；
- [x] 接受独立 Editor crate 与所有权边界；
- [x] 定义目标模块、身份域、生命周期和输入仲裁；
- [x] 建立 GEF API family 覆盖账本；
- [x] 创建 `novadraw-editor` 单 crate 目录骨架；
- [x] 通过 workspace fmt/check/clippy/test；
- [x] 完成差异复核并记录验证结果。

G0 验证结果：

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
new-document local link check: PASS
git diff --check: PASS
```

## G1 完成记录

- [x] `ModelId` 由应用定义，要求 Copy/Eq/Hash/Debug，不使用 FigureId；
- [x] `ModelRevision` 非零、单调且不回绕；
- [x] `ModelEvent` 保存 revision、subject 和 typed payload；
- [x] `ModelAdapter` 提供 root、稳定 child order 和 ordered event drain；
- [x] `Command` 只操作应用模型，支持 canExecute/canUndo/canRedo 及显式拒绝；
- [x] `CompoundCommand` 固定执行顺序、逆序 undo 和可恢复补偿；
- [x] `CommandStack` 支持 execute/undo/redo、redo flush、undo limit、event journal；
- [x] dirty/save 使用历史状态身份，不使用栈长度推断；
- [x] rejected/recoverable failure 保留 history，未知状态、补偿失败和 panic 使 stack faulted；
- [x] faulted stack 拒绝后续编辑、flush 和 save marking。

验证入口：

- `novadraw-editor/tests/g1_model_contract.rs`；
- `novadraw-editor/tests/g1_command_stack_contract.rs`；
- `doc/verification/reviews/g1-model-command-completion-2026-09-13.md`。

## G2 完成记录

- [x] `ModelAdapter::revision` 与有序批次固定 gap、stale 和同 revision 多事件边界；
- [x] `EditPartId` 使用 Viewer namespace + generational key，跨 Viewer 与退休句柄失效；
- [x] `PartTree` 独立维护 controller containment，不复用 ModelId 或 FigureId；
- [x] `EditPartBehavior` / `EditPartFactory` 固定对象安全扩展边界；
- [x] `GraphicalViewer` 拥有 model、factory、PartTree、registry 与 Runtime；
- [x] 初始递归投影和增量 reuse/reorder/create/remove/reparent 已闭合；
- [x] primary Figure、content pane 和 ancestor visual owner 查询已闭合；
- [x] duplicate model、revision gap/stale 与扩展失败进入结构化拒绝或 fault；
- [x] remove、Viewer drop 与重建身份生命周期已验证。

连接模型的 source/target 双向发现、connection part 去重和 connection layer 挂载统一在
G5 闭合；G2 不在 root layer 尚未建立时创建临时连接投影。

验证入口：

- `novadraw-editor/tests/g2_viewer_projection_contract.rs`；
- `doc/verification/reviews/g2-viewer-projection-completion-2026-09-13.md`。

## G3 自动门禁记录

- [x] `SelectionModel` 支持 replace/append/toggle/remove/clear、primary 与 typed delta；
- [x] EditPart focus 与 Figure keyboard focus 分离；
- [x] visual registry 区分 Part、Handle 与 Feedback；
- [x] targeting 支持 handle 优先、feedback 穿透、part ancestor 与 contents fallback；
- [x] root/scalable/printable/primary/connection/handle/feedback layer 顺序固定；
- [x] part 删除同步 reconcile selection、focus 和 owned overlay；
- [x] `DispatchOutcome` 公开 target、handled 与 capture；
- [x] Figure widget consumed/capture 阻止 Editor selection fallback；
- [x] `node-editor-demo` Native 窗口启动并完成截图级非空验证；
- [ ] 检查点 A 人工验收。

自动验证入口：

- `novadraw-scene/tests/p2_dispatch_outcome_contract.rs`；
- `novadraw-editor/tests/g3_selection_contract.rs`；
- `novadraw-editor/tests/g3_viewer_interaction_contract.rs`；
- `doc/verification/reviews/g3-selection-targeting-behavior-2026-09-13.md`。

人工验收入口：

- `cargo run -p node-editor-demo`；
- `doc/verification/manual/g3-selection-targeting.md`。

## 最小毕业场景

`apps/native/node-editor-demo` 是计划中的产品验证入口，不是架构真源。它必须由
`novadraw-editor` 公共 API 构建，至少覆盖：

1. 加载一个带节点和连接的模型；
2. 单选、多选和 primary selection；
3. 创建、移动、resize 和删除节点；
4. 创建与重连连接；
5. 每项操作 undo/redo；
6. Viewport 滚动、缩放和拖拽 auto-expose；
7. 保存、加载并验证模型等价；
8. Native、Web、Headless 使用同一编辑事务。

## 明确后置

- Palette UI；
- direct text edit、caret 与 IME；
- clipboard 与系统 DnD；
- ruler、guide、snap-to-geometry；
- TreeViewer；
- Eclipse Workbench/JFace 兼容；
- 协同编辑和分布式 history；
- 稳定公共 crate 发布承诺。

## 变更规则

- 新增或修改 GEF 能力时先更新 `api-coverage.md` 的 Family ID。
- 涉及身份、Command 原子性、输入仲裁或模型所有权时，必须先评审设计。
- 若发现 Draw2D Core 缺口，建立独立 P2 delta 和 contract probe，不重开 M1-M10。
- 每个里程碑状态只在本文更新。
- 详细步骤见 [`implementation-plan.md`](implementation-plan.md)。
