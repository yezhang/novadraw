# AGENTS.md

## 角色定位

本文件是本仓库的启动宪法（bootstrap contract），用于所有 Agent 的首次进入与快速对齐。

- `AGENTS.md`：启动门禁、关键事实镜像、跨 Agent 最小约束。
- `CLAUDE.md`：完整规则手册与项目级唯一真源（SSOT）。
- `project_memory.md`：跨会话兜底记忆，保存高价值项目事实与长期约定。

如使用 Claude Code、Cursor、OpenCode、Zed 等任何 Agent 工具，均应先遵循本文件，再继续读取 `CLAUDE.md`。

## 启动门禁

首次进入仓库或开始新任务时，必须按以下顺序执行：

1. 先读取 `AGENTS.md`。
2. 若仓库根目录存在 `CLAUDE.md`，必须继续读取 `CLAUDE.md`，再开始分析、设计或实现。
3. 若任务涉及架构、g2/GEF 对标、第三方源码分析或历史决策，必须补充读取项目记忆（如 `project_memory.md`）。

未完成上述启动步骤前，不应假定自己已经掌握项目上下文。

## 启动检查清单

- [ ] 已读取 `AGENTS.md`
- [ ] 已读取 `CLAUDE.md`
- [ ] 已确认本次任务是否允许扫描本项目现状实现
- [ ] 已确认第三方参考源码路径
- [ ] 已确认本次任务的 SSOT 文档或代码入口

## 关键事实镜像

以下信息同时属于启动阶段必须掌握的关键事实，即使完整定义在 `CLAUDE.md` 中，也在此镜像一份，避免遗漏：

### 参考源码路径

- draw2d/GEF: `/Users/bytedance/Documents/code/GitHub/gef-classic`
- 可运行的 Draw2D 参考样例：`third_party/draw2d-examples/`；仅用于行为核验，不属于
  Novadraw 一方示例或实现源码
- 对标范围严格限定为该仓库中的 `org.eclipse.draw2d` 与 `org.eclipse.gef` 包；
  `org.eclipse.zest` 是基于 Draw2D 的上层扩展，不得作为 Novadraw 的需求语义、
  架构设计或实现逻辑参考
- SWT GC: `/Users/bytedance/Documents/code/GitHub/eclipse.platform.swt`
- vello: `/Users/bytedance/Documents/code/GitHub/vello`
- xilem: `/Users/bytedance/Documents/code/GitHub/xilem`
- Zed: `/Users/bytedance/Documents/code/GitHub/zed`

### 文档入口

- 项目文档总入口：`doc/00-index.md`
- 完整项目规则：`CLAUDE.md`
- **Draw2D API 语义覆盖账本**：`doc/parity/draw2d/api-coverage.md`
- **产品交付清单 / Demo 矩阵**：`doc/roadmap/`
  - M1-M10 编号与状态入口：`doc/roadmap/00-index.md`
- **Editor / GEF 路线图**：`doc/roadmap/editor/00-index.md`
- **Editor 架构 SSOT**：`doc/design/editor/architecture.md`
- **Core 公开 API 边界**：`doc/adr/adr-017-core-public-api-boundary.md`
- **Runtime 驱动与结构化测量 API**：
  `doc/adr/adr-018-runtime-driving-and-measurement-api.md`
- **公开 Facade 与 Feature 边界**：
  `doc/adr/adr-021-public-facade-and-feature-boundary.md`
- **第三方类型与渲染依赖边界**：
  `doc/adr/adr-022-third-party-type-and-render-dependency-boundary.md`
- **Crate 收口与扩展边界**：
  `doc/adr/adr-023-crate-consolidation-and-extension-boundaries.md`
- **统一 Graphics 与 glyph 预处理边界**：
  `doc/adr/adr-025-unified-graphics-and-glyph-preparation.md`；
  字体指标、文字测量与图文绘制统一 API，布局后曲线预处理与片段着色器数据归后端；
  已接受并完成 P2-G02 Core API 与默认/可替换轮廓链路；自研后端算法不属于当前交付
- **Core 公开 API 审计**：`doc/verification/reviews/core-public-api-audit-2026-09-22.md`
- **语义审计整改状态**：`doc/verification/reviews/draw2d-gef-semantic-remediation-2026-09-16.md`
- **可执行验证清单**：`verification/suites.toml`
> 推进 M1-M10 的 architecture/parity delta 时，必须检查对应 `api_semantics` 的 Draw2D API 语义是否完整；语义账本见 `doc/parity/draw2d/api-coverage.md`。

### 当前执行门禁

- 2026-10-02 起优先推进框架核心架构与 API 的灵活、扩展和稳定；以已完成的
  GA-1/GA-4 为基础，先推进 P2-S01，再评审 Graphics 与外部布局算法扩展。
  WindowServer/Chrome 性能采证及 Windows/Linux 原生 runner 验收后置到发布准备，
  保留未完成状态，不阻塞平台无关 Core 开发；当前顺序以
  `doc/roadmap/goal-alignment-adjustment-plan-2026-09-30.md` 为准。
- M1-M10、D3.1-D3.4 与 D4.1-D4.6 已完成；最近一次长期架构审计的 A01-A08 已关闭。
- 2026-09-13 macOS/Web/Headless 总审计与 R9.4 capability 消融复查通过，Draw2D
  Core 1.0 完成。Editor framework 已按 ADR-015 完成独立 G0-G5 roadmap，G0 架构启动与
  G1 Model Adapter/CommandStack、G2 EditPart Tree/Viewer 投影、G3
  Selection/Targeting/Input Arbitration、G4 Tool/Request/EditPolicy、G5.1
  Connection Projection、G5.2 Connection Creation 与 G5.3 Connection Reconnect
  已完成，G5.4 Connection Bendpoint 与 G5.5 Viewport/Auto-expose 自动门禁已完成；
  检查点 B 与检查点 C 已通过人工验收。
  Draw2D 后续能力必须进入明确的 P2 delta。
- Core 公开 API 的 P0 Batch A/B 已按 ADR-017 完成；Runtime 驱动、坐标查询和
  Layout measurement 已按 ADR-018 收口；detached 构造与挂载后 scoped editor
  调用面已按 ADR-019 收口；Color、Render IR 与 Geometry 基础值已按 ADR-020 收口；
  公开 API 分层已按 ADR-021 收口；Core、Editor、Inspector、Vello backend 与平台
  adapter 的 package 边界已按 ADR-023 收口。Graphics 双方言与 Figure capability
  属于后续 P1/P2；P2-R02 image source rectangle 已完成。
  原 G6 的 schema、serializer 和产品级 Native/Web 场景已移交独立产品包，不再作为
  本仓库引擎门禁。
- 2026-09-16 全量 Draw2D/GEF 语义审计及 2026-09-20 后续批次的 22 条 P1
  已全部关闭；18 条次级候选保留待定向验证，状态以
  `doc/verification/reviews/draw2d-gef-semantic-remediation-2026-09-16.md` 为准。
- 2026-09-10 架构修订以 `doc/adr/adr-014-extensibility-and-lifecycle-boundaries.md`
  为准；ADR-013 已替换。D4.3-D4.6 门禁已完成，新设计不等于已实现。
- 默认架构检索排除 `doc/archive/`；旧 ADR 全文只供显式历史追溯，不能用于当前设计。
  审计入口：`doc/verification/reviews/adr-audit-2026-09-10.md`。

### 架构分析边界

- 架构设计优先从需求、第一性原理、draw2d/GEF 参考源码出发。
- 第三方源码分析仅对标 `org.eclipse.draw2d` 与 `org.eclipse.gef`；不得用
  `org.eclipse.zest` 的源码或行为作为 Novadraw 的需求、语义、架构或实现依据。
  扩展能力必须直接从 Draw2D/GEF 的基础契约和扩展点确认。
- 若任务是“理想架构设计”，禁止先扫描本项目实现，以避免现状偏差。
- 若任务是“实现修复或落地”，必须先明确目标契约，再审阅本项目代码。

## 核心规则

| 规则 | 说明 |
|------|------|
| 树遍历 | 递归深度限制 10,000 层；性能专项开始前不得重新引入迭代渲染主线 |
| 禁止临时方案 | 问题必须从根因解决 |
| 禁止全局状态 | 不使用 Singleton |
| 渲染热路径 | 不打印日志 |
| 渲染主循环保护 | 当前主线只保护 `novadraw/src/graph/render_recursive.rs`；`render_iterative.rs` 已归档到 tag `archive/render-iterative-poc-20260617` |
| 硬编码 | 业务代码中不使用 magic numbers |
| 通用机制分层 | 事件分发、坐标转换、事件点适配、通用上下文必须放在引擎层，examples 只做平台输入适配与示例编排 |
| 第三方类型边界 | backend-neutral 公共签名不得暴露 Kurbo、Vello 或 Winit 类型 |
| Git 提交 | 提交信息摘要必须使用中文，并按主题保持原子化 |

## 项目特性

- **语言**: Rust (Edition 2024)
- **渲染**: Vello (WebGPU)
- **构建**: `cargo build && cargo test`
- **分层门禁**: 修改内环使用 crate 级 check/精确测试；功能切片运行对应 suite；
  `cargo xtask check --quick` 用于同类整改批次，`cargo xtask check --full` 只在最终
  提交、推送、合并或里程碑关闭前执行一次
- **公开包**: `novadraw` Core、`novadraw-editor`、`novadraw-inspector`、
  `novadraw-backend-vello`、`novadraw-platform-winit`、`novadraw-platform-web`
- **3D 边界**: 保留独立 Scene3D / Projective3D 扩展契约；真实用例出现前不建设空 crate

## 交互方式原则（摘要）

> 详细版见 `CLAUDE.md` 的“交互方式”章节；此处为跨 Agent 的关键摘要，确保不同工具保持一致行为。

### 思维原则

- 使用第一性原理推导；不盲从经验与路径依赖。
- 可以采用苏格拉底提问法和奥卡姆剃刀原理。
- 需求不明确时先澄清，再执行；目标清晰但路径低效时提出更优方案。

### 交互模式

- 默认直接推进用户目标：读取必要上下文、定位问题、实现修改、运行验证，并给出清晰结果。
- 不要求每次回复固定拆分为“直接执行 / 深度交互”。
- 深度交互仅在能降低误解、架构偏移或长期维护风险时主动展开。
- 简单问答 / 小修改直接完成；Bug 修复先定位根因再修复验证；新功能先定义契约再测试实现；架构设计先澄清目标与约束。
- 大范围或高风险修改遵循 Explore → Plan → Gate → Execute → Verify → Review。
- 验证成本随风险逐级提升；不得在每个小修改后重复运行 workspace 全量门禁。

### 深度交互触发条件

- 用户目标不清晰，可能存在 XY 问题。
- 当前做法会破坏项目长期架构。
- 存在多条路径且代价差异明显。
- 修改超过单一模块或超过 50 行。
- 涉及渲染主循环、坐标协议、事件分发、UpdateManager 等核心契约。

### 行为准则

- 重大架构变更：先评审方案，获批后实施。
- Bug 修复：先定位根因并说明，再提交修复。
- 代码改动：超过 50 行应分步提交，保持粒度清晰。
- 新增功能：先定义接口契约，再迭代实现细节。
- 性能优化：提供基准数据或统计支撑（前后对比）。

### 核心禁止事项（复述）

- 递归遍历深度上限 10,000 层；性能专项开始前不得重新引入迭代渲染主线。
- 禁止临时方案与“先糊后修”，必须从根因解决。
- 禁止全局状态（Singleton）。
- 渲染热路径禁止打印日志。
- `render_recursive.rs` 是当前渲染主循环保护区，通常不应改主循环逻辑；`render_iterative.rs` 为历史 POC，已从主线删除并归档到 tag `archive/render-iterative-poc-20260617`。
- 递归渲染在 M1-M10 核心契约完备前，不得恢复迭代渲染入口、I 键切换或递归/迭代等价门禁。
- 禁止 magic numbers（硬编码）。
- Git 提交信息摘要必须使用中文，并按主题保持原子化。

## 三层信息分工

为避免关键信息只存在于单一文档，约定如下：

### AGENTS.md 负责什么

- 启动门禁
- 必读文件顺序
- 关键事实镜像
- 跨 Agent 最小行为约束

### CLAUDE.md 负责什么

- 完整规则说明
- 详细开发流程
- 架构设计原则
- 提交、调试、实现细则

### project_memory.md 负责什么

- 跨会话仍需保留的高价值事实
- 容易遗漏但会显著影响判断的路径、约束与长期决策
- 不适合塞进启动摘要、但又不能依赖短期会话记忆的项目背景

## 维护规则

- 若某条信息会影响“搜索范围、源码定位、设计结论、任务边界”，则不得只存在于 `CLAUDE.md`。
- 这类信息至少应在 `AGENTS.md` 中保留摘要镜像，并在需要时同步到 `project_memory.md`。
- 若 `AGENTS.md` 与 `CLAUDE.md` 存在冲突，以 `CLAUDE.md` 为唯一真源（SSOT）。
- 若 `AGENTS.md` 未明确要求读取 `CLAUDE.md`，应视为配置缺陷而非可忽略项。
