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

K07-5 的公开领域为 `host`。`HostBackgroundRegistry` 由宿主拥有，使用
`HostBackgroundProviderKey<P>` 和 `HostBackgroundProvider<P>` 注册提供者；
默认 `RECORDED_BACKGROUND` 也走此路径。Runtime 不保存 registry 或调用 provider。
`RuntimeFrames::{begin_background,complete_background,remove_background}` 管理受检交接；
`set_background_input`、`background_input`、`current_background` 和 `background_error`
提供独立输入与诊断，`set_background_waker` 唤醒宿主。

`BackgroundFrameInput` 冻结 surface、位置与 DPI 的独立版本和 `host_to_canvas`。
同来源的新 request 保留上次成功结果；更换来源或移除推进 generation。
同 generation 的候选 revision 由宿主单调递增；同 request/revision 的完整内容
重复可幂等，同版本异内容报错。未知 request、倒序输入及当前坐标失配分别返回
`StaleRequest`、`StaleInput` 和 `InputMismatch`。背景闭包复用 scene 导入表，
候选失败不改变当前结果、目标资源注册表或导入身份。

帧包包含 `clear_color` 与 `background_input`，背景 drawing 排在 Canvas 命令前。
基础清屏通过后端清屏输入完成，因此透明值能擦除旧像素。
背景参与时完整录制 Canvas 并使用 Full damage；首批不承诺局部背景修复。
后端必须声明 `RenderCapability::HostBackground` 并在资源接收前调用
`RenderSubmission::validate_composition`。Vello surface、截图与离屏目标消费相同输入。
Core 外部契约见 [`p2_host_background`](../../novadraw/tests/p2_host_background.rs)；
GPU 像素反例见 [`host_background_pixels`](../../novadraw-backend-vello/tests/host_background_pixels.rs)。
官方消费者已通过 [K07-6](../verification/reviews/k07-6-inherit-background-evidence-2026-10-23.md)：
宿主自定义 typed provider、双 Canvas Runtime、内容滚动与 Canvas 移动、
背景独立变化/移除、DPI 修复、暂停恢复和 backend session reset 均有 Native 像素证据。
此结论限于 macOS Native，其他平台背景像素仍待验收。

`api_semantics`：`frame.preparation`、`graphics.context`、`resource.lifecycle`、
`render.backend_session`、`coordinate.conversion`、`figure.properties`、`damage.repaint`。

此决定补充 ADR-031 的统一输入冻结及资源协议的背景 lease/恢复责任，
不增加第二个帧提交出口，不修改 ADR-030 的平台加载边界。
资源闭包和多版本导入复用 [ADR-044](adr-044-shared-scene-capture-and-view.md)。
完整运行合同见 [已批准设计](../design/architecture/shared-scene-view-background-proposal.md)；
执行切片只在 [P2 backlog](../roadmap/p2-delta-backlog.md)维护。
