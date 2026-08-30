# Novadraw Apps - 验证应用程序集

## 概述

本目录包含 Novadraw 图形引擎的各个功能验证 App，每个 App 专注于验证一个特定功能模块，不重叠。

## App 列表

| App | 主题 | 场景数 | 运行命令 |
|-----|------|--------|----------|
| **shape-app** | 图形类型 | 8 | `cargo run -p shape-app` |
| **style-app** | 视觉属性 | 7 | `cargo run -p style-app` |
| **transform-app** | M4 坐标域闭环 | 4 | `cargo run -p transform-app` |
| **viewport-app** | Viewport Figure 树语义 | 4 | `cargo run -p viewport-app` |
| **scroll-pane-demo** | M8 ScrollPane / RangeModel | 4 | `cargo run -p scroll-pane-demo` |
| **clip-app** | 裁剪机制 | 10 | `cargo run -p clip-app` |
| **layout-app** | 布局管理 | 8 | `cargo run -p layout-app` |
| **event-app** | 输入事件 | 4 | `cargo run -p event-app` |
| **border-app** | Border 装饰器 | 5 | `cargo run -p border-app` |
| **update-app** | 更新生命周期 + 通知 | 4 | `cargo run -p update-app` |
| **editor** | 集成编辑器 | - | `cargo run -p editor` |
| **ndcanvas-app** | NdCanvas 底层 API | 8 | `cargo run -p ndcanvas-app` |
| **vello-app** | Vello 原始 API | 1 | `cargo run -p vello-app` |

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
└── apps/
    ├── shape-app/        ← 图形类型验证
    ├── style-app/        ← 视觉属性验证
    ├── transform-app/    ← 坐标变换验证
    ├── viewport-app/     ← Viewport Figure 树语义验证
    ├── clip-app/         ← 裁剪验证
    ├── layout-app/       ← 布局验证
    ├── event-app/        ← 事件验证
    ├── border-app/       ← Border 装饰器验证
    ├── update-app/       ← 更新生命周期验证
    ├── editor/           ← 集成编辑器
    ├── ndcanvas-app/     ← NdCanvas 底层 API 验证
    └── vello-app/        ← Vello 原始 API 验证
```

## 运行所有测试

```sh
cargo check --workspace
cargo test --workspace
```

## 添加新 App

1. 在 `apps/` 下创建新目录
2. 添加 `Cargo.toml` 和 `src/main.rs`
3. 在 workspace `Cargo.toml` 中添加成员
4. 更新本 README
