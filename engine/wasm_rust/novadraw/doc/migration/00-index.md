# 迁移指南

类型：`migration-guide`

本目录记录可复用的迁移方法与待评审迁移提案。示例用于解释迁移方法，不构成
Novadraw 当前 API；实际项目契约以 `design/`、已接受 ADR 和 Rust 源码为准。

## 通用指南

- [`java-rust/migration-guide.md`](java-rust/migration-guide.md)：Java 到 Rust 的迁移流程。
- [`java-rust/oo-mapping.md`](java-rust/oo-mapping.md)：继承、接口、所有权和动态分发映射。

## 待评审提案

- [`draw2d-gef-semantic-migration-plan-2026-09-15.md`](draw2d-gef-semantic-migration-plan-2026-09-15.md)：
  基于 Draw2D/GEF 语义审计形成的实施建议，不替代 roadmap。

已完成的架构重构、R8 和 R9 执行计划已移至 [`../archive/`](../archive/00-index.md)。
