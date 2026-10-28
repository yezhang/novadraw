# Tree 修订 2 复核

类型：`architecture-review`

日期：2026-10-24

状态：暂不通过；F01、F02 需要收口后再复核。未批准实施。

对象：[Tree 表现几何修订 2](../../design/animation/tree-layout-presentation-proposal.md)。
前次判断与实现证据保留在 [首轮评审](tree-layout-presentation-review-2026-10-24.md)。
本记录只评估本次候选契约，不表示实现或平台验收完成。

## 原评审已响应，仍有两个契约边界

修订 2 已补充真实 bounds、收拢终点、完整绘制读取点、语义 channel 身份、
provider 依赖图、可见性传播、viewport 接管和退出证据。没有发现需要推翻这些方向
的新问题。剩余问题集中在时间快照与预算失败归属，影响 retarget 和活动计划稳定性。

| 首轮意见 | 本次核对 |
|---|---|
| R01 | 已定义独立收拢终点、中心转换和一次性包围域构造；具体几何正确性留给 T-1/T-3 验证 |
| R02 | 已覆盖完整绘制读取点、组合顺序与裁剪策略 |
| R03 | 已定义 Motion/派生角色、依赖图和普通失败；采样与发布时序仍见 F01 |
| R04 | 已补目标/依赖集合、兄弟影响与版本表；捕获时刻仍见 F01 |
| R05 | 已区分合法保留、外部隐藏与 surface suspend |
| R06 | 已定义 Oa/Xa、双轴 K、直接输入接管和旧 transform owner 冲突；锚点版本仍见 F01 |
| R07 | 已补旧帧 envelope、去重工作量和恢复；预算失败归属仍见 F02 |
| R08 | 已声明共用线条算法、显式 Viewport 与默认关闭的面板 |
| R09 | 已明确语义类型身份、旧 affine API 保留与文字冻结 |
| R10 | 已补语义 family 和 T-1–T-4 证据范围 |

上述“已定义”表示设计响应已纳入，不表示已通过测试或实现问题关闭。

## F01：当前采样值与已发布快照必须指向同一明确时刻

优先级：P1。涉及 R03、R04、R06。

**位置。** “捕获范围、版本与原子替换”规定 capture 保存“最后一次已发布”的表现
几何；“采样阶段、派生依赖与错误传播”先描述帧准备采样基础值，又规定帧准备不
再次推进 Motion；“Viewport 跟踪与布局共用一个进度”使用当前 sampled origin
和最后发布的 origin 作为 retarget/输入锚点。

**问题。** “已发布”没有定义为 Runtime 采样完成、快照冻结、submission 准备还是
backend 已呈现。这些时刻可能不同。现有
[advance_time](../../../crates/novadraw/src/animation/service.rs#L400-L446)更新 channel 采样值，
而帧准备在 source 稳定后才构造快照；已有
[公开时间合同](../../design/animation/public-api-contract.md)要求 query 不采样新值、
retarget 使用当前 presentation，Retry 不回退时间。

例如先录制 t=0 的快照，再成功 advance 到 t=50，但下一次帧准备尚未发生。此时
retarget 或滚动接管，究竟取 t=0 的完整几何，还是 t=50 的 channel 值？如果 bounds、
marked position 和 origin 分属两个时刻，就不能保持一个一致的跟踪锚点。
AwaitingCompletion、Skipped/Retry 和首次尚无已发布快照时也需要明确取值规则。

**建议。** 明确 Runtime 采样 epoch、冻结快照和 backend 呈现的关系，以及
advance_time、稳定化、capture、帧准备各自承担哪一步。建议以最后一次成功 Runtime
采样的统一 epoch 为当前表现；capture 可以复用缓存的 Motion 结果，在同一时间和
合法源版本下取得完整几何，但不推进时钟或隐式采样一个新时刻。bounds、visibility、
world/clip、marked position 与 origin 必须整体取自同一 epoch。

若选择以已录制或已呈现帧为起点，则需明确这是一项不同的 retarget/输入策略，并
处理它与已接受“当前 presentation”合同的关系。不能让具体实现自行解释“发布”。

**退出证据。** advance 后尚未 prepare 即 retarget/滚动；连续 advance；同时间源
变化；首次 Scheduled；AwaitingCompletion；Skipped/Retry。断言所有锚点的 epoch
一致、query 不推进时间、接管不混用旧 geometry 与新 origin。

## F02：共享预算不足与单个计划越限必须分开

优先级：P1。涉及 R03、R07。

**位置。** “Damage、遍历预算与帧恢复”定义 capture、active group 和 frame work
预算，并规定逐帧预算失败终止“超限关联组”，同时要求无关组不被驱逐。

**问题。** 当前没有说明共享 frame budget 如何预留，以及如何判定哪个组越限。
若两个合法计划共享帧预算，后执行的计划可能因前一计划已消耗预算而失败。
仅凭失败发生在某个 provider，就终止其关联组，会使存活结果依赖执行顺序；
该组未必超过自身准入上限。

[ADR-026 失败边界](../../adr/adr-026-animation-and-presentation-plane.md)规定预算耗尽
只拒绝新 plan，不破坏无关 active plan；公开 API 合同同样限制预算驱逐。
候选开头又明确不覆盖已接受 ADR，因此不能将所有逐帧 BudgetExceeded 都解释为
终止活动组。

**建议。** 分开定义：

- admission/capture 的容量不足：拒绝新工作，retarget 保留有效旧计划；
- 已准入计划的节点、依赖、线段和 drawing 上限：在 admission 预留或证明容量；
- provider 超出自己声明的工作/输出上限：归因到该计划的受检失败，清除关联组；
- 共享帧预算不足或配置改变：明确保持活动计划合同的处理方式，不能按执行顺序
  驱逐仍在自身声明上限内的组。

若要采用不同的运行期驱逐策略，应明确提出 ADR-026/公开失败合同的修订范围，
经评审接受后实施。静态修复预算仍须与失败动画路径隔离。

**退出证据。** 两个 active group 的执行顺序交换、预算恰满、新计划被拒、
provider 超出自身声明上限、retarget 净容量替换和静态 Full 修复。证明失败主体
归属明确，合法无关组的 owner、采样值与 wakeup 保留。

## 复核范围与限制

基线提交：`3b6e43d30040f72c0a165c006e3690b8a1ce89c8`。
对象为本次工作树中的修订 2；仅核对方案、既有动画时间/失败合同及相关 Rust 入口。
本轮没有修改设计正文、ADR 或 Rust，也未重跑运行测试。
首轮 15 项过渡测试和 29 项动画基础测试的结果仍保留原日期与限制，不用来证明
本次新增契约已成立。

记录整理检查：中文半角标点、破折号、元话语、机械章节编号及对比句式均为 0 处，
无豁免项；引用路径为 0 个错误。`cargo xtask docs` 通过，检查 371 个 Markdown
文件及当前 xtask 引用；`git diff --check` 通过。复核意见保持建议模态，未改变
首轮事实或扩大实施范围。
