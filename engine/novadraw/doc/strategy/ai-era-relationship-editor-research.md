# AI 时代的关系编辑器机会研究

类型：`product-strategy`

状态：`current`

调研日期：2026-09-17

## 1. 范围与判断方式

本文研究 AI 辅助创作和 Agent 工作流对专业图形编辑器的影响，并评估 Novadraw 的
可持续价值。它是产品和商业决策输入，不定义运行时架构、公开 API 或路线图状态。

本文首先回答“AI 如何参与一份关系图的编辑”。在此基础上，让 AI 进一步生成领域模型、
视觉映射、连接规则和工具，从而动态形成一套图形编辑器的候选产品设计，见
[AI 图形编辑器动态生成器产品设计](ai-graphical-editor-generator.md)。

文中严格区分：

- **外部事实**：公开产品、工程文章或研究论文已经陈述的能力与观察；
- **项目事实**：仓库文档和可执行验证已经证明的 Novadraw 能力；
- **战略判断**：基于前两者给出的定位与取舍；
- **待验证假设**：需要设计伙伴、原型或付费试点才能证实的判断。

“AI 时代”不自动证明市场需求。它只改变了用户创建、修改和审查图模型的成本结构；
目标领域是否愿意为可靠性、私有部署或可审计编辑付费，仍须由客户证据证明。

## 2. 外部市场信号

### 2.1 从一次性生成转向可编辑、可审查的图

Figma 在 2025 年发布 Figma Make，将自然语言或现有设计转为原型或应用；随后提出
“第一个 prompt 是创作起点而非终点”，把 AI 输出视为需要继续编辑的媒介
([Config 2025](https://www.figma.com/blog/config-2025-press-release/)，
[Figma Weave](https://www.figma.com/blog/welcome-weavy-to-figma/))。

Mermaid AI 把文档、PRD、会议记录或自然语言转为图，并支持在 Visual Editor 中继续
编辑。它在应用前展示 AI 提出的 diff，而不是静默改图
([产品页](https://mermaid.ai/products/ai/)，
[AI 编辑说明](https://mermaid.ai/docs/build-and-edit/add-and-edit-with-ai))。

这两个产品事实支持的结论是：

> “文本直接产出最终图像”不是稳定工作流；更合理的产品形态是 AI 产出第一版结构，
> 人类在可编辑表面中检查、修正和继续演化。

它们不证明 Novadraw 可以与 Figma 或 Mermaid 竞争，也不证明通用白板是 Novadraw 的
目标市场。

### 2.2 Agent 需要清晰工具、反馈和评估

Anthropic 对生产 Agent 的公开工程建议强调：对定义明确的任务采用可预测 workflow，
只在确有需要时引入自主 agent；工具应边界清晰、返回有意义的上下文，并通过评估持续
改进 ([Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)，
[Writing effective tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents))。

这意味着一个面向 Agent 的编辑器不应把“修改任意图形树”暴露为唯一写接口。较好的
工具面应把读取、提案、验证、提交和回放分开，允许模型探索，但把不变量的判断留给
确定性系统。

### 2.3 研究信号：人类需要检查和改写 Agent 计划

AIPOM 将 LLM 生成的多 Agent 计划表示为可直接操作的图，使用户能检查节点、agent
分配、数据流和输出；作者将透明度、可控性和人工监督视为关键问题
([AIPOM, 2025](https://arxiv.org/html/2509.24826v1))。

这是一项研究原型，而非市场规模证据，但它与上述产品趋势一致：复杂 Agent 的价值不在
于把执行隐藏起来，而在于让用户能够理解、约束和修订计划。

### 2.4 MCP 将图形系统变成 Agent 可调用工具的候选

MCP 的目标是以统一协议连接 AI 助手和外部数据、工具或开发环境
([Anthropic MCP 公告](https://www.anthropic.com/news/model-context-protocol))。Mermaid
已通过 MCP 提供图的创建、校验和渲染，说明“图形系统作为 Agent 工具”已有明确产品
方向 ([Mermaid MCP](https://mermaid.ai/blog/posts/mermaid-feature-drop-building-diagramming-momentum))。

这不要求 Novadraw 立即实现 MCP；它表明若要进入 AI 工具链，系统需要可描述、可校验、
有最小副作用的操作边界。

## 3. 市场空白不在生成，而在受约束编辑

通用生成产品已经能快速生成流程图、白板、设计稿或静态矢量内容。其主要优势是低冷启动
成本，主要限制是当图成为业务事实时，图的关系、历史、权限和行为需要可靠维护。

| 层级 | 用户要解决的问题 | 常见 AI 方案的充分性 | Novadraw 应承担的价值 |
|---|---|---|---|
| 灵感与沟通 | 快速画出一个可讨论的草图 | 通常充分 | 不作为首要竞争方向 |
| 文档图 | 从文本、代码或 PRD 生成说明图 | Mermaid 类方案通常充分 | 可作为导入或审查入口 |
| 专业关系图 | 节点、端口、连接、约束与状态必须一致 | 常常不足 | 图模型的编辑、约束和几何运行时 |
| 可执行工作流 | 修改图会影响审批、数据流、工具调用或设备配置 | 生成文本不足 | 事务、验证、撤销、审计和领域规则 |
| 高风险场景 | 需要私有部署、离线、跨端一致和可重放验收 | 取决于产品实现 | Native/Web/headless 共核与确定性验证 |

战略判断：

> Novadraw 不应以“让 AI 生成更多节点”定义自身，而应以“让 AI 对受约束关系模型提出
> 可验证变更”定义自身。

## 4. Novadraw 已有的差异化资产

### 4.1 模型变更与视觉变更分离

Editor 架构规定模型是持久化事实源，Figure 是可重建视图；`Command` 修改模型，
`EditPart` 和 Viewer 负责把模型投影为 Figure
([Editor 架构](../design/editor/architecture.md))。这为 AI 提供了正确的写入位置：
Agent 应生成或选择可执行的模型 Command，而不是直接改 Figure、Canvas 或像素。

这带来四项潜在客户价值：

1. 变更可以在提交前检查领域约束；
2. 成功提交可由模型通知重建视觉，而非保留临时 UI 状态；
3. 同一个 CommandStack 可支持人工和 Agent 的 undo/redo；
4. UI 重建、跨端运行或远程审查不会把 Figure identity 误当持久身份。

### 4.2 几何、输入和反馈共享同一坐标协议

在专业编辑器里，Agent 提案不只改变数据，还需要在缩放、滚动、拖动和重连中维持
视觉反馈与模型坐标一致。Novadraw 将 Figure 树、ViewPort、RangeModel、命中、事件
点降域、Connection routing 与 damage 放在同一坐标链中
([坐标契约](../design/coordinates/coordinate-system.md))。

G5.5 已有的自动门禁证明：在 scroll/zoom、窗口 resize 和 auto-expose 后，feedback、
target 与 connection 预览重新投影；这是一项可复用的编辑器基础，而不是单一 demo
效果 ([G5.5 行为验证](../verification/reviews/g5-viewport-autoexpose-behavior-2026-09-15.md))。

### 4.3 确定性运行时可成为 Agent 的验证器

Draw2D Core 已验证 Native、Web 和 headless 的共同核心，以及 validation、damage、
事件、滚动缩放与 Connection 的自动和人工证据
([Core 1.0 最终审计](../verification/reviews/draw2d-core-1.0-final-audit-2026-09-13.md))。

这使 Novadraw 有机会提供比“Agent 输出一张图”更强的闭环：

```text
Agent 读取图模型
-> 提出 typed Command 序列
-> dry-run 验证关系、几何和领域约束
-> 生成模型 diff 与视觉预览
-> 人工批准
-> 原子提交
-> headless replay、断言和审计记录
```

其中 Agent 负责候选生成，确定性 Runtime 负责执行边界与验证。该分工符合外部 Agent
工程对清晰工具契约和可评估行为的建议。

### 4.4 Connection 是 AI 工作流的真实难点

AI workflow、数据血缘、软件依赖或工业拓扑的业务含义主要存在于连接，而不只在节点。
Novadraw 的 Anchor、Router、Connection Runtime、创建、重连、bendpoint 与 viewport
协作，覆盖了关系图在移动、缩放、删除和直接操作下的几何生命周期。

这类能力比“矩形加连线”更难被通用生成工具替代，因为客户还需要：

- typed port 和 source/target 兼容性；
- 环路、基数、安全域和执行顺序约束；
- 创建、预览、批准、重连和删除的事务语义；
- 稳定的 visual feedback 与可撤销历史；
- 大图中的布局、路由和局部更新。

## 5. 推荐定位与首批场景

### 5.1 推荐定位

> **面向专业关系图的 AI 协作编辑运行时：AI 提出模型变更，人类直接修订，系统验证并
> 重放结果。**

这比“Rust 绘图引擎”更接近客户购买的结果，也比“通用 AI 白板”更准确地利用当前技术
资产。

该定位还可以向上形成“AI 动态生成领域图形编辑器”的产品层：AI 生成版本化
`EditorSpec`，确定性编译器将其装配为基于 Novadraw 的编辑器。两者并不冲突：
前者约束 AI 如何修改业务图，后者约束 AI 如何定义用于修改业务图的编辑器。

### 5.2 优先场景

| 场景 | AI 的作用 | 人工必须保留的决定 | 与 Novadraw 的匹配 |
|---|---|---|---|
| Agent/tool workflow | 从目标生成候选步骤和连接 | 权限、工具选择、失败与补偿策略 | 高 |
| 数据血缘与管道 | 从 schema、SQL 或日志生成候选依赖 | 合规边界、业务定义和发布 | 高 |
| 软件架构与依赖 | 从代码或部署配置生成/更新图 | 系统边界、所有权和演进决策 | 高 |
| 工业或设备拓扑 | 从配置生成初始拓扑 | 安全规则、物理连接和现场批准 | 高 |
| 通用白板 | 生成草图、摘要或会议图 | 低 | 低，不宜首发 |

这些是市场假设，不是项目已经具备的领域套件。每个方向都要以真实文档模型、连接约束
和设计伙伴验证，而不是仅演示 prompt-to-diagram。

## 6. 需要补齐的产品能力

下表不是对现有 roadmap 的修改，而是 AI 产品化的依赖分析。

| 优先级 | 能力 | 原因 | 当前边界 |
|---|---|---|---|
| P0 | 持久化文档、稳定业务身份、版本迁移 | Agent、用户和 CI 必须指向同一可重放事实 | 已移交独立产品包，尚无产品协议 |
| P0 | 受限 Agent mutation API | 防止 Agent 绕过 Command/Runtime 写入临时 Figure 状态 | 尚未定义 |
| P0 | dry-run、语义诊断与批准前 diff | 使 AI 变更可检查、可拒绝、可解释 | 可复用现有事务，但尚无产品接口 |
| P1 | 领域约束 capability | 端口类型、环路、权限和拓扑规则不能由 prompt 保证 | 需要垂直领域设计 |
| P1 | 模型 diff + 视觉 diff | 同时审查业务影响和最终空间结果 | 需要产品层设计 |
| P1 | 审计、provenance 与分支历史 | 区分人和 Agent 的贡献，支持回放、批准和历史修订 | 需与保存加载一起设计 |
| P2 | MCP 或其他 Agent tool adapter | 让外部 Agent 能发现并调用受限操作 | 协议选择取决于目标客户 |

最重要的约束是：不得为了 Agent 便利而允许它直接写 `FigureTree`、`Runtime` 私有状态
或渲染命令。AI 的便利接口必须收敛到持久模型与受验证 Command。

## 7. 最小可验证产品切片

首个 AI 产品原型应选一个强连接语义场景，例如 Agent workflow 或数据管道，并只实现：

1. 可保存、可加载的领域模型和稳定 ID；
2. 三到五种 typed 节点、端口和连接规则；
3. Agent 可读取的紧凑 graph snapshot；
4. `propose -> validate -> preview -> approve -> apply` 操作链；
5. CommandStack undo/redo 和不可变审计记录；
6. headless replay 与至少一项领域约束断言；
7. Native/Web 同一份结果的人工验收。

不应以训练自有模型、自然语言无限自由编辑、协作系统或通用白板作为首个前置条件。

## 8. 验证指标与反证条件

### 8.1 应测量的指标

- 从领域输入到可审查第一版图的时间；
- 提案被直接批准、人工修改后批准和拒绝的比例；
- 每份提案违反约束的数量与类别；
- 用户定位并修正错误连接的时间；
- replay、headless 验证和 Native/Web 结果一致率；
- Agent 操作是否能在不查看渲染实现细节时完成；
- 设计伙伴是否愿意为私有部署、验证或领域套件付费。

### 8.2 会推翻定位的证据

下列结果应使项目收缩或调整此方向：

- 目标用户只需要静态文档图，且 Mermaid/现有 DSL 已满足需求；
- 用户不需要人工审查或可撤销变更；
- 领域模型没有连接约束，普通表单或表格比图更有效；
- Agent 提案的主要失败来自上游数据缺失，而不是图模型/交互问题；
- 客户不重视跨 Native/Web/headless 一致性，也无私有部署需求；
- 设计伙伴不愿为可靠编辑、验证或支持付费。

## 9. 结论

AI 降低了图的初始创建成本，却提高了“如何审查、约束、修改和证明 AI 变更正确”的
价值。Novadraw 最值得建设的不是一次性图片或图文生成器，而是使 AI 输出成为长期
可维护业务图的执行与验证底座；在该底座之上，可以进一步生成可版本化、可验证的领域
图形编辑器。

其长期公式可表达为：

```text
AI 协作价值
= 结构化领域模型
× 受约束关系编辑
× 可撤销事务
× 人类直接审查
× 可重复验证
```

任一因子缺失，产品都会退化为普通绘图或一次性生成工具。

## 10. 资料

项目内：

- [商业价值分析](commercial-value-analysis.md)
- [AI 图形编辑器动态生成器产品设计](ai-graphical-editor-generator.md)
- [Editor 架构](../design/editor/architecture.md)
- [Editor 路线图](../roadmap/editor/00-index.md)
- [Draw2D Core 1.0 最终审计](../verification/reviews/draw2d-core-1.0-final-audit-2026-09-13.md)
- [G5.5 Viewport/Auto-expose 验证](../verification/reviews/g5-viewport-autoexpose-behavior-2026-09-15.md)

外部：

- [Figma Config 2025](https://www.figma.com/blog/config-2025-press-release/)
- [Figma Weave](https://www.figma.com/blog/welcome-weavy-to-figma/)
- [Mermaid AI](https://mermaid.ai/products/ai/)
- [Mermaid AI 编辑与 diff](https://mermaid.ai/docs/build-and-edit/add-and-edit-with-ai)
- [Anthropic: Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- [Anthropic: Writing effective tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents)
- [Anthropic: Model Context Protocol](https://www.anthropic.com/news/model-context-protocol)
- [AIPOM: Agent-aware Interactive Planning](https://arxiv.org/html/2509.24826v1)
