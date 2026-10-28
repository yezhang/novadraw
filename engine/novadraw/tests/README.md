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

长文本完整采样使用直接命令，smoke 不代替统计验收：

```bash
python3 tools/scripts/textflow_performance.py compare target/verification/long-text/comparison
python3 tools/scripts/textflow_performance.py summarize target/verification/long-text/comparison/baseline
python3 tools/scripts/textflow_performance.py summarize target/verification/long-text/comparison/optimized
python3 tools/scripts/textflow_performance.py memory target/verification/long-text/comparison
python3 tools/scripts/textflow_performance.py freeze target/verification/long-text/extended
python3 tools/scripts/textflow_performance.py run target/verification/long-text/extended --cold-per-session 0 --extended
target/release/textflow-perf 1 extended
```

`compare` 使用先前冻结的 `target/verification/long-text/baseline`，
交错运行两版本，每版本 5 个会话、每会话 20 次预热、200 次 fresh Runtime
与 40 个新进程。目录必须尚不存在，以保留每次原始证据。
`memory` 保留每版本 5 个交替新进程的 macOS 峰值 RSS。
`extended` 额外记录公开 viewport 滚动、resize 和 CPU 帧录制，
默认 5 个会话、每会话 20 次预热及 200 个样本，并按会话汇总。
冻结的旧版驱动没有这些场景，扩展采样只描述当前成本。
驱动不创建窗口或 GPU，呈现始终为 `unavailable`。

默认二进制路径遵循 `CARGO_TARGET_DIR`，也可通过 `--binary` 显式指定。
从源码归档恢复的目录没有 Git metadata 时，`smoke` 留存完整源码归档、
实际 Cargo.lock、字体与二进制散列；在 Git checkout 中仍留存 dirty patch。
显式的 `freeze --source-directory <恢复目录>` 可使用同一归档模式。

后续两版冻结驱动都支持扩展场景时，可用
`compare <新目录> --baseline <上一版冻结目录> --extended` 交替比较。
脚本先保留两版预检输出，缺少扩展记录或场景分母不符时停止，
避免把未运行的滚动、resize 当作有效样本。最初的优化前基线不支持此模式。

两份完整源码已冻结时，使用 `--optimized` 指定第二份二进制与 manifest。
脚本核对二进制和源码归档散列，要求两份二进制不同、驱动与输入相同。
重新构建两版时使用独立 Cargo target，并从源码归档恢复 Cargo.lock，
通过 `--locked` 构建；恢复材料以归档为准。
本轮已验证的冻结目录可用于重新采样，输出必须另建：

```bash
python3 tools/scripts/textflow_performance.py compare target/verification/long-text/phase4/repeat-original --baseline target/verification/long-text/phase4/baseline --optimized target/verification/long-text/phase4/optimized --extended
python3 tools/scripts/textflow_performance.py compare target/verification/long-text/phase4/repeat-paragraph --baseline target/verification/long-text/phase4/baseline --optimized target/verification/long-text/phase4/optimized --workload paragraph
python3 tools/scripts/textflow_performance.py memory target/verification/long-text/phase4/repeat-paragraph --workload paragraph
```

`--workload` 还支持 `line` 和 `mixed`，内存采样使用相同参数。
三种新增工厂分别为单个长段落、NoWrap 长行和含真实 Figure 的混排；
其源提交由 receipt 验证。阶段记录区分总耗时与自身耗时，
每个样本核对初始和追加阶段的自身耗时之和。
第四轮基线在混排缩窄到 386 时触发默认准备预算拒绝，
测量输出分离后的优化版预检成功；两版预算配置相同。
这项 resize 使用优化版单独采样，保留基线失败诊断，不计算前后耗时比。
标准宽度的成功分布与 resize 结果分别留存。
优化版已完成 1000 次热样本和 200 个新进程的重复 resize，失败为 0。
重新采样时先复制冻结目录到不存在的新路径，再运行同一采样器：

```bash
cp -R target/verification/long-text/phase4/optimized target/verification/long-text/phase4/repeat-mixed-resize
python3 tools/scripts/textflow_performance.py run target/verification/long-text/phase4/repeat-mixed-resize --extended --workload mixed
```

本轮详细结果和限制见[长文本实施记录](../doc/verification/reviews/long-text-performance-implementation-2026-10-27.md)。

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
  `doc/verification/evidence/<audit-id>/`。
