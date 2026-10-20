# Novadraw 仓库设计方案

类型：`repository-target-design`

状态：`accepted-target`

实施状态：`deferred`

本文记录 Novadraw 图形编辑框架与未来图形编辑产品的目标仓库设计。它定义长期边界和
迁移顺序，不表示 GitHub Organization、产品仓库或发布流程已经建立。

## 1. 决策摘要

目标是在同一个 GitHub Organization 下维护两个主要仓库：

```text
<organization>/
├── novadraw           # 图形编辑框架
└── novadraw-studio    # 图形编辑产品，名称暂定
```

- 当前仓库演进为纯粹的 **Novadraw Framework** 仓库；
- 面向最终用户的正式产品使用独立仓库；
- Core、Editor、Inspector、渲染后端和平台适配仍在同一个框架仓库；
- 产品把框架当作外部依赖，只通过公开 API 使用它；
- 产品原型可以短期作为 example 验证框架，但不能在框架仓库内继续演化为正式产品；
- 当前历史 `client/` 不作为未来产品仓库或产品代码的起点。

仓库名和 Organization 名需在实施时根据 GitHub 可用性最终确认。本文使用
`novadraw` 和 `novadraw-studio` 表达逻辑角色。

## 2. 为什么采用两个仓库

框架和产品有不同的治理目标：

| 维度 | 框架 | 产品 |
|---|---|---|
| 主要使用者 | 应用开发者、扩展作者 | 最终用户 |
| 稳定对象 | API、行为契约、扩展点 | 文档格式、工作流、交互体验 |
| 发布节奏 | 语义版本和兼容策略 | 产品版本和渠道发布 |
| Issue 分类 | API、平台、性能、正确性 | 功能、体验、数据和账号 |
| 许可策略 | 开源框架许可 | 可独立决定开源或商业策略 |
| 安全边界 | 依赖、内存、平台适配 | 用户数据、同步、账号和服务 |

独立产品仓库还建立了一项重要约束：产品必须像真实外部调用者一样消费框架。这样可以
持续发现公共 API 缺口，防止产品绕过 Runtime、依赖私有模块或把产品特例下沉到框架。

## 3. 框架仓库边界

框架仓库负责可复用、平台无关或具有明确替换契约的能力：

- `novadraw`：Figure、FigureTree、Runtime、布局、输入、连接和渲染协议；
- `novadraw-editor`：模型、EditPart、Viewer、Tool、Request、Policy、Command；
- `novadraw-inspector`：只读诊断与运行时观测；
- `novadraw-backend-*`：Vello 等可替换渲染后端；
- `novadraw-platform-*`：Winit、Web、Headless 等平台适配；
- 公共示例、benchmark、验证套件、书籍、规范和 ADR；
- 面向所有消费者的通用文件协议或扩展协议，但必须先经过独立设计评审。

以下内容不应进入框架：

- 产品专属画布、面板、菜单、快捷入口和引导流程；
- 产品业务模型、模板市场、账号、同步、计费和运营能力；
- 只为某个产品场景存在的 Figure、Policy 或 Runtime 快捷路径；
- 产品私有文档格式、存储策略和服务接口；
- 绕过公共 API 的产品专用内部访问。

内置能力和外部扩展继续使用同一套公开机制，不能因为产品由同一团队开发而获得私有
特权路径。

## 4. 产品仓库边界

产品仓库负责把框架组装成可交付的最终用户产品：

- 应用 Composition Root 和产品 UI；
- 产品领域模型、命令和工作流；
- 产品文档格式、迁移和持久化；
- 桌面端或 Web 端应用壳、安装包和自动更新；
- 用户配置、最近文件、模板和导入导出；
- 账号、同步、协作、服务端接口和遥测；
- 产品级集成测试、发布渠道、用户文档和隐私策略。

产品可以实现公开扩展点，但不能：

- 依赖框架私有模块或测试辅助 API；
- 在 CI 中使用指向开发者本机的 path dependency；
- 复制框架源码到产品仓库；
- 直接修改 FigureTree、UpdateManager 或其他受 Runtime 管理的状态；
- 要求框架中心枚举识别产品专属类型。

## 5. 依赖与版本策略

### 0.x 阶段

框架 API 仍快速演进时，产品使用 Git tag 或精确 commit 固定依赖：

```toml
[dependencies]
novadraw = { git = "https://github.com/<organization>/novadraw", rev = "<commit>" }
novadraw-editor = { git = "https://github.com/<organization>/novadraw", rev = "<commit>" }
```

本地联合开发可以通过 Cargo `[patch]` 或本地配置临时覆盖为相邻 checkout。该覆盖不能
提交为产品 CI 的默认依赖。

### 稳定发布阶段

公共 API 和发布流程稳定后，再决定是否：

- 发布 crates.io package；
- 只发布 Git tag 和 release artifact；
- 同时提供 crates.io 与 Git 依赖。

产品每次升级框架都应通过独立的 dependency-bump Pull Request，记录目标版本、迁移
内容和回归结果。不能默认跟随框架主分支。

## 6. 跨仓库变更流程

跨框架与产品的能力按以下顺序推进：

1. 在框架仓库定义通用问题、公开契约和失败语义；
2. 用框架测试或通用 example 证明机制，不提交产品业务代码；
3. 合入框架并产生可引用的 tag 或 commit；
4. 在产品仓库升级依赖并实现产品扩展；
5. 在产品验收中发现公共缺口时，重新回到框架仓库形成通用修订。

框架变更和产品适配不使用一个跨仓库的不可分原子提交。框架提交必须能够由自己的测试
独立证明，产品也必须能够固定或回退到上一个已知可用版本。

## 7. GitHub 治理

建议 Organization 至少维护：

```text
.github/               # 可选：组织主页和共享模板
novadraw/              # 公开框架仓库
novadraw-studio/       # 产品仓库
```

两个仓库分别维护：

- Issue、Project 和 roadmap；
- CI、release workflow 和版本号；
- `CHANGELOG`、贡献指南和安全策略；
- CODEOWNERS 与发布权限；
- 许可证和第三方声明。

仅当服务端形成独立部署、独立安全边界和独立发布周期后，再评估第三个
`novadraw-service` 仓库。不要为空的未来设想预建仓库。

## 8. 分阶段迁移

### 阶段 0：固定目标设计

当前阶段。

- 将本文件作为仓库拆分的目标设计；
- 当前仓库不再新增正式产品代码；
- examples 只验证通用框架能力；
- 新增仓库级调整时保持 Git 历史可追溯。

完成条件：README、仓库说明和后续决策不再把当前仓库描述为最终用户产品仓库。

### 阶段 1：建立 Organization 与框架仓库

- 创建或确认 GitHub Organization；
- 将当前仓库迁移或重命名为框架仓库，保留完整 Git 历史、Issue 和 tag；
- 审计并删除历史 `client/`、研究目录和失效工具；
- 评估把 Rust workspace 移到仓库根目录；
- 补齐 CI、贡献指南、安全策略和仓库元数据。

完成条件：从仓库根目录可以发现、构建、测试和阅读框架，不需要理解历史目录结构。

### 阶段 2：建立框架发布基线

- 为 workspace package 补齐 `license`、`repository`、`description` 和 `rust-version`；
- 定义 0.x 版本策略、tag 规则和 changelog；
- 建立格式化、Clippy、测试、文档和平台构建门禁；
- 保持公开 API snapshot 与迁移说明；
- 产出产品仓库可以固定引用的首个框架版本。

完成条件：外部仓库不使用本地 path dependency 即可重复构建一个最小应用。

### 阶段 3：创建产品仓库

- 创建 `novadraw-studio`，名称可在实施时调整；
- 选择产品许可、应用技术栈和发布目标；
- 使用框架 tag 或 commit 初始化依赖；
- 建立产品 Composition Root、领域模型、文档格式和端到端测试；
- 把产品需求通过公开扩展点接入，不复制 example 里的临时宿主结构。

完成条件：产品可以独立构建、发布和回退框架版本。

### 阶段 4：稳定跨仓库发布

- 校准框架兼容性和弃用周期；
- 自动化产品 dependency-bump 验证；
- 发布框架版本后再推进产品升级；
- 分别维护框架和产品发布说明；
- 根据真实外部消费者反馈继续收窄公共 API。

完成条件：两个仓库可以独立发布，且产品不依赖未公开的框架实现。

## 9. 产品仓库的创建门槛

出现以下任一条件时，应创建产品仓库，不再把代码留在 framework example 中：

- 开始维护用户文档格式或持久化兼容性；
- 开始制作安装包、应用壳或正式 Web 应用；
- 出现账号、同步、协作、模板或服务端需求；
- 产品需要独立版本、Issue、路线图或发布渠道；
- 产品许可或可见性可能与框架不同；
- 产品 UI 已经超出验证某个框架契约所需的最小范围。

API 尚处于 0.x 不是继续使用单仓库的理由。可以通过精确 commit、tag 和本地 patch
保持开发效率，同时保留真实的仓库边界。

## 10. 不变量

迁移和后续开发必须保持：

1. 框架仓库不依赖产品仓库。
2. 产品只依赖框架公开 API。
3. `novadraw-editor` 属于框架，不属于产品。
4. 产品需求只有具备通用消费者价值时才能进入框架。
5. 平台与渲染后端保持可替换，不因产品选型进入 Core。
6. 框架与产品分别版本化、验证和发布。
7. 仓库迁移保留 Git 历史，不通过复制源码创建“新起点”。
8. 新仓库数量由真实所有权和发布边界决定，不按目录数量决定。

## 11. 延后决策

以下事项在对应迁移阶段单独决定，不阻塞当前目标设计：

- GitHub Organization 的最终名称；
- 产品仓库的最终名称；
- 产品开源、闭源或商业许可策略；
- crates.io 发布范围；
- 桌面端、Web 端和服务端的具体技术栈；
- 是否需要独立服务端仓库。

## 12. 相关文档

- [仓库结构与线程边界](repository-layout.md)
- [工程文档入口](../engine/wasm_rust/novadraw/doc/00-index.md)
- [架构概览](../engine/wasm_rust/novadraw/doc/design/architecture/overview.md)
- [公共 API 契约](../engine/wasm_rust/novadraw/doc/design/architecture/public-api-contract.md)
- [贡献工作流](../engine/wasm_rust/novadraw/book/contributor/src/01-contribution-workflow.md)
