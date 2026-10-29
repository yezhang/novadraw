# 三种布局的真实窗口验收

类型：`manual-verification`

能力归属：平台／宿主验收，验证既有 Core 布局消费者的实际窗口链路；
本次没有新增 Core 布局能力或 Editor／GEF 编辑能力。

日期：2026-10-29

状态：macOS 系统窗口 resize、稳定显示与恢复检查通过。
鼠标拖拽 resize、控件矩阵和跨平台呈现尚未由本次验收。

## 本次检查实际屏幕

用户要求先完成 FlowLayoutExample、GridLayoutExample 与 ToolbarLayoutExample 的
真实窗口验收。本轮运行官方交互二进制，通过 macOS System Events 的窗口属性
接口调整原生窗口，使用 PageUp／PageDown 切换实际场景。
没有调用 `--screenshot`，没有直接向 Runtime 注入 resize 或替换场景状态。

窗口截图通过 `screencapture -R` 读取实际显示器像素，包含原生标题栏和 egui 面板。
交互宿主的路径为 `WindowEvent::Resized` → `request_surface_redraw` →
`resize_logical_viewport` → `renderer.submit`。屏幕上的新尺寸、重排和恢复画面
证明这条路径产生了可见输出；本次没有采集逐事件日志、帧完成计数或显示延迟。

启动使用[原生启动脚本](../../../tools/scripts/run_draw2d_macos.py)，构建耗时
1 分 05 秒。系统进程的 executable 路径指向本 checkout 的
`target/official-native/Draw2D Official Examples.app/Contents/MacOS/draw2d-official-examples`，
并通过辅助功能查询核对窗口身份。应用继续运行，没有重启其他会话的进程。

## 环境与操作顺序

环境为 macOS 14.7.8（23H730）、Apple M1 Pro、Rust 1.99.1。
显示器为内置 Color LCD，3456×2234 Retina；本次截图为每逻辑单位 2 像素，
没有执行系统 DPI 切换。
代码基线为 `4f104633fcb7c555d165cc20d2902ab554aa6d4a` 加既有 S08 工作树修改。
二进制和相关源文件散列保存在
[收据](../evidence/layout-native-window-2026-10-29/receipt.json)及
[源文件指纹](../evidence/layout-native-window-2026-10-29/source-fingerprints.json)中。

依次验收 Flow、Grid、Toolbar。每个场景使用同一窗口，左上角为逻辑坐标
`(80,60)`。先固定外框 1200×828，再缩到 820×628、620×428、420×328；
随后连续改变 21 次宽度，固定外框高度 628，最后恢复 1200×828。
外框高度包括 28 单位标题栏，因此基线内容区为 1200×800。

连续宽度序列为：
`500,620,740,860,980,1100,1200,1100,980,860,740,620,500,420,500,620,740,860,980,1100,1200`。
每次系统请求之间留 0.08 秒，稳定截图前留 0.7～0.8 秒。
这是系统窗口连续 resize 请求，不是人工拖动边框；系统可能合并事件，
请求次数不能当作实际收到的事件数或呈现帧数。

## 画面与恢复结果

每个场景保存 baseline、medium、narrow、minimum、restored 五张原始截图。
[总览](../evidence/layout-native-window-2026-10-29/contact-sheet.png)按场景和操作顺序展示，
原图与 SHA-256 见收据。

| 场景 | 实际观察 | 恢复 Canvas 差异 |
|---|---|---|
| Flow | 初始 11 个图形单行；缩小时分行；极窄时内容超出可见高度并被裁剪；恢复后重回单行 | 0／2,667,100 像素 |
| Grid | 初始两个图形保持两列；Canvas 右边和底边跟随窗口；极窄时保留自然尺寸并裁剪；恢复后边界与图形恢复 | 0／2,286,960 像素 |
| Toolbar | 初始五个图形保持单行；缩小时 Canvas 边界变化，超出的图形被裁剪；恢复后五个图形完整出现 | 0／2,668,140 像素 |

逐像素比较只取 Canvas 区域，排除窗口装饰和临时控制面板。
比较框采用原始截图像素坐标：
Flow `(20,80,1810,1570)`、Grid `(780,134,2340,1600)`、
Toolbar `(20,112,1850,1570)`。三组 RGB 差值均为零，
覆盖所选区域中的图形、边界与清空背景。
这证明稳定恢复画面的可重复性，不宣称连续 resize 中每一帧都已截图或无闪烁。

## 仍需保留的边界

本次 System Events 的坐标 click 返回成功，但没有观察到 Flow checkbox 状态变化。
因此没有把它当作方向、stretch 或其他控件操作成功。
Grid 的 Add／Clear／选择／span／grab，以及 Flow／Toolbar 的方向、对齐和 stretch
沿用[既有消费者证据](official-layout-caret-consumers-2026-10-27.md)；
本轮没有重新完成它们的物理鼠标窗口操作。

窗口激活和场景读取最初存在异步时序，曾捕获到编辑器或错误场景。
这些初探截图未进入验收原图；正式批次在操作前验证 `[07/25]`、`[08/25]`、
`[09/25]`，各阶段窗口标题与尺寸均一致。

没有 Windows／Linux、浏览器、跨显示器 DPI、上游原生栅格对照或输入响应性能证据。
三项 catalog 继续为 `in_progress`，只更新已通过的本机系统 resize 与屏幕恢复范围。
不修改历史报告，也不把本次恢复比较当作新的 Partial／Full 帧模式统计。

## 检查与交付

本轮没有修改布局算法、事件分发或渲染主循环。新增原生启动脚本与 Run action，
更新验收记录和剩余范围；既有 S08 工作树修改保持。
原图、收据和总览保存在本记录引用的证据目录。
启动脚本语法和 Run 配置契约检查通过；文档门禁已通过 455 个 Markdown 与
当前 xtask 引用检查。差异空白检查通过。

交付前按好好说话技能复核中文，保留平台、次数、单位、观察范围与待验收状态。
编号标题、翻案句、元话语、中文半角标点和破折号机械扫描均为零命中。
