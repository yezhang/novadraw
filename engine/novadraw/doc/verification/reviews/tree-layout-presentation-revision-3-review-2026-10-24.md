# Tree 修订 3 设计复核通过

类型：`architecture-review`

日期：2026-10-24

状态：设计复核通过，未实施。

对象：[Tree 表现几何修订 3](../../design/animation/tree-layout-presentation-proposal.md)。
前次发现分别保留在 [首轮评审](tree-layout-presentation-review-2026-10-24.md)和
[修订 2 复核](tree-layout-presentation-revision-2-review-2026-10-24.md)。
本文记录当前设计复核结果，不改写历史判断或声明实现验收完成。

## F01、F02 已收口

修订 3 已达到本次设计评审的通过条件。R01–R10 的设计响应保持完整，F01、F02
不再构成实施前的设计阻塞项；可以按方案 T-1 至 T-4 推进。
本轮没有发现需要新增修订批次的设计问题。

| 复核项 | 修订 3 的契约 | 结论 |
|---|---|---|
| F01：当前表现时刻 | S 对应最后成功 Runtime 时间采样；完整表现版本 E 关联 S、active-set revision 与合法源依赖版本 | 通过 |
| F01：几何补齐与查询 | 显式 capture/准备复用缓存 Motion 值物化 E；普通 query 只读缓存，未准备时返回明确结果 | 通过 |
| F01：锚点一致性 | capture、retarget 和输入接管从同一个 E 取得 bounds、visibility、world/clip、marked position 与 origin | 通过 |
| F01：提交与恢复 | submission/session/frame 和呈现确认独立于 S；未录制、Unanchored、在途、Skipped/Retry 与 reset 的处理明确 | 通过 |
| F02：共享活动容量 | 候选声明多维上限 B，激活前预留；运行不按执行顺序借用共享余额，容量下调低于存活责任时拒绝 | 通过 |
| F02：retarget 容量 | 按净替换校验活动容量，另计 staging；成功原子转移，失败保留有效旧组 | 通过 |
| F02：快照留存责任 | 缓存与候选共存峰值预留；capture、in-flight 和恢复前缀按独立生命周期持有容量与资源 lease | 通过 |
| F02：失败主体 | 自身声明上限被突破才归为该组受检失败；共享不可用使准备失败，不驱逐合法活动组 | 通过 |

### 时间快照与既有合同一致

“当前表现由 Runtime 采样时刻确定”明确选择 Runtime 当前采样，未将旧录制或呈现帧
作为 retarget 起点。advance 后尚未 prepare 时，显式 capture 在同一 S 补齐几何；
源或活动集合变化由 E 区分，不混合两个物化版本的几何与锚点。

普通 query 不推进时钟、不物化、不产生 damage。首次 Scheduled 不伪造时间，
AwaitingCompletion 不修改旧提交包，Retry/reset 不回写旧 E。上述规则解决了 F01
关于采样、冻结、录制和呈现时刻混淆的问题，并保留现有时间与恢复边界。

### 预算失败与既有合同一致

“准入容量与单组上限分别记账”用准入预留保证合法组的工作容量，区分活动额度、
临时 staging 和存活快照存储。净活动容量替换不折抵仍存活的旧 capture 或提交包。
同一 E 的复用不重复执行 provider，新 E 使用活动预留完成物化。

自身输出/工作上限突破归为 provider/plan 受检失败；共享不足、瞬时分配不可用和
记账不一致不能按最后执行的 provider 驱逐合法组。配置下调不足时原子拒绝更新，
静态修复与失败动画路径隔离。F02 的归属问题已解决，无需改变 ADR-026 的
“共享容量不足拒绝新工作、无关合法活动组保留”原则。

## 实施按既定切片验证

本次通过范围为方案的所有权、时间、几何、依赖、失败和预算契约。具体 Rust 签名、
收拢几何算法、缓存/预留实现及 Native 像素仍按 T-1 至 T-4 验证，不以设计通过代替证据。

| 切片 | 通过后继续执行的退出要求 |
|---|---|
| T-1 | 外部 typed channel/provider；完整几何读取点；S/E 一致性；容量预留、净替换与失败原子性；无动画零新增扫描 |
| T-2 | 静态/动画端点同源；不逐帧 layout、shaping 或写 Lines；字号、描边与资源版本一致 |
| T-3 | 收拢与兄弟重排；显式 Viewport；同 E 锚点、双轴 clamp、反向与直接输入接管 |
| T-4 | 默认关闭的真实面板；Native 中间帧和旧像素擦除；Retry/reset、资源释放与静态等价；工作量测量 |

签名与实现遵守本方案及已有 ADR 边界；若发现破坏契约的新问题，应先回到设计处理，
不通过 Tree 私有路径、第二套渲染主线或隐式 source mutation 绕过。

## 复核范围与验证限制

本轮读取修订 3 全文，并对照修订 2 的 F01/F02 和此前已核对的
[ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)、
[公开 API 合同](../../design/animation/public-api-contract.md)及
[坐标规范](../../design/coordinates/coordinate-system.md)。
仅更新评审记录、方案评审状态和索引，未修改 Rust 或执行实施工作。

本轮不重跑运行测试。首轮 15 项过渡测试和 29 项基础测试仍属于原快照，不能证明
新增 S/E、预算预留或 Tree 中间帧已实现。前两轮未通过结论保留各自基线。

记录整理检查：中文半角标点、破折号、元话语、机械章节编号及对比句式均为 0 处，
无豁免项；引用路径为 0 个错误。`cargo xtask docs` 通过，检查 372 个 Markdown
文件及当前 xtask 引用；`git diff --check` 通过。事实核对保留设计与实现的区别，
通过结论仅适用于本次修订的设计范围。
