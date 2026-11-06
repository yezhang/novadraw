# Richtext Figure Edit

归属：richtext 提供领域控制器；Editor 协调会话、反馈与文档历史；
应用持有提交模型；Core 发布正文和反馈；Winit 桥接原生输入。

Native 使用 ADR-061 的统一会话。点击文字放置光标，拖动或 Shift+方向键选择，
输入文字或中文 preedit，按 Ctrl+Enter 接受整个草稿，Esc 取消。
Ctrl/Cmd+Z 先接受当前草稿再撤销文档命令，Shift+Ctrl/Cmd+Z 重做。
IME Commit 只改草稿，接受会话才产生一项历史。取消和 NoChange 不增加历史。

当前支持同段替换、字素删除、视觉及词边界导航；Ctrl+A 选择当前硬段落。
跨硬段落替换、段落合并、剪贴板与交互样式工具尚未提供。选区可跨段显示，
尝试跨段改写会明确拒绝。Enter 插入当前段落中的软换行。

```sh
cargo run --release -p richtext-figure-edit-app
cargo run --release -p richtext-figure-edit-app -- --text-load large
cargo run --release -p richtext-figure-edit-app -- --text-load giant
cargo run --release -p richtext-figure-edit-app -- --headless
cargo run --release -p richtext-figure-edit-app -- --screenshot --replay-edit
```

`mixed` 约 2.1 万字符，含混合字号和 Emoji；`large` 约 27.5 万字符；
`giant` 将长文集中为一个段落。F2/F3/F4 在头部、中部、尾部插入 `编辑🙂`；
F5 重放九次插入，F6/F7 定位首尾。窗口标题报告输入及反馈、CPU 命令记录成本。

两类宿主复用同一 `RichTextEditController`，各自只有一份领域草稿。
`--benchmark-edit` 测独立组件直接发布，`--benchmark-session` 测 Editor 候选输入、
反馈布局及模型接受。两者使用相同文本、字体、窗口和回放轮数。
头、中、尾的插入、替换、删除每组 24 个样本；定位和选区设置不计入编辑耗时。

```sh
cargo run --release -p richtext-figure-edit-app -- --benchmark-edit --output target/verification/text-adr061/component
cargo run --release -p richtext-figure-edit-app -- --benchmark-session --output target/verification/text-adr061/session
cargo run --release -p richtext-figure-edit-app --features profiling -- --benchmark-session --trace --output target/verification/text-adr061/trace
```

嵌入模型保存不可变文档，Command 只持有前后文档，模型通知刷新正文。
草稿正文使用 `RichTextContentFigure`，不 flatten 成 TextFlow；caret、selection 和
preedit 来自同版 `RichTextLayoutSnapshot`。应用白底遮盖反馈隔开旧正文和草稿；
当前仍记录两份正文命令，这项成本包含在 Editor 回放中。

回放注入的 preedit/commit、Native 截图和无窗口测试不能代替人工系统输入法验收。
CPU 回放没有 GPU 提交；既有长文黑帧与全文装配成本继续开放。
Emoji 使用 macOS `/System/Library/Fonts/Apple Color Emoji.ttc`。
