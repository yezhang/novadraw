# quiet-md 富文本编辑工程样例参考

类型：`reference-analysis`

记录日期：2026-11-05。范围：富文本模型、编辑行为、行内对象、虚拟化与验证。
关联能力归属：Editor 的富文本草稿与编辑事务；Core 的文字排版与交互几何；
platform 的 IME 桥接；应用的持久模型、格式转换和文档视图。
本页用于后续工程样例参考，不定义 Novadraw 行为，也不改变执行队列。

## 仓库与核对版本

- 仓库：[s1dashu/quiet-md](https://github.com/s1dashu/quiet-md)。
- 用户提供的入口：[架构文档](https://github.com/s1dashu/quiet-md/blob/main/docs%2FARCHITECTURE.md)。
- 本次核对提交：[`937b254f8c4553e4998f83ce942675da558c743a`](https://github.com/s1dashu/quiet-md/tree/937b254f8c4553e4998f83ce942675da558c743a)。
- 许可证：[MIT](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/LICENSE)；
  后续若复制代码或实质性片段，保留其版权与许可声明，第三方依赖另核对。

quiet-md 是 Rust／GPUI 文档编辑应用。其文档模型与编辑内核位于不依赖 GPUI 的
`fdl_core`，应用层负责视图、系统输入和渲染。本次读取了架构文档及相关源码，
未编译、运行或引入该仓库；以下源码入口固定到本次核对提交。

## 值得参考的机制与源码入口

| 研究主题 | 源码入口 | 值得借鉴的内容 |
|---|---|---|
| 富文本与样式区间 | [`model.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/model.rs)，`RichText`、`push`、`marks_at`、`update_marks` | 文本加 run-length 样式；完整覆盖、非空 run、相邻相同样式合并；输入位置的样式继承 |
| 编辑与历史 | [`editor/core.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/editor/core.rs)，`pending_marks`、`checkpoint`、`undo`、`redo` | 空选区的后续输入样式、跨 block 编辑、连续输入合并与选择恢复 |
| 行内对象与格式保留 | [`FORMAT.md`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/docs/FORMAT.md)，`Inline { kind, xml }`、`BlockKind::Raw` | U+FFFC 对象位置、解析后的含义与原始 XML 同时保留、未知元素和属性的往返保存 |
| 文本显示与交互 | [`text.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/text.rs)，`TextLayout`、`RichTextElement` | 排版结果供绘制、命中、caret 和 selection 共用；行内对象的占位与交互 |
| 文档虚拟化 | [`render.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/render.rs)，`hash_block`、`sync_items` | 依赖未变时跳过列表重建、公共前后缀差分、保留顶部条目内滚动偏移 |
| IME 桥接 | [`view.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/src/editor/view.rs)，`EntityInputHandler`、`to16`、`from16` | UTF-16 平台范围与 UTF-8 编辑位置转换、marked range、候选窗几何查询 |
| 编辑验证 | [`fuzz_tests.rs`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/crates/core/src/editor/fuzz_tests.rs)，`check`、`round_trip`、`run` | 随机操作后检查不变量、序列化稳定性、完整撤销／重做与 seed 重放 |
| 性能采样 | [`PERFORMANCE.md`](https://github.com/s1dashu/quiet-md/blob/937b254f8c4553e4998f83ce942675da558c743a/docs/PERFORMANCE.md) | 固定环境与输入脚本，区分操作处理和同步绘制耗时，报告分位数与测量终点 |

## 借鉴时保留的边界

优先参考样式编辑语义、行内对象的数据一致性和可重放验证。
其 `core` 是应用文档引擎，不能按名称对应到 Novadraw Core。
Novadraw 继续由 Editor 持有草稿、选择与 composition，由应用持有持久模型，
消费 Core 的不可变排版与交互几何；具体合同见
[direct edit](../design/editor/p2-direct-text-edit.md)。

- **存储与撤销成本**：`RichText::replace`、`update_marks` 重建 block 内文本。
  新撤销步骤使用整篇文档复制，连续输入会合并；不据此认定已实现局部存储增量，
  或每次按键都会复制整篇文档。
- **虚拟化与精确几何**：列表使用估计高度，文本 key 只取长度等部分信息，
  容许同长度修改后屏幕外高度暂时过期。不能作为精确 TextFlow 的缓存身份；
  文档变化后仍重建条目与 key，也不能视为完整的局部增量算法。
- **对象与扩展**：其封闭类型枚举和替代字符占位是应用实现。
  后续研究 Novadraw 行内对象时，另定义开放扩展、度量、baseline、断行、命中与来源映射。
- **输入与验证**：IME preedit 直接修改其编辑文档；不覆盖 Novadraw 的会话提交与取消合同。
  fuzzer 的字符边界检查不证明 grapheme、bidi 或视觉 caret 全部正确；
  往返测试显式预处理了已知的单元格尾随空白损失。
- **性能证据**：作者的百万字测试对应历史版本 `0e92b51`；
  同步 `window.draw` 耗时不包含屏幕呈现延迟，未在本项目复现。
  不据此推断 Novadraw 收益、FR-02 整页组装复用效果或替换 Parley 的必要性。

后续适用性核对可结合
[有限富文本提案分析](../verification/reviews/bounded-rich-text-proposal-assessment-2026-11-03.md)。
本页保存样例入口与固定版本事实；实际采用仍按对应能力的设计、验证和
[P2 执行队列](../roadmap/p2-delta-backlog.md)维护。

本次完整技术判断见
[quiet-md 富文本编辑适用性报告](../verification/reviews/quiet-md-rich-text-assessment-2026-11-05.md)。
