# xtask 命令体系价值评估

类型：`architecture-review`

评估日期：2026-10-16

## 结论

当前 xtask 不是空壳。它已经形成三类有实际价值的能力：

- 统一执行 102 个底层 command，并在任一子进程失败时立即返回非零状态；
- 用 58 个 suite 固化功能切片与验证命令的映射；
- 提供一组 Cargo 自身无法替代的项目门禁，例如公开 API 快照、第三方类型泄漏、
  Core 依赖边界、热路径日志、资源 I/O、Kurbo 单版本和 Editor 开放协议检查。

它目前还不能称为完整的 CI/CD 门禁系统。Git 仓库顶层没有 GitHub Actions、
Buildkite、CircleCI、Jenkins 或 GitLab CI 配置，所有触发时机都只是开发规范和历史
评审中的人工约定。xtask 也不消费 `paths`、`platforms`、`layers`、`artifacts` 等
大部分 suite 元数据，不会按变更自动选 suite，不会按当前平台过滤命令，也不会在命令
结束后确认声明的产物确实生成。

综合判断如下：

| 分类 | 结论 |
|---|---|
| 项目特有、不可由普通 Cargo 命令替代 | 文档结构校验、架构边界脚本、公开 API 快照、开放协议消费者、应用 replay、真实像素和平台证据采集 |
| 有效但主要是统一入口 | fmt、check、Clippy、test、crate/target 编译和大多数定向测试 |
| 有数据价值但不是正确性门禁 | binary size 和 GA-2 性能报告；没有自动阈值或基线回归判定 |
| 仅负责启动人工流程 | `manual`、`web.serve`、Native 示例和四个平台运行入口 |
| 形式化或名实不符 | 未消费的 suite 元数据、`workspace.quality` 的“提交门禁”命名、两个完全相同的 Linux 构建入口、所有名为 release 但实际只执行 dev `cargo check` 的平台构建入口 |

建议保留 xtask，但把它从“命令目录”升级为可验证的门禁调度器。优先级最高的工作是接入
真实 CI、校验 suite 产物、修正平台 qualification 语义，并把 `docs` 纳入最终门禁。

## 评估范围和方法

本次检查覆盖：

- [`tools/xtask/src/main.rs`](../../../tools/xtask/src/main.rs) 的全部执行与校验逻辑；
- 当时的 `verification/suites.toml` 中 102 个 command、
  2 个 profile 和 58 个 suite；
- xtask 调用的 shell、Node、Rust benchmark、示例验证程序；
- `AGENTS.md`、`CLAUDE.md`、Book、verification 文档和历史证据中的触发约定；
- Git 仓库顶层及当前子项目中的 CI 配置。

价值判断采用四级口径：

| 等级 | 含义 |
|---|---|
| A | 项目特有且能阻断真实缺陷，普通 Cargo 命令不能覆盖 |
| B | 有实质检查或构建效果，但 xtask 主要提供稳定名称和编排 |
| C | 只采集数据、生成报告或启动人工流程，不直接证明项目正确 |
| D | 当前无法证明其名称所声称的能力，或与另一入口完全重复 |

本次没有逐个运行 102 个 command。平台窗口、Windows、Linux 和完整性能采样受宿主环境、
耗时或人工交互限制，采用实现审查和仓库既有执行证据。代表性动态执行结果见文末。

## 顶层命令

每次调用都会先读取并完整执行 `manifest.validate()`，然后才进入子命令分支。因此，
`list`、`run` 和 `manual` 也会被任意文档元数据错误阻断。

| 顶层命令 | 实际流程 | 输出与失败条件 | 建议触发时机 | 评价 |
|---|---|---|---|---|
| `cargo xtask list` | 校验 manifest，列出 2 个 profile 和 58 个 suite | 只输出 profile/suite；不列 102 个 command | 选择验证切片前 | B：目录有效，但 `run <command-id>` 的可发现性不完整 |
| `cargo xtask docs` | 校验 manifest、文档类型、本地链接、xtask 引用、部分索引分类和 parity 状态，再运行 `test.facade-doc` | 输出数量和 PASS；任一结构错误或 doctest 失败即非零 | 纯文档变更 | A/B：检查真实，但“public API probes”表述过宽 |
| `cargo xtask check --quick` | 顺序运行 11 个格式、编译和架构边界 command | 首个失败即停止；不生成汇总报告 | 同类整改批次完成 | A：覆盖面实在；`api.third-party-types` 较重，“quick”并不总快 |
| `cargo xtask check --full` | 顺序运行 12 个边界、Clippy 和 workspace test command | 首个失败即停止；不含 `docs` | 最终提交、推送、合并或里程碑关闭 | A-：当前最强本地门禁，但不是完整门禁 |
| `cargo xtask verify <suite-id>` | 解析完整 ID 或唯一前缀，顺序运行 suite 的 command | 输出 suite、每条命令和子进程输出 | 功能切片完成 | A：稳定 suite ID 有价值；元数据不参与执行 |
| `cargo xtask verify --all` | 按 suite 顺序收集 command，按 command ID 去重后执行 | 不按平台过滤，遇到宿主不支持的命令即失败 | 当前没有可靠的通用触发场景 | C：名称像全量门禁，实际上是跨平台命令串行合集 |
| `cargo xtask manual <suite-id>` | 打印手册路径并启动 manual command | 应用正常退出即成功；不记录人工检查结论 | 人工验收 | C：只是启动器 |
| `cargo xtask run workspace.test` 等 `run` 入口 | 直接运行一个注册 command | 完整透传 stdout/stderr，以退出码判定 | 调试单项门禁 | B：有效的底层逃生口 |

### `docs` 实际检查边界

`docs` 的有效检查包括：

- manifest schema、ID、引用关系和未使用 command；
- suite 的命令引用、文档存在性和 API family 引用；
- `doc/` 下类型标记；
- `doc/` 与 `book/` 下本地 Markdown 目标文件是否存在；
- 文档里的 xtask 子命令、profile、suite 前缀和 command ID 是否存在；
- 两个设计索引段落中已链接文档的类型；
- parity 表中被识别行的状态词；
- Core facade 的正向和 `compile_fail` doctest。

以下内容没有检查：

- Markdown 锚点、图片内容、外部 URL 和 mdBook 是否能构建；
- 设计索引是否收录了目标目录中的全部文档；
- parity family 是否重复，或格式不符合“5 列且以 `| \`` 开头”的行；
- `api.surface`、`api.third-party-types`、Editor doctest；
- suite 的 `paths` 是否存在或匹配文件；
- suite 的 `artifacts` 是否在执行后生成。

本次动态检查中，`cargo xtask docs` 通过，随后 `cargo xtask run api.surface` 因当前工作树
新增 `CoordinateFailure` 和 `CoordinateQueryError`、公开 API 快照尚未同步而失败。
这直接证明 `docs` 的 PASS 不能代表公开 API 快照通过。

## Profile 价值

### `check --quick`

实际包含：

1. workspace 格式和默认 feature 编译；
2. Core `--no-default-features` 编译；
3. Core 当前启用依赖图检查；
4. Host/Wasm 公开签名中的第三方类型扫描；
5. 公开 API 符号快照和 Runtime 方法预算；
6. 热路径日志、Core 资源 I/O、Kurbo 单版本检查；
7. Native 与 Wasm Vello feature 编译。

这些检查大多有直接阻断能力。问题在于 `api.third-party-types` 会清理独立 rustdoc target
并为 Host/Wasm 各构建一次文档，成本明显高于普通 quick gate。建议拆成
`check --quick` 与 `check --architecture`，或给 rustdoc 结果加可靠缓存。

### `check --full`

`full` 用 Clippy 代替普通 workspace check，再运行 workspace test，避免完全重复编译，
这个取舍合理。仍有四个缺口：

- 不运行 `cargo xtask docs`，文档引用和 manifest 错误可以通过 full；
- `cargo clippy --workspace` 没有 `--all-targets`，测试目标不会获得同等 lint 保证；
- Cargo 命令没有 `--locked`，不具备 CI 级依赖锁定；
- 没有覆盖全部 feature/target 组合，仍依赖具体 suite 补齐。

建议新增单一的 final gate，内部依次执行 docs、full 和按变更边界选择的平台矩阵，
避免调用者自行记忆组合。

## 102 个底层 command

### 工作区和架构边界

| Command | 实际作用 | 有效程度 | 改进方向 |
|---|---|---|---|
| `workspace.fmt` | `cargo fmt --all -- --check` | B，高 | 保留 |
| `workspace.check` | 默认 feature 的 workspace `cargo check` | B，高 | 说明不覆盖全部 feature/target |
| `workspace.clippy` | workspace 默认目标 `-D warnings` | B，高 | 最终门禁增加分层的 `--all-targets`/feature lint |
| `workspace.test` | workspace unit、integration、doctest | B，高 | 增加 `--locked`；不要宣称覆盖未启用 feature |
| `facade.core` | Core 无默认 feature 编译 | B，高 | 保留 |
| `test.facade-doc` | Core API 正向与 compile-fail doctest | A，高 | 保留；`docs` 输出应准确称为 facade doctest |
| `test.editor-doc` | Editor API doctest 和负向逃逸检查 | A，高 | 纳入公开 API 或 final gate |
| `backend.native-vello` | Native feature 编译 | B，高 | 保留 |
| `backend.web-vello` | wasm32 Web feature 编译 | B，高 | 保留 |
| `facade.dependencies` | 扫描 Core 当前启用的 normal dependency tree | A，中高 | 增加 `--all-features` 或直接审计 Cargo metadata；当前可漏掉未启用 optional 依赖 |
| `api.third-party-types` | 从 Host/Wasm rustdoc HTML 扫描 Kurbo、Vello、Winit 公开签名 | A，高 | 保留；减少无条件 `cargo clean --doc`，为 rustdoc HTML 格式变化加测试 |
| `api.surface` | 比较公开符号快照，限制 crate root/prelude 和 Runtime 方法数 | A，高 | 保留；把 25 的预算来源写入 ADR 或配置 |
| `quality.hot-path-logging` | grep 受保护路径中的 tracing/print 宏 | A，中 | 作为启发式门禁保留；长期改为 lint，避免别名绕过 |
| `quality.core-resource-io` | grep Core render/runtime 中已知文件 I/O API | A，中 | 扩展为依赖/能力边界检查，当前模式容易漏别名和其他 I/O API |
| `dependencies.kurbo` | 全 workspace、全 feature、全 target 检查 Kurbo 单版本 | A，高 | 保留 |
| `check.p2-s04-editor-open-protocol` | 拒绝闭合协议残留并编译独立外部消费者 | A，高 | 保留；文本 grep 可逐步迁到 compile-fail/consumer test |
| `check.tc12-native-host`、`check.tc07-native-host` | 完全相同的 node-editor host `cargo check` | B，中 | 合并为一个 command ID，suite 可共同引用 |
| `check.tc12-web-host` | wasm32 编译 Web host | B，高 | 保留 |
| `check.tc07-web-host` | wasm32 编译 Web ViewerDrive host | B，高 | 保留 |
| `check.tc11-web-provider` | wasm32 编译 Web resource feature | B，高 | 保留并纳入 feature matrix |
| `build.platform-web-p2-e02` | wasm32 编译 Web editor feature | B，高 | 与相邻 Web host 编译检查评估合并 |
| `web.build` | release 构建 Wasm，执行 wasm-bindgen，重建 dist | B，高 | 增加 dist 内容清单或浏览器 smoke test |

### 定向契约测试

下列 command 都执行真实 Rust 测试，断言失败会返回非零。它们不是形式化占位符。

| Command | 覆盖范围 | 有效程度 |
|---|---|---|
| `test.p2-s07-layout-query`、`test.p2-s07-layout-query-internal` | Query 身份、失败分类、测量准入和缓存隔离 | A，高 |
| `test.ga2-webgpu-browser-runner` | CDP 超时、可见性、硬件 GPU、dirty build 和证据越界声明 | A，高 |
| `test.ga2-work-counters` | 文字和路由确定性工作量 | A，高 |
| `test.p2-g01-graphics`、`test.tc01-graphics`、`test.p2-g02-text-graphics` | Graphics、发布边界、文字测量和 glyph 轮廓 | A，高 |
| `test.p2-m01-animation`、`test.tc04-property` | Animation 和类型化属性日志 | A，高 |
| `test.tc12-render-failure` | 后端失败、恢复确认和 session 失效 | A，高 |
| `test.vello-render-failure`、`test.vello-p2-graphics` | 两个 ID 都执行相同的 Vello 全量 lib test | B，高但重复；合并为 `test.vello-lib` |
| `test.tc11-resource-provider`、`test.tc11-native-provider` | Core token 和 Native 文件 provider | A，高 |
| `test.render-image-source`、`test.vello-image-source` | image source rectangle 与 Vello lowering | A，高 |
| `test.scene-dispatch`、`test.scene-viewport`、`test.scene-connection` | 公共 dispatch、viewport 和 connection 契约 | A，高 |
| `test.p2-c01`、`test.p2-c02`、`test.p2-f01` | 连接装饰、最短路由、可缩放多边形 | A，高 |
| `test.p2-t01`、`test.p2-t02` | TextFlow 和交互几何 | A，高 |
| `test.editor-p2-e02` | 直接文字编辑会话 | A，高 |
| `test.platform-winit-p2-e02`、`test.platform-web-p2-e02` | Native/Web 文字输入桥 | B，高 |
| `test.node-editor-p2-e02` | Native demo 中的直接编辑测试过滤器 | B，高 |
| `test.editor-g1-model`、`test.editor-g1-command`、`test.editor-g2` | Model、CommandStack、Viewer 投影 | A，高 |
| `test.editor-s06-failure`、`test.core-s06-admission` | S06 失败隔离和批量准入 | A，高 |
| `test.editor-g3-selection`、`test.editor-g3-interaction` | Selection、Targeting、Viewer interaction | A，高 |
| `test.editor-g4`、`test.editor-g5-projection`、`test.editor-g5-editing` | Tool/Policy 编辑闭环和连接编辑 | A，高 |
| `test.ga4-core-extension-consumers`、`test.ga4-editor-extension-consumer` | 外部 Core/Editor 消费者 | A，高 |
| `test.p2-child-insertion`、`test.p2-figure-capability` | 原子插入和开放 Figure capability | A，高 |
| `test.p2-s03-runtime-facades`、`test.context-phase-authority` | Runtime facade 和阶段权限 | A，高 |
| `test.p2-s05-deep-layout-presentation` | 深层布局与同帧文字呈现 | A，高 |

定向 suite 允许普通编译 warning 通过。本次 `core.p2-s07-layout-query` 的 10 项测试全部
通过，同时输出了 15 个 warning。这是预期的职责分层：定向 suite 证明行为，不代替
`workspace.clippy -D warnings`。

### 应用验证、replay 和像素检查

| Command | 实际作用 | 有效程度 | 改进方向 |
|---|---|---|---|
| `verify.update`、`verify.event`、`verify.scroll-pane` | 无窗口运行真实应用路径，断言并写 JSON | A，高 | 执行后由 xtask 校验报告 schema 和 fresh timestamp |
| `verify.editor-s06-native-exit` | 真实 macOS GPU drain 和 Runtime replacement | A，高、平台限定 | 在 macOS CI runner 执行 |
| `verify.animation-example`、`verify.uml-example` | 复杂示例行为验证并写报告 | A，高 | 保留 |
| `replay.editor-g3`、`replay.editor-g4`、`replay.editor-g5.2`、`replay.editor-g5.3`、`replay.editor-g5.4`、`replay.editor-g5.5` | 复用真实 EditorHarness 做有序 headless replay | A，高 | 保留；报告加 revision 和 dirty 状态 |
| `native.p2-g01-pixels` | 真实 Vello offscreen 像素断言 | A，高、平台限定 | 保留 |
| `native.p2-g01-present` | retained repair 和 surface submission | A，高、平台限定 | 保留 |
| `web.p2-g01-pixels-1`、`web.p2-g01-pixels-2` | 读取既有 Web PNG，再用共享像素断言校验 | C，中 | suite 内先生成新 Web 图片并删除旧文件；当前可消费陈旧产物 |

### 报告和性能 command

| Command | 实际作用 | 有效程度 | 改进方向 |
|---|---|---|---|
| `report.binary-size` | release 构建、记录 bytes 和 SHA-256 | C，中 | 加 reviewed baseline、允许回退比例和超限失败；否则只叫 report |
| `report.ga2-performance` | 14 个隔离 CPU 场景，记录样本、p50/p95、RSS 和稳定工作量 | C+，数据有效 | 增加同机基线比较；现在只保证 harness 成功 |
| `report.ga2-draw2d-performance` | 同机 Draw2D/SWT 参考采样 | C+，数据有效 | 固化依赖指纹并自动生成可比性判定 |
| `report.ga2-native-vello-performance` | Native Vello CPU/GPU completion；surface 不可用时可降级 offscreen | C+，数据有效 | suite 标题不要暗示必然 surface-present；增加阈值层 |
| `report.ga2-native-presentation-performance` | 强制可见 macOS surface 和 GPU completion | C+，证据有效 | 加 baseline/预算判定 |
| `report.ga2-native-window-server-performance` | 合成输入到 WindowServer 可见像素 | A/C：证据资格强，性能阈值无 | 保留证据校验，另加回归阈值 |
| `report.ga2-webgpu-browser-performance` | 强制 Chrome 版本、可见前台、Apple Metal、clean build 和 30 个样本 | A/C：证据资格强，性能阈值无 | Chrome major 不应长期硬编码为单一版本；阈值单独配置 |

这些 command 会验证采样环境和数据完整性，但没有把当前结果与基线自动比较。因此，
“命令成功”只表示报告可信地产生，不表示性能没有回退。

### 平台构建和人工启动

| Command | 实际作用 | 有效程度 | 改进方向 |
|---|---|---|---|
| `build.native-macos-release` | 实际是 dev profile 的 `cargo check -p node-editor-demo` | D，名称失真 | 改名 `check.native-macos`，或真正执行 `cargo build --release --locked` |
| `build.native-windows-release` | cargo-xwin 的 dev profile `check` | B/D：跨编译有效，release 声明无效 | 拆为 `check` 与真正 release build |
| `build.native-linux-x11-release` | x86_64 Linux dev `cargo check`，同时启用 X11/Wayland | D，不能单独证明 X11 | 用独立 feature 配置或容器矩阵 |
| `build.native-linux-wayland-release` | 与 X11 command 参数完全相同 | D，重复且不能单独证明 Wayland | 删除重复入口，或建立真正 Wayland-only build |
| `web.serve` | 启动本地 HTTP server | C，便利入口 | 不作为 PASS 证据 |
| `native.node-editor`、`native.advanced-figures` | 打开人工验收应用 | C，便利入口 | 写结构化人工结论或只称 launcher |
| `run.native-macos-release`、`run.native-windows-release`、`run.native-linux-x11-release`、`run.native-linux-wayland-release` | 校验宿主环境后 `cargo run` dev 应用 | C/D：能打开窗口，不证明 release 或验收通过 | 运行已构建 release artifact，并要求手册结果记录 |

## 58 个 suite

### 高价值自动 suite

以下 suite 都运行真实测试、外部消费者、应用断言、replay、编译边界或像素检查，失败会
阻断命令：

`core.p2-s07-layout-query`、`core.facade`、`core.image-source-rectangle`、
`core.p2-c01-connection-decoration`、`core.p2-c02-shortest-path-routing`、
`core.p2-f01-scalable-polygon`、`core.p2-t01-text-flow`、
`core.p2-t02-text-interaction`、`editor.p2-e02-direct-text-edit`、
`platform.p2-e02-text-input`、`ga4.extension-consumers`、
`core.tc01-graphics-cutover`、`core.api-surface-hot-path`、
`editor.tc07-viewer-boundary`、`core.tc04-typed-property-identity`、
`editor.p2-s06-failure-isolation`、`editor.p2-s06-native-exit`、
`core.tc12-render-failure`、`core.tc11-resource-provider`、
`core.p2-f02-figure-capability`、`core.p2-s01-child-insertion`、
`core.p2-s03-runtime-facades`、`editor.p2-s04-open-protocol`、
`core.p2-s05-deep-layout-presentation`、`example.uml-extension`、
`core.context-phase-authority`、`example.animation-orthogonality`、
`core.p2-g01-graphics`、`core.p2-g02-text-graphics`、
`core.p2-m01-animation`、`graphics.p2-g01-visual`、
`graphics.p2-g01-present`、`core.runtime`、`m8.viewport-scroll-zoom`、
`g1.model-command`、`g2.viewer-projection`、`g3.selection-targeting`、
`g4.editing-loop`、`g5.1.connection-projection`、
`g5.2.connection-creation`、`g5.3.connection-reconnect`、
`g5.4.connection-bendpoint`、`g5.5.viewport-autoexpose`。

评价：A 或 B。suite ID 提供稳定的工作流契约，但其 `paths`、`layers`、`platforms` 和
`artifacts` 只是说明性字段。

### 观测型 suite

| Suite | 实际价值 | 评价 |
|---|---|---|
| `core.binary-size` | 构建并记录体积，无阈值 | C |
| `core.performance` | 工作量测试可阻断，性能数据只报告 | B/C |
| `reference.draw2d-performance` | 参考基线采样 | C |
| `backend.native-performance` | GPU completion 数据和工作稳定性 | C+ |
| `backend.native-presentation-performance` | 可见 surface 证据 | C+ |
| `backend.native-window-server-performance` | WindowServer 可见证据 | A/C |
| `backend.webgpu-browser-performance` | WebGPU 证据资格严格，性能无阈值 | A/C |

### 存在明显缺口的 suite

| Suite | 无效点 | 建议 |
|---|---|---|
| `workspace.quality` | 只跑 fmt、Clippy、test，却叫“Workspace submission gate”；缺少 full 中 9 个架构/target 门禁；声明的 `target/verification/runs` 也没有生产者 | 删除此重复 suite，或让它直接等价于 full，并移除虚假 artifact |
| `graphics.p2-g01-web-pixels` | 只验证已存在 PNG，不负责生成，当前两个声明文件也不存在 | 合并浏览器截图生产与像素断言，执行前清理旧产物 |
| `web.build` | 只证明 Wasm/dist 构建，manual 只启动 server | 增加浏览器 smoke test |
| `platform.native-macos-release` | dev check + dev run | 改名或真正构建/运行 release |
| `platform.native-windows-release` | dev cross-check，manual 依赖另一个原生 Windows 宿主 | 拆分 compile qualification 与 native runner qualification |
| `platform.native-linux-x11-release` | 与 Wayland build 完全相同 | 建立 X11-only feature/runner |
| `platform.native-linux-wayland-release` | 与 X11 build 完全相同 | 建立 Wayland-only feature/runner |
| `platform.web-chrome-release` | 自动部分只做 build，manual 只 serve | 复用 WebGPU browser runner做 Chrome smoke/qualification |

## 元数据的实际效力

| 字段 | 当前是否影响执行 | 实际效果 |
|---|---|---|
| `commands` | 是 | 决定顺序执行的子进程，是 suite 的核心 |
| `documents` | 是 | 启动前要求文件存在 |
| `api_semantics` | 部分 | 只要求 contract/application 非空且 family 存在 |
| `kind` | 很弱 | 只决定是否强制 `api_semantics`，其余用于展示 |
| `milestone` | 很弱 | 只检查非空并打印 |
| `layers`、`platforms` | 否 | 只检查非空；不调度、不筛选 |
| `paths` | 否 | 只检查不是绝对路径或 `..`；通配符可以不匹配任何文件 |
| `artifacts` | 否 | 只检查相对路径格式；执行前后都不检查存在性、新鲜度或内容 |
| `evidence_root` | 否 | 只检查路径格式 |
| `manual.document` | 部分 | 要求存在并打印 |
| `manual.command` | 是 | 启动程序，但不记录人工结论 |

`artifacts` 允许“预先声明尚不存在的证据”本身合理，但 suite 成功后仍不检查产物，使该字段
无法承担验收合同。建议为 artifact 增加 `required_after_run`、类型、schema、最小大小和
新鲜度规则。

## CI/CD 和开发工作流

### 文档定义的触发时机

| 阶段 | 当前约定 |
|---|---|
| 编辑内环 | 直接运行 crate `cargo check --lib`、单个 test 和 Clippy |
| 功能切片完成 | `cargo xtask verify <suite-id>` |
| 同类整改批次完成 | `cargo xtask check --quick` |
| 纯文档变更 | `cargo xtask docs` + `git diff --check` |
| 提交、推送、合并、里程碑关闭 | `cargo xtask check --full` |
| 平台和视觉验收 | `cargo xtask manual <suite-id>` 或平台专属 suite |

### 实际自动触发

没有。Git 顶层 `/Users/bytedance/Documents/code/GitHub/drawjs` 下未发现 CI 配置，
当前项目内也没有 CI 文件或受版本控制的 Git hook。历史评审日志只能证明某次有人运行过
命令，不能保证每次 push、merge 或 release 都会运行。

建议的最小 CI 分层：

1. 每个 PR：`docs`、`check --quick`、受影响 suite；
2. 合并主干：`check --full`，并使用 `--locked`；
3. macOS runner：Native surface、WindowServer、WebGPU Chrome；
4. Windows runner：原生 release build 和人工/自动 smoke；
5. Linux X11、Wayland runner：分开的 feature/build 和真实窗口 smoke；
6. nightly：性能采样、binary size、Draw2D 对照，按基线阈值报警。

## 优化清单

### P0：修正门禁真实性

- 建立真实 CI；否则“提交门禁”仍依赖自觉。
- 让 final gate 包含 `docs`，并统一使用 `--locked`。
- suite 成功后检查必需 artifact 的存在、新鲜度和 schema。
- 把 Linux X11/Wayland 入口改成真正不同的构建与运行环境。
- 将所有 `*-release` 构建和运行入口改为真正 release，或改名为 `check`/`dev-run`。

### P1：让 manifest 元数据参与决策

- 用 `paths` 做 changed-file 到 suite 的推荐或强制映射，并检查 glob 至少匹配一个文件。
- 用 `platforms` 在 `verify --all` 中生成可执行矩阵，而不是在单机串行运行全部平台。
- 为 `kind=quality` 区分 `gate` 与 `report`，避免“报告生成成功”被理解成质量通过。
- `list` 增加 `--commands`、`--json`、平台和预计成本，解决底层 command 不可发现问题。
- 检测不同 command ID 的相同 argv，阻止无意义别名和 `verify --all` 重复执行。

### P2：增强检查器稳定性

- 用 Cargo metadata、rustdoc JSON 或编译消费者替代易碎的 HTML/grep 检查。
- parity ledger 使用结构化格式，或至少严格拒绝列数错误、重复 family 和未识别行。
- Markdown 校验增加锚点和 mdBook 构建；索引规则增加“目标目录全部收录”模式。
- binary size 和性能命令增加可审阅基线、环境指纹、容忍区间和回退失败条件。
- manual 验收生成带 commit、平台、检查项和结论的结构化记录。

## 动态执行证据

本次实际运行：

| 命令 | 结果 | 证明范围 |
|---|---|---|
| `cargo test -p xtask` | 5 项通过 | manifest、suite 前缀、文档命令引用和基础解析单测 |
| `cargo xtask list` | 通过 | 当前为 2 profiles / 58 suites |
| `cargo xtask docs` | 通过；Core doctest 4 项、compile-fail 28 项通过 | 文档结构和 facade doctest |
| `cargo xtask verify core.p2-s07-layout-query` | 10 项通过 | suite 确实执行外部和内部失败语义测试 |
| `cargo xtask run replay.editor-g4` | 通过并写出 JSON | replay 确实经过 bounds edit 和 command history |
| `cargo xtask run quality.hot-path-logging` | 通过 | 静态脚本真实执行 |
| `cargo xtask run facade.dependencies` | 通过 | 依赖树脚本真实执行 |
| `cargo xtask run dependencies.kurbo` | 通过，版本为 0.13.1 | 全 feature/target 依赖图检查 |
| `cargo xtask run test.ga2-webgpu-browser-runner` | 9 项通过 | WebGPU 证据校验器有正负用例 |
| `cargo xtask run api.surface` | 失败 | 当前公开符号快照与工作树不一致，证明门禁可阻断 |

没有运行当前 `check --quick` 和 `check --full`，因为 `api.surface` 已确定会在到达后续步骤
前失败。仓库已有历史日志证明 quick/full 会执行各自命令；其中 full 也曾在
`workspace.clippy -D warnings` 阶段真实阻断代码。

## 最终价值分类

| 分类 | 保留项 | 处理建议 |
|---|---|---|
| A：项目核心门禁 | `docs` 的结构检查、公开 API/依赖边界脚本、开放协议消费者、关键契约 suite、replay、像素和平台证据资格检查 | 强制进入 CI，增强结构化校验 |
| B：有效统一入口 | Cargo fmt/check/Clippy/test、feature/target 编译、普通定向测试、`run` | 保留，补 `--locked` 和准确命名 |
| C：观测或人工 | binary size、性能报告、serve、示例启动、manual | 与 PASS 门禁分开显示，记录报告或人工结论 |
| D：形式化或失真 | 未消费元数据、`workspace.quality`、重复 Vello/host check 别名、重复 Linux build、伪 release 名称、依赖陈旧 PNG 的 Web pixel suite | 合并、改名、补真实执行语义或移除 |

xtask 已经具备实际作用，当前问题是有效门禁、观测报告、人工启动器和说明性元数据混在
同一个成功语义里。下一步应先统一“什么结果可以叫 PASS”，再扩展命令数量。
