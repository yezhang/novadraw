# rich-text-figure-app

能力归属：`novadraw-richtext` 的 Native 样例。同一画布左侧用普通 Figure 子树显示类图，右侧使用
`novadraw-richtext` 的独立文档组件；平台输入和最终绘制复用共享 Native 宿主。
本样例不提供 Editor／GEF 编辑能力。

## 运行

在仓库根目录执行：

```sh
cargo run -p rich-text-figure-app
```

当前实现已用 macOS Native 截图路径核验。其他 Native 平台尚未运行验收；
独立组件的 Wasm 编译已通过，app 本身是 Native package。
需要支持 Rust 2024 Edition 的工具链及本仓库 Native 构建环境。
字体由 app 显式注册，使用仓库内置的 Inter、Noto Sans SC、Noto Sans Arabic
和 JetBrains Mono。包含 emoji 的压力档额外加载 macOS 自带的
`/System/Library/Fonts/Apple Color Emoji.ttc`，当前只支持该字体路径可用的环境；
其他平台可先运行 `--text-load basic`。

窗口只有一个 Runtime 和画布，类图背景透明。画布根包含类图和富文本对象组；
富文本组容纳组件和选择面，没有独立画布。正文包含标题、局部粗体和颜色、斜体、中文／英文／阿拉伯文，
默认显示多段长文压力档，高度超过视口，垂直滚动条始终显示。

使用 `--text-load basic|mixed|large|giant` 选择数据：

- `basic`：保留原来的 27 段，用于比较已有缩放基线。
- `mixed`：14／18／28／36／40／64 px，包含同段混合字号，以及肤色、职业／家庭 ZWJ、
  旗帜和键帽 emoji。
- `large`（默认）：混合字号和 emoji 展示后，追加 128 个长段落及一个更长的段落。
- `giant`：同样的展示区，加上 400 次重复语料构成的单个超长段落。

```sh
cargo run --release -p rich-text-figure-app -- --text-load mixed
cargo run --release -p rich-text-figure-app -- --text-load giant
```

长文和超长单段档使用 app 的 preparation budget：work 80,000,000，
allocation 32,000,000。其单位沿用 Core 的累计计费协议，不能把 allocation 直接当成
实际内存字节。原样例和混合档保持 Core 默认预算；库默认策略没有改变。

单击类框、关系线、标题、关系标签或富文本卡片可选择对象，显示蓝色边框。
成员文字归属所在类框，点击它会选中整个类框。当前是互斥单选，
点击画布空白清除选择；没有对象移动或多选。

双击富文本卡片进入编辑模式，边框变绿，此时正文滚轮和滑块拖动可用。
点击其他对象或空白退出编辑模式；普通选择状态不滚动正文。
本版编辑模式只开放内部滚动，文字输入、光标和文本选择留待后续添加。
缩放窗口时，左右区域重新安排，正文按视口宽度换行。
小窗口会裁剪类框，当前没有类图自动缩放或编辑。
共享宿主提供 `R` 重建、`S` 截图和 `Esc` 退出。
方案中的 F1–F4 文档控制留待后续切片，本版没有注册这些功能键。

## 验证

```sh
cargo run -p rich-text-figure-app -- --headless
cargo run -p rich-text-figure-app -- --headless --text-load mixed
cargo run -p rich-text-figure-app -- --screenshot
```

`--headless` 实际构造 Runtime，并投递鼠标事件，验证六类对象选择、焦点互斥、
双击进入编辑、选择模式滚轮阻断、编辑模式滚轮与滑块 capture、点击退出及空白清除。
同时验证正文高度、滚动范围、类图位置隔离、
typed 文档替换、相同快照 NoChange、缩窄重排及错误文档身份拒绝。
它还检查 app 根只有两个内容孩子，正文内容面没有段落子 Figure。

`--screenshot` 使用共享宿主捕获 Vello 绘制结果后退出，图片位于
`target/visual-verification/screenshots/rich-text-figure-app_*.png`。
该图片证明场景输出，不等于桌面截图或真实鼠标验收。
本地验证可用 `--target-dir /tmp/novadraw-text-stage-target` 复用已有缓存。

已修复 `No segmentation model for complex script: Chinese/Japanese`：
workspace 的 Parley 0.12.0 依赖启用官方 `complex-scripts` feature，
为复杂文字分析加载 ICU4X 词典。中文／日文词边界回归通过，
修复后的无窗口和 Native 截图运行均未出现 ICU4X 告警。
完整复杂文本断行与字体覆盖仍须单独验收。

## 缩放成本采样

```sh
cargo run --release -p rich-text-figure-app -- --benchmark-resize --output /tmp/richtext-resize
cargo run --release -p rich-text-figure-app -- --benchmark-resize --text-load basic --output /tmp/richtext-basic
cargo run --release -p rich-text-figure-app --features profiling -- --benchmark-resize --trace --output /tmp/richtext-resize-trace
```

固定回放包含连续宽度变化、仅高度变化与 Core 滑块拖拽事件，共 240 个操作。
`samples.json` 记录 mutation／稳定化及绘制命令生成的 CPU 耗时；
`metadata.json` 记录数据档、字符和字节数、最长段落、样式数量、准备预算，
以及场景创建至首次稳定化的耗时（包含字体加载，不包含首次 GPU 绘制）；
`--trace` 另生成 `resize.pftrace`。回放不创建窗口，也不执行 GPU 提交；
其帧回执只是采样宿主对 CPU 命令记录的确认，不代表真实图像发布。
计时对照须关闭 tracing，阶段归因另行采集。
结果见[缩放优化验证](../../../../../doc/verification/reviews/rich-text-figure-resize-optimization-2026-11-05.md)。
不同字号、emoji 与长文的实测见
[压力验证](../../../../../doc/verification/reviews/rich-text-figure-stress-performance-2026-11-05.md)。
当前 `mixed` 的 Native 图像已核对；`large` 的 Native 截图出现黑帧，
虽然其无窗口回放和输入检查通过，仍不能视为绘制通过。需要查看已验证的字号和 emoji
时先使用 `--text-load mixed`；长文档保留为暴露失败的压力数据。

当前优化复用同一约束下的完整快照，没有跨宽度增量排版承诺。
后续真实 Native 基线仍须覆盖类图负载、滑块拖拽和连续 resize；
不能直接用它与纯 TextFlowLarge app 的总帧时间比较。
实现范围见[组件 README](../../../README.md)，
证据见[基础切片验证](../../../../../doc/verification/reviews/rich-text-figure-foundation-2026-11-05.md)。
