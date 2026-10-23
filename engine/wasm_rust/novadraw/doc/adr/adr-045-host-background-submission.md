# ADR-045：宿主背景统一帧提交

类型：`architecture-decision`

状态：`accepted`；用户于 2026-10-23 批准实施，按共享场景切片之后推进。

## 背景与依据

InheritBackground 需要透明 Canvas 透出宿主背景，背景位置不随内容 viewport 滚动。
该能力已随 [修订 3](../design/architecture/shared-scene-view-background-proposal.md)
通过设计复核，用户明确批准实施。它与 Thumbnail 分别决策和验收。

## 决策

Host 提供不可变候选背景、来源/request identity、版本、宿主到 Canvas 的变换、
覆盖区域与资源闭包。平台 I/O 和调度由 Host 负责；Core 接收受检结果，
不持有 ResourceProvider 或平台 future。

destination Runtime 是唯一提交权威。帧准备统一冻结背景、Canvas、资源 lease
和 surface/位置/DPI 版本；后台或宿主不能在 Runtime 准备后追加未经检查的背景。
backend 消费同一个受检 RenderSubmission，合成顺序为背景、Canvas、现有 overlay。
opaque root 遮住背景；透明 root 保留 alpha。

背景变化和移除独立产生 pending work 与唤醒。过期完成被拒绝，移除有明确清屏值；
版本不匹配时不提交混合帧。Retry、Failed 和 device reset 保留当前受检背景及
所需 CPU lease，并遵循原有资源同步、重试前缀和单 in-flight 规则。
宿主坐标变换不叠加 Canvas viewport origin。

## 外部消费者证据

背景适配能力使用与默认实现相同的 typed identity、payload trait 和 registry。
独立消费者验证只更新背景、移除和迟到完成、Canvas 移动与内容滚动、
opaque/transparent root、DPI、repair、Retry 和 device reset。
不支持的 backend 在提交前明确拒绝，不能将未验证平台记为通过。

## 关系与实施

`api_semantics`：`frame.preparation`、`graphics.context`、`resource.lifecycle`、
`render.backend_session`、`coordinate.conversion`、`figure.properties`、`damage.repaint`。

此决定补充 ADR-031 的统一输入冻结及资源协议的背景 lease/恢复责任，
不增加第二个帧提交出口，不修改 ADR-030 的平台加载边界。
资源闭包和多版本导入复用 [ADR-044](adr-044-shared-scene-capture-and-view.md)。
完整运行合同见 [已批准设计](../design/architecture/shared-scene-view-background-proposal.md)；
执行切片只在 [P2 backlog](../roadmap/p2-delta-backlog.md)维护。
