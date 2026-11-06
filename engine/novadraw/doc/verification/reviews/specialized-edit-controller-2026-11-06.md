# ADR-061 专用控制器实施记录

类型：`verification`

能力归属：Editor 会话、输入仲裁与历史；richtext 领域草稿；应用模型及适配；
Core 场景发布；platform 输入 lease。日期：2026-11-06。

状态：公开协议、独立控制器与嵌入样例已实施。定向自动验证、Native 回放图像及
mixed/large CPU 采样已完成；平台组合与完整性能验收未关闭。
决策见 [ADR-061](../../adr/adr-061-edit-session-and-specialized-controller-boundary.md)，
验收分母见[矩阵](../plans/specialized-edit-controller.md)。

## 实际实现

`novadraw-editor::EditController`、typed key、descriptor 与 `EditSessionPlan` 经
既有 Request/Policy registry 接入单一 Viewer 活动槽。plain 贡献转换为同一协议，
不维护第二个会话。协调记录保存目标、版本、lease 与反馈；领域草稿保存在控制器中。

输入先修改候选克隆，成功发布反馈后才替换旧控制器。输入和选区回调不能更新旧
反馈组件或 bounds；新反馈几何回调只能写自己的目标。可恢复拒绝保留旧会话；
`ViewerError::InconsistentState` 与扩展 panic 进入 fault 清理，释放输入。
host snapshot 也经 Viewer 的失败边界查询。

`novadraw-richtext::RichTextEditController` 不依赖 Editor，持有文档、受检选区、
composition 及其取消基线。克隆共享不可变文档；同段编辑保留存活样式，Emoji
删除按字素处理，导航消费同版 `RichTextLayoutSnapshot`。跨硬段落替换明确拒绝。

Native 样例适配器只持有该控制器，不另存草稿或 selection。接受生成一个前后文档
替换 Command，模型通知更新正文；IME Commit 不直接生成文档历史。
白底应用提供遮盖及草稿反馈，选区、caret、preedit 使用同版布局。两份正文仍会
生成命令，当前策略不是渲染优化，也不是透明内容的通用显隐协议。

Tool 切换先完成 focus-loss 策略；拒绝时不激活新 Tool。文本拖选按当前 session
身份路由，不能驱动对象拖动。反馈宽度与候选窗裁剪读取已发布的 host viewport。

## 自动验证

以下命令使用默认系统 Cargo/Rust。此机编译使用系统 clang，并将构建缓存放在
`/tmp/novadraw-text-stage-target`；这不是项目工具链要求。

| 检查 | 结果 |
|---|---|
| Editor `--lib` | 52 项通过 |
| G3/G4 交互与编辑事务 | 8 + 20 项通过 |
| `p2_e02_direct_text_edit_contract` | 10 项通过：plain IME、字素、NoChange、历史、失效和反馈 |
| richtext `--lib` 与 `edit_controller` | 7 + 3 项通过：文档共享、同版几何、样式、composition 和相同文字替换后的选区收起 |
| 外部 `editor-open-protocol --lib` | 首轮 77 项通过、3 项成本用例按既有规则忽略 |
| 外部 `edit_session` | 最终 9 项通过：数字领域控制器、候选拒绝、命令恢复、panic、迟到 lease、资产释放、拒绝切换与未知状态 |
| edit-app `--bin` | 最终 3 项通过：独立 Runtime、字素导航及嵌入事务；拖选与精确 host 宽度断言通过 |
| release `--headless` | 两类宿主通过：真实源更新、composition、Emoji 删除、历史、首中尾定位与 resize/zoom |
| Clippy | 本次包以 `--no-deps -D warnings` 通过；含依赖的首次严格检查被并行 Core `projection.rs` 的 `type_complexity` 阻断 |
| Wasm | Editor/richtext `--lib --target wasm32-unknown-unknown` 通过 |
| 格式与文档 | 定向 rustfmt、diff whitespace 通过；`cargo xtask docs` 校验 517 份 Markdown，0 结构错误 |

公开测试入口：

```sh
cargo test -p novadraw-editor --lib
cargo test -p novadraw-editor --test p2_e02_direct_text_edit_contract
cargo test -p novadraw-richtext --lib --test edit_controller
cargo test --manifest-path tests/consumers/editor-open-protocol/Cargo.toml --lib edit_session
cargo test -p richtext-figure-edit-app --bin richtext-figure-edit-app
```

独立消费者的 controller 使用数字草稿，完全不含 `DirectTextEditState` 或 TextFlow。
测试检查取消后外部资产引用数恢复、反馈子树清空、命令拒绝后恢复同一 lease，
以及 retired/revision conflict 不提交旧草稿。引用数证据不等于任意文档留存容量验收。

## Native 图像

`--screenshot --replay-edit` 注入九组 preedit/commit，随后接受、undo、redo，再进入
文本会话。退出码为 0。检查的 2240 × 1520 图像中，中文、14–64 px 混合字号、
彩色 Emoji 和 caret 正常，未发现新旧正文叠字或黑帧。
截图位于本地
`target/visual-verification/screenshots/richtext-figure-edit-app_Editing_performance_1793944133.png`；
前轮 `1793942893.png` 也保留。局部对照
`target/verification/text-adr061/native-flag-comparison.png` 确认国旗与相邻 Emoji 均存在。

该图像证明这一 mixed 回放的真实 Vello 输出；不证明操作系统输入法候选窗口、
跨显示器 DPI 或既有长文黑帧已通过。最终修订的拖选及 host 宽度路径自动复验通过。

## CPU 成本

两类宿主使用相同文本、字体、1120 × 760 逻辑窗口与预算，关闭 tracing。
每档、每宿主 216 个样本；首、中、尾 × 插入、替换、删除各 24 个样本。
定位、滚动与选区设置排除在编辑耗时之外，回放检查插入→替换→删除恢复内容。
没有向 GPU 提交，CPU synthetic acknowledgement 不代表呈现延迟。

表中为插入后的 CPU 整帧 P50/P95，单位 ms：

| 负载／宿主 | 首 | 中 | 尾 |
|---|---:|---:|---:|
| mixed 20,772 字符，独立组件 | 17.428 / 17.934 | 17.266 / 17.957 | 17.367 / 17.748 |
| mixed，Editor | 10.088 / 10.620 | 10.081 / 10.395 | 10.313 / 10.660 |
| large 274,772 字符，独立组件 | 225.482 / 268.825 | 223.031 / 257.678 | 233.842 / 349.969 |
| large，Editor | 131.119 / 160.581 | 136.978 / 180.634 | 146.491 / 180.875 |

Editor 激活为 8.311/110.988 ms（mixed/large），接受为 0.420/7.595 ms。
组件样本分 mutation、layout、record；Editor 样本分 input+feedback、record、accept。
input+feedback 包含候选克隆、领域修改、布局和反馈更新，不能当作纯协调器开销。
独立宿主包含 RichTextFigure 内部 ScrollPane，嵌入宿主使用 ContentFigure 与 Viewer
Viewport；两类宿主的拓扑和发布方式不同。表中差异不归因于某一个算法，
也不宣称整体性能优化完成。
large 编辑仍远高于 16.7 ms 帧预算。

原始 JSON 位于 `target/verification/text-adr061/`：
`final-component-mixed`、`final-session-mixed`、`component-large`、`session-large`。
mixed 为交付前重采结果；large 为前一轮结果，各负载内部的宿主对照使用同一构建。
工作区同期存在 Core 修改，不将跨轮数据作为性能前后对照。
源数据、每组 P50/P95、冷启动及接受成本都保留在对应目录。

giant 回放首次在 Core 的 `frozen damage return` 检查处失败，原始错误为
`Coordinate(Projection(Input(PreparationInputError { name: "novadraw.viewport.projection",
kind: Changed { .. }, .. })))`。未生成完整 giant 样本，不用局部结果计算统计。
该失败是投影输入一致性证据，不能改记为 CPU 完成或 GPU 黑帧整改通过。

## 剩余范围

人工系统输入法、完整 Bidi/resize/zoom/DPI 组合、Web 原生输入和跨平台发布验收
保持开放。当前没有交互样式工具、剪贴板、跨硬段落拓扑编辑或草稿撤销栈。
后续通用透明内容显隐、独占协调器耗时与任意文档留存容量须分别定义和验证。
FR-02、长文增量排版与原长文黑帧保留各自状态。

工作区同期有 Core 投影/Animation 修改；若再次出现编译错误，不能把它当作
ADR-061 已通过最终工作区门禁。已有成功测试、运行和采样只证明所列基线。
本次为恢复定向构建，在 `animation/service.rs` 的 geometry source 回调补充
`Ok::<_, AnimationError>(changed)` 错误类型注解；未改变该回调的行为。
