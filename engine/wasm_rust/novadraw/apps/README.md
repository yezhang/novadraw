# Novadraw Apps - 验证应用程序集

## 概述

本目录按运行环境组织 Novadraw 的应用入口。功能场景本身统一定义在
`novadraw-demo-scenes`，这里仅保留平台宿主、CLI、诊断和性能入口。

| 目录 | 职责 |
|------|------|
| `native/` | 基于 winit 的 macOS/桌面窗口应用 |
| `web/` | Wasm + DOM/WebGPU 浏览器应用 |
| `benchmarks/` | 无窗口性能基线，不属于 Native/Web UI |

## App 列表

| 平台 | App | 主题 | 场景数 | 运行命令 |
|------|-----|------|--------|----------|
| Native | **shape-app** | 图形类型 | 8 | `cargo run -p shape-app` |
| Native | **style-app** | 视觉属性 | 8 | `cargo run -p style-app` |
| Native | **transform-app** | M4 坐标域闭环 | 4 | `cargo run -p transform-app` |
| Native | **viewport-app** | Viewport Figure 树语义 | 4 | `cargo run -p viewport-app` |
| Native | **scroll-pane-demo** | M8 ScrollPane / RangeModel | 4 | `cargo run -p scroll-pane-demo` |
| Native | **clip-app** | 裁剪机制 | 10 | `cargo run -p clip-app` |
| Native | **layout-app** | 布局管理 | 10 | `cargo run -p layout-app` |
| Native | **event-app** | 输入事件 | 4 | `cargo run -p event-app` |
| Native | **border-app** | Border 装饰器 | 5 | `cargo run -p border-app` |
| Native | **update-app** | 更新生命周期 + 通知 | 4 | `cargo run -p update-app` |
| Native | **editor** | 集成编辑器 | - | `cargo run -p editor` |
| Native | **ndcanvas-app** | NdCanvas 底层 API | 8 | `cargo run -p ndcanvas-app` |
| Native | **vello-app** | Vello 原始 API | 1 | `cargo run -p vello-app` |
| Web | **web-validation** | 共享场景浏览器验证 | 69+ | `./scripts/build_web_validation.sh` |
| Benchmark | **r8-perf** | FigureTree 性能基线 | - | `cargo run -p r8-perf` |

## 主题划分原则

每个 App 验证一个引擎核心概念，不重叠：

| 概念 | App | 验证内容 |
|------|-----|----------|
| 图形类型 | shape-app | Rectangle, Ellipse, RoundedRect, Polyline, Triangle, Z-Order, Parent-Child |
| 视觉属性 | style-app | Fill color, Stroke (width/color/cap/join), Alpha, LineJoin, Stroke vs Border |
| 坐标域 | transform-app | 嵌套坐标根、absolute/relative 往返、坐标根移动、事件点降域 |
| 视口 | viewport-app | ViewportFigure、content 裁剪、scroll、ScalableLayeredPane、嵌套 viewport |
| 滚动容器 | scroll-pane-demo | RangeModel、ScrollPane、ScrollBar、wheel fallback、zoom |
| 裁剪 | clip-app | basic, nested, multi_layer, circle, path, transparent, animation |
| 布局 | layout-app | XYLayout, FillLayout, FlowLayout, BorderLayout, 嵌套, 约束更新 |
| 输入事件 | event-app | Mouse, Keyboard, Focus, capture、坐标根事件点 |
| Border 装饰器 | border-app | RectangleBorder, LineBorder, MarginBorder, Border+insets |
| 更新生命周期 | update-app | prim_translate, repaint, revalidate, notification effect, damage repair |
| 底层 API | ndcanvas-app | NdCanvas 直接调用: fill_rect, stroke_rect, ellipse, line, polyline |
| 渲染后端 | vello-app | Vello 原始 API 验证（不使用 novadraw） |

## 通用操作

所有 DemoApp 共享相同的操作方式：

- 按左右方向键 / `PageUp` / `PageDown` 切换场景
- 按 `Home` / `End` 切换到首个 / 最后一个场景
- 按数字键切换到对应场景
- 按 `S` 保存当前帧截图
- 按 `U` 切换 UpdateManager 开关（仅用于诊断）
- 按 `ESC` 退出程序

当前主线不提供迭代渲染入口或 `I` 键切换。核心渲染管线的完整人工验收步骤见
[`doc/verification/manual/core-pipeline.md`](../doc/verification/manual/core-pipeline.md)。

## 架构设计

```
novadraw/ (workspace)
├── novadraw-math/        ← 数学运算
├── novadraw-geometry/    ← 几何运算
├── novadraw-core/        ← 核心数据类型
├── novadraw-render/      ← 渲染后端
├── novadraw-scene/       ← 场景图、Figure 接口、UpdateManager
├── novadraw-apps/        ← 共享 DemoApp 框架
├── novadraw-demo-scenes/ ← Native/Web 共用场景目录
└── apps/
    ├── native/           ← winit/macOS/桌面宿主
    │   ├── editor/
    │   ├── *-app/
    │   └── scroll-pane-demo/
    ├── web/
    │   └── web-validation/
    └── benchmarks/
        └── r8-perf/
```

各功能场景必须定义在 `novadraw-demo-scenes`，并通过稳定的 suite/scene ID 注册到
catalog。`apps/native/*` 只保留 Native CLI、窗口运行和验证报告逻辑；
`apps/web/web-validation` 通过同一 catalog 构造 Web 场景。平台输入适配、截图保存
和 DOM/winit 代码不得进入共享场景 crate。

## 运行所有测试

```sh
cargo check --workspace
cargo test --workspace
```

## 添加新 App

1. 根据运行环境在 `apps/native/`、`apps/web/` 或 `apps/benchmarks/` 下创建目录
2. 添加 `Cargo.toml` 和 `src/main.rs`
3. 在 workspace `Cargo.toml` 中添加成员
4. 更新本 README
