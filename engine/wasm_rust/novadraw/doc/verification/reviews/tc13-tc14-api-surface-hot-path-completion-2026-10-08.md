# TC-13 / TC-14 公开表面与热路径日志收口

类型：`implementation-verification`

日期：2026-10-08

状态：`complete`

## 1. 范围

本批次关闭临时概念审计中的：

- TC-13：crate root / prelude 专业 API 平铺与缺少可执行快照；
- TC-14：递归绘制、布局和 Vello lowering / submit 热路径日志。

不改变领域模块的专业扩展能力，也不引入运行时 trace sink。

## 2. 公开 API 边界

crate root 只保留 Runtime、Figure tree、基础几何、常用 Figure / Layout 和规范
Graphics 入口。prelude 面向常规 Figure / Runtime 开发。以下协议改从命名模块使用：

- `connection`：anchor、router、locator 和 connection runtime；
- `container`：range、viewport、scroll、zoom 和 layer；
- `event`：输入、listener、focus、tooltip 与 accessibility；
- `figure`：capability、具体 Figure 状态和 border；
- `render`：backend、submission、resource、text 和 `NdCanvas`；
- `runtime`：scoped mutation、resource 与 frame service。

`verification/public-api/novadraw-symbols.txt` 保存 root、prelude、全部一级领域模块和
`advanced` 的 rustdoc 符号集合。`scripts/check_public_api_surface.sh` 对新增和删除都
输出 diff，只有显式评审后执行 `--update` 才能更新基线。

Core 内部暂时保留 `pub(crate)` 旧 root 别名，作为 crate-local prelude；它们不进入
rustdoc 或外部 API。workspace 生产消费者与 P2-F02 定向 contract consumers 已迁移到
命名模块，不保留公开转发壳。

## 3. 热路径

已删除：

- `render_recursive.rs` 的逐 Figure、client area 和 child 日志；
- Fill / Flow layout 的逐次布局日志；
- Vello 的逐命令状态日志、damage 日志和每帧完成日志；
- Core / Vello backend 中仅为这些日志存在的 `tracing` 依赖。

显式、feature-gated 的离线 scene/tree dump 保留在非受保护路径。新增
`scripts/check_hot_path_logging.sh`，禁止受保护 render、layout 和 backend 源码重新
引入 tracing 或 print 宏。

## 4. 验证

- `cargo check --workspace`：通过；
- `scripts/check_public_api_surface.sh`：通过；
- `scripts/check_hot_path_logging.sh`：通过；
- `cargo xtask verify core.p2-f02-figure-capability`：126 项通过；
- `cargo xtask verify core.api-surface-hot-path`：通过；
- `cargo xtask docs`：通过；
- `cargo xtask check --quick`：通过；
- `git diff --check`：通过。

本批次不重复运行 full gate；P2-F02 里程碑边界已执行一次，已知
`cargo test --workspace` 仍受并行 Builder `Result` 迁移中的旧 `cfg(test)` 调用阻断。
