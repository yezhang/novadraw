# 验证与审计

类型：`verification`

- `reference/`：外部参考文档与源码的一致性审计
- `reviews/`：阶段性实现审查报告
- `manual/`：窗口、交互和视觉手工验收
- `checklists/`：开发与验证检查清单

本次目录治理和双向一致性结论见
[`reviews/design-code-audit-2026-08-29.md`](reviews/design-code-audit-2026-08-29.md)。

验证文档记录证据与结果，不定义新的架构。发现不一致时，应回到 `design/` 或 ADR
先确定合理契约，再调整实现。
