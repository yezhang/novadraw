# 理想目录与 Crate 边界

类型：`normative-design`

本文定义理想架构映射到 Rust 模块和 crate 的原则，不记录当前仓库迁移状态。

## 1. 原则

1. 目录首先表达职责和依赖方向。
2. 先稳定模块契约，再提升为独立 crate。
3. 不为每个概念创建 crate。
4. 平台实现、领域运行时和渲染后端必须可独立替换。
5. 目录结构不能迫使核心类型使用 `Arc<Mutex<_>>` 或不必要的 trait object。

## 2. 逻辑分层

```text
core + geometry
  ↑
figure / layout protocols
  ↑
tree
  ↑
runtime
  ↑
platform / application

render-protocol ← runtime
render-backend  → render-protocol
scene3d         → render-protocol
```

### Geometry

平台无关的二维几何：

- Point、Size、Rect、Insets；
- Affine2D；
- path 和精度策略；
- 相交、包含和投影后的二维 bounds。

3D 数学可以放在独立模块或依赖中，但不能把二维公共类型模糊成可切换的类型别名。

### Core

- `FigureId` 等运行时身份；
- 平台无关错误和基础枚举；
- 不依赖 Figure、Layout 或 Runtime 的稳定协议值。

### Figure

- `Figure` 和可选 capability；
- style、border 和 paint context；
- 内置 Figure。

### Layout

- LayoutManager；
- constraints；
- measurement；
- LayoutOutput 和缓存。

### Tree

- FigureTree；
- arena 和 FigureNode；
- NodeState；
- topology mutation；
- tree queries；
- 坐标链、hit-test 和 paint traversal 所需的树视图。

Tree 不包含交互 session、frame scheduling 或平台对象。

### Runtime

- InteractionState；
- EventDispatcher；
- UpdateManager；
- MutationQueue；
- Runtime 组合根；
- callback contexts 和 effects；
- frame preparation。

### Platform

- Winit adapter；
- Web adapter；
- Headless host；
- surface 生命周期；
- cursor、clipboard、IME 和 accessibility bridge。

### Render

- RecordingCanvas；
- CommandStream；
- RenderSubmission；
- RenderBackend；
- Vello、软件或测试后端；
- resource upload/release。

### Scene3D

可选独立子域：

- SpatialNode；
- Camera；
- 3D bounds 和 ray hit-test；
- material、lighting 和 depth；
- 与二维 Figure 的嵌入/合成适配。

## 3. 单 Crate 阶段

在边界稳定前，可以在一个 scene/runtime crate 中保持以下模块：

```text
src/
├── core/
│   ├── id.rs
│   └── error.rs
├── figure/
│   ├── traits.rs
│   ├── style.rs
│   ├── border/
│   └── builtin/
├── layout/
│   ├── traits.rs
│   ├── state.rs
│   └── builtin/
├── tree/
│   ├── node.rs
│   ├── node_state.rs
│   ├── topology.rs
│   ├── coordinates.rs
│   ├── hit_test.rs
│   └── traversal.rs
├── runtime/
│   ├── runtime.rs
│   ├── interaction.rs
│   ├── event/
│   ├── update/
│   ├── mutation/
│   ├── effects.rs
│   └── resources.rs
├── render/
│   ├── recording.rs
│   └── submission.rs
└── container/
    ├── viewport.rs
    ├── scroll_pane.rs
    └── layer.rs
```

平台实现和具体 GPU 后端即使早期位于同一 workspace，也应与上述领域模块分开。

## 4. Workspace 发布边界

当前结构为：

```text
novadraw
novadraw-editor
novadraw-inspector
novadraw-backend-vello
novadraw-platform-winit
novadraw-platform-web
examples/{native,web,support,scenes}/*
benchmarks/*
```

`novadraw` 直接拥有 Geometry、Figure、Layout、Tree、Runtime 和 Render protocol。
这些逻辑层使用 module 隔离，不再拆成独立 package。只有满足以下条件才新增 crate：

- 已有清晰且稳定的公开契约；
- 依赖方向单向；
- 能独立测试或复用；
- 拆分不会产生大量 facade 转发；
- 编译隔离或发布边界有实际价值。

## 5. 命名

- `tree` 表示节点存储和拓扑，不再使用含义过宽的 `scene` 承载全部机制；
- `runtime` 表示跨树、交互和更新的事务协调；
- `platform` 表示原生系统边界；
- `render` 表示绘制录制、提交和后端；
- `container` 表示 Viewport、Layer、ScrollPane 等 Figure 组合；
- `scene3d` 表示真正的 3D 场景，不与二维 tree 混用。

## 6. 禁止的依赖

- geometry 不依赖 Figure 或 Runtime；
- core 不依赖 Figure、Layout、Tree 或 Runtime；
- Figure 不依赖 FigureTree、Runtime、PlatformHost 或 RenderBackend；
- LayoutManager 不长期持有 FigureTree；
- FigureTree 不依赖 InteractionState、UpdateManager 或平台层；
- Runtime 不依赖 winit、DOM、AppKit、Win32 或具体 GPU 后端；
- RenderBackend 不回调修改 Runtime；
- PlatformHost 不执行布局、命中或 Figure 回调；
- Scene3D 不通过替换二维 Point/Rect 类型侵入 Figure API。

## 7. Facade

顶层 `novadraw` facade 只导出稳定用户入口：

- Runtime builder；
- Figure 和 LayoutManager 扩展接口；
- 平台无关输入与几何类型；
- RenderBackend / PlatformHost 边界；
- 常用内置 Figure 和容器。

内部 arena、队列、具体 dispatcher 和 update 数据结构不应因 facade 便利而公开。

Core 公开表面分为三层：

- crate root 与 `prelude`：仅保留高频、无歧义的应用入口；
- `figure`、`layout`、`container`、`connection`、`event`、`runtime`、`host`、
  `graphics`、`render`：稳定领域模块；
- `advanced`：低层诊断协议。

Editor、Inspector、Vello backend 和 Winit/Web platform adapter 是独立 package，
只单向依赖 `novadraw`。Core 不声明或转发具体后端 feature。

自 ADR-022 起，第三方类型与渲染依赖还受以下边界约束：

- backend-neutral 公共签名不得出现 Kurbo、Vello 或 Winit 类型；
- Kurbo 按通用 Geometry 私有实现、Render IR 私有算法、Vello lowering 三类归属；
- 完整 feature 依赖图只允许一个 Kurbo 版本；
- Vello 位于独立 `novadraw-backend-vello`；
- release 二进制体积生成独立趋势报告，不与无后端 facade 门禁混为一项。

3D 继续保留独立 Scene3D/Projective3D 扩展方向，但真实用例出现前不创建空 crate。
