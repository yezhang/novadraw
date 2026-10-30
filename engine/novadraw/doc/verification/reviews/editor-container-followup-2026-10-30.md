# E03 容器编辑：故障与平台输入补验

类型：`implementation-verification`

能力归属：Editor（`novadraw-editor`）。
Core revision 耗尽是依赖故障，平台消费者负责转发真实输入；
本次未修改 Core 或 Editor 的产品源码与公共 API。

状态：本轮补验通过，E03 仍为 `in_progress`。
已补实际移动／创建 Tool 的 revision 耗尽链路、浏览器 DPR 1／2 输入、
Native Retina 输入、取消与窗口外释放。跨显示器 DPI、
系统 DnD 及完整平台组合仍未关闭。
目标契约见 [ADR-055](../../adr/adr-055-container-editing-protocol.md)，
验收分母见[容器矩阵](../plans/editor-container-editing.md)。

## 真实 revision 耗尽如何进入 Tool

此前公开 API 无法将 64 位 revision 安全推进到耗尽，提前退休反馈也不能替代该故障。
本轮复制当前 Core 与 Editor 源码到隔离目录，仅在该副本启用
`e03_fault_fixture`，让测试能够设置已有 ConnectionRuntime 的私有 dirty revision。
设置后仍执行实际 `ContainerMoveTool`、`ContainerCreationTool`、
`remove_overlay_visuals` 和 Core disposal，不替换其结果或模拟清理失败。

测试使用真实投影连接，使每次反馈清理必须通过 Core revision 检查。
移动选择两个节点以取得两项反馈；创建的外部测试 Plan 返回两项等价反馈，
用于观察清理一项后拒绝第二项。后者只调整测试夹具的反馈数量，
不改变创建 Tool 或引擎对第三方反馈的处理。

| Tool | 私有计数器起点 | 实际结果 |
|---|---|---|
| 移动 | `u64::MAX` | 两项均存活，命令未构造 |
| 移动 | `u64::MAX - 1` | 一项退休、一项存活，命令未构造 |
| 创建 | `u64::MAX` | 两项均存活，命令未构造 |
| 创建 | `u64::MAX - 1` | 一项退休、一项存活，命令未构造 |

四项均检查模型、revision、选择与 focus 保持，history 为零。
拒绝保留 `Tool → Viewer → Runtime(Rejected)`；
Viewer 进入故障状态，Core Runtime 保持健康。
`last_projection_cleanup` 的退休前缀、存活后缀及 registry 所有权与实际树一致，
没有复活已经清理的反馈，也没有构造后续模型命令。

这是故障注入的端到端证据，不是用户通过公开 API 自然耗尽 revision 的证据。
注入只写计数器；副本中的额外方法不进入产品构建。
[注入补丁](../evidence/p2-e03-followup-2026-10-30/isolated-fault-injection.patch)、
[原始日志](../evidence/p2-e03-followup-2026-10-30/fault-contract.log)和
[源码指纹](../evidence/p2-e03-followup-2026-10-30/source-fingerprints.json)已保存。

独立[复现入口](../../../tests/consumers/editor-container-input/run_fault_fixture.py)
先检查指纹，再复制源码并应用补丁；复验成功的
[日志](../evidence/p2-e03-followup-2026-10-30/fault-replay.log)单独保存。

## 平台消费者与证据边界

新增独立 [editor-container-input](../../../tests/consumers/editor-container-input/README.md)
消费者，复用现行容器契约测试的模型、Request、Policy、Plan 和 Tool。
宿主只适配输入与 viewport 变化，不直接执行换父或创建模型对象。
它记录父关系、模型 bounds、投影视觉 bounds、历史、命令构造次数及反馈。

Web 的 SVG 和 Native 的 egui 矩形仅显示查询得到的几何，帮助定位输入。
本轮验证平台输入进入 Editor 后的编辑行为，不验证 Core paint、
Vello 合成或产品 GUI 的视觉对等。
验证依赖只属于独立消费者，未加入 root workspace 或产品默认 feature。

## 浏览器输入

隔离 Chrome 通过 Playwright 的 mouse／keyboard 与 CDP touch 输入操作页面。
每次 pointer 事件均检查 `isTrusted=true`；
没有通过 JavaScript 调用 Rust 接口替代鼠标手势。
浏览器内 DPR 1 和 2 分别建立独立上下文，不声称执行了物理显示器切换。

每种 DPR 的十二个检查点覆盖：

- 换父预览、释放及 undo／redo；预览不改模型，释放只构造一次 Command。
- 移动的 Escape 取消；反馈清零，父关系与历史保持。
- 指针移出 SVG 后由 pointer capture 接收释放；完成移到根容器。
- 嵌套点击创建与 undo／redo；新对象属于嵌套目标容器。
- 拖动创建预览、释放及 Escape 取消；取消不留下模型对象或历史。

两种 Tool 另在 DPR 2 通过真实 touch `pointercancel`。
四个 stationary pointer 场景分别覆盖移动／创建与 DPR 1／2：
按下并移动后，通过真实键盘输入改变 zoom 和 scroll，
之后不再发送 pointer move；检查更新后的反馈与释放后的投影 bounds 相同，
history 为一、命令只构造一次、反馈清零。

证据：[十二检查点原始记录](../evidence/p2-e03-followup-2026-10-30/browser-receipt.json)、
[pointercancel](../evidence/p2-e03-followup-2026-10-30/browser-cancel.json)、
[静止指针与视口](../evidence/p2-e03-followup-2026-10-30/browser-viewport.json)。
驱动器与日志在同一证据目录中保存。

TRAE 内置浏览器最初能读取状态，但 SVG click／drag 未产生指针事件，
拖动工具还发生超时；这些尝试不计为通过。
后续改用隔离 Chrome，只有检查到真实事件和模型结果后才形成上述结论。

## Native 输入

macOS 的独立验证窗口通过 CoreGraphics 投递系统鼠标和键盘事件，
由 eframe／winit 进入消费者，再调用 Editor 输入 API。
验收在实际 Retina 显示器进行，宿主 scale 为 2，本机只检测到一块显示器。

已观察并核对：

- 节点从左容器换到右容器，释放后 history 为一，构造一次命令。
- 通过实际按钮 undo／redo，父关系分别恢复到原父和新父。
- 拖动创建和嵌套点击创建；新对象在目标容器中，undo 删除，redo 恢复。
- 两种 Tool 的 Escape 取消与实际窗口失焦取消；模型、历史保持，反馈清零。
- 两种 Tool 从窗口内按下后拖到窗口外释放，仍收到 release，
  手势结束、history 为一、只构造一次命令且反馈清零。

取消后再收到 release 不会提交旧手势。
窗口外测试的外框为 `(434,123,860,668)`，释放点为屏幕逻辑坐标 `(1384,333)`，
位于右边界 1294 之外。该输入经系统鼠标捕获返回原窗口，
证据见[窗口外事件](../evidence/p2-e03-followup-2026-10-30/native-outside-events.jsonl)。
这只证明本机 macOS 的两项释放场景，不泛化到其他平台或跨显示器。
事件记录包含正式操作之前的试探输入，报告只采用出现实际状态变化的操作。
最初连续投递的部分快捷键没有产生观测结果，未据此声称快捷键通过；
undo／redo 结论来自已记录的原生按钮操作。

证据：[完整事件记录](../evidence/p2-e03-followup-2026-10-30/native-events-with-focus.jsonl)、
[最终桥接源码复验](../evidence/p2-e03-followup-2026-10-30/native-events-current-source.jsonl)、
[原生 baseline](../evidence/p2-e03-followup-2026-10-30/native-baseline.png)、
[嵌套创建画面](../evidence/p2-e03-followup-2026-10-30/native-nested-create.png)。
截图验证宿主示意图和状态，不替代模型事务断言。

## 回归与剩余工作

Native 与隔离故障构建使用 Rust 1.96.0，Wasm 使用 stable Rust 1.99.1，
wasm-bindgen 库与 CLI 同为 0.2.129。
消费者复用的容器契约 29 项通过、0 失败、2 项成本探针默认忽略；
定向严格 Clippy 与 Native／Wasm 构建通过。
没有变更产品代码，因此不重复 workspace 全量回归；
文档、独立消费者和复现脚本按受影响范围验证。

| 原缺口 | 本轮处置 |
|---|---|
| CE-15：真实耗尽进入移动／创建 Tool | 隔离源码计数器注入，实际 Tool 与 Core 清理四场景通过 |
| CE-06／11／13：DPI、创建和取消 | Web DPR 1／2、Native scale 2 的上述范围通过 |
| CE-14：静止指针下视口变化 | Web zoom／scroll 与反馈／提交一致通过；已有无窗口 resize／auto-expose 回归继续通过 |
| Native 窗口外 capture | macOS 两种 Tool 的窗口外释放通过；其他平台和跨显示器组合未验收 |
| 跨显示器 DPI | 未完成；单块 Retina 显示器不满足跨显示器条件 |
| 系统 DnD、完整组合矩阵 | 未完成；普通 pointer 创建不替代系统 DnD |
| CE-18：完整成本拆分 | 本轮未新增采样；模型复制、投影重建、反馈与稳定化拆分继续保留 |

本轮没有引入吸附能力，没有启动 E04 实施。
E03 的 remaining 条目以 [P2 backlog](../../roadmap/p2-delta-backlog.md)为准。

消费者源码与证据 SHA-256 见
[汇总收据](../evidence/p2-e03-followup-2026-10-30/final-receipt.json)。
成稿已分别回读事实、逻辑与中文表达，明确注入条件、平台范围和未关闭项；
未进行独立读者复核。正文半角标点与空泛元话语扫描为零命中，
见[表达检查](../evidence/p2-e03-followup-2026-10-30/chinese-review-final.json)。
