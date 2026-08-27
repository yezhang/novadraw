# Novadraw 设计文档索引

本文档对 doc 目录下所有设计文档进行分类说明，便于查阅和理解。

---

## 文档分类结构

```text
doc/
├── 00-index.md                        # 文档索引（本文档）
│
├── 01-architecture/                  # 架构设计
│   ├── gef_principle.md              # GEF 框架核心原理
│   ├── draw2d_design_axioms.md       # Draw2D 底层设计公理
│   ├── draw2d_api_coverage.md        # Draw2D API 语义覆盖清单
│   ├── draw2d_notification_design.md # Draw2D 通知机制设计分析
│   ├── displaylist_design.md          # DisplayList 中间层设计
│   ├── draw2d-history.md             # draw2d 历史与架构演变
│   ├── zed_reactive_design.md        # Zed 响应式设计分析与通知机制借鉴
│   ├── m8-m9-contract-plan.md        # Viewport/Scroll 与 Connection 契约实施计划
│   ├── ideal-directory-structure.md   # 理想目录结构设计（模块/目录/crate 演进策略）
│   ├── swt-gc-analysis.md           # SWT GC 底层绘制 API 分析
│   ├── ideal-architecture-static.md   # 理想架构 - 静态结构（组件关系、Trait 层级）
│   └── ideal-architecture-dynamic.md  # 理想架构 - 动态结构（事件流、更新流程）
│
├── 02-figure/                        # Figure 核心
│   ├── figure_core_concepts.md       # Figure 核心概念
│   ├── figure_bounds.md               # Figure 边界机制
│   ├── figure_tree_position.md       # Figure 树父子位置
│   ├── ifigure_interface.md          # g2 IFigure 接口分析
│   ├── figure_implementation.md      # g2 Figure 实现分析
│   ├── figure_tree_operations.md     # g2 Figure 树操作机制分析
│   ├── figure_box_model.md           # 盒模型分析
│   └── layout-constraints.md         # 布局约束机制
│
├── 03-rendering/                      # 渲染管线
│   ├── rendering_pipeline.md          # 渲染管线概览
│   ├── update_manager_pipeline.md    # g2 UpdateManager + 渲染管线分析
│   ├── update_manager_design.md      # Novadraw UpdateManager 设计
│   ├── core-pipeline-review-2026-08-23.md # 核心渲染管线审查与修复状态
│   ├── manual_core_pipeline_verification.md # 核心渲染管线手工验收步骤
│   ├── manual_m8_viewport_verification.md # M8 Viewport/Scroll/Zoom 手工验收
│   ├── trampoline_rendering.md       # Trampoline 渲染任务管理
│   ├── displaylist_detailed.md       # DisplayList 详细设计
│   ├── graphics_api.md               # Graphics API 参考
│   └── clip_principle.md             # Clip 裁剪原理
│
├── 04-coordinates/                   # 坐标系与变换
│   └── coordinates.md                 # 坐标系统原理
│
├── 05-java-rust/                     # Java to Rust 迁移
│   ├── java_to_rust_oo.md            # Java OOP 特性等价实现
│   └── java_to_rust_migration.md     # 迁移步骤指南 + 多态支持
│
├── 06-roadmap/                       # 路线图（产品视图 + Demo 视图）
│   ├── 00-index.md                   # 路线图三方职能边界说明
│   ├── product-deliverables.md       # 每个 milestone 的产品策略层清单
│   └── demo-matrix.md                # 每个 milestone 的 demo + 验证矩阵
│
├── adr/                              # 架构决策记录
│   ├── README.md                     # ADR 列表与模板
│   ├── adr-001-webgpu-rust-stack.md  # Rust + WebGPU 技术栈决策
│   └── adr-002-notification-effect-queue.md # 通知机制 effect queue 决策
│
└── deprecated/                        # 历史文档（已被取代）
    └── 架构设计-历史.md              # 早期架构设计，已被《理想架构设计.md》取代
```

---

## 文档详细说明

### 1. 架构设计

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `gef_principle.md` | GEF 框架架构 | MVC 模式、EditPart 控制器、Command 模式、Request/EditPolicy 机制、连接支持 |
| `draw2d_design_axioms.md` | Draw2D 设计公理 | Figure 树、bounds、坐标根、两阶段更新、damage 修复、事件状态机 |
| `draw2d_api_coverage.md` | Draw2D API 覆盖 | 按 API family 记录 draw2d 语义契约、Novadraw 合理变体、覆盖检查点与后续 probes |
| `draw2d_notification_design.md` | Draw2D 通知机制 | Figure/Coordinate/Property/Ancestor/Input/Update 六类通知语义、实现方式、与 Zed/Novadraw 的对应关系 |
| `displaylist_design.md` | DisplayList 设计 | crate 设计决策、协议定义、与渲染层解耦方案 |
| `draw2d-history.md` | draw2d 历史 | draw2d 架构演变、设计决策背景 |
| `zed_reactive_design.md` | Zed 响应式设计 | `Entity<T>`、`notify/emit` 分离、`Subscription` 生命周期、effect flush、对 draw2d 等价通知机制的借鉴 |
| `m8-m9-contract-plan.md` | M8-M9 契约计划 | RangeModel/Viewport/ScrollPane 与 Connection/Anchor/Router 的 API、运行时所有权、验证门禁和实施顺序 |
| `ideal-directory-structure.md` | 理想目录结构 | 模块分层、目录命名、crate 演进顺序、目录调整禁止项 |
| `swt-gc-analysis.md` | SWT GC 分析 | SWT GC 底层绘制 API、IServerOcr2d 接口 |
| `ideal-architecture-static.md` | 理想架构 - 静态结构 | 组件关系图、Trait 层级、数据结构、平台解耦设计（d2 绘图） |
| `ideal-architecture-dynamic.md` | 理想架构 - 动态结构 | 事件分发流程、setCapture 机制、两阶段更新、数据流（d2 绘图） |
| `render-iterative-archive.md` | 迭代渲染归档 | 历史 POC 归档 tag、恢复条件、当前禁止项 |

### 2. Figure 核心

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `figure_core_concepts.md` | Figure 基础 | Figure 接口、paint 流程、GeometryHolder、父子关系 |
| `figure_bounds.md` | 边界机制 | bounds 定义、preferredSize、validate 流程、布局触发 |
| `figure_tree_position.md` | 父子位置 | 坐标系转换、translateToParent/useLocalCoordinates、嵌套变换 |
| `ifigure_interface.md` | IFigure 接口 | 接口方法分类、设计意图、上帝接口模式分析 |
| `figure_implementation.md` | Figure 实现 | 核心数据结构、paint/setBounds 实现、关键设计模式 |
| `figure_tree_operations.md` | 树操作机制 | 树遍历方法分类、递归/迭代机制、传播方向分析 |
| `figure_box_model.md` | 盒模型 | Bounds/Insets/ClientArea/Border/Outline 关系 |
| `layout-constraints.md` | 布局约束 | 约束系统、LayoutManager 接口、布局约束机制 |

### 3. 渲染管线

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `rendering_pipeline.md` | 管线概览 | 三环节验证：IR 层、后端层、场景层 |
| `update_manager_pipeline.md` | g2 UpdateManager | g2 LightweightSystem、UM 两阶段、Figure.paint、Graphics、EventDispatcher |
| `update_manager_design.md` | Novadraw UpdateManager | Novadraw SceneUpdateManager 实现、设计决策、与 g2 差异 |
| `core-pipeline-review-2026-08-23.md` | 核心管线审查 | 渲染、更新、事件、宿主和后端的缺陷清单与修复状态 |
| `manual_core_pipeline_verification.md` | 核心管线手工验收 | 从门禁、无窗口 verification 到渲染、事件、更新、坐标和裁剪的操作与通过标准 |
| `manual_m8_viewport_verification.md` | M8 手工验收 | Viewport、RangeModel、ScrollPane、ScrollBar、wheel fallback 与 scalable pane 的窗口验收 |
| `trampoline_rendering.md` | 任务遍历 | Trampoline 模式、任务队列、避免递归栈溢出 |
| `displaylist_detailed.md` | 详细实现 | RenderCommand 结构、场景图到命令的转换 |
| `graphics_api.md` | API 参考 | Graphics 状态管理、绘制 API、变换 API |
| `clip_principle.md` | 裁剪机制 | Clip 架构、LazyState、IServerOcr2d 接口 |

### 4. 坐标系与变换

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `coordinates.md` | 坐标系统 | 物理像素、入口域逻辑坐标、坐标根分段、Figure 树变换、MouseEvent 事件点降域 |

### 5. Java to Rust 迁移

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `java_to_rust_oo.md` | OOP 等价实现 | 20 种 Java OOP 特性与 Rust 对应关系 |
| `java_to_rust_migration.md` | 迁移指南 | 迁移步骤、多态调用支持、决策流程 |

### 6. 路线图

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `06-roadmap/00-index.md` | 路线图文档边界 | 语义账本 / 产品清单 / demo 验证三类文档的分工 |
| `06-roadmap/product-deliverables.md` | 产品交付清单 | 每个 milestone 下要交付的图元/布局/边框等策略层清单 |
| `06-roadmap/demo-matrix.md` | Demo 与验证矩阵 | 每个 milestone 配套的 demo + 截图断言/帧率断言策略 + GEF 层探索附录 |

> 路线图统一使用 `M1-M10` 编号；相关产品清单与验证矩阵集中维护在 `doc/06-roadmap/`。
> API 语义覆盖与 milestone 对应关系见 `doc/01-architecture/draw2d_api_coverage.md`。

### 7. 历史文档

| 文档 | 主题 | 说明 |
|------|------|------|
| `deprecated/架构设计-历史.md` | 早期架构设计 | 已被《理想架构设计.md》取代，仅作历史参考 |

### 8. 架构决策记录

| 文档 | 主题 | 关键内容 |
|------|------|----------|
| `adr/README.md` | ADR 索引 | 架构决策记录列表与模板 |
| `adr-001-webgpu-rust-stack.md` | 技术栈决策 | Rust、WebGPU、vello、winit、cosmic-text |
| `adr-002-notification-effect-queue.md` | 通知机制决策 | Draw2D 语义分层、Zed 式 effect queue、Novadraw 事务 flush 边界 |

---

## 阅读路径建议

### 新人入门

```text
1. gef_principle.md              # 理解整体架构
2. draw2d_design_axioms.md       # 理解最底层不变量
3. draw2d_api_coverage.md        # 理解 API family 与 Novadraw 覆盖账本
4. draw2d_notification_design.md # 理解 draw2d 的通知语义分层
5. figure_core_concepts.md       # 理解 Figure 模型
6. coordinates.md                # 理解坐标系
7. zed_reactive_design.md        # 理解现代 Rust 响应式/通知机制参考
```

### 渲染开发

```text
1. rendering_pipeline.md            # 了解管线结构
2. update_manager_pipeline.md      # g2 UpdateManager + EventDispatcher 机制
3. manual_core_pipeline_verification.md # 执行核心管线手工验收
4. trampoline_rendering.md        # 理解遍历机制
5. graphics_api.md               # 熟悉绘图 API
6. displaylist_detailed.md       # 深入实现细节
```

### 特性开发

| 场景 | 推荐文档 |
|------|----------|
| 裁剪功能 | `clip_principle.md` |
| 边界布局 | `figure_bounds.md` + `figure_tree_position.md` |
| 连接线 | `gef_principle.md` (连接章节) + `graphics_api.md` |
| 撤销重做 | `gef_principle.md` (Command 章节) |
| UpdateManager | `update_manager_pipeline.md` (g2 参考) + `update_manager_design.md` (本项目) |
| 通知体系 | `adr/adr-002-notification-effect-queue.md` + `draw2d_notification_design.md` + `zed_reactive_design.md` |

### Java to Rust 迁移

```text
1. java_to_rust_oo.md              # 理解 Java OOP 在 Rust 中的等价物
2. java_to_rust_migration.md       # 掌握迁移步骤和多态支持
3. ifigure_interface.md            # 分析源接口
4. figure_implementation.md        # 分析实现类
```

---

## 文档命名规范

- 使用小写字母和下划线
- 语义清晰，体现主题
- 避免过长的文件名
- 相关文档使用相似前缀以便分组

---

## 贡献指南

新增文档时请：

1. 确定所属分类，放置到对应子目录
2. 遵循现有文档的格式风格
3. 通过 markdownlint 检查
4. 更新本文档索引
