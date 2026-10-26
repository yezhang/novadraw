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
| `animation.tree` | 表现几何与容量预留、Tree 操作、Native GPU 中间帧及 retained-partial 像素；需要本机图形会话 |
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

## 边界

- `cargo xtask docs` 不编译 Rust，只检查 Markdown 类型、本地链接、当前 xtask 引用、
  设计索引分类和 parity 状态。
- suite 顺序执行，首个失败立即停止。
- 平台人工验收按 `doc/verification/manual/` 的直接命令执行。
- 性能采样按 `doc/verification/performance/` 的直接命令执行。
- 生成证据放在 `target/verification/`；需要提交的审计日志放在
  `verification/evidence/<audit-id>/`。
