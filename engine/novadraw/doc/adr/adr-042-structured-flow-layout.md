# ADR-042：结构化文字流与内嵌 Figure

类型：`architecture-decision`

能力归属：Core Flow。

状态：已接受，2026-10-21。

## 问题

把多个来源 Figure 压成没有身份的字符串，或按 span 独立 shaping，
会丢失结构归属、跨 span 字形关系和统一交互几何。

## 决策

贡献模型保留 Figure 来源身份；段落整体 shaping，不逐 span 独立塑形。
文字、inline box、caret、selection、hit testing 使用同一受检不可变快照。
Flow 承载结构组合；底层文字引擎保持可替换。

## 替代与代价

不复制示例私有排版器、不把 inline Figure 冒充字形，也不单独计算 paint 与交互。
结构化贡献及映射增加准备成本；复用必须维持来源与版本的精确等价。

## 关联设计

[结构化 Flow](../design/architecture/structured-flow-layout.md)维护模型、排版与输入；
[精确复用](../design/architecture/exact-flow-layout-reuse.md)维护后续复用合同。
