# Novadraw 文档索引

类型：`documentation-index`

本目录按文档的**知识来源与规范效力**组织。技术主题是第二层分类，不能再把
Draw2D 源码分析、Novadraw 设计决策和实施状态混放在同一目录。

## 当前状态

- Draw2D Core：M1-M10 已完成；后续能力进入明确的 P2 delta。
- Editor framework：G0-G5 已完成，检查点 C 已通过人工验收；原 G6 的产品 schema、
  serializer 和产品级 Native/Web 场景移交独立产品包，不再作为引擎完成门禁。
- Core 公开 API：ADR-017 的 P0 Batch A/B 已完成；ADR-018 已完成 Runtime
  驱动、坐标查询与结构化测量收口；ADR-019 已完成 detached 构造与挂载后 scoped
  editor 调用面收口；ADR-020 已完成 Graphics、Geometry 与基础值契约整改；
  ADR-021 已完成聚合 facade 与 backend feature 边界收口；ADR-022 已完成第三方类型、
  Kurbo 版本与渲染依赖门禁。
- Core P2 delta：P2-R02 image source rectangle 已完成，状态见
  [`roadmap/p2-delta-backlog.md`](roadmap/p2-delta-backlog.md)。
- Draw2D Core 之后的动画与表现平面、图布局、内置组件、概览和输出能力建议顺序见
  [`roadmap/draw2d-capability-enrichment-plan-2026-10-05.md`](roadmap/draw2d-capability-enrichment-plan-2026-10-05.md)；
  该计划不替代 P2 backlog 的状态真源。
- 统一 Graphics API 与 glyph 预处理：ADR-025 已接受，P2-G02 已实现并进入最终门禁；
  字体指标、文字测量与图文绘制使用同一 API 集合，自研后端预处理格式保持独立。
- 动画正交模型已登记为
  [`P2-M01`](roadmap/p2-delta-backlog.md)，当前为 `in_progress`；
  [Animation / Presentation Plane 设计](design/animation/animation-system.md)、
  [公开 API 合同](design/animation/public-api-contract.md)、
  [领域能力集成矩阵](design/animation/capability-integration.md)与
  [ADR-026](adr/adr-026-animation-and-presentation-plane.md)已接受；M01-A、M01-B
  已完成，当前推进 M01-C 领域消费者。
- 2026-09-16 与 2026-09-20 两批语义审计整改均已完成，状态见
  [`verification/reviews/draw2d-gef-semantic-remediation-2026-09-16.md`](verification/reviews/draw2d-gef-semantic-remediation-2026-09-16.md)。
- 里程碑状态只在 [`roadmap/00-index.md`](roadmap/00-index.md) 及
  [`roadmap/editor/00-index.md`](roadmap/editor/00-index.md) 维护。
- 可执行验证只在 [`../verification/suites.toml`](../verification/suites.toml) 维护。

## 权威边界

| 目录 | 回答的问题 | 规范效力 |
|---|---|---|
| [`reference/`](reference/00-index.md) | Draw2D、GEF、SWT、Zed 实际如何工作？ | 外部事实；不直接约束 Novadraw |
| [`design/`](design/00-index.md) | Novadraw 应该如何工作？ | Novadraw 设计 SSOT |
| [`parity/`](parity/00-index.md) | 哪些外部语义被继承、调整或拒绝？ | 项目与参考实现之间的桥梁 |
| [`adr/`](adr/README.md) | 为什么接受某项关键决策？ | 已接受决策及其后果 |
| [`roadmap/`](roadmap/00-index.md) | Core 与 Editor 何时交付、当前到哪里？ | M1-M10 与 G0-G5 引擎状态，不定义架构 |
| [`strategy/`](strategy/00-index.md) | 为谁创造价值、如何验证？ | 产品与商业决策输入，不定义架构 |
| [`verification/`](verification/00-index.md) | 如何证明事实、设计和实现一致？ | 审计、验收和检查记录 |
| [`migration/`](migration/00-index.md) | 如何完成语言与工程迁移？ | 方法指南 |
| [`archive/`](archive/00-index.md) | 哪些内容已失效或仅保留历史？ | 非当前契约 |

## 核心 SSOT

- 2026-09-10 架构修订裁决：
  [`adr/adr-014-extensibility-and-lifecycle-boundaries.md`](adr/adr-014-extensibility-and-lifecycle-boundaries.md)
- Core 构建期/运行期公开 API 边界：
  [`adr/adr-017-core-public-api-boundary.md`](adr/adr-017-core-public-api-boundary.md)
- Runtime 驱动、坐标查询与结构化测量：
  [`adr/adr-018-runtime-driving-and-measurement-api.md`](adr/adr-018-runtime-driving-and-measurement-api.md)
- 可组装构造与挂载后 scoped mutable facade：
  [`adr/adr-019-composable-api-and-scoped-editors.md`](adr/adr-019-composable-api-and-scoped-editors.md)
- 引擎基础值与公开渲染契约：
  [`adr/adr-020-engine-value-and-render-contract.md`](adr/adr-020-engine-value-and-render-contract.md)
- 公开 Facade 与 Feature 边界：
  [`adr/adr-021-public-facade-and-feature-boundary.md`](adr/adr-021-public-facade-and-feature-boundary.md)
- 第三方类型与渲染依赖边界：
  [`adr/adr-022-third-party-type-and-render-dependency-boundary.md`](adr/adr-022-third-party-type-and-render-dependency-boundary.md)
- 统一 Graphics API 与 glyph 预处理边界：
  [`adr/adr-025-unified-graphics-and-glyph-preparation.md`](adr/adr-025-unified-graphics-and-glyph-preparation.md)
- Core 公开 API 审计与整改状态：
  [`verification/reviews/core-public-api-audit-2026-09-22.md`](verification/reviews/core-public-api-audit-2026-09-22.md)
- ADR-001 至 ADR-013 审计与替代关系：
  [`verification/reviews/adr-audit-2026-09-10.md`](verification/reviews/adr-audit-2026-09-10.md)
- Novadraw 总体职责边界：[`design/architecture/overview.md`](design/architecture/overview.md)
- 静态结构：[`design/architecture/static-architecture.md`](design/architecture/static-architecture.md)
- 动态时序：[`design/architecture/dynamic-architecture.md`](design/architecture/dynamic-architecture.md)
- 坐标协议：[`design/coordinates/coordinate-system.md`](design/coordinates/coordinate-system.md)
- Scroll/Zoom 输入协议：[`design/input/scroll-zoom-gesture-contract.md`](design/input/scroll-zoom-gesture-contract.md)
- UpdateManager：[`design/rendering/update-manager.md`](design/rendering/update-manager.md)
- Connection / Anchor / Router：
  [`design/architecture/connection-routing.md`](design/architecture/connection-routing.md)
- Reusable Shape 与 Border：
  [`design/architecture/reusable-shape-border.md`](design/architecture/reusable-shape-border.md)
- Text Layout、Label 与 TitleBarBorder：
  [`design/architecture/text-layout.md`](design/architecture/text-layout.md)
- 统一文字测量/图形绘制与自研字体后端输入：
  [`design/rendering/text-graphics-integration.md`](design/rendering/text-graphics-integration.md)
- Clickable、Button 与 Toggle：
  [`design/architecture/basic-widgets.md`](design/architecture/basic-widgets.md)
- Tooltip 与 Accessibility Bridge：
  [`design/architecture/tooltip-accessibility.md`](design/architecture/tooltip-accessibility.md)
- Runtime 派生状态收敛事务：
  [`design/architecture/derived-state-convergence.md`](design/architecture/derived-state-convergence.md)
- 资源因果与 Backend Session：
  [`design/architecture/resource-lifecycle.md`](design/architecture/resource-lifecycle.md)
- Figure 生命周期与 Runtime 身份域：
  [`design/architecture/figure-lifecycle.md`](design/architecture/figure-lifecycle.md)
- Editor 框架边界：
  [`adr/adr-015-editor-framework-boundary.md`](adr/adr-015-editor-framework-boundary.md)
- Editor 规范架构：[`design/editor/architecture.md`](design/editor/architecture.md)
- Editor Viewport/Auto-expose：
  [`design/editor/g5-viewport-autoexpose.md`](design/editor/g5-viewport-autoexpose.md)
- GEF 核心语义覆盖账本：[`parity/gef/api-coverage.md`](parity/gef/api-coverage.md)
- G0-G5 唯一编号与状态：[`roadmap/editor/00-index.md`](roadmap/editor/00-index.md)
- 可执行验证 suite 与命令：[`../verification/suites.toml`](../verification/suites.toml)
- Draw2D API 语义覆盖账本：[`parity/draw2d/api-coverage.md`](parity/draw2d/api-coverage.md)
- M1-M10 唯一编号与状态：[`roadmap/00-index.md`](roadmap/00-index.md)

候选方案不属于核心 SSOT：

- DisplayList 候选协议：[`design/rendering/display-list-protocol.md`](design/rendering/display-list-protocol.md)
- DisplayList 探索计划：[`design/rendering/displaylist-implementation-plan.md`](design/rendering/displaylist-implementation-plan.md)

## 冲突处理

出现文档冲突时按以下顺序判断：

1. 当前已接受 ADR 决定关键取舍；明确的替代关系优先，ADR-013 已由 ADR-014 替换。
   设计决策可经有证据的修订取代，不把历史批准视为不可更改。
2. `design/` 下标记为 `normative-design` 且范围更窄的专题契约优先于架构总览；
   `proposal` 不具有覆盖效力。
3. `parity/` 只解释外部语义到 Novadraw 的映射，不覆盖 Novadraw 设计契约。
4. `reference/` 必须服从参考源码事实，但不能据此推导 Novadraw 必须照搬。
5. `roadmap/`、`verification/`、`migration/` 和 `archive/` 不定义运行时架构。

发现设计与代码不一致时，不允许直接把文档改成代码现状。先判断：

- 设计仍合理：补充设计依据、测试契约，再调整代码。
- 实现形成了更合理的新约束：先修改设计或新增 ADR，再调整测试与索引。
- 只是实施未完成：在设计文档中明确 `target` 状态，并在 roadmap 记录进度。

## 文档类型

新增或大幅修改文档时，应在标题后的说明中明确以下类型之一：

- `documentation-index`
- `reference-analysis`
- `normative-design`
- `parity-contract`
- `architecture-decision`
- `verification`
- `roadmap`
- `product-strategy`
- `migration-guide`
- `archive`
- `proposal`

每篇参考分析应给出仓库、基线提交以及稳定的 class/method 证据；行号仅用于辅助
阅读。每篇设计文档应明确适用范围、失败模式、扩展点和验证入口。

## 阅读路径

### 理解 Novadraw

1. [`design/architecture/overview.md`](design/architecture/overview.md)
2. [`reference/draw2d/architecture/design-axioms.md`](reference/draw2d/architecture/design-axioms.md)
3. [`parity/draw2d/api-coverage.md`](parity/draw2d/api-coverage.md)
4. [`design/coordinates/coordinate-system.md`](design/coordinates/coordinate-system.md)
5. [`design/rendering/update-manager.md`](design/rendering/update-manager.md)

### 开发 Viewport / Scroll / Zoom

1. [`reference/draw2d/figure/scalable-zoom.md`](reference/draw2d/figure/scalable-zoom.md)
2. [`design/input/scroll-zoom-gesture-contract.md`](design/input/scroll-zoom-gesture-contract.md)
3. [`verification/manual/m8-viewport.md`](verification/manual/m8-viewport.md)

### 验证 Web 平台

1. [`adr/adr-001-webgpu-rust-stack.md`](adr/adr-001-webgpu-rust-stack.md)
2. [`verification/manual/web-platform.md`](verification/manual/web-platform.md)
3. [`verification/reviews/m10-web-equivalence-2026-09-13.md`](verification/reviews/m10-web-equivalence-2026-09-13.md)

### 修改 Draw2D 对标语义

1. 核对 `reference/` 中对应源码分析。
2. 更新 [`verification/reference/draw2d-source-audit.md`](verification/reference/draw2d-source-audit.md) 的审计基线或结论。
3. 在 [`parity/draw2d/api-coverage.md`](parity/draw2d/api-coverage.md) 更新受影响 family。
4. 若改变 Novadraw 行为，先更新 `design/` 或 ADR，再修改测试和代码。

### 开发 Editor / GEF 层

1. [`verification/reviews/gef-readiness-analysis-2026-09-13.md`](verification/reviews/gef-readiness-analysis-2026-09-13.md)
2. [`adr/adr-015-editor-framework-boundary.md`](adr/adr-015-editor-framework-boundary.md)
3. [`design/editor/architecture.md`](design/editor/architecture.md)
4. [`parity/gef/api-coverage.md`](parity/gef/api-coverage.md)
5. [`roadmap/editor/00-index.md`](roadmap/editor/00-index.md)

### 理解产品与商业价值

1. [`strategy/commercial-value-analysis.md`](strategy/commercial-value-analysis.md)
2. [`strategy/ai-era-relationship-editor-research.md`](strategy/ai-era-relationship-editor-research.md)
3. [`strategy/ai-graphical-editor-generator.md`](strategy/ai-graphical-editor-generator.md)
4. [`roadmap/00-index.md`](roadmap/00-index.md)
5. [`roadmap/product-deliverables.md`](roadmap/product-deliverables.md)

## 命名与维护

- 文件和目录使用小写 kebab-case；固定入口保留 `00-index.md`。
- 移动文档后必须更新仓库内所有 Markdown 链接和 `AGENTS.md`、`CLAUDE.md`。
- 已失效内容移入 `archive/`，不得继续被设计文档作为当前契约引用。
- 默认检索排除 `doc/archive/`；只有历史追溯才显式读取。
- 原始日志、JSON、探针和审计中间产物放在 `verification/evidence/`，不混入
  `doc/` 的人读信息架构。
- 原始快照、SHA-256 和旧新映射保留在归档入口；不得将新设计写成已有实现证据。
- 设计、路线图和审计报告分别维护，不在一篇文档中重复保存三套状态。
