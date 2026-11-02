# Novadraw Examples

本页是全仓库样例导航。Core 和 Editor 样例随各自 crate 维护，本目录保留跨层集成、
后端样例和共享宿主工具。验证命令和证据入口见
[`tests/README.md`](../tests/README.md)。性能基线独立放在
[`benchmarks/`](../benchmarks/)。

## 目录结构

| 目录 | 职责 |
|------|------|
| [`../crates/novadraw/examples/native/`](../crates/novadraw/examples/native/) | Core 桌面示例 |
| [`../crates/novadraw/examples/scenes/`](../crates/novadraw/examples/scenes/) | `novadraw-example-scenes`：Core Native/Web 共用的无平台场景 |
| [`../crates/novadraw-editor/examples/native/`](../crates/novadraw-editor/examples/native/) | Editor 桌面示例 |
| [`integration/`](integration/) | 跨层集成：Web 验证与双 Canvas 宿主 |
| [`backend/`](backend/) | 渲染后端示例 |
| `support/` | `novadraw-example-support`：示例共享宿主与工具 |

`support` 和 Core 的 `scenes` 是仓库内部的示例基础设施，不属于稳定产品 API。产品应用应直接
组合 `novadraw`、渲染后端、平台适配器，以及按需使用的 Editor 或 Inspector crate。
Shapes 与 Logic 共用 macOS 风格的 [GEF 样例工作台](support/README.md)，
统一窗口、工具栏、侧栏与状态栏样式。

目录按主要展示的能力归属，平台作为第二层分类。完整应用保留自己的 `Cargo.toml`
和独立 workspace package，使用 `cargo run -p <package>`；它们不作为库的 Cargo
example target，也不将 GUI 依赖加入 Core／Editor 的 `dev-dependencies`。

## 示例列表

| 归属／平台 | 示例 | 主题 | 运行命令 |
|------|------|------|----------|
| Core／Native | `shape-app` | 基础 Figure 与树结构 | `cargo run -p shape-app` |
| Core／Native | `style-app` | 填充、描边与透明度 | `cargo run -p style-app` |
| Core／Native | `transform-app` | 坐标转换与坐标根 | `cargo run -p transform-app` |
| Core／Native | `viewport-app` | Viewport Figure 树语义 | `cargo run -p viewport-app` |
| Core／Native | `scroll-pane-demo` | ScrollPane、RangeModel、Layer 与 Freeform | `cargo run -p scroll-pane-demo` |
| Core／Native | `clip-app` | 裁剪机制 | `cargo run -p clip-app` |
| Core／Native | `layout-app` | 布局管理器 | `cargo run -p layout-app` |
| Core／Native | `event-app` | 输入、焦点与 capture | `cargo run -p event-app` |
| Core／Native | `border-app` | Border 装饰器 | `cargo run -p border-app` |
| Core／Native | `text-app` | 文本、图像与资源 | `cargo run -p text-app` |
| Core／Native | `widgets-app` | Widget 与 Tooltip | `cargo run -p widgets-app` |
| Core／Native | `update-app` | 更新生命周期与通知 | `cargo run -p update-app` |
| Core／Native | `connections-demo` | Anchor、Router 与 Connection | `cargo run -p connections-demo` |
| Core／Native | `advanced-figures-app` | 连接装饰与路由、可缩放多边形、TextFlow | `cargo run -p advanced-figures-app` |
| Core／Native | `animation-app` | 正交模型、Motion/Composition、Trigger、运行期控制与领域动画 | `cargo run -p animation-app` |
| Core／Native | `uml-demo` | 可拖拽复合 Figure、嵌套布局与 UML 关系自动重路由 | `cargo run -p uml-demo` |
| Core／Native | `draw2d-official-examples` | Draw2D 官方 25 个示例入口的对等实现与缺口目录 | `cargo run -p draw2d-official-examples` |
| Editor／Native | [`node-editor-demo`](../crates/novadraw-editor/examples/native/node-editor-demo/) | Editor 交互与命令历史 | `cargo run -p node-editor-demo` |
| Editor／Native | [`gef-editor-demo`](../crates/novadraw-editor/examples/native/gef-editor-demo/README.md) | Shapes 模型编辑与属性面板 | `cargo run -p gef-editor-demo` |
| Editor／Native | [`gef-logic-demo`](../crates/novadraw-editor/examples/native/gef-logic-demo/README.md) | Logic 编辑与连接路由 | `cargo run -p gef-logic-demo` |
| Core／Native | `ndcanvas-app` | NdCanvas 绘图 API | `cargo run -p ndcanvas-app` |
| 后端／Native | [`vello-app`](backend/vello-app/) | Vello 原始 API | `cargo run -p vello-app` |
| 集成／Native | [`winit-canvas-demo`](integration/winit-canvas-demo/README.md) | 双 Canvas、后端与平台桥接 | `cargo run -p winit-canvas-demo` |
| 集成／Web | [`web-validation`](integration/web-validation/README.md) | Core 共享场景与 Editor 浏览器验证 | `./tools/scripts/build_web_validation.sh` |

## 共享场景

功能场景定义在 `novadraw-example-scenes`，通过稳定的 suite/scene ID 注册到 catalog。
Core 的 `examples/native/*` 负责 Native CLI、窗口运行和验证报告；
`examples/integration/web-validation` 使用同一
catalog 构造浏览器场景。平台输入适配、截图保存和 DOM/winit 代码不得进入共享场景
crate。

使用 `novadraw-example-support` 场景宿主的 Native 示例提供以下统一操作：

- 左右方向键、`PageUp`、`PageDown`、`Home`、`End` 或数字键切换场景；
- `R` 重建并重播当前场景；
- `S` 保存当前帧截图；
- `U` 切换 UpdateManager 诊断开关；
- `Esc` 退出。

Web 示例的构建、启动和验收方法见
[`integration/web-validation/README.md`](integration/web-validation/README.md)。

## 验证

`node-editor-demo` 的平台无关编辑链路可通过 Headless Replay 重放：

```sh
cargo xtask verify g3
cargo xtask verify g4
cargo xtask verify g5.2
cargo xtask verify g5.3
cargo xtask verify g5.4
cargo xtask verify g5.5
```

UML 扩展样例的结构、路由和渲染投影可通过 Headless probe 验证：

```sh
cargo xtask verify example.uml-extension
```

动画正交模型、Trigger、layout/viewport/route transition、continuous dash、pulse、
source/presentation 分离、运行期 interruption/suspension 和 temporary visual 生命周期可通过固定时间采样验证：

```sh
cargo xtask verify example.animation-orthogonality
```

完整工作区门禁：

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

Draw2D 官方示例应用按上游源码顺序运行所有可构造场景，并通过 `--list` 输出 25 个
入口的 `complete`、`in_progress`、`not_implemented` 或 `blocked` 状态及具体缺口：

```sh
cargo run -p draw2d-official-examples -- --list
```

逐项源码行为、实现映射和能力差额见
[`draw2d-official-examples-parity-2026-10-16.md`](../doc/verification/reviews/draw2d-official-examples-parity-2026-10-16.md)。

## 添加示例

1. 按主要展示的能力，在 Core 或 Editor 的 `examples/<platform>/` 下创建独立 package；
   跨层集成放入根 `examples/integration/`，后端样例放入 `examples/backend/`。
2. Core 可复用的无平台场景放入 `crates/novadraw/examples/scenes/`。
3. 跨样例的宿主共用逻辑放入根 `examples/support/`，不放入产品专用逻辑。
4. 在 workspace `Cargo.toml` 中注册 package。
5. 需要跨工具自动验证时，在 xtask 中登记稳定 suite。
6. 更新本 README。
