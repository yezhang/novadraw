# 渲染设计入口

类型：`documentation-index`

本目录负责从稳定派生结果到绘图录制、后端提交的契约。布局版本和发布时序见
[收敛事务](../architecture/derived-state-convergence.md)，坐标见
[坐标规范](../coordinates/coordinate-system.md)。

| 阅读顺序 | 文档 | 唯一职责 |
|---:|---|---|
| 1 | [UpdateManager](update-manager.md) | validation、damage、帧准备与恢复 |
| 2 | [Graphics 扩展](p2-g01-graphics-extension.md) | Paint、StrokeStyle、路径 clip、能力拒绝及 XOR 处置 |
| 3 | [文字与图形整合](text-graphics-integration.md) | Graphics/Measure/Paint 上下文、文字指标、轮廓和后端预处理 |
| 4 | [图像源区域](image-source-rectangle.md) | 源像素域与目标逻辑域、IR、拒绝和 lowering |
| 5 | [SVG 栅格密度](svg-raster-density.md) | 保留矢量源、显式像素预算、自然尺寸与资源替换 |

文字布局与组件语义见 [Text Layout](../architecture/text-layout.md)，
录制及出版决策见 [ADR-031](../../adr/adr-031-graphics-recording-and-publication-boundary.md)。
实现、surface 验收和待采证状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

候选研究：[DisplayList](display-list-protocol.md)，包含探索顺序和晋级门禁，
不作为现行渲染流程的前提。

已批准增量：[逐图形抗锯齿](shape-antialias-proposal.md)，按 ADR-047 实施 K08 的 OFF
覆盖率、独立文字提示和后端受预算约束的覆盖准备；完成状态见 P2 backlog。

外部参考：[字形渲染算法与彩色 Emoji 字体](../../reference/fonts/glyph-rendering-and-color-emoji.md)，
记录轮廓算法比较、局限性和彩色字体格式；其中选型建议不构成现行契约或交付状态。
解码与填充前提见
[字形轮廓存储规范](../../reference/fonts/glyph-outline-storage-conventions.md)，
包含 TrueType 与 CFF/CFF2 的控制点、方向、闭合和填充规则差别。

当前源码的完整流程与库职责见
[文本渲染管线分析](../../reference/fonts/text-rendering-pipeline.md)，
包含标注库作用的管线图、字形／路径分支和自研替换范围；不定义新的包边界。
