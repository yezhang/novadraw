# TextFlowLarge 打开耗时分析

类型：`verification`

日期：2026-10-26。范围：当前 Rust 对等样例的 CPU 阶段测量、源码分析及优化建议。
本次未修改引擎，不宣称性能优化已经实施。

**测量结论**

TextFlowLarge 的长时间等待可以在无窗口、无 GPU 的稳定布局调用中复现。
debug 构建放大了这条路径的成本；release 构建仍存在首次布局和单字符追加的明显延迟。
应优先优化全文快照的重复计算与数据组织，再建设可见行绘制。

环境为 macOS、Apple M1 Pro、rustc 1.94.1。代码基于 `bf6409d0` 加测量时已有的工作区修改。
临时探针通过 `runnable_entries()` 找到原样例工厂，依次计时工厂调用、
首次 `frames().stabilize_for_query()`、无变更的第二次稳定查询。
Large 随后投递 `Key::Character('Z')`，计时输入分发及稳定查询。
探针链接实际 Cargo 构建的库；release 探针和依赖均使用优化构建。
每个场景在同一进程内构造三个独立 Runtime，结果包含第一次调用和随后两次调用。
这组数据不是新进程冷启动、实际窗口首帧呈现或 p95/p99 基准。

| release 阶段 | Large 第一次 | Large 第二次 | Large 第三次 | 普通 TextFlow 三次范围 |
|---|---:|---:|---:|---:|
| 工厂构造 | 31.375 ms | 7.595 ms | 5.589 ms | 4.461–8.620 ms |
| 首次稳定布局 | 422.091 ms | 420.418 ms | 419.118 ms | 6.687–7.284 ms |
| 无变更稳定查询 | 2.244 ms | 1.818 ms | 1.824 ms | 0.120–0.123 ms |
| 追加 Z 并稳定布局 | 184.221 ms | 164.741 ms | 165.744 ms | 未测量 |

debug 初次采证中，Large 前两次构造分别为 43.275、22.299 ms，首次稳定布局分别为
11588.554、11830.899 ms；普通 TextFlow 三次首次布局为
150.107、144.968、143.568 ms。debug 场景当时并行运行，适合确认阻塞，
不能与顺序执行的 release 场景做严格性能比值比较。

Large 三次稳定快照均包含 40 个段落、501 个文字来源、51232 字节文本、
561 行、1022 个 styled glyph run、51232 个 glyph 和 4168 个 fragment。
来源身份与段落数量来自实际快照。该输入约为 50 KiB，尚未达到通常所说的海量文本规模。

测量不包含绘制 IR、后端编码、GPU 执行和 surface 呈现。因此可以确认 CPU 路径
已构成阻塞，不能据此排除实际应用中额外的绘制成本。
系统 `sample` 因权限限制未成功；以下内部成本点均为源码证据，
尚未取得函数级耗时占比。

**等待如何产生**

场景工厂在 UI 场景切换时同步构造 Runtime，布局也同步完成。
字体注册位于构造期，构造时间已经包含该成本。
本次测量支持先排查稳定布局，不支持把字体加载或 GPU 作为主要原因。

| 路径 | 当前源码行为 | 优化方向 |
|---|---|---|
| 约束测量 | `FlowLayoutService::measure()` 直接调用 `build_snapshot()`，不查询该 service 的整页缓存；构造完整段落及来源几何 | 让测量与准备复用受检的段落布局，测量仍不发布 child bounds |
| 准备缓存 | 缓存只有一个整页条目；键含 scene revision、文字引擎 revision 和 bounds 四分量 | 分开保存段落局部布局与整页位置；追加首段文本不使其余段落 shaping 缓存失效 |
| child presentation | 每个 placement 用 `members.contains()`，再扫描全部 fragments 找到自己的区域；缓存命中也执行该过程 | 一次构建 membership 与 source → fragment 索引，按实际区域直接查询 |
| styled 绘制布局 | 每个 run 构造独立 TextLayout，复制段落文本键、全部行指标和 run glyph | 不可变共享布局加 run 范围，避免每个 run 携带整段数据 |
| 示例根 presentation | `InteractivePage::prepare_scene()` 对已有 FlowSnapshot 执行值级 clone，再放入新 Arc；paragraph、fragment、placement 和布局容器随之复制 | 通过统一的受检 presentation 数据共享接口复用原 Arc |
| glyph 来源映射 | 每个定位后的 glyph run 都从头扫描底层 run 的 `visual_clusters()` | 每个 shaping run 建一次 cluster 索引，保留 bidi、ligature 与 grapheme 语义 |
| 绘制 | FlowSnapshot 遍历全部段落，Graphics 遍历全部 styled paint runs；该层不按可见行筛选 | 绘制命令记录前查询可见段落与行，保留最终 graphics clip |

child presentation 的片段查找成本按 placement 数 P 和 fragment 数 F 为 O(PF)，
成员查找还带来线性扫描。布局复制按某段文本字节数 B、行数 L、run 数 R
可增加 O(R(B+L)) 的文本及行指标复制。
单个 shaping run 跨多行时，cluster 来源映射会重复扫描相同序列。
这些属于可证明的重复工作；各项占 420 ms 的比例仍需分别计时。

证据：
[工厂与字体](../../../examples/native/draw2d-official-examples/src/examples/text_examples.rs)、
[交互根与值级快照复制](../../../examples/native/draw2d-official-examples/src/examples/text_interaction.rs)、
[FlowLayoutService 与全文绘制](../../../novadraw/src/figure/flow/snapshot.rs)、
[styled 布局](../../../novadraw/src/text/styled.rs)、
[布局复制及 cluster 映射](../../../novadraw/src/render/text.rs)、
[Graphics 文字提交](../../../novadraw/src/graphics/context.rs)。

**Zed 的参考价值**

Zed 的 Editor 在 `element.rs` 中根据滚动位置和可见高度确定 `start_row..end_row`，
只收集该区间的 row 信息，并把同一区间交给 `layout_lines()`。
全文显示映射与本帧可见行的绘制布局分开维护。Novadraw 可以借鉴这条边界，
但不能直接采用 Zed 的固定行高寻址，因为 TextFlow 支持混合字号、边框和内嵌 Figure。
应为段落和行建立实际几何区间索引。

GPUI 的 `LineLayoutCache` 复用当前帧与上一帧的布局，并提供按内容哈希查找的入口，
允许在缓存命中前避免拼接完整连续字符串。可借鉴先查版本及布局依赖，再构造请求的顺序。
具体缓存路径的复杂度仍需核对，不能把全部查找都当作常数时间。

`WrapMap` 使用后台任务处理较大的换行更新，设置前台等待时限，
通过 snapshot 和 patch 同步结果。源码中的 5 ms、1 ms 是 Zed 的调度选择。
Novadraw 应使用自身预算，后台只读冻结输入，候选完成后检查版本并在 Runtime 发布。
旧候选不能覆盖后续编辑或 resize 的结果。

Rope/SumTree 适合维护大量可编辑文本与聚合位置摘要。
对目前 40 段输入，有序数组、段落缓存及二分查询已经可以发挥作用。
Rope 解决文本编辑和存储成本，不能自动解决 shaping、换行和 GPU 提交成本。
百万行场景可以进一步评审按段落高度摘要寻址及局部更新的数据结构。

GPUI 的 `paint_glyph()` 使用 sprite atlas 缓存光栅化 glyph，
缓存参数包含字体、glyph、字号、缩放因子及 subpixel variant 等。
这适合大量重复文字，但存在 atlas 内存、淘汰、DPI 和缩放适配成本。
Novadraw 当前使用 Vello glyph runs，先减少可见区外的 CPU 工作与提交，
再凭 GPU 测量决定是否需要独立文字后端；本次未证明更换后端有必要。

Zed 证据：
[可见行范围及布局](file:///Users/bytedance/Documents/code/GitHub/zed/crates/editor/src/element.rs#L10035-L10065)、
[行缓存](file:///Users/bytedance/Documents/code/GitHub/zed/crates/gpui/src/text_system/line_layout.rs#L390-L420)、
[后台换行](file:///Users/bytedance/Documents/code/GitHub/zed/crates/editor/src/display_map/wrap_map.rs#L187-L370)、
[Rope](file:///Users/bytedance/Documents/code/GitHub/zed/crates/rope/src/rope.rs#L14-L27)、
[glyph atlas](file:///Users/bytedance/Documents/code/GitHub/zed/crates/gpui/src/window.rs#L3569-L3620)。
这些是本地 Zed 源码版本的事实，不构成 Novadraw 已具备相应能力的证明。

Draw2D 官方样例明确用于文档变更后的布局性能测试，因此优化必须保持
首来源追加、来源身份、嵌套 inline、混合字号、tooltip 和全文滚动语义。
[官方源码](file:///Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/text/TextFlowLargeExample.java)。

**优化顺序**

先保留精确全文布局，优化现有数据流：

- 为来源片段建立索引；消除 InteractivePage 的完整快照复制及按 run 的全文数据复制。
- 段落局部布局与页面 placement 分开缓存。首段追加只重排受影响段落，
  高度变化更新后续偏移；同约束测量与准备复用。页面 revision 继续更新。
- 颜色变化复用 glyph 位置；纯位移复用局部布局；宽度变化仍按换行依赖失效。
  缓存键覆盖内容、顺序、布局样式、约束、字体/fallback generation 与内嵌测量依赖。
- 注册字体可以按消费者依赖加载并批量更新，但应在当前更高成本路径之后排序。
  减少运行时注册不会自动减少已编译进去的 `include_bytes!` 资源和 Wasm 下载量。

接着增加可见段落及行查询。保持全文尺寸与交互快照有效，
把有效 clip 经通用坐标转换映射到文本局部空间，
按实际行区间筛选 glyph 和边框，再生成 Render IR。
单个超长段落必须支持行级筛选，仅筛段落仍会提交大量不可见文字。
非轴对齐变换使用保守区间，并包含 glyph ink 和 border 扩展。
目标是纯滚动不重新 shaping，记录成本主要随可见输出增长。

真正的大文档再引入后台重排和首屏优先布局。后台重排改善 UI 响应，
若首帧仍等待全文完成，首屏时间不会自动缩短。
首屏优先布局需要显式区分精确与未布局或估算几何，定义查询的 Result、
滚动范围、来源锚点、跳到末尾、取消及过期结果丢弃。
不能把估算位置作为稳定的 caret、selection 或命中事实。
Native executor 与 Web worker/可让出的分片任务应保持相同发布契约。

所有机制归 Core/text service 或具体后端，外部 Figure、contributor、border 和
替代引擎必须能使用相同的缓存与可见查询。按现有开放协议实现，
不为 Large 样例添加特权路径。当前递归渲染主线无需替换。
共享布局公共边界、可见绘制、异步发布和渐进布局需要架构评审。

**实施后的验收**

同配置测量首次稳定布局、真正首次呈现、首段追加、滚动、连续 resize、
p50/p95/p99、峰值内存与缓存预算。增加段落数以及等量文本的单长段落两类输入。
计数段落 shaping、重复测量、片段扫描、复制字节与已记录 glyph，
不能仅凭文本内容不变的断言证明增量成本达标。

行为回归覆盖混合字号、RTL、ligature、跨来源选择、片段空白命中、
reparent、内嵌 Figure、边框、字体替换、缩放和过期后台任务。
可见 IR 随可见内容增长；首段追加时未受影响段落的 shaping 次数应为零。
同时核对 Native/Vello 像素，评估可选依赖、字体和 Wasm 的实际传输体积。
目前只有前述基线测量通过，所有优化收益仍待实施后验证。

完整架构取舍继续参考
[10 月 23 日性能提案](../../design/architecture/textflow-performance-proposal-2026-10-23.md)；
本报告补充测量事实并校准优先级，不改变路线图执行队列。
