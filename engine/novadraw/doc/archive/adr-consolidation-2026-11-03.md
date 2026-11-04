# ADR 长期边界收敛记录

类型：`archive`

能力归属：Core／Editor／后端与平台的文档分类；不改变运行时能力。

2026-11-03：现行 ADR 从 52 篇收敛为 10 篇。保留的正文只记录长期所有权、
依赖、身份、发布和失败边界的选择与取舍；领域协议、算法、参数和组件组合归 design。
013、021 是已替代记录，只归档。060 仍是未接受的修订 1 提议，不因迁移获得授权。

本次是表达归并，不重新接受架构。原批准日期、修订效力和局部替代关系保留，
实现状态由 roadmap／verification 管理。编号不重排、不复用；不保留空壳 ADR。
原稿逐篇保留原始字节，路径及 SHA-256 见
[校验清单](adr-consolidation-2026-11-03/manifest.json)；维护前整库基线见
[备份清单](../verification/evidence/adr-consolidation-2026-11-03/manifest.json)。
历史 review 引用原稿，当前契约引用专题；当前检索不默认进入归档。

## 逐篇去向

| 原编号 | 处置 | 当前入口与原因 | 原始字节 |
|---|---|---|---|
| 001 | 保留并压缩 | [Rust 与 WebGPU 技术栈](../adr/adr-001-webgpu-rust-stack.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-001-webgpu-rust-stack.md.txt) |
| 002 | 并入专题设计 | [通知与内部失效分离](../design/rendering/update-manager.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-002-notification-effect-queue.md.txt) |
| 003 | 保留并压缩 | [Runtime 所有权与二维坐标边界](../adr/adr-003-rust-runtime-and-geometry-boundaries.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-003-rust-runtime-and-geometry-boundaries.md.txt) |
| 004 | 并入专题设计 | [Layer 与 Freeform](../design/architecture/layer-and-freeform.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-004-layer-and-freeform-contract.md.txt) |
| 005 | 并入专题设计 | [连接路由](../design/architecture/connection-routing.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-005-connection-routing-contract.md.txt) |
| 006 | 并入专题设计 | [Shape 与 Border](../design/architecture/reusable-shape-border.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-006-reusable-shape-border-contract.md.txt) |
| 007 | 并入专题设计 | [文字布局](../design/architecture/text-layout.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-007-parley-text-layout.md.txt) |
| 008 | 并入专题设计 | [共享路由与视口准入](../design/architecture/connection-routing.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-008-m9-contract-recovery.md.txt) |
| 009 | 并入专题设计 | [Mutation 与子树准入](../design/architecture/runtime-mutation-and-subtree-admission.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-009-runtime-dynamic-mutation-contract.md.txt) |
| 010 | 并入专题设计 | [通知订阅生命周期](../design/architecture/notification-subscription-lifecycle.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-010-runtime-listener-lifecycle.md.txt) |
| 011 | 并入专题设计 | [派生状态收敛](../design/architecture/derived-state-convergence.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-011-derived-state-convergence.md.txt) |
| 012 | 并入专题设计 | [资源因果与 Backend Session](../design/architecture/resource-lifecycle.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-012-resource-causality-and-backend-session.md.txt) |
| 013 | 历史归档 | [已替代生命周期记录](adr-consolidation-2026-11-03/original/doc/adr/adr-013-figure-lifecycle-and-runtime-identity.md.txt) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-013-figure-lifecycle-and-runtime-identity.md.txt) |
| 014 | 保留并压缩 | [分层原子性、生命周期与稳定发布](../adr/adr-014-extensibility-and-lifecycle-boundaries.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-014-extensibility-and-lifecycle-boundaries.md.txt) |
| 015 | 保留并压缩 | [Core、Editor 与应用模型边界](../adr/adr-015-editor-framework-boundary.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-015-editor-framework-boundary.md.txt) |
| 016 | 并入专题设计 | [Inspector 只读诊断](../design/architecture/figure-inspector.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-016-figure-inspector-observability.md.txt) |
| 017 | 并入专题设计 | [构建与运行期公共 API](../design/architecture/public-api-contract.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-017-core-public-api-boundary.md.txt) |
| 018 | 并入专题设计 | [驱动与结构化测量](../design/architecture/public-api-contract.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-018-runtime-driving-and-measurement-api.md.txt) |
| 019 | 并入专题设计 | [Owned 与 Scoped API](../design/architecture/public-api-contract.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-019-composable-api-and-scoped-editors.md.txt) |
| 020 | 并入专题设计 | [引擎基础值](../design/architecture/engine-value-contract.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-020-engine-value-and-render-contract.md.txt) |
| 021 | 历史归档 | [已替代 Facade 记录](adr-consolidation-2026-11-03/original/doc/adr/adr-021-public-facade-and-feature-boundary.md.txt) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-021-public-facade-and-feature-boundary.md.txt) |
| 022 | 并入专题设计 | [第三方类型与依赖规范](../design/architecture/third-party-dependency-boundaries.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-022-third-party-type-and-render-dependency-boundary.md.txt) |
| 023 | 保留并压缩 | [发布包、公共路径与第三方依赖边界](../adr/adr-023-crate-consolidation-and-extension-boundaries.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-023-crate-consolidation-and-extension-boundaries.md.txt) |
| 024 | 并入专题设计 | [Paint、Stroke 与 Clip](../design/rendering/p2-g01-graphics-extension.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-024-graphics-paint-stroke-and-clipping.md.txt) |
| 025 | 保留并压缩 | [图文协议、独立录制与后端预处理边界](../adr/adr-025-unified-graphics-and-glyph-preparation.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-025-unified-graphics-and-glyph-preparation.md.txt) |
| 026 | 保留并压缩 | [源状态与动画表现分离](../adr/adr-026-animation-and-presentation-plane.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-026-animation-and-presentation-plane.md.txt) |
| 027 | 保留并压缩 | [类型化身份与统一开放扩展模型](../adr/adr-027-open-figure-capability-registry.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-027-open-figure-capability-registry.md.txt) |
| 028 | 并入专题设计 | [类型化属性与通知日志](../design/architecture/typed-property-identity.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-028-typed-property-identity.md.txt) |
| 029 | 并入专题设计 | [后端失败与恢复](../design/rendering/backend-failure-and-recovery.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-029-structured-render-backend-failure.md.txt) |
| 030 | 并入专题设计 | [资源加载与请求 Token](../design/architecture/resource-loading-and-request-tokens.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-030-resource-provider-and-request-token-boundary.md.txt) |
| 031 | 并入专题设计 | [录制与帧出版](../design/rendering/graphics-recording-and-publication.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-031-graphics-recording-and-publication-boundary.md.txt) |
| 032 | 并入专题设计 | [Viewer 编辑会话](../design/editor/viewer-edit-session-and-drive.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-032-viewer-edit-session-and-drive-boundary.md.txt) |
| 033 | 并入专题设计 | [Runtime 子系统 Facade](../design/architecture/runtime-subsystem-facade.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-033-runtime-subsystem-facade.md.txt) |
| 034 | 并入专题设计 | [Editor 开放交互协议](../design/editor/tool-request-handle-protocol.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-034-editor-tool-request-handle-protocol.md.txt) |
| 035 | 并入专题设计 | [唯一领域导入](../design/architecture/public-api-contract.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-035-explicit-domain-imports-and-root-boundary.md.txt) |
| 036 | 并入专题设计 | [阶段权限投影](../design/architecture/context-phase-authority.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-036-context-phase-authority-projection.md.txt) |
| 037 | 并入专题设计 | [Editor 增量失败隔离](../design/editor/incremental-failure-isolation.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-037-editor-incremental-failure-isolation.md.txt) |
| 038 | 保留并压缩 | [身份、合法缺失与领域失败分离](../adr/adr-038-query-identity-and-failure-semantics.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-038-query-identity-and-failure-semantics.md.txt) |
| 040 | 并入专题设计 | [连接组求解](../design/architecture/group-route-solving.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-040-group-route-solving.md.txt) |
| 042 | 并入专题设计 | [结构化 Flow](../design/architecture/structured-flow-layout.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-042-structured-flow-layout.md.txt) |
| 043 | 并入专题设计 | [有向图布局](../design/architecture/directed-compound-graph-layout.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-043-directed-compound-graph-layout.md.txt) |
| 044 | 保留并压缩 | [可变场景所有权与不可变跨 Runtime 捕获](../adr/adr-044-shared-scene-capture-and-view.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-044-shared-scene-capture-and-view.md.txt) |
| 045 | 并入专题设计 | [宿主背景帧提交](../design/architecture/shared-scene-view-background-proposal.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-045-host-background-submission.md.txt) |
| 046 | 并入专题设计 | [表现几何与动画容量](../design/animation/tree-layout-presentation-proposal.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-046-sampled-geometry-and-animation-reservations.md.txt) |
| 047 | 并入专题设计 | [逐操作抗锯齿](../design/rendering/shape-antialias-proposal.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-047-per-operation-antialias.md.txt) |
| 051 | 并入专题设计 | [精确 Flow 复用](../design/architecture/exact-flow-layout-reuse.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-051-exact-flow-layout-reuse.md.txt) |
| 055 | 并入专题设计 | [容器编辑](../design/editor/container-editing-protocol.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-055-container-editing-protocol.md.txt) |
| 056 | 并入专题设计 | [Detached 输入不变量](../design/architecture/detached-input-invariants.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-056-detached-input-invariants.md.txt) |
| 057 | 并入专题设计 | [服务失败与订阅恢复](../design/architecture/service-failure-and-inspector-lifecycle.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-057-service-failure-and-inspector-lifecycle.md.txt) |
| 058 | 并入专题设计 | [Editor 开放吸附](../design/editor/open-snapping.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-058-open-editor-snapping.md.txt) |
| 059 | 并入专题设计 | [可选阶段文字协议](../design/architecture/staged-text-width-reuse.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt) |
| 060 | 并入专题设计 | [FR-02 贡献组装复用提议](../design/architecture/flow-assembly-reuse.md) | [原稿](adr-consolidation-2026-11-03/original/doc/adr/adr-060-flow-assembly-reuse.md.txt) |

039、041、048、049、050、052、053、054 已在前次分类中迁出，本次不重新处置，
其去向及原稿见[前次分类记录](adr-classification-2026-11-03.md)。

保留 10 项：技术栈、Runtime 所有权和坐标、分层原子性和生命周期、
Core／Editor／应用分工、发布包和依赖、图文与后端预处理、动画表现、
统一开放扩展、身份与失败、不可变跨 Runtime 捕获。

## 交付校验

- ADR 正文 52 → 10 篇，含索引的行数 4,458 → 454，减少约 90%。
- 40 篇内容并入专题设计，2 篇已替代记录归档；具体方案不保留空壳 ADR。
- 补齐通知订阅、基础值、第三方依赖和属性身份四个专题，均可从文档总入口到达。
- 52 份 ADR 原稿与维护前 620 份 Markdown 备份逐份通过字节数和 SHA-256 核验。
- `cargo xtask docs` 通过：473 份 Markdown 与当前命令引用。
- `git diff --check` 通过；本次产生的本地路径和锚点问题已全部处理。

历史页只调整导航，不改原判断；原始快照保持原始字节。
生成的 deepwiki 仍有先前的旧源码与仓库相对路径问题，不作为现行规范入口。
本次未修改 Rust，也未执行 Git 提交。
