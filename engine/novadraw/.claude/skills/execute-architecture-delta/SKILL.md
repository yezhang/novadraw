---
name: execute-architecture-delta
description: 执行一个最小架构差距并先做根因分析。适合已选定当前 delta、准备小步推进时调用。
---

# Execute Architecture Delta

执行一个最小架构差距，确保每轮都是小步、可验证、可恢复的。

## When To Use

- 已经通过恢复流程确认了当前 delta
- 需要推进一轮架构改进时
- 希望把修改限制在一个最小职责边界问题时

## Required Inputs

- `agent/governance-architecture-contracts.md`
- `agent/outer-loop-delta-backlog.yaml`
- `agent/inner-loop-checkpoint.md`
- 与当前 delta 相关的代码文件

## Procedure

1. 读取当前 delta 的契约、证据和完成标准
2. 先解释根因，再给最小方案
3. 只修改本轮必要文件
4. 运行验证命令
5. 更新 backlog 状态
6. 更新 checkpoint 和 worklog

## Output Format

- Root Cause
- Minimal Plan
- Files To Change
- Verification Result
- Residual Risk
- Backlog Update
- Next Small Step

## Guardrails

- 一次只处理一个 delta
- 不允许跨多个层级做大范围重构
- 不允许用临时桥接代码掩盖职责错误
- 如果修改范围失控，立即拆分 delta
- 如果验证失败，不得将状态推进到 `done`
