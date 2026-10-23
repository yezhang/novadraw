# K07-2 共享组结构准入

类型：`verification-record`

日期：2026-10-23

状态：`complete`；仅完成 K07-2 共享组结构准入。

范围：[ADR-044](../../adr/adr-044-shared-scene-capture-and-view.md)的共享组切片。
本记录不证明目标快照安装、资源导入、SceneView 或官方 Thumbnail 已完成。

## 提交前核验结构依赖

`SceneBindingGroup` 只保存参与者身份、父关系、捕获根和视图来源，不持有可变
Runtime 或 Figure。composition root 创建一个实例，每个 Runtime 在登记 source
前加入。默认和外部视图都通过 typed dependency policy 声明来源。
捕获根随 source 身份固定；换根通过终止旧源、登记新身份完成。

Core 根据捕获根的完整子树生成依赖，不因 rect、hidden、零尺寸、clip 或 Pending
删除边。准备候选时固定 group revision，提交保护中再次检查当前版本和整个候选图。
直接或间接环返回 `CyclicSceneViewBinding`；交错提交使旧候选返回
`StaleSceneAdmission`，本地结构和资源写入均不执行。

普通重挂载、Layer 重挂载、回调延迟操作和 typed component/capability Effect
进入相同准入。组件批次先预检所有 Effect，再提交结构与资源；扩展组件的 `commit`、
生命周期回调和用户对象析构在保护之外执行。扩展代码 panic 仍使 Runtime faulted。
当前 LayoutOutput 只输出几何，不改变结构成员；未来结构 Effect 必须复用提交器。

整棵子树替换一次删除旧结构并发布新结构，旧节点交回 Runtime 清理 side state，
然后执行生命周期。批量插入同样只提升一次 graph revision，并在回调前登记所有
节点的资源引用。循环、过期或耗尽不留下已安装前缀。

快照记录历史 graph revision、producer 成员版本和完整直接来源集合。
结构证据不参与 drawing 内容相等性，也不充当当前图的准入凭证。
捕获驱动只检查 producer 成员版本，不因其他 Runtime 的无关修改而重新捕获，
不在驱动检查中扫描 Figure 树。

## 可执行证据

`cargo test -p novadraw --test p2_scene_group --test p2_subtree`：9 项共享组反例、
28 项子树回归通过，日志为 `target/verification/k07/group-subtree-final.log`。
共享组反例通过公开 API 验证：

- hidden、零尺寸、rect 外及尚未安装 drawing 的 view 已登记结构边；
- 直接重挂载与组件 Effect 在循环拒绝后保留旧 parent、revision 和组件值；
- 回调延迟重挂载走 Runtime 准入，错误交出一次，不形成持续重试；
- 三个 producer 的间接环、外部 typed policy、跨组来源和独立解绑；
- source Runtime 释放后无法重新绑定其旧身份；
- 子树替换只提升一次图版本，同时交出旧 source 的 `Closed`；
- 结构证据刷新不改变稳定 epoch 和 drawing 身份，无关成员不触发再次捕获。

`cargo test -p novadraw --lib scene::group::tests`：5 项通过，日志为
`target/verification/k07/group-internal-tests.log`。包括两个交错候选的提交点版本检查、
嵌套捕获根、版本耗尽，以及最后一个可用版本上的完整替换与双节点批量插入。

受影响的完整库级回归：400 项通过；组件更新 8 项、Capability 10 项、capture
13 项通过，日志为 `target/verification/k07/group-regression.log`。
随后新增的两个耗尽批次反例已通过前述定向库级检查。
既有库级测试仍有 dead-code 和 unused-must-use warnings，本记录不声称这些测试
代码全量 Clippy 无警告。

`cargo clippy -p novadraw --lib --test p2_scene_group -- -D warnings`：通过。
日志为 `target/verification/k07/group-clippy.log`。

`scripts/check_architecture.sh --update-api`：Native/Wasm 公开表面、架构边界通过，
Runtime 保持 28/28，Kurbo 0.13.1；日志为
`target/verification/k07/group-api-surface.log`。
新增能力均位于 `scene` 领域和现有 facade，没有新增 Runtime 组合入口。

`cargo xtask docs`：356 个 Markdown 文件与当前引用通过。
`git diff --check`：通过。
本切片使用定向验证；workspace 完整门禁留在 K07 最终交付边界运行。

中文记录的半角标点与破折号扫描：0 项；技术标识符保留原词。

## 后续边界

K07-3 仍需在目标安装时核验当前源结构、binding 和配置，验证 stale/conflict、
多版本 font/image、目标测量、完整 damage 及 reset/Retry lease。
K07-4 才提供真实共享源的 Thumbnail Native 图像。
K07 整体和官方示例状态继续保留未完成。
