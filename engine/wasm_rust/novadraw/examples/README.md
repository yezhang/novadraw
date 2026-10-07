# Novadraw Examples

本目录提供可运行、可阅读、可改造的 Novadraw 示例。示例同时承担集成验证职责，但
验证命令和证据仍由 `verification/suites.toml` 统一管理。性能基线独立放在
[`benchmarks/`](../benchmarks/)。

## 目录结构

| 目录 | 职责 |
|------|------|
| `native/` | 基于 winit 的桌面示例 |
| `web/` | Wasm + DOM/WebGPU 浏览器示例 |
| `scenes/` | `novadraw-example-scenes`：Native/Web 共用的无平台场景 |
| `support/` | `novadraw-example-support`：示例共享宿主与工具 |

`support` 和 `scenes` 是仓库内部的示例基础设施，不属于稳定产品 API。产品应用应直接
组合 `novadraw`、渲染后端、平台适配器，以及按需使用的 Editor 或 Inspector crate。

## 示例列表

| 平台 | 示例 | 主题 | 运行命令 |
|------|------|------|----------|
| Native | `shape-app` | 基础 Figure 与树结构 | `cargo run -p shape-app` |
| Native | `style-app` | 填充、描边与透明度 | `cargo run -p style-app` |
| Native | `transform-app` | 坐标转换与坐标根 | `cargo run -p transform-app` |
| Native | `viewport-app` | Viewport Figure 树语义 | `cargo run -p viewport-app` |
| Native | `scroll-pane-demo` | ScrollPane、RangeModel、Layer 与 Freeform | `cargo run -p scroll-pane-demo` |
| Native | `clip-app` | 裁剪机制 | `cargo run -p clip-app` |
| Native | `layout-app` | 布局管理器 | `cargo run -p layout-app` |
| Native | `event-app` | 输入、焦点与 capture | `cargo run -p event-app` |
| Native | `border-app` | Border 装饰器 | `cargo run -p border-app` |
| Native | `text-app` | 文本、图像与资源 | `cargo run -p text-app` |
| Native | `widgets-app` | Widget 与 Tooltip | `cargo run -p widgets-app` |
| Native | `update-app` | 更新生命周期与通知 | `cargo run -p update-app` |
| Native | `connections-demo` | Anchor、Router 与 Connection | `cargo run -p connections-demo` |
| Native | `advanced-figures-app` | 连接装饰与路由、可缩放多边形、TextFlow | `cargo run -p advanced-figures-app` |
| Native | `animation-app` | 正交模型、Trigger、布局/视口/路由与持续效果 | `cargo run -p animation-app` |
| Native | `uml-demo` | 可拖拽复合 Figure、嵌套布局与 UML 关系自动重路由 | `cargo run -p uml-demo` |
| Native | `node-editor-demo` | Editor 交互与命令历史 | `cargo run -p node-editor-demo` |
| Native | `ndcanvas-app` | NdCanvas 绘图 API | `cargo run -p ndcanvas-app` |
| Native | `vello-app` | Vello 原始 API | `cargo run -p vello-app` |
| Web | `web-validation` | 共享场景的浏览器版本 | `./scripts/build_web_validation.sh` |

## 共享场景

功能场景定义在 `novadraw-example-scenes`，通过稳定的 suite/scene ID 注册到 catalog。
`native/*` 负责 Native CLI、窗口运行和验证报告；`web/web-validation` 使用同一
catalog 构造浏览器场景。平台输入适配、截图保存和 DOM/winit 代码不得进入共享场景
crate。

Native 示例通过 `novadraw-example-support` 获得统一操作：

- 左右方向键、`PageUp`、`PageDown`、`Home`、`End` 或数字键切换场景；
- `S` 保存当前帧截图；
- `U` 切换 UpdateManager 诊断开关；
- `Esc` 退出。

Web 示例的构建、启动和验收方法见
[`web/web-validation/README.md`](web/web-validation/README.md)。

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
source/presentation 分离和 temporary visual 生命周期可通过固定时间采样验证：

```sh
cargo xtask verify example.animation-orthogonality
```

完整工作区门禁：

```sh
cargo xtask check --full
```

## 添加示例

1. 在 `native/` 或 `web/` 下创建 package。
2. 可复用的无平台场景放入 `scenes/`。
3. 示例宿主共用逻辑放入 `support/`，不要下沉产品专用逻辑。
4. 在 workspace `Cargo.toml` 中注册 package。
5. 需要自动验证时，在 `verification/suites.toml` 注册稳定命令与 suite。
6. 更新本 README。
