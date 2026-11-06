# novadraw-richtext

能力归属：独立富文本扩展组件。依赖 Core 的公开 Figure、文字、mutation 和滚动协议；
不依赖 Editor、平台或渲染后端。

当前是[独立 RichTextFigure 方案](../../doc/design/architecture/rich-text-figure-proposal.md)
的基础切片。正文直接保存在文档快照中，段落不会转换为 Flow 子 Figure。

## 已有能力

- 显式硬段落、私有 Ropey backing、稳定段落身份与不可变文档快照。
- 有限额的文档构造、已解析且不重叠的 UTF-8 样式 runs、段落方向、对齐和段间距。
- 单段替换与纯文本段内编辑；保留未变段落资产，拒绝非法 UTF-8 端点及错误版本。
- 持久化平衡高度索引，支持累计位置、逻辑可见范围和单高度更新。
- Runtime 文字服务上的完整段落排版、不可变绘制快照及同版文本点位查询。
- 真实 `RichTextFigure` 外壳、一个正文内容 Figure、内部 ScrollPane 与垂直滚动条。
- `ReplaceRichTextDocument` typed update 和 `RICH_TEXT_DOCUMENT` 属性通知。

文档版本包含文档实例、修订号和快照身份。从同一旧版本构造不同候选时，
即使修订号相同，候选也具有不同身份。相同快照更新返回 NoChange。

## 接入

```rust
use novadraw::{
    geometry::Rectangle,
    graphics::Color,
    text::{FontDescriptor, TextStyle},
};
use novadraw_richtext::{ParagraphInput, RichTextDocumentBuilder, RichTextFigure};

let mut document = RichTextDocumentBuilder::default();
document.push(ParagraphInput::new(
    "Hello, 富文本",
    TextStyle::new(FontDescriptor::default(), Color::BLACK),
))?;
// tree 和 parent 由消费者提供；此时还没有交给 Runtime。
let handle = RichTextFigure::add_to(
    &mut tree.builder(), parent,
    Rectangle::new(0.0, 0.0, 480.0, 600.0), document.build(),
)?;
```

消费者在自己的 Runtime 中注册所需字体。组件不另建字体服务。
挂载后更新使用
`runtime.figure(handle.content())?.update_component(ReplaceRichTextDocument { expected, document })`。
更新候选可以在 Runtime 外构造，但 attached 源状态只能经 typed update 提交。
布局快照通过正文的 `prepared_presentation().shared_data::<RichTextLayoutSnapshot>()`
取得；稳定化后再查询，不能把旧快照当作新源的当前布局。

`add_to` 仅用于 detached tree builder 阶段。构造失败时调用方应丢弃这份未发布的
候选 tree；它不是 attached 复合子树的原子挂载入口。
handle 保存受检 Figure 身份，不保存另一份文档或滚动位置。

## 当前边界

本切片保留完整段落排版作为正确性起点。相同文档、文字引擎身份／generation、
有效宽度和布局方向的测量／准备共享完整快照，最多保留两个条目。
缺少可缓存身份的外部文字引擎继续完整计算；字体或文档变化会使旧条目失配。
尚无独立段落资产缓存或跨宽度 analysis 复用。宽度变化仍完整排版，
纯滚动可以命中快照，但不承诺所有引擎和场景下都零排版。
高度索引已支持路径更新，但当前 layout 构造仍从完整高度列表建立新索引。
正文 paint 输出全部段落，由 Core 裁剪；逻辑高度范围不能单独作为墨迹覆盖证明。

Ropey 1.6.1 使用 MIT 许可证，类型不进入公开签名。当前每段保存一个 Rope，
文档目录用共享段落数组；编辑会物化受影响段落并复制目录。跨段 split／merge、
样式覆盖树、全局文档 Rope 和巨段局部编辑仍待后续切片。
当前样式输入要求调用方先解析为非重叠 runs，不接受嵌套覆盖。
下划线、range／caret 对外查询、完整编辑器输入、IME、撤销和 Markdown 均未交付。
相同文本但不同文档实例仍作为换文档处理，尚无整文档语义去重。

文档入口限制文本字节、段落数和样式数；preparation 使用 Core 共享预算，
覆盖输入物化、索引／布局元数据和受支持的文字阶段。真实 glyph backing、
外部 lease 留存占用与逐出计费还需单独建立组件合同。

## 验证与样例

```sh
cargo test -p novadraw-richtext --lib
cargo check -p novadraw-richtext --lib --target wasm32-unknown-unknown
cargo run -p rich-text-figure-app -- --headless
cargo run -p rich-text-figure-app
```

[单画布样例](examples/native/rich-text-figure-app/README.md)支持类图与富文本对象单击选择，
双击富文本进入目前仅能滚动的编辑模式。选择和模式入口属于 app 编排；
组件库的文本输入与光标能力尚未提供。[验证记录](../../doc/verification/reviews/rich-text-figure-foundation-2026-11-05.md)
区分代码、截图与尚未完成的人工／性能验收。
窗口缩放的 CPU 对照与 Perfetto 归因见
[缩放优化验证](../../doc/verification/reviews/rich-text-figure-resize-optimization-2026-11-05.md)。
样例默认打开长文压力档，也提供原始 27 段、混合字号／emoji 和超长单段；
数据规模、样例预算与测量口径见
[压力验证](../../doc/verification/reviews/rich-text-figure-stress-performance-2026-11-05.md)。
