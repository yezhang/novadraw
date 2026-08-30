# Novadraw 设计

类型：`normative-design`

本目录是 Novadraw 行为与架构契约的 SSOT：

- `architecture/`：组件职责、静态结构、动态时序、目录边界和 milestone 契约
- `coordinates/`：坐标域、变换、命中、事件点与 damage 投影
- `input/`：平台无关输入与手势分发
- `rendering/`：UpdateManager、DisplayList 与渲染协议

`architecture/overview.md` 是导航和总体约束；出现细节冲突时，范围更窄的专题设计
优先。设计可以引用 `reference/` 作为依据，但不得把外部源码描述直接当作本项目契约。
