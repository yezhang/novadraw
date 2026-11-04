# E03 容器编辑：组合输入与成本拆分续验

类型：`implementation-verification`

能力归属：Editor（`novadraw-editor`）。本轮修改独立平台验证消费者，
未修改 Core／Editor 产品源码或公共 API。

本轮浏览器组合输入和成本拆分通过，E03 保持 `in_progress`。
新增浏览器 DPR 1／2 共 46 个检查点；消费者契约 32 项通过，
2 项成本测试默认忽略，成本专项另行执行。
跨显示器 DPI、Native 系统文件 DnD、完整 Native 平台组合和部分工作量计数仍未完成。
本页承接[上一轮补验](editor-container-followup-2026-10-30.md)，
不改写原始证据，也不将其中的 Native 正例推广到本轮未执行的组合。

目标契约见 [ADR-055](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-055-container-editing-protocol.md.txt)，
验收分母见[容器矩阵](../plans/editor-container-editing.md)，
执行状态在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 浏览器组合输入

复用独立 [editor-container-input](../../../tests/consumers/editor-container-input/README.md)
消费者的真实移动／创建 Tool、外部模板、Policy 和模型 Command。
宿主新增多选、有序目标、模型 revision、policy 失效和 Tool 切换入口。
这些操作使用实际键盘输入；窗口尺寸改变通过 ResizeObserver 更新逻辑 viewport。

Playwright 驱动隔离 Chrome，分别建立 DPR 1 和 2 的上下文。
每种 DPR 的 23 个检查点如下。

| 范围 | 检查点数 | 核对结果 |
|---|---:|---|
| 多选、有序插入、undo／redo | 4 | 两节点共同预览；目标顺序为 `[4, 5, 8]`，相对间距保持；释放只构造一次命令 |
| 移动／创建的 revision、policy 失效与 Tool 切换 | 12 | 六种组合各检查预览和释放；取消后无反馈、命令和历史，模型保持，Viewer 健康 |
| 移动／创建的静止指针、zoom、scroll 和实际窗口 resize | 4 | 两种 Tool 各检查预览和释放；视口改变后不增加 pointer move，反馈与最终投影视觉 bounds 一致 |
| HTML 模板 drop、undo／redo | 3 | drop 到嵌套容器 6；父域偏移后 bounds 为 `[25, 36, 30, 20]`，一条历史，撤销重做恢复相同关系 |

所有观测到的 pointer 事件均为 `isTrusted=true`；
模板的 `dragstart` 和 `drop` 也均为浏览器真实事件。
HTML drag/drop 使用浏览器 DataTransfer，再进入同一个
`EditorDomain::drop_into_container`，没有直接修改模型。
它证明浏览器模板接入，不证明操作系统文件 DnD。

首轮模板 drop 得到 `[26, 37, 30, 20]`，比目标父域偏移多 1 像素。
原因是验证宿主只扣除 SVG 外框左上角，没有扣除边框。
本轮统一按 `clientLeft／clientTop` 扣除边框，pointer 和 drop 共用内容域映射；
驱动器的鼠标坐标也以内容域为起点。修正后重新执行两种 DPR 的全部新增检查点。
这项修复只涉及验证宿主输入适配。

证据：[46 个检查点](../evidence/p2-e03-combinations-2026-10-30/browser-receipt.json)、
[通过日志](../evidence/p2-e03-combinations-2026-10-30/browser.log)、
[首次坐标断言失败](../evidence/p2-e03-combinations-2026-10-30/browser-first-failure.log)。
复现入口：[verify_browser.cjs](../../../tests/consumers/editor-container-input/verify_browser.cjs)。

## 成本拆分

此前的完整提交成本混合了应用模型复制、模型通知、投影更新与稳定化。
本轮在隔离源码副本中对实际执行边界计时，继续使用原 scaling fixture：
16／64 个源父，以及一个源节点下的 32／2048 个后代，每组 32 个样本。
先由真实 `pointer_moved` 获取反馈，再测量 release 和随后用于查询的稳定化。
未用空反馈清理时间代替已获取反馈的释放成本。

计时器使用线程局部固定数组，不申请堆内存；
计时开销仍会影响结果，不将数据解释为未插桩产品的精确延迟。
模型复制计时包含实际 Command 的完整快照复制，没有替换应用算法。
projection 计时覆盖退休集合处理、连接准备、子树退休／刷新／重建和连接同步。
该范围仍将刷新与重建合并记录，未声称每项内部算法都已单独归因。

macOS／aarch64，Rust 1.96.0，release，默认 feature。
后台负载未受控；本轮只报告当前场景的观测，不与旧采样作稳定提速比较。
下表均为 p95，单位毫秒。

| 场景／节点数 | release | 模型复制 | snapshot | 投影更新 | 已有反馈清理 | 稳定化累计 |
|---|---:|---:|---:|---:|---:|---:|
| 16 源父／40 | 0.354 | 0.004 | 0.007 | 0.154 | 0.069 | 0.049 |
| 64 源父／136 | 1.856 | 0.013 | 0.022 | 0.848 | 0.623 | 0.138 |
| 32 后代／42 | 0.309 | 0.002 | 0.006 | 0.224 | 0.005 | 0.057 |
| 2048 后代／2058 | 15.388 | 0.101 | 0.309 | 13.856 | 0.302 | 1.201 |

模型复制包含在命令构造／执行计时内；稳定化累计包含 release 内调用和 release 后调用。
各字段的 p95 来自各自样本排序，不能将表中分位数相加还原单次总耗时。
单独的 release 后稳定化 p95，四组分别为 0.008、0.016、0.006、0.269 毫秒。
预览反馈挂载 p95 分别为 0.037、0.141、0.003、0.019 毫秒。
release 复用已有反馈，所以该阶段的反馈挂载计时为零，不能解释成挂载无成本。

在 2058 节点夹具中，投影更新明显高于模型复制和 snapshot；
下一轮性能设计应先定向检查子树退休／重建与刷新范围。
64 源父的反馈清理也有明显成本，需要分别检查 registry 和批次清理路径。
这些结果支持定位下一步研究范围，不授权本轮新增差量模型协议或缓存机制。

证据：[逐样本数据](../evidence/p2-e03-combinations-2026-10-30/cost-samples.json)、
[汇总](../evidence/p2-e03-combinations-2026-10-30/cost-summary.json)、
[原始日志](../evidence/p2-e03-combinations-2026-10-30/cost-cost.log)、
[插桩差异](../evidence/p2-e03-combinations-2026-10-30/cost-instrumentation.patch)、
[完整源码指纹](../evidence/p2-e03-combinations-2026-10-30/cost-complete-source-fingerprints.json)。
复现入口：[run_cost_fixture.py](../../../tests/consumers/editor-container-input/run_cost_fixture.py)。
首次插桩构建因诊断模块缺少文档失败，补齐文档后通过；
[首次日志](../evidence/p2-e03-combinations-2026-10-30/cost-first-compile-failure.log)保持原始内容。

## Native 与环境限制

本机 CoreGraphics 只列出一块 active display；Native 宿主初次画面显示 scale 2。
这个环境不能证明跨显示器 DPI 切换。

Native 宿主新增系统文件模板接入和输入坐标记录。
构建、严格 Clippy 通过；初次验证窗口可见，但试探输入没有构造模型命令，
因此不计为新增 Native 行为验收。
重启后发现进程仍存在、可访问窗口查询无有效窗口，新的事件文件为空。
stderr 只出现 macOS 可恢复状态警告，没有足够证据将窗口问题归因为 Core 缺陷。
本轮没有取得系统文件 drop 事件或对应模型提交。
已关闭本轮验证应用和独立 Finder 源窗口。

证据：[初次画面](../evidence/p2-e03-combinations-2026-10-30/native-started-window.png)、
[未计入验收的输入](../evidence/p2-e03-combinations-2026-10-30/native-unaccepted-events.jsonl)、
[重启事件文件](../evidence/p2-e03-combinations-2026-10-30/native-restart-events.jsonl)、
[stderr](../evidence/p2-e03-combinations-2026-10-30/native-restart-stderr.log)、
[Native Clippy](../evidence/p2-e03-combinations-2026-10-30/native-clippy.log)。
上一轮 macOS 换父、创建、取消和窗口外释放证据继续保留；
本轮没有重新执行或扩张其通过范围。

## 验收状态

| 条目 | 本轮结果 | 仍缺什么 |
|---|---|---|
| CE-08／09：多选、有序插入 | 新增 Web DPR 1／2 输入和历史证据 | 全部 Native 输入组合 |
| CE-11：嵌套 drop | 浏览器真实 HTML drop 与父域偏移通过 | Native 系统文件 DnD |
| CE-13：失效与换 Tool | 两种 Tool 的三种失效组合在 Web 通过 | Native 对应组合 |
| CE-14：静止指针视口变化 | Web zoom／scroll／实际 resize 与释放一致通过 | Native 对应组合；其余矩阵沿用原证据 |
| CE-18：阶段耗时 | 模型复制、snapshot、投影、反馈与稳定化分别记录 | handler 内部扫描、完整缓存失效与工作量计数；刷新／重建内部细分 |
| 跨显示器 DPI | 未验收 | 两块显示器及真实跨屏手势 |

消费者新增三个桥接契约测试，验证父域 drop 偏移、批次顺序／间距，
以及六种失效组合不构造命令。加上复用契约，共 32 项通过、0 失败，
2 项成本测试默认忽略；成本专项单独通过。
Native／Wasm 构建、消费者库和 Native target 的严格 Clippy 通过。
本轮未修改产品源码，因此不重复 workspace 全量编译测试。
文档门禁和 `git diff --check` 通过，见
[文档日志](../evidence/p2-e03-combinations-2026-10-30/docs.log)。
验证脚本语法、原始证据 SHA-256、检查点数与成本样本数均已核对。

成稿按 technical-writing-clarity 和好好说话回读事实、逻辑与中文；
未进行独立读者复核。原始证据指纹见[收据](../evidence/p2-e03-combinations-2026-10-30/receipt.json)。
