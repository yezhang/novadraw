# Verification

Novadraw 直接使用 Cargo 完成常规编译和测试。xtask 只保留项目特有的文档校验与跨工具
验收编排。

## xtask

```bash
cargo xtask list
cargo xtask docs
cargo xtask verify <suite-id>
```

当前 suite：

| Suite | 覆盖范围 |
|---|---|
| `shape.independent-corners` | 逐角圆角、逐边颜色、旧 Shape/Border、共享边像素归属、独立样例与 WASM Core 编译；需本机 GPU |
| `text.long-layout` | 精确段落复用、共享 glyph 范围与旧 lease、来源几何、轮廓与段落索引、Full/Partial GPU 像素及原 Large release CPU smoke；需本机 GPU |
| `animation.tree` | 表现几何与容量预留、Tree 操作、Native GPU 中间帧及 retained-partial 像素；需要本机图形会话 |
| `example.zoom-input` | 浮动面板输入归属、Zoom 滚动条按下/拖动/释放及缩放联动；无需窗口 |
| `render.antialias` | 逐操作 AA 契约、Native Msaa16 像素、40 个 ShapeStyles 矩阵及 Web 编译；需本机 GPU |
| `text.layout-direction` | 继承方向、styled/structured flow、外部文字引擎、Bidi 共享滚动与 resize；含本机 GPU Partial/Full 验证 |
| `render.low-miter` | 低 miter、Cocoa 样例零宽映射与 AA ON/OFF 开闭反向路径像素；需本机 GPU |
| `text.underlines` | 实际字体下划线、外部引擎、Path 消费者与 resize/替换的局部像素；需本机 GPU |
| `render.antialias-cost` | 25 单元 release 准备/编码、GPU 等待、输出及缓存成本；需本机 GPU |
| `core.query-contracts` | Query identity、数值失败与恢复 |
| `core.facade` | 公开 API、依赖、热路径和 backend feature |
| `core.resources` | Core token、Native/Web provider 与 Vello image lowering |
| `core.render-failure` | 后端失败恢复与 Native/Web host |
| `core.graphics` | Core Graphics、Text Graphics 与 Vello lowering |
| `extensions` | Core/Editor 外部消费者和开放协议 |
| `runtime.apps` | Update、Event、Viewport、UML、Animation 应用探针 |
| `editor.contracts` | Editor 完整测试与 Native host 编译 |
| `editor.replay` | G3、G4、G5.2-G5.5 单进程回放 |
| `platform.text-input` | Winit/Web 文字输入与 wasm host |
| `graphics.native` | Native 像素与 surface present |
| `graphics.host-background` | Host 背景受检投递、帧恢复与 Vello 合成像素 |
| `canvas.native` | 独立 Canvas 合成、Novadraw 离屏纹理与双 Canvas Native 自检 |
| `web.build` | release Wasm 与 wasm-bindgen 产物 |
| `web.performance` | Chrome/WebGPU 证据校验与采样 |

单个 Cargo test、应用启动、性能观测和平台构建不再包装成 suite。

ADR-061 控制器验证入口：
`cargo test -p novadraw-editor --test p2_e02_direct_text_edit_contract`、
`cargo test --manifest-path tests/consumers/editor-open-protocol/Cargo.toml --lib edit_session`、
`cargo test -p novadraw-richtext --test edit_controller` 和
`cargo test -p richtext-figure-edit-app --bin richtext-figure-edit-app`。
外部数字领域控制器不依赖 TextFlow 或 plain 草稿；样例同时验证独立与嵌入宿主。
范围与未完成平台项见[验证矩阵](../doc/verification/plans/specialized-edit-controller.md)。

ADR-059 A 使用 [text-stages 独立消费者](consumers/text-stages/README.md)
验证 typed 分析资产、lease、预算和阶段绑定。
私有事务入口为 `cargo test -p novadraw --lib text::staged`；
公开入口为 `cargo test -p novadraw --test p2_text_stages`。
该片不选择默认 Parley 的阶段能力，Flow 全输出等价与性能仍归后续 B／C／D。

FR-02 的[共享内容消费者](consumers/flow-assembly-reuse/README.md)验证 owned／shared
request、内容 lease、缩容及非法输入；新增 typed 输入登记和 Runtime 内实际读取用例。
Core 入口为
`cargo test -p novadraw --test flow_assembly_reuse_contract`；
统一输入证明传递与不可变 Flow 消费正在验证；
完整 Flow 声明与整页复用尚未接入，基础测试不关闭整批 FR-02。

Parley 上游接口入口为
`cargo test -p novadraw --test p2_parley_public_api_contract`，
覆盖复制后宽度重排与新建布局等价，以及构建上下文／字体 collection
复用后的布局独立性。Core 通用阶段预算继续由 `p2_text_stages` 验证。
旧 Parley／Harfrust／Fontique 补丁消费者和重放工具已退役；
替代边界与历史证据见[迁移记录](../doc/verification/reviews/parley-upstream-migration-2026-11-05.md)。

ADR-057 首片使用 Animation 定向回归与独立消费者：

```bash
cargo test -p novadraw --lib s09_
cargo test -p novadraw --test p2_s09_animation_failure --test p2_animation_contract --test p2_animation_behavior_contract --test p2_animation_geometry_contract --test p2_animation_transition_contract --test p2_animation_sample_reservation_contract
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline
cargo run --manifest-path tests/consumers/service-failure/Cargo.toml --release --bin animation-failure-cost --locked --offline
```

消费者的可达体积变体和采样范围见 [service-failure](consumers/service-failure/README.md)。
结果按 [S09-A 实施记录](../doc/verification/reviews/p2-s09-a-implementation-2026-10-30.md)
统计。S09-B Inspector／Observations 已完成，见
[实施记录](../doc/verification/reviews/p2-s09-b-implementation-2026-10-30.md)。

S09-C Connections 注册使用以下定向入口，最终仍执行 workspace 门禁：

```bash
cargo test -p novadraw --lib s09_faulted_connection_registration
cargo test -p novadraw --test p2_s09_connection_registration
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections
cargo check --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections --target wasm32-unknown-unknown
```

测试范围及平台限制见 [S09-C 实施记录](../doc/verification/reviews/p2-s09-c-implementation-2026-11-02.md)。

S09-D Editor 扩展诊断使用公开夹具和独立消费者：

```bash
cargo test -p novadraw-editor --test p2_s09_extension_failure
cargo test -p novadraw-editor --doc
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --no-default-features --features consume-editor
```

实现、成本探针与待补验门禁见
[S09-D 实施记录](../doc/verification/reviews/p2-s09-d-implementation-2026-11-02.md)。

ADR-055 容器协议使用公开 API 契约夹具及独立 workspace 消费者：

```bash
cargo test -p novadraw-editor --test p2_e03_container_editing_contract
cargo test --manifest-path tests/consumers/editor-open-protocol/Cargo.toml --locked --offline
cargo test -p novadraw-editor --release --test p2_e03_container_editing_contract container_cost_samples_include_sync_hit_geometry_policy_feedback_and_commit -- --ignored --exact --nocapture
cargo build --manifest-path tests/consumers/editor-open-protocol/Cargo.toml --release --bin editor-container-delivery-cost --locked --offline
cargo build --manifest-path tests/consumers/editor-open-protocol/Cargo.toml --release --bin editor-container-delivery-cost --features container-cost --locked --offline
```

产物比较需分别指定独立 `--target-dir`，保持编译器、target 与 release 配置相同。
`container-cost` 只控制测量消费者是否调用容器能力，不是 Editor feature。
覆盖边界和结果见[容器编辑实施记录](../doc/verification/reviews/adr-055-implementation-2026-10-29.md)。

长文本 CPU 观测使用官方 Perfetto SDK，smoke 不代替统计验收：

```bash
cargo test -p novadraw --features perfetto --test long_text_layout_contract --test p2_t03_external_flow_engine --test p2_t03_structured_flow_contract
cargo run -p textflow-large-app --release -- --headless --replay --trace --requests
```

trace 由 Perfetto UI／Trace Processor 分析，先检查采集丢失，再比较相同场景、
工具链、构建、预算和采集配置。方法见[Perfetto 接入](../doc/design/architecture/perfetto-observation.md)
和[研究应用](../crates/novadraw/examples/native/textflow-large-app/README.md)。
默认关闭采集；完整请求可能明显增加成本。SDK 的真实 trace 验证覆盖外部引擎、
失败返回、Runtime 身份和开启／关闭采集的布局等价。
旧 Python 采样器与 `textflow-perf` 已删除，冻结报告及原始数据仍按
[长文本实施记录](../doc/verification/reviews/long-text-performance-implementation-2026-10-27.md)追溯。

## 常规门禁

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

纯文档变化：

```bash
cargo xtask docs
git diff --check
```

## 公开 API 快照

[Core 符号快照](public-api/novadraw-symbols.txt)列出受门禁约束的公开路径。
方法的完整签名与调用前提以对应领域的公开 rustdoc 为准；设计中的角色示意
不作为可独立编译示例。可执行消费者与编译门禁从上面的 xtask 表进入。

## 边界

- `cargo xtask docs` 不编译 Rust，只检查 Markdown 类型、本地链接、当前 xtask 引用、
  设计索引分类和 parity 状态。
- suite 顺序执行，首个失败立即停止。
- 平台人工验收按 `doc/verification/manual/` 的直接命令执行。
- 性能采样按 `doc/verification/performance/` 的直接命令执行。
- 生成证据放在 `target/verification/`；需要提交的审计日志放在
  `doc/verification/evidence/<audit-id>/`。
