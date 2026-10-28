# K07-3 目标场景视图

类型：`verification-record`

日期：2026-10-23

状态：`complete`；完成 K07-3 目标端契约，官方消费者另行验收。

范围：[ADR-044](../../adr/adr-044-shared-scene-capture-and-view.md)的目标导入、视图、
测量和资源保留。本记录不证明官方 Thumbnail Native 消费或宿主背景已完成。

## 在目标提交点安装完整结果

目标先建立只读 `SceneViewHandle`，将它移入默认或外部 Figure，使用 `SCENE_VIEW`
typed descriptor 绑定。展示策略通过 `SceneViewPolicyKey<P>` 注册，默认与外部
Figure 共用配置、测量、安装及失败通道，句柄没有公开写入或跨 Figure 克隆入口。

安装检查当前 binding、source/subscription/config 及 epoch。
同完整身份、同内容的投递幂等；同版本异内容返回 `SnapshotConflict`，
旧身份或倒序 epoch 返回 `StaleSceneDelivery`。同源重绑保留 epoch 比较基线。
闭包校验、资源映射和命令重写在私有候选中完成。
提交点重新检查当前 source 成员版本与结构边，在共享组保护中一起替换 Registry、
导入表和视图结果；失败不部分安装。旧值和用户配置析构在保护外执行。

来源资源键包含 producer namespace、resource ID、source revision 和 kind。
同键去重，不同 revision 使用不同目标 ID，每个 ID 的 Ready payload 不可变。
`InstalledScene`、准备缓存、保留结果、在途帧及 reset 恢复前缀保留 CPU lease。
公开资源修改返回 `ImmutableImport`；最后 lease 释放后，下一次成功帧准备登记 Remove。
reset 先接回 CPU lease，再释放旧 token，新 session Snapshot 保留仍需引用的 ID。

目标展示采用普通 Figure preparation 与 Graphics 回放，保留目标 transform/clip。
目标 opacity 与已冻结的源 opacity 合成，源快照内容保持不变。
默认策略保持源比例，Core 统一处理 Border insets。
目标配置、resize、重绑和终止请求完整重绘，覆盖被合并更新的旧像素。
源字体替换会改变捕获稳定输入中的 TextEngine revision，重新生成 glyph 与闭包。

## 已执行的检查

`cargo test -p novadraw --lib --test p2_scene_view --test p2_scene_capture --test p2_scene_group`：
407 项库级、13 项捕获、9 项共享组及当时的 7 项目标视图测试通过。
日志为 `target/verification/k07-3-contracts.log`。
随后增加 Border、Retry、同源重绑和目标 opacity 反例：
`cargo test -p novadraw --test p2_scene_view` 最终 11 项通过，
日志为 `target/verification/k07-3-view-final.log`。
`cargo test -p novadraw --lib scene::` 最终 13 项通过，
日志为 `target/verification/k07-3-unit-final.log`。

`p2_scene_view` 通过公开 API 检查多目标独立安装、配置切换、终止清理、
旧新 image/font 共存、在途 reset、外部 Figure/策略和失败恢复。
`scene::import::tests` 检查闭包失败、去重、冲突及最后 lease 回收；
`scene::view::tests` 检查尺寸提示、显式 preferred、minimum/零尺寸，
以及同版本异内容和破损闭包不改变当前结果。

`cargo clippy -p novadraw --lib --test p2_scene_view -- -D warnings`：通过，
日志为 `target/verification/k07-3-clippy-final.log`。
`tools/scripts/check_architecture.sh --update-api`：Native/Wasm 公开表面通过，
Runtime 28/28、Kurbo 0.13.1，日志为 `target/verification/k07-3-api.log`。
`cargo xtask docs`：357 个 Markdown 文件和当前引用通过，
日志为 `target/verification/k07-3-docs.log`。`git diff --check`：通过。

库级测试仍保留既有 dead-code 和 unused-must-use warnings；
本记录不声称这些历史测试全量 Clippy 无警告。
workspace 完整门禁留在 K07 最终交付边界执行。

中文记录的半角标点和破折号扫描：0 项；源码标识符保留原词。

## 后续边界

K07-4 提供单一可变 source 的真实 Thumbnail 拖拽、等比 overview resize 和 Native
图像。K07-5/6 独立实现并验收宿主背景统一合成。
K07 整体及两个官方示例在各自消费者验收前保持未完成。
