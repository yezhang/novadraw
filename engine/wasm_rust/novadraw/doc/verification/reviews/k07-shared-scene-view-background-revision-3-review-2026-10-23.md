# K07 修订 3 复核通过

类型：`architecture-review`

日期：2026-10-23

状态：架构设计复核通过；功能、平台验收和用户实施批准另行处理。

对象：[共享视图与宿主背景方案](../../design/architecture/shared-scene-view-background-proposal.md)
修订 3。行号对应本轮读取的 400 行方案正文。

依据：[首轮评审](k07-shared-scene-view-background-review-2026-10-23.md)、
[修订 2 复核](k07-shared-scene-view-background-revision-2-review-2026-10-23.md)和
[修订 3 响应](k07-shared-scene-view-background-revision-3-response-2026-10-23.md)。
本报告保留前轮历史判断；新的通过结论只适用于修订 3 的设计契约。

## 修订 3 可以通过设计复核

首轮 K07-R01 至 R08 均已在设计层面回应。修订 2 复核留下的循环准入 P1
和继承 alpha P2 已补齐，本轮未发现阻止方案固化的设计问题。
无需继续扩张本次修订范围；具体 Rust 签名和实现证据按既定实施切片核对。

| 编号 | 设计复核结果 | 修订 3 依据 |
|---|---|---|
| K07-R01 | 通过 | L202–226：source-local 捕获独立于外部 Viewport；入口样式含 alpha，内部变换和 clip 保留 |
| K07-R02 | 通过 | L67–106：显式提出 ADR-031 capture 例外；稳定化复用与订阅失败隔离明确，不推进 source 帧确认状态 |
| K07-R03 | 通过 | L108–128：源、订阅、配置和目标绑定分别管理；完整身份判重，过期与冲突结果拒绝 |
| K07-R04 | 通过 | L146–172：初次主动捕获，有界更新槽与独立终止交接；合并更新完整修复旧/新 view |
| K07-R05 | 通过 | L331–366：destination Runtime 统一冻结背景、场景、资源和 surface 输入，覆盖独立唤醒、移除及恢复 |
| K07-R06 | 通过 | L174–200：不同来源 revision 使用共存的目标资源身份，CPU 与在途 lease 保活并支持 session 重建 |
| K07-R07 | 通过 | L228–254：producer 与 destination 策略分工明确，测量、insets、零尺寸与失效规则完整 |
| K07-R08 | 通过 | L256–329：保守结构依赖、统一 Runtime 准入、提交点版本校验与原子图更新形成完整契约 |

上述“通过”表示设计问题已解决，不表示反例测试、外部消费者或 Native/Web 验收已经执行。

## 循环依赖与提交保证已一致

依赖统计覆盖捕获根的整棵子树，包括根自身；rect 外、hidden、零尺寸、clipped
和尚未 Ready 的 imported view 仍登记结构边。
因此，普通移动、布局、显示或裁剪变化不会新增依赖，前轮反例会在建立反向绑定时拒绝。
可能拒绝不可见结构环的保守限制也已明确接受。

改变捕获根、成员拓扑或 view 来源的候选，由同一个 Runtime 提交器处理。
直接 API、callback Effect、typed update 和携带关系 effect 的 derived output
共用整批预检；纯几何 LayoutOutput 保持现行提交路径，无需应用手工协调图更新。

候选绑定 group 身份、graph revision、本地 generations 与操作 payload。
提交时在同一个 group 的独占保护下重验版本、检查候选图，再安装本地结构和图边。
保护区不执行用户回调或析构，也不保留可恢复失败的外部步骤；
过期或循环候选在写入前拒绝。该保证由 Runtime 强制，避免前轮依赖 Host 自律的缺口。

capture/import 的后置检查只用于拒绝迟到结果，不能替代前置结构准入。
source 与 derived failure 的处理继续遵守
[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)，
保留已经提交的因果前缀，不承诺跨 Runtime 模型事务。

## 继承 alpha 已明确

L207–215 将 resolved alpha 加入捕获入口冻结输入，并规定入口解析一次、子树递推。
祖先 0.5、source 无覆盖及子节点覆盖 0.8 的行为按最近显式值定义，不沿链相乘，
与 [Figure 样式契约](../../design/architecture/figure-style.md)一致。
L378 已将该反例纳入后续验收。

## 固化和验收沿用既定边界

共享场景与宿主背景仍分别批准、固化决策和验收。
共享场景先固化 capture 例外、身份/终止、资源多版本及结构准入契约；
宿主背景另行固化统一帧输入、资源 lease 和恢复契约。
执行状态仍只由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

实现阶段保留方案 L376–389 的全部反例，重点验证跨 Runtime 交错提交和过期候选、
整批拒绝无部分写入、source 暂停/在途时捕获、资源旧新版本共存及背景恢复。
独立消费者和平台证据按各自范围记录，不能用文档复核替代。

本次只新增复核记录与导航，未扫描 Novadraw Rust 实现、未执行功能或平台测试，
未改写现行 accepted 契约。实施按两个独立决策经用户批准后推进。
