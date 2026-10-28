# Zoom 滑块输入修复

类型：`verification`

日期：2026-10-26

## 原因

共享宿主使用 egui-winit 的 `response.consumed` 判断按钮事件归属。
该判断读取上一轮 egui pass 的鼠标位置。鼠标从浮动面板移动到 Zoom 滑块后，
若按下发生在下一轮重绘前，面板悬停状态仍为真，宿主提前返回，
Core 未收到按下事件，也无法建立 ScrollBar 捕获。

修复前，`canvas_press_after_leaving_panel_does_not_wait_for_a_redraw` 失败；
直接经平台适配器向 Core 投递拖动则通过，说明问题位于共享宿主的输入过滤。

## 修复

宿主在面板过滤前记录所有鼠标移动，并在离窗时清除位置。移动、按钮和滚轮使用
最新物理位置换算后的 egui 坐标判断面板归属，同时保留面板正在使用鼠标的状态。
Canvas 已捕获时，移动和按钮事件继续交给 Canvas。键盘继续使用原有消费结果。

未修改 Core ScrollBar、缩放算法或渲染主循环；未增加依赖和资源。

## 验证范围

`example.zoom-input` 编排共享面板与实际 Zoom 消费者测试，覆盖：

- 面板悬停后未重绘即按下 Canvas，以及未重绘即进入面板。
- 面板滑块拖动越过面板边界时仍保留输入。
- 实际 Zoom 滑块的按下、连续移动、时间推进、面板刷新、缩放联动和释放。
- Canvas 捕获后移入面板继续拖动，释放后移动不再改变缩放。

上述测试无需窗口，未证明实际窗口事件循环或 surface 呈现通过人工验收。
人工核验：重新启动官方示例，切换 ZoomExample；从面板移到顶部滑块立即拖动，
观察类图连续缩放，再检查松开后停止变化。

## 执行结果

`example.zoom-input` 通过：共享面板 6 项、Zoom 5 项测试全部通过。
工作区格式检查及 `git diff --check` 通过。
`cargo check --workspace` 与 `cargo clippy --workspace -- -D warnings` 通过。
`cargo test --workspace` 通过：1,483 项通过、0 项失败、6 项既有忽略。
官方示例主程序重建通过。Tree 浮动面板的选择与双击回归同时通过。
`cargo xtask docs` 通过：375 个 Markdown 文件及当前 xtask 引用检查通过。
