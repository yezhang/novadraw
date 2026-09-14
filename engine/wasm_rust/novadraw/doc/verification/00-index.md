# 验证与审计

类型：`verification`

- `reference/`：外部参考文档与源码的一致性审计
- `reviews/`：阶段性实现审查报告
- `manual/`：窗口、交互和视觉手工验收
- `performance/`：可重复执行的性能基线与前后对比
- `checklists/`：开发与验证检查清单

ADR-001 至 ADR-013 的设计审计、源码证据、旧新映射与剩余门禁见
[`reviews/adr-audit-2026-09-10.md`](reviews/adr-audit-2026-09-10.md)。
这是设计审计，不是新增 Runtime 验证；历史通过记录不能覆盖 ADR-014 新契约。

ADR-014 首批实验实现、同树 Runtime 重包装身份复活及暂停门禁见
[`reviews/adr014-implementation-gate-2026-09-10.md`](reviews/adr014-implementation-gate-2026-09-10.md)。

ADR-014 D4.4 的结构化 topology 错误、外部 TextLayout、frame 状态与 backend
session baseline 增量见
[`reviews/adr014-d4.4-increment-2026-09-10.md`](reviews/adr014-d4.4-increment-2026-09-10.md)。

ADR-014 D4.5 的递归 style/validation 性能恢复、1k/10k 前后基准与全链路深度门禁见
[`reviews/adr014-d4.5-performance-2026-09-10.md`](reviews/adr014-d4.5-performance-2026-09-10.md)。

ADR-014 D4.6 的 A01-A08 关闭矩阵与 Rust/WASM/Headless/Native 最终门禁见
[`reviews/adr014-d4.6-completion-2026-09-10.md`](reviews/adr014-d4.6-completion-2026-09-10.md)。

M10.5 Tooltip 状态机、Accessibility Snapshot/Delta、Native/Web bridge 与自动门禁见
[`reviews/m10.5-tooltip-accessibility-2026-09-10.md`](reviews/m10.5-tooltip-accessibility-2026-09-10.md)。

macOS live resize 的 CAMetalLayer/Core Animation transaction 根因、失败方案与升级
回归门禁见
[`reviews/macos-live-resize-transaction-2026-09-12.md`](reviews/macos-live-resize-transaction-2026-09-12.md)。

M1-M7 核心管线与 M8 Viewport/Scroll/Zoom 的自动、Native 和 Web 收口证据见
[`reviews/m1-m8-manual-acceptance-2026-09-12.md`](reviews/m1-m8-manual-acceptance-2026-09-12.md)。

M10 Text/Image/Widget 共享场景、WebGPU 资源状态、交互、Tooltip 与 accessibility
等价验收见
[`reviews/m10-web-equivalence-2026-09-13.md`](reviews/m10-web-equivalence-2026-09-13.md)。

Draw2D Core 1.0 的 Rust/Wasm/Headless/Native/Web 总审计、P0/P1 账本与 R9.4
capability 消融结论见
[`reviews/draw2d-core-1.0-final-audit-2026-09-13.md`](reviews/draw2d-core-1.0-final-audit-2026-09-13.md)。

Draw2D Core 到独立 Editor/GEF 层的启动条件、源码证据、缺口分级与 crate 边界见
[`reviews/gef-readiness-analysis-2026-09-13.md`](reviews/gef-readiness-analysis-2026-09-13.md)。

Editor G1 Model Adapter、Command/CompoundCommand、CommandStack、dirty/save 与 fault
边界见
[`reviews/g1-model-command-completion-2026-09-13.md`](reviews/g1-model-command-completion-2026-09-13.md)。

Editor G2 EditPart identity/tree、Viewer registry、model notification 与增量
containment 投影见
[`reviews/g2-viewer-projection-completion-2026-09-13.md`](reviews/g2-viewer-projection-completion-2026-09-13.md)。

Editor G3 selection、targeting、root layers 与 Figure/Editor 输入仲裁见
[`reviews/g3-selection-targeting-behavior-2026-09-13.md`](reviews/g3-selection-targeting-behavior-2026-09-13.md)；
人工验收步骤见
[`manual/g3-selection-targeting.md`](manual/g3-selection-targeting.md)。

Editor G4 Tool/Request/EditPolicy 编辑闭环与检查点 B 结果见
[`reviews/g4-editing-loop-behavior-2026-09-14.md`](reviews/g4-editing-loop-behavior-2026-09-14.md)。

Editor G5.1 Connection Projection 的设计复核、实现和自动门禁见
[`reviews/g5-connection-projection-design-2026-09-14.md`](reviews/g5-connection-projection-design-2026-09-14.md)。

本次目录治理和双向一致性结论见
[`reviews/design-code-audit-2026-08-29.md`](reviews/design-code-audit-2026-08-29.md)。

Draw2D Core 1.0 能力覆盖、公共 API 可达性与路线图一致性审计见
[`reviews/draw2d-core-capability-audit-2026-09-08.md`](reviews/draw2d-core-capability-audit-2026-09-08.md)。

D3.2 Runtime 动态 mutation 的实现与自动门禁见
[`reviews/d3-runtime-mutation-2026-09-08.md`](reviews/d3-runtime-mutation-2026-09-08.md)。

D3.3 Runtime listener 公共面与 self-removal 语义验证见
[`reviews/d3-runtime-listener-2026-09-08.md`](reviews/d3-runtime-listener-2026-09-08.md)。

长期架构可持续性审计及 D4.0 当前 HEAD 校准见
[`reviews/architecture-sustainability-review-2026-09-08.md`](reviews/architecture-sustainability-review-2026-09-08.md)
和 [`reviews/d4-audit-calibration-2026-09-09.md`](reviews/d4-audit-calibration-2026-09-09.md)。

D4.1 typed worklist、自动路由、文本双阶段与 Viewport 延迟提交验证见
[`reviews/d4-derived-state-convergence-2026-09-09.md`](reviews/d4-derived-state-convergence-2026-09-09.md)。

D4.2 ordered resource ops、Ready snapshot 与 Backend Session 验证见
[`reviews/d4-resource-causality-and-backend-session-2026-09-09.md`](reviews/d4-resource-causality-and-backend-session-2026-09-09.md)。

## 手工验证入口

- [`manual/core-pipeline.md`](manual/core-pipeline.md)：M1-M7 核心渲染与事件链路；
- [`manual/m8-viewport.md`](manual/m8-viewport.md)：Viewport、Scroll 与 Zoom；
- [`manual/d1-figure-style.md`](manual/d1-figure-style.md)：FigureStyle 继承、覆盖与 cursor；
- [`manual/d1-focus-traversal.md`](manual/d1-focus-traversal.md)：Native/Web
  Tab/Shift+Tab 焦点遍历与边界；
- [`manual/m10-widgets.md`](manual/m10-widgets.md)：M10.4 Button/Toggle
  pointer、keyboard 与状态视觉验收；
- [`manual/m10-tooltip-accessibility.md`](manual/m10-tooltip-accessibility.md)：M10.5
  Tooltip delay/placement、Accessibility focus/default action 与 Web bridge；
- [`manual/d3-m9-contract-recovery.md`](manual/d3-m9-contract-recovery.md)：D3.1
  shared Manhattan 与严格 viewport topology 增量验收；
- [`manual/web-platform.md`](manual/web-platform.md)：Wasm 构建、静态资源服务、
  Vello WebGPU 与 Canvas2D 浏览器验收。
- [`performance/r8-baseline-2026-09-02.md`](performance/r8-baseline-2026-09-02.md)：
  R8 canonical 名称迁移前 CPU 基线。

验证文档记录证据与结果，不定义新的架构。发现不一致时，应回到 `design/` 或 ADR
先确定合理契约，再调整实现。
