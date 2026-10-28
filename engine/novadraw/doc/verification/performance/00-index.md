# 性能证据入口

类型：`documentation-index`

报告只证明标注环境、版本和工作负载的测量。平台支持声明见
[平台矩阵](../../roadmap/platform-support-matrix.md)，GA-2 剩余门禁见
[目标调整计划](../../roadmap/goal-alignment-adjustment-plan-2026-09-30.md)。

| 顺序 | 文档 | 证据范围 |
|---:|---|---|
| 1 | [GA-2 方法](ga2-methodology.md) | 工作负载、统计口径及验收维度 |
| 2 | [Novadraw CPU](ga2-novadraw-cpu-2026-10-01.md) | 无窗口 Core CPU 基线 |
| 3 | [Draw2D 对照](ga2-draw2d-comparison-2026-10-01.md) | 对照工作负载与比较边界 |
| 4 | [Native/Vello](ga2-native-vello-2026-10-01.md) | GPU/窗口结果及未完成 surface 采证 |
| 5 | [WebGPU 浏览器](ga2-webgpu-browser-2026-10-01.md) | 浏览器环境和剩余采证 |
| 6 | [R8 历史基线](r8-baseline-2026-09-02.md) | 当时版本的性能回归依据 |

CPU、GPU 完成与 compositor 呈现是不同维度，不能互相替代或合并为一个“已通过”。
