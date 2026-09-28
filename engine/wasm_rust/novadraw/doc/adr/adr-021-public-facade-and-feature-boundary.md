# ADR-021: 公开 Facade 与 Feature 边界

类型：`architecture-decision`

## 状态

已接受，已验证

## 背景

`novadraw-core`、`novadraw-geometry`、`novadraw-render`、`novadraw-scene` 和
`novadraw-editor` 已形成稳定的单向依赖链，但聚合 crate `novadraw` 仍是开发期导出：

- crate root 平铺重导出 Figure、Layout、事件、通知、资源、submission、backend
  session 和内部状态类型；
- 高频应用 API 与 `FigureNode`、`NodeState`、`UpdateManager`、`RenderCommand` 等
  低层协议处于同一命名层级；
- 默认 feature 启用 native Vello/Winit，使平台无关用户隐式承担后端依赖；
- native 与 web feature 直接沿用实现名，且 `novadraw-scene` 还有一层无行为的后端转发；
- Editor framework 是本仓库的引擎能力，却要求下游同时理解并依赖另一个 crate；
- rustdoc 缺少从普通使用、扩展实现到 backend 集成的清晰导航。

Facade 的职责不是掩盖内部 crate，而是提供稳定、可学习的默认路径。需要底层扩展的用户
仍可显式依赖子 crate，但普通使用者不应先理解 workspace 拆分。

## 决策

### 1. Facade 使用三层公开表面

#### 1.1 Crate root

crate root 只保留高频且无歧义的稳定入口：

- 基础值：`Color`、`Point`、`Vec2`、`Rectangle`、`Dimension`、`Insets`、
  `PointList`、`Affine2D`；
- 生命周期入口：`Runtime`、`FigureTree`、`FigureTreeBuilder`、`FigureId`；
- 核心扩展 trait：`Figure`、`LayoutManager`、`PlatformHost`、`RenderBackend`；
- 绘图入口：`NdCanvas`；
- 常用 Figure 与 Layout。

错误、配置、专业扩展协议和低层状态不因使用频率偶然升到 root。

#### 1.2 Prelude

`novadraw::prelude::*` 面向常规 Figure/Runtime 开发，包含 root 高频类型以及实现自定义
Figure、Layout 和 host 时经常需要的 trait。Prelude 不包含：

- raw `RenderCommand` / submission journal；
- backend session 与资源同步细节；
- `FigureNode`、`NodeState`、`LayoutState`；
- `UpdateManager`、`NotificationQueue`、`PendingMutations`、`EventDispatcher`；
- 具体平台 backend。

#### 1.3 显式模块

专业能力按领域组织：

- `geometry`、`graphics`、`figure`、`layout`、`container`、`connection`；
- `event`、`runtime`、`host`；
- `render::{command,submission,text}`；
- `editor`；
- `advanced`，仅承载确有外部诊断/深度集成价值、但不应位于 root 的低层类型；
- `backend`，仅在对应 feature 启用时存在。

Facade 不再 `pub use` 整个实现 crate，防止其新增公开项自动扩大聚合 API。

### 2. Editor 属于引擎 facade

`novadraw-editor` 是平台无关的 GEF 风格引擎层，不是产品包。`novadraw` 直接依赖并通过
`novadraw::editor` 导出其公共 API，但不把 Editor 类型平铺到 crate root 或 prelude。

`novadraw-inspector` 继续是独立诊断包。它不是构建普通图形应用或编辑器所必需的依赖，
因此本批次不把它并入默认 facade。

### 3. 默认构建不启用平台 backend

`novadraw` 的默认 feature 为空。后端由使用方显式选择：

- `native-vello`：启用 native Vello/Winit backend；
- `web-vello`：启用 Web Vello backend。

feature 只从 facade 向 `novadraw-render` 单向传播。`novadraw-scene` 不声明或转发具体
backend feature，因为 Scene/Runtime 只依赖 backend-neutral render protocol。

不保留 `vello`、`vello-web` 兼容 feature。当前版本为 `0.1.0`，workspace 调用方同步
迁移，避免同一能力长期存在实现名和平台名两套入口。

### 4. 子 crate 仍是显式扩展边界

本决策不把实现 crate 改为私有，也不新增 crate：

- 普通消费者依赖 `novadraw`；
- backend、host 或框架扩展者可按需直接依赖 `novadraw-render`、
  `novadraw-scene` 或 `novadraw-editor`；
- facade 只稳定选定路径，不承诺转发子 crate 的全部公开项。

### 5. 文档与编译契约

- `novadraw` README/rustdoc 先展示无 backend 的核心构建，再说明 feature；
- 文档按“常规入口、领域模块、低层集成、后端选择”组织；
- 外部集成测试验证 root、prelude、领域模块和 Editor 路径；
- `cargo check -p novadraw --no-default-features` 是核心无平台依赖门禁；
- native/web feature 各有独立编译门禁；
- workspace 应用优先从 facade 导入；仅在测试目标明确验证子 crate 时直接依赖子 crate。

## 失败处理

- 未启用 backend feature 时，`novadraw::backend` 不存在，避免运行期 unsupported；
- 同时启用 native/web feature 不改变 backend-neutral API；目标平台不支持的构造函数
  继续由 backend crate 的 target `cfg` 控制；
- facade 移除的 root 名称不提供 deprecated alias，编译错误应指引用户进入领域模块；
- feature 组合错误必须在编译期暴露，不通过运行时分支隐藏。

## 验证

- root 与 `prelude` 的外部导入测试；
- `figure`、`layout`、`connection`、`render`、`editor` 模块导入测试；
- `cargo check -p novadraw --no-default-features`；
- `cargo check -p novadraw --no-default-features --features native-vello`；
- `cargo check -p novadraw --no-default-features --features web-vello`；
- native demo、Web validation 和 Editor 测试继续通过；
- `cargo xtask docs`、`cargo xtask check --quick`；
- 最终交付执行一次 `cargo xtask check --full`。

## 实施结果

2026-09-28 已完成：

- crate root、`prelude`、领域模块、`advanced` 与条件编译 `backend` 已按三层表面落地；
- `novadraw-editor` 已纳入 `novadraw::editor`，workspace 应用已迁移到 facade；
- 默认 feature 已置空，native/web 后端分别由 `native-vello` 与 `web-vello` 单向启用；
- `novadraw-scene` 已移除 Vello feature 转发，核心 facade 依赖图不含 Vello、Winit
  与 macOS presentation 依赖；
- `novadraw/README.md`、rustdoc、外部 facade 契约测试及依赖图检查已建立；
- `core.facade`、`web.build`、文档、quick 与 full gate 均通过。

## 关系

- 落实 [ADR-017](adr-017-core-public-api-boundary.md) 的聚合导出后续项；
- 保持 [ADR-018](adr-018-runtime-driving-and-measurement-api.md) 的 Runtime 驱动边界；
- 保持 [ADR-019](adr-019-composable-api-and-scoped-editors.md) 的生命周期 API 分层；
- 延续 [ADR-020](adr-020-engine-value-and-render-contract.md) 的唯一基础值词汇；
- 落实
  [引擎能力与 API 稳定化评估](../verification/reviews/engine-capability-assessment-2026-09-28.md)
  的 E3 facade 收口。

## 日期

2026-09-28
