# 迁移指南

类型：`documentation-index`

本目录记录可复用的迁移方法。示例用于解释迁移方法，不构成
Novadraw 当前 API；实际项目契约以 `design/`、已接受 ADR 和 Rust 源码为准。

## 通用指南

- [`java-rust/migration-guide.md`](java-rust/migration-guide.md)：Java 到 Rust 的迁移流程。
- [`java-rust/oo-mapping.md`](java-rust/oo-mapping.md)：继承、接口、所有权和动态分发映射。

已取代的语义迁移提案、已完成的架构重构、R8/R9 和 Editor 实施计划从
[历史入口](../archive/00-index.md)追溯。当前行为差异见
[parity](../parity/00-index.md)，执行顺序见 [P2 backlog](../roadmap/p2-delta-backlog.md)。
