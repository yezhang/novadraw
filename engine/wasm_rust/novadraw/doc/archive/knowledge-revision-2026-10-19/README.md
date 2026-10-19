# 知识修正原稿

类型：`archive`

本目录保存 2026-10-19 知识修正前的原始字节，供决策与修订追溯。
原稿中的 current、待评审、complete 和推荐 API 只代表当时表达。
当前知识从 [总入口](../../00-index.md)进入，处置见
[修正记录](../../verification/reviews/documentation-knowledge-revision-2026-10-19.md)。

## 替代与归并

| 原稿 | 当前承接 |
|---|---|
| [公共 API 综合提案](original/doc/design/architecture/public-api-experience-proposal.md.txt) | [公共 API 契约](../../design/architecture/public-api-contract.md)、领域设计与 ADR-031/033/035 |
| [2026-09-15 语义迁移提案](original/doc/migration/draw2d-gef-semantic-migration-plan-2026-09-15.md.txt) | [迁移方法](../../migration/java-rust/migration-guide.md)、parity 和 P2 backlog |
| [Editor 实施计划](original/doc/roadmap/editor/implementation-plan.md.txt) | [Editor 状态](../../roadmap/editor/00-index.md)、阶段核验；拆分判据进入目录规范 |
| [ADR-021 原文](original/doc/adr/adr-021-public-facade-and-feature-boundary.md.txt) | [ADR-021 替代指引](../../adr/adr-021-public-facade-and-feature-boundary.md) |
| [DisplayList 探索计划](original/doc/design/rendering/displaylist-implementation-plan.md.txt) | [候选协议与探索门禁](../../design/rendering/display-list-protocol.md) |
| [迁移教程](original/doc/migration/java-rust/migration-guide.md.txt) | [语义迁移方法](../../migration/java-rust/migration-guide.md) |
| [OO 映射](original/doc/migration/java-rust/oo-mapping.md.txt) | [语言特性对照](../../migration/java-rust/oo-mapping.md) |
| [Figure 概览](original/doc/reference/draw2d/figure/core-concepts.md.txt) | [概念地图](../../reference/draw2d/figure/core-concepts.md)与参考专题 |
| [Bounds 分析](original/doc/reference/draw2d/figure/bounds.md.txt) | [bounds 存储与更新](../../reference/draw2d/figure/bounds.md)，转换和命中由独立专题维护 |
| [更新管线分析](original/doc/reference/draw2d/rendering/update-manager.md.txt) | [更新与渲染事实](../../reference/draw2d/rendering/update-manager.md) |
| [Scale/Zoom 分析](original/doc/reference/draw2d/figure/scalable-zoom.md.txt) | [外部缩放事实](../../reference/draw2d/figure/scalable-zoom.md)，项目映射由规范与 parity 承接 |
| [能力补齐规划](original/doc/roadmap/draw2d-capability-enrichment-plan-2026-10-05.md.txt) | [能力分组](../../roadmap/draw2d-capability-enrichment-plan-2026-10-05.md)，动画细节归领域集成 |

## 字节与链接

[manifest.json](manifest.json)记录原路径、快照路径、字节数和 SHA-256。
共保存 51 份修订前文档，除上表外也包括本次更新的入口、规范、账本与报告原稿。
`.md.txt` 原稿不进入当前 Markdown 索引；其内部相对链接按原路径理解。
历史 review 指向这些原稿时只表示当时依据，不能覆盖新规范。
既有 `verification/evidence/` 和此前归档快照未被修改。
