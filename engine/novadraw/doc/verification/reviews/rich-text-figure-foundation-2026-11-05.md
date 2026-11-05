# RichTextFigure 基础切片验证

类型：`verification`

能力归属：`novadraw-richtext` 拥有文档、段落索引和 Figure 适配；
`rich-text-figure-app` 拥有类图、单画布对象选择与双区域样例。Core 的既有文字、滚动、
输入和绘制协议被复用，本切片没有增加 Editor／GEF 能力。

日期：2026-11-05。状态：基础实现、Native 图像检查和定向门禁通过；
人工交互、复杂文本完整正确性及性能门禁未关闭。
用户已授权按[RichTextFigure 方案](../../design/architecture/rich-text-figure-proposal.md)
先构建核心数据结构，再给出能力简单的独立 app。

## 实际交付

新增平台无关 `crates/novadraw-richtext` 和独立
`crates/novadraw-richtext/examples/native/rich-text-figure-app` workspace package。
样例按用户要求随 richtext crate 维护，保留独立应用包和原运行命令。
正文由单个 `RichTextContentFigure` 拥有，
通过 Figure preparation 将完整段落布局发布为不可变 presentation。
外壳是真实 `RichTextFigure`，包含 Core ScrollPane；垂直条始终显示、
水平条隐藏、内容跟随视口宽度。它没有第二套 GPU 或字体服务。

文档模型包含受检段落输入、私有 Ropey 1.6.1 backing、
稳定 ParagraphId、DocumentVersion 和共享段落目录。
单段替换保留身份，纯文本段内编辑拒绝非法 UTF-8 端点。
文档版本包含快照身份，避免同源不同分支在相同修订号上冲突。
typed 替换校验预期版本，沿 Core 属性日志发布文档变化，
相同快照更新返回 NoChange。

高度索引为持久化平衡二叉摘要树，累计位置与单高度更新沿树路径完成。
其逻辑可见范围不构成墨迹证明。当前 preparation 完整排版、完整重建高度索引，
paint 提交全部段落，由 Core 裁剪；尚未建立段落资产缓存。

## 验证结果

以下命令使用系统 Rust 1.99.1，复用 `/tmp/novadraw-text-stage-target`。

| 验证 | 结果及范围 |
|---|---|
| `cargo test -p novadraw-richtext --lib` | 3 项通过：Unicode／版本／未变段落共享、拒绝准入后的 builder 状态、索引与线性参考对照及持久化更新 |
| `cargo clippy -p novadraw-richtext --lib -- -D warnings` | 通过 |
| `cargo clippy -p rich-text-figure-app --bin rich-text-figure-app -- -D warnings` | 通过 |
| `cargo check -p novadraw-richtext --lib --target wasm32-unknown-unknown` | 通过；不代表 Web app 运行 |
| app `--headless` | 27 段；正文高度 2736，视口 680；范围可滚动、类图位置隔离、typed 编辑、相同快照不失效、缩窄重排及错误实例拒绝通过 |
| app `--screenshot` | Native Vello 截图成功并已查看；静态类图、中文／英文／阿拉伯文、混合字号／颜色／粗斜体及内部滚动条可见 |

截图使用共享宿主的渲染结果捕获，不是桌面窗口截图。
首轮截图发现外壳沿用了 Rectangle 的蓝色默认背景且正文贴边；
本切片已改为明确的白色外壳及 MarginBorder 内边距，第二轮截图已核对。
未改动 Core 渲染主循环、坐标、事件分发或后端实现。

本地产物：

- `target/verification/rich-text-figure-headless.log`
- `target/verification/rich-text-figure-native.log`
- `target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793881638.png`

上述日志与图片不进入 Git。没有测量产物体积。

## 尚未交付与后续验收

追加的单画布交互切片已通过 app Clippy 与 `--headless`：
类图透明背景与富文本共享一个 Runtime；类框（含成员文字）、关系线、
标题、关系标签和富文本可单击互斥选择。双击富文本启用内部滚动，
点击其他对象或空白退出。蓝框表示选择，绿框表示编辑。
选择状态由 Core focus 驱动，边框及命中变化通过 app 私有 typed update 提交。

无窗口验证实际投递 press／release／double-click／wheel／drag，
检查旧选择清除、浏览时滚轮阻断、编辑时滚动隔离、
滑块 capture 保持和释放、滑块操作不夺走编辑焦点，以及退出后恢复选择面命中。
日志：`target/verification/rich-text-figure-selection-headless.log`。
单画布 Native 截图已生成并查看：
`target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793883251.png`；
类图背景透明，富文本卡片、正文和滚动条共同出现在画布中。
Native 日志：`target/verification/rich-text-figure-selection-native.log`。
本轮 `cargo xtask docs` 通过（508 份 Markdown，0 错误）。
编辑模式目前只有滚动能力；本切片没有文本输入、光标或文本选择。
这些验证不等于 macOS 真实鼠标与原生双击验收。

本次不是整份方案的完成声明。当前按段保存 Rope，目录更新仍为数组复制；
样式要求预解析成不重叠 runs。跨段编辑、split／merge 身份映射、样式覆盖树、
下划线、对外 caret／range 查询、完整键盘／IME／撤销、F1–F4 控制均未交付。
`add_to` 用于 detached builder，不宣称 attached 复合挂载原子性。
文档及 preparation 有限额，但真实留存容量、外部 lease 与逐出计费尚未验收。

当前无段落布局缓存。不能宣称纯滚动零排版、宽度变化零 shaping、
可见行输出或相对 Flow 的净收益。
首次打开、修改、滚动与 resize 的匹配 release 成本及 Perfetto 归因留待下一片；
每个优化点仍须比较前后时间，并覆盖真实拖拽和窗口缩放。
本版 27 段是起步样例，不是旧 TextFlowLarge 40 段的性能等价负载。

基础切片最初在中文排版时触发 Parley／ICU4X 的
`No segmentation model for complex script: Chinese/Japanese` 告警。
本轮修复已关闭缺失分词模型告警，证据见下一节；
完整语义断行对照和字体覆盖仍未关闭。
Native 真实滚轮、滑块 capture、切走再切回、移除后 handle 和跨平台输入仍待验收。
以上结果不关闭 FR-02／LT 或整体发布门禁。

## ICU4X 分词模型配置修复

能力归属：Core 默认 Parley 适配器的依赖配置；样例用于消费者验证。
根因是 Parley 0.12.0 默认未启用 `complex-scripts`。上游
`parley_engine/src/analysis.rs` 在该 feature 关闭时使用
`WordSegmenter::new_for_non_complex_scripts`，ICU4X 2.3.0 遇到中文／日文时
在 `complex/mod.rs` 报告缺失分词模型。启用后切换为词典分词，
复杂文字断行也采用上游词典路径。修复只调整 workspace Parley feature；
版本、上游源码和日志行为保持原样。

新增 `cjk_word_navigation_uses_dictionary_boundaries` 回归，验证中文
“你好世界”和日文“こんにちは世界”的前后词边界导航。
修复前中文 NextWord 返回字节位置 3，预期词边界为 6，同时输出 ICU4X 告警；
修复后中文和日文前后导航均通过。

| 检查 | 本轮结果 |
|---|---|
| Core `render::text::tests` | 13 项通过，1 项成本采样按原设置忽略 |
| app `--headless` | 27 段、正文高度 2736、视口 680；选择／滚动／typed 编辑／resize 检查通过；ICU4X 告警 0 次 |
| app `--screenshot` | Native Vello 绘制成功并已查看；ICU4X 告警 0 次 |
| workspace fmt/check | 通过；已有 `block 0.1.6` future-incompatibility 提示保留 |
| Core `--lib` Clippy | `-D warnings` 通过 |
| richtext `--lib` Wasm check | 通过 |

本地日志位于 `/tmp/novadraw-cjk-{word-regression,text-tests,richtext-headless,richtext-native,workspace-check,wasm-check,clippy}.log`。
新截图为
`target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793883639.png`。
原始基础切片日志和截图保留；本轮没有测量性能或产物体积。
