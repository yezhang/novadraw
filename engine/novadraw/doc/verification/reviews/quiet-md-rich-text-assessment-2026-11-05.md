# quiet-md 富文本编辑技术适用性分析

类型：`architecture-review`

日期：2026-11-05。
能力归属：Editor 的富文本草稿、选择与编辑事务；Core 的文字排版、交互几何与增量协议；
platform 的系统输入桥接；应用的持久模型、格式转换与文档视图。
状态：静态研究完成；采用建议未授权实施，不改变现有执行队列。

## 分析范围与证据

研究对象为 [s1dashu/quiet-md](https://github.com/s1dashu/quiet-md)，
用户提供的入口是其架构文档。本次核对固定到提交
`937b254f8c4553e4998f83ce942675da558c743a`，读取了架构、模型、编辑内核、
文本渲染、虚拟化、IME、随机编辑测试、格式和性能说明。
仓库登记、许可证与源码阅读入口见
[工程样例参考页](../../reference/quiet-md-rich-text.md)。[cite:1](#source-1)

本报告整理本次研究结论，没有编译或运行 quiet-md，也没有扫描 Novadraw Rust 实现。
Novadraw 能力边界依据现有
[direct-edit 规范](../../design/editor/p2-direct-text-edit.md)和
[有限富文本提案分析](bounded-rich-text-proposal-assessment-2026-11-03.md)核对。
规范中的目标、实施状态和实际性能分别判断。

quiet-md 的行为来自同一仓库的文档与源码，不构成多个独立工程复现。
下文区分固定版本源码事实、作者历史测量和本报告的采用建议。
没有证据证明其算法在 Novadraw 中更快，或可以替代现有 Parley／TextFlow 链路。

## 适用性判断

quiet-md 最值得借鉴的是将样式、选择、结构编辑、行内对象和历史组织成可验证的编辑行为。
它可以作为有限富文本消费者的工程样本。长文档虚拟化也值得研究，但其估计高度和
屏幕外缓存暂时过期的选择，与 Novadraw 精确 TextFlow 的要求不同。[cite:1](#source-1)[cite:5](#source-5)

| 技术 | 借鉴价值 | Novadraw 归属与条件 |
|---|---|---|
| 文本与规范化样式 run | 高，参考编辑语义 | Editor 草稿／应用模型；存储算法另比较 |
| 样式继承、待输入样式 | 高，补充纯文本会话 | Editor；定义端点、混合选区和格式事务 |
| 行内对象、未知数据保留 | 高，适合后续对象研究 | 应用模型；Core 度量与几何单列合同 |
| 随机编辑、不变量、历史回放 | 高，可采用验证方法 | 各 owner 的定向验证；补足 Unicode 与会话覆盖 |
| 列表差分、滚动位置保持 | 有条件，适合文档消费者 | 应用；渐进布局需独立 Core 合同 |
| 整篇快照撤销 | 可作为正确性基线 | Editor；另测复制、历史留存与离散操作 |
| GPUI 自绘文本与交互缓存 | 可参考职责配合 | 消费既有 Core 几何与 platform 桥接 |

## 富文本模型与样式编辑

### 样式区间有明确的不变量

`RichText` 使用 `String + Vec<Run>`，run 保存 UTF-8 字节长度和 `Marks`。
所有 run 必须完整覆盖文本，不允许零长度 run；相邻相同样式由 `push` 合并。
`slice`、`replace`、`split_off` 和 `update_marks` 围绕这一表示操作。[cite:2](#source-2)

这一模型适合有限富文本的正确性基线：插入、删除和格式修改之后，文本与样式区间
仍须一致。Novadraw 可以借鉴这些不变量，避免将“能渲染 styled request”视为
“已经具备富文本编辑事务”。现有 direct-edit 首批草稿是纯文本；
样式编辑需要定义草稿内容、选择映射、操作与历史之间的合同。

### 输入样式继承比字段列表更值得借鉴

`marks_at` 明确处理输入位置的继承：位于 run 内部时允许链接和 inline code 延伸，
边界输入使用专门的 typing marks 规则；普通输入不继承行内对象属性。
编辑内核另持有 `pending_marks`，用于光标处切换格式后的后续输入。[cite:2](#source-2)[cite:3](#source-3)

后续 Editor 研究应明确空选区格式、端点插入的 affinity、默认样式继承、
混合选区状态，以及段落分割合并后的选择和样式映射。
quiet-md 提供了可读的行为样本，具体规则仍须与 Novadraw 的位置和会话合同核对。
CodeMirror 的事务与位置映射也可补充这一研究，但其 UTF-16 位置域不能直接代替
Novadraw 的 UTF-8 文档位置。[cite:9](#source-9)

### 编辑语义与存储性能分开采用

该版本的 `replace` 通过前缀、插入内容和后缀重建 block 内文本；
`update_marks` 遍历样式并构造新的 `RichText`。
局部修改因此仍可能复制整个 block，不能据此认定其已经提供局部存储增量。[cite:2](#source-2)

采用建议是先固定操作语义，再比较短标签、长文档和超长单段落的存储成本。
测量应覆盖复制字节、run 扫描、来源组装和向排版输入物化的工作量。
不能只因单个字段使用 `String` 或改成树，就推断端到端收益。

## 行内对象与格式往返

quiet-md 用 U+FFFC 表示行内对象的位置，样式中保存 `Inline { kind, xml }`。
`kind` 是解析后的含义，供视图与导出使用；`xml` 保留原始内容与未知属性。
未知 block 使用 `BlockKind::Raw` 保存。其 DocxXML 指飞书／Lark 文档 XML，
不能据名称推断它具备 Microsoft Word `.docx` 支持。[cite:2](#source-2)[cite:4](#source-4)

值得借鉴的是对象同时参与文档位置、选择和编辑，并维护加载、修改、保存的一致性。
未来公式、标签、图标或 inline Figure 的研究应明确对象前后 caret、原子删除、
复制导出和未知对象保留规则。格式与持久数据仍归应用，排版和几何依赖归 Core。

其文本渲染使用与 U+FFFC 相同 UTF-8 长度的替代字符，再调整对象占位与绘制几何。
这是一种应用实现，不能直接成为 Core 的对象排版合同。
Novadraw 应另定义对象的度量、baseline、断行、命中、覆盖与来源映射，
并继续使用开放扩展模型；封闭 `InlineKind` 枚举不作为框架扩展方案。[cite:6](#source-6)

原始数据保留也不等于完全无损。格式说明明确记录：单元格中带嵌套子级的 block，
其尾随空格或 Tab 可能在读回时丢失；fuzzer 为此做预处理。
采用往返验证方法时，应保留并显式列出同类例外。[cite:4](#source-4)[cite:8](#source-8)

## 文档虚拟化与缓存失效

### 可借鉴列表差分与滚动位置保持

`sync_items` 按文档构造列表条目与 key。依赖未变化时提前返回；
变化后比较新旧 key 的公共前缀和后缀，用 `splice` 替换中间区间。
顶部条目受替换影响时，保留条目内滚动偏移，减少页面跳动。[cite:5](#source-5)

这三项目标适合文档型消费者：减少无变化时的遍历、局部更新视图投影、
保持用户正在阅读的位置。但文档变化后仍会重建条目和 key；
多个分散修改之间的中间区间也可能被一起替换。
文本等部分嵌套 block 独立成行，容器子级不一定独立虚拟化。
不能把它概括为任意结构下只处理最小变化集合。

### 不完整 key 依赖估计几何模式

`hash_block` 读取文本长度、run 数量和部分结构属性，不比较完整文本与样式。
源码明确解释：可见行会重新测量，同长度修改可能让屏幕外高度暂时过期。
滚动条对未测条目使用估计高度。[cite:1](#source-1)[cite:5](#source-5)

在精确 TextFlow 中，等长字符或相同 run 数量不保证度量相同。
布局身份必须保留完整依赖，或由可靠的局部 revision 和变更传播证明复用安全。
估计高度、懒测量和滚动锚定可以进入独立渐进模式，
但应声明未测区域的查询限制以及估计尺寸对父布局和连接消费者的影响。
CodeMirror 官方也明确说明，未渲染的视口外位置不能提供坐标查询。[cite:9](#source-9)

quiet-md 的列表投影和测量复用不等同于 Novadraw 的整页来源组装复用。
本次研究不能证明 FR-02 收益，其已有证据范围见
[基线核对](fr02-review-baseline-2026-11-04.md)。

## 撤销、交互几何与 IME

### 快照可作为正确性基线，成本取决于操作边界

`checkpoint` 在新历史步骤开始时复制 `Document`。
同 block、同编辑类型的连续输入通常按 700ms 时间窗口合并；
撤销和重做恢复文档与文本选择，同时保留部分历史之外的属性。[cite:3](#source-3)

这意味着连续输入测试不是每次按键都复制整篇文档。
它也不能代表频繁开启新步骤的格式操作、跨 block 编辑或历史恢复成本。
其历史上限为 500 步及约 256MB，但至少保留 20 步的策略使字节预算不是绝对上限。
后续比较应包括新步骤复制、undo／redo、历史保留内存和超长单段落。

Novadraw 继续区分会话内历史与接受后的全局 CommandStack。
取消不生成持久模型历史，接受时仍核对来源 revision。
快照、结构共享和操作日志是可比较实现，不因 quiet-md 采用快照就确定默认方案。

### 共用排版几何值得保留，平台类型与会话状态仍分层

`TextLayout` 为绘制、命中、caret 导航和选择提供共用几何；
`EntityInputHandler` 通过 `to16`／`from16` 转换平台 UTF-16 范围和编辑 UTF-8 位置，
处理 marked range，并从布局缓存查询候选窗几何。[cite:6](#source-6)[cite:7](#source-7)

这些路径可作为交互测试场景参考，但其 `Pos` 不携带 Novadraw 的显式 affinity，
不能据此证明 bidi 和 soft-wrap 边界行为满足本项目要求。
其 preedit 会直接修改编辑文档；Novadraw 仍由 Editor 会话持有草稿与 composition，
platform 执行系统输入效果，Core 提供与 layout revision 一致的不可变交互几何。

## 随机编辑与差分验证

fuzzer 从包含样式、嵌套结构、表格和行内对象的文档出发，随机生成编辑操作，
逐步检查 block 身份、run 覆盖、结构、选区和 generation。
随后检查保存／加载／保存的稳定性及完整撤销／重做。
失败输出 seed 和操作轨迹，支持重放。[cite:8](#source-8)

这一方法值得优先采用，因为富文本缺陷常出现在连续操作组合中：
单独正确的格式修改、删除和段落合并，组合后仍可能破坏样式或选择。
不过其检查使用 UTF-8 字符边界，不能当作 grapheme、bidi、视觉 caret 或 IME 的完整覆盖。

建议补充三类验证：文本与样式事务的组合不变量；
全量正确路径与增量结果的差分；会话提交、取消和过期事件的状态验证。
Unicode 样本应覆盖组合字符、Emoji、连字和双向文本。
随机负载明确标为测试输入，不能冒充实际用户分布。

## 性能证据与复现边界

作者性能说明记录了 2026-11-03 的历史测试：
源码 `0e92b51`，release 构建，Apple M5 Pro／48 GiB、macOS 26.5，
1280 × 800 pt 窗口与 2× 缩放，生成文档包含 1,001,307 字符、549 章。
这些条件属于作者报告，未独立复现。[cite:10](#source-10)

| 操作 | 帧处理 P50 | 帧处理 P95 | 最大 |
|---|---:|---:|---:|
| 连续输入 | 5.06 ms | 5.78 ms | 6.94 ms |
| 连续退格 | 4.99 ms | 5.55 ms | 5.69 ms |
| 连续滚动 | 2.08 ms | 4.14 ms | 9.23 ms |

测试在首段末尾输入 100 个 `x`、退格 100 次，再滚动 100 次；
每次操作后同步调用 `window.draw`。结果是应用内帧处理耗时，
不包含屏幕呈现延迟，也不是输入到屏幕的端到端延迟。
不能将历史版本测量与本次源码快照合并，或用于计算 Novadraw 的提速比。[cite:10](#source-10)

值得借鉴的是固定配置、脚本与测量终点，分开记录操作和绘制阶段。
Novadraw 的后续实验还应覆盖超长单段落、密集样式、离散格式修改、IME、
大量短节点与单一活动会话；计量复制、来源组装、重排、IR、提交及最终呈现。
连续输入分位数不能代替新历史步骤和最坏操作的成本。

## 后续采用建议

1. 先定义有限富文本的样式事务、输入继承和位置映射，沿用 Editor 草稿与应用模型边界。
2. 建立 seed 可重放的随机编辑验证，并用全量路径检查增量正确性。
3. 研究行内对象的数据与编辑语义；所需 Core 度量和几何作为独立合同增量。
4. 在代表性负载上比较存储、历史和来源物化成本，再选择数据结构。
5. 将文档虚拟化与渐进布局单独研究，保留估计几何和查询限制。

以上是研究优先级建议，不建立第二份执行队列。
实际推进仍以 [P2 backlog](../../roadmap/p2-delta-backlog.md)为准；
外部应用只能提供工程机制参考，不作为 Draw2D／GEF 对等语义的新来源。

## 来源

<a id="source-1"></a>
1. [quiet-md 架构文档，固定提交](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/docs/ARCHITECTURE.md)。
<a id="source-2"></a>
2. [模型与富文本操作](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/model.rs)。
<a id="source-3"></a>
3. [编辑内核与历史](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/editor/core.rs)。
<a id="source-4"></a>
4. [格式、未知数据保留与已知限制](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/docs/FORMAT.md)。
<a id="source-5"></a>
5. [列表差分与缓存身份](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/render.rs)。
<a id="source-6"></a>
6. [文本排版、交互与行内对象显示](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/text.rs)。
<a id="source-7"></a>
7. [编辑视图与 IME](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/view.rs)。
<a id="source-8"></a>
8. [随机编辑、不变量与历史回放](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/editor/fuzz_tests.rs)。
<a id="source-9"></a>
9. [CodeMirror 官方 guide：事务、位置映射与 viewport](https://codemirror.net/docs/guide/)；
   本次读取，在线文档未固定版本。
<a id="source-10"></a>
10. [quiet-md 历史性能测试说明](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/docs/PERFORMANCE.md)；
    文档中的实测源码为 `0e92b51`。
