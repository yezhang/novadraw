# 架构决策记录 (ADR)

类型：`architecture-decision`

## 什么是 ADR

ADR (Architecture Decision Record) 是记录架构决策的文档，用于记录项目中重要的设计决策及其上下文。

## ADR 列表

| 编号 | 标题 | 状态 | 日期 |
|------|------|------|------|
| [001](adr-001-webgpu-rust-stack.md) | 使用 Rust + WebGPU 实现图形框架 | 已通过 | 2025-01-13 |
| [002](adr-002-notification-effect-queue.md) | 采用 Draw2D 语义分层与 Zed 式 effect queue 的通知机制 | 已通过 | 2026-05-06 |
| [003](adr-003-rust-runtime-and-geometry-boundaries.md) | Rust Runtime 所有权与二维几何边界 | 已通过 | 2026-08-30 |

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
