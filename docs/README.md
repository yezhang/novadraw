# Novadraw 根文档入口

本目录只保存仓库级说明和第三方参考源码。Novadraw 当前的规范、架构决策、路线图、
验证证据与三本使用手册，统一维护在 Rust workspace 内。

## 文档导航

| 内容 | 入口 | 事实层级 |
|---|---|---|
| 仓库目录、历史边界与 Web Worker 定位 | [repository-layout.md](repository-layout.md) | 仓库级说明 |
| 工程文档总入口 | [doc/00-index.md](../engine/wasm_rust/novadraw/doc/00-index.md) | 当前设计与状态导航 |
| 应用开发与扩展指南 | [book/user](../engine/wasm_rust/novadraw/book/user/src/README.md) | 面向使用者 |
| 核心原理书 | [book/internals](../engine/wasm_rust/novadraw/book/internals/src/README.md) | 稳定模型与算法 |
| 贡献者指南 | [book/contributor](../engine/wasm_rust/novadraw/book/contributor/src/README.md) | 仓库开发流程 |
| 第三方参考源码 | [third_party](third_party/README.md) | 上游事实核验，不是项目 API |

## 维护规则

- 当前行为只由 workspace 内的规范性设计和已接受 ADR 定义；
- 实现与交付状态只从 roadmap 和 verification 入口读取；
- 第三方源码只用于语义核验，不能作为 Novadraw 自有代码或公共 API；
- 过期设计进入 workspace 的 `doc/archive/`，不在根目录维护第二套架构文档；
- README 只提供项目入口，不重复维护完整能力清单和架构规范。
