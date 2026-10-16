# xtask 命令体系精简计划

类型：`verification-plan`

日期：2026-10-16

状态：`complete`

## 目标

把 xtask 收口为 Novadraw 特有的文档校验器和跨工具验收编排器。Cargo 已经提供的
fmt、check、Clippy、test 和 run 能力直接使用，不再增加同义别名。

精简后必须满足：

- 每个保留入口都执行 Cargo 无法单独表达的项目逻辑；
- 同一验收切片不重复编译或运行相同测试目标；
- 文档变更不触发 Rust doctest 编译；
- 单测试 target 不再包装成 suite；
- 人工应用、性能观测和平台构建使用直接命令；
- 历史评审保留当时的命令记录，不再约束当前命令 API。

## 保留的顶层入口

| 命令 | 用途 |
|---|---|
| `cargo xtask list` | 列出当前跨边界 suite |
| `cargo xtask docs` | 校验文档类型、本地链接、当前命令引用、设计索引和 parity 状态 |
| `cargo xtask verify <suite-id>` | 顺序执行一个跨边界验收切片 |

删除：

- `check --quick`；
- `check --full`；
- `verify --all`；
- `run <command-id>`；
- `manual <suite-id>`。

删除原因：

- quick/full 只是标准 Cargo 和脚本命令的别名；
- verify all 混合互斥平台环境，不能形成可靠的单机门禁；
- run 只转发一个已经可直接执行的命令；
- manual 只打开应用，不记录人工结论。

## 保留的 suite

| Suite | 必要性 |
|---|---|
| `core.query-contracts` | 组合 P2-S07 的内部和外部查询失败语义 |
| `core.facade` | 组合公开 API、依赖边界、Host/Wasm rustdoc 和 backend feature |
| `core.resources` | 组合 Core、Native、Web 和 Vello 资源合同 |
| `core.render-failure` | 组合 Core/Vello 失败恢复及 Native/Web host 编译 |
| `core.graphics` | 组合 Core Graphics、Text Graphics 和 Vello lowering |
| `extensions` | 组合 Core/Editor 外部消费者和开放协议探针 |
| `runtime.apps` | 组合 Update、Event、Viewport、UML 和 Animation 应用探针 |
| `editor.contracts` | 运行完整 Editor crate 契约测试和宿主编译 |
| `editor.replay` | 一次进程执行 G3、G4、G5.2-G5.5 全部 replay |
| `platform.text-input` | 组合 Winit/Web 文字输入和 wasm host 编译 |
| `graphics.native` | 组合 Native offscreen 像素和 surface present |
| `web.build` | 构建 release Wasm 与 wasm-bindgen 产物 |
| `web.performance` | 组合浏览器证据校验器和真实 Chrome/WebGPU 采样 |

其余 suite 分为三类处理：

- 单 Cargo test：改为直接 `cargo test`；
- 性能、体积和对照观测：改为 benchmark 文档中的直接命令；
- 平台人工验收：改为手册中的直接构建和运行命令。

## 配置与实现收口

删除 `verification/suites.toml` 及以下结构：

- 109 个全局 command 别名；
- quick/full profile；
- 只作说明、不参与执行的 paths、layers、platforms 和 artifacts；
- command 引用计数和 manifest schema 校验。

13 个 suite 以静态数据定义在 xtask 内。每个 suite 只保留：

- 稳定 ID；
- 一行说明；
- 实际执行步骤。

`tools/xtask` 删除 `serde` 和 `toml` 依赖。

## 架构脚本合并

把以下脚本合并为 `scripts/check_architecture.sh`：

- `check_facade_dependencies.sh`；
- `check_public_api_dependencies.sh`；
- `check_public_api_surface.sh`；
- `check_hot_path_logging.sh`；
- `check_core_resource_io.sh`；
- `check_kurbo_versions.sh`。

合并后的执行顺序：

1. 先运行源码扫描和依赖图检查；
2. Host rustdoc 只生成一次，同时检查第三方类型和公开符号快照；
3. Wasm rustdoc 只生成一次，只检查第三方类型；
4. 不执行 `cargo clean --doc`，仅删除三个被检查 crate 的旧 rustdoc 目录；
5. `--update-api` 显式更新公开符号快照。

`report_binary_size.sh` 没有回归阈值，只生成观测数据，删除该脚本。

## 直接执行的通用门禁

编辑内环：

```bash
cargo check -p <crate> --lib
cargo test -p <crate> --test <target>
```

同类整改批次：

```bash
cargo fmt --all -- --check
cargo check --workspace
```

最终提交边界：

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

公共 API、feature、target 或平台边界变化时，额外运行对应 suite。纯文档变化只运行：

```bash
cargo xtask docs
git diff --check
```

## 文档迁移

更新当前规范和使用指南：

- `AGENTS.md`；
- `CLAUDE.md`；
- `verification/README.md`；
- `doc/verification/00-index.md`；
- Contributor Book 验证章节；
- User Book 验证入口；
- 当前 roadmap 和 manual 中仍作为操作入口的命令。

历史 review、历史 performance 报告和完成记录保留当时命令，不做追溯改写。
xtask 只校验当前工作流文档中的命令引用，不再把历史记录当作当前命令契约。

## 实施顺序

1. 新增本计划并登记索引；
2. 合并架构检查脚本；
3. 重写 xtask 为三入口和静态 suite；
4. 删除 manifest、冗余脚本和依赖；
5. 迁移当前文档；
6. 运行 xtask 单测和文档校验；
7. 运行可编译的定向 suite；
8. 记录因当前并行 API 迁移导致的外部阻塞，不修改无关业务代码。

## 验收标准

- xtask 仅暴露三个顶层入口；
- suite 数量为 13；
- 不存在全局 command 注册表和 profile；
- `tools/xtask` 不依赖 serde/toml；
- 文档门禁不启动 rustdoc/doctest；
- replay 使用一个 `all` 进程；
- 架构检查不重复生成 Host rustdoc；
- 删除的脚本和 manifest 没有当前规范链接；
- `cargo test -p xtask` 通过；
- `cargo xtask docs` 通过；
- `git diff --check` 通过。

## 实施结果

2026-10-16 完成：

- 顶层入口从 7 类收口为 `list`、`docs`、`verify`；
- 60 个 suite 收口为 13 个跨边界 suite；
- 删除 109 个 command alias、2 个 profile 和 `verification/suites.toml`；
- xtask 删除 serde/toml，只保留 anyhow；
- 六个架构脚本合并为 `scripts/check_architecture.sh`；
- Host rustdoc 由两次降为一次，取消 `cargo clean --doc`；
- 六个 Editor replay 由六次进程启动合并为一次 `all`；
- 三个 P2-S07 suite 的十个 command 收口为一个 suite 的两个 Cargo 进程；
- 删除无阈值的 binary-size 报告脚本；
- 当前规范、贡献指南、用户指南、roadmap 和 manual 已迁移。

已执行：

- `cargo test -p xtask`：5 项通过；
- `cargo xtask docs`：325 份 Markdown 通过；
- `scripts/check_architecture.sh`：通过；
- `core.query-contracts`：外部 39 项、内部 13 项通过；
- `editor.replay`：6 组通过；
- Core doctest/compile-fail：32 项通过；
- Native/Web Vello feature check：通过。

本机热缓存资源观测：

- `cargo xtask docs`：1.45 秒，最大 RSS 约 10 MB；
- 旧 xtask、manifest 和七个脚本约 3,797 行，收口后 xtask 与统一架构脚本约
  1,140 行，减少约 70%；
- 文档门禁不再启动 Core doctest，Rust 编译成本归入明确的 test 或 suite。
