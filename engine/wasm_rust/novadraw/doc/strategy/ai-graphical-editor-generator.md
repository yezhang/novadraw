# AI 图形编辑器动态生成器产品设计

类型：`product-strategy`

状态：`proposal`

日期：2026-09-29

## 1. 文档目的

本文定义一个建立在 Novadraw 之上的产品方向：用户通过自然语言、领域文档、样例数据
和直接操作，动态生成一套领域专用图形编辑器。

本文是产品设计与商业验证输入，不定义 Novadraw Core 或 Editor 的公开 API，也不改变
现有 M1-M10、G0-G5 或 P2 delta 的状态。产品 schema、serializer、AI adapter、
宿主界面和交付场景属于独立产品包；只有经过原型验证且确需下沉的通用能力，才通过
`design/` 或 ADR 进入引擎。

相关背景：

- [Novadraw 商业价值分析](commercial-value-analysis.md)
- [AI 时代的关系编辑器机会研究](ai-era-relationship-editor-research.md)
- [Editor 框架架构](../design/editor/architecture.md)
- [独立 Editor 框架边界](../adr/adr-015-editor-framework-boundary.md)
- [Core 公开 API 边界](../adr/adr-017-core-public-api-boundary.md)
- [可组装 API 与 Scoped Editor](../adr/adr-019-composable-api-and-scoped-editors.md)

## 2. 产品愿景

候选产品名：**Novadraw Studio**。

产品目标不是让 AI 在固定画布里生成一张图，而是：

> 把领域意图编译成可运行、可编辑、可保存、可验证的专业图形编辑器。

用户可以描述：

> 生成一套设备拓扑编辑器。设备分为网关、传感器和执行器；传感器只能连接到网关；
> 网关具有连接数限制；节点状态影响外观；支持创建、移动、重连、属性编辑、撤销和
> JSON 保存。

系统输出的不是设备拓扑截图，也不只是一份初始拓扑数据，而是一套后续可用于创建和
维护任意设备拓扑文档的编辑器。

产品价值公式为：

```text
编辑器生成价值
= 领域意图理解
× 可版本化编辑器规范
× 确定性规范编译
× 专业关系编辑运行时
× 可验证交付
```

## 3. 市场位置

### 3.1 相邻产品方向

当前市场已出现三个相邻方向：

1. **Prompt-to-App**：Figma Make 等产品从自然语言生成可编辑界面、交互和代码。
2. **Generative UI**：A2UI、assistant-ui 等方案允许模型从受控组件目录中组合界面。
3. **模型驱动编辑器生成**：Eclipse Sirius 等工具通过领域元模型、视觉映射和工具定义，
   动态生成领域建模工作台。

这些方向共同证明：

- 用户希望从意图直接得到可操作产品，而不是只得到文本建议；
- 生成结果必须能继续直接编辑；
- 组件目录和 schema 比任意 UI 代码更适合稳定生成；
- 领域模型、图形表示和编辑工具可以通过独立规范建立映射；
- Prompt 只是起点，版本、修订和交付能力决定长期价值。

### 3.2 市场空白

通用 AI App Builder 主要生成 Web 应用，通用 Diagram AI 主要生成图，传统建模工作台
需要专家手工维护复杂规范。Novadraw 的候选位置是三者之间尚未充分覆盖的部分：

> 面向专业关系模型的 AI-native graphical editor workbench。

其差异不在于比通用模型生成更多代码，而在于：

- 生成结果具有稳定领域 schema，而不是临时组件树；
- 节点、端口、连接和编辑行为受明确规则约束；
- 人工操作和 AI 修改进入同一可撤销事务体系；
- 编辑器可在 Native、Web 和 Headless 中使用同一核心；
- 生成结果可以重建、迁移、回放和自动验收。

### 3.3 不应进入的竞争范围

本产品不以以下方向作为首发目标：

- 通用网站或全栈应用生成；
- Figma 类通用界面设计；
- Miro 类多人白板；
- 任意 React、Rust 或脚本代码即时执行平台；
- 完整 CAD、三维建模或物理约束求解；
- 训练或托管自有基础模型。

## 4. 目标用户与核心任务

### 4.1 目标用户

第一层用户是需要交付领域图形工具的开发团队：

- 工业软件和设备管理团队；
- 数据血缘、可观测性和系统拓扑产品团队；
- AI Workflow、Agent Builder 和开发者工具团队；
- 游戏行为树、状态机、材质和音频图工具团队；
- 使用 Rust、Tauri、WASM 或需要 Native/Web 共核的产品团队。

第二层用户是领域专家。他们不必理解 Figure、EditPart 或 CommandStack，但需要能够
确认领域概念、关系、约束、属性和工作流。

### 4.2 核心任务

产品必须帮助用户完成：

1. 从自然语言、文档或样例数据得到第一版领域编辑器；
2. 直接修改节点类型、连接规则、视觉和工具；
3. 使用样例文档实时预览编辑器行为；
4. 比较并批准 AI 提出的规范变更；
5. 发布稳定编辑器版本；
6. 使用生成的编辑器创建和维护真实业务文档；
7. 对编辑器版本和业务文档分别保存、迁移、回放和审计。

## 5. 三类事实必须分离

产品必须维护三个独立状态域：

| 状态域 | 含义 | 身份与历史 |
|---|---|---|
| `EditorSpec` | 编辑器是什么 | Spec ID、Spec revision、独立历史 |
| `DocumentModel` | 用户用编辑器创建的业务事实 | 稳定 Model ID、文档 revision、CommandStack |
| `WorkspaceState` | 当前会话如何查看和操作 | selection、focus、viewport、临时 feedback |

核心不变量：

1. 修改 `EditorSpec` 不等于修改当前业务文档；
2. 修改 `DocumentModel` 不得隐式扩展编辑器能力；
3. `WorkspaceState` 不参与持久业务语义；
4. Spec history 与 Document history 不共享 undo/redo；
5. Spec 切换后允许重建 EditPart 和 Figure，不保留旧运行时身份；
6. 只有持久 Model ID 可以跨编辑器重建保持业务身份；
7. AI 操作必须显式声明目标状态域。

建议的 AI 操作分类：

```text
modify_editor_spec
modify_sample_document
modify_production_document
```

例如“增加一种数据库节点”属于 `modify_editor_spec`，“在当前图中增加订单数据库”属于
`modify_production_document`。系统不能仅凭文字相似性混淆两者。

## 6. EditorSpec 产品模型

`EditorSpec` 是产品层的版本化 SSOT。它是概念名称，不代表本文已经接受具体 Rust API
或序列化格式。

```text
EditorSpec
├── metadata
│   ├── spec_id / version
│   ├── compatibility
│   └── required_capabilities
├── model
│   ├── entity types
│   ├── fields and value types
│   ├── containment
│   └── references
├── notation
│   ├── node and container templates
│   ├── labels and ports
│   ├── conditional styles
│   └── layers and filters
├── connections
│   ├── source / target compatibility
│   ├── direction and cardinality
│   ├── cycle and domain constraints
│   └── anchor / router / decoration
├── tools
│   ├── create / delete
│   ├── move / resize
│   ├── connect / reconnect
│   └── direct edit and domain actions
├── inspector
│   ├── property groups
│   ├── editors and choices
│   └── validation messages
├── persistence
│   ├── document schema version
│   ├── defaults
│   └── migration declarations
└── verification
    ├── generated examples
    ├── behavior scenarios
    └── headless assertions
```

### 6.1 受控目录

AI 不得发明引擎中不存在的类型名或能力。生成上下文必须包含可机器读取的目录：

- Figure template；
- Layout、Router、Anchor 和 Border；
- Tool、Request 和 Policy capability；
- 属性编辑器；
- Host panel；
- 导入导出 adapter；
- 可选插件。

目录项必须包含稳定 ID、参数 schema、适用范围、失败模式和兼容版本。AI 只负责选择和
组合，规范编译器负责验证真实可用性。

### 6.2 受限表达式

条件样式、派生标签、连接约束和属性可见性需要受限表达式语言，但不能使用任意脚本作为
默认机制。首版表达力应限制为：

- 读取 schema 中声明的字段和关系；
- 比较、布尔组合、枚举和集合判断；
- 有预算的图查询；
- 纯函数式值转换；
- 无网络、文件、时钟和隐式全局状态。

超出表达式能力的需求进入显式插件边界，而不是偷偷扩展 DSL。

## 7. 生成与运行闭环

```text
领域描述 / 文档 / 样例数据
        |
        v
AI Editor Architect
        |
        v
EditorSpec candidate
        |
        v
parse -> resolve catalog -> validate -> compile -> generate tests
        |
        +---- diagnostics ----> AI / human revision
        |
        v
EditorPackage
        |
        v
Novadraw Editor + Runtime
        |
        v
Web / Native / Headless generated editor
```

### 7.1 AI 的职责

- 从领域材料识别候选实体、属性和关系；
- 选择已有模板、策略和工具；
- 生成完整 `EditorSpec` 候选；
- 根据结构化诊断修订候选；
- 生成样例文档和验收场景；
- 解释每项规则来自何处；
- 在领域语义不明确时请求确认。

### 7.2 确定性系统的职责

- 解析和版本校验；
- capability 与 catalog resolution；
- 类型、引用、基数和连接约束校验；
- 表达式预算和插件权限校验；
- Spec diff 和迁移影响分析；
- `EditorPackage` 装配；
- Native/Web/Headless 一致性验证；
- 激活、回滚和审计。

LLM 输出不能代替以上判断。

## 8. EditorPackage 与 Novadraw 映射

规范编译器将 `EditorSpec` 转换为可运行但仍不包含活 Runtime 身份的
`EditorPackage`。概念映射如下：

| 产品定义 | Novadraw 承载点 |
|---|---|
| entity / containment | 应用 `DocumentModel` + `ModelAdapter` |
| model-to-visual mapping | `PartFactory` / `EditPartBehavior` |
| node/container notation | Figure template / content pane |
| relation notation | ConnectionPart / Anchor / Router |
| edit tool | Tool / typed Request |
| domain rule | EditPolicy / Command preparation |
| transaction | Command / CommandStack |
| preview and handles | feedback / handle layer |
| rendering and input | Figure Runtime |
| verification | headless replay / suite |

产品层可以提供通用的 spec-driven 实现，例如：

```text
DynamicDocumentModel
SpecDrivenModelAdapter
SpecDrivenPartFactory
DeclarativeEditPolicy
SchemaCommandFactory
FigureTemplateFactory
GeneratedInspectorDefinition
GeneratedContractSuite
```

这些名称表示职责，不是已接受的 crate 或公开类型。

## 9. 运行时解释、代码生成与插件

### 9.1 首选：解释 EditorPackage

首版应由稳定运行时解释已编译的 `EditorPackage`：

- 生成后立即预览；
- 同一 package 可被 Web、Native 和 Headless 加载；
- 规范修改不需要重新编译整个 Rust workspace；
- capability 和权限可以集中控制；
- 部署产物保持可重建。

### 9.2 可选：生成静态部署产物

稳定 Spec 可以导出为嵌入应用的静态资源或预编译 package。即使生成 Rust/TypeScript
胶水代码，该代码也只是可丢弃的派生产物，不得取代 `EditorSpec`。

### 9.3 逃生口：沙箱插件

无法由目录和受限表达式表达的需求，可以进入版本化插件：

- 插件显式声明输入、输出、权限和失效类别；
- 插件不能获得任意 `&mut Runtime` 或 `&mut FigureTree`；
- 更新继续遵循 prepared update、结构化错误和 fault boundary；
- Web 目标优先考虑受限 Wasm 插件；
- Native 插件的加载、签名和 ABI 需要独立设计；
- AI 可以生成插件草案，但不能未经构建、测试和批准直接加载。

## 10. 产品交互

### 10.1 三种工作模式

| 模式 | 用户目标 | 可修改状态 |
|---|---|---|
| Define | 定义编辑器 | `EditorSpec` |
| Preview | 用样例验证编辑器 | 独立 sample document |
| Use | 编辑真实业务文档 | production `DocumentModel` |

三个模式必须在视觉和操作权限上明确区分。Preview 不能误写生产文档，Use 中的普通
节点操作不能隐式修改 Spec。

### 10.2 Spec 修改流程

```text
当前 Spec revision
-> AI 或人工提出 patch
-> 展示结构化 diff
-> 编译候选 EditorPackage
-> 在 sample document 中预览
-> 运行自动场景
-> approve / reject / fork
-> 激活新 Spec version
```

编辑器定义发生不兼容变化时，默认创建 fork，不覆盖历史版本。

### 10.3 热更新边界

不得把新 Spec 直接热补丁到活动 PartTree/FigureTree。规范切换应：

1. 保存当前业务文档；
2. 预验证 schema compatibility；
3. 必要时执行显式模型迁移；
4. 由新 `EditorPackage` 重建 EditorDomain；
5. 保留持久 Model ID；
6. 生成新的 EditPartId 和 FigureId；
7. 恢复允许迁移的 WorkspaceState。

这符合模型是事实源、Figure 是可重建视图的现有边界。

## 11. Host UI 边界

Novadraw 是专业二维画布和编辑框架，不应因此扩张为通用 GUI 框架。

产品可以生成属性面板、palette、菜单和对话框的 schema，但实际控件由宿主 adapter
渲染：

- Web 可以使用现有 Web UI 技术；
- Native 可以使用选定的桌面宿主工具包；
- Headless 只消费 schema、command 和验证结果；
- Figure-native widget 只用于必须位于画布坐标域中的交互。

Host action 最终仍转换为 typed Request、Command 或产品层 Spec operation，不能绕过
Editor/Runtime 修改状态。

## 12. AI 安全、证据与治理

### 12.1 双平面

产品采用双平面：

- **语义生成平面**：LLM、检索、需求理解和候选 Spec；
- **确定性执行平面**：parser、compiler、constraint、Command、Runtime 和 replay。

语义生成平面的输出始终是待验证候选，不能成为确定性事实的裁判。

### 12.2 证据

每个 AI 生成的关键定义应记录：

- 来源文档、样例或用户确认；
- 生成模型和版本；
- prompt/template revision；
- 生成时间和操作者；
- 编译诊断；
- 人工修订与批准记录。

领域材料没有支持的连接规则、基数或权限要求必须标记为假设，不能伪装成已确认事实。

### 12.3 权限

AI adapter 按最小能力暴露操作：

```text
read_catalog
read_editor_spec
propose_spec_patch
compile_candidate
run_preview_tests
request_activation
```

`activate_editor_version`、生产文档迁移和插件加载不应是默认自动权限。

## 13. 最小可验证产品切片

首个切片的目标是证明“同一个生成内核可以生成不同领域的可用编辑器”，而不是做完整
低代码平台。

### 13.1 必需能力

1. 版本化 `EditorSpec` 和稳定 Spec identity；
2. entity、field、containment、reference；
3. node、container、port 和 connection 定义；
4. 受控 Figure、Layout、Anchor 和 Router 目录；
5. create、move、delete、connect 和 reconnect；
6. 自动属性面板 schema；
7. CommandStack undo/redo；
8. 文档保存、加载和版本标记；
9. Spec parse、validate、compile 和结构化诊断；
10. Web Preview 与 Headless 行为门禁；
11. Spec diff、approve、reject 和 fork；
12. AI 从领域描述生成和修订候选 Spec。

### 13.2 双领域验收

至少使用两个语义差异明显的编辑器验证通用性：

- 状态机编辑器：状态、转换、起止约束、环路；
- 数据管道编辑器：source、transform、sink、typed ports 和方向约束。

如果两者需要复制两套专用生成逻辑，说明 `EditorSpec` 或 catalog 抽象尚未成立。

### 13.3 暂不包含

- 实时多人协作；
- 任意脚本和在线依赖安装；
- 自动生成完整宿主应用；
- 通用数据库和后端生成；
- 插件市场；
- 全自动生产发布；
- 未经批准的 schema migration；
- 自主执行生产工作流。

## 14. 产品价值与商业化

### 14.1 客户价值

| 价值 | 可观测结果 |
|---|---|
| 降低首个编辑器成本 | 从领域描述到可操作原型的时间 |
| 降低专业图形缺陷 | 连接、坐标、撤销和生命周期回归数量 |
| 让领域专家直接参与 | Spec 中由领域专家确认的规则比例 |
| 降低跨平台维护 | Web/Native 行为差异和重复代码量 |
| 提升 AI 生成可信度 | 可编译、可回放、可审计的生成结果比例 |
| 形成长期复用 | catalog、模板和领域套件复用次数 |

### 14.2 商业产品层

候选商业分层：

1. **Novadraw Engine**：通用 Figure、Runtime 和 Editor 框架；
2. **Novadraw Studio**：EditorSpec、编译器、预览、diff 和 AI 协作；
3. **Enterprise Runtime**：私有部署、LTS、权限、审计和插件治理；
4. **Domain Kits**：数据血缘、工业拓扑、Agent Workflow、状态机等领域目录。

客户购买的不是一次 Prompt，而是缩短专业编辑器交付时间、降低长期维护风险并获得
可验证跨平台运行时。

### 14.3 可持续差异

- Draw2D 级 Figure、坐标、更新和 Connection 语义；
- GEF 式模型投影和编辑事务；
- Spec 与业务文档双重版本化；
- Native、Web、Headless 同核；
- AI 输出经过确定性 compiler，而不是直接执行；
- 领域 catalog、迁移和自动验证积累；
- Rust/WASM、私有部署和长生命周期支持。

## 15. 验证指标

产品指标：

- 从需求输入到首个可操作编辑器的时间；
- 首次生成后需要人工修改的 Spec 项数量；
- Spec 首次编译通过率；
- AI 修订后诊断收敛轮次；
- 直接批准、修改后批准和拒绝的比例；
- 用户无需编写插件即可完成的需求比例；
- 从 Preview 到稳定发布的时间。

正确性指标：

- 非法连接、基数和 schema 引用的拒绝率；
- undo/redo 后文档等价率；
- 保存、加载和重建一致率；
- schema migration 成功率与失败可恢复性；
- Headless replay 稳定率；
- Web/Native 几何和行为一致性；
- Spec 激活失败时旧版本保持可用的比例。

商业指标：

- 外部团队从 Prompt 到领域 PoC 的时间；
- 设计伙伴持续迭代 Spec 的频率；
- 客户自研图形基础设施减少量；
- 对私有部署、LTS、领域 kit 和插件支持的付费意愿；
- 生成编辑器进入真实业务流程的比例。

## 16. 反证条件与主要风险

### 16.1 反证条件

出现以下证据时应收缩或调整方向：

- 用户只需要生成一张图，不需要可复用编辑器；
- 目标编辑器主要是表单和列表，图形关系不是核心；
- `EditorSpec` 的学习和维护成本接近手写代码；
- 大部分真实需求都必须进入任意代码插件；
- 用户不需要 Native、Headless、离线或确定性验证；
- 设计伙伴不愿为生成后的维护、迁移和支持付费。

### 16.2 主要风险

**DSL 膨胀**：不断加入例外会使 Spec 比代码更复杂。解决方式是保持有限核心、以真实
场景扩展，并给复杂需求明确插件出口。

**生成结果同质化**：只有少量模板会产生低质量编辑器。需要高质量 catalog、领域 kit
和允许人工直接修改的 Studio。

**Schema 演进**：生成第一版容易，已有文档后的兼容修改困难。版本、migration、fork
和回滚必须从第一版进入模型。

**宿主能力错位**：属性表单、菜单、认证和数据接入不全属于 Novadraw。必须保持
Host adapter 边界。

**任意代码风险**：AI 生成插件可能带来安全、供应链和运行时 fault。首版不得以动态
执行任意代码作为卖点。

**AI 幻觉**：模型可能发明不存在的 capability 或领域规则。机器可读 catalog、compiler
诊断、证据记录和人工批准缺一不可。

## 17. 分阶段验证建议

### 阶段 A：手写 Spec 驱动

先不接 LLM，证明一个稳定 `EditorSpec` 可以生成两个不同领域的编辑器，并通过 Web 与
Headless 验证。

### 阶段 B：AI 生成 Spec

让模型只生成候选 Spec，通过 compiler diagnostics 自动修订。验证 Prompt、文档和
样例数据三种输入。

### 阶段 C：Studio 迭代

支持人工直接修改、自然语言修改、Spec diff、Preview、Fork 和显式激活。

### 阶段 D：生产交付

补充 Native 交付、迁移、权限、审计、私有模型、LTS 和一个真实领域 kit。

任何阶段若需要改变 Novadraw 引擎契约，必须先从产品原型提取通用需求，再进入独立
设计或 ADR，不以产品便利为由直接扩大 Core。

## 18. 待决策问题

以下问题需要在实施前形成产品级 SSOT：

1. `EditorSpec` 使用 JSON Schema、专用 DSL 还是二者组合；
2. 动态 `DocumentModel` 的值类型、稳定身份和通知协议；
3. Spec compiler 的中间表示与兼容策略；
4. 属性面板 schema 与 Web/Native host adapter 的共同边界；
5. 受限表达式的能力预算和图查询范围；
6. schema migration 的声明式边界；
7. Preview 切换新 Spec 时可保留哪些 WorkspaceState；
8. 插件机制是否进入首个商业版本；
9. AI provider、MCP 和本地模型如何作为可替换 adapter；
10. 首个真实设计伙伴选择哪个垂直领域。

## 19. 参考资料

项目内：

- [Novadraw 商业价值分析](commercial-value-analysis.md)
- [AI 时代的关系编辑器机会研究](ai-era-relationship-editor-research.md)
- [Editor 框架架构](../design/editor/architecture.md)
- [独立 Editor 框架边界](../adr/adr-015-editor-framework-boundary.md)
- [扩展协议、生命周期与稳定发布边界](../adr/adr-014-extensibility-and-lifecycle-boundaries.md)
- [Core 公开 API 边界](../adr/adr-017-core-public-api-boundary.md)
- [可组装 API 与 Scoped Editor](../adr/adr-019-composable-api-and-scoped-editors.md)
- [公开 Facade 与 Feature 边界](../adr/adr-021-public-facade-and-feature-boundary.md)

外部：

- [Figma Make: AI App Builder](https://www.figma.com/solutions/ai-app-builder/)
- [CopilotKit Dynamic Schema A2UI](https://docs.copilotkit.ai/strands/generative-ui/a2ui/dynamic-schema)
- [assistant-ui Generative UI](https://www.assistant-ui.com/docs/tools/generative-ui)
- [Eclipse Sirius](https://eclipse.dev/sirius/getstarted.html)
- [Sirius Specifier Manual](https://eclipse.dev/sirius/doc/specifier/Sirius%20Specifier%20Manual.html)
- [Raiven: LLM-Based Visualization Authoring via DSL Mediation](https://arxiv.org/html/2604.10008)
