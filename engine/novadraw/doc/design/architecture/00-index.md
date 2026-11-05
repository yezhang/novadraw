# Core Architecture

类型：`documentation-index`

本目录定义 Novadraw Core 的规范架构。总体文档负责边界和导航，范围更窄的专题契约
负责具体行为；明确标记为 `proposal` 的文档不具有覆盖效力。

## 基础结构

| 文档 | 职责 |
|---|---|
| [overview.md](overview.md) | 总体职责、设计原则和权威边界 |
| [directory-structure.md](directory-structure.md) | Crate 与模块边界 |
| [static-architecture.md](static-architecture.md) | 类型、所有权与依赖结构 |
| [dynamic-architecture.md](dynamic-architecture.md) | 运行时交互和事务时序 |
| [runtime-ownership-and-module-boundaries.md](runtime-ownership-and-module-boundaries.md) | Runtime 所有权、tree/update 协作与扩展更新入口 |
| [public-api-contract.md](public-api-contract.md) | 当前公共 API 规范：领域路径、owner、权限、单位、失败与组合 |
| [runtime-subsystem-facade.md](runtime-subsystem-facade.md) | 无状态 facade、领域调用表与统一 Figure mutation |
| [context-phase-authority.md](context-phase-authority.md) | Context 阶段权限、Query/Effects 与能力授权 |
| [query-identity-and-failure.md](query-identity-and-failure.md) | 查询身份、合法缺失、坐标和测量失败 |
| [engine-value-contract.md](engine-value-contract.md) | 受检 Color、几何词汇、圆弧与可执行命令 |
| [third-party-dependency-boundaries.md](third-party-dependency-boundaries.md) | 中立签名、私有算法、interop 与依赖检查 |

## 核心协议

| 文档 | 职责 |
|---|---|
| [figure-style.md](figure-style.md) | Figure 样式继承与覆盖 |
| [detached-input-invariants.md](detached-input-invariants.md) | ADR-056 已接受：Border 构造、受检样式、外部准入与派生溢出 |
| [figure-lifecycle.md](figure-lifecycle.md) | Figure 生命周期与 Runtime 身份 |
| [runtime-mutation-and-subtree-admission.md](runtime-mutation-and-subtree-admission.md) | Mutation、FIFO effects、完整子树准入与回执 |
| [tree-search-and-focus.md](tree-search-and-focus.md) | 树查询、命中与焦点遍历 |
| [derived-state-convergence.md](derived-state-convergence.md) | 派生状态收敛事务 |
| [deep-layout-presentation-convergence.md](deep-layout-presentation-convergence.md) | 深层布局盒模型与文字 Presentation 收敛 |
| [graph-projection-preparation.md](graph-projection-preparation.md) | 已实施修订 2：ADR-043 adapter 的候选快照、约束测量与必需路由发布增量，GP01–GP04 已处理，最终验证通过 |
| [resource-lifecycle.md](resource-lifecycle.md) | 资源因果与 Backend Session |
| [resource-loading-and-request-tokens.md](resource-loading-and-request-tokens.md) | 平台 provider、纯解码与异步完成仲裁 |
| [figure-inspector.md](figure-inspector.md) | 稳定场景与提交后事件诊断 |
| [notification-subscription-lifecycle.md](notification-subscription-lifecycle.md) | 订阅 scope、FIFO、自注销与历史事实 |
| [typed-property-identity.md](typed-property-identity.md) | 类型化属性源、异构 journal 与行为匹配 |
| [service-failure-and-inspector-lifecycle.md](service-failure-and-inspector-lifecycle.md) | 修订 3：Core 服务/观测、Inspector 本地故障/恢复、Editor 扩展 source |
| [component-update.md](component-update.md) | 第三方组件 typed update 规范 |

## Figure 能力

| 文档 | 职责 |
|---|---|
| [figure-capability-model.md](figure-capability-model.md) | Figure typed capability 注册、发现、mutation 与生命周期统一模型 |
| [layer-and-freeform.md](layer-and-freeform.md) | Layer、Freeform 与范围传播 |
| [connection-routing.md](connection-routing.md) | Connection、Anchor 与 Router |
| [connection-owner-locator.md](connection-owner-locator.md) | owner 方位、child 测量与标签 placement |
| [group-route-solving.md](group-route-solving.md) | 完整连接组求解、visibility 算法与批次失败 |
| [directed-compound-graph-layout.md](directed-compound-graph-layout.md) | 独立图请求、开放算法、硬约束与 Figure 投影 |
| [reusable-shape-border.md](reusable-shape-border.md) | 基础 Shape 与 Border |
| [independent-corners-and-edge-borders.md](independent-corners-and-edge-borders.md) | 逐角轮廓、矩形分边、自由端与设备覆盖 |
| [official-example-border-output-increment.md](official-example-border-output-increment.md) | 官方样例的 SchemeBorder、字体下划线指标与 ICO/帧输出消费者增量 |
| [official-example-capability-increment-2026-10-26.md](official-example-capability-increment-2026-10-26.md) | 样例几何与文字适配；通用协议由各专题负责 |
| [shared-scene-view-background-proposal.md](shared-scene-view-background-proposal.md) | K07 修订 3 已批准，ADR-044/045 接受：保守结构依赖、Runtime 提交点版本校验、独立捕获与宿主背景统一提交；功能验收按切片推进 |
| [text-layout.md](text-layout.md) | Text Layout、Label 与标题边框 |
| [draw2d-text-algorithm-adoption.md](draw2d-text-algorithm-adoption.md) | Draw2D 截断候选复用、当前字体指标缓存及平均宽度预测的适用限制 |
| [structured-flow-layout.md](structured-flow-layout.md) | 结构化贡献、段落 shaping、inline 与交互快照 |
| [inherited-layout-direction.md](inherited-layout-direction.md) | Figure 方向继承、文字片段位置与 ScrollPane |
| [exact-flow-layout-reuse.md](exact-flow-layout-reuse.md) | 精确缓存、共享 glyph 与滚动 IR |
| [flow-assembly-reuse.md](flow-assembly-reuse.md) | FR-02：共享内容基础迁移已开始；整页复用、证明链与留存容量仍逐项冻结和验收 |
| [staged-text-width-reuse.md](staged-text-width-reuse.md) | 可选阶段、调用绑定、资产登记、计费与完整回退 |
| [text-performance-strategy.md](text-performance-strategy.md) | 顶尖性能路线：精确增量、宽度复用、可见绘制、共享资产与调度；隔离实验完成，生产接入合同 ADR-059 已接受 |
| [parley-replacement-candidate.md](parley-replacement-candidate.md) | 未来技术候选：当前保留 Parley，记录 Core 直接编排底层文本库的职责、取舍与重新评估条件；替换尚未授权 |
| [可选阶段文字协议 修订 2](staged-text-width-reuse.md) | 已接受、已批准实施：先查询有效资产再判断新增准入，复合失败和原始 source 保留；阶段绑定、账本及启用门禁不变 |
| [text-analysis-admission-proposal.md](text-analysis-admission-proposal.md) | 限定依赖补丁已获准：ADR-059 B 的字体查询、冷 shaping 与导出计费；B1 扩张暂停，未完成范围见专题验证状态 |
| [long-text-performance-proposal.md](long-text-performance-proposal.md) | 修订 2 首批设计通过，LT-00～LT-03 实施中；后续准入、预算与渐进协议待独立评审 |
| [long-text-scroll-resize-optimization-proposal.md](long-text-scroll-resize-optimization-proposal.md) | 修订 4 已获通过并授权执行；按工作包分别验证组合、输入分母、覆盖和启用门禁 |
| [rich-text-pipeline-reduction-proposal.md](rich-text-pipeline-reduction-proposal.md) | 待评审：富文本主线删减范围、共享资产、证明去重、纯投影与可见绘制的实施顺序和退出条件 |
| [long-text-incremental-layout-proposal.md](long-text-incremental-layout-proposal.md) | 待评审：基于第四轮测量与 Lapce／Xi 的 source delta、分阶段排版、安全巨段增量及可见输出细化 |
| [long-text-native-coverage-boundary.md](long-text-native-coverage-boundary.md) | 原生字形、数值转换与设备覆盖的定向核验；生产证书合同待复核 |
| [basic-widgets.md](basic-widgets.md) | Clickable、Button 与 Toggle |
| [scrollbar-and-figure-activity.md](scrollbar-and-figure-activity.md) | 五部件 ScrollBar、RangeModel、repeat 与活动时钟 |
| [tooltip-accessibility.md](tooltip-accessibility.md) | Tooltip 与 Accessibility Bridge |

坐标、输入和渲染专题分别见
[`../coordinates/`](../coordinates/coordinate-system.md)、
[`../input/`](../input/scroll-zoom-gesture-contract.md) 和
[`../rendering/`](../rendering/update-manager.md)。
