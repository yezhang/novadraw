# 验证与审计

类型：`verification`

- `reference/`：外部参考文档与源码的一致性审计
- `reviews/`：阶段性实现审查报告
- `manual/`：窗口、交互和视觉手工验收
- `performance/`：可重复执行的性能基线与前后对比
- `checklists/`：开发与验证检查清单

本次目录治理和双向一致性结论见
[`reviews/design-code-audit-2026-08-29.md`](reviews/design-code-audit-2026-08-29.md)。

## 手工验证入口

- [`manual/core-pipeline.md`](manual/core-pipeline.md)：M1-M7 核心渲染与事件链路；
- [`manual/m8-viewport.md`](manual/m8-viewport.md)：Viewport、Scroll 与 Zoom；
- [`manual/d1-selection.md`](manual/d1-selection.md)：selection 外移后的 editor 选择反馈；
- [`manual/d1-figure-style.md`](manual/d1-figure-style.md)：FigureStyle 继承、覆盖与 cursor；
- [`manual/d1-focus-traversal.md`](manual/d1-focus-traversal.md)：Native/Web
  Tab/Shift+Tab 焦点遍历与边界；
- [`manual/web-platform.md`](manual/web-platform.md)：Wasm 构建、静态资源服务、
  Vello WebGPU 与 Canvas2D 浏览器验收。
- [`performance/r8-baseline-2026-09-02.md`](performance/r8-baseline-2026-09-02.md)：
  R8 canonical 名称迁移前 CPU 基线。

验证文档记录证据与结果，不定义新的架构。发现不一致时，应回到 `design/` 或 ADR
先确定合理契约，再调整实现。
