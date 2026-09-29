# ADR-023: Crate 收口与扩展边界

类型：`architecture-decision`

## 状态

已接受，已验证

## 背景

当前 workspace 按早期逻辑分层拆出 `novadraw-core`、`novadraw-geometry`、
`novadraw-render` 和 `novadraw-scene`。这些 package 并没有形成独立发布、版本或复用
边界，反而迫使 Figure、Layout、Tree、Runtime 之间的内部协议跨 crate 公开。
顶层 `novadraw` 只是聚合 facade，并且无条件依赖 `novadraw-editor`，普通绘图应用无法
只安装核心引擎。

同时，`novadraw-render` 混合 backend-neutral Render IR 与具体 Vello 实现，
`examples/support` 混合可复用的平台输入/宿主适配和 demo shell。Winit、Web 与 Vello 的
依赖边界没有对应到真实的可选安装和移植需求。

未来可能加入完整 3D 场景、将二维 Figure 投影为 3D 视图，或在二维 Figure 树中嵌入
局部 3D 视口。当前没有可验证的 3D 消费者，不应为此建设空 crate，但本次收口也不能
封死后续扩展方向。

## 决策

### 1. `novadraw` 是平台无关 Core

将原 `novadraw-core`、`novadraw-geometry`、`novadraw-render` 的 backend-neutral
协议以及 `novadraw-scene` 合并到 `novadraw`。逻辑职责继续通过 Rust module 表达：

```text
novadraw
├── geometry
├── graphics
├── render
├── figure
├── layout
├── tree
├── runtime
├── event
├── host
├── container
└── connection
```

合并不表示取消内部边界。Geometry、Figure、Layout、Tree、Runtime 和 Render
protocol 继续保持单向职责；只是不再为了尚不存在的独立发布需求暴露 crate 间实现协议。

`novadraw` 不依赖 Editor、Inspector、具体 GPU backend、Winit、DOM 或应用 package。

### 2. Editor 与 Inspector 保持可选

`novadraw-editor` 作为图形编辑框架独立发布，只单向依赖 `novadraw`。
`novadraw` 不重导出 Editor。需要编辑能力的用户显式添加 `novadraw-editor`。

`novadraw-inspector` 作为只读诊断能力独立发布，只单向依赖 `novadraw`。在真实 IPC
协议需要独立版本兼容前，不额外拆分 protocol crate。

### 3. 具体 backend 与 platform adapter 独立

Vello 实现提取为 `novadraw-backend-vello`。它实现 `novadraw` 定义的
`RenderBackend`，拥有 Vello、WGPU、surface 和 backend-local cache，不拥有 FigureTree、
Runtime 或平台事件语义。

平台适配分别提取为：

- `novadraw-platform-winit`：窗口输入、IME、cursor、redraw 和宿主生命周期适配；
- `novadraw-platform-web`：DOM 输入、canvas、浏览器调度和宿主生命周期适配。

Winit 与 Web 是 platform adapter，不是 render backend。应用 composition root 显式组合：

```text
Native = novadraw + novadraw-platform-winit + novadraw-backend-vello
Web    = novadraw + novadraw-platform-web   + novadraw-backend-vello
Test   = novadraw + test host/backend
```

backend crate 不依赖 platform adapter；platform adapter 不拥有 RenderBackend、
FigureTree 或 Runtime 的领域实现。

### 4. 3D 保留契约，不建设空骨架

本次不创建 `novadraw-3d`、`novadraw-scene3d` 或其他占位 package，也不把无消费者的
`novadraw-math` 合入二维 Core。`novadraw-math` 从主 workspace 移除。

二维 Core 保留以下稳定扩展条件：

- `RenderBackend` 和 `RenderSubmission` 允许未来增加独立的 3D/compositor backend；
- `PlatformHost` 只协调平台生命周期，不假定唯一二维 GPU 实现；
- Figure 的绘制与命中协议继续使用明确的二维局部坐标，不替换为 2D/3D 可切换别名；
- 局部 3D 视图通过明确的 Figure、resource/surface 与合成边界接入；
- 完整 3D 场景使用独立的 SpatialNode、Camera、3D bounds、ray hit-test、material 和
  depth 契约，不侵入二维 FigureTree。

只有出现真实的 Scene3D 用例、至少一个消费应用以及可执行验证后，才决定 3D package
名称和边界。

### 5. Workspace 私有 package 不发布

`examples/*`、`benchmarks/*` 和 `xtask` 是示例、验证或工具 package，显式设置
`publish = false`。它们不构成框架公共 crate 集合；示例是否参与门禁由
`verification/suites.toml` 表达，不通过目录归属区分。

### 6. 不保留旧 crate 转发壳

这是 0.1 阶段的一次 breaking migration。完整迁移闭合后删除
`novadraw-core`、`novadraw-geometry`、`novadraw-render`、`novadraw-scene`，不保留
兼容转发 package，避免永久维护重复入口。

## 公共路径

已有 facade 用户路径尽量保持：

- `novadraw::geometry`
- `novadraw::graphics`
- `novadraw::render`
- `novadraw::figure`
- `novadraw::layout`
- `novadraw::tree`
- `novadraw::runtime`
- `novadraw::event`
- `novadraw::host`
- `novadraw::container`
- `novadraw::connection`

原始实现 crate 路径属于 0.1 内部迁移面，不承诺兼容。Editor 用户路径改为
`novadraw_editor::*`，具体 Vello 类型改为 `novadraw_backend_vello::*`。

## 失败处理

- 若合并迫使 Core 依赖 Winit、WebSys 或 Vello，依赖门禁失败；
- 若 Editor 或 Inspector 形成反向依赖，Cargo 依赖图无法闭合，迁移不得发布；
- 若 backend 或 platform adapter 暴露第三方类型到 backend-neutral Core API，
  公共 API 门禁失败；
- 若 3D 扩展需要修改二维 Point/Rectangle/FigureTree 的基础语义，应先新增 ADR，
  不得以兼容补丁侵入 Core；
- 任一阶段无法保持 Native、Web、Headless 行为契约时，完整迁移不得发布。

## 迁移顺序

1. 提取 `novadraw-backend-vello`；
2. 合并 Core、Geometry、Render protocol 与 Scene/Runtime；
3. 将 Editor、Inspector 改为只依赖 `novadraw`；
4. 提取 Winit 与 Web platform adapter；
5. 移除旧 crate 和未使用 Math；
6. 更新文档、验证入口和发布元数据；
7. 通过定向、quick、Web 与 full workspace 门禁。

每个阶段独立提交，但只在完整依赖图闭合后对外发布。

## 实施结果

- `novadraw` 已直接拥有 Geometry、Render protocol、Figure、Layout、Tree 和 Runtime；
- `novadraw-editor` 与 `novadraw-inspector` 已改为只依赖 `novadraw`；
- Vello、Winit 和 Web 适配分别位于独立 crate；
- 旧 Core/Geometry/Render/Scene package 与无消费者 Math package 已移除；
- app、fixture、benchmark 和 xtask package 已标记为不发布；
- 3D 扩展契约由现有 RenderBackend、surface composition 和独立 Scene3D 边界保留，
  未创建占位 crate。

## 验收条件

- `cargo tree -p novadraw` 不包含 Editor、Inspector、Vello、Winit 或 WebSys；
- `novadraw-editor` 和 `novadraw-inspector` 只单向依赖 `novadraw`；
- backend crate 不依赖 platform 或 app package；
- Native、Web、Headless composition root 使用同一 `novadraw`；
- 公共 API 第三方类型、Kurbo 单版本和二进制体积门禁继续通过；
- Core、Editor、Inspector、Native、Web 与 full workspace suite 通过。

## 替代关系

- 替代 [ADR-021](adr-021-public-facade-and-feature-boundary.md) 中由 `novadraw` 聚合
  Editor 和具体 backend feature 的 package 结构；
- 替代 [ADR-022](adr-022-third-party-type-and-render-dependency-boundary.md) 中
  “第二个生产 backend 出现后再拆 Vello crate”的暂缓决策；
- 保留 ADR-021 的稳定 facade 分层原则和 ADR-022 的第三方类型隔离原则。

## 日期

2026-09-29
