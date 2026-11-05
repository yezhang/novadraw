# Draw2D 文本优化算法采用

类型：`proposal`

能力归属：Core Text／Figure。字体指标缓存归默认 Parley 引擎，截断搜索归 Core
私有文本算法，Label 与基础 TextFlow 共用。Editor、backend、platform 无新增能力。

日期：2026-11-05。用户已授权采用适用于本项目的 Draw2D 优化，局部实现与成本对照已完成。
本切片归入既有 FR-02，不改变公开接口、不可变快照或实验开关。
实施和测量结论见[验证记录](../../verification/reviews/draw2d-text-algorithm-adoption-2026-11-05.md)。

`api_semantics`：`text.flow`、`text.interaction`、`widgets.basic`、`graphics.context`。
现有语义入口为[text-layout](text-layout.md)和
[Draw2D API 账本](../../parity/draw2d/api-coverage.md)。

## 适用范围

| Draw2D 方法 | 本项目处理 |
|---|---|
| Label 测量、截断结果缓存 | 已有 intrinsic／presentation 缓存，继续保留；搜索中保存最佳已测布局，避免最后重复排版 |
| FlowPage 三宽度尺寸缓存 | 已有段落精确请求缓存和准备阶段测量缓存；独立多宽度缓存仍遵循 assembly 声明、输入证明和预算合同，不直接默认启用 |
| 重排时复用 TextFragmentBox | 已发布 FlowSnapshot 可被外部消费者持有，不原地改写其片段。截断搜索的临时字符串可在单次调用内独占复用 |
| 平均字符宽度预测、真实测量收敛 | 真实 Arabic 混排反例显示前缀宽度不单调；保留原二分探测顺序，采用已测候选复用，不默认加入预测 |
| 按 clip 跳过片段 | 已有段落和 glyph 包围盒索引；继续保留未知覆盖、描边等回退条件 |
| 共享 TextLayout／GC、减少字体设置 | 已有实例持有的 Parley 上下文和字体资源缓存；增加当前 descriptor 的字体指标缓存，禁止照搬全局 GC／singleton |

参考源码仅只读：`org.eclipse.draw2d/Label.java`、`text/FlowPage.java`、
`text/TextLayout.java`、`text/FlowUtilities.java`、`TextUtilities.java`、
`text/TextFlow.java`、`FigureUtilities.java`、`SWTGraphics.java`。
换行继续由 Parley 的 cluster advance 和断行规则处理，不引入第二套换行器。

## 截断搜索

输入仍为 source、字体、约束和既有 ellipsis。搜索候选只在 Unicode grapheme 边界
截取，使用同一个可变字符串缓冲区拼接前缀和 ellipsis。
测量回调仍调用消费者提供的 TextLayoutEngine，不增加内置引擎特权路径。

首轮尝试从完整文本宽度和 grapheme 数计算平均 advance，按“剩余宽度／平均 advance”
预测下一位置。真实 Arabic 混排测试发现前缀宽度受连接字形和尾部空白影响，
不保证单调；探测位置变化可能改变原来的截断边界。
外部 TextLayoutEngine 也没有提供前缀单调性证明。因此采用原二分顺序，
不按引擎类型增加特权预测路径。基础 TextFlow 行数截断共用缓冲区与最佳候选保留。

搜索保存二分过程中当前获选的前缀及其布局。结束后直接使用保存的布局，
不再重新 shape 同一前缀。空前缀的 ellipsis、Label 宽度不足以容纳 ellipsis、
TextFlow 零行、visible range、full width 和错误传播维持原语义。
TextFlow 延迟准备空前缀，只有搜索没有任何获选布局时才测量 ellipsis。
搜索保留既有非单调文本的结果，不声称总能找到任意上下文文本的全局最长容纳前缀。

## 字体指标缓存

ParleyTextEngine 独占一个最近 descriptor 和成功 FontMetrics 的条目。
descriptor 完全相同且字体集合未变化时直接返回指标；字体注册、替换或移除导致
上下文重建时清空。失败不写入缓存，非法字体替换不破坏已有成功状态。
显式 resolved_font_metrics 继续校验 run 的资源版本，不通过该缓存绕过校验。
单条目避免长期保留任意多字体请求；返回值仍为原来的 owned FontMetrics。

## 验证与性能判断

先保存修改前 release 成本样本，再在相同配置下采集修改后样本。
Label 样本覆盖均匀、不均匀和 Unicode 混排，分别使用完整宽度的 10%、50%、90%；
同时记录 layout 调用次数和局部总耗时。字体样本重复查询同一 descriptor。
计时测试单线程、关闭 tracing，不与其他构建或测试并行。

正确性用单调案例的独立前缀枚举和非单调案例的旧二分对照验证 grapheme 边界、ellipsis、visible range、
完整布局及错误；缓存验证字体变化、失败恢复和实例隔离。
既有 Label、基础 TextFlow、字体装饰及统一 Graphics 消费者运行定向回归。

局部测量只证明截断与指标查询的成本。结构化长文本滚动通常不调用 Label 截断，
不能据此宣称拖拽、resize、GPU 或屏幕延迟改善，也不关闭 FR-02、LT 或平台门禁。

## 第二批采用范围

2026-11-05 用户追加授权实施等值修改保护、Flow 临时工作区复用和默认 Parley
分析／重排分离。三项仍归 Core，按顺序在 FR-02 内推进，不另建执行队列。

等值修改保护首先覆盖 Label 的文本、图标、placement、三个 alignment 和 gap。
合法输入与现值相同时保留 intrinsic／presentation 与文本 revision；
不同值沿用原失效范围，非法 gap 仍先报错。此保护不删除 Runtime 的 mutation
通知合同，也不把已发布快照变成可变布局。

Flow 工作区只保存一次组装期间独占的 ancestor stack、visited set 和 source offset map
的容量，不保存内容、布局、用户 handler、场景引用或发布输出。
每次借用前清空逻辑状态，成功或错误返回都清空。容量按服务实例持有，
服务 clone 共用同一串行借用；独立服务不共享。保留容量纳入既有留存账本与
cache limit，超限时释放工作区，准备预算和结构检查不因命中容量而省略。

默认 Parley 分离沿用[分阶段协议](staged-text-width-reuse.md)：
宽度无关分析资产不可变，当前宽度的重排状态独占，最终完整输出按原合同验证。
普通 `builder.build` 不能冒充受准入分析。
当前生产缺口见[分析准入方案](text-analysis-admission-proposal.md)与
[阶段状态](../../verification/reviews/text-stages-status.md)，包括 Fontique 选择、
Swash parser／字体表 helper、复杂 shaping 和 Core 导出。逐项证实准入前不启用，
不以缓存接口、测试引擎或已预热字体替代生产证明。

每项冻结上一版产物，局部计数与 release 独立计时分别验证。
Flow 项另使用匹配的拖拽、滚轮和连续 resize 回放及 Perfetto 归因；
无屏幕呈现证据时继续明确离屏 GPU 的限制。未完成下层准入时，
第三项必须保留未完成状态并给出具体恢复条件。
