# 路线图与待办校准复核

类型：`verification`

日期：2026-10-14

状态：文档校准完成；不代表剩余能力已实现或已验收。

## 1. 范围与依据

本轮检查 `doc/roadmap/` 全部现行页面、整改总清单及审计闭环工作流。以
[目标调整计划](../../roadmap/goal-alignment-adjustment-plan-2026-09-30.md) 的
核心灵活性、扩展性、稳定性优先方向为准；完成事实来自独立完成记录，候选机制必须
有需求或反例。未重新进行全仓实现审计，未改写历史报告、M1-M10 或 G0-G5 完成状态。

主要依据：

- [S03 完成记录](p2-s03-runtime-facade-completion-2026-10-10.md)、
  [S04 完成记录](p2-s04-editor-open-protocol-completion-2026-10-10.md)、
  [S05 完成记录](p2-s05-deep-layout-presentation-completion-2026-10-12.md)。
- [ADR-036 完成记录](adr-036-context-phase-authority-completion-2026-10-13.md)。
- FigureUpdateEffect 整批预检及权限整改提交 `b6a34b1`、`ec415b2`。
- [S06 机制必要性复核](p2-s06-mechanism-necessity-2026-10-14.md)。
- [五项契约修订](framework-architecture-contract-revision-2026-10-12.md)、
  [文档与架构审查](framework-documentation-and-architecture-review-2026-10-12.md)。
- [Graphics 证据](p2-g01-graphics-evidence.md)、
  [M01-B](p2-m01b-animation-presentation-evidence.md)、
  [M01-C](p2-m01c-animation-consumer-evidence.md)、
  [Web EditContext partial 验证](web-edit-context-capability-2026-09-30.md)。

## 2. 发现与处置

| 问题 | 原描述的风险 | 校准动作 |
|---|---|---|
| 主入口仍先做 S01，动画概述仍下一 C | 重复执行已完成工作 | 当前队列统一链接 S06–S09 → L01 Design；A/B/C 保留完成证据 |
| 丰富度计划另有 Graphics → 动画 → 布局的阶段主链 | 与当前 API 整改方向竞争 | 改为能力拆分建议；价值分组不代指当前优先级 |
| M01-B 前必须关闭 G01 surface，但 B/C 已完成 | 依赖与证据矛盾 | G01 present 作为 D 的 Native surface 视觉毕业依赖；Core/headless 完成态不倒退 |
| G01/M01/GA-2/GA-3 多项 in_progress 被当成并行实施 | 错误关项或虚构并行 | 区分交付未闭合与当前 Execute，单链约束后者 |
| S06 强制新增 Plan/Batch 与完整投影回滚 | 过度抽象，保证范围无法由已有证据支持 | 延续必要性复核：新增子树失败清理、Connection 晚期覆盖、Overlay 写前校验；外部副作用与模型/history 分离 |
| S07 强制单一 QueryError 与统一 Result<Option> | 抹平领域语义、重复 Context 迁移 | 任意 ID 校验；合法缺失/无上限/不可逆按领域表达；可评审 typed snapshot view |
| S08 强制新值类型、全字段私有、全构造 Result | 与已有受检值重复，误删合法 clamp | 按真实输入不变量收口，复用 Opacity/StrokeStyle；保留范围收敛、圆角适配及 backend 预检 |
| S09 强制新 guard、批量按 try 名称删除 | 重复订阅机制或误删不同语义入口 | 先盘点 Observations/注册现状；验证 detach、fault 与错误链，具名机制非毕业条件 |
| 新规范待验证项缺少执行去向 | 文档接受被误读为实现完成 | 剩余 prefix/no-op/版本/权限要求放 Hold；已提交 Effect 整改单独列为基线 |
| GA-2 性能候选、AT provider 重复列项 | 同一根因多队列实施 | 性能候选统一 Hold；AT 实现归 A01，GA-3 引用资格证据 |
| IME 待办未区分 textarea 与 EditContext | 重开已完成 E02，或错误关闭 experimental 证据 | 保留 E02；EditContext trusted event/候选窗/character bounds/样式裁决独立保留 |
| S08 序列化、Editor 最终保存加载仍像引擎门禁 | 产品职责回流 Core | 产品 schema/serializer/产品入口归独立产品包 |
| 测试增量 1,100/目标 1,250 与 demo 30/60fps | 无证据数字冒充当前验收 | 移除旧数量目标；行为覆盖与 GA-2 固定负载测量为准 |
| 工作流仍下一个 Runtime facade/ADR-033 | 已完成任务再启动 | 链接当前队列；保留 API snapshot 基线，辅助工具仅为候选 |
| 同类新发现可在 Execute 扩大范围 | 破坏冻结前缀 | 仅既有范围已覆盖时纳入，否则 Hold 或回 Design |

平台支持矩阵保持原等级：macOS/Chrome runtime_verified，Windows/Linux
build_verified，Firefox/Safari not_verified。本轮没有新的平台证据，不能提升等级。
GA-6 完成表示已完成一次审计，其 `not_ready` 结论继续有效；审计完成不等于发布就绪。

## 3. 校准后的推进边界

当前下一步是 S06 Design：先冻结单次新建子树、既有 Connection 清理和 Overlay 批量
校验的可恢复失败保证。随后按既定 S07 → S08 → S09 顺序收口公开 API 与失败边界，
再进入 L01 独立图模型和外部算法 adapter Design。

G01 surface 的 Skipped 保留为待定位失败，不假定它只缺人工验收，也不在没有当前
反例时扩大为 Core 重构。M01-D、GA-2/GA-3 的平台/视觉/性能证据保留到发布准备。
如果定向取证证明核心正确性问题，应重新 Triage；后置不能免除正确性要求。

W01/T03/O01/LC01 与未登记组件、结构动画、概览继续按真实消费者评审。已有 lifecycle
Trigger、temporary visual、capability 与动画消费者应复用，不重复建设。

## 4. 验证与限制

本次为文档变更：

- `git diff --check`：PASS。
- 本批路线图、工作流、清单及本报告共 12 份文档，119 个本地链接和 20 个章节
  anchor 检查：PASS；已修复清单中 UML F-01 的旧 anchor。
- `cargo xtask docs`：受并行书籍目录迁移阻塞。首轮发现 `book/README.md` 的
  `src/README.md` 已失效；该入口随后由并行修改更新，复跑停在历史报告
  `goal-design-code-audit-2026-09-30.md:226` 的
  `../../../book/src/09-verification-and-extension.md`。没有将并行迁移计入本批修改，
  也没有修改历史报告来掩盖阻塞。
- 提交前复跑 `cargo xtask docs`：PASS。并行迁移已修正上述旧链接；
  manifest 97 commands / 2 profiles / 55 suites，文档元数据、链接、索引分类及
  32 项公开 API doctest 均通过。书籍迁移及其链接修改不属于本批提交。

本报告不提供新的 Rust 行为、像素或性能结论；S07–S09 仍需当前实现盘点及
Design，Hold 尚未证明为实现缺陷。
