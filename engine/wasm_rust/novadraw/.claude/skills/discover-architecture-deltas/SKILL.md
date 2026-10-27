---
name: discover-architecture-deltas
description: 从理想架构与当前实现偏差中发现候选问题。适合开始新阶段、backlog 失真或执行后需要继续发现问题时调用。
---

# Discover Architecture Deltas

从理想架构文档与当前实现的偏差中识别候选问题，并决定哪些问题值得进入正式 backlog。

## When To Use

- 新一轮架构改进开始前
- 当前 backlog 价值下降或过期时
- 执行完一个 delta 后，需要判断是否还存在残余问题时
- 发现测试失败、代码异味、职责回流，但还没被结构化时

## Required Inputs

- `CLAUDE.md`
- `AGENTS.md`
- `doc/理想架构设计.md`
- `agent/governance-architecture-contracts.md`
- `agent/outer-loop-delta-backlog.yaml`
- `agent/governance-contract-coverage.md`
- `agent/quality-discover-smoke-test.md`
- 与候选问题相关的代码文件

## Procedure

1. 先读取 `agent/governance-architecture-contracts.md`，逐条列出本轮要审计的契约，而不是直接自由发挥。
2. 读取 `agent/governance-contract-coverage.md`，优先检查状态为 `drifting` 或 `unassessed` 的契约。
3. 对每条契约，至少检查一个核心代码入口，不允许只读文档不读代码。
4. 对照理想契约，回答“当前实现是否把职责放错位置、是否发生职责回流、是否存在缺失服务边界”。
5. 将发现的问题写成 candidate delta，并标注它更像：
   - 职责边界错误
   - 状态归属错误
   - 调度边界错误
   - 文档契约缺口
6. 为每个 candidate 标注 `evolution_kind`：
   - architecture
   - parity
   - new_feature
7. 判断 candidate 是：
   - promote：进入正式 backlog
   - reject：当前证据不足或价值不高
   - split：问题过大，需要拆成多个候选项
8. 为建议 promote 的项补齐优先级、证据、完成标准和建议入口文件。

## Contract Audit Checklist

### C-03 / C-04 FigureGraph vs UpdateManager

必查文件：

- `novadraw-scene/src/update/mod.rs`
- `novadraw-scene/src/update/deferred.rs`
- `novadraw-scene/src/scene/mod.rs`
- `apps/editor/src/system.rs`

必问问题：

- UpdateManager 是否直接读取或决定图级语义，而不是只做 phase 编排？
- FigureGraph 是否真正持有该语义所需的状态和决策？
- 上层 system 是否引入了本该属于组合根以外的调度样板？

### C-06 SceneHost Thin Boundary

必查文件：

- `apps/editor/src/scene_manager/scene_host.rs`
- `apps/editor/src/system.rs`

必问问题：

- SceneHost 是否持有业务规则或图状态，而不仅是平台调度？
- 平台生命周期处理是否回流到业务层？

### C-07 Composition Root

必查文件：

- `crates/novadraw/src/lib.rs`
- `apps/editor/src/system.rs`

必问问题：

- 依赖装配是否集中在组合根？
- 是否出现额外全局状态、越层引用或隐藏装配？

### C-08 PendingMutation Timing

必查文件：

- `novadraw-scene/src/mutation/mod.rs`
- `apps/editor/src/system.rs`
- `doc/理想架构设计.md`

必问问题：

- 结构性变更是否只在约定时机应用？
- 分发期间是否直接改图结构？

### C-01 / C-02 Figure / FigureBlock Ownership

必查文件：

- `novadraw-scene/src/figure/**`
- `novadraw-scene/src/graph/**`

必问问题：

- Figure 是否偷偷持有 parent/children/focus/capture 等外部关系？
- FigureBlock 是否承担了运行时节点状态容器的职责？

### C-05 EventDispatcher

必查文件：

- `novadraw-scene/src/event/**`
- `apps/editor/src/system.rs`

必问问题：

- EventDispatcher 是否只分发，不持有图状态归属？
- 交互状态是否回流到错误层级？

## Discovery Output Requirements

每次 discover 至少要输出：

- 本轮审计了哪些契约
- 每条契约检查了哪些代码入口
- 哪些契约没有来得及审计
- 新发现 candidate delta 数量
- 若为 0 个 candidate，必须解释“为什么是 0”，不能只给结论

## Smoke-Test Mode

如果用户要求验证工作流本身的发现能力，必须读取 `agent/quality-discover-smoke-test.md`，并按其中的已知问题样本执行一次发现能力自测。

## Output Format

- Candidate Deltas
- Audited Contracts
- Checked Code Entrypoints
- Evolution Kind
- Root Cause Summary
- Promote Or Reject Decision
- Suggested Priority
- Suggested Done When
- Recommended Next Delta

## Guardrails

- 不要把模糊观察直接写成正式 backlog
- 不要一次生成过多难以执行的大问题
- 如果问题只是现象而非职责边界偏差，继续下钻根因
- 如果与已有 delta 重复，优先合并或标记重复
- 如果某条契约未审计到代码入口，明确标记为未覆盖，不要假装“高度对齐”
- 如果 candidate 数量为 0，必须输出审计范围与遗漏范围，避免把“没看出问题”误写成“没有问题”
- 如果某项属于 `new_feature`，明确指出它不属于当前默认主线，不要伪装成架构收敛或 parity 缺口
