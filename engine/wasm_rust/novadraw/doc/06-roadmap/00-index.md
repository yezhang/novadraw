# 06-Roadmap 路线图

本目录承载 Novadraw 朝 draw2d 核心演进的**产品视图**与 **demo 视图**；相关 API 语义覆盖与 milestone 对应关系由 `doc/01-architecture/draw2d_api_coverage.md` 补充。

## 文档职能边界

| 文件 | 职能 | 性质 | 更新频率 |
|------|------|------|----------|
| `doc/01-architecture/draw2d_api_coverage.md` | **语义账本**：draw2d API family、Novadraw 对照方向、覆盖状态与 milestone 映射 | 人读，架构与实现对齐入口 | 按语义收敛持续更新 |
| `doc/06-roadmap/product-deliverables.md` | **产品视图**：每个 milestone 下要交付的图元数量、布局种类、边框种类等策略层清单 | 人读，启动期定稿 | 启动期一次，后续微调 |
| `doc/06-roadmap/demo-matrix.md` | **验证视图**：每个 milestone 配套的 demo 名称、覆盖范围、截图/帧率断言策略 | 人读，启动期定稿 | 启动期一次，后续微调 |

## 编号唯一来源

**本目录统一使用 `M1-M10` 编号，并与 `draw2d_api_coverage.md` 中的 milestone 映射保持一致。**

本目录内任何 milestone 引用必须直接使用 `M{n}`，不允许在 doc 内发明额外编号。

## GEF 边界

当前语义账本中标记为 GEF 层或非核心目标的能力，不纳入 draw2d 核心里程碑：

- EditPart / EditPolicy / Tool / Command / Request / Viewer / Palette / Selection provider / Undo-redo command stack

这些不在 draw2d 核心里程碑内。带"节点编辑器"性质的 demo 视为 GEF 层早期探索，详见 `demo-matrix.md` 附录。

## 文档列表

| 文档 | 主题 |
|------|------|
| `product-deliverables.md` | 每个 milestone 下要交付的产品策略层清单 |
| `demo-matrix.md` | 每个 milestone 对应的 demo + 验证矩阵 |
