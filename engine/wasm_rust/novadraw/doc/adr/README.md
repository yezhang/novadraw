# 架构决策记录 (ADR)

类型：`architecture-decision`

## 什么是 ADR

ADR (Architecture Decision Record) 是记录架构决策的文档，用于记录项目中重要的设计决策及其上下文。

## ADR 列表

2026-09-10 已完成 001-013 的设计审计。Draw2D Core 当前修订裁决：
[ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md)。
Draw2D Core 1.0 之后的 Editor 框架边界由
[ADR-015](adr-015-editor-framework-boundary.md) 接受；
[审计与逐项处置](../verification/reviews/adr-audit-2026-09-10.md)；
[历史快照入口](../archive/adr-audit-2026-09-10/README.md)。
001-012 保留合理主线并同步修订正文；新契约不等于代码已实现。

| 编号 | 标题 | 状态 | 日期 |
|------|------|------|------|
| [001](adr-001-webgpu-rust-stack.md) | 使用 Rust + WebGPU 实现图形框架 | 已通过 | 2025-01-13 |
| [002](adr-002-notification-effect-queue.md) | 采用 Draw2D 语义分层与 Zed 式 effect queue 的通知机制 | 已通过 | 2026-05-06 |
| [003](adr-003-rust-runtime-and-geometry-boundaries.md) | Rust Runtime 所有权与二维几何边界 | 已通过 | 2026-08-30 |
| [004](adr-004-layer-and-freeform-contract.md) | Layer 与 Freeform 范围契约 | 已通过 | 2026-09-04 |
| [005](adr-005-connection-routing-contract.md) | Connection 路由与依赖状态边界 | 已通过 | 2026-09-06 |
| [006](adr-006-reusable-shape-border-contract.md) | Reusable Shape 与 Border 产品化边界 | 已通过 | 2026-09-07 |
| [007](adr-007-parley-text-layout.md) | 可替换文本布局与后端无关 Glyph IR | 已通过 | 2026-09-07 |
| [008](adr-008-m9-contract-recovery.md) | M9 共享 Manhattan 与 Viewport Topology 收口 | 已通过 | 2026-09-08 |
| [009](adr-009-runtime-dynamic-mutation-contract.md) | Runtime 动态 Mutation 事务 | 已通过 | 2026-09-08 |
| [010](adr-010-runtime-listener-lifecycle.md) | Runtime Listener 生命周期 | 已通过 | 2026-09-08 |
| [011](adr-011-derived-state-convergence.md) | Runtime 派生状态收敛事务 | 已通过 | 2026-09-09 |
| [012](adr-012-resource-causality-and-backend-session.md) | 资源因果日志与 Backend Session | 已通过 | 2026-09-09 |
| [013](adr-013-figure-lifecycle-and-runtime-identity.md) | Figure 生命周期与 Runtime 身份域 | 已被 014 替换 | 2026-09-09 |
| [014](adr-014-extensibility-and-lifecycle-boundaries.md) | 扩展协议、生命周期与稳定发布边界修订 | 已接受，已验证 | 2026-09-10 |
| [015](adr-015-editor-framework-boundary.md) | 独立 Editor 框架边界 | 已接受 | 2026-09-13 |
| [016](adr-016-figure-inspector-observability.md) | FigureInspector 可观测性边界 | 已接受 | 2026-09-22 |
| [017](adr-017-core-public-api-boundary.md) | Core 公开 API 边界与失败契约 | 已接受 | 2026-09-24 |
| [018](adr-018-runtime-driving-and-measurement-api.md) | Runtime 驱动与结构化测量 API | 已接受 | 2026-09-24 |
| [019](adr-019-composable-api-and-scoped-editors.md) | 可组装 API 与 Scoped Editor | 已接受，已验证 | 2026-09-28 |
| [020](adr-020-engine-value-and-render-contract.md) | 引擎基础值与公开渲染契约 | 已接受，已验证 | 2026-09-28 |
| [021](adr-021-public-facade-and-feature-boundary.md) | 公开 Facade 与 Feature 边界 | 部分由 023 替代 | 2026-09-28 |
| [022](adr-022-third-party-type-and-render-dependency-boundary.md) | 第三方类型与渲染依赖边界 | 部分由 023 替代 | 2026-09-29 |
| [023](adr-023-crate-consolidation-and-extension-boundaries.md) | Crate 收口与扩展边界 | 已接受，已验证 | 2026-09-29 |

## 现行与历史隔离

本目录正文只包含现行决策或替代指引。失效全文移至 `doc/archive/`，不在当前正文
保留“历史候选仍可选”的叙述。追溯旧方案时显式进入归档，不默认检索归档。
“已通过”表示设计效力；实现与验收状态只以 roadmap/verification 为准。

## ADR 模板

```markdown
# ADR-XXX: [标题]

## 状态

[提议/已通过/已废弃/已替换]

## 背景

[描述问题和上下文]

## 决策

[描述选择的方案]

## 后果

### 正面
- ...

### 负面
- ...

## 参考
- ...

## 日期
YYYY-MM-DD
```

## 创建新的 ADR

1. 在 `doc/adr/` 目录创建新文件，命名格式：`adr-XXX-标题.md`
2. 使用上述模板填写内容
3. 更新本 README.md 添加条目
