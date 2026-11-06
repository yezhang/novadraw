# Novadraw

Rust 图形与编辑框架，Core 对标 Draw2D，Editor 对标 GEF。

开发规则见 [AGENTS.md](AGENTS.md) 和 [CLAUDE.md](CLAUDE.md)；
架构、路线图和验证记录从 [文档总入口](doc/00-index.md) 开始阅读。
使用与原理手册见 [book](book/README.md)。

## 目录

| 目录 | 内容 |
|---|---|
| `crates/` | Core、Editor、Inspector、Vello 后端、平台适配、独立 winit Canvas、richtext 与 UI 工作台组件 |
| `crates/novadraw/examples/` | Core 示例与 Native/Web 共用的无平台场景 |
| `crates/novadraw-editor/examples/` | Editor 示例，各应用保留独立 Cargo package |
| `examples/` | 跨层集成示例、后端示例、共享宿主工具和全仓库样例导航 |
| `tests/` | 跨包消费者、Draw2D oracle 资料和公开 API 快照；各 crate 的单元及集成测试仍随包维护 |
| `benchmarks/` | 性能基准工程 |
| `tools/` | xtask 与构建、验收、采样脚本 |
| `assets/` | 运行时共享资源与字体 |
| `doc/` | 架构、ADR、参考资料、路线图和需保存的验证证据 |
| `book/` | 用户、内部原理和贡献者手册 |
| `third_party/` | 只读第三方参考样例 |
| `target/` | 构建产物、临时日志、截图和本地采样输出 |

字体比较报告保存在 [参考报告目录](doc/reference/fonts/reports/00-index.md)；
提交的历史证据保存在 `doc/verification/evidence/`。
这两类材料保留原始内容，其内部历史路径不代表当前目录布局。

样例按主要展示的能力归属到对应 crate；平台作为第二层分类。
完整应用仍是独立 workspace package，不作为库的 Cargo example target，
也不将 GUI 依赖加入 Core／Editor 的 `dev-dependencies`。
全部入口见 [样例导航](examples/README.md)，原有 `cargo run -p <package>` 命令保持不变。

## 构建与验证

包名保持不变，在本目录执行：

```bash
cargo check --workspace
cargo test --workspace
cargo xtask list
cargo xtask docs
```

专项验证、外部消费者和性能采样说明见 [测试入口](tests/README.md)。
脚本位于 `tools/scripts/`；临时输出写入 `target/`。

在根目录执行 `cargo run -p gef-editor-demo` 可启动 egui 编辑画布。
egui／eframe／winit 使用 crates.io 官方发布版，不使用定制分支。
系统默认 Rust 工具链须满足依赖的最低版本要求。
UI 包属于根 workspace，但不在 `default-members` 中；普通 `cargo build/test`
保持原有范围，显式 `--workspace` 验证包含 UI 包。Core/Editor 不依赖 UI 包。
可选 Masonry/Xilem 宿主仍保留独立依赖，接入见
[平台适配包](crates/novadraw-platform-masonry/README.md)；
其历史迁移与原生验收记录见 [迁移记录](doc/verification/reviews/masonry-upstream-migration-2026-10-28.md)。

本地 egui 工作台见 [novadraw-ui](crates/novadraw-ui/README.md)，运行
`cargo run -p novadraw-workbench-app` 可验证停靠布局、原生浮窗和 Vello Canvas。
共享 GPU 链路为官方 Vello 0.11／WGPU 30、egui 0.37 与 winit 0.30；
工作台源码在仓库本地维护。
