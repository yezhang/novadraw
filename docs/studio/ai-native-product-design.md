# `<prefix> Studio` AI-native 产品设计

类型：`product-design`

状态：`proposal`

归属：未来 Studio 产品仓库

日期：2026-10-20

## 1. 文档目的与边界

本文记录 `<prefix> Studio` 作为 AI-native 图形中台产品的候选设计，重点回答：

- 定性空间推理如何进入产品 DSL；
- AI 如何参与绘图、着色、布局、审查和长期维护；
- AI 候选如何经过确定性校验、人工批准和可回放事务进入业务事实；
- Studio 与 Novadraw Framework 如何保持产品和仓库边界。

本文不定义 Novadraw Core、Editor、Runtime 或渲染后端的公开 API，也不改变引擎 roadmap、
ADR 和验证状态。正式产品代码、产品协议和发布计划应进入独立 Studio 仓库。

## 2. 产品判断

Studio 不应被定义为“在画布旁增加聊天框”，也不应以一次性 prompt-to-image 或
prompt-to-diagram 为最终形态。推荐定位是：

> 面向专业关系模型的 AI-native 图形中台：AI 提出语义结构和受约束变更，确定性系统
> 负责校验、求解和执行，人类负责确认最终事实。

产品输出不只是图片，而是可继续编辑、保存、迁移、审查、回放和自动验证的关系模型。
其核心闭环为：

```text
自然语言 / 文档 / 数据 / 现有图
-> AI 生成带证据的语义候选
-> DSL parser 与 compiler 校验
-> 布局、路由和样式系统确定性求解
-> 模型 diff + 空间 diff + 视觉预览
-> 人工批准、拒绝、修改或 fork
-> typed Command 原子提交
-> 保存、回放、审计和自动验收
```

## 3. 核心不变量

### 3.1 双平面

Studio 必须分离两个平面：

- **语义控制平面**：LLM、检索、需求理解、候选结构、解释和修订建议；
- **确定性数据平面**：parser、类型系统、约束求解、Command、持久化、迁移和 replay。

语义控制平面可以扩大候选空间，但不能裁决确定性事实。LLM 输出只有经过编译、约束
校验和权限检查后，才可能成为可提交操作。

### 3.2 状态分域

产品至少维护以下独立状态域：

| 状态域 | 负责内容 | 历史与身份 |
|---|---|---|
| `EditorSpec` | 编辑器支持哪些实体、关系、视觉、工具和规则 | Spec ID、version、独立历史 |
| `DiagramDocument` | 用户正在维护的业务节点、连接、属性和稳定 ID | Document revision、业务历史 |
| `SpatialSpec` | 分组、方向、对齐、固定和布局偏好 | 随文档或视图方案版本化 |
| `WorkspaceState` | selection、focus、viewport、临时 feedback | 会话状态，不是业务事实 |
| `CommandPatch` | AI 或人工提出的候选变更 | proposal ID、前置 revision、审批记录 |

必须保持：

1. 修改 `EditorSpec` 不等于修改业务文档；
2. 修改业务文档不得隐式扩展编辑器能力；
3. `WorkspaceState` 不参与业务持久化和语义判断；
4. Spec history 与 Document history 不共享 undo/redo；
5. AI 操作必须显式声明目标状态域；
6. 只有稳定业务 ID 可以跨重建、迁移和回放保留身份；
7. Figure、运行时句柄和最终渲染命令不得进入产品持久事实。

### 3.3 证据与推断分离

节点、连接和领域约束需要记录来源：

- 外部系统给出的确定性事实；
- 用户明确创建或确认的事实；
- 规则引擎推导的确定性结果；
- AI 生成但尚未确认的候选；
- 无法验证、仅用于排查的语义建议。

AI 不能因为语言上“看起来合理”而把连接写成确定性事实。缺少来源的关系必须标记为
假设，不能与已验证关系共用同一种状态。

## 4. DSL 不是单一大语言

Studio 应提供一组共享类型系统和身份协议的 DSL/结构化协议，而不是不断扩张的单一
语法。推荐拆分为四类用户可见产物和两类内部产物。

### 4.1 用户可见产物

| 产物 | 职责 |
|---|---|
| `EditorSpec` | 定义领域 schema、notation、连接规则、工具、属性面板和验证 |
| `DiagramDocument` | 表达某一份图中的业务对象、端口、连接、属性和证据 |
| `SpatialSpec` | 表达人类可理解的空间关系与布局意图 |
| `CommandPatch` | 表达有前置条件、影响范围、证据和解释的候选变更 |

### 4.2 内部产物

| 产物 | 职责 |
|---|---|
| `SpatialIR` | 归一化定性关系、硬软约束、作用域、优先级和求解诊断 |
| `EditorPackage` | 已编译且可由 Web、Native 或 Headless 宿主加载的产品包 |

`EditorSpec` 是产品能力的单一事实源；`EditorPackage`、生成代码和运行时对象都是可重建
派生产物。

### 4.3 概念示例

以下语法只说明职责边界，不是已接受的文法：

```text
diagram order_lineage uses lineage@1 {
  node orders : table
  node clean  : transform
  node mart   : table

  connect orders.out -> clean.in
    evidence sql_ref("job.clean_orders")

  connect clean.out -> mart.in
    evidence sql_ref("job.build_mart")

  spatial {
    require orders left_of clean
    require clean left_of mart
    prefer [orders, clean, mart] align center_y
    prefer preserve_existing_positions
  }

  appearance {
    color by status using semantic_status
    edge_style by evidence_kind
  }
}
```

LLM 应优先生成这类稳定身份、关系和约束，而不是绝对坐标、任意颜色值或底层 Figure
操作。

## 5. 定性空间推理的定位

### 5.1 负责什么

定性空间推理（Qualitative Spatial Reasoning，QSR）适合作为 `SpatialSpec` 的关系层，
把“在左侧”“位于容器内”“相邻但不重叠”“保持同一层”等人类意图转换为可验证约束。

首版建议支持：

| 类别 | 候选关系 |
|---|---|
| 拓扑 | `inside`、`contains`、`touches`、`overlaps`、`disjoint` |
| 方向 | `left_of`、`right_of`、`above`、`below` |
| 顺序 | `before`、`after`、`same_layer` |
| 组织 | `align`、`distribute`、`near`、`far` |
| 图形 | `same_container`、`avoid_crossing`、`port_side` |
| 稳定性 | `pin`、`preserve_order`、`preserve_position` |

首版不追求完整复刻 RCC-8 或建立通用空间逻辑系统，只实现真实图形工作流需要且能够
稳定编译的关系。

### 5.2 不负责什么

QSR 不直接解决：

- 文字测量和节点最终尺寸；
- 边交叉最小化与大图分层；
- 端口分配、正交路由和折点；
- 像素级坐标、裁剪和渲染；
- 颜色对比度与主题适配；
- 业务连接是否合法。

这些职责分别由布局器、路由器、样式系统、领域约束和图形框架承担。QSR 是意图层，
不是渲染引擎或通用正确性证明器。

### 5.3 约束模型

每条空间约束至少包含：

```text
relation + subjects + scope + strength + priority + provenance
```

- `require` 表示硬约束，冲突时拒绝候选；
- `prefer` 表示软约束，可在布局质量、历史位置和局部稳定性之间权衡；
- `scope` 限定文档、容器、选区或局部子图；
- `provenance` 记录用户操作、模板规则、导入事实或 AI 建议；
- 求解必须使用稳定 seed，并返回可重现结果；
- 失败诊断应指出冲突关系和最小可解释冲突集，不能只返回“布局失败”。

用户拖拽后的结果需要显式转换为 `pin`、顺序或弱位置偏好。系统不能一边声称自动布局
仍然受控，一边把不可解释的绝对坐标静默写入文档。

### 5.4 编译链

```text
SpatialSpec
-> 名称与身份解析
-> 关系归一化
-> 拓扑与方向一致性检查
-> 硬约束 / 软目标分离
-> 布局引擎和路由引擎选择
-> 精确几何求解
-> 后置约束验证
-> SpatialDiff + diagnostics
```

布局实现必须通过可替换 `LayoutEngine` 协议接入。Studio 可以使用或适配成熟算法，但
不应让某个外部布局器的私有 option 成为产品 DSL 的公共语义。

## 6. 自动布局与自动着色

### 6.1 自动布局

AI 的职责是识别图的语义结构、选择布局策略和提出空间约束；确定性布局器负责最终
坐标。产品需要支持：

- 全图首次布局；
- 选区或受影响子图的局部布局；
- 已确认历史前缀和锁定区域不移动；
- 旧位置作为弱约束以保持 mental map；
- 新节点优先进入局部空位，避免无关节点大范围跳动；
- 约束冲突时先显示诊断，不静默丢弃用户锁定；
- 布局前后提供可审查的空间 diff。

### 6.2 自动着色

自动着色不属于 QSR，应由独立 `AppearancePolicy` 管理。颜色必须来源于明确语义角色：

- 类型或领域类别；
- 状态、风险或健康度；
- 来源和证据类别；
- 选择、焦点和变更审查状态；
- 用户主题和无障碍需求。

AI 可以建议“哪些语义值得编码”，但不能任意分配最终颜色。确定性系统负责受控 palette、
对比度、跨主题一致性、同类稳定映射和色觉缺陷兼容。颜色变化同样需要出现在视觉 diff
和审计记录中。

## 7. Agent 可参与的产品角色

以下是同一 Agent 工具面的职责角色，不要求首版部署多个自治 Agent：

| 角色 | 主要任务 | 输出 |
|---|---|---|
| Editor Architect | 从领域材料提取 schema、关系、规则和工具 | `EditorSpec` patch |
| Graph Author | 创建、补全和局部修改业务图 | `DiagramDocument` patch |
| Layout Planner | 识别主路径、分组和空间意图 | `SpatialSpec` patch |
| Style Director | 建议视觉编码和层级 | `AppearancePolicy` patch |
| Reviewer | 查找非法连接、孤立节点、循环、歧义和缺证关系 | diagnostics / review |
| Migration Planner | 分析 Spec 演进并提出文档迁移 | migration candidate |
| Explainer | 回答“为什么连接、放置或着色” | evidence-backed explanation |
| Test Author | 生成样例、反例、回放和 headless 断言 | verification scenarios |

Agent 还可以参与：

- 从 SQL、代码、配置、日志和文档持续同步关系候选；
- 识别重复子图并建议抽象、模板或折叠；
- 对大图做语义摘要、聚类和渐进披露；
- 根据用户目标生成多种布局方案供比较；
- 生成可访问名称、图说明和审查摘要；
- 在发布前检查 schema、迁移、权限和文档兼容性；
- 把已批准的工作流节点映射为外部工具调用。

## 8. AI-native 交互形态

Studio 建议提供三个明确隔离的模式：

| 模式 | 用户目标 | 可修改状态 |
|---|---|---|
| Define | 定义领域编辑器 | `EditorSpec` |
| Preview | 用隔离样例验证编辑器 | sample document |
| Use | 编辑真实业务文档 | production `DiagramDocument` |

AI 入口必须服从当前模式和选区作用域。核心交互包括：

1. **选区级自然语言编辑**：默认只修改选区及其显式依赖；
2. **历史前缀锁定**：进入因果传播时，已确认前缀保持只读；
3. **幽灵预览**：候选 Figure 位于 feedback 层，不提前成为业务事实；
4. **三联审查**：同时展示模型 diff、空间 diff 和最终视觉预览；
5. **逐项决策**：允许批准、拒绝、修改或拆分 patch；
6. **显式分叉**：不兼容 Spec 变化或历史修改默认创建 fork；
7. **来源面板**：显示关系、约束和视觉编码的证据、假设与操作者；
8. **Why 查询**：可追溯某个节点为何存在、为何连接、为何位于此处；
9. **统一撤销**：人工和 Agent 的已批准修改进入同一业务 Command 历史；
10. **可重放审计**：记录输入 revision、工具版本、候选、诊断和批准结果。

## 9. Agent 工具协议

Agent 不应获得“任意修改画布”或“返回一段可执行脚本”的宽接口。推荐按副作用分层：

### 9.1 读取

```text
read_capability_catalog
read_editor_spec
read_document_snapshot
read_selection_context
query_relationship_evidence
query_constraint_diagnostics
```

### 9.2 提案与验证

```text
propose_spec_patch
propose_document_patch
propose_spatial_patch
validate_candidate
compile_preview
run_headless_scenarios
explain_candidate
```

### 9.3 受控提交

```text
request_approval
apply_approved_patch
fork_editor_version
fork_document_revision
revert_command
```

激活产品版本、迁移生产文档、加载插件和执行外部工作流不能是默认自动权限。未来接入
MCP 时，MCP 只是工具发现和调用 adapter，不改变这些权限边界。

## 10. 产品架构

```text
领域材料 / 用户意图 / 外部数据
                 |
                 v
        语义控制平面
LLM / Retrieval / Agent workflow
                 |
                 v
EditorSpec / Document / SpatialSpec / CommandPatch candidates
                 |
                 v
        确定性数据平面
parser -> type check -> constraint -> compile -> replay
                 |
                 v
      Studio product model and history
                 |
                 v
 Novadraw public Editor / Runtime / Platform API
```

Studio 拥有上方所有产品状态和协议。Novadraw 只作为外部框架依赖，不感知 AI provider、
prompt、产品文档格式、审批 UI 或商业工作流。

## 11. 最小可验证产品

首个切片应选择两个语义差异明显的关系领域：

- 数据管道：source、transform、sink、typed ports 和方向约束；
- 状态机：状态、转换、起止规则、环路和可达性。

必须具备：

1. 版本化 `EditorSpec`、`DiagramDocument` 和稳定业务 ID；
2. 节点、端口、连接、容器和最小属性系统；
3. `SpatialSpec` 与首批定性空间关系；
4. 硬软约束、结构化诊断和确定性布局结果；
5. create、move、delete、connect、reconnect 和属性编辑；
6. `propose -> validate -> preview -> approve -> apply`；
7. 模型 diff、空间 diff 和视觉预览；
8. undo/redo、保存加载、不可变审计和 headless replay；
9. Web Preview 与至少一个真实宿主验收；
10. 两个领域共用同一 compiler 和运行时解释机制。

首版不包含：

- 任意脚本执行；
- 未经批准的生产修改和 schema migration；
- 通用白板与多人实时协作；
- 插件市场和在线依赖安装；
- 自主执行高风险外部工作流；
- 训练或托管自有基础模型。

## 12. 分阶段实施

### 阶段 A：手写规范

不接 LLM，先证明 DSL、compiler、布局协议和产品状态分域能够驱动两个领域。此阶段解决
确定性基础，不用 prompt 成功掩盖协议缺陷。

### 阶段 B：AI 候选生成

让模型只生成 schema-guided candidate，使用结构化 diagnostics 自动修订。验证自然
语言、文档和样例数据三种输入。

### 阶段 C：Studio 协作闭环

加入选区编辑、三联 diff、批准/拒绝、fork、Why 查询、证据面板和历史回放。

### 阶段 D：生产交付

补齐迁移、权限、私有模型、MCP adapter、领域 Kit、LTS、Native 交付和企业审计。

任何阶段发现框架缺口时，先在 Studio 记录真实用例、失败模型和预期公共契约。只有
证明具有通用消费者价值后，才在框架仓库单独设计和实现。

## 13. 验证指标

### 13.1 生成与审查

- 从领域输入到可审查第一版图或编辑器的时间；
- Spec 和 patch 首次编译通过率；
- diagnostics 收敛轮次；
- 直接批准、修改后批准和拒绝的比例；
- 缺少证据的 AI 关系被阻止进入业务事实的比例。

### 13.2 空间与视觉

- 硬约束满足率；
- 局部修改造成的无关节点位移量；
- 边交叉、标签遮挡和容器溢出数量；
- 用户手工调整后再次自动布局的保持率；
- 颜色对比度和跨主题语义一致率。

### 13.3 正确性

- undo/redo 后文档等价率；
- 保存、加载、迁移和重建一致率；
- headless replay 稳定率；
- Native/Web 业务结果一致率；
- proposal、批准和执行链的 provenance 覆盖率。

## 14. 反证条件

出现以下证据时应收缩或调整方向：

- 目标用户只需要一次性静态图，不需要长期编辑和审查；
- 领域关系没有约束，表单或表格比图更有效；
- DSL 维护成本接近或超过直接开发；
- 大部分需求都必须进入任意代码插件；
- 自动布局不能在真实大图中保持用户 mental map；
- Agent 错误主要来自上游事实缺失，产品无法通过证据和审查缓解；
- 用户不重视迁移、回放、私有部署或确定性验证。

## 15. 待决策问题

1. 产品和产品仓库的最终名称；
2. DSL 使用文本语法、JSON schema，还是共享 AST 的双表示；
3. `SpatialSpec` 哪些关系进入持久文档，哪些只属于视图方案；
4. 布局引擎协议、求解预算和跨平台确定性要求；
5. 手工拖拽转换为 pin、顺序或位置偏好的具体规则；
6. 样式策略与领域 Kit 的扩展边界；
7. Spec migration 的声明式能力和插件逃生口；
8. 证据模型、置信度和人工确认的状态机；
9. MCP、模型 provider 和本地推理的 adapter 边界；
10. 首个设计伙伴和真实付费场景。

## 16. 参考

项目内：

- [框架与产品的双仓库目标设计](../repository-strategy.md)
- [仓库结构与线程边界](../repository-layout.md)
- [Studio 产品设计入口](README.md)

外部：

- [A survey of qualitative spatial representations](https://doi.org/10.1017/S0269888913000350)
- [Penrose language overview](https://penrose.cs.cmu.edu/docs/ref)
- [ELK Layered](https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html)
- [Raiven: LLM-Based Visualization Authoring via Domain-Specific Language Mediation](https://arxiv.org/abs/2604.10008)
