# 布局消费者后续核验

类型：`verification`

日期：2026-10-23

范围：FlowLayoutExample、ToolbarLayoutExample、GridLayoutExample 的既有 Canvas 契约。
本批承接 [剩余工作](../plans/draw2d-official-examples-next-slices-2026-10-23.md)，
执行状态由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 修复了示例配置与窗口布局

Draw2D `FlowLayout.layout` 用 `minorSpacing` 累加行内位置，用 `majorSpacing` 推进
下一行。示例原来将两者反向传入 Core。现改为 `minor_spacing → with_spacing`、
`major_spacing → with_row_spacing`，保持 Core 的元素间距与行间距 API 不变。
用不等间距 7／13 分别验证横向和纵向，防止默认值均为 5 时掩盖错误。

Flow／Toolbar 原来只给 Canvas 设置固定 bounds，Host 更新根视口后 Canvas 不随之
调整。现在用普通 MarginBorder 保留工具区，再用 FillLayout 填满剩余空间。
右侧 egui 面板按窗口右边缘定位，宽度与保留空间匹配；没有示例私有的 resize 分发。
原有 Figure mock 控件仅用于旧测试，保留原坐标。

## 自动回归直接消费官方场景

22 项布局库级回归通过。新增证据覆盖：

- Flow 横纵双轴 alignment 矩阵、奇数剩余空间的整数居中、行内与行间不同间距、
  窄画布换行、stretch 和方向切换后的自然尺寸恢复。
- Toolbar 通过真实 egui 点击切换 Start／Center／End、stretch 和方向；
  缩放窗口视口后验证 Shape 收缩及恢复。
- Grid 通过真实 egui 鼠标／键盘修改 widthHint、heightHint、indent、双轴 span／grab；
  通过实际 Add／Clear 控件验证移除选择、禁用编辑器、新节点身份和默认尺寸。

独立 `layout_submission` 回归另验证三种 Canvas 可用空间缩至零时，子节点几何仍为
有限、非负值，恢复视口后全部 geometry 与初始值一致。使用 retained-partial
capability 的连续提交验证旧／新 Shape 中心均被 damage 覆盖，Grid 移除也覆盖旧位置。

## Native GPU 连续提交擦除了旧像素

`native_gpu_layout_changes_erase_old_pixels_and_restore_the_same_image` 在本机 Native
GPU 上显式执行并通过。它从真实 `panel_entries` 获取场景，不复制测试专用图形树。
每个 Runtime 连续使用同一 VelloTextureRenderer 与目标纹理，并确认每帧已接受：

- Flow／Toolbar 的红色 Shape 改为纵向排列后，原中心像素为纯白；
  恢复横向后，1200 × 800 RGBA 图像与初始图逐字节相同。
- Grid 移除全部 Shape 后，原中心像素为纯白，Canvas surface 原点仍为 (390, 20)。

Vello texture backend 使用 full-frame damage。本证据证明该后端的连续提交与擦除，
不能替代 retained-partial 后端的像素验证。CPU damage 回归只检查位置覆盖。
Native 窗口 resize 的连续截图、任意文字消费者的 minimum，以及其他平台像素仍待验收；
三项入口继续为 `in_progress`，catalog 仍为 16／25 complete。

图像保存在
[本批证据目录](../../../verification/evidence/k02-layout-consumers-2026-10-23/README.md)。

## 最终门禁

`cargo fmt --all -- --check`、`cargo check --workspace`、
`cargo clippy --workspace -- -D warnings` 和 `cargo test --workspace` 通过。
完整测试为 1,409 passed、5 ignored、0 failed；本批 GPU 测试已单独显式执行通过。
新增 integration target 的严格 Clippy 也通过。
`cargo xtask docs` 通过，检查 364 份 Markdown；`git diff --check` 通过。
Flow／Toolbar 的 Native 窗口截图已检查，右侧面板与 Canvas 分开。
两份新增中文记录已重新审读，半角标点、元话语、翻案句式和连续破折号扫描均为 0。
