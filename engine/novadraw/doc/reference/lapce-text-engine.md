# Lapce 与 Xi 文本机制参考

类型：`reference-analysis`

日期：2026-10-27。范围：文本存储、编辑 delta、视觉行布局与绘制。
本页记录外部源码和理论事实，不定义 Novadraw 行为。
采用建议见[后续优化提案](../design/architecture/long-text-incremental-layout-proposal.md)。

## 来源版本

Lapce `master` 本次解析为
`f66ffaa95387794205c35864517de70958b9ea8d`，提交日期为 2026-09-30。
其 Cargo.toml 与 Cargo.lock 将 Floem 和 floem-editor-core 固定到
`31fa8f444c37f4c314f47d88c23ffdbc25f2ab53`，使用 lapce-xi-rope 0.3.2。
分析读取了这两份固定提交，未编译或运行 Lapce。[cite:1](#source-1)

Rope Science 是 2016 年开始撰写的设计系列；Introduction 明确说明，
文章不能替代正式文档。part 6 中的并行和异步换行当时还是计划，
不能从文章推断它们已在本次 Lapce 版本中实现。[cite:2](#source-2)

Lapce 使用 Xi 派生的 rope，两个来源具有继承关系。
本页不会将两者计为独立性能复现。源码行为属于固定版本事实，
理论迁移和 Novadraw 收益均须另行验证。

## 编辑变更有显式范围

Lapce 的 `Doc::apply_deltas` 接收旧 Rope、RopeDelta 和 InvalLines，
逐次更新样式、inlay、diagnostics、completion、find 和断点映射，
随后生成 SyntaxEdit。其 `lapce-core` 的 Buffer 实际来自
floem-editor-core 的重导出。[cite:3](#source-3)

Floem `Buffer::apply_edit` 在安装新文本时增长 revision，
用 `delta.summary()` 计算旧、新逻辑行范围，返回
`start_line`、`inval_count`、`new_count` 和旧 Rope。
这是来源变更的范围描述，尚不是视觉行或像素 damage。[cite:4](#source-4)

Rope Science part 5 将编辑描述为旧范围与替换 rope，
并让自动换行产生对旧 breaks 的编辑。
其正确性条件是增量结果等于完整重新计算。
这说明 delta 可在派生阶段继续传播，而不只用于文本存储。[cite:5](#source-5)

## 视觉行与逻辑行分开

Floem 用 `VLine` 表示绝对视觉行，`RVLine` 表示逻辑行及内部换行索引。
`TextLayoutCache` 按字号和逻辑行保存 `Arc<TextLayoutLine>`，
并持有 document cache revision、配置身份和已见最大宽度。
`get_init_text_layout` 按需建立缺失布局并发出 CreatedLayout 事件。[cite:6](#source-6)

该版本的 `Lines::check_cache` 在 revision 或配置变化时清空布局；
`set_wrap` 改变宽度策略时也清空。Lapce `Doc::on_update` 会进入
`clear_style_cache`，后者增加文本 cache revision。
因此，显式 delta 不代表这里已实现局部保留所有布局资产。[cite:3](#source-3)[cite:6](#source-6)

Floem `Editor::new_text_layout` 获取完整逻辑行、组合 phantom text，
构造样式并调用 TextLayout。其底层包装 Cosmic Text，
`TextLayout::set_text` 重建 BufferLine，调用 `shape_until_scroll`。
按需请求一行仍可能处理一个很长的逻辑行。[cite:7](#source-7)

## 可见行驱动绘制

Floem `normal_compute_screen_lines` 从 viewport 得到视觉行范围，
建立 ScreenLines；`EditorView::paint_text` 遍历这些行并绘制布局。
Lapce 使用该 paint_text 入口，也另外绘制 sticky header 等内容。[cite:8](#source-8)

这里证明的是屏幕行驱动。它不单独证明长行已经按横向可见 glyph 裁剪，
也不提供可迁移到 Vello 的任意变换覆盖证书。
`normal_compute_screen_lines` 的源码还明确保留统一行高假设的待改进注释，
该计算不能直接用于含不同字体、边框和真实 Figure 的 TextFlow。[cite:8](#source-8)

## Rope 摘要能覆盖的成本

Rope Science part 1 用可结合的摘要保存首行片段、完整行最大值和末行片段，
在更新叶子后只重算祖先。示例假定 ASCII 和等宽字体，
不能把字节数或字符数当作一般字体的精确宽度。[cite:9](#source-9)

part 2 讨论 UTF-8、UTF-16、硬换行和 difficulty 摘要，
同时指出软换行等有状态结果适合计算后存入与原文本对齐的第二棵树。
它也提出带行高的派生度量。文本树与布局树可以共享坐标单位，
但不必共享分块或节点边界。[cite:10](#source-10)

树查找和局部摘要更新可具有对数路径成本。
读取替换内容、长叶子处理、输出 glyph 和首次全文分析仍有相应规模成本，
不能将整个编辑、shaping 或渲染过程统称为 O(log N)。

## 增量换行的适用边界

part 5 的实际算法从编辑前两个 break 附近开始向前换行，
在编辑后遇到与旧结果一致的 break 时停止。
这一停止规则建立在其换行和宽度模型上。
文章的后续讨论明确提醒复杂文字、ligature、跨空格字体行为和安全分段问题；
2016 年的约 40 µs 结果有特定实现、输入和环境，不是 Novadraw 的目标证据。[cite:5](#source-5)

UAX #14 产生候选换行机会，最终断行选择由知道实际宽度的上层算法完成。
UAX #9 规定段落方向分析及逐行重排；存储顺序与视觉顺序不同。
安全换行位置、grapheme 边界和可独立 shaping 的边界各有合同。[cite:11](#source-11)

HarfBuzz 文档建议分段 shaping 时提供完整段落上下文。
项目现有 HarfRust 0.3.2 的 GlyphInfo 提供 `unsafe_to_break` 与
`unsafe_to_concat`，后者必须启用 `PRODUCE_UNSAFE_TO_CONCAT`，
而且旧侧、重新 shaping 的新侧都需要满足拼接条件。
这只是 shaping 层的证据，不覆盖 bidi、fallback、样式变更或行布局状态。[cite:12](#source-12)
HarfRust 没有直接对应片段 end-of-text 的 glyph 位置供这项证明使用；
右端需额外 shaping，使待拼接位置成为内部 cluster 边界。
最后一个 glyph 的标志不能直接当作片段末端证明。[cite:12](#source-12)

现有 Parley 0.7.0 有 `Layout::break_lines`、`break_all_lines` 与 `align`，
`BreakLines::break_next` 能逐行消费已经分析的布局。
单次 break_next 仍会扫描该行的 cluster；NoWrap 巨行可能一次处理很多输入。
本次未发现 Parley 对外导出上述 HarfRust 拼接证明的接口，
不能据此声称默认适配器已支持安全增量 shaping。[cite:13](#source-13)
`revert` 只恢复上一次逐行计算前的状态；新建 breaker 从段首开始，
公开 API 没有跨编辑检查点 export／restore。逐行计算不等于持久状态恢复。[cite:13](#source-13)

Parley 的 shaping 在 inline box 插入位置设置 run 分隔。
因此，盒的新增、删除和位置变化属于结构输入，不能统一视为纯尺寸更新。
bidi resolver 即使判断无需复杂 bidi，也先遍历输入建立类型及层级状态；
全段分析的读取成本与局部 shaping 窗口成本需要分别记录。[cite:16](#source-16)

## 输出变化继续传播

Rope Science part 12 用 line cache shadow 和 render plan 区分有效、失效及需绘制行，
输出 copy、invalidate、insert、update 等变化。
光标变化可以只更新部分有效性；viewport 外的失效不必立即重绘。
这给出了局部派生与局部输出的理论路径，不要求 Novadraw 采用 Xi 的 RPC 协议。[cite:14](#source-14)

Lapce 的异步 syntax 路径携带 revision，回到 UI 时检查版本，
只在匹配时安装结果。它是异步派生任务的版本检查参考，
不能替代包含宽度、字体、inline Figure 依赖和 Runtime 生命周期的排版提交合同。[cite:15](#source-15)

## 来源

<a id="source-1"></a>[cite:1] [Lapce Cargo.toml](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/Cargo.toml#L73-L89)；
[Cargo.lock](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/Cargo.lock#L1747-L1758)。

<a id="source-2"></a>[cite:2] [Introduction](https://xi-editor.io/docs/rope_science_00.html)；
[parallel and asynchronous word wrapping](https://xi-editor.io/docs/rope_science_06.html)。

<a id="source-3"></a>[cite:3] [Lapce Doc::apply_deltas、on_update](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/lapce-app/src/doc.rs#L656-L718)；
[clear_style_cache、clear_text_cache](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/lapce-app/src/doc.rs#L861-L882)；
[lapce-core reexports](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/lapce-core/src/lib.rs)。

<a id="source-4"></a>[cite:4] [Floem Buffer::apply_edit](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/editor-core/src/buffer/mod.rs#L333-L363)。

<a id="source-5"></a>[cite:5] [incremental word wrapping](https://xi-editor.io/docs/rope_science_05.html)。

<a id="source-6"></a>[cite:6] [Floem TextLayoutCache](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/visual_line.rs#L126-L181)；
[check_cache](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/visual_line.rs#L896-L929)；
[get_init_text_layout](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/visual_line.rs#L938-L1006)。

<a id="source-7"></a>[cite:7] [Editor::new_text_layout](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/mod.rs#L1221-L1310)；
[TextLayout::set_text](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/renderer/src/text/layout.rs#L230-L280)。

<a id="source-8"></a>[cite:8] [ScreenLines 计算](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/mod.rs#L1495-L1568)；
[paint_text](https://github.com/lapce/floem/blob/31fa8f444c37f4c314f47d88c23ffdbc25f2ab53/src/views/editor/view.rs#L729-L808)；
[Lapce 绘制入口](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/lapce-app/src/editor/view.rs#L1170-L1210)。

<a id="source-9"></a>[cite:9] [MapReduce for text](https://xi-editor.io/docs/rope_science_01.html)。

<a id="source-10"></a>[cite:10] [metrics](https://xi-editor.io/docs/rope_science_02.html)；
[grapheme cluster boundaries](https://xi-editor.io/docs/rope_science_03.html)。

<a id="source-11"></a>[cite:11] [UAX #14 revision 57](https://www.unicode.org/reports/tr14/tr14-57.html)；
[UAX #9 revision 52](https://www.unicode.org/reports/tr9/tr9-52.html)。
引用版本用于本次研究，不表示修改项目当前 Unicode 行为版本。

<a id="source-12"></a>[cite:12] [HarfBuzz buffer 与上下文](https://harfbuzz.github.io/harfbuzz-hb-buffer.html#hb-buffer-add-codepoints)；
[HarfRust 0.3.2 GlyphInfo](https://docs.rs/harfrust/0.3.2/harfrust/struct.GlyphInfo.html)。
HarfRust 方法正文另由本机该版本 registry 源码核对。

<a id="source-13"></a>[cite:13] [Parley 0.7.0 Layout](https://docs.rs/parley/0.7.0/parley/layout/struct.Layout.html)；
[BreakLines](https://docs.rs/parley/0.7.0/parley/layout/struct.BreakLines.html)。
接口和循环粒度由本机该版本 registry 源码核对。

<a id="source-14"></a>[cite:14] [minimal invalidation](https://xi-editor.io/docs/rope_science_12.html)。

<a id="source-15"></a>[cite:15] [Lapce trigger_syntax_change](https://github.com/lapce/lapce/blob/f66ffaa95387794205c35864517de70958b9ea8d/lapce-app/src/doc.rs#L836-L857)。

<a id="source-16"></a>[cite:16] [Parley 0.7.0 shaping](https://docs.rs/crate/parley/0.7.0/source/src/shape/mod.rs)；
[bidi resolver](https://docs.rs/crate/parley/0.7.0/source/src/bidi.rs)。
本机固定版本核对 inline run 分隔及全段类型扫描；
这些源码事实不提供增量性能实测。
