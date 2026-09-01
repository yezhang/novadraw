# Novadraw 商业价值分析

类型：`product-strategy`

状态：`current`

更新日期：2026-08-30

## 1. 文档目的

本文从商业视角分析 Novadraw 的价值、目标市场、竞争位置、商业化条件和主要风险。
分析基于以下三个事实来源：

1. Eclipse Draw2D / GEF 长期积累的图形编辑架构与行为语义；
2. Rust、WebGPU、WASM 与 Rust GUI / Diagram 生态的当前状态；
3. Novadraw 当前 M1-M10 路线图及其已验证能力。

本文不定义运行时架构，不改变 `design/`、`parity/` 或 `roadmap/` 中的现有契约和
里程碑状态。市场规模、价格和客户意愿属于待验证商业假设，不应被当作实现需求。

## 2. 核心结论

Novadraw 的商业价值不在于“用 Rust 重写 Eclipse Draw2D”，而在于：

> 将 Draw2D / GEF 二十多年验证过的专业图形编辑语义迁移到
> Rust + WebGPU + WASM 时代，形成面向专业节点、连线和关系型编辑器的
> 跨平台基础设施。

这是一个目标客户专业且集中、工程门槛高、潜在客单价较高的基础软件机会。它不以
大众用户数量取胜，而以降低专业软件的研发成本和长期风险创造价值。项目最合理的
商业定位不是通用 GUI 框架、通用白板或普通 Canvas，而是：

> 面向工业软件和开发者工具的 Rust/WASM 可嵌入式关系编辑器 SDK。

当前 M1-M8 已形成较强的技术资产，但距离可售 SDK 仍有差距。M9 Connection 是项目
从“可靠画布”进入“关系型图形编辑器”的价值拐点；后续 GEF 式编辑事务、模型绑定和
领域语义决定项目能否形成商业闭环。

## 3. Eclipse Draw2D / GEF 的价值

### 3.1 Draw2D 的价值不是基础绘图

矩形、文本、折线和图片等绘制能力容易被渲染库替代。Draw2D 更重要的资产是一个
长期验证的统一 Figure 协议：

- Figure 树同时承载组合、Z-order、绘制和命中；
- bounds、client area、insets 和坐标转换使用一致的盒模型；
- paint、layout、hit-test、event target 和 damage 共享同一组空间语义；
- Validation 与 Damage Repair 构成两阶段更新；
- Layout、Border、Anchor、Router 和 Locator 通过策略扩展；
- Figure 生命周期和分层通知维持对象关系的一致性；
- Viewport、Scroll 和 Zoom 作为 Figure 语义的一部分参与整条管线。

这些机制的价值在于消除组合场景中的系统性错误。专业编辑器最昂贵的问题通常不是
“画不出某个图形”，而是节点在嵌套、缩放、滚动、重排、删除或重挂载后，绘制、
命中、事件和更新结果不再一致。

### 3.2 Draw2D 与 GEF 的分层是重要资产

Draw2D 负责图形系统：

- Figure、Graphics、Layout；
- Connection、Anchor、Router；
- 事件、坐标、Viewport 和 UpdateManager。

GEF 在其上负责编辑系统：

- Model 与 View 的映射；
- EditPart / Viewer；
- Tool / Request / EditPolicy；
- Command / CommandStack；
- selection、palette、创建、删除、重连和撤销重做。

这一边界说明“图形引擎”和“图形编辑器”不是同一个产品层。Draw2D 解决显示与基础
交互的正确性，GEF 把用户操作转换为可验证、可撤销的业务模型变更。

### 3.3 长期验证降低架构探索风险

[GEF Classic 在 2026 年仍有持续发布][gef-release]。其商业启示不是 Java/SWT
仍是最佳技术栈，而是 Figure、Connection、Request、Command 和 EditPolicy
所解决的问题具有长期稳定性。

Novadraw 可以复用这些稳定问题定义，减少从零发明图形编辑协议的风险，但不应机械
照搬 Java 对象模型。项目当前采用 ID 引用树、Arena 所有权、小 Trait、显式上下文
和原子事务，是符合 Rust 特性的迁移方向。

### 3.4 Draw2D 的现代化缺口

Draw2D / GEF Classic 的主要限制是产品载体：

- 绑定 Java、SWT 和 Eclipse Workbench；
- 难以独立嵌入现代 Rust、Tauri 或 WebAssembly 产品；
- 无法自然共享 Native、Web 和 headless 的同一套核心逻辑；
- 继承体系和宽接口不适合 Rust 的所有权与组合模型；
- 渲染后端难以直接利用现代 WebGPU 管线。

这构成 Novadraw 的机会窗口：迁移经过验证的语义，而不是复制旧平台。

## 4. Rust 生态带来的机会

### 4.1 基础图形栈正在成熟

Rust 已经具备构建现代图形编辑器所需的大部分底层组件：

- `wgpu` 提供跨 Vulkan、Metal、DirectX 和 WebGPU 的 GPU 抽象；
- [Vello][vello] 提供 GPU 驱动的二维矢量渲染；
- cosmic-text、Parley 等项目提供文本 shaping 和布局能力；
- winit 提供跨平台窗口与输入；
- Tauri 和 WASM 提供桌面与浏览器交付路径；
- serde、slotmap 和成熟测试工具适合构建可验证的数据模型。

Novadraw 不需要重新发明 GPU、窗口或序列化基础设施，可以集中投入 Figure、
Connection 和编辑语义。

### 4.2 Rust GUI 生态仍然碎片化

Rust 已有 egui、iced、Slint、[Xilem][xilem]、[GPUI][gpui] 等 GUI 方案，但它们
在以下方面仍存在明显差异：

- immediate mode 与 retained mode；
- Native、Web、移动和嵌入式覆盖；
- API 稳定性；
- 文本与 accessibility；
- 渲染后端和平台集成；
- 商业授权与长期支持。

这意味着通用 GUI 领域竞争激烈且尚未收敛。Novadraw 不应与这些项目竞争按钮、
表单、主题和普通应用布局，而应作为专业画布或 Diagram 子系统与其互补。

### 4.3 Rust Diagram 生态不再是空白

Rust 生态已经出现 `egui_node_editor`、[egui_xyflow][egui-xyflow]、
`hanabi_node_graph`、[flowmaid][flowmaid] 和 `dagre-rs` 等项目。它们证明节点图、
流程图和自动布局存在需求，也意味着“Rust 中没有节点编辑器”不能作为 Novadraw
的定位。

这些方案多集中在以下一种能力：

- 某个 GUI 框架中的节点图 Widget；
- Mermaid 类 DSL 与静态图生成；
- 图布局算法；
- 特定产品的内部画布；
- 基础节点、端口和连线交互。

Novadraw 的潜在差异是提供完整 retained-mode 协议，包括深层 Figure 组合、统一
坐标根、两阶段更新、精确 damage、生命周期、Connection Runtime，以及未来的
GEF 式编辑事务。

### 4.4 Rust 采用增长不等于 GUI 市场自动增长

[2025 State of Rust Survey][rust-survey] 显示企业中的 Rust 代码和招聘需求继续
增长，但 Rust GUI 仍是整个 Rust 市场中的细分领域。因此：

- “Rust 用户增加”是有利条件，不是收入证明；
- “生态缺少完整框架”可能意味着机会，也可能意味着需求不足；
- 商业验证必须面向有预算的产品团队，而不能只统计 crate 下载和社区关注；
- 目标市场不应限定为 Rust 开发者，还应通过 WASM/TypeScript 和 C ABI 服务其他
  技术栈。

### 4.5 “市场边界”需要分层理解

“纯 Rust GUI crate”“跨平台 Diagram SDK”和“垂直领域编辑产品”不是同一个市场：

| 市场层 | 购买者 | 市场特征 | Novadraw 的作用 |
|---|---|---|---|
| Rust crate | Rust GUI 开发团队 | 客户数量较少，社区采用驱动 | 建立生态入口和技术信任 |
| Diagram SDK | 跨平台产品团队 | 客户集中，按许可与支持付费 | 提供可嵌入编辑器核心 |
| 垂直方案 | 行业客户 | 按业务 ROI 采购 | 提供领域模型和工作流 |

因此，“Rust GUI 是细分领域”只描述第一层入口，不能代表整个商业上限。只发布 Rust
crate 会受到 Rust GUI 团队数量限制；提供 WASM/TypeScript、C ABI 和 headless
能力后，购买者不必使用 Rust；进入工业拓扑、数据血缘或 AI Workflow 后，客户为
业务交付和风险降低付费，而不是为编程语言付费。

用市场分析术语表达：

- **TAM**：所有需要嵌入节点、连接、拓扑和关系编辑能力的专业软件；
- **SAM**：需要高可靠、跨 Native/WASM、可私有部署或可无头验证的 Diagram 产品；
- **SOM**：项目早期能够实际触达的 Rust/Tauri 团队和少数工业设计伙伴。

当前真正较小的是早期 SOM，不是长期 TAM。商业战略的任务是以 Rust 作为技术切入点，
再通过多语言绑定、完整编辑能力和领域套件扩大 SAM，而不是永远停留在 Rust crate
市场。

## 5. Rust 能转化成哪些客户价值

### 5.1 Native、WASM 与 headless 共核

同一份模型、路由、布局、命中和验证逻辑可以运行于：

- 原生桌面应用；
- 浏览器 WebAssembly；
- 服务端图片生成；
- CLI 验证；
- 自动化测试。

客户由此避免在 Web 与 Native 产品中维护两套容易产生差异的编辑器内核。

### 5.2 确定性和可测试性

纯计算 Router、稳定 ID、显式事务和可序列化结果有利于：

- golden test；
- 截图回归；
- 路由与布局结果重放；
- 服务端文档验证；
- AI Agent 生成图后的自动校验；
- 问题报告的确定性复现。

### 5.3 安全、离线和长生命周期

工业控制、设备配置、基础设施和本地专业工具通常更重视：

- 内存安全；
- 无网络运行；
- 可控资源占用；
- 长期支持；
- 私有部署；
- 与现有 C/C++ 或 Rust 核心集成。

这些需求比普通 Web 流程图更能体现 Rust 的商业优势。

### 5.4 性能上限

Rust + WebGPU 为大规模动态二维场景提供了更高的性能上限，但性能不能只靠技术栈
宣称。商业版本必须用以下指标提供证据：

- 节点和连接规模；
- 节点移动时的增量重路由成本；
- 局部更新与全量更新耗时；
- 内存占用；
- Native 与 WASM 的帧时间；
- headless 输出吞吐量。

## 6. Connection 为什么是商业价值拐点

### 6.1 节点表达实体，Connection 表达业务

专业图形产品中的节点通常代表业务对象，而连接代表：

- 工作流中的控制流；
- AI Pipeline 中的数据流；
- 电气系统中的物理连接；
- 网络系统中的拓扑；
- 数据平台中的血缘；
- 软件模型中的依赖；
- 规则系统中的条件转移。

图元绘制解决“看见对象”，Connection 解决“理解和编辑关系”。后者更接近客户
真正购买的业务能力。

### 6.2 Connection 横跨全部核心协议

一个可靠的 Connection 系统必须同时处理：

- Anchor 对 owner、形状和端口位置的查询；
- Router 对端点、约束和障碍物的计算；
- 节点移动、resize、reparent 和 remove；
- route cache 及依赖索引失效；
- 新旧路径的 projected damage；
- stroke-aware hit-test；
- 缩放、滚动和嵌套坐标根；
- 标签、箭头和其他 Decoration 定位；
- ConnectionLayer 的绘制与命中顺序。

因此 Connection 是检验 Figure、坐标、通知和 UpdateManager 是否真正闭环的综合
能力，而不是一个独立绘制功能。

### 6.3 商业 Connection 不止是 Router

要形成可售编辑器能力，Connection 还需进入业务语义层：

- typed ports；
- source / target 方向；
- 端口基数；
- 类型兼容；
- 环路约束；
- 跨层或跨安全域规则；
- 创建、预览、取消和重连；
- 删除时的级联、拒绝或悬空策略；
- Command 与 undo/redo；
- 自动布局协同；
- 持久化和版本迁移。

所以应将商业意义上的 Connection 定义为：

> 关系生命周期 + 几何路由 + 编辑事务 + 领域约束。

### 6.4 Connection 的迁移成本形成护城河

客户一旦围绕 Anchor、Port、Router、连接约束和持久化格式建立业务模型，更换底层
框架的成本会显著提高。相比颜色、图元或工具栏，Connection API 更容易形成长期
依赖和续费基础。

## 7. 目标市场判断

| 市场 | 匹配度 | 商业理由 | 主要风险 |
|---|---:|---|---|
| 工业拓扑、电气图、设备编排 | 高 | 离线、高可靠、强关系语义 | 行业知识和格式复杂 |
| 数据血缘、可观测性拓扑 | 高 | 大图、增量更新、关系分析需求明确 | 自动布局要求高 |
| Rust/Tauri 专业节点工具 | 高 | 原生方案不足，集成优势清晰 | 单一语言入口限制触达面 |
| 游戏行为树、材质和音频图 | 中高 | Native 性能需求强 | 已有 egui 和游戏引擎插件 |
| AI Workflow / Agent Builder | 中高 | 增长快、连接语义强 | React Flow 类方案竞争激烈 |
| 通用流程图和白板 | 低 | 用户广泛 | 免费 Web 生态和协作产品成熟 |
| 通用 CAD | 低 | 客单价高 | 几何内核、约束求解和格式远超当前范围 |

### 7.1 第一优先级

面向 Rust、Tauri、WASM 或 C++ 产品团队的可嵌入式专业节点编辑器 SDK。

### 7.2 第二优先级

选择一个关系约束强、离线或私有部署明确的垂直领域，例如工业拓扑、数据血缘或
设备编排，形成端到端领域套件。

### 7.3 不建议的首发方向

不建议优先建设通用白板、Figma 类设计工具、普通流程图网站或完整 CAD。这些市场
需要大量协作、内容、格式和产品运营能力，无法充分利用当前引擎资产。

## 8. 竞争格局与付费信号

成熟 Diagram SDK 已证明企业愿意为降低自研成本付费：

- [React Flow][react-flow-pro] 以 MIT 开源核心和付费支持、模板形成商业模式；
- [JointJS+][jointjs-pricing] 以每开发者永久许可销售高级编辑能力；
- [GoJS][gojs-pricing] 以开发者和团队许可提供完整 Diagram SDK；
- [yFiles][yfiles-pricing] 以高价许可销售高级布局、路由、分析和长期支持。

这些产品的价格不能直接作为 Novadraw 定价依据。成熟产品的价格包含：

- 多年算法和兼容性积累；
- 大量示例、文档和集成；
- 高级布局与路由；
- 企业支持；
- 品牌和采购信任。

它们共同证明客户购买的不是绘制 API，而是：

- 缩短上市时间；
- 降低连接、布局和交互的自研风险；
- 获得长期兼容与支持；
- 避免维护大量边缘情况；
- 获得可直接嵌入的产品级能力。

Novadraw 应避免仅以“更便宜的 yFiles”竞争。可持续差异应是：

> Rust-native + WebGPU + WASM + headless + 确定性 +
> Draw2D 级协议完整性。

## 9. 当前项目的商业成熟度

根据当前路线图，M1-M8 已达到 `behavior_verified`，M9 和 M10 尚未完成。由此可将
当前价值判断为：

| 维度 | 当前 | M9 完成后 | 最小 GEF 层完成后 |
|---|---:|---:|---:|
| 技术资产 | 高 | 很高 | 很高 |
| Rust 生态稀缺性 | 高 | 很高 | 很高 |
| 用户可感知价值 | 中低 | 中 | 高 |
| 可售 SDK 程度 | 低 | 中低 | 高 |
| 竞争壁垒 | 中 | 中高 | 高 |
| 短期收入能力 | 低 | 中低 | 中高 |

该表是定性判断，不是估值模型。当前项目更接近高质量核心资产，而不是可直接采购的
商业 SDK。

### 9.1 M1-M8 的商业作用

M1-M8 建立了正确性的基础：

- 几何、Graphics 和 Figure 树；
- paint、clip 和 hit-test 一致性；
- 坐标域与事件点降域；
- Layout、Validation 和 UpdateManager；
- 输入状态机和通知分层；
- Viewport、Scroll 和 Zoom。

这些能力难以独立销售，但会显著降低后续产品的缺陷率和维护成本。

### 9.2 M9 的商业作用

M9 使项目第一次能够支撑具有业务关系的 Diagram。它是从图形内核到 Diagram SDK
的转折点，但还不是完整编辑器。

### 9.3 M10 与 GEF 层的商业作用

M10 的文本、基础 Figure、Tooltip 和 accessibility 影响实际节点的信息表达和可用性。
后续最小 GEF 层至少需要：

- model-view binding；
- selection；
- Tool / Request / Command；
- undo/redo；
- create、delete、move 和 reconnect；
- clipboard；
- serialization；
- property editing 接口。

没有这些能力，客户仍需要自行完成最昂贵的编辑器集成工作。

## 10. 可持续护城河

Novadraw 最有价值的护城河不应是代码量或 Draw2D API 数量，而应是：

1. Draw2D 行为语义的系统化契约测试；
2. Connection 在移动、缩放、删除和深层嵌套下的正确性；
3. Native、WASM 和 headless 的结果一致性；
4. 大图增量更新、路由缓存和 damage 性能；
5. 稳定的文档模型与版本迁移协议；
6. 可插拔领域约束、Router 和 Layout；
7. GEF 式 Command / Request / EditPolicy 编辑架构；
8. 垂直行业套件和真实客户案例；
9. LTS、兼容承诺和企业支持。

其中 1-4 是技术壁垒，5-8 是产品和迁移壁垒，9 是持续收入基础。

## 11. 商业模式建议

### 11.1 开放核心

建议开放能够建立生态和技术信任的基础能力：

- Figure、Geometry 和 Graphics；
- Layout、Viewport 和事件系统；
- 基础 Connection、Anchor 和 Router；
- Native / WASM 示例；
- 核心契约测试。

### 11.2 商业 Pro SDK

可收费能力应直接降低专业编辑器的交付成本：

- 高级正交避障、平行边和增量路由；
- typed ports 与连接约束；
- Command stack 和 undo/redo；
- selection、tools 和 reconnect；
- 序列化、导入导出和版本迁移；
- 大图虚拟化；
- 性能诊断工具；
- TypeScript / C bindings；
- LTS 和企业支持。

### 11.3 领域套件

领域套件比通用引擎更容易表达 ROI：

- Industrial Topology Kit；
- Data Lineage Kit；
- Workflow Kit；
- AI Pipeline Kit；
- Electrical Diagram Kit。

### 11.4 初期价格假设

在产品成熟且经过客户验证后，可测试以下价格区间：

- Pro SDK：每开发者每年 1,000-3,000 美元；
- 企业源码、LTS 和私有支持：每年 20,000-100,000 美元；
- 领域集成项目：50,000-300,000 美元；
- 定制 Router、格式和性能服务：按项目报价。

这些数字仅用于访谈和报价实验。早期产品不能直接按 yFiles 定价，因为当前尚未具备
相同的算法广度、文档、支持和采购信任。

## 12. 主要商业风险

### 12.1 生态空白可能来自需求不足

Rust 缺少成熟 Diagram SDK 既可能代表机会，也可能代表大部分客户仍选择 Web
技术。必须通过客户访谈和付费试点区分二者。

### 12.2 只支持 Rust 会限制市场

商业版本至少需要规划：

- Rust crate；
- WASM / TypeScript binding；
- C ABI；
- 可选的 Tauri 集成。

否则市场上限被限制为纯 Rust GUI 团队。

### 12.3 过度追求 Draw2D parity

Draw2D 是语义基线，不是产品边界。继续补 API 只有在以下情况下才有价值：

- 支撑目标客户工作流；
- 降低集成成本；
- 关闭核心一致性缺口；
- 构成可验证的扩展点。

### 12.4 Connection 停留在绘制层

如果 M9 只完成 Anchor 和 Router，而没有端口、重连、约束、Command 和持久化，
项目仍然只是图形库。

### 12.5 自动布局能力不足

复杂 Diagram 的可读性高度依赖自动布局和边路由。应允许集成外部算法，并在真实
领域图上验证增量布局和 mental map 保持能力。

### 12.6 文本、accessibility 和平台集成不足

专业节点的大部分信息最终是文本。字体、测量、换行、输入法、accessibility 和
剪贴板并非装饰功能，而是产品可用性的组成部分。

### 12.7 上游技术成熟度

Vello、文本和 Rust GUI 生态仍在演进。Novadraw 应保持渲染与场景语义边界，避免
把商业 API 与单一上游实现过度绑定。

### 12.8 许可证与来源治理

[GEF Classic 使用 EPL-2.0][gef-classic]。若实现包含源码翻译、结构性派生或代码
复制，而不仅是基于公开行为和独立契约重新实现，商业授权和源码分发义务需要正式
法律审查。

项目应维护：

- 参考源码基线；
- 语义证据与实现决策的分离；
- 第三方许可证清单；
- 贡献者来源声明；
- 发布制品的许可证审计。

本文不构成法律意见。

## 13. 商业验证路径

### 阶段 A：完成关系内核

完成 M9，并以以下行为而不是类型存在性作为验收标准：

- 节点移动、resize、reparent 后连线正确；
- owner 删除后引用状态明确；
- viewport / zoom 下绘制和命中一致；
- route cache 和 damage 可验证；
- Connection demo 具有视觉断言。

### 阶段 B：建立最小编辑闭环

在不污染 Draw2D 核心的前提下建立最小 GEF 垂直切片：

- 创建节点；
- 创建和重连连接；
- 移动与删除；
- selection；
- undo/redo；
- 保存与加载。

这一步用于证明框架能承载产品，而不是扩充 Demo 数量。

### 阶段 C：选择一个垂直领域

优先选择具有强连接语义、私有部署或 Native 需求的领域。围绕真实模型实现：

- 领域节点；
- typed ports；
- 连接合法性；
- 属性编辑；
- 导入导出；
- 错误反馈；
- 大图性能。

### 阶段 D：外部客户验证

建议找到 5-10 个 Rust、Tauri、工业软件或开发者工具团队，验证：

- 当前采用什么方案；
- 自研 Diagram 的人员与时间成本；
- 最痛的 Connection / Layout / Undo 问题；
- 是否需要 Native + Web 共核；
- 可接受的采购和授权方式；
- 是否愿意为试点付款。

只有付费试点、采购意向或明确替换计划才能作为商业需求证据。

## 14. 商业验收指标

技术指标：

- time-to-first-editor；
- 支持的稳定节点和连接规模；
- 增量重路由延迟；
- 局部更新帧时间；
- Native / WASM 行为一致性；
- headless 输出一致性；
- API breaking change 频率。

产品指标：

- 外部团队完成集成所需时间；
- 示例是否覆盖完整编辑工作流；
- 文档搜索和问题解决时间；
- 客户需要自行编写的基础设施代码量；
- 从试用到可运行 PoC 的转化率。

商业指标：

- 有效设计伙伴数量；
- 付费试点数量；
- 年度支持或许可意愿；
- 客户集成后的持续使用率；
- 定制需求中可沉淀为通用能力的比例。

## 15. 战略边界

Novadraw 应坚持：

- 以 Draw2D / GEF 的稳定语义作为架构输入；
- 以 Rust 最佳实践重新表达，而非机械复制 Java；
- 把 Connection 和编辑事务作为产品主线；
- 把 Native、WASM、headless 一致性作为差异化；
- 用垂直领域验证商业价值；
- 用测试、基准和客户交付证明价值。

Novadraw 不应：

- 扩展成全功能通用 GUI 框架；
- 仅以“Rust 实现”作为卖点；
- 在没有客户验证前追求所有 Draw2D API；
- 直接与通用白板或成熟 Web Diagram 产品正面竞争；
- 把 Demo 可运行误认为产品可采购；
- 把生态空白误认为已存在市场。

## 16. 最终判断

Novadraw 具有三层商业价值：

1. **短期：Rust 图形基础设施价值**

   填补成熟 retained-mode 图形编辑协议的缺口，但直接收入能力有限。

2. **中期：可嵌入 Diagram SDK 价值**

   M9 Connection 加上最小 GEF 层后，可服务 Rust/Tauri、工业工具和专业桌面软件。

3. **长期：领域编辑平台价值**

   当 Connection 具备领域约束、Command 事务、持久化和自动布局后，可以支撑
   高客单价工业产品和长期企业支持。

最终价值公式可以概括为：

```text
商业价值
= Draw2D/GEF 的成熟语义
× Rust/WASM 的现代交付能力
× Connection 的关系表达能力
× GEF 的编辑事务能力
× 垂直领域的付费需求
```

其中任何一项接近零，整体商业价值都会明显下降。项目应被定位为“专业关系型编辑器
的跨平台核心”，而不是“Rust 版 Draw2D”。

## 17. 相关资料

项目内：

- [Novadraw 文档索引](../00-index.md)
- [Draw2D API 语义覆盖账本](../parity/draw2d/api-coverage.md)
- [M1-M10 路线图](../roadmap/00-index.md)
- [产品交付清单](../roadmap/product-deliverables.md)
- [M8-M9 Viewport 与 Connection 交付计划](../roadmap/m8-m9-contract-plan.md)
- [GEF 核心原则](../reference/gef/core-principles.md)
- [Draw2D 设计公理](../reference/draw2d/architecture/design-axioms.md)

外部：

- [Eclipse GEF Classic](https://github.com/eclipse-gef/gef-classic)
- [GEF Developer's Guide][gef-guide]
- [2025 State of Rust Survey](https://blog.rust-lang.org/2026/03/02/2025-State-Of-Rust-Survey-results/)
- [Vello](https://github.com/linebender/vello)
- [Xilem](https://github.com/linebender/xilem)
- [React Flow](https://reactflow.dev/)
- [JointJS+ Pricing](https://www.jointjs.com/pricing)
- [GoJS Pricing](https://gojs.net/latest/pricing)
- [yFiles Pricing](https://www.yworks.com/products/pricing/)

[gef-classic]: https://github.com/eclipse-gef/gef-classic
[gef-guide]: https://github.com/eclipse-gef/gef-classic/blob/master/org.eclipse.gef.doc.isv/guide-src/guide.adoc
[gef-release]: https://download.eclipse.org/tools/gef/classic/milestone/latest/
[rust-survey]: https://blog.rust-lang.org/2026/03/02/2025-State-Of-Rust-Survey-results/
[vello]: https://github.com/linebender/vello
[xilem]: https://github.com/linebender/xilem
[gpui]: https://github.com/zed-industries/zed/tree/main/crates/gpui
[egui-xyflow]: https://crates.io/crates/egui_xyflow
[flowmaid]: https://crates.io/crates/flowmaid
[react-flow-pro]: https://reactflow.dev/pro
[jointjs-pricing]: https://www.jointjs.com/pricing
[gojs-pricing]: https://gojs.net/latest/pricing
[yfiles-pricing]: https://www.yworks.com/products/pricing/
