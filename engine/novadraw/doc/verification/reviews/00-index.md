# 评审与验证结论

类型：`documentation-index`

本页按专题组织当前结论。执行状态以[P2 backlog](../../roadmap/p2-delta-backlog.md)为准，
评审通过、实现完成与平台验收分别判断；原始报告只证明其指定基线。

## 当前专题

| 专题 | 当前结论与证据 |
|---|---|
| 文档内容质量 | [DQ01–DQ08 修订记录](documentation-quality-completion-2026-11-04.md)：确定问题已修正，FR-02 基线缺失及引用候选保留限制；[本轮分析](documentation-quality-follow-up-2026-11-04.md)与[前轮分析](documentation-content-quality-2026-11-03.md)保留原发现，Q07 的 ADR 压缩与本轮两组 design 去重已完成 |
| 长文本性能 | [优化验证与最新成本](../performance/text-optimization-validation.md)、[重复工作归因](flow-runtime-repeat-work-analysis-2026-11-03.md)、[FR-01 实施与匹配比较](flow-runtime-input-reuse-2026-11-03.md)、[FR-02 内部归因与诊断扰动](flow-assembly-attribution-2026-11-03.md) |
| 富文本管线删减 | [逐项实施与前后对照](rich-text-pipeline-reduction-2026-11-05.md)：关闭 tracing 的拖拽／滚轮／连续 resize 与 GPU 完成等待，Perfetto 分析另列；整体仍在实施 |
| 独立 RichTextFigure | [基础切片](rich-text-figure-foundation-2026-11-05.md)：文档快照、稳定段落、高度索引、typed update 与类图／正文双内容 app；Native 图像和定向检查通过，缓存、人工交互及性能仍待验收 |
| RichTextFigure 缩放 | [快照复用与 CPU 成本](rich-text-figure-resize-optimization-2026-11-05.md)：测量／准备共享完整结果，连续变宽 CPU P50 从 17.491 降至 3.566 ms；Perfetto 快照次数下降 89.69%，真实 Native resize 和 GPU 完成待验 |
| RichTextFigure 压力 | [混合字号、emoji 与长文](rich-text-figure-stress-performance-2026-11-05.md)：四档 Runtime／CPU 回放通过，混合字号和彩色 emoji 图像通过；25 万字符变宽 P50 189.463 ms，Native 黑帧，长文绘制未通过 |
| RichTextFigure 编辑 | [Native 编辑性能样例](richtext-figure-edit-app-2026-11-06.md)：段内输入、字素删除、受检光标和 IME 提交回放；混合档编辑后图像通过，完整输入法与长文绘制仍待验 |
| ADR-061 会话与控制器 | [实施验证](specialized-edit-controller-2026-11-06.md)：统一活动槽、外部领域控制器、richtext 独立及嵌入接入；Native 回放和匹配 CPU 成本，平台与长文限制分别保留 |
| 富文本点击 GPU 瓶颈 | [63 ms 专项诊断](richtext-click-gpu-bottleneck-2026-11-06.md)：局部 damage 未约束全文记录与编码；Emoji 消融 P50 61.644 → 25.065 ms 证明无关绘制成本，整改优先打通局部绘制范围，GPU／屏幕呈现边界继续保留 |
| 富文本后端局部修补 | [实施与完整输出复测](richtext-local-damage-2026-11-06.md)：编码前字形筛选、小纹理 GPU 修补；点击到 GPU 完成 P50 61.644 → 7.614 ms，Core 全文记录与屏幕呈现验收仍保留 |
| Draw2D 文本算法采用 | [局部实现与匹配成本](draw2d-text-algorithm-adoption-2026-11-05.md)：候选缓冲区及最佳布局复用、当前字体指标缓存；平均宽度预测未采用，不代表长文本整体性能验收 |
| Draw2D 文本优化第二批 | [实施与验证](draw2d-text-followup-2026-11-05.md)：等值修改保护、独占工作区容量复用与生产分析／重排分离的准入缺口 |
| Perfetto Native trace | [四份 trace 诊断](perfetto-trace-diagnosis-2026-11-04.md)、[官方 skill 文本渲染分析](perfetto-text-rendering-bottlenecks-2026-11-04.md)：重复测量组装、准备校验、文字引擎子阶段与 backend 观测缺口；debug 手动运行，不代表 release 性能验收 |
| Large 滚动专项 | [最新复用开关对照](flow-assembly-scroll-comparison-2026-11-05.md)：两组各 192 次滚动，开启后固定滚轮活跃中位 19.83 → 30.93 ms，新增成本集中在 Core 稳定化和帧准备；[前轮 release trace 与方案](textflow-scroll-perfetto-2026-11-04.md)保留原基线；无观测逐帧成本、连续 OS 输入与 GPU／屏幕延迟待补验 |
| 滚动与 resize 修订 3／4 | [原评审](long-text-scroll-resize-revision-3-review-2026-11-04.md)与[修订 4 回应](long-text-scroll-resize-revision-4-response-2026-11-04.md)：回应已获用户通过并授权执行；各工作包分别验收 |
| Large 纹理复用实验 | [GPU 离屏复用与像素对照](textflow-scroll-raster-experiment-2026-11-04.md)：默认关闭，匹配对照确认热块复用；上游 Flow 组装、冷块成本与少量字形边缘差异仍待整改 |
| FR-02 复用合同评审 | [草案评审](adr-060-draft-review-2026-11-03.md)、[修订 1 回应](adr-060-revision-1-response-2026-11-03.md)、[共享内容基础迁移](flow-shared-content-implementation-2026-11-04.md)与[输入证明及不可变 Flow 消费](flow-input-proof-implementation-2026-11-04.md)：基础门禁通过；[整页采用首片](flow-assembly-adoption-implementation-2026-11-04.md)已接通并通过首轮 6 项反例，家族容量及完整生产门禁仍待验；[基线核对](fr02-review-baseline-2026-11-04.md)保留历史缺失范围 |
| 阶段文字引擎 | [ADR-059 与 B1 验证状态](text-stages-status.md)：范围、最新分母、未完成边界及原稿入口 |
| Parley 上游与补丁必要性 | [0.12.0／main API 复核](parley-upstream-api-and-patch-necessity-2026-11-05.md)：公开分析／shaping 数据、重排及扁平样式输入；35 份补丁的退役条件与未满足的预算／容量合同，无补丁 Native 消费者通过 |
| Parley 存储与预算边界 | [v0.12.0 再评估](parley-v012-storage-and-budget-reassessment-2026-11-05.md)：公开数组可构造自有计费快照，逐行控制已验证；建议保留 Core 累计工作与留存限额，重新裁决默认后端的深层逐操作预算，合同未修改 |
| Parley 上游迁移 | [实施与回归验证](parley-upstream-migration-2026-11-05.md)：默认适配器使用未打补丁的 0.12.0；旧源码、35 份补丁及专用入口退役，Core 通用阶段合同保留 |
| Parley 0.12 后续性能 | [新基线、输入检查与投影分离](parley012-input-scope-and-projection-2026-11-05.md)：已获按序推进授权，release 基线已冻结；交互采样与检查有效区间实施待完成 |
| FigureTree 通用投影分离 | [实施记录](figure-tree-projection-implementation-2026-11-05.md)：活动交接删减与成员索引已验证；默认交接拖拽 P50 下降 7.17%，成员索引整帧收益不稳定；三份 Perfetto 仍显示重复输入检查和全量 glyph 提交，整树门禁未完成 |
| Large 研究宿主 | [独立应用验证](textflow-large-app-implementation-2026-11-03.md)、[有界富文本候选判断](bounded-rich-text-proposal-assessment-2026-11-03.md) |
| quiet-md 工程样例 | [富文本编辑技术适用性分析](quiet-md-rich-text-assessment-2026-11-05.md)：固定源码的样式事务、行内对象、虚拟化、IME 与随机验证参考；性能为作者历史测量，采用建议未实施 |
| 服务失败／Inspector | [ADR-057 与 S09 结论](service-failure-review-status.md)：A／B／C 完成，D 和整批待补验 |
| Editor 开放吸附 | [ADR-058 与 E04 结论](open-snapping-review-status.md)：已验证范围、成本及平台限制 |
| Editor 容器编辑 | [最新组合验证](editor-container-combinations-2026-11-02.md)、[框架状态](core-capability-status-2026-11-02.md) |
| 共享场景／Tree／容器设计 | [已接受设计的评审结论](framework-contract-review-status.md)：K07、表现几何和 E03 的当前验证边界 |
| 输入不变量 | [S08 完成记录](p2-s08-d-completion-2026-10-29.md)、[颜色／文字输入](core-color-text-input-2026-10-29.md) |
| 官方 Draw2D 示例 | [当前验收矩阵](../plans/draw2d-official-examples-remaining-contracts.md)、[最新 Zoom 文字指标](zoom-typographic-alignment-2026-10-30.md)、[布局窗口](official-layout-native-mouse-2026-10-29.md)、[Scroll／Zoom 窗口](scroll-zoom-native-window-2026-10-29.md) |
| GEF 应用消费者 | [Shapes](shapes-editor-implementation-2026-10-28.md)、[Logic 路由](gef-logic-routing-2026-10-30.md)、[Scene backend](gef-scene-target-implementation-2026-10-28.md) |
| 语义整改 | [Draw2D／GEF 整改状态](draw2d-gef-semantic-remediation-2026-09-16.md)、[Core 公开 API 审计](core-public-api-audit-2026-09-22.md) |
| 长期架构 | [目标／设计／实现审计](goal-design-code-audit-2026-09-30.md)、[架构契约修订](framework-architecture-contract-revision-2026-10-12.md)、[API／架构审计](api-architecture-review-2026-10-09.md) |
| Flow／图布局文档 | [证据导航](flow-and-graph-design-evidence-2026-11-04.md)：完成事实、实施前缺口与当前合同分别阅读 |

## 历史追溯

[历史基线导航](historical-baselines.md)保存既有独立报告入口，不把不同版本或配置的
测量改写成当前提速结论。已归并的修订评审、逐项回应和 B1 子片退出当前阅读路径，
原路径、字节与 SHA-256 见[维护前快照](../../archive/knowledge-maintenance-2026-11-03.md)。
