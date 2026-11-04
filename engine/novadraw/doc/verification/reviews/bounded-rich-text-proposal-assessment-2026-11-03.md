# 有限富文本技术提案在 Novadraw 中的适用性分析

类型：`architecture-review`

日期：2026-11-03。
能力归属：Core 的排版、几何与增量协议；Editor 的富文本编辑会话；
backend 的 glyph 编码与 GPU 资源；platform 的系统输入桥接。
状态：提案分析与合同核对完成，建议未授权实施；不改变现有执行队列。
`api_semantics`：`text.flow`、`text.interaction`、`viewport.scroll_zoom`、
`notification.layout_update`、`paint.protocol`、`render.backend_session`。

## 分析范围与依据

输入为用户提供的《wgpu 海量有限富文本编辑框技术提案》，原件位于
`/Users/bytedance/Downloads/wgpu海量有限富文本编辑框技术提案.md`。
SHA-256：
`34425666f6475fdbf7c92a87cf16c64827418d2d69452fe9d09e09cded73b891`。
本分析保留原件，不改写或移入现行规范目录。

核对依据为已接受 ADR、Editor 专题、已记录的性能证据及提案引用的官方工程资料。
没有扫描项目 Rust 实现，也没有运行新的算法基准。
下文“已有”指被对应契约或实施报告明确支持的能力，不把 ADR 接受等同于实现完成。
性能收益、数据结构优胜关系和新缓存策略均未实测。

外部资料只用于验证工程机制与限制，不作为 Draw2D／GEF 对等语义的新来源。
本分析不引入第三方编辑器代码，也不提出替换现有 Parley／文字引擎链路。

## 适用性判断

提案最有价值的部分是将局部修改、样式变化、布局失效、位置索引和可见输出联系起来，
并要求按实际操作检查工作量。它适合补充 Novadraw 的有限富文本编辑与增量研究，
但其“从零建设独立 wgpu 编辑器”的模块分工不适合直接移植。

Novadraw 已有统一 Graphics、TextLayoutEngine、TextFlow、位置几何查询、
Editor direct-edit session 和 backend submission 边界。新增工作应复用这些入口，
避免在 Editor 或应用中新建另一套字体上下文、换行、命中与坐标转换。
依据见 [ADR-025](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-025-unified-graphics-and-glyph-preparation.md.txt)、
[ADR-051](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-051-exact-flow-layout-reuse.md.txt)和
[Editor direct edit](../../design/editor/p2-direct-text-edit.md)。

| 提案技术 | 项目判断 | 能力归属 | 应用条件 |
|---|---|---|---|
| 局部文本读取、分块存储、聚合索引 | 适合隔离比较，暂不指定 piece tree 为默认 | Editor 编辑数据结构；应用模型持久化；Core 来源接口另评审 | 先验证文本物化、来源组装和小文本成本 |
| 连续样式区间、规范化与事务 | 适合有限富文本增量 | Editor／应用模型 | 定义继承、插入样式、区间映射和混合选区规则 |
| 段落精确缓存、共享 glyph 数据 | 已有基础，应复用并补缺口 | Core | 保留完整依赖、预算和旧快照生命周期 |
| 宽度变化复用 shaping | 与 ADR-059 方向一致，尚不能宣布生产路径完成 | Core／文字引擎 | 维持 B1 暂停与现有阶段状态，另核对接入成本 |
| 颜色与排版属性分级失效 | 适合单列合同增量 | Core 定义依赖；Editor 产生修改 | 不能直接从现有请求 key 删除颜色 |
| 精确高度／宽度聚合索引 | 值得研究 | Core 布局与查询 | 保持全文尺寸、浮点聚合、墨迹与发布一致性 |
| 估计高度、懒布局、滚动锚定 | 后置到独立渐进模式 | Core 新合同；应用选择模式 | 未测区域明确未知，不提供虚构几何 |
| 可见行／run 输出、overscan | 优先研究可见 IR | Core 选择输出；backend 验证覆盖 | 需要保守墨迹与设备覆盖依据 |
| glyph atlas、实例化四边形 | 后端候选，暂不设为默认路线 | backend | 可见 IR 完成后，仍有编码／GPU 瓶颈才对照 |
| 事务化输入、IME、caret 与 selection | 复用会话基础，扩展富文本事务 | Editor；platform 桥接系统能力 | 保留 lease、冲突、取消与模型提交语义 |
| 随机编辑、差分验证、分层基准 | 可直接采用验证方法 | 各 owner 的定向验证与应用研究入口 | 增加海量短节点负载，固定配置与终点 |

## 数据结构与编辑事务

### 分块存储适用，piece tree 优先级需要证据

提案 §5.1 的局部读取与子树聚合值得采用。VS Code 的官方实践同时指出，
实际收益取决于运行环境、调用方式和负载，小文本的简单行数组也可能更快。
因此不能把其 JavaScript 大文件结果直接解释为 Rust 图形编辑框的默认选择。
参见 [Text Buffer Reimplementation](https://code.visualstudio.com/blogs/2018/03/23/text-buffer-reimplementation)。

本项目应分别比较大量短节点与单个长文本域。短标签不宜被迫为每个字段建立复杂树和
独立的大型缓存；长文本可比较 piece tree、rope／分块平衡树与现有存储。
首个实验应计量插入删除、局部读取、索引、metadata、历史留存和向排版输入物化的成本，
再决定是否需要统一存储表示或可替换的来源协议。

文本树和撤销历史的内存要按实际 backing storage 计费。
被删除的文字可能仍被历史或旧快照引用；只统计当前文本长度会低估占用。
提案“避免全文快照”应理解为避免无界复制：具有结构共享、有限历史和明确生命周期的
不可变快照仍是可比较方案，不能仅因称为 snapshot 就排除。

仅替换 String 不足以解决端到端问题。如果 Core 每次测量仍把分块内容重新拼成完整
段落并组装所有样式，存储节省会被外围工作抵消。
[重复工作分析](flow-runtime-repeat-work-analysis-2026-11-03.md)已记录，
原冻结 Large resize 存在 520 次段落请求、514 次命中和 13 次贡献组装。
这是既有运行证据，不能解释为本次对新数据结构的性能测量。

### 样式区间需要编辑语义与位置映射

提案 §5.2 的区间合并、样式去重、格式事务和统一撤销适用于新增 Editor 能力。
还需补充：空选区的后续输入样式、区间端点插入的 affinity、段落分割合并、
默认样式继承、混合选区状态，以及文本变化后样式和选择的位置映射。
[CodeMirror 的事务与位置映射](https://codemirror.net/docs/guide/)提供了工程机制参考，
其 UTF-16 偏移不能直接作为 Novadraw 的 UTF-8 文档位置。

应用模型继续拥有持久内容；Editor 会话持有草稿及其文本／样式事务。
会话内撤销与接受后的全局 CommandStack 必须定义衔接，取消不生成模型历史。
默认不随每次键入修改业务模型；若以后需要即时提交，应另定义提交策略。
当前 direct edit 首批是纯文本草稿，富文本样式事务属于新 Editor delta，
不能由“已有 TextFlow 支持 styled request”推断其已经具备。
依据见 [direct-edit 状态与提交合同](../../design/editor/p2-direct-text-edit.md)。

样式身份、Request 和扩展操作继续使用项目的类型化身份与注册机制。
应用可以限制允许编辑的属性，框架不能为内置操作建立闭合枚举或特权路径。

## 排版与绘制失效

### 颜色只影响绘制是目标合同，不是当前实现结论

提案 §5.2／5.3 将纯颜色变化排除在 shaping 与换行失效之外，方向适合普通文本
前景色修改。但 [ADR-059](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt) 第一片的分析请求
只去掉 max_width，保留颜色、span 范围与顺序等其他字段；
ADR-051 的布局复用仍比较完整受检输入。

实现这一优化需要区分 shaping 依赖、断行依赖、paint 依赖和来源映射。
复用旧 glyph／行布局时，应重新组合当前颜色与绘制状态，保留完整输出的正确身份。
单纯从 key 删除颜色可能返回旧颜色，或忽略外部引擎的真实依赖。
外部引擎未声明依赖时继续完整计算；新的拆分须单列 Core 合同和外部消费者验证。

同理，caret／selection 变化不应重新 shaping 是可验证目标，
不代表整个 Runtime 稳定化、来源几何和 IR 录制已经没有重复工作。
验证应同时检查这些阶段，不能只报告文字引擎 cache hit。

### 段落增量与宽度复用是两条不同研究线

宽度变化时文本和 shaping 输入未变，可按引擎声明复用分析结果并重新断行；
字符编辑则可能改变分割、字体 fallback、连字、双向顺序和断行机会。
二者不能共用“只改一小块，所以其余结果安全”的推断。

段落可作为第一层失效单位；超长段落必须单独测量。
安全 shaping 边界与断行收敛需要依赖证据，不能在样式边界、grapheme 边界或视觉行边界
任意切分。无法证明局部安全时计算到段末，并保留最坏情况和退化计数。
该方向已在[增量排版提案](../../design/architecture/long-text-incremental-layout-proposal.md)讨论，
其提案状态不代表实现完成。

### 可见输出值得优先，图集应由后端证据决定

提案 §5.5 的目标是减少不可见内容的实例生成，这与本项目可见 IR 研究一致。
但 `GlyphRun`／IR 命令不能直接等同于 GPU draw call；Vello 默认已经有自身 glyph
编码与缓存路径。[ADR-025](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-025-unified-graphics-and-glyph-preparation.md.txt)
明确保留 Vello draw_glyphs，并将预处理格式和 GPU buffer 放在 backend-local 合同中。

应先按可靠覆盖筛选可见 run／glyph，复用共享范围，减少 CPU IR 与后端编码输入。
line box、advance 或固定 overscan 不能替代斜体、负 bearing、描边及设备 AA 的覆盖证明。
未知覆盖必须完整输出，且保持绘制顺序；不能为了合批按 atlas／pipeline 任意重排
重叠文字、背景与装饰。依据见 ADR-051 和现有[可见绘制研究](../../design/architecture/long-text-incremental-layout-proposal.md)。

覆盖位图 atlas 可作为独立 backend 实验，与默认 Vello 路径对照。
实验需包含缩放、DPI、子像素、AA、实际字体变体、缓存淘汰、旧帧资源与 device/session
重置，并保留原有文字度量、描边和曲线路径。布局成功不能掩盖上传或提交失败。
普通覆盖 glyph 可将前景色放在绘制状态；彩色字形和 palette 参数要单独明确依赖。
本次没有证据证明图集比当前 Vello 更快。

## 高度、视口与框架语义

提案 §5.4 把准确高度与估计高度混用。[CodeMirror 官方 guide](https://codemirror.net/docs/guide/#viewport)
明确指出，视口外未渲染的位置不能提供坐标查询，全文高度先估计、显示时再测量。
这个限制是技术适用边界，不能只借鉴其滚动流畅度而省去查询限制。

Novadraw 的精确 TextFlow 同步合同要求完整布局与交互几何一起发布。
建议先研究精确行／段落高度的聚合索引和前缀查询。
浮点高度聚合需固定顺序、误差与溢出规则，不能把整数树摘要的结合律直接搬到 f64。
布局高度索引和墨迹重叠索引也应分别维护。

估计高度、滚动锚点与懒布局可进入独立渐进模式。其结果必须声明覆盖范围，
未测区域不能产生精确 caret、hit、全文最大宽度或 accessibility 几何。
在图形编辑框架中，全文尺寸可能影响节点 autosize、父布局、连接锚点和重路由；
这些消费者必须明确是否接受估计。已有[增量提案](../../design/architecture/long-text-incremental-layout-proposal.md)
将该能力放在独立渐进合同，而不是当前精确模式的隐式优化。

提案还需增加两个层级的可见性：画布上的文本节点是否可见，
以及长文本节点内部的行／glyph 是否可见。单个长文档的高度树不能替代画布级索引，
也不能证明大量短节点的布局、共享字体缓存和活动会话成本已经受控。

## 能力收窄与输入桥接

提案 §3.2 排除 bidi、复杂文字和跨样式精确 shaping，不适合作为 Core 能力收缩。
ADR-051 的外部消费者与验证要求包含跨来源 shaping 和 bidi。
有限富文本应用可以声明首批编辑能力，但已有 Core 对等语义及渲染正确性应保留。

即便只支持中英文和常见 Emoji，也不能把颜色区间等同于 shaping run，
或把 grapheme、cluster 和 glyph 当作同一单位。
[HarfBuzz cluster 文档](https://harfbuzz.github.io/working-with-harfbuzz-clusters.html)
说明重排、合成、分解与替换会改变 cluster 映射；
[UAX #29](https://www.unicode.org/reports/tr29/)定义的是用户感知字符等分割边界，
不提供全部 shaping 安全边界。规范和测试数据应固定版本，与引擎使用的数据匹配。

提案中“全部自研命中和坐标转换”应改为消费 Core 的文本交互 map 与坐标服务。
Editor 负责动作、选择与会话仲裁，platform 负责输入事件、候选窗和系统能力。
Web 应保留当前受控不可见 input host：它承载 IME／软键盘与事件校准，
不作为可见文本或业务模型真值。禁止可见 contenteditable overlay 不等于禁止这个桥接。
依据见 [direct edit 的平台合同](../../design/editor/p2-direct-text-edit.md)。

剪贴板与 accessibility text provider 是当前 direct edit 首批之外的能力，
需作为 Editor／platform 增量定义格式转换、异步结果 session 校验和语义查询。
大小写转换建议后置；若作为文本编辑事务，需处理文本长度与位置变化。
非破坏性显示转换还需要 source-to-display 映射、复制语义和多位置 caret 规则。

## 建议的研究顺序与验收

以下是评审建议，不新建第二份执行队列，也不恢复 ADR-059 B1 扩张。
当前推进状态仍以 [P2 backlog](../../roadmap/p2-delta-backlog.md)为准。

1. 在既有研究入口补齐修改、捕获、组装、布局、IR、提交的工作量账本；
   按当前队列先形成 FR-02 贡献复用合同，保持外部 contributor 的依赖验证。
2. 定义有限富文本数据、样式和事务合同，再隔离比较文本存储与位置映射，
   证明接入 Core 后仍避免不必要的全文物化。
3. 评审 paint-only 属性依赖与原生 glyph 覆盖，分别验证缓存复用和可见 IR。
4. 在明确的编辑工作负载上研究安全局部 shaping、断行收敛与精确高度索引。
5. 仅在必要时再推进渐进布局、后台任务及 atlas；各自拥有独立正确性合同。

| 场景 | 必查工作量／行为 |
|---|---|
| 原 Large 首次、追加、滚动与 resize | 捕获、贡献组装、cache hit/miss、IR glyph 与提交终点 |
| 大量短节点，少量可见，单一活动会话 | 节点遍历、非活动文本重排、字体共享、每节点 metadata |
| 长文档头／中／尾随机编辑 | 文本物化字节、受影响段落、位置映射与事务一致性 |
| 超长段落、密集样式 | shaping 扩张范围、断行扫描、最坏退化与累计工作 |
| 颜色／字号／字体变化 | 不同失效阶段、当前绘制属性、字体版本与 fallback |
| grapheme、连字、bidi、IME | 全量路径差分、caret affinity、提交与取消、旧 lease 拒绝 |
| 精确索引／未来渐进模式 | 全文尺寸对照、锚点稳定、未测区域查询与原子发布 |
| 缩放、DPI、裁剪、设备重置 | 墨迹覆盖、共享资源存活、绘制顺序和失败传播 |

[TextFlowLarge 研究应用](textflow-large-app-implementation-2026-11-03.md)可提供固定原场景的
CPU／Native 观测基础，但它的研究帧请求 full redraw，不能直接代表默认增量 damage。
富文本事务、海量短节点和最终局部重绘需各自补充真实消费者场景。
算法实验使用全量正确路径差分；新增随机负载应明确标为测试输入，不冒充实际用户分布。
性能对照固定字体、构建配置、diagnostics、viewport 与终点，记录退化和 dropped。

## 证据效力与待证问题

现有 owner、精确布局、平台输入和后端边界由项目合同直接支持；
原重复捕获与组装由既有运行报告支持。
VS Code、CodeMirror、HarfBuzz 和 Unicode 资料确认相应机制及限制，
不构成在 Novadraw 中的提速证明。

尚未实测：piece tree 与 rope 的优胜关系、颜色依赖拆分收益、
精确高度索引的维护成本、增量 shaping 的覆盖率，以及 atlas 对 Vello 的优势。
这些问题分别需要数据结构对照、完整依赖验证、全量结果差分和端到端匹配采样。
本分析没有据此确认新的 Core 必要能力缺失；若后续确认 Editor 缺失 Core 依赖，
需按项目门禁补全 GEF → Draw2D → Core 证据并单列增补建议。

## 主要来源

- 项目后端与 glyph 所有权：[ADR-025](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-025-unified-graphics-and-glyph-preparation.md.txt)。
- 项目精确布局、共享 glyph 与覆盖边界：[ADR-051](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-051-exact-flow-layout-reuse.md.txt)。
- 项目阶段分析请求与复用条件：[ADR-059](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt)。
- 项目编辑状态与平台输入：[direct-edit 专题](../../design/editor/p2-direct-text-edit.md)。
- 性能证据：[Flow／Runtime 重复工作分析](flow-runtime-repeat-work-analysis-2026-11-03.md)、
  [FR-01 实施与采样](flow-runtime-input-reuse-2026-11-03.md)。
- 候选增量算法与渐进模式边界：[增量排版提案](../../design/architecture/long-text-incremental-layout-proposal.md)；
  该文档不是已接受规范。
- VS Code 官方工程记录：[Text Buffer Reimplementation](https://code.visualstudio.com/blogs/2018/03/23/text-buffer-reimplementation)。
- CodeMirror 官方文档：[System Guide](https://codemirror.net/docs/guide/)。
- HarfBuzz 官方文档：[Working with HarfBuzz clusters](https://harfbuzz.github.io/working-with-harfbuzz-clusters.html)。
- Unicode 官方规范：[UAX #29](https://www.unicode.org/reports/tr29/)。
