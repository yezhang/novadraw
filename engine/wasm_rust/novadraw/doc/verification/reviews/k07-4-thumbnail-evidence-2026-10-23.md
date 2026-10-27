# K07-4 官方 Thumbnail 消费者

类型：`verification-record`

日期：2026-10-23

状态：`complete`；范围为官方 Canvas 内容及 macOS Native 消费。

## 一棵源树驱动两个视图

`thumbnail.rs` 的宿主持有独立 source Runtime，源树包含官方尺寸和颜色的两个可拖动
Rectangle。目标 Runtime 只有主视图和 overview，没有复制源 Figure。
两处通过同一订阅安装完整快照，overview 使用普通 SceneViewFigure 和 GroupBoxBorder。
主视图通过外部 Figure descriptor 接入同一 preparation 和安装路径。

主视图采用固定的 600 × 600 逻辑区域，和源捕获域一比一对应。
Core 将鼠标事件转换为目标局部坐标后，Figure 只把值事件交给宿主；
宿主调用 source input facade，源 Rectangle 经既有事件和 mutation 路径完成拖拽。
Figure 不持有 source Runtime，也不跨 Runtime 调用源 paint。
宿主在每次帧准备前处理输入、检查 needs_capture，再接管有界 handoff。
没有私有计时器、后台扫描或共享可变 Runtime。

overview resize 只修改目标 Figure bounds，Border insets 和等比显示由既有 Core
preparation 处理，不触发 source capture。取消订阅产生 Closed，宿主接管后关闭两个
binding；旧图像清空，overview 自身边框保留。单 Runtime 工厂返回活源树；
Native panel entry 才是持有源和目标的完整共享消费者。

与上游两个 Shell 的区别是宿主将两处 Canvas 内容放进一个窗口。
Shell 和 SWT 控件不属于本项目 Canvas 对等范围。
不声称完成 Web、Windows 或 Linux 平台验收。

## 自动和图像证据

消费者测试通过真实目标输入检查一次拖动后的源 bounds、两处快照相等、
源 epoch 前进、overview resize 后源无需捕获，以及取消后的 binding 清理。
多目标独立绑定、目标暂停、迟到投递、Retry、reset 和资源回收由
[K07-3 外部契约](k07-3-scene-view-evidence-2026-10-23.md)覆盖。

本切片执行：

```sh
cargo test -p draw2d-official-examples --lib
cargo clippy -p draw2d-official-examples --lib -- -D warnings
NOVADRAW_THUMBNAIL_EVIDENCE=1 cargo run -p draw2d-official-examples -- --screenshot-all
```

68 项库级测试通过；更新目录状态后的精确目录测试也通过，
日志为 `target/verification/k07-4-catalog-final.log`。
测试与 Clippy 日志分别为 `target/verification/k07-4-tests-final.log`、
`target/verification/k07-4-clippy.log`。Native 日志为
`target/verification/k07-4-native.log`。三张 Vello 截图已逐张查看：

- [初始](../evidence/k07-thumbnail/draw2d-official-examples_ThumbnailInitial_1792739663.png)：两处显示同一组图形和源边框。
- [拖动与宽扁 overview](../evidence/k07-thumbnail/draw2d-official-examples_ThumbnailDraggedWide_1792739665.png)：两处绿色矩形同步移动，overview 保持正方形捕获域比例。
- [关闭](../evidence/k07-thumbnail/draw2d-official-examples_ThumbnailClosed_1792739667.png)：两处图像清空，保留目标边框。

截图阶段使用同一 Native renderer，阶段工厂分别重建消费者输入；
连续旧帧、暂停和终止语义由前述契约测试验证。
workspace 最终门禁在 K07-5/6 完成后执行。

目录更新为 15 个 complete、8 个 in_progress、2 个 blocked。
K07-5/6 和宿主背景整体仍未完成。

中文成稿已按好好说话复核；机械扫描的半角标点、破折号和空判断均为 0 项。
