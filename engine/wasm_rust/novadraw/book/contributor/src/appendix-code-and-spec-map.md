# 附录：代码与规范地图

本附录用于开始定向阅读，不代替模块自己的文档和测试。

## A. 项目入口

| 内容 | 入口 |
|---|---|
| 项目规则 | [`AGENTS.md`](../../../AGENTS.md)、[`CLAUDE.md`](../../../CLAUDE.md) |
| 文档总索引 | [`doc/00-index.md`](../../../doc/00-index.md) |
| ADR 索引 | [`doc/adr/README.md`](../../../doc/adr/README.md) |
| 验证清单 | [`verification/suites.toml`](../../../verification/suites.toml) |
| 用户指南 | <a href="../../user/html/index.html">《Novadraw 应用开发与扩展指南》</a> |
| 核心原理 | <a href="../../internals/html/index.html">《深入理解 Novadraw》</a> |

## B. Core

| 主题 | 代码入口 | 规范入口 |
|---|---|---|
| Figure 与能力 | [`novadraw/src/figure`](../../../novadraw/src/figure) | [`figure-capability-model.md`](../../../doc/design/architecture/figure-capability-model.md) |
| Figure 树 | [`novadraw/src/graph`](../../../novadraw/src/graph) | [`architecture`](../../../doc/design/architecture/00-index.md) |
| Runtime | [`novadraw/src/runtime`](../../../novadraw/src/runtime) | [`ADR-018`](../../../doc/adr/adr-018-runtime-driving-and-measurement-api.md) |
| 几何与坐标 | [`novadraw/src/geometry`](../../../novadraw/src/geometry) | [`coordinate-system.md`](../../../doc/design/coordinates/coordinate-system.md) |
| 布局 | [`novadraw/src/layout`](../../../novadraw/src/layout) | [`Draw2D API 账本`](../../../doc/parity/draw2d/api-coverage.md) |
| 连接 | [`novadraw/src/connection`](../../../novadraw/src/connection) | [`connection-routing`](../../../doc/design/architecture/connection-routing.md) |
| 渲染协议 | [`novadraw/src/render`](../../../novadraw/src/render) | [`ADR-025`](../../../doc/adr/adr-025-unified-graphics-and-glyph-preparation.md) |

## C. Editor

| 主题 | 代码入口 |
|---|---|
| 模型适配 | [`novadraw-editor/src/model`](../../../novadraw-editor/src/model) |
| EditPart | [`novadraw-editor/src/part`](../../../novadraw-editor/src/part) |
| Viewer | [`novadraw-editor/src/viewer`](../../../novadraw-editor/src/viewer) |
| 请求与策略 | [`request`](../../../novadraw-editor/src/request)、[`policy`](../../../novadraw-editor/src/policy) |
| 命令历史 | [`novadraw-editor/src/command`](../../../novadraw-editor/src/command) |
| 工具与编辑域 | [`tool`](../../../novadraw-editor/src/tool)、[`domain.rs`](../../../novadraw-editor/src/domain.rs) |

Editor 的规范入口：

- [`Editor 架构`](../../../doc/design/editor/architecture.md)
- [`Editor 路线图`](../../../doc/roadmap/editor/00-index.md)
- [`GEF API 语义账本`](../../../doc/parity/gef/api-coverage.md)

## D. 平台与后端

| 责任 | 代码入口 |
|---|---|
| Vello backend | [`novadraw-backend-vello`](../../../novadraw-backend-vello) |
| Winit adapter | [`novadraw-platform-winit`](../../../novadraw-platform-winit) |
| Web adapter | [`novadraw-platform-web`](../../../novadraw-platform-web) |
| Native 示例宿主 | [`examples/support`](../../../examples/support) |

## E. 验证入口

```bash
cargo xtask list
cargo xtask docs
cargo xtask check --quick
cargo xtask check --full
```

选择 suite 时先按修改路径和语义范围查询
[`verification/suites.toml`](../../../verification/suites.toml)，不要凭章节中的示例命令
推断当前门禁。
