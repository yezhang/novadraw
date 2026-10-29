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
| 4 | P2-S05 深层布局/presentation 收敛 | P0 | `complete` | UML F-01 | [Completion record](../reviews/p2-s05-deep-layout-presentation-completion-2026-10-12.md) |
| 5 | P2-S06 Editor 增量投影失败隔离与 Overlay 批量校验 | P0 | `complete` | R06、TC-08 | [完成记录](../reviews/p2-s06-failure-isolation-completion-2026-10-15.md)，矩阵修订 2 |
| 6 | P2-S07 查询身份与失败语义 | P1 | `complete` | R04、TC-09、API-F06、API-07 | [完成核验](../reviews/p2-s07-query-identity-completion-2026-10-16.md)，ADR-038 已实现 |
| 7 | P2-S08 Detached 输入不变量 | P0 | `complete` | R03、TC-03、API-F04、API-10 | [ADR-056](../../adr/adr-056-detached-input-invariants.md)已实施；A／B／C／D 完成，见[关闭记录](../reviews/p2-s08-d-completion-2026-10-29.md) |
| 8 | P2-S09 服务失败与 Inspector | P1 | `design-draft` | R07、TC-10、API-F08、API-F10、API-F09 register alias 后续 | [ADR-057 草案](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)，待评审，未实施 |

执行顺序默认按表推进。只有依赖、用户优先级或新的 P0 正确性证据发生变化时才调整；
调整必须先更新 Backlog。

本表为 Backlog 的执行映射；G01/M01/GA-2/GA-3 的 `in_progress` 表示仍待交付证据，
不表示它们与本表同时 Execute。S06 已于 2026-10-15 按矩阵修订 2 完成，
S07／S08 已完成；S09 已形成设计草案，未进入 Execute。
本轮校准记录见[路线图复核](../reviews/roadmap-calibration-2026-10-14.md)。

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
- [x] 删除 `crates/novadraw/src/prelude.rs` 与 `pub mod prelude`。
- [x] 删除 root 的 struct/enum/trait/function/constant/macro 重导出。
- [x] 保留必要的 `pub(crate) use`，不把内部便利 alias 重新公开。

### Verify / Close

- [x] API snapshot 删除 prelude scope。
- [x] root snapshot 只包含 module。
- [x] source gate 禁止 root 非 module 项和 `novadraw::prelude`。
- [x] 每个禁止 root 名称有独立 compile-fail 证据。
- [x] 外部 consumer 只用显式领域路径。
- [x] quick 与 S02/S03/S04 组合交付 full gate 通过，见 S04 完成记录。
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
- [x] quick 与 S02/S03/S04 组合交付 full gate 通过，见 S04 完成记录。
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
  overlay 暴露面。TC-08/R06 的投影失败隔离与批量校验仍归 P2-S06。

## 6. P2-S05：深层复合布局与文字 Presentation 收敛

来源：
[UML 扩展能力验证 F-01](../reviews/uml-extension-capability-2026-10-05.md#f-01深层复合布局与文字-presentation-未稳定收敛已关闭)。

- [x] 提取两层以上嵌套 Toolbar/Layout + Label 的最小复现。
- [x] 证明问题位于 FigureNode layout content/outer measurement 投影遗漏
  `NodeState.insets`，而非 presentation snapshot、clip/transform 或稳定化重启。
- [x] 通过
  [normative design](../../design/architecture/deep-layout-presentation-convergence.md)
  定义“同一稳定帧使用最终 geometry”的契约。
- [x] 修复 FigureNode content/outer measurement 投影，不在应用层重复 refresh。
- [x] Headless command、Native/Vello 像素和 damage 回归通过。
- [x] 实现只修改 FigureNode measurement 组合与查询调用点，没有扩大稳定化扫描，
  没有修改递归渲染主循环。
- [x] [Completion record](../reviews/p2-s05-deep-layout-presentation-completion-2026-10-12.md)
  关闭 UML F-01。

## 7. P2-S06：Editor 增量投影失败隔离与 Overlay 批量校验

来源：R06、TC-08。

- [x] 先复核机制必要性，撤销三个具名类型的强制建设要求；见复核报告。
- [x] 形成 ADR-037 草案：所有权范围、activation 前缀、清理错误、通知/damage、模型/history 与恢复。
- [x] 形成[逐阶段验证矩阵](../plans/p2-s06-incremental-failure-isolation.md)，映射既有 G2/G3/G5.1 suite；实施进度见证据记录。
- [x] 按[首轮评审](../reviews/adr-037-draft-review-2026-10-14.md) S06-DR01–DR05 修订 ADR 与矩阵。
- [x] 复核 dispose 阶段、fault 权限、unresolved 分类、分领域生命周期和旧提交退出。
- [x] 评审并接受 ADR-037，回写 Editor architecture/G5.1 的保证范围。
- [x] 冻结单次新建子树失败隔离、完整同步前缀和 callback/fault 的保证边界。
- [x] 新建子树可恢复拒绝时清除本次新增 Figure、Part、registry、behavior/policy 登记；成功激活前缀只撤销一次。
- [x] Connection 复用已有清理路径，分别覆盖 binding/activation 拒绝清理、route 失败转稳定 unresolved 和 lifecycle panic；prepare 拒绝按修订 2 组合证明，不为已有能力再造 Plan。
- [x] Overlay 批量预检覆盖重复 ID、祖孙重叠、foreign/stale/owner、Figure/style；按草案拒绝重复和重叠，不隐式规范化。
- [x] 清理错误不得静默丢弃；callback panic 或清理失败进入 fault，不宣称完整回滚。
- [x] 不承诺 Drop 能回滚 Runtime；只有未发布 owned 候选可以靠正常释放撤销。
- [x] fault injection 与修订 2 组合证明按上述边界验证新增结构、registry、anchor、overlay 和通知/damage；不要求完整 synchronize 任意失败前后完全相同。
- [x] 模型/history 已提交事实与视觉投影失败分别报告，不以视觉失败伪装模型回滚。
- [x] 与 P2-S04 的 typed Tool/Handle ownership 集成。
- [x] [completion record](../reviews/p2-s06-failure-isolation-completion-2026-10-15.md) 按修订 2 关闭 TC-08/R06，最终 full 通过。

已通过用例及未验收边界见[实施证据](../reviews/p2-s06-implementation-evidence-2026-10-15.md)。
后续已补充真实 Native GPU 队列退出和两层拒绝证据；
用户已接受[剩余验收复核](../reviews/p2-s06-remaining-evidence-review-2026-10-15.md)的
组合证明和 Native GPU 队列宿主范围；完整 prepare 拒绝集成未执行的限制保留，
不以 retained texture 结果勾选完整平台验收。Web fault 退出见 `S06-PL02`。

## 8. P2-S07：结构查询身份与失败语义

来源：R04、TC-09、API-F06、API-07。

- [x] 先核对 ADR-036 已完成的 Event/Layout/Route 命名和阶段权限迁移，不重复实施。
- [x] ADR 草案定义身份、合法缺失与领域失败的分类及组合；仅在共享消费者需要时统一错误类型。
- [x] 完成[当前反例盘点](../reviews/p2-s07-query-design-inventory-2026-10-15.md)及
  [验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)，区分既有基线与未执行用例。
- [x] 完成[ADR-038 首轮评审](../reviews/adr-038-draft-review-2026-10-15.md)。
- [x] 按 S07-DR01/02 明确求逆数值失败和测量成功值校验，并补充 DR03/04 的依赖
  指纹与 SSOT 迁移清单；已写入 ADR 与矩阵修订 1（2026-10-16）。
- [x] [复核 ADR-038 修订 1](../reviews/adr-038-revision-1-review-2026-10-16.md) 的 S07-DR01–DR04 处理，不以文档修订代替行为验证。
- [x] 评审并接受 ADR-038，明确任意 ID 返回值和 LayoutManager 测量失败传播。
- [x] 任意 ID 入口对 unknown/foreign/stale 返回结构化错误；
  已验证 snapshot/typed view 可保留直接 getter。
- [x] `Option` 表示合法缺失；不可逆变换/无路径遵循其领域契约。
- [x] LayoutSceneQuery 不为无效 ID 返回 ZERO、INFINITY 或 default 伪造成功；
  合法空布局、零尺寸、无上限约束仍按原合同表达。
- [x] `is_visible/is_enabled` 不把 unknown 折叠成 false。
- [x] 删除或改名非权威查询，避免 FigureNode/FigureTree 同名不同语义。
- [x] foreign/stale/absent/fault 负向测试和外部 LayoutManager consumer 通过。
- [x] [completion record](../reviews/p2-s07-query-identity-completion-2026-10-16.md) 关闭 TC-09/R04/API-F06/API-07。

## 9. P2-S08：Detached Figure / Border / Style 输入不变量

来源：R03、TC-03、API-F04、API-10。

- [x] 已形成[输入盘点与设计草案](../../design/architecture/detached-input-invariants.md)、
  [ADR-056](../../adr/adr-056-detached-input-invariants.md)及
  [验证矩阵](../plans/p2-s08-detached-input-invariants.md)；已接受并按切片实施。
- [x] 先盘点 Opacity、StrokeStyle、几何与字体现有不变量；仅为真实缺口补充受检值，
  不预定 `Alpha01` 等新类型，不为所有 f64 建 wrapper。
- [x] 评审并接受 ADR-056，冻结构造迁移和外部输出边界。
- [x] 防止字段写入绕过跨字段不变量；已受检且可独立替换的字段无需一律私有化。
- [x] 接受未受检输入的构造/更新在对象发布前返回结构化错误；
  默认构造与安装已验证值不机械改成 Result。
- [x] 删除 Border 构造 assert/panic alias。
- [x] 删除隐藏非法输入的 silent clamp；保留已定义的 viewport range 收敛、圆角按
  目标几何适配等合法算法行为，区分负坐标与非负尺寸。
- [x] 构造保证领域不变量；admission 保留跨对象/namespace/topology 及派生溢出校验，
  backend 精度与 capability 预检继续独立存在。
- [x] 内置与外部 Figure 使用相同值类型和失败语义。
- [x] 构造、更新、外部 Figure admission 的负测与渲染回归通过；
  产品 schema/serializer 不进入引擎验收。
- [x] [completion record](../reviews/p2-s08-d-completion-2026-10-29.md) 关闭 TC-03/R03/API-F04/API-10。

## 10. P2-S09：服务失败、Inspector 与错误 Source

来源：R07、TC-10、API-F08、API-F10，以及 API-F09 未覆盖的 connection register alias。

- [x] [ADR-057 草案](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)收口本批服务的 fault、mutation、check 与 idempotence 语义；尚未接受，
  身份/查询规范引用 S07，不重复改造所有 getter。
- [ ] AnimationMut 的 fault 与 `Ok(false)` 幂等未变化分离。
- [ ] Inspector 锁 poison 返回结构化 fault，不再返回空集合或静默跳过。
- [ ] Inspector attach 明确订阅所有者、注销与 observer fault 的可观测路径；
  优先复用 Observations token，仅在无法满足生命周期时增加 guard。
- [ ] 盘点 S03 迁移后的残余注册入口；有失败可能的注册保留一个 fallible 入口，
  删除 panic alias，不按名称批量删除不同语义的 `try_*`。
- [ ] Command/Policy/extension 的错误链可追溯，不仅保存 String/label；
  领域分类与 source 保留，panic payload 的诊断策略单独定义。
- [ ] fault injection 覆盖 Animation、Inspector、registration 和 extension panic。
- [ ] 与 P2-S03 Runtime facade 的 Observations/Connections 边界一致。
- [ ] completion record 按实际覆盖关闭 TC-10/R07/API-F08/API-F10，
  回填 API-F09 残余 register alias，不把已完成 CommandStack check/has 重新列为缺口。

## 11. 审计派生的能力 Backlog

这些是已确认能力缺口，不等于当前实现缺陷。状态继续以 P2 Backlog 为准。

| Delta | 状态 | 审计来源 | 剩余验收 |
|---|---|---|---|
| P2-G01 Graphics extension | `in_progress` | GOAL-CAP、Graphics 审计 | 定位 present suite 首帧 Skipped，闭合 Native surface partial repair 证据 |
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
- 候选性能问题的唯一待复核列表见 §13；本节仅维护发布预算与采证。

### GA-3 平台

- [ ] Windows 原生 runner。
- [ ] Linux X11 原生 runner。
- [ ] Linux Wayland 原生 runner。
- [ ] Firefox/Safari 支持等级复核。
- Native accessibility provider 由 P2-A01 单项承接，GA-3 只引用其平台资格证据。

### Web EditContext 扩展验收

- P2-E02 的 Native/Web textarea 输入与系统 IME 人工复验已完成，不重开。
  以下仅针对[EditContext partial 集成](../reviews/web-edit-context-capability-2026-09-30.md)：
- [ ] EditContext 路径真实系统 IME composition、候选窗及 characterboundsupdate 验收。
- [ ] trusted keyboard 触发 EditContext `textupdate`。
- [ ] 裁决 `textformatupdate` 样式映射范围与 fallback；如增加反馈协议，
  先登记独立平台切片，不默认为 P2-E02 旧完成范围。

### Web Editor fault 退出

- [ ] `S06-PL02`：真实 Web/Vello 异步提交期间 Core fault 后停止旧 RAF/输入生产者，
  等待或撤销旧 GPU work，释放旧 Viewer/Runtime，再由新 Runtime 以
  Snapshot + Full 接管；迟到旧 completion 拒绝，基线完成后恢复编辑。
  承接 S06 修订 2 保留的独立平台证据，不能由 Native retained texture 或
  Web 编译回归替代。依据见
  [已接受的验收调整](../reviews/p2-s06-remaining-evidence-review-2026-10-15.md)。
- Native surface `Skipped` 继续由 P2-G01 定向复核；
  S06 的 Native GPU 队列退出通过不改变其状态。

## 13. Hold / 待定向复核

以下条目有历史证据，但尚不足以直接进入实现：

- [ ] 首帧 Full promotion 是否仍重复录制：归 GA-2，先建立当前版本 probe。
- [ ] Label/TextFlow 深链祖先扫描是否构成预算问题：归 GA-2，先测 p50/p95 与访问量。
- [ ] 外部 LayoutOutput 非有限值预检：在 P2-S08 Design 时判断属于值不变量还是
  Layout contract。
- [ ] 浏览器 composition 去重：只有真实事件序列证明重复写回后才升级为缺陷。
- [ ] 2026-10-12 新增规范的其余证据：prepare 当前提交前缀与 NoChange 容量边界、
  过期 measurement/layout/route/presentation 候选和 stable epoch 失败隔离、
  disabled Figure 几何查询、外部标准容器 effect/clip/projection 权限对等。
  来源见[契约修订记录 §3](../reviews/framework-architecture-contract-revision-2026-10-12.md#3-实现证据与待验证要求)；
  先核对既有 suite 再补负测，不直接断言缺陷或归入 S06。
- [ ] RootLayerFactory/SelectionPolicy/tracker 外部可达性、新资源消费者及一般多区域
  裁剪需求：先以公开 consumer 定向证明，来源为 2026-10-12 审查 §8。

Hold 项不得顺手并入正在执行的批次；先完成复现和 Triage。
FigureUpdateEffect 越权与整批末项拒绝的预检整改已由 `b6a34b1`/`ec415b2` 完成；
不重复新建批次，也不以它代替上述其余契约证据。

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
| ADR-036 | Event/Layout/Route 阶段权限与 Query 命名迁移，见独立完成记录 |
| P2-S06 / R06 / TC-08 | 新建失败作用域清理、Overlay 批量预检及 fault 退出；按修订 2 组合证明和 Native 队列范围关闭，限制见完成记录 |
| FigureUpdateEffect（2026-10-14） | Runtime 整批顺序预检与授权目标限制；其证据独立于 Editor S06 |
| P2-C01 connection.locator | decoration orientation/reference 与 locator 已验证 |
| M1-M10 / G0-G5 | 既定 Core 与 Editor milestone 已完成 |

## 15. 每个批次的固定关闭动作

- [ ] 当前 batch 的 ADR/normative design 已接受。
- [ ] 当前只有该整改 batch 进入 Execute；待采证 delta 保留原交付状态。
- [ ] completion criteria 在 Execute 前冻结。
- [ ] 新发现已登记为 hold，没有隐式扩大范围。
- [ ] 按风险完成定向 suite 与必要 quick gate；纯文档只运行 docs 与 diff check。
- [ ] 最终提交、推送、合并或里程碑关闭前执行一次 full gate（纯文档按 CLAUDE.md
  例外）；不因每个小补丁重复运行 workspace 门禁。
- [ ] 新建 completion record。
- [ ] 回填原审计 ID、Backlog 状态和文档索引。
- [ ] 下一 batch 才可进入 `in_progress`。
