# 审计整改执行总清单

类型：`verification`

日期：2026-10-09

状态：`active`

## 1. 用途与边界

本清单汇总现行审计中仍需执行、验证或保留到发布阶段的事项。它用于回答：

- 当前有哪些审计问题仍未关闭；
- 重复发现合并到哪个执行批次；
- 每个批次开始、实施、验证和关闭时必须检查什么；
- 哪些历史审计已经形成稳定基线，不应重复启动。

本清单不建立新的架构或里程碑状态真源：

- 执行状态以
  [`p2-delta-backlog.md`](../../roadmap/p2-delta-backlog.md) 和
  [`goal-alignment-adjustment-plan-2026-09-30.md`](../../roadmap/goal-alignment-adjustment-plan-2026-09-30.md)
  为准；
- 目标契约以 accepted ADR、`design/` 和 parity ledger 为准；
- 历史事实与证据以原审计报告和 completion record 为准；
- 工作方式遵循
  [`audit-remediation-workflow.md`](../audit-remediation-workflow.md)。

汇总规则：

1. 同一根因只保留一个执行批次，旧 ID 作为来源映射；
2. 新审计覆盖旧审计的当前判断，但不改写旧报告；
3. 功能缺口、架构缺陷、性能证据和平台资格分开管理；
4. 同一时刻最多一个审计整改批次处于 `in_progress`；
5. 实施中发现的新问题先进入 hold，不隐式扩大当前批次。

## 2. 当前执行队列

| 顺序 | 批次 | 优先级 | 当前状态 | 合并的审计发现 | 设计入口 |
|---|---|---|---|---|---|
| 1 | P2-S02 显式领域导入与 Root 收口 | P0 | `complete` | R02、API-F07、API-08、A04、B05、TC-13 follow-up | [ADR-035](../../adr/adr-035-explicit-domain-imports-and-root-boundary.md) |
| 2 | P2-S03 Runtime 子系统 Facade | P0 | `complete` | R01、API-F03/A06 的 concrete facade 后续 | [ADR-033](../../adr/adr-033-runtime-subsystem-facade.md) |
| 3 | P2-S04 Editor 开放协议 | P0 | `complete` | R05、TC-06、R08 暴露面 | [ADR-034](../../adr/adr-034-editor-tool-request-handle-protocol.md) |
| 4 | P2-S05 深层布局/presentation 收敛 | P0 | `not_started` | UML F-01 | 待补 normative design 或缺陷契约 |
| 5 | P2-S06 Editor prepared publication | P0 | `not_started` | R06、TC-08 | 待补 ADR |
| 6 | P2-S07 查询身份与失败语义 | P1 | `not_started` | R04、TC-09、API-F06、API-07 | 待补 ADR |
| 7 | P2-S08 Detached 受检值 | P0 | `not_started` | R03、TC-03、API-F04、API-10 | 待补 ADR |
| 8 | P2-S09 服务失败与 Inspector | P1 | `not_started` | R07、TC-10、API-F08、API-F10、API-F09 register alias 后续 | 待补 ADR |

执行顺序默认按表推进。只有依赖、用户优先级或新的 P0 正确性证据发生变化时才调整；
调整必须先更新 Backlog。

## 3. P2-S02：显式领域导入与 Root 收口

### Design

- [x] ADR-035 修订 1：删除库级 prelude，root 只公开领域模块。
- [x] 明确每个公开符号只有一个规范领域路径。
- [x] 明确应用本地 prelude 合法，但不能成为框架稳定 API。
- [x] 评审并接受 ADR-035。

### Execute

- [x] 补齐 `graphics::Color`、`figure::FigureStyle`、`tree::FigureId` 等规范领域导出。
- [x] 迁移 Core 外其他公开 crate 到显式领域路径。
- [x] 迁移 examples/support、Native/Web examples 与 benchmarks。
- [x] 迁移 tests、README 和 book。
- [x] 删除 `novadraw/src/prelude.rs` 与 `pub mod prelude`。
- [x] 删除 root 的 struct/enum/trait/function/constant/macro 重导出。
- [x] 保留必要的 `pub(crate) use`，不把内部便利 alias 重新公开。

### Verify / Close

- [x] API snapshot 删除 prelude scope。
- [x] root snapshot 只包含 module。
- [x] source gate 禁止 root 非 module 项和 `novadraw::prelude`。
- [x] 每个禁止 root 名称有独立 compile-fail 证据。
- [x] 外部 consumer 只用显式领域路径。
- [x] `cargo xtask check --quick` 通过；最终组合交付 full gate 待执行。
- [x] 新建 P2-S02 completion record，关闭 R02/API-F07。

## 4. P2-S03：Runtime 子系统 Facade 与统一 Figure Mutation

### Design

- [x] ADR-033 修订 1：Runtime 保持唯一 owner/commit authority。
- [x] 领域服务拆为无状态短借用 facade。
- [x] 删除内置 concrete Figure facade 特权。
- [x] 内置与外部 Figure 共用 typed component/capability mutation。
- [x] 评审并接受 ADR-033。

### Execute

- [x] 引入 Frames、Resources、Connections、Input、Focus、Tooltip、
  Accessibility、Animation、Observations、Text facade。
- [x] 每个 facade 只保存 `&mut Runtime`，Drop 无行为。
- [x] 逐 crate 迁移现有调用点。
- [x] 将 `label/image/text_flow/viewport/...` 调用迁到
  `FigureMut + FigureComponentUpdate/CapabilityKey`。
- [x] 删除 Runtime 本体旧公开入口和 concrete facade。
- [x] Runtime 本体公开方法数收敛到不超过 25。

### Verify / Close

- [x] 旧 Runtime 平铺方法和 concrete facade 有独立 compile-fail 证据。
- [x] 外部 Figure component update 与 capability update 共用内置提交链。
- [x] 各子 facade 分别进入 API snapshot。
- [x] Core、Editor、Native/Web platform 和 examples suite 通过。
- [x] quick gate 通过；full gate 随 P2-S02/S03/S04 组合交付执行。
- [x] 新建 P2-S03 completion record，关闭 R01。

## 5. P2-S04：Editor Tool / Request / Policy / Handle 统一开放协议

### Design

- [x] ADR-034 修订 1：公开 request 始终类型化，擦除仅限 registry 内部。
- [x] Request metadata 与 payload 分离。
- [x] EditPolicy 通过 install-time builder 注册 typed handler。
- [x] 内置 `start_*` hook 与闭合 PolicyRole 一并删除。
- [x] Tool/Handle/Policy 内置实现与外部实现采用相同 key/registration。
- [x] 评审并接受 ADR-034。

### Execute

- [x] 建立 `RequestProtocol<A>`、typed RequestKey、RequestMetadata 和私有 erased registry。
- [x] 建立 typed PolicyKey/PolicySlot，删除 PolicyRole/Custom。
- [x] 迁移一个内置 request family，验证 contribution/reducer 模型。
- [x] 迁移其余 request，删除 EditorRequest 和三个专用 `start_*` hook。
- [x] 引入 object-safe Tool/ToolContext，迁移四个内置 Tool，删除 ActiveTool。
- [x] 引入 HandleActionProtocol，迁移内置 action，删除 HandleRole。
- [x] 收口 VisualOwner/HandleId 等 overlay 注册内部类型（R08 暴露面）。

### Verify / Close

- [x] 外部 Marquee Tool。
- [x] 外部 AlignToGuide RequestProtocol + Policy handler。
- [x] 外部 GuideHandle action。
- [x] source gate 禁止 Core match 内置 request/tool/handle/policy。
- [x] duplicate、missing handler、rejection、descriptor mismatch、panic fault 测试。
- [x] G1-G5、P2-E02、定向 suite 与 quick gate 通过。
- [x] P2-S02/S03/S04 组合交付 full gate 通过。
- [x] 新建 P2-S04 completion record；关闭 TC-06/R05，并关闭 R08
  overlay 暴露面。TC-08/R06 的 prepared publication 仍归 P2-S06。

## 6. P2-S05：深层复合布局与文字 Presentation 收敛

来源：
[UML 扩展能力验证 F-01](../reviews/uml-extension-capability-2026-10-05.md#f-01深层复合布局与文字-presentation-未稳定收敛)。

- [ ] 提取两层以上嵌套 Toolbar/Layout + Label 的最小复现。
- [ ] 证明问题位于 layout、presentation snapshot、clip/transform 或稳定化重启中的哪一层。
- [ ] 定义“同一稳定帧使用最终 geometry”的契约。
- [ ] 修复 Runtime 派生状态收敛，不在应用层重复 refresh。
- [ ] Headless command、Native/Vello 像素和 damage 回归通过。
- [ ] 记录工作量，确认没有无依据扩大稳定化扫描。
- [ ] completion record 关闭 UML F-01。

## 7. P2-S06：Editor Prepared Publication

来源：R06、TC-08。

- [ ] ADR 定义 `PartProjectionPlan`、`ConnectionProjectionPlan` 和 `OverlayBatch`。
- [ ] 明确 prepare/validate/commit 的状态所有者。
- [ ] Part/Connection 创建不再依靠跨 registry 手工补偿。
- [ ] Overlay Drop 只做不可失败的本地撤销，显式 commit 后转交 Viewer。
- [ ] 外部 callback 副作用明确排除在回滚保证之外。
- [ ] fault injection 验证失败前后 Runtime、PartTree、registry、anchor 和 overlay 快照一致。
- [ ] 与 P2-S04 的 typed Tool/Handle ownership 集成。
- [ ] completion record 关闭 TC-08/R06。

## 8. P2-S07：结构查询身份与失败语义

来源：R04、TC-09、API-F06、API-07。

- [ ] ADR 定义统一 QueryError 分类。
- [ ] `Option` 只表示已验证对象上的合法缺失。
- [ ] unknown/foreign/stale identity 返回结构化错误。
- [ ] LayoutSnapshot 不返回 ZERO、INFINITY 或 default 伪造成功。
- [ ] `is_visible/is_enabled` 不把 unknown 折叠成 false。
- [ ] 删除或改名非权威查询，避免 FigureNode/FigureTree 同名不同语义。
- [ ] 保留坐标不可逆/无路径等真正可选语义，不机械把全部 Option 改为 Result。
- [ ] foreign/stale/absent/fault 负向测试和外部 LayoutManager consumer 通过。
- [ ] completion record 关闭 TC-09/R04/API-F06/API-07。

## 9. P2-S08：Detached Figure / Border / Style 受检值

来源：R03、TC-03、API-F04、API-10。

- [ ] ADR 或 normative design 定义 `Alpha01`、非负长度、圆角、stroke 和字体值。
- [ ] Figure、Border、Style 公共字段改为私有 accessor。
- [ ] detached constructor 在对象发布前返回 Result。
- [ ] 删除 Border 构造 assert/panic alias。
- [ ] 删除用于隐藏非法输入的 silent clamp。
- [ ] admission 只保留跨对象、namespace 和 topology 最后防线。
- [ ] 内置与外部 Figure 使用相同值类型和失败语义。
- [ ] 构造负测、序列化边界和渲染回归通过。
- [ ] completion record 关闭 TC-03/R03/API-F04/API-10。

## 10. P2-S09：服务失败、Inspector 与错误 Source

来源：R07、TC-10、API-F08、API-F10，以及 API-F09 未覆盖的 connection register alias。

- [ ] ADR 统一 query、check、mutation 和 idempotence 返回语义。
- [ ] AnimationMut 的 fault 与 `Ok(false)` 幂等未变化分离。
- [ ] Inspector 锁 poison 返回结构化 fault，不再返回空集合或静默跳过。
- [ ] Inspector attach 返回 subscription token/guard，明确 detach 生命周期。
- [ ] `register_*` 只保留 fallible 入口，删除 `try_*` 双入口和 panic alias。
- [ ] Command/Policy/extension error 保留 `source`，不压缩为 String/label。
- [ ] fault injection 覆盖 Animation、Inspector、registration 和 extension panic。
- [ ] 与 P2-S03 Runtime facade 的 Observations/Connections 边界一致。
- [ ] completion record 关闭 TC-10/R07/API-F08/API-F10。

## 11. 审计派生的能力 Backlog

这些是已确认能力缺口，不等于当前实现缺陷。状态继续以 P2 Backlog 为准。

| Delta | 状态 | 审计来源 | 剩余验收 |
|---|---|---|---|
| P2-G01 Graphics extension | `in_progress` | GOAL-CAP、Graphics 审计 | Native surface partial repair 人工验收 |
| P2-M01 Animation/Presentation | `in_progress` | Animation 架构审计 | M01-D 平台/视觉发布证据 |
| P2-L01 Directed/Compound layout | `not_started` | GOAL-CAP、UML F-02 | 独立图模型、确定性输出、外部算法 consumer |
| P2-C03 跨 viewport connection | `not_started` | 能力分母、UML 未覆盖边界 | common viewport、clip、damage、hit-test |
| P2-A01 平台 accessibility | `not_started` | GA-3/GOAL-PORT | Web action/focus、Native AT provider |
| P2-T03 fragment style | `deferred` | UML F-03 | measurement/paint/interaction 同一 snapshot |
| P2-W01 widgets | `deferred` | 能力分母 | repeat scheduler、radio/checkbox/slider |
| P2-O01 output target | `deferred` | 能力分母 | printing/export、scale/clip/text/image 等价 |
| P2-LC01 live unmount/mount | `deferred` | 生命周期审计 | 真实 consumer、ID/资源/退出路径 |

## 12. 发布准备证据

这些事项不阻塞平台无关 Core API 开发，但阻塞总体发布资格。

### GA-2 性能

- [ ] compositor present 延迟。
- [ ] input-to-present 延迟。
- [ ] Chrome WebGPU 固定场景预算。
- [ ] validation/routing 与 Draw2D 同机同场景复核。
- [ ] 首帧重复录制、Label/TextFlow 深链祖先扫描等候选先测量再决定优化。

### GA-3 平台

- [ ] Windows 原生 runner。
- [ ] Linux X11 原生 runner。
- [ ] Linux Wayland 原生 runner。
- [ ] Firefox/Safari 支持等级复核。
- [ ] Native accessibility provider（由 P2-A01 承接）。

### Web 文本输入

- [ ] 真实系统 IME composition 与候选窗人工验收。
- [ ] trusted keyboard 触发 EditContext `textupdate`。
- [ ] `textformatupdate` composition 样式是否进入平台无关反馈协议。

## 13. Hold / 待定向复核

以下条目有历史证据，但尚不足以直接进入实现：

- [ ] 首帧 Full promotion 是否仍重复录制：归 GA-2，先建立当前版本 probe。
- [ ] Label/TextFlow 深链祖先扫描是否构成预算问题：归 GA-2，先测 p50/p95 与访问量。
- [ ] 外部 LayoutOutput 非有限值预检：在 P2-S08 Design 时判断属于值不变量还是
  Layout contract。
- [ ] 浏览器 composition 去重：只有真实事件序列证明重复写回后才升级为缺陷。

Hold 项不得顺手并入正在执行的批次；先完成复现和 Triage。

## 14. 已关闭基线

以下结论不应重复启动；若出现新反例，应登记新 finding 并引用旧完成证据。

| 来源 | 已关闭内容 |
|---|---|
| Core pipeline CP-01~CP-05 | damage、递归深度、callback invalidation、clip state |
| Draw2D/GEF 语义审计 | 2026-09-16 与 2026-09-20 两批整改完成 |
| Goal audit GA-0/1/4/5/6 | 能力分母、确定缺陷、模块扩展、文档门禁、发布审计 |
| Goal audit B01~B05 | 首帧 validation、Web wheel、DOM blur borrow、Policy panic、独立负向导入 |
| TC-01 | Graphics/PaintContext/CommandRecorder 单入口 |
| TC-02 / TC-05 | Figure capability 与 Animation binding 开放协议 |
| TC-04 | Typed PropertyKey 与 erased journal |
| TC-07 | ViewerEditSession、synchronize_model、ViewerDrive |
| TC-11 | ResourceProvider 与 request token |
| TC-12 | RenderBackend 结构化运行失败 |
| TC-13 / TC-14 | 分层 API snapshot 与热路径日志门禁 |
| P2-C01 connection.locator | decoration orientation/reference 与 locator 已验证 |
| M1-M10 / G0-G5 | 既定 Core 与 Editor milestone 已完成 |

## 15. 每个批次的固定关闭动作

- [ ] 当前 batch 的 ADR/normative design 已接受。
- [ ] Backlog 只有该 batch 为 `in_progress`。
- [ ] completion criteria 在 Execute 前冻结。
- [ ] 新发现已登记为 hold，没有隐式扩大范围。
- [ ] 定向 suite 通过。
- [ ] `cargo xtask check --quick` 通过。
- [ ] 对公共跨 crate API、高扩散或里程碑边界执行一次 full gate。
- [ ] 新建 completion record。
- [ ] 回填原审计 ID、Backlog 状态和文档索引。
- [ ] 下一 batch 才可进入 `in_progress`。
