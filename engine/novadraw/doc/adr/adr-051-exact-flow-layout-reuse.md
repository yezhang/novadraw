# ADR-051：精确文字流布局复用

类型：`architecture-decision`

日期：2026-10-27。状态：Accepted（用户已批准 LT-00～LT-03；实施验收另记）。

## 目标契约

依据[修订 2 复核](../verification/reviews/long-text-performance-revision-2-review-2026-10-27.md)
实施精确 TextFlow 的同步优化。完整布局、来源几何和交互仍一起发布。
后台排版、渐进覆盖、紧凑文档来源与 glyph atlas 不属于本增量。

## 决策

文字布局的文本、行指标与 glyph run 存储采用不可变共享资产。
绘制分片只引用受检 run 范围；切片不重新验证全文，也不复制整段 glyph。
来源 fragment 索引随快照一次构建，来源准备使用该索引。
公开 presentation 提供共享数据读取，使外部 root 能保留同一快照。

Flow 服务持有段落缓存，测量与准备经过同一个受检入口。
引擎通过 `TextLayoutEngine::cache_identity` 声明实例身份与精确结果可复用合同；
缺省返回缺失，保留完整计算路径。内置与外部引擎使用同一个声明入口。
实例身份、完整 styled request 与文字引擎 revision 共同决定布局复用；散列只用于查找，
命中还比较完整输入，不以散列相等代替输入相等。
宽度保留在 key 中，不假定任意引擎支持 shaping 与换行分离。
贡献、边框度量和真实内嵌 Figure 测量仍读取当前场景；其依赖观察继续参与发布准入。
局部文字几何可以共享，页面 origin、来源身份和边框片段按当前贡献重新组合。
缓存有实例所有权和有限容量；失效或淘汰不修改旧快照。

测量与准备共用贡献读取、边框度量、真实内嵌 Figure 测量、完整 styled request、
文字引擎验证和行间距计算。测量只返回这些计算得出的尺寸，
来源 selection geometry、fragment、placement、边框绘制片段及墨迹索引由准备阶段生成。
内部构建结果用类型区分尺寸与完整快照，不发布缺少几何的 FlowSnapshot。
省去的输出不再占用其分配预算；保留计算仍受同一预算与失败传播约束。
不跨场景缓存完整测量结果：contributor 与 registry handler 没有纯度或 revision 合同，
每次测量仍读取其当前贡献。内置与外部能力共用此路径。

准备阶段以类型化 `DependencySubject` 索引去重计算观察，另保留首次读取的
有序记录及 generation。重复观察不覆盖首次 generation，缓存复用仍逐项重验；
两个容器在插入前共同预留预算，预算耗尽后不留下部分记录。
这只是既有观察日志的内部算法调整，外部 Figure、布局器与内置消费者共用。
段落边框组合只扫描本段生成的 fragment，来源、绘制顺序与最终几何不变。

同一 PreparationScene 内的树和 resolved style 由不可变借用保护；
源输入与含派生几何的签名分别缓存这部分基础计算。外部 preparation provider
仍在每次签名请求时重读 revision，预算失败仍立即返回错误。
缓存只活到这次只读 scene 结束，不跨越树修改、布局发布或下一次场景捕获。
同一 scene 按完整字体描述符复用解析结果，FlowContributionContext 的既有
text_style 入口让内置和外部 contributor 共用。颜色与来源仍读当前 resolved style，
字体资源选择与版本仍交给文字引擎；解析缓存不替代字体布局缓存。

OutlineTextEngine 按内部引擎身份、revision 和完整 request 留存独立 shaping 资产。
provider-only 更新只刷新轮廓与索引；内部引擎未声明缓存合同时仍完整计算。
字体注册或移除清空该缓存，缓存淘汰不使已发布的共享资产失效。

可见绘制使用实际轮廓的保守包络摘要，保持原绘制顺序。
未知轮廓与未知边框范围走不可剪枝路径，不用逻辑行范围代替墨迹范围。
有效 clip 由 Graphics 的当前变换与嵌套 clip 推导；不能可靠换算时返回未知。
只读几何 clip 不包含设备覆盖，消费者不得单独据此裁剪 AA 或原生 glyph。
后端通过 `BackendCapabilities::filled_path_coverage_outset_pixels` 显式声明
填充路径与 clip 的保守设备覆盖范围；缺省未知，Core 不启用该路径的 CPU 剪枝。
声明不覆盖原生 glyph hinting、描边或未知边框。
覆盖范围按 SurfaceInfo 的 scale factor 换算，嵌套 clip 先扩张再求交，
避免相邻子像素 clip 的几何空交被误认为设备覆盖也为空。
layout、ink、paint 与索引必须对应同次受检发布；已有源 mutation 不承诺失败回滚。
性能证据分别记录 H、C、G、V，允许重叠或未知范围退化为全扫描。
段落与字形分别建立保持来源顺序的层次摘要。
段落摘要区分空、已知与未知；未知节点继续下降，未知叶子完整绘制，
已知 sibling 仍可剪枝。未知边框保持原来文字之后的完整绘制顺序。

## 外部消费者证据

`draw2d-official-examples` 的 InteractivePage 使用公开 FlowLayoutService，
是独立 root 消费者。外部引擎、contributor、border 与 outline provider
仍使用原有开放协议和受检构造器；缓存不按内置类型选择路径。
外部契约测试须覆盖跨来源 shaping、bidi、内嵌测量、字体与轮廓 revision，
以及旧快照留存。通过情况只记入实施记录。

## 成本与验证

RuntimeFrames 提供默认关闭的实例计时开关与读取后清空的阶段记录。
仅同步内部阶段写入记录，外部与内置准备能力经过相同的计时边界。
记录包含子阶段的总耗时和扣除子阶段后的自身耗时；失败退出也结束计时。
诊断阶段名只用于展示，不承担行为身份。计时不修改 source、预算、布局或发布，
不能充当源提交 receipt、GPU 完成或屏幕呈现证据。关闭时不读取时钟或分配计时记录。

2026-10-28 测量增量：同一实例 recorder 留存完整受检 styled request 与来源 component
versions、root/paragraph owner、服务 build 次序、measure/prepare、inline prefix、
engine identity/revision、命中与失败结果；scene signature 使用已经取得的值，
诊断不重读外部 provider，不增加准备预算或依赖观察。
最多保留 4096 次请求，溢出显式报告 dropped；读取后清空，关闭诊断丢弃记录。
完整 request 用于分类，来源与依赖差异仍需独立核对，不能仅按 signature 或宽度判定重复。
`TextLayoutEngine::layout_styled_with_diagnostics` 默认委托原入口；
内置与外部均可通过 `TextLayoutDiagnostics::time` 报告同步细阶段，
无法拆分的阶段保持未知。Parley 的分析和 shaping 合并计时。

不增加字体、GPU 资源或第三方依赖。共享存储减少布局分片复制；
缓存增加留存内存，实施中记录容量、峰值和产物大小。
基准使用原 TextFlowLargeExample 工厂与固定追加输入，
保留 dirty patch、未跟踪源码、字体散列和整数纳秒样本。
CPU 稳定化不能作为窗口呈现证据。完整采样和像素反例通过前不关闭性能验收。

## 滚动共享 IR 的最小增量合同

2026-10-28 用户批准滚动与 resize 方案；本节冻结随后共享 IR 切片的边界，
实施与迁移完成情况只记入对应实施记录。

公开 `text::shaping::GlyphBuffer` 拥有不可变 glyph 数组，
owned 构造检查有限坐标；`GlyphRange` 持有同一缓冲与受检 glyph 索引范围。
范围构造检查起止次序与缓冲长度，拆分只校验范围，不再次复制或扫描全文数组。
空范围合法；索引属于 glyph 序列，不能冒充 source byte range。
读取保持原序；范围类型不提供可变数组入口。

`GlyphRun.glyphs` 改用 `GlyphRange`，
`normalized_coords` 改用不可变共享 slice；字体、size、skew 与绘制状态仍独立受检。
owned 数据仍可从统一公开构造入口进入，内置与外部引擎使用同一类型。
既有数值、字体资源、layout、device 和 submission 校验继续执行；
共享与范围合法性不替代这些校验。字体 import 可重映射目的身份，
但必须先验证精确资源版本与内容；不能因数组共享而省略 session 绑定。

布局、Recorder、command clone、submission 与外部 backend 都保留相同缓冲 lease；
两处 run/commands clone 不再复制 glyph 或 normalized coords 数组。
真实行位置改变时显式建立新缓冲，旧布局与旧提交保持不可变；
缓存淘汰、来源删除或字体失效不能释放外部仍持有的合法旧 lease。
backend 的编码转换独立计费，不声明全链路零复制。

迁移须覆盖所有 owned literal、逐 glyph 遍历、行距调整和存储预算。
读取采用 slice 语义；owned glyph 数据通过受检 buffer/range 构造，
owned normalized coords 转为共享 slice。Core 不暴露第三方 Layout 或后端类型。
公开 API 快照、Native/Web 构建边界及外部消费者须一并核验。

内存分别记录范围引用的 payload、实际 backing payload 与 metadata、当前帧、
缓存及旧 lease；同一责任域内重叠范围不重复计算同一 backing allocation。
范围钉住全文缓冲时必须计费全文，不能只按可见片段长度报内存。
保守准入预算与实际唯一 allocation 统计分别报告。

外部消费者证据要求：独立 engine 构造受检范围，
独立 backend 读取 submission 并保留旧 lease；校验非法范围、
不同字体/coords、缓存淘汰、source retire 与 session/font 变化。
GPU 对照保留当前原生输出和绘制顺序；共享成功不表示裁剪成功。
