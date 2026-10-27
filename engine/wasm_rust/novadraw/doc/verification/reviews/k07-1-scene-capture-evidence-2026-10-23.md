# K07-1 场景捕获与订阅交接

类型：`verification-record`

日期：2026-10-23

状态：`complete`；仅完成 K07-1 producer 切片。

范围：[ADR-044](../../adr/adr-044-shared-scene-capture-and-view.md)的 producer 切片。
目标端循环准入、资源导入、SceneView、Thumbnail 与宿主背景仍待后续切片验收。

## 实现结果

`Runtime::scenes()` 提供 source 登记、typed capture policy 登记、订阅配置、
捕获驱动、取消和 source 释放。默认选择与外部选择共用同一 registry 和校验路径。
身份使用 producer namespace 与不复用的单调序号，配置版本独立递增。

捕获复用稳定化内核和递归 Figure 绘制，冻结入口继承样式，抵消 source 外部位置，
固定捕获 clip 和 transform 原点。结果只包含受检 drawing 与实际引用的 image/font
不可变 payload，不消费源帧状态或资源同步前缀。输入 serial 与配置版本分别比较，
同一稳定输入只改配置时不递增 source epoch；counter 耗尽不回绕。

每个订阅的 owned handoff 只保留一个最新结果、一个最近错误和一个终止事件。
取消、source Figure 释放和 Runtime 释放均可交出 `Closed`，不依赖后续 source 帧。
失败尝试不自动重复唤醒，Host 通过新输入、配置变化或显式 capture 重试。

## 验证证据

`cargo test -p novadraw --test p2_scene_capture`：13 项通过。
该 integration target 作为外部 Rust 消费者，仅使用公开领域 API：

- 非零原点、source 外部 placement、祖先 clip 与暂停 surface；
- alpha 0.5 继承、显式 0.8 覆盖、入口字体冻结和合法零尺寸；
- 外部 policy 注册、同稳定 epoch 新配置、最新结果覆盖未消费旧结果；
- source 与 subscription 独立身份、跨 producer 拒绝、退役身份不复用；
- 单订阅配置/录制错误隔离，主窗口自身仍拒绝绘制错误；
- source 帧在途时捕获新 image revision，保留原帧确认和后续更新；
- 精确 font 闭包排除未引用 image，payload 在 producer 释放后仍保留；
- 初始/结果/终止唤醒、独立取消和释放后读取终止事件。
- source 主窗口的帧阶段通知不触发再次捕获。

`cargo clippy -p novadraw --lib -- -D warnings`：通过。

`cargo clippy -p novadraw --test p2_scene_capture -- -D warnings`：通过，包含帧通知反例。

`cargo test -p novadraw --lib`：397 项通过，包括身份/配置耗尽反例和最大树深度释放。
日志为 `target/verification/k07/core-lib.log`。既有测试代码包含 dead-code 与
unused-must-use warnings；本报告不声称库级测试代码全量 Clippy 无警告。

通知队列定向复核：2 项通过。生命周期 panic 定向复核：1 项通过；
source 结构释放后先发布终止，之后的第三方 detach callback panic 不会丢失 `Closed`。

`tools/scripts/check_architecture.sh --update-api`：通过 Native/Wasm 公开表面、
依赖与热路径检查，Kurbo 0.13.1，Runtime 28/28。ADR-033 的预算仅按 ADR-044
为 `scenes()` 组合入口增加一项，业务方法仍位于短借用 facade。
日志为 `target/verification/k07/api-surface.log`。

`cargo xtask docs`：355 个 Markdown 文件与当前引用通过。
`git diff --check`：通过。

## 证据边界

本报告不证明目标端 stale/conflict 安装、结构循环拒绝、多版本导入与 GPU lease 回收；
不证明默认 view 测量、Native Thumbnail 或宿主背景合成。
外部 capture policy 已通过；独立产品消费者的 capture/view/background 组合证据由
后续切片提供。K07 整体与两个官方入口继续保留未完成状态。
