# ADR-057 修订 3 设计复核

类型：`architecture-review`

日期：2026-10-30

能力归属：Core Observations、Inspector 与宿主恢复编排；
Animation、Connections 和 Editor 仍按 ADR 的四个切片分别实施与验收。

状态：本轮设计复核通过，建议接受 ADR-057 修订 3。
这表示所审设计缺口已解决，不表示用户已经批准、Rust 已实现或 S09 已进入 Execute。
执行状态仍以 [P2 backlog](../../roadmap/p2-delta-backlog.md)为准。

## 基线与范围

对象为 [ADR-057 修订 3](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)，
文本 SHA-256 为
`29a0f0084683fa999fa1a279953cef86219fe43884003c43941727c3db09d5b6`。

本轮对照 [修订 2 复核](adr-057-revision-2-review-2026-10-30.md)、
[修订 3 回应](adr-057-revision-3-response-2026-10-30.md)和
[S09 验证矩阵](../plans/p2-s09-service-failure.md)，
复核 checkpoint、无 GPU 通知结清、发布游标和首 fault 来源。
同时检查这些规则与既有通知、稳定发布、订阅及 Behavior 顺序合同的关系。
未重新扫描 Rust 实现，未运行新增行为测试或性能测量。

## 上轮意见复核

| 原意见 | 修订 3 的解决方式 | 结论 |
|---|---|---|
| S09-R2-01：checkpoint 与 backend 完成分层 | 在途 submission 和仅有 damage 不单独导致 NotStable；延迟回执按正常 completion 协议在新边界后处理；明确扩展现有稳定化驱动，在无 surface／submission／完成回执时结清合法通知 | 设计缺口已解决 |
| S09-R2-02：已发布游标的推进点与持久性 | 每个 Runtime 持有完整分发前缀；当前 record 处理完成后推进，零 listener／无匹配／self-removal 仍推进；队列为空不重置；分发中拒绝查询，panic 后保留既有前缀并返回 Faulted | 设计缺口已解决 |
| S09-R2-03：首 fault 的操作与来源 | 首次成功完整发布的候选胜出；操作与来源来自同一个值；读取／清空胜出时来源为空，后续 observer 不补填；原无条件 namespace 表述已修正 | 设计缺口已解决 |

SF-03／04／18／20 已同步胜出值和竞争发布规则。
SF-20 已覆盖 headless、Idle／Suspended、延迟回执、backend 失败、游标推进及失败清理，
并将无 GPU 分发列为未来公开消费者的验证要求。
这些是退出条件，当前没有相应的新行为通过结果。

## 通过依据与实施核对

恢复顺序现在可以完整复述：宿主暂停新驱动并串行编排 Runtime，
清理旧 token，经 CPU 稳定化与正常通知分发取得 checkpoint，
建立新订阅、捕获快照并核对同一 checkpoint，接受基线后恢复驱动和正常回执处理。
任何建立或校验失败均清理本次新 token，不伪造成功恢复。

查询与驱动已经分开：checkpoint 不产生副作用；稳定化驱动负责必需工作和通知结清，
其新增行为明确属于 S09-B，且不录制像素、不清除未绘制 damage、不消费在途帧身份。
首 fault、Runtime fault、注册拒绝和恢复失败也没有合并成同一错误语义。
本轮未发现新的设计阻断项。

实施时继续核对以下已有合同，不作为新增设计阻断意见：

- 新增通知分发应保留
  [Behavior 触发合同](../../design/animation/behavior-trigger-contract.md)的消费顺序：
  Behavior 先处理未消费 committed facts，再向普通 listener 分发；
  后续帧或重试不得重复消费这些事实。修订 3 回应说明现有稳定化入口委托 Behavior processing，
  实施验证仍须覆盖新分发路径与该委托的组合。
- 持久发布游标会在正常 record 完成时更新。S09-B 的成本记录应包含该状态更新，
  不能把整个成功路径的增量概括为只有错误分支检查。
- 正文使用 `Frames::stabilize_for_query()`，回应使用 `RuntimeFrames::stabilize_for_query`。
  公共 API 核对时统一名称，或直接使用 `runtime.frames().stabilize_for_query()` 的调用形状；
  不因此新增同语义入口。

## 状态与交接

建议接受修订 3 的设计；接受与实施授权仍由用户决定。
首个实施片保持 S09-A Core Animation，不能因本轮复核通过同时启动其他切片。
S09-B 新增稳定化分发和 checkpoint 必须按公开消费者、私有故障注入与成本证据分别验收。
原审阅记录和回应保留其基线，不改写成实现完成记录。

本记录已按技术文档清晰度与好好说话复核事实、逻辑和中文。
文档门禁仅检查类型、链接和格式，不替代行为、性能或用户接受。
