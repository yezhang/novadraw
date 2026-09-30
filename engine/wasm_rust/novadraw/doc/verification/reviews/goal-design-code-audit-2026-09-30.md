# 项目目标、设计与实现一致性审计

类型：`verification`

日期：2026-09-30

目标：使用 Rust 实现能力对等或超过 Draw2D 的图形框架，尤其性能、可扩展性、跨平台。

## 1. 审计结论

**设计主干促进项目目标，但目前不能证明整体能力、性能和四平台交付已经达标；
实现也存在需要修正的契约偏差。**

六个公开 package、二维 Figure 协议、Runtime 单一提交权威、布局/路由的只读输入与
受控发布、Editor 模型权威、平台与渲染后端分离，应继续保留。当前没有推倒重建或
再次按概念拆分大量 crate 的依据。

主要问题集中在：

1. **目标没有完整验收闭环**：M1-M10 是既定切片，不能代表全部 Draw2D 能力；
   剩余能力、性能对照、Windows/Linux 运行资格没有进入同一追踪体系。
2. **实现与扩展契约尚有差距**：初始 validation、平台输入、扩展 fault 边界存在
   可达问题；部分第三方更新路径不能贯穿 Figure callback 与 Editor refresh。
3. **性能证据与实际主路径不充分对应**：历史基准主要测 CPU 录制，真实文本稳定化、
   独立连接、GPU、present 与端到端交互缺少可比基线。
4. **文档与门禁会给出过强信心**：根层 API 再次扩大，旧符号仍标为 verified；
   现有负向编译示例与 docs gate 无法发现这些偏差。

建议执行顺序为：目标与剩余能力登记 → API/确定缺陷与文档整理 →
性能基线、四平台验证并行 → 有证据的内部模块与能力扩展调整。
完整任务、依赖和毕业条件见
[调整计划](../../roadmap/goal-alignment-adjustment-plan-2026-09-30.md)。

## 2. 范围与证据等级

起始版本为 `811b748f0a510edb90ad198346950ca3752b404d` 加已有 P2-E02 工作区改动。
审计期间这些改动被另一工作流提交，最终复核 HEAD 为
`841be423815833d97c19862da7b1b0a8ad05591f`。该提交与起始已采集源码内容一致。
交付检查时另有并行的 Label/TextFlow 样式与对齐、caret 展示和示例改动出现，
详见 [交付指纹检查](../../../verification/evidence/goal-alignment-audit-2026-09-30/delivery-checks.json)。
它们不在本轮完整审计范围；已复查其差异未修复这里列出的五项问题，Policy 的当前
代码锚点随新增行调整。本次没有修改或提交引擎实现。

Draw2D 参考基线为本地 gef-classic
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，仅使用 `org.eclipse.draw2d`、
`org.eclipse.gef`，未将 Zest 作为框架能力依据。

审查顺序是目标与规范在先，实现和验证证据在后。覆盖总体/静态架构、API ADR、
坐标与更新、资源与生命周期、连接与文本、Editor、平台、parity、路线图和验证工具；
默认排除归档。Book 与策略材料作为使用入口和目标偏移检查的补充。
这是分组、按契约与调用链定向审计，**不是所有代码逐行穷尽检查或四平台认证**。

区分以下结论：

- **目标差距**：当前尚未交付或无法证明达标，不等于运行失败。
- **设计/实现偏差**：代码或文档未兑现现行契约；先判定契约是否合理，再决定修改方向。
- **确定代码问题**：有具体输入、调用链或工作量证据；实测与静态确认分别标注。
- **已接受限制**：当前实现遵守已有 ADR，但该限制仍需在长期目标中有去向。

原始清单与每组阅读范围：
[基线](../../../verification/evidence/goal-alignment-audit-2026-09-30/baseline.json)、
[基线复核](../../../verification/evidence/goal-alignment-audit-2026-09-30/baseline-recheck.json)、
[性能组](../../../verification/evidence/goal-alignment-audit-2026-09-30/group/group_1.md)、
[平台组](../../../verification/evidence/goal-alignment-audit-2026-09-30/group/group_2.md)、
[扩展组](../../../verification/evidence/goal-alignment-audit-2026-09-30/group/group_3.md)、
[文档组](../../../verification/evidence/goal-alignment-audit-2026-09-30/group/group_4.md)。
Inventory 数量只代表候选范围，不能当作逐行审查覆盖率。

## 3. 目标追踪矩阵

| 目标 | 设计依据 | 实现/已有证据 | 审计判断 |
|---|---|---|---|
| Draw2D 行为能力 | overview、parity、M1-M10、P2 backlog | 树/布局/坐标/事件/连接/文本等主链有契约；仍有 Graphics、图布局、跨 viewport 等剩余面 | 基础体系成立，整体对等尚未闭合 |
| 性能 | 增量更新、derived worklist、damage、R8/D4.5 | 有历史 CPU 改善证据；当前仍有全扫描、重复录制、文本祖先重复访问 | 不足以证明等于或超过 Draw2D |
| 扩展性 | ADR-014/019、Figure/Layout/Router/TextLayoutEngine | 外部组件、路由和文本构造入口真实存在 | 局部可扩展；callback/Editor/派生快照链路尚不完整 |
| 跨平台 | overview 四平台、ADR-023、PlatformHost | Core 依赖隔离；macOS/Chrome 的历史运行记录；Web 编译链路 | Windows/Linux、其他浏览器不能由这些记录推导 |
| 模块/API | ADR-017—023 | package 方向合理，scoped editor 保持统一提交 | 内部模块约束与实现矛盾，root 导出和 capability 表面继续扩大 |
| 可验证与可维护 | docs/parity/suite 权威分工 | 47 commands、23 suites；契约测试丰富 | 状态、符号、能力分母和性能结果之间缺少机器校验 |

## 4. 文档与目标的偏差

### A01：Draw2D 对等目标缺少完整分母与剩余交付项

优先级：高。置信度：高。性质：目标追踪缺口。

[parity 账本](../../parity/draw2d/api-coverage.md#L197-L211) 的方法行统计为
83 verified、8 partial、1 deferred。**这不是覆盖率**：行的粒度不同，一行可含多个
API；graph layout、打印等还只出现在高层能力表。部分 verified 行内部又明确排除
fragment 样式、完整原生 AT provider 等子能力。

主要剩余项包括：

- path clip、gradient、custom dash/offset、miter 配置；
- 原子 child + constraint + index、可替换 clipping、跨 viewport 连接；
- 文本片段样式与更完整的 flow 组合、部分 widget 行为；
- Draw2D 本体的 `DirectedGraphLayout`/`CompoundDirectedGraphLayout`；
- 打印/输出目标、完整平台 accessibility，以及同域对象保活的能力评估。

其中不少项停留在“真实需求出现再做”，没有进入
[P2 backlog](../../roadmap/p2-delta-backlog.md) 的独立条目、依赖与验收。
“按需排序”合理，“长期不追踪却宣称对等”不合理。
自动布局位于允许对标的 Draw2D graph 包，不能按 Zest 上层能力排除。

处置：在既有账本中固定能力分母，区分等价、组合替代、待补和明确拒绝；剩余项进入
正式 delta。保留已完成 M1-M10 的历史含义，不修改历史测试来扩大其结论。

### A02：性能目标没有覆盖真实出帧路径的可比较验收

优先级：高。置信度：高。性质：目标证据缺口与设计/实现偏差。

[demo-matrix](../../roadmap/demo-matrix.md#L62-L69) 给出部分场景 30/60fps，
但缺少固定硬件、场景、可见比例、统计口径与可执行阈值；
[R8 基线](../performance/r8-baseline-2026-09-02.md)、
[D4.5](adr014-d4.5-performance-2026-09-10.md) 主要证明历史 CPU 路径回归或改善。
现有 suite 没有接入 Draw2D 同场景性能对照。

实际差距包括：

- `DerivedWorkQueue` 是阶段 bool 集合，每次稳定化重启多个全局阶段；
  与 [subject/revision/generation worklist 契约](../../design/architecture/derived-state-convergence.md)
  的粒度不同。缓存命中不等于不扫描。
- Partial damage 当前仍录制完整可见树、逐条 Vello lowering；GPU 使用 scratch、
  retained 与 surface 复制。设计允许这种实现，因此它不是单独的像素正确性缺陷，
  但“局部 damage”不能被解释为 CPU/GPU 工作量按脏区线性减少。
- R8 深树使用 Rectangle，文本场景是 TextProbeFigure，未覆盖真实 Label/TextFlow
  的稳定化成本；单组 64 条避障连接计数也未覆盖大量独立 route group。
- 本地历史 JSON 支持 D4.5 的 10k 录制约 787ms→9.86ms、validation
  约 413ms→1.49ms；这些是旧版本、特定 CPU 路径的数据，不能当作当前端到端帧时。

处置：先建立同场景 CPU/GPU/present 与内存基线，再做增量调度和重复工作优化。
保留递归主线保护，不能仅因复杂度怀疑就恢复旧迭代 POC。

### A03：四平台是设计承诺，当前证据主要是 macOS 与 Chrome

优先级：高。置信度：高。性质：目标证据缺口。

[overview](../../design/architecture/overview.md#L12-L21) 明确四平台；
manifest 的平台标签只有 host、native-macos、web、headless、wasm。
[Web 验收说明](../manual/web-platform.md#L28) 明确不替代 Windows/Linux 和
Firefox/Safari 验证，历史总审计也保留 Windows/Linux 发布资格为后续范围。

P2-E02 最新记录已经是 complete；本次不沿用旧记忆把它改判成未完成。
但特定 macOS/Chrome 人工序列通过，不能替代其他键盘布局、IME 时序和窗口后端。
Native accessibility 当前是 snapshot/update 承载，完整 OS provider 明确后置。

处置：建立逐平台支持等级、构建与真实运行矩阵。缺少环境写“未验证”，不写 PASS 或
FAIL；优先覆盖输入符号、DPI、surface 恢复、文本输入与 accessibility action。

## 5. API、模块与文档一致性

### A04：三层公开 API 边界回退，负向测试没有发现

优先级：高。证据：独立编译确认。

[ADR-021](../../adr/adr-021-public-facade-and-feature-boundary.md) 的 facade 分层由
ADR-023 保留；[Core README](../../../novadraw/README.md#L36-L40) 也声明低层类型
不可从根层导入。但 [lib.rs](../../../novadraw/src/lib.rs#L93-L142) 仍导出
FigureNode、NodeState、UpdateManager、EventDispatcher、PendingMutations、
NotificationQueue 等低层类型和大量专业协议。

现有示例：

```rust,ignore
use novadraw::{FigureNode, RenderCommand, UpdateManager};
```

它只因 RenderCommand 不存在就满足 `compile_fail`。本次 doctest 通过，而独立探针
确认其他四个被抽查的低层类型均可 root 导入：
[探针结果](../../../verification/evidence/goal-alignment-audit-2026-09-30/public-import-probes.json)。
因此这是 API 边界回退与验证盲点，**不等于已经获得绕过 Runtime 的任意可变权限**。

处置：root 白名单、advanced 从定义模块导出、逐符号负向检查与领域路径正向检查。
依赖禁入脚本也应补上 Editor/Inspector；当前依赖图正确不等于门禁覆盖所有禁止项。

### A05：package 拆分合理，但内部依赖声明与实现不一致

优先级：中。证据：静态确认。性质：架构契约矛盾。

[directory-structure](../../design/architecture/directory-structure.md#L188-L202)
禁止 FigureTree 依赖 UpdateManager，实际
[graph/mod.rs](../../../novadraw/src/graph/mod.rs#L2020-L2087) 接收并驱动它。
Figure 的回调上下文来自 runtime；layout 又使用 viewport effect 与 notification 值。
合并 crate 或从 root 转发类型没有消除实际依赖。

同时，ADR-018 允许内部 tree/update 协作，静态架构明确概念图不承诺字段/路径。
所以不能只机械移动文件，应先明确“禁止依赖”究竟约束稳定协议、可变服务所有权，
还是所有实现调用。

处置：明确依赖规则后按职责拆内部模块，保持 Runtime/Viewer 所有权集中。
继续保留 Core、Editor、Inspector、Vello、Winit、Web 六 package；没有理由重新拆回
多个强耦合的 Core/Geometry/Render/Scene 发布包。

### A06：扩展入口真实存在，但没有贯穿完整调用生命周期

优先级：高。证据：静态确认与既有外部组件测试通过。

`FigureComponentUpdate`、LayoutSnapshot/Output、Router/Anchor、TextLayoutEngine
支持外部实现；本次 `d4_component_update` 四项通过。以下边界仍不足：

- [EventContext](../../../novadraw/src/runtime/context.rs#L196-L325) 的 FIFO mutation
  没有任意 typed component update，ADR-014 所述 callback owned update 未贯通；
- [VisualUpdateContext](../../../novadraw-editor/src/part/mod.rs#L974-L1020)
  只有有限的 primary bounds/style/label 操作，外部复合 Figure 私有数据无法沿标准
  模型通知 refresh 通路更新；
- 任意动态宽度与字体 revision 下的外部文本派生快照，没有与内置文本等价的服务/发布
  闭环；固定两个预制 layout 的测试不能证明这一点；
- Figure 基础 trait 继续知道 text_flow、label、clickable 等内置能力，通用
  FigureEditor 继续容纳专用 mutator；这是已记录的能力面收口债务。

处置：以外部复合 Figure 同时走“模型通知刷新”和“自身输入更新”为验收，补受控
update 通路；不让应用借共享可变 Figure 绕开 Runtime。通用节点 API 与专用能力 editor
分开，但不建设无消费者的通用 plugin registry。

### A07：当前文档中存在会误导实施的过期指引

优先级：中。置信度：高。

| 位置 | 偏差 | 调整方向 |
|---|---|---|
| [design 索引](../../design/00-index.md#L48-L54) | component-update 被列为非规范提案，专题却已 accepted/implemented | 统一规范效力与索引 |
| [Inspector 设计](../../design/architecture/figure-inspector.md) | 承诺后续独立 protocol crate，与 ADR-023 的按实际版本需求拆分不一致 | 引用最新裁决，区分 target 与 deferred |
| [parity](../../parity/draw2d/api-coverage.md#L202-L225) | 已删除 crate 路径；FigureTree 私有 setter 仍作为 verified 公开 API | 对齐真实模块与 scoped editor |
| [viewport parity](../../parity/draw2d/api-coverage.md#L304-L309) | 把已经内部化的 handle mutator 作为公开入口 | identity handle 与 Runtime editor 分开 |
| [product-deliverables](../../roadmap/product-deliverables.md) | ShortestPath 仍为 Year 2，而 P2-C02 已完成 | 更新能力清单，保留旧阶段范围说明 |
| [roadmap](../../roadmap/00-index.md) | 当前执行方向仍写从已完成的 ADR-020 继续 | 当前入口只指向真实下一步 |
| [book](../../../book/src/09-verification-and-extension.md#L72-L78) | `workspace.quality` 已不存在 | 替换为当前有效验证入口 |
| design 状态/类型 | target、approved、accepted/complete 混用，basic-widgets 使用未列入类型表的 architecture | 规范效力与交付状态分离 |

`cargo xtask docs` 只校验 manifest 引用与 parity 状态词，不解析上述公开符号、
索引归类或能力语义，因此它通过不能反证这些问题。
策略材料中的商业定位可作为需求输入，不能将本次明确的通用框架目标悄然收窄为
某种关系编辑器产品。AI Studio、产品 schema、3D 和 IPC 协议应继续遵守已有后置边界。

## 6. 优先代码问题

以下保留五项优先复核/修复入口；最终结构化清单、置信度和建议见
[代码审查报告](../../../verification/evidence/goal-alignment-audit-2026-09-30/report.md)。
P1/P2 为代码缺陷级别，与历史 roadmap 的 P2 delta 命名无关。

### B01：构建期失效树交给 Runtime 后首次 validation 可能被跳过

P1，置信度 10/10。位置：
[Runtime 初始化](../../../novadraw/src/runtime/runtime.rs#L758-L793)。

Builder 安装 StackLayout 会把树标为 invalid，但 Runtime 新建空 UpdateManager，
activation 没有将初始布局根入队。普通矩形树没有文本等附加刷新时，
stabilize 只看更新队列，可能在布局未执行时录制并提升 stable epoch。

最小输入：100×100 根、10×10 child、StackLayout，不额外 validate/revalidate。
本轮探针确认首次出帧后 child 仍为 10×10、root_valid=false；显式 revalidate 后
child 才变为 100×100。应由接管事务完成初始验证，不让宿主猜测还需一次 resize
或 revalidate。

### B02：Web 滚轮符号未转换为 Core 内容位移约定

P1，置信度 10/10。位置：
[WebInputAdapter](../../../novadraw-platform-web/src/input.rs#L145-L158)。

DOM 正 deltaY 表示向下滚动请求；adapter 只换量纲，Core ScrollPane 使用
`old - distance`。在顶部向下滚动会被 clamp 在零，中段会反向移动。
应在 Web adapter 转换普通滚动的符号，保留 Ctrl-wheel zoom 的独立语义。
这是静态输入/消费链确认，本次没有浏览器重放。

### B03：DOM Release 在 bridge 借用期间触发同步 blur

P1，置信度 7/10。位置：
[WebTextInputHost::apply_effect](../../../novadraw-platform-web/src/dom_text_input.rs#L205-L230)。

`if let` 中的 `bridge.borrow_mut()` 保持到成功分支结束；Release 调 textarea.blur，
同步 blur listener 再借用同一 RefCell。保持 textarea 聚焦时 Enter/无 composition 的
Escape 释放会进入该路径。清空 active 不能规避借用检查。

先取得 owned action 并结束借用，再执行 DOM focus/blur。结论来自 Rust 生命周期与
实际调用链；本次未运行浏览器，需定向补 Enter/Escape/外部失焦三条路径。

### B04：Policy panic 未进入 Viewer 的 fault 边界

P1，置信度 9/10。位置：
[command_for_request](../../../novadraw-editor/src/viewer/mod.rs#L2096-L2128)。

可修改自身状态的外部 policy 直接执行，没有与 refresh 一致的 panic/fault guard。
CommandStack 的保护发生在 Command 已生成之后，无法覆盖此前的 policy panic。
宿主捕获 unwind 后 Viewer 仍可被当作可用；已有 fault 也未统一阻止 policy/feedback 入口。

应在公开入口先检查 fault，将扩展 panic 纳入 Viewer 统一失效隔离；不承诺任意副作用
回滚。本次为静态确认，尚未运行故障注入。

### B05：合并 compile_fail 掩盖单个禁止导出名的回归

P1，置信度 10/10。位置：
[Core README](../../../novadraw/README.md#L38-L40)。

同一个负向编译块同时导入三个名称，只能验证“至少一个不可导入”，不能验证三者
均不可导入。RenderCommand 的缺失掩盖 FigureNode、UpdateManager 的根层泄漏；
本轮独立编译已确认，详见 A04。按每个禁止名独立建立负向断言，并收窄实际导出。
这是测试可靠性问题，不是一般文档措辞问题。

## 7. 其他观测与调整边界

分组原始证据还记录了首帧/Full promotion 重复录制、独立连接复制兄弟列表、
文本祖先重复扫描、LayoutOutput 数值预检不足，以及 Web 按钮映射和 accessibility
action 集成问题。未列入五项摘要不表示已关闭；后续逐项处置以独立复核结论为准。

重复完整录制已由探针确认：首次 submission 的一个 Figure 被 paint 两次。
Label/TextFlow 在缓存命中前逐节点扫描祖先，深链为 O(N²) 工作量；未测实际帧时，
独立复核将其保守列为 P2 性能项。LayoutOutput 风险限定为外部输出防御不足，
没有证据表明所有内置布局当前都会输出非法值。

独立复核撤回了原始组报的 Windows AltGr 缺陷：Winit 会过滤相应 Ctrl/Alt modifier，
原触发前提不成立。composition 去重候选降为待平台取证：必须证明浏览器在
ClearValue 后重新写回文本，不能把 input 事件本身当作 value 恢复的证据。
原始 JSONL 保留审查过程，不能未经本轮裁决直接当作最终缺陷。排除与降级理由见
[跨组复核](../../../verification/evidence/goal-alignment-audit-2026-09-30/cross-group-review.md)。

以下不是本次建议：

- 以测试数量或大文件行数作为质量/性能结论；
- 因为原生 AT、graph layout 尚缺，就否认现有 Core 的有效能力；
- 为满足 Draw2D 类名一一对应而复制 Java 所有权；
- 将所有 example composition root 都搬入 Core；
- 未经测量改写递归主循环、引入全局状态或新建占位 3D/协议 crate；
- 通过把设计改成当前代码来关闭尚未论证的偏差。

## 8. 本轮验证与限制

| 验证 | 结果与意义 |
|---|---|
| `cargo xtask docs` | PASS：47 commands、2 profiles、23 suites；仅证明现有检查范围 |
| Core doctest | 5 个普通用例 + 1 个 compile_fail PASS；同时发现合并导入造成的漏检 |
| facade + component 定向测试 | 2 + 4 项 PASS，证明现有正向导入与外部组件基本更新链 |
| Core 依赖门禁 | PASS，无平台渲染依赖；另行静态确认 Editor/Inspector 单向依赖 |
| 独立 root import 探针 | 四个低层名称可导入，仅 RenderCommand 被拒绝，确认 A04 |
| Runtime 最小探针 | 首帧 child=10×10/root invalid，显式 revalidate 后 100×100；首次 submission paint_calls=2，复现两项问题 |
| book 失效命令 | `workspace.quality` 返回 unknown command，确认读者路径失效 |

日志与探针：
[定向测试](../../../verification/evidence/goal-alignment-audit-2026-09-30/targeted-tests.log)、
[依赖检查](../../../verification/evidence/goal-alignment-audit-2026-09-30/facade-dependencies.log)、
[Runtime 探针](../../../verification/evidence/goal-alignment-audit-2026-09-30/runtime-probe.json)。

本轮没有重跑 workspace full、跨平台实际窗口、浏览器 IME、GPU 像素差分或
Draw2D 性能对照。历史 PASS 只作为对应历史范围的证据，不替代当前目标验收。
报告与计划只调整后续工作依据；修复与架构实施需按新计划分切片推进。
