# Group 3：扩展契约、模块与 API 一致性

scope: full_file

## 审计边界与结论

- 日期：2026-09-30。REPO_ROOT：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- PROJECT_ROOT：`engine/wasm_rust/novadraw`。下文位置默认相对此目录；JSONL 的 `file` 相对 REPO_ROOT。
- 引用缩写：`figure/`、`layout/`、`graph/`、`runtime/`、`connection/`、`render/`、`geometry/` 前缀为 `novadraw/src/`；`tests/` 前缀为 `novadraw/`；`part/`、`viewer/`、`command/`、`domain.rs` 前缀为 `novadraw-editor/src/`。设计短文件名位于 `doc/design/architecture/`，Editor architecture 位于 `doc/design/editor/`，ADR 编号对应指定的 `doc/adr/` 文件。
- SKILL_ROOT：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- WORK_DIR：`verification/evidence/goal-alignment-audit-2026-09-30`。本次只交付 Group3，不覆盖总报告或其他组。
- 初始审计清单基线为 `811b748f0a510edb90ad198346950ca3752b404d` 加用户 dirty P2-E02；研究期间其他操作提交了这些改动。交付复核 HEAD 为 `841be423815833d97c19862da7b1b0a8ad05591f`，受审实现已提交；并行产生的其他文档与证据不在本组写入范围。未修改、回退或提交用户实现。
- 已按 AGENTS → CLAUDE → 指定 ADR/设计 → 实现的顺序研究。读取 skill 的 general-workflow、review-dimensions、review-rule；Rust 无额外语言专项规则。
- 这是指定边界的完整函数/接口与直接调用方审查，不是全仓逐行穷尽。候选不受 diff 行限制，不把历史删除代码当缺陷；不递归扩展追踪无关间接调用方。
- 确定代码问题：P0 = 0，P1 = 1，P2 = 1。仅这两项进入 `group_3.jsonl`。未运行复现程序，置信度来自静态可达路径，不代表已通过运行时验证。
- 总体：Core 的 Figure、LayoutManager、Router、TextLayoutEngine 确实有可用外部实现入口；不能据此宣称所有扩展场景、所有生命周期阶段均已开放。Editor/Inspector 的独立安装方向成立。未取得性能对比数据，不能证明速度已超过 Draw2D。

## 先确立的目标契约

用户目标是 Rust 能力对等或超过 Draw2D，尤其性能、扩展与跨平台，而不是复制 Java 类结构。

| 契约 | 规范证据 | 本组判据 |
|---|---|---|
| 外部私有状态可更新 | ADR-014:27-49；component-update.md:17-105 | 外部 Figure 不修改 Runtime 类型分支；prepare/commit、revision、失效及错误可达 |
| 失败与发布分层 | ADR-014:51-85；Editor architecture.md:329-346 | 源操作失败、派生失败、stable publication、presentation ack 不混同；扩展 panic 后明确 fault |
| 阶段与写入权威分离 | ADR-017、018、019 | detached 直接构造；Builder 写树；attached editor 借用 Runtime；查询不取得可变树 |
| Layout/Router 真正替换 | static-architecture.md；connection-routing.md | 自定义算法消费只读输入，返回受校验输出，不持有可变树或维护第二套拓扑 |
| 文本服务与后端独立 | text-layout.md:43-76,133-138,177-190 | Runtime 注入文本服务；外部能构造非空 glyph 快照；不靠 Vello 或系统字体兜底 |
| 模块与可选 package | ADR-023:29-80；directory-structure.md:188-220 | Core 不依赖 Editor/Inspector/platform/backend；内部依赖与声明分别核验 |

Draw2D 对照只使用 `gef-classic/org.eclipse.draw2d` 和 `org.eclipse.gef`，未使用 Zest。`LayoutManager.java:21-79` 直接围绕 IFigure 测量/布局，`ConnectionRouter.java:21-63,83-96` 直接修改 Connection；Novadraw 的 snapshot/output 与 Runtime 单一提交是已接受的 Rust 迁移，不以不同方法名判错。`IFigure.java:959` 的布局策略设置、`AbstractGraphicalEditPart.java:316,465-475` 的可替换 Figure 创建也不等价于“只能实例化内置类型”。

## 确定代码缺陷

### G3-01 [P1][健壮性] Policy 扩展调用未进入 Viewer fault 边界

- 主位置：`novadraw-editor/src/viewer/mod.rs:2102-2105`；置信度 **9/10**；条件性功能缺陷。
- 契约：`doc/design/editor/architecture.md:345-346` 要求扩展 panic 后 Editor faulted；不是要求回滚任意用户副作用。
- 完整上下文：`command_for_request:2089-2121` 直接执行 policy；`resolve_policy_targets:2066-2085` 和 `resolve_policy_target:2048-2064` 也直接执行 target，没有 catch/fault guard。
- 接口证据：`novadraw-editor/src/policy/mod.rs:166-191` 的 `command`、`feedback` 接收 `&mut self`，可以改变扩展内部状态。不是类型系统保证的纯函数。
- 直接调用方：`novadraw-editor/src/domain.rs:418-433` 在 `command_for_request` 返回后才调用 CommandStack。后者 `command/mod.rs:399-447` 的 catch 只覆盖 Command 自身，不能捕获尚未产生 Command 的 policy panic。
- 具体触发：外部 policy 的 `target` 返回当前 active part，`understands` 为 true，`command` 修改自己的状态后 panic；宿主在顶层 catch unwind。Viewer 的 `faulted` 仍为 false，CommandStack 尚未运行，后续 `model_mut` 或另一次请求可以继续使用这个未知状态的扩展。无需模型脏写或 unsafe。
- 同根边界缺口：`show_feedback_for_request:2124-2151` 也直接执行扩展；这些入口未调用 `ensure_ready`。已因 revision gap/refresh panic faulted 的 Viewer，仍可经该入口生成 feedback，`add_feedback_visual:1376-1394` 也不检查 Viewer fault。
- 对照：`refresh:2486-2495` 已有 catch 并置 fault；`ensure_ready:2603-2608` 已存在。`tests/g2_viewer_projection_contract.rs:488-513` 验证 refresh panic 后只禁止 refresh/model_mut，未验证 policy 入口。

问题片段：

```rust
for policy in roles.values_mut() {
    if policy.understands(request)
        && let Some(command) = policy.command(host, request, &self.model)?
    {
        commands.push(command);
    }
}
```

目标影响：可组合扩展的失效隔离不完整。不是“任意 panic 必须恢复执行”，而是已选择 unwind/fault 协议时不能继续标为可用。

修复方向：统一保护 Viewer 的公开 policy 计算/feedback 入口，先检查 fault，在扩展 panic 时先记录 fault，再执行适当清理并传播 panic；不要仅依赖 CommandStack 的保护。

最小验证建议：外部 EditPolicy 的 command panic 后检查 Viewer fault、model_mut/再次请求/feedback 均拒绝；再用已有 revision-gap fixture 检查 faulted Viewer 不产生新 overlay。仅建议，本次未写或运行测试。

### G3-02 [P2][健壮性] LayoutOutput 预检丢弃 bounds，非法几何可进入 NodeState

- 主位置：`novadraw/src/graph/mod.rs:2241-2245`；置信度 **10/10**；防御性边界缺陷，保守定 P2。
- 契约：ADR-014:46-58 要求候选校验与整份 LayoutOutput 发布；布局错误已有 `LayoutError::NonFiniteGeometry`（`layout/mod.rs:157-180`）。不能将私有组件的自校验责任推广为引擎结构几何可免检。
- 输出接口：`layout/mod.rs:248-263` 允许外部 layout 添加任意 Rectangle；Rectangle 的四个 f64 字段公开（`geometry/rect.rs:80-95`）。
- 完整预检：`graph/mod.rs:2236-2281` 对 Bounds 使用 `_`，仅验证 child 身份/parent；没有有限值或非负尺寸检查。
- 直接消费方：`apply_layout_output:2283-2331`、`apply_layout_output_without_update:2333-2373` 在同一预检后逐项写入；`set_bounds:4434-4477`、`set_bounds_with_update:4491-4531` 也不验证数值，最终 `set_node_bounds:515-517` 直接赋值。
- 具体触发：自定义 LayoutManager 对一个合法直接 child 输出 `Rectangle { x: 0.0, y: 0.0, width: -1.0, height: 10.0 }` 并返回 Ok；Builder `validate_subtree` 经 `try_revalidate_body:2195-2233` 返回成功且 child 保存负尺寸。换为 NaN/Infinity 同样不会在输出预检被拒绝。运行期也复用该校验函数。
- 影响不依赖完整帧最终是否因其他检查被拒绝：非法值已进入可查询 NodeState，输出边界没有返回约定的 LayoutError；不存在将原 bounds 自动回滚的代码。
- 定级理由：本次未证明正常内置 layout 当前会产生此输入，不升级为 P1/P0；已确认的是第三方输出防御不足，不宣称生产场景必现。
- 现有测试：`tests/m5_layout_contract.rs:403-478` 的 invalid-output case 仅以 root 冒充 child，验证身份原子性，没有非法几何维度。

问题片段：

```rust
LayoutChange::Bounds(child_id, _)
| LayoutChange::Visibility(child_id, _)
| LayoutChange::Invalidate(child_id) => Some(*child_id),
```

目标影响：第三方布局可能把坏几何带进布局、命中和 damage 的共同状态，扩展与引擎之间的校验边界弱于 RouteOutput/TextLayout。

修复方向：在任何输出写入前完整验证每个 bounds 的有限性与尺寸合法性，非法候选整份拒绝；保留源输入与派生输出的不同原子性层级。

最小验证建议：一个 LayoutOutput 先含合法 child 更新、再含另一合法 child 的 NaN/负尺寸，验证两个 child 均保持旧 bounds 且返回 LayoutError；同时覆盖 Builder 与 Runtime 路径。本次未写或运行测试。

## 已对齐证据

| 项目 | 实现与调用证据 | 能证明什么 / 不夸大的边界 |
|---|---|---|
| 自定义 Figure/Shape | `figure/mod.rs:435-697,919-998`；`tests/d4_component_update.rs:46-96,191-304` | Figure 无 Send+Sync 强制要求，没有 Shape→Figure blanket impl；外部 Badge 的非空私有内容可更新，不改 Runtime 分派 |
| 组件提交 | `runtime/mutation/mod.rs:10-88`；`runtime/runtime.rs:2022-2125` | default invalidation 保守；类型、身份、revision 容量先验证；prepare/commit 在 guard 内。private candidate 校验仍是 provider 责任 |
| 生命周期与 fault | `runtime/runtime.rs:2127-2201`；`tests/d4_component_update.rs:307-410` | dispose 提取 retired 后清关联再完成生命周期；组件 prepare/commit panic 有拒绝后续提交测试。不是所有扩展回调已穷尽证明 |
| Layout 扩展 | `layout/mod.rs:91-155,306-359`；`runtime/runtime.rs:2329-2343,2367-2385` | snapshot 只读，output 有限写集；默认 constraint 拒绝而非默认成功；manager replacement 先校验已有 constraints |
| 约束测量 | `tests/d4_constrained_measurement.rs:1-241`；`graph/mod.rs:2400-2441` | 外部 Figure + Layout 的宽度/baseline/glyph 复用测试真实存在；但仅覆盖预制 natural 与一个 constrained 快照 |
| Router/Anchor | `connection/router.rs:60-90,119-204`；`connection/runtime.rs:703-815,1008-1099,1167-1188` | 公开纯 route 接口、动态 SceneQuery、受检 RouteOutput、constraint TypeId、稳定 group 输入；不需持有 FigureTree |
| Router 提交与恢复 | `runtime/runtime.rs:1409-1588`；`tests/m9_connection_runtime.rs:311-391` | 整批几何/locator 预检在提交前；失败清空 route、保留恢复依赖，不伪装成旧路线成功 |
| 外部 Router 用例 | `novadraw-editor/tests/g5_connection_creation_contract.rs:33-105` | 外部组合 router 可在 base route 上产生非空自环 route；并非只编译空实现。注册及验收函数已检索定位，未重跑 |
| TextLayoutEngine | `runtime/runtime.rs:753-794,5128-5221`；`render/text.rs:645-689,1133-1151`；`tests/m10_text_extension_contract.rs` | Runtime 独占注入；公开受检 parts 可产非空 glyph；非 Parley 与非 Vello 消费分别有测试 |
| Scoped editor | `runtime/runtime.rs:317-367`；`tests/api_scoped_editor_contract.rs` | facade 委托同一个 Runtime，foreign/disposed/capability mismatch 有外部测试；不是第二份树 |
| Editor 模型权威 | `novadraw-editor/src/model/mod.rs:136-160`；`command/mod.rs:65-101,399-459,620-650`；`viewer/mod.rs:2498-2534` | Command 操作应用模型；refresh 来自 revision/events；unknown-state/command panic 与 dirty/save 门禁明确 |
| Inspector | `novadraw-inspector/src/lib.rs:52-78,140-163,179-194`；`runtime/runtime.rs:3679-3689` | capture 经 stable_query，外部 observer 只记录；未以开放 mutation 换取诊断能力 |
| 可选依赖 | 六个公开 crate 的 Cargo.toml；`novadraw/src/lib.rs:1-46` | Core 无 Editor/Inspector/backend/platform 依赖；平台 editor feature 默认关闭；backend 不依赖 platform |

本次实际只运行两个只读、offline Cargo tree 查询（直接依赖与正常传递依赖）。Core 正常依赖中 `novadraw-editor/novadraw-inspector/vello/wgpu/winit/web-sys/raw-window-handle` 匹配数 **0**。直接依赖含 image、kurbo、parley、resvg、serde、slotmap、stacker、tracing、unicode-segmentation、uuid。未编译 workspace、未跑单测、未跑 full/quick gate；表中的测试均为源码证据。

产物检查：`jq` JSONL 解析与必填字段/等级/路径前缀校验通过；两项缺陷的源码行号已重新读取核实；本组 Markdown/JSONL 的 `git diff --no-index --check` 无空白错误。该命令的退出码 1 表示新增文件存在差异，不是测试失败。

## 覆盖缺口与结构风险（不进入 JSONL）

### R1：基础 capability 闭集与 scoped editor 增长是已知后续目标

`figure/mod.rs:621-696` 固定知道 connection、point_list、scalable_polygon、text_flow、label、clickable；label 更返回具体 LabelFigure。`runtime/runtime.rs:370-450` 在通用 FigureEditor 中组织内置 mutator。第三方实现这些已有能力或通过 typed update 更新自身并未被封死，但引擎新增一种横切能力仍需修改 trait。

`doc/verification/reviews/engine-capability-assessment-2026-09-28.md:118-132,159-169` 已把 E3b/E4 列作后续。不能把大文件长度、内置方法多或没有动态 plugin registry 直接报 bug。目标影响是接口演进与 downstream 适配成本，不是已有 API 不执行。建议以“新增外部 Figure 无需改 Runtime”和“新增内置 capability 触及哪些协议”分别计量。

### R2：外部更新贯通不到全部入口

- Editor：`part/mod.rs:974-1020` 的 VisualUpdateContext 仅有 primary bounds/style/label text；没有 update_component、任意 owned visual scoped editor 或 TextFlow page 更新。`viewer/mod.rs:3398-3410` 直接用它执行 refresh。自定义 Badge 私有文本或 compound visual 内部 child 的模型属性不能沿该 context 更新。
- Core callback：`runtime/context.rs:196-325` 和 `runtime/mutation/mod.rs:258-325` 的 FIFO mutation 是闭集，没有 typed component update 分支；外部 event handler 拿不到可变 Runtime，不能直接复用 FigureEditor::update_component。ADR-014:38 的 callback owned update 声明因此没有完整外部组件证据。
- 不能用“应用持有 RefCell 自行改 Figure”来算完成，这会绕过提交权威；应用另排队再通过 viewer.runtime_mut 调用也不等于标准 refresh/event 链路已覆盖。

分类：现行扩展协议的集成覆盖缺口，而非某个已有 setter 的确定执行错误；不混入 defect JSONL。建议先明确是否承诺 Editor/callback 的任意组件更新，再以一个外部 compound Figure 的模型通知和按键更新闭环验收。

### R3：通用私有派生快照仍未等于内置文本能力

`component-update.md:99-105` 明确把布局后依赖 bounds 的 typed derived component 协议留待后续；`FigureLifecycle::validate`（`figure/mod.rs:860-872`）只接收 bounds，不能访问 Runtime 独占文本服务或返回结构化候选。`LayoutOutput` 公开变更仅 bounds/visible/invalidate（`layout/mod.rs:248-263`）。

现有 constrained measurement 测试在挂载前手工生成两个 TextLayout，不能证明外部 Figure 对任意动态宽度、字体 revision 都能使用该服务并发布私有快照。`BorderSnapshot` 的 kind 与构造器封闭在 TitleBar/Compound（`figure/border/mod.rs:26-103`），静态 Border 可扩展，不等于第三方 owner-dependent snapshot 任意可构造。

分类：明确后续协议及覆盖不足，不作为当前实现 bug。建议用三次不同宽度加字体 revision 变化的外部文本组件验证需求，不以一次固定宽度 fixture 宣称全覆盖。

### R4：package 方向成立，不等于内部 module 分层声明成立

`directory-structure.md:194` 禁止 FigureTree 依赖 UpdateManager，但 `graph/mod.rs:33-36,1716-1743,2020-2087,2283-2331` 直接接收并驱动 UpdateManager。`figure/mod.rs:62` 通过 crate root 引入实际定义于 runtime 的 EventContext 与输入事件类型。`layout/mod.rs:23-25,225-240` 依赖内置 viewport effect 与 runtime notification PropertyValue；这些都是实际源码依赖，不能因合并 crate 或 re-export 改名视为消失。

同时 static architecture 的概念接口不保证字段/模块名，ADR-018 接受 tree 与内部更新协作。因此这里是规范之间及规范与模块责任边界的矛盾，应先裁决“禁止依赖”指稳定协议还是整个实现模块，再决定提取基础协议/协调算法。没有具体错误路径，不按 P0/P1/P2 报告，也不建议为目录整齐重新拆 package。

### R5：可替换不等于最小编译依赖或发布可用性已全验证

- `novadraw/Cargo.toml:10-23` 的 Parley/resvg 等是无条件依赖；外部引擎注入不自动消除默认文本/图像实现的编译与体积成本。无数据不能宣称性能缺陷，也不能宣称零成本替换。
- `scripts/check_facade_dependencies.sh:4-10` 的 forbidden list 没有 Editor/Inspector，现图正确但回归门禁没有覆盖 ADR-023 的所有禁止项。`verification/suites.toml:416-444` 已将它列入 core.facade。
- Editor、Inspector、backend、platform manifests 使用仅 path 的本地 Novadraw 依赖；独立 registry 发布还需版本约束与发布验证。当前 0.1 workspace 可用不等于已证明 crates.io 可发布，本次未运行 package/publish。
- P2-E02 当前 docs 已随用户提交标为 complete；不沿用旧记忆中的 in_progress，不把该批次视作未完成缺陷。本组未复验 Native/Web 人工结果。

分类：依赖/发布/性能证据边界，不进入 JSONL。建议补充更完整的正常依赖禁入门禁，并单独验证发布与替换引擎的构建成本。

## 最小验证优先级

1. 先验证 G3-01 policy panic 与已 faulted Viewer 的再入拒绝，覆盖 command 和 feedback。
2. 验证 G3-02 合法 child 的非法几何整批拒绝，而不仅是错误 child 身份。
3. 定义并验证一个外部自定义 Figure 经 Editor refresh / Figure callback 的内容更新闭环。
4. 按需验证动态宽度与字体 revision 的外部文本派生，不将固定快照 fixture 扩大解释。
5. 最后检查 facade 正常依赖与 package 发布条件；性能专项留给基准测试，不以 API/文件数量代理性能。

以上是后续建议，不是本次执行记录。不写单测，不修改实现；只对本组产物做 JSON、引用行号与空白格式校验。

## 阅读清单与覆盖粒度

### 完整读取

- 启动：AGENTS.md、CLAUDE.md；skill 的 SKILL.md、general-workflow.md、review-dimensions.md、review-rule.md；用户/项目 memory 与 20260930 topics（仅作线索，当前源码/文档优先）。
- 契约：ADR-014、017、018、019、023；design/architecture 下 static-architecture、directory-structure、component-update、figure-lifecycle、connection-routing、text-layout、reusable-shape-border；design/editor/architecture.md。
- 辅助：engine-capability-assessment-2026-09-28.md；WORK_DIR/review_files.md、review_groups.md。
- manifests：根 Cargo.toml；novadraw、novadraw-editor、novadraw-inspector、novadraw-backend-vello、novadraw-platform-web、novadraw-platform-winit 的 Cargo.toml。
- 实现：novadraw/src/lib.rs、figure/mod.rs、layout/mod.rs；novadraw-editor/src/lib.rs、model/mod.rs、part/mod.rs、policy/mod.rs；novadraw-inspector/src/lib.rs。
- 外部测试：d4_component_update.rs、d4_constrained_measurement.rs、m10_text_extension_contract.rs、api_scoped_editor_contract.rs、facade_contract.rs、r8_extension_boundaries.rs。
- 门禁：scripts/check_facade_dependencies.sh、check_public_api_dependencies.sh。
- 第三方：org.eclipse.draw2d 的 LayoutManager.java、ConnectionRouter.java。

### 定向完整函数/片段读取及检索

- runtime/runtime.rs：editor 委托 303-450；构造 750-802；路由 1400-1610；组件更新/销毁/guard 1950-2210；layout replacement/constraint 2300-2395；stable query 3670-3720；稳定化与帧发布 4515-4870；非 Parley 用例 5120-5245。
- runtime/mutation/mod.rs：1-225、245-495；runtime/context.rs：1-110、190-330，另检索全部公开入口。
- graph/mod.rs：495-535、2020-2465、2740-3035、4415-4540；另检索 UpdateManager、lifecycle、measurement 和 geometry 校验调用位置。
- connection/router.rs：1-280；connection/runtime.rs：701-825、1000-1108、1150-1210；其余路由/几何入口只定位，未穷尽算法。
- render/text.rs：624-712、925-969、1120-1168；figure/border/mod.rs：1-290；geometry/rect.rs：1-95。
- Editor viewer/mod.rs：已读本次完整 dirty diff；当前 785-815、1344-1440、2010-2158、2470-2615、3360-3420；结合入口/fault/生命周期符号检索，未逐行穷尽 viewer。
- Editor command/mod.rs：1-200、395-460、620-694；domain.rs：410-462；其余仅入口检索。
- 测试片段：m5_layout_contract.rs:345-478；m9_connection_runtime.rs:292-395；g4_editing_loop_contract.rs:1-280；g2_viewer_projection_contract.rs:488-513；g5_connection_creation_contract.rs:1-105；G1/G2/G5 其余相关测试通过符号检索定位，不算执行或完整阅读。
- verification/suites.toml：30-80、410-447、520-563；roadmap/editor、p2-delta-backlog、parity/gef 针对状态/扩展/fault 检索。
- 第三方 IFigure.java、Shape.java、AbstractGraphicalEditPart.java：仅扩展入口检索，未据此宣称完整 GEF 对等性。

七维筛查结论：逻辑/领域语义用于协议路径核验；健壮性得到上述两项；性能仅记录证据不足；未在已读边界发现可证实的安全或并发缺陷；纯质量/文件长度问题不入缺陷集。
