# 附录：代码与规范地图

本附录用于开始定向阅读，不代替模块自己的文档和测试。

## A. 项目入口

| 内容 | 入口 |
|---|---|
| 项目规则 | [`AGENTS.md`](../../../AGENTS.md)、[`CLAUDE.md`](../../../CLAUDE.md) |
| 文档总索引 | [`doc/00-index.md`](../../../doc/00-index.md) |
| ADR 索引 | [`doc/adr/README.md`](../../../doc/adr/README.md) |
| 验证入口 | [`tests/README.md`](../../../tests/README.md) |
| 用户指南 | <a href="../../user/html/index.html">《Novadraw 应用开发与扩展指南》</a> |
| 核心原理 | <a href="../../internals/html/index.html">《深入理解 Novadraw》</a> |

## B. Core

| 主题 | 代码入口 | 规范入口 |
|---|---|---|
| Figure 与能力 | [`crates/novadraw/src/figure`](../../../crates/novadraw/src/figure) | [`figure-capability-model.md`](../../../doc/design/architecture/figure-capability-model.md) |
| Figure 树 | [`crates/novadraw/src/graph`](../../../crates/novadraw/src/graph) | [`architecture`](../../../doc/design/architecture/00-index.md) |
| Runtime | [`crates/novadraw/src/runtime`](../../../crates/novadraw/src/runtime) | [`ADR-018`](../../../doc/adr/adr-018-runtime-driving-and-measurement-api.md) |
| 几何与坐标 | [`crates/novadraw/src/geometry`](../../../crates/novadraw/src/geometry) | [`coordinate-system.md`](../../../doc/design/coordinates/coordinate-system.md) |
| 布局 | [`crates/novadraw/src/layout`](../../../crates/novadraw/src/layout) | [`Draw2D API 账本`](../../../doc/parity/draw2d/api-coverage.md) |
| 连接 | [`crates/novadraw/src/connection`](../../../crates/novadraw/src/connection) | [`connection-routing`](../../../doc/design/architecture/connection-routing.md) |
| 渲染协议 | [`crates/novadraw/src/render`](../../../crates/novadraw/src/render) | [`ADR-025`](../../../doc/adr/adr-025-unified-graphics-and-glyph-preparation.md) |

## C. Editor

| 主题 | 代码入口 |
|---|---|
| 模型适配 | [`crates/novadraw-editor/src/model`](../../../crates/novadraw-editor/src/model) |
| EditPart | [`crates/novadraw-editor/src/part`](../../../crates/novadraw-editor/src/part) |
| Viewer | [`crates/novadraw-editor/src/viewer`](../../../crates/novadraw-editor/src/viewer) |
| 请求与策略 | [`request`](../../../crates/novadraw-editor/src/request)、[`policy`](../../../crates/novadraw-editor/src/policy) |
| 命令历史 | [`crates/novadraw-editor/src/command`](../../../crates/novadraw-editor/src/command) |
| 工具与编辑域 | [`tool`](../../../crates/novadraw-editor/src/tool)、[`domain.rs`](../../../crates/novadraw-editor/src/domain.rs) |

Editor 的规范入口：

- [`Editor 架构`](../../../doc/design/editor/architecture.md)
- [`Editor 路线图`](../../../doc/roadmap/editor/00-index.md)
- [`GEF API 语义账本`](../../../doc/parity/gef/api-coverage.md)

## D. 平台与后端

| 责任 | 代码入口 |
|---|---|
| Vello backend | [`novadraw-backend-vello`](../../../crates/novadraw-backend-vello) |
| Winit adapter | [`novadraw-platform-winit`](../../../crates/novadraw-platform-winit) |
| Web adapter | [`novadraw-platform-web`](../../../crates/novadraw-platform-web) |
| Native 示例宿主 | [`examples/support`](../../../examples/support) |

## E. 验证入口

```bash
cargo xtask list
cargo xtask docs
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

选择跨工具 suite 时查看 [`tests/README.md`](../../../tests/README.md)
和 `cargo xtask list`，单个测试直接使用 Cargo。
