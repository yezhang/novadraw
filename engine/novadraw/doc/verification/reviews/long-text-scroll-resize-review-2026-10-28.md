# 长文本滚动与 resize 优化方案评审

类型：`architecture-review`

日期：2026-10-28。结论：方向成立，原生裁剪与宽度复用的实施前合同待补齐。

对象：[长文本滚动与 resize 优化提案](../../design/architecture/long-text-scroll-resize-optimization-proposal.md)，修订 1。
审计基线 SHA-256：`37d5a42f54df90e007c54428dd3481c4b61ca8a677bd9fe8decbea3c71859e3c`。
基线共 280 行；下文行号对应此基线，修订后使用章节与意见编号定位。

## 结论与适用范围

共享 IR、原生可见输出和宽度复用的方向成立。
建议前置阶段测量，再根据实测选择滚动或 resize 的优化分支；
原生覆盖合同不必成为宽度复用可行性评估的前置条件。
每次 Execute 仍保持单切片推进。

SR-R01、SR-R02 为 P1，分别约束原生裁剪与默认引擎宽度复用的实施准备，
不阻止共享 IR 或既有授权范围内的测量。
SR-R03～SR-R06 为 P2，在相关接口、成本与验收口径冻结前处理。
这些意见指出设计待证问题，不据此判定现有实现存在正确性 Bug。
本记录不批准 ADR、不新增 Execute 切片、不修改被审方案，
也不关闭前序 LTI-R01／R02。

本次核对方案、当前定向源码、Parley 0.7.0 与既有采样记录；
没有新增性能、算法、窗口或 GPU 测试。
记录中的源码事实不等于收益证据，建议接口与路径不表示已经实现。

## 意见索引

| 编号 | 优先级 | 基线定位 | 修订重点 |
|---|---|---|---|
| SR-R01 | P1 | L113～130，原生可见输出 | 覆盖证明进入录制前查询的通道与每帧检查成本 |
| SR-R02 | P1 | L149～179，宽度复用 | 默认适配候选及复制、换行、导出、交互成本 |
| SR-R03 | P2 | L64～82、L134～136 | 阶段测量前置与按实测选择优化分支 |
| SR-R04 | P2 | L95～111、L259～264 | 完整 IR 提交链路、资源校验与共享留存计费 |
| SR-R05 | P2 | L121～129、L252～253 | 非字形命令、边框遍历和可见范围碎片成本 |
| SR-R06 | P2 | L181～195、L221～255 | 几何消费屏障、分层 oracle 与交互成功判据 |

## SR-R01：原生覆盖证明的准入与查询成本

方案列出证明需要对应的字体、lowerer、设备缩放、clip 与变换，
但具体接口和调用顺序仍委托给原生覆盖专题。
当前 Runtime 将填充路径 coverage 换算后传入绘制，
Recorder 的 `covered_text_clip` 依赖该声明；
这一通道不能直接表达原生 glyph 的操作级证明。[cite:1](#source-1)

修订应在本页写明实施准入条件，由原生覆盖专题维护完整合同：
谁生成静态墨迹资产，谁提供实际后端 policy，
Runtime 如何绑定 backend/session 与资源版本，
Recorder 如何在生成 IR 前按当前操作与变换查询，
以及实际 backend 如何保证采用被证明的 lowering policy。
后端状态不进入 source 或文字布局缓存，内置与外部实现使用同一准入通道。

静态字体墨迹和索引可以按不可变资产留存，
设备相关检查应按当前帧及实际变换进行。
不能为减少提交量而每帧先遍历全文 glyph 生成或重验全部证书。
明确子树摘要、候选范围及未知下降规则，并把已知、未知和退化路径分别计时。
允许未知或高度重叠的输入退化，不承诺任意输入都具有可见量复杂度。

**复核标准：** 能从静态资产追溯到本帧查询和实际 lowering；
没有循环依赖或后端状态写回；检查工作量纳入 H、C、G、V 及独立计时。
覆盖 session 更换、字体替换、操作变化、数值域失败与混合已知／未知范围。
对应合同未复核时保持完整原生输出。

## SR-R02：宽度复用的默认适配候选与成本

Parley 0.7.0 支持在 Layout 上重新执行 `break_all_lines` 和 `align`，
但 Layout 及内部 LayoutData 的派生 clone 会复制其中的 Vec：
包括 shaping run、cluster、glyph 和行结果。
当前 Novadraw 适配器还逐个导出 positioned glyph，
并构建独立交互 provider。[cite:2](#source-2)[cite:3](#source-3)

建议先定义最小可行性候选：保留不可变分析模板，
对每个宽度生成独立 Layout，重新换行、对齐并导出完整结果。
这只是一条待验证候选，不预先认定满足预算。
分别测量模板复制、换行扫描、对齐、位置导出、来源几何、
交互构建、校验和发布；失败前已完成的工作也计入。
若复制与导出已经占据主要预算，再评估更深的资产拆分或上游支持。

不得共同修改已发布的 Parley Layout，也不得把内部类型暴露为公共合同。
重复换行测试须覆盖宽度往返与 alignment 状态恢复，
再扩展到真实 inline 反馈和不支持分阶段能力的外部引擎。
本项不依赖跨编辑检查点恢复。

**复核标准：** 明确候选路径、状态所有权与剩余待证项，
提供上述阶段的测量协议和多宽度完整等价计划。
保留 resize CPU 完整精确稳定查询 p95 ≤ 16 ms 为建议目标；
减少 shaping 调用、缓存命中或单阶段加速不能证明该目标已经达成。

## SR-R03：阶段测量与推进顺序

既有记录中滚动 p95 为 7.187 ms，resize p95 为 36.673 ms，
resize 新增 styled layout 为 6 次；这些数据没有拆出 resize 的 shaping、
换行、导出及反馈成本，也没有证明六次调用都是重复 request。[cite:4](#source-4)

建议将阶段测量放在本轮最前，记录每次布局的段落身份、
完整受检 request、有效宽度、调用阶段与缓存结果。
区分不同段落、合法反馈宽度、measure／prepare 重入以及同 request 重算。
滚动分别记录贡献捕获、测量、签名、投影、依赖重验和发布。
不要用整条稳定查询时间替代某个子阶段。

建议推进顺序为：
阶段测量 → 共享 IR → 按实测选择原生裁剪或稳定化复用 →
宽度复用 → 有证据再推进调度和 GPU 缓存。
宽度复用的可行性评估可提前，不因原生覆盖合同尚未完成而等待。
这不改变单一 Execute 切片规则，也不新建执行队列。

**复核标准：** 工作包明确列出进入条件和待证假设；
收益判断能追溯到独立阶段和请求分母。
同 request 重用仍受完整输入、外部依赖、预算和发布校验约束。

## SR-R04：共享 IR 的完整链路与留存

当前 `draw_glyph_runs` 使用 `run.clone()` 录制命令，
`to_submission_for_session` 又 clone 整个 commands；
glyph 与 normalized_coords 为 Vec，因此两处均可能复制数组。[cite:5](#source-5)

共享资产合同应贯穿布局、录制、submission 构造和 backend 消费，
分别记录每个阶段的复制字节与分配次数。
不要求 backend 编码天然零复制，但应把它与 Core 数组复制分开取证。
可在同帧按完整字体实例去重重复资源检查，
仍校验当前资源版本及变量坐标；裁剪掉输出不能消除应保留的依赖语义。

范围视图可能用很小的片段钉住很大的 backing buffer。
建议按实际唯一 allocation 计费，单列可见引用字节、backing 字节、
缓存计费与旧 lease 留存；不能按视图长度低估资源。
划分资产粒度属于待评估实现选择，不改变旧快照不可变性。

**复核标准：** run clone 与 submission clone 均不重复复制 glyph 数组；
覆盖旧快照长期持有、缓存淘汰、来源删除、字体替换和 backend session 更换。
记录实际峰值留存，明确预算不能强制释放消费者仍持有的合法 lease。

## SR-R05：字形之外的输出与片段成本

当前 FlowSnapshot 的段落绘制之后仍遍历全部边框并生成状态与绘制命令。
仅减少 native glyph 数，不必然让完整 IR 成本随可见量收缩。[cite:6](#source-6)
多个不连续可见范围也可能减少 glyph 数却增加 draw run 数，
其 backend 编码收益仍需测量。

建议补充可见片段数、draw run 数、状态命令数、
边框检查数及 backend 编码量。
已知边框可在相应同源覆盖证明成立后剪枝，
未知边框保持完整绘制与原序；装饰同样按自身证明处理。
只有状态兼容且在原序中连续的范围才考虑合并，
不能跨字体、paint、变换或绘制顺序拼接。

**复核标准：** 固定可见查询＋完整 IR 指标包含这些成本；
保留 glyph／command 输出分母与范围碎片分布。
用长行、RTL、混合字体与跨字体装饰核验顺序和像素，
不以 selected glyph 下降单独关闭 4 ms 目标。

## SR-R06：几何消费屏障与分层验收

方案正确保留必须消费几何的事件顺序，但尚未冻结消费屏障。
若所有指针移动都要求立即完成昂贵布局，
连续拖动中的 resize 合并收益可能很小；这是待测假设。
当前 Native 宿主在 Resized 时同步逻辑 viewport 并请求 Full redraw，
昂贵准备发生在 render 路径，需先识别已有合并与剩余工作。[cite:7](#source-7)

建议明确实际几何消费操作如何形成屏障，
不让宿主凭事件名称猜测外部处理器是否需要几何。
验证 resize → move → resize → press、键盘／IME、
DPI 变化、最小化／恢复、focus 和 surface 重建等序列。
比较实际完成的宽度数、过时计算数与无合并 oracle 的输入结果。

验收须区分三层：

- 完整布局：尺寸、断行、来源映射、inline placement、caret／selection 等价。
- 裁剪 IR：保留原字体、位置、paint 与顺序，形成完整输出的合法有序片段；
  不要求裁剪后的全文 glyph 列表与完整列表相同。
- 实际绘制：与完整计算、完整绘制的实际 backend 像素对照。

连续交互目前只有采证要求，应另行冻结窗口线程阻塞、
错过刷新及停止输入后的最终收敛判据，具体阈值需依据目标刷新率与实测确定。
无法获取实际显示时间时明确 unavailable，不用 submit 或 Presented acknowledgement 替代。
各阶段 p95 不相加推算帧率。

**复核标准：** 合并屏障可执行且保持输入因果顺序；
三层 oracle 的比较对象与允许差异明确。
固定 CPU 目标、连续交互指标和真实显示终点分别判定。

## 修订 Agent 交接

修订前读取 AGENTS.md、CLAUDE.md、本记录、被审方案、
母方案、原生覆盖专题、增量排版提案及其 LTI 评审、ADR-051。
当前实际仓库位于 `/Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw`。
本记录是固定评审证据，修订响应写回方案，不重写原判断。

逐项处理 SR-R01～SR-R06，先补齐 P1 的准入条件与默认适配可行性安排，
再补测量顺序、完整提交成本和分层验收。
完整接口与算法仍归各自专题，本页只引用并规定实施准入，不复制第二份正文。
理想架构修订以目标合同和第三方参考为依据；
核验本记录中的实现事实时才定向读取对应 Novadraw 代码。

方案增加评审响应表，至少包含意见编号、正文定位、处理方式和剩余验证项。
状态写“已补齐设计响应，待复核”，不自行关闭意见。
证据不足时明确待证、完整回退与实施前检查，
不补造接口可用性、采样数据或通过结论。
本轮交接仅用于文档修订，不修改 Rust、不批准 ADR、不新增 Execute 切片。

可直接交给另一 Agent：

> 按 doc/verification/reviews/long-text-scroll-resize-review-2026-10-28.md
> 修订 long-text-scroll-resize-optimization-proposal.md，逐项响应 SR-R01～SR-R06。
> 必要时同步完善原生覆盖与增量排版专题，但保持正文归属。
> 优先明确覆盖证明进入 Core 录制前查询的合同、每帧检查成本，
> 以及 Parley 宽度复用候选的复制／换行／导出／交互测量。
> 前置阶段测量，补齐共享 IR 完整链路、留存计费、
> 非字形命令成本、几何消费屏障和三层 oracle。
> 增加评审响应表并保持待复核，不修改实现、ADR 接受状态或执行队列。
> 完成后运行 cargo xtask docs 与 git diff --check。

## 核验依据

<a id="source-1"></a>[cite:1]
[Runtime frame preparation](../../../crates/novadraw/src/runtime/runtime/frame_submission.rs)：
`prepare_submission_inner` 的 coverage 下传；
[Recorder](../../../crates/novadraw/src/render/context.rs)：`covered_text_clip`。

<a id="source-2"></a>[cite:2]
[Parley 0.7.0 Layout](https://docs.rs/crate/parley/0.7.0/source/src/layout/layout.rs) 与
[LayoutData](https://docs.rs/crate/parley/0.7.0/source/src/layout/data.rs)。
本机固定版本源码核对 Layout 的 Clone、`break_all_lines`、`align`，
以及 LayoutData 的 Vec 所有权；未运行适配实验。

<a id="source-3"></a>[cite:3]
[默认文字适配器](../../../crates/novadraw/src/render/text.rs)：
`ParleyTextEngine::layout_styled`、positioned glyph 导出与 ParleyTextInteraction。

<a id="source-4"></a>[cite:4]
[既有实施记录：原 Large 源提交观测补录](long-text-performance-implementation-2026-10-27.md#原-large-源提交观测补录)：
1000 次热样本及滚动、resize、IR 分母；未重新采样。

<a id="source-5"></a>[cite:5]
[Recorder](../../../crates/novadraw/src/render/context.rs)：
`draw_glyph_runs`、`to_submission_for_session`；
[GlyphRun](../../../crates/novadraw/src/render/text.rs)：Vec 字段。

<a id="source-6"></a>[cite:6]
[FlowSnapshot](../../../crates/novadraw/src/figure/flow/snapshot.rs)：
FigureDrawing 的段落索引查询与随后边框遍历。

<a id="source-7"></a>[cite:7]
[Native 宿主](../../../examples/support/src/app.rs)：
Resized、`request_surface_redraw`、`sync_logical_viewport`、`render`。

文档门禁仅验证类型、导航与引用等规则，不证明上述算法、性能或呈现目标通过。
