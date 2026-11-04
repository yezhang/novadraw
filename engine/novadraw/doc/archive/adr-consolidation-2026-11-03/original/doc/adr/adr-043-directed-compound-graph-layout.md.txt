# ADR-043：有向图布局的独立模型与开放求解器

类型：`architecture-decision`

能力归属：Core 独立图布局域与 Figure adapter。

状态：已接受，修订 1，2026-10-22。

## 问题

若图算法直接依赖 FigureTree、Runtime 或示例业务模型，独立消费者无法复用，
算法结果也难以在提交前验证完整性与约束。

## 决策

独立图模型与开放 solver 不依赖 Figure 身份；基础结果与 typed 能力结果分开。
输入、预算与候选硬约束受检，结果不可变。Figure adapter 负责身份映射、
约束测量与稳定发布，不把原始图算法嵌进布局 callback 或示例。

## 替代与代价

不直接绑定具体 Figure、不采用闭合算法枚举，也不以 Zest 作为需求或实现依据。
独立模型需要 adapter 和版本校验，但可支持外部算法与不使用 Figure 的消费者。

## 关联设计

[图布局协议](../design/architecture/directed-compound-graph-layout.md)
维护模型、结果、算法与恢复；
[投影准备](../design/architecture/graph-projection-preparation.md)
维护 adapter 后续已接受修订。
