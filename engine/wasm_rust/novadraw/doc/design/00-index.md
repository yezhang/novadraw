# Novadraw 设计

类型：`normative-design`

本目录包含 Novadraw 的规范设计和明确标记的候选提案；其中
`normative-design` 文档构成行为与架构契约的 SSOT：

- `architecture/`：组件职责、静态结构、动态时序和目录边界
- `coordinates/`：坐标域、变换、命中、事件点与 damage 投影
- `input/`：平台无关输入与手势分发
- `rendering/`：UpdateManager 与渲染提交协议；DisplayList 文件仅为 proposal

`architecture/overview.md` 是导航和总体约束；出现细节冲突时，范围更窄的专题设计
优先。设计可以引用 `reference/` 作为依据，但不得把外部源码描述直接当作本项目契约。

## 核心设计

1. [`architecture/overview.md`](architecture/overview.md)
2. [`architecture/static-architecture.md`](architecture/static-architecture.md)
3. [`architecture/dynamic-architecture.md`](architecture/dynamic-architecture.md)
4. [`coordinates/coordinate-system.md`](coordinates/coordinate-system.md)
5. [`input/scroll-zoom-gesture-contract.md`](input/scroll-zoom-gesture-contract.md)
6. [`rendering/update-manager.md`](rendering/update-manager.md)

## 非规范提案

- [`rendering/display-list-protocol.md`](rendering/display-list-protocol.md)
- [`rendering/displaylist-implementation-plan.md`](rendering/displaylist-implementation-plan.md)

提案只有经过 ADR 接受后才能覆盖或扩展规范设计。
