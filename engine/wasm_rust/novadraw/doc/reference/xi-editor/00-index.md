# Xi Editor 参考

类型：`reference-analysis`

本页记录 Xi Editor 的编辑器开发资料入口。它只作为外部参考，不定义 Novadraw 的
Editor、TextFlow 或 Core 契约。

## 文档入口

- Xi Editor 文档索引：<https://xi-editor.io/docs/>
- Rope Science 入口：<https://xi-editor.io/docs/rope_science_00.html>

Rope Science 是 Xi Editor 设计阶段的一组思考文章。Introduction 页面说明，这组
文章来自 2016 年设计 Xi Editor 时的内部帖子，讨论用于改进 Xi 的高级计算机科学
概念和一些探索性设计。作者也说明它们有助于理解 Xi 内部机制，但不能替代正式文档。

## Rope Science 系列

- [Introduction](https://xi-editor.io/docs/rope_science_00.html)
- [MapReduce for Text](https://xi-editor.io/docs/rope_science_01.html)
- [Metrics](https://xi-editor.io/docs/rope_science_02.html)
- [Grapheme Cluster Boundaries](https://xi-editor.io/docs/rope_science_03.html)
- [Parenthesis matching](https://xi-editor.io/docs/rope_science_04.html)
- [Incremental Word Wrapping](https://xi-editor.io/docs/rope_science_05.html)
- [Parallel and Asynchronous Word Wrapping](https://xi-editor.io/docs/rope_science_06.html)
- [CRDTs for Concurrent Editing](https://xi-editor.io/docs/rope_science_08.html)
- [CRDT Follow-up](https://xi-editor.io/docs/rope_science_08a.html)
- [CRDT Approach to Async Plugins and Undo](https://xi-editor.io/docs/rope_science_09.html)
- [Designing for a Conflict-free World](https://xi-editor.io/docs/rope_science_10.html)
- [Practical Syntax Highlighting](https://xi-editor.io/docs/rope_science_11.html)
- [Minimal Invalidation](https://xi-editor.io/docs/rope_science_12.html)

官方目录没有列出第 7 篇。Introduction 页面提到，原计划的第 7 篇应讨论 spans
和 interval trees，但作者没有写成。

## 可参考范围

- rope 和多指标文本度量的设计。
- Unicode grapheme cluster 边界处理。
- 括号匹配、自动换行、语法高亮的增量计算思路。
- 并行或异步排版对编辑器响应性的影响。
- CRDT、异步插件和 undo 模型之间的关系。
- 最小失效范围与长文本局部重算策略。

## 边界

- 不能直接作为 Novadraw TextFlow、Editor 或协同编辑契约。
- 如需采用其中某个机制，必须重新做源码或算法证据摘录，并映射到 Novadraw 的状态所有权、
  失效边界、通知语义和性能门禁。
