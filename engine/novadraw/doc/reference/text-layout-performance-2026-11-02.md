# 文本排版与绘制的性能技术

类型：`reference-analysis`

日期：2026-11-02。外部参考归属：LibreOffice VCL、Qt、VS Code、Xi、Zed／GPUI。
本文记录公开源码与文档中的机制、限制和对 Novadraw 的适用性，不定义 Core API。
Novadraw 的采用合同以 [ADR-051](../adr/adr-051-exact-flow-layout-reuse.md)和
[长文本方案](../design/architecture/long-text-performance-proposal.md)为准。

## 性能问题要按阶段判断

文本编辑包含存储修改、Unicode／bidi 分析、字体选择与 shaping、断行／位置、
交互几何、绘制记录、字形栅格化和最终合成。不同缓存只能消除对应阶段的重复工作。
Rope／piece tree 提升文本修改和位置查找，glyph atlas 提升重复字形的栅格化；
两者都不能自动消除全文重新排版。

shaping 将字符及上下文映射成实际字体的 glyph、advance 和 cluster。
断行使用这些度量决定视觉行；栅格化再把 glyph 变成设备覆盖。
只改窗口宽度通常值得研究复用 shaping，但必须核对 bidi 逐行重排、字体 fallback、
hyphenation、样式和 inline box 的依赖。滚动若只改视图位置，可以保留文字布局。

```mermaid
flowchart LR
  A["来源与样式版本"] --> B["字体选择与 shaping 资产"]
  B --> C["宽度相关断行与位置"]
  C --> D["全文尺寸和交互几何"]
  D --> E["可见内容查询与 IR"]
  E --> F["字形栅格缓存与合成"]
```

图中是职责划分，缓存命中仍须满足完整依赖和版本合同。

## LibreOffice 有哪些技术

TDF 在 2021 年的专项招标中明确指出：测量全文、测量子串、断行和绘制反复执行
shaping，复杂文字及打印／PDF 输出尤其受影响。提出的方向包括延长 shaping
结果寿命、改进 OutputDevice API 和增加缓存。该公告是当时的问题与项目目标，
不能把其中的性能退化描述当作 2026 年版本的基准。[cite:1](#source-1)

本次读取的官方 VCL Doxygen 源码快照生成于 2023-07-30。它包含
`SalLayoutGlyphsCache`，按输出设备、字体和文本参数缓存 `SalLayoutGlyphs`，
使用带成本计算的 LRU map；绘制及相关测量入口可以复用已排版字形。
额外状态变化需要清理缓存。文档特别说明：fallback 可在缓存寿命内变化且没有
进入该版 cache key，因此普通 fallback 情况不能无条件缓存。[cite:2](#source-2)

`SalLayoutGlyphsImpl::cloneCharRange` 和 `makeGlyphsSubset` 尝试从完整字形结果
提取子范围，避免每个子串重新 shaping。源码检查方向一致性、cluster 边界、
multi-character glyph 和 `IsUnsafeToBreak`；不能证明等价时返回不可复用，
重新布局。`GetLayoutGlyphs` 还识别连续子串请求并尝试缓存完整字符串。
这是可复核的实现事实，未在本轮运行 LibreOffice 基准。[cite:3](#source-3)

对 Novadraw 最有价值的是：测量／绘制共享布局资产、缓存依赖必须完整、
子串复用必须有安全边界。LibreOffice 的全局 `self()` 缓存不适合直接移植；
本仓库继续使用实例所有权与外部／内置统一协议。

TDF 的 2026-08-26 发布说明还宣布 Writer 26.8 引入 Paragraph Composer，
跨连续行平衡字间距，区别于逐行贪心选择；它属于排版质量能力。
该公告没有给出这项算法的阶段耗时或编辑尾延迟，
不能把更好的两端对齐直接解释为更快的排版。[cite:10](#source-10)

## Qt 提供的布局复用

`QTextLayout` 将创建视觉行、指定宽度和绘制分开。
`setCacheEnabled(true)` 保留完整布局信息以供后续绘制；默认在 `endLayout`
后会释放较多布局数据，以换取较低内存占用。`glyphRuns` 文档明确提示该查询
较昂贵，选择必要的 retrieval flags 可以减少数据物化与分配。[cite:4](#source-4)

`QStaticText` 用于文字、字体和布局较少变化的场景，可预先 `prepare`，
并提供 Moderate／AggressiveCaching 的内存与速度选择。平移不要求重新排版，
其他 transform 或字体修改可能触发重算；频繁改变布局时，静态缓存未必划算。
它不是通用富文本树，内嵌图片可以参与布局但不由该类绘制。[cite:5](#source-5)

这支持将“当前布局资产”和“本帧绘制位置”分开，但不支持跳过字体、缩放和
真实 inline Figure 的依赖检查。

## 编辑器采用的不同策略

| 项目 | 可核验机制 | 解决的阶段 | 不能据此推断 |
|---|---|---|---|
| VS Code | 多缓冲 piece tree；红黑树保存子树文本长度与换行摘要 | 大文件存储、编辑与行位置查找 | 排版、bidi 或 glyph 渲染也具有对数时间 |
| Xi | 显式文本 delta 和独立 breaks；从编辑附近重算，满足旧／新断行收敛条件后停止 | 增量自动换行 | 2016 年的简单宽度模型可直接用于任意复杂脚本和真实 Figure |
| Zed | visible rows 驱动 `layout_lines`；当前／上一帧行缓存；Rope／SumTree 显示映射 | 只准备可见行、复用已准备行、位置寻址 | TextFlow 可采用固定行高，或同一哈希就能证明全部输入等价 |
| GPUI | 按字形参数查 sprite atlas，缺失时 `rasterize_glyph` | 重复字形的设备栅格化 | 本项目 Vello 的 CPU 排版瓶颈可由换 atlas 自动解决 |

VS Code 的官方文档明确区分字符串拼接、行对象分配和位置查找成本，
并说明小文档中简单行数组仍可能更快。其基准属于 2018 年特定环境，不用于
估算 Novadraw 提速。[cite:6](#source-6)

Xi part 5 的算法与简单宽度假设、Unicode 候选断点绑定；复杂脚本、ligature、
跨空格 kerning 等需要额外安全分析。part 6 的并行和异步 rewrap 是该文章当时
的计划，不能引用为已交付机制。分段后台加载／估算滚动高度也不同于 Novadraw
已批准的全文精确几何合同。[cite:7](#source-7)[cite:8](#source-8)

Zed 本轮只读源码版本为 `b3a67c988f9d28a009086952d46fc8c977ad028c`：
`LineLayoutCache` 保存当前／上一帧资产，内容哈希入口可延迟 materialize text，
但该版本的按哈希查找实际遍历 map，并接受调用方的哈希碰撞风险。
Novadraw 只借鉴延迟物化与资产复用，保留完整请求相等校验。
`WrapMap` 的前台等待采用 5 ms／1 ms 超时后由 task 延续；这些数字属于其调度，
本项目不能当作普适预算照抄。[cite:9](#source-9)

Lapce／Floem 的固定版本分析已有[独立参考](lapce-text-engine.md)：
screen lines 驱动绘制，但部分 revision 更新会清空缓存，
按需布局逻辑行也不能保证巨长行只处理横向可见 glyph。
因此“使用 Rope”“按需布局”和“精确增量 shaping”需要分别核对。

## 对 Novadraw 的采用顺序

先保持当前精确模式，在受检输入和输出不变的前提下消除重复查找、复制及物化。
本轮先优化 `FlowWork` 段落缓存查找：用文本长度缩小候选范围，避免先遍历完整
文本、样式与 inline box 计算指纹；命中仍比较完整请求、引擎身份和 revision。
该长度不是缓存身份，也不能作为排版结果等价的依据。
本轮不增加文字引擎能力或改变来源几何。
随后按实际阶段成本选择重复贡献／稳定化复用或宽度分阶段布局。
贡献 callback 没有纯度和 revision 合同时，不能跨场景缓存其结果。

原生可见字形筛选必须先补齐字体墨迹、数值域、实际 transform／clip／AA 与
lowerer 的覆盖依据。默认原生路径仍输出完整 glyph；逻辑行盒或固定像素常量
不能充当证明。证明未知时继续完整录制，不能丢字换速度。

worker、渐进覆盖、Rope 来源和 glyph atlas 在有明确规模或阶段瓶颈时独立评审。
性能验证比较同源码输入、字体、构建配置、cache 命中分母和正确性结果，
分别记录首次布局、编辑、滚动、resize、CPU IR 和实际呈现。
P50／P95 描述采样分布，不能把不同阶段分位数相加当作总延迟。

## 来源和核对限制

<a id="source-1"></a>[cite:1] TDF：[Text layout performance tender，2021-12-08](https://blog.documentfoundation.org/blog/2021/12/08/tender-to-optimize-text-layout-performance-for-print-and-pdf-export-202112-01/)。

<a id="source-2"></a>[cite:2] LibreOffice：[SalLayoutGlyphsCache](https://docs.libreoffice.org/vcl/html/classSalLayoutGlyphsCache.html)，官方 Doxygen 快照生成于 2023-07-30。

<a id="source-3"></a>[cite:3] LibreOffice：[impglyphitem.cxx](https://docs.libreoffice.org/vcl/html/impglyphitem_8cxx_source.html)，重点为 `cloneCharRange`、`isSafeToBreak`、`makeGlyphsSubset`、`GetLayoutGlyphs`。

<a id="source-4"></a>[cite:4] Qt：[QTextLayout](https://doc.qt.io/qt-6/qtextlayout.html)。

<a id="source-5"></a>[cite:5] Qt：[QStaticText](https://doc.qt.io/qt-6/qstatictext.html)。

<a id="source-6"></a>[cite:6] VS Code：[Text Buffer Reimplementation，2018-03-23](https://code.visualstudio.com/blogs/2018/03/23/text-buffer-reimplementation)。

<a id="source-7"></a>[cite:7] Xi：[Rope Science part 5，2016-04-22](https://xi-editor.io/docs/rope_science_05.html)。

<a id="source-8"></a>[cite:8] Xi：[Rope Science part 6，2016-04-24](https://xi-editor.io/docs/rope_science_06.html)。

<a id="source-9"></a>[cite:9] Zed 固定只读源码：
[行缓存](file:///Users/bytedance/Documents/code/GitHub/zed/crates/gpui/src/text_system/line_layout.rs#L392-L425)、
[哈希查询](file:///Users/bytedance/Documents/code/GitHub/zed/crates/gpui/src/text_system/line_layout.rs#L635-L715)、
[后台换行](file:///Users/bytedance/Documents/code/GitHub/zed/crates/editor/src/display_map/wrap_map.rs#L187-L330)、
[字形 atlas](file:///Users/bytedance/Documents/code/GitHub/zed/crates/gpui/src/window.rs#L3569-L3625)。

<a id="source-10"></a>[cite:10] TDF：[LibreOffice 26.8 发布说明，2026-08-26](https://blog.documentfoundation.org/blog/2026/08/26/libreoffice-26-8/)。

研究未运行这些产品的性能基准，也未证明外部方案在 Novadraw 的收益。
LibreOffice GitHub API 因凭据无效返回 401，改用官方公开 Doxygen 和 TDF 文档；
不能据此声称已审阅最新 2026 年 LibreOffice 分支。
单项目文档与源码属于同一来源，不能冒充独立性能复现。
