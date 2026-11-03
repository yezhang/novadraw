# ADR-057 修订 1 审阅意见

类型：`architecture-review`

日期：2026-10-30

能力归属：Core 的 Animation、Observations 和 Connections；Inspector 的本地故障与订阅；
Editor 的扩展错误。主要待修问题属于 Core Observations、Inspector 和宿主恢复编排。

状态：审阅完成，建议修改后接受；以下意见待修订与复核，不表示 ADR 已接受或获准实施。
本记录用于交接另一 Agent 修改方案，不改变 S09 的执行状态。

## 审阅基线与结论

审阅对象为 [ADR-057 修订 1](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)。
工作区 HEAD 为 `9825957efb1859ffca504e03eafd79ff19a226bd`；
ADR 包含未提交改动，本次审阅文本的 SHA-256 为
`baef90fdad9516d0d8f03c15f056e37f0d80d9f5128f3db0ea09e296cae225db`。
下文行号仅定位该文本，修改后应按章节和原句查找。

本轮交叉审阅了 ADR、[现状盘点与设计自审](p2-s09-design-inventory-2026-10-30.md)、
[S09 验证矩阵](../plans/p2-s09-service-failure.md)、ADR-010／014／038 和相关语义账本。
未重新扫描 Rust 实现，未运行行为测试或性能测量。以下发现是设计合同缺口与文档冲突，
不据此宣称实现存在对应缺陷。

S09-A 的 Animation 目标已比较明确。修订 1 已处理普通 behavior 拒绝保留成功前缀、
共享 listener 分配器和错误 source 与相等比较的边界。
主要剩余问题是 Inspector fault 的发布入口、锁竞争承诺和恢复时的快照／订阅衔接。
本轮未提出新增 Animation 阻断项，但这不构成 S09-A 的实施批准。

## 待修意见

### S09-R1-01（P1）：所有 poison 发现入口应明确首 fault 的发布行为

归属：Inspector。

位置：ADR L71–81，“events 和 clear_events 的锁 poison 返回结构化 Inspector 错误”，
以及 observer 发现 poison 后保存首个本地 fault 的步骤。

问题：读取与清空接口只规定返回操作错误，首 fault 则在 observer 路径发布。
如果之后没有新事件，宿主可能读到操作错误，却仍从采集状态入口读到健康状态。
读取入口也没有当前 record，不能强行填入 epoch／sequence。

建议：所有发现本地 poison 的入口统一发布首 fault，明确首次发布后不被覆盖。
操作类别必填；事件来源位置仅在 callback 中可用时填写。
补充共享实例 fault 后 attach 是否拒绝，以及后续 callback 如何停止入队并请求注销。

复核条件：扩展 SF-03／04／18，覆盖读取或清空先发现 poison、之后没有新 record、
Clone 读取共享 fault，以及首 fault 不被后续发现覆盖。

### S09-R1-02（P1）：“不阻塞 Core”需要锁竞争合同

归属：Inspector；Core 负责维持 observer 分发边界。

位置：ADR L83–89，“该路径不抛出 panic、不阻塞 Core”，
以及首 fault 使用不依赖事件锁的发布路径。

问题：不依赖事件锁不能单独证明 fault 发布不会等待或再次 poison。
正常入队也涉及锁；目前没有定义锁竞争时的行为，无法判断“不阻塞”的承诺范围。

建议：明确承诺是“不等待宿主恢复”，还是严格“不等待锁”。
若选择后者，定义竞争时的记录处理、诊断与时间线缺口语义，
并说明首 fault 状态如何有界发布、为何不会引入另一条等待或 poison 路径。
本意见不预先指定同步原语，也不把正常锁竞争自动等同于 poison。

复核条件：SF-04／14 的场景和指标能检查所选承诺；
正文明确等待、丢弃或拒绝的行为，不仅验证 poison 分支。

### S09-R1-03（P1）：恢复需定义快照与新订阅的衔接边界

归属：宿主编排；Inspector 消费 Core 的稳定查询和订阅协议。

位置：ADR L81–86，“显式 detach、重建 Inspector 后再 attach”，
以及“重连需重新捕获稳定快照，并标明时间线缺口”。

问题：快照捕获和 attach 的顺序未定义。宿主在两步之间驱动 Runtime，可能漏记变化；
已有待分发 journal 也可能与快照重叠。仅要求标明缺口，仍不足以判断新时间线从哪里开始。
[ADR-010](../../adr/adr-010-runtime-listener-lifecycle.md)和
[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)区分历史 record 与最新稳定查询；
[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)还保留稳定查询的 fault／未稳定准入拒绝。

建议：定义宿主暂停驱动期间的恢复步骤，明确旧登记清理、新订阅建立、
稳定快照捕获及恢复驱动的顺序。说明待分发 journal 如何处理，
记录每个 Runtime 的快照／事件边界，失败时清理已建立的新 token。
不增加历史重放或跨 Runtime 全局顺序承诺。

复核条件：扩展 SF-08／18，覆盖恢复间隙的场景变化、待分发 record、
快照或 attach 失败，以及每个 Runtime 的边界标识。

### S09-R1-04（P2）：补全 token 和 attach／detach 的公共合同

归属：Inspector API；依赖 Core Observations 身份与注销规则。

位置：ADR L93–122，订阅所有权、重复 detach、foreign token 与共享实例规则。

问题：所有权已有说明，但 attach／detach 的返回类型、scope 参数、
token 是否可 Clone，以及 detach 出错后 token 是否仍可向正确 Runtime 重试尚未明确。
“首次移除返回成功”也应区分首次调用和登记实际被移除。

建议：补充接口合同表，列出参数、结果、fault／耗尽／身份失败及错误归属。
detach 宜借用 token，使 foreign 调用失败后仍能向正确 owner 重试；
若采用其他所有权形式，应给出等效的失败后保留合同。
明确登记实际存在且被移除时返回变化，Figure dispose 或 self-removal 后返回未变化。
Inspector 的 Clone 与 token 的 Clone 是不同问题，应分别说明。

复核条件：SF-06／07／18 覆盖 foreign 调用后向正确 owner 重试、
自动注销后的首次 detach、重复 detach、fault 后清理和 token 的所有权使用。

### S09-R1-05（P2）：明确对 ADR-010 的条款替代

归属：Core Observations。

位置：ADR L98–110、L139–141，foreign 注销结构化错误、fault 后清理和分配耗尽拒绝。

问题：[ADR-010](../../adr/adr-010-runtime-listener-lifecycle.md)仍规定未知或已移除 ID 返回 false，
并将 listener ID 分配溢出视为不可恢复 invariant。
ADR-057 改变了相关失败语义，却没有明确条款替代关系。

建议：列出 ADR-057 接受后替代 ADR-010 的具体条款，
区分 foreign、本域合法缺失和已移除身份。
明确分配耗尽是否仅拒绝本次注册、Runtime 是否保持健康。
按文档维护规则处理旧决策中的替代指引，保留原历史判断，不扩大到其他服务。

复核条件：SF-07／17 对应唯一、无冲突的当前合同；
耗尽后的 Runtime 健康状态、登记状态与注销能力均有明确要求。

### S09-R1-06（P2）：修正 Router“注册拒绝后有效重试”的用例

归属：Core Connections；独立消费者验证。

位置：ADR L137–138 确认对象注册没有 validate 回调，真实拒绝准入是 Runtime fault；
L177–179 仍要求“外部 Router 注册拒绝后有效重试”。

问题：faulted Runtime 不会因换一个 Router 就恢复。
该用例容易被解释为在同一 faulted owner 上换有效输入即可成功，
与前文及 SF-09 的真实拒绝条件不一致。

建议：改为 faulted Runtime 拒绝且不留下登记或伪 ID；
健康或重建后的 Runtime 接受有效对象。
不虚构对象校验回调，也不混入 route 阶段的输入错误。

复核条件：ADR 的外部消费者要求与 SF-09 一致，并明确成功注册发生在哪个 owner 上。

### S09-R1-07（P2）：按切片补全 api_semantics

归属：Core／Editor 分项维护语义映射。

位置：ADR L171–172，当前只列两个通知 family 和三个 GEF family。

问题：列表遗漏 Animation、Anchor／Router，以及共享 listener 改动涉及的其他通知 family，
不利于分片验收后准确回填语义账本。

建议：按 S09-A、Observations／Inspector、Connections、Editor 分片列出受影响 family。
Animation 核对 `animation.timeline`／`animation.presentation`；
Connections 核对 `connection.anchor`／`connection.router`；
通知范围按实际公共入口核对，不仅限于 property 和 layout_update。
沿用 [Draw2D 账本](../../parity/draw2d/api-coverage.md)与
[GEF 账本](../../parity/gef/api-coverage.md)的既有 ID，不新增无依据的语义 family。

复核条件：每片的改动、验证用例和 family 有对应关系；
Inspector poison 仍标为 Rust 本地合同，不计作新增 Draw2D 对等能力。

## 接手修改范围与交付

下一 Agent 先修订 ADR-057，并同步必要的验证矩阵和关联决策替代指引。
执行状态仍以 [P2 backlog](../../roadmap/p2-delta-backlog.md)为准。
本记录仅保存本轮审阅意见，不授权修改 Rust、启动 Execute 或关闭来源审计项。

修订交付应逐项回应 S09-R1-01～07，列出修改位置、所选合同和仍待验证的部分；
有不同取舍时说明理由，不将本记录中的建议当作已批准的规范。
本记录保留原基线判断，后续结论另建复核记录并链接。

本记录已按技术文档清晰度与好好说话检查事实、逻辑和中文表达。
建议、待补合同及未来复核条件均未标为已实现或已通过。
