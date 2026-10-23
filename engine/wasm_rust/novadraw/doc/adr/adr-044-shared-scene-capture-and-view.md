# ADR-044：共享场景捕获与只读视图

类型：`architecture-decision`

状态：`accepted`；用户于 2026-10-23 批准实施，实现进行中。

## 背景与依据

Thumbnail 要求两个 Runtime 观察同一可变 Figure 树，不能复制场景或跨 Runtime
挂载活 Figure。设计经过三轮修订，
[修订 3 复核通过](../verification/reviews/k07-shared-scene-view-background-revision-3-review-2026-10-23.md)，
用户随后明确批准实施。完整契约由
[共享视图与宿主背景](../design/architecture/shared-scene-view-background-proposal.md)负责。

## 决策

source Runtime 独占可变树。通过独立 scene facade 捕获已稳定的 source-local 子树，
返回 owned、不可变、受检的 SceneSnapshot 与资源闭包。捕获入口解析继承样式一次，
包含 alpha；子树递推使用最近显式值，不沿父链相乘。

公开组合入口为 `Runtime::scenes()`，业务方法位于 `scene::RuntimeScenes`。
ADR-033 的组合根预算仅为此入口从 27 调整至 28；不开放 Runtime 本体的独立捕获、
订阅或 view 安装方法。

capture 复用稳定化内核，不推进 source session/frame、不消费 damage、资源 pending
前缀或确认 cursor。单订阅失败隔离，source 本体 fault 保持现行边界。
此决定在 ADR-031 中增加受检 SceneSnapshot 的独立出口例外，不能导出内部 recorder
或未受检命令容器，也不新增 surface 帧提交入口。

source、subscription、capture config revision、stable epoch 和 destination binding
分别管理。普通更新有界合并，关闭通过独立终止交接；迟到、过期和同版本异内容
分别拒绝。目标安装修复完整旧/新 view，目标 resize 不伪造源版本。

destination 按来源资源身份与 revision 导入：同 revision 去重，不同 revision 使用
不同本地资源 ID，保留 CPU、快照、binding 和在途 lease。源帧呈现不构成目标更新前提。

循环准入统计整棵捕获子树的结构依赖，不根据 rect、hidden、clip 或 Ready 状态过滤。
Runtime 的直接、deferred、typed update 和关系 derived commit 共用图准入；
在同一 group 提交保护下校验 graph revision 和本地 generations。
循环或过期候选在写入前拒绝，不回滚此前已提交的源输入。

实现中，捕获根随 source 身份固定；换根通过关闭旧 source、登记新身份完成。
参与者先加入 composition-root 创建的 `SceneBindingGroup`，再登记 source。
外部视图通过 typed dependency policy 注册来源，所有已绑定视图都参与结构统计。
私有候选只使用一次，提交时再次检查同一共享组的 revision；纯几何不提升该版本。
子树替换和批量插入一次提交完整结构，用户回调随后执行。
快照冻结的 `SceneDependencyEvidence` 与内容相等性分开，不作为当前图准入凭证。

## 外部消费者证据

默认与外部 capture/view policy、Figure binding 共用 typed identity、payload trait
和 registry。独立消费者必须验证同一源的多个目标、旧新资源共存、布局/可见性
不改变结构边、callback reparent、过期候选、取消与释放。Native Thumbnail
提供真实拖拽和 overview resize 证据；测试及截图通过前不关闭 K07。

K07-1 的外部捕获消费者见 `p2_scene_capture`；K07-2 的外部依赖策略和直接、
deferred、typed reparent 反例见 `p2_scene_group`，内部候选交错与版本耗尽见
`scene::group::tests`。目标端安装、资源共存和 Native Thumbnail 仍未验收。

## 关系与实施

`api_semantics`：`paint.protocol`、`figure.properties`、`figure.tree`、
`figure.lifecycle`、`coordinate.conversion`、`layout.manager`、`runtime.identity`、
`notification.layout_update`、`resource.lifecycle`、`frame.preparation`。
账本按实际切片补证据。
ADR-014 的原子性分层、ADR-023 的包边界及 ADR-030 的宿主资源加载责任保持有效。
宿主背景由 [ADR-045](adr-045-host-background-submission.md)单独定义和验收。
唯一执行队列为 [P2 backlog](../roadmap/p2-delta-backlog.md)。
