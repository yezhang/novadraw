# ADR-022: 第三方类型与渲染依赖边界

类型：`architecture-decision`

## 状态

已接受，已验证；Vello package 归属已由 ADR-023 替代

## 背景

Novadraw 需要使用 Kurbo 的曲线算法、Vello 的 GPU renderer、Winit 的窗口事件以及
Resvg 的 SVG rasterization，但这些依赖的升级周期和平台覆盖范围不等同于引擎公共契约。
此前存在以下问题：

- `Affine2D::into_kurbo` 让 Geometry 公共 API 直接返回 Kurbo 类型；
- `Path` 同时负责 backend-neutral IR、Kurbo 曲线算法和 Vello `BezPath` lowering；
- `VelloRenderer::new` 直接要求 `winit::Window`，Web 初始化直接返回 `vello::Error`；
- Resvg 0.45 间接引入 Kurbo 0.11，而 Geometry、Render 和 Vello 使用 Kurbo 0.13；
- 现有 facade 无后端依赖检查不能发现公开签名泄漏、同库多版本或体积回归。

## 决策

### 1. 公共契约使用 Novadraw 领域类型

默认引擎与 facade 公共签名不得出现具体第三方实现类型。当前自动门禁至少禁止：

- `kurbo::`
- `vello::`
- `winit::`

公开值使用 `Point`、`Vec2`、`Rectangle`、`Dimension`、`Affine2D`、`SurfaceInfo`
和 Novadraw 自有错误类型。后端构造只接受 Novadraw 定义的窗口能力协议；具体 Winit
类型由应用层适配。平台句柄协议只允许存在于明确的 backend interop 边界，不能进入
Render IR、Scene、Editor 或 facade 的 backend-neutral 表面。

该规则由 host 与 wasm 两套 rustdoc 扫描执行。只做源码文本搜索不足以证明类型未通过
alias、re-export 或条件编译签名泄漏。

### 2. Kurbo 按能力归属，不按“第三方库”集中

Kurbo 是算法实现依赖，不是 Vello 专属类型。使用位置按语义分为三类：

| 类别 | 归属 | 规则 |
|------|------|------|
| 通用几何 | `novadraw::geometry` 私有实现 | `Affine2D` 可内部使用 Kurbo，但只公开系数和 Novadraw 值类型 |
| Render IR 算法 | `novadraw::render` 私有实现 | arc normalization、curve bounds 可使用 Kurbo；输入输出必须是 `PathOp` 和 Novadraw 几何值 |
| Vello lowering | `novadraw-backend-vello` | `vello::kurbo::BezPath`、stroke、clip 和 affine 只在 backend 内构造 |

`RenderCommand` 与 `PathOp` 不持有 Kurbo 值。Vello backend 不反向定义通用几何或
Render IR 语义。

### 3. 依赖版本尽可能统一

Workspace 直接 Kurbo 版本固定为 0.13.1。Resvg 升级到 0.48.1，使 Resvg、Usvg、
Svgtypes、Vello、Geometry 和 Render 收敛到 Kurbo 0.13.1。

验证使用完整 workspace、all-features、all-target dependency graph。若未来第三方依赖
确实无法统一，必须先记录不可统一的依赖链、影响和退出条件，再显式调整门禁；不得静默
接受新增 Kurbo 主版本。

### 4. 依赖、API 和体积分别验证

- 保留 `facade.dependencies`，证明无 feature 的 facade 不启用平台 renderer；
- `api.third-party-types` 检查公开签名；
- `dependencies.kurbo` 检查 Kurbo 单版本；
- `report.binary-size` 构建使用标准 facade/backend 路径的 release `shape-app`，记录字节数
  与 SHA-256。

体积报告是趋势证据，不设置缺乏历史数据支撑的固定阈值。需要在后续发布基线中比较同一
target、profile 和 feature 组合。

### 5. Vello 实现不得进入 Core

ADR-023 执行整体 crate 收口时，Vello 被提取到 `novadraw-backend-vello`。该拆分依据是
Core 的可选安装和平台依赖隔离，而不是预设第二个 renderer。共享 Render protocol
仍由 `novadraw` 定义，具体 backend 只单向依赖 Core。

## 失败处理

- 公开签名出现禁用类型时，host 或 wasm rustdoc 门禁失败并打印签名；
- Kurbo 出现零个或多个版本时，依赖门禁失败并打印版本集合；
- 体积构建失败时不生成报告；报告本身不替代性能或视觉正确性验证；
- backend 初始化失败通过 Novadraw 自有错误返回，不把 `vello::Error` 作为公共契约。

## 实施结果

- 删除 `Affine2D::into_kurbo`；
- 将 Path 的 arc normalization 与 bounds 收入私有 `path_geometry`；
- 将 `vello::kurbo::BezPath` 构造移入 Vello lowering；
- `novadraw` Core 不依赖 Winit 或 Vello，native backend 通过窗口句柄能力和
  `SurfaceInfo` 构造；
- Web backend 使用 `VelloInitializationError`；
- Resvg 0.48.1 将依赖图统一为 Kurbo 0.13.1；
- 公共 API、Kurbo 版本和二进制体积入口已登记到 `verification/suites.toml`。

## 关系

- [ADR-023](adr-023-crate-consolidation-and-extension-boundaries.md) 保留本文的第三方
  类型隔离原则，但已将 Vello lowering 提取到独立 backend crate；
- 收紧 [ADR-020](adr-020-engine-value-and-render-contract.md) 的 Geometry 与 Render IR
  第三方类型边界；
- 延续 [ADR-021](adr-021-public-facade-and-feature-boundary.md) 的 facade 与 backend
  feature 隔离；
- 不改变 `RenderBackend`、`RenderSubmission` 或 Scene/Editor 的 backend-neutral
  契约。

## 日期

2026-09-29
