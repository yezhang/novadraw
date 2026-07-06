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
- SWT GC: `/Users/bytedance/Documents/code/GitHub/eclipse.platform.swt`
- vello: `/Users/bytedance/Documents/code/GitHub/vello`
- xilem: `/Users/bytedance/Documents/code/GitHub/xilem`
- Zed: `/Users/bytedance/Documents/code/GitHub/zed`

### 文档入口

- 项目文档总入口：`doc/00-index.md`
- 完整项目规则：`CLAUDE.md`
- **Draw2D API 语义覆盖账本**：`doc/01-architecture/draw2d_api_coverage.md`
- **产品交付清单 / Demo 矩阵**：`doc/06-roadmap/`
> 推进 M1-M10 的 architecture/parity delta 时，必须检查对应 `api_semantics` 的 Draw2D API 语义是否完整；语义账本见 `doc/01-architecture/draw2d_api_coverage.md`。

### 架构分析边界

- 架构设计优先从需求、第一性原理、draw2d/GEF 参考源码出发。
- 若任务是“理想架构设计”，禁止先扫描本项目实现，以避免现状偏差。
- 若任务是“实现修复或落地”，必须先明确目标契约，再审阅本项目代码。

## 核心规则

| 规则 | 说明 |
|------|------|
| 树遍历 | 递归深度限制 10,000 层；性能专项开始前不得重新引入迭代渲染主线 |
| 禁止临时方案 | 问题必须从根因解决 |
| 禁止全局状态 | 不使用 Singleton |
| 渲染热路径 | 不打印日志 |
| 渲染主循环保护 | 当前主线只保护 `render_recursive.rs`；`render_iterative.rs` 已归档到 tag `archive/render-iterative-poc-20260617` |
| 硬编码 | 业务代码中不使用 magic numbers |
| 通用机制分层 | 事件分发、坐标转换、事件点适配、通用上下文必须放在引擎层，apps 只做平台输入适配 |
| Git 提交 | 提交信息摘要必须使用中文，并按主题保持原子化 |

## 项目特性

- **语言**: Rust (Edition 2024)
- **渲染**: Vello (WebGPU)
- **构建**: `cargo build && cargo test`
- **模块**: `novadraw-core`, `novadraw-scene`, `novadraw-render`, `novadraw-math`

## 交互方式原则（摘要）

> 详细版见 `CLAUDE.md` 的“交互方式”章节；此处为跨 Agent 的关键摘要，确保不同工具保持一致行为。

### 思维原则

- 使用第一性原理推导；不盲从经验与路径依赖。
- 可以采用苏格拉底提问法和奥卡姆剃刀原理。
- 需求不明确时先澄清，再执行；目标清晰但路径低效时提出更优方案。

### 回答格式

- 输出包含两部分：
- 直接执行：按要求给出结果（代码、命令、变更点）。
- 深度交互：挑战需求动机与路径，提供替代方案。

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
