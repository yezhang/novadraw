# ADR-057 修订 2 审阅回应

类型：`architecture-review`

日期：2026-10-30

能力归属：Core Observations、Inspector 与宿主恢复编排为主要修订；
Connections 校正消费者用例；Core Animation 与 Editor 扩展错误保持原范围。

状态：S09-R1-01～07 已逐项修订并完成文档自审，待复核与接受。
未修改 Rust、运行新行为测试或启动 Execute。原
[修订 1 审阅记录](adr-057-revision-1-review-2026-10-30.md)保持原文。
当前目标为 [ADR-057 修订 2](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)与
[验证矩阵](../plans/p2-s09-service-failure.md)。

## 逐项回应

| 审阅 ID | 所选合同与修改位置 | 未来验证 |
|---|---|---|
| S09-R1-01 | “Inspector 本地故障”：events／clear／入队都发布首 fault；第一次发现操作必填，record 位置仅 callback 可用；Clone 共享、不覆盖；fault 实例拒绝 attach | SF-03／04／08／18：读取或清空先发现、无新 record、共享状态与首次值不变 |
| S09-R1-02 | “锁竞争与故障发布”：选择不等待宿主恢复，允许健康竞争等待短临界区；不丢弃或将竞争当 poison；首 fault 为固定字段、独立有界非 poison 发布，不执行用户代码 | SF-04／14：正常竞争、满／未满队列、等待与复制时间、首 fault 竞争发布 |
| S09-R1-03 | “宿主恢复与事件边界”：暂停新驱动并结清回执，清理旧 token，正常结清旧 journal，checkpoint，attach，snapshot，再核对 checkpoint，接受后恢复驱动 | SF-08／18／20：旧 record、步骤间变化、未稳定、部分成功和失败后的 token 清理 |
| S09-R1-04 | “订阅所有权”接口表：attach／attach_scoped fallible；scope 显式；token 不 Clone／Copy；detach 借用 token，实际登记移除才返回变化，foreign 后可向正确 owner 重试 | SF-06／07／18：自动注销后的首次 detach、重复调用、fault 清理与 foreign 后重试 |
| S09-R1-05 | “ADR-010 的局部替代关系”：列出注册／注销返回值、foreign、本域合法缺失和 ID 耗尽条款；耗尽只拒绝本次注册，Runtime 与已有登记保持健康 | SF-07／17：失败不改计数／owner／登记，既有订阅仍可观察与注销 |
| S09-R1-06 | “外部消费者证据”：faulted owner 拒绝无登记；另在健康或重建后的 owner 注册有效 Router；换对象不修复原 owner | SF-09：明确拒绝与成功分别发生在哪个 Runtime |
| S09-R1-07 | “对标与合理变体”：按四片使用账本既有 Animation、通知、Connections、GEF family；Inspector 本地故障不当作 Draw2D 新能力 | 各片 SF ID 与 family 对照回填，保留其他平台未完成状态 |

## 两项取舍的理由

健康锁竞争选择允许短临界区等待。严格不等待锁需要另行定义 record 丢弃、
诊断和时间线缺口，会扩大本批合同；仅因正常竞争就使 Inspector fault 也不合适。
本方案把“不阻塞 Core”改为“不等待宿主恢复”，不宣称实时或 lock-free。
事件锁不跨 Runtime 驱动或恢复步骤持有，首 fault 发布不调用用户代码。
实现与测量须证明这些限制，文字修订不能代替性能验证。

恢复需要每个 Runtime 的明确已发布边界。现有 epoch 本身不能区分待分发 journal，
因此修订 2 显式列出 `Observations::checkpoint` 为 S09-B 的 Core 基础增量。
该查询只读 namespace、stable epoch 和最后已发布 sequence；
遇未稳定、在途提交或待发布 journal 时拒绝，不能自己 flush。
宿主在暂停期间先正常结清旧工作，新 Inspector 从接受的快照边界之后开始采集。
这不增加历史重放、跨 Runtime 原子快照或全局顺序。

token 采用借用式 detach，与 Inspector 的 Clone 分开。
foreign 错误不会消耗清理凭证；scope 自动注销后，首次显式调用也可正确返回未变化。
多次 attach 的每个 token 仍由宿主独立管理；共享 Inspector fault 后的新 attach 拒绝，
其余登记在下一 callback self-remove，或由宿主显式清理。

## 旧决策与证据保护

ADR-010 只增加待生效的局部替代指引，原注册／注销与耗尽判断保留。
ADR-057 接受前，没有改变 ADR-010 的效力；也不扩大到 record sequence 或通用 arena。
修改前的 ADR-010、ADR-057 修订 1 与矩阵原始字节及散列见
[输入快照收据](../evidence/p2-s09-design-2026-10-30/revision-2-inputs.json)。
修订 1 文本 SHA-256 与原审阅记录的
`baef90fdad9516d0d8f03c15f056e37f0d80d9f5128f3db0ea09e296cae225db` 一致。
首轮盘点、九项既有基线结果与此次新增场景分别记录，不把旧基线当新合同已通过。

## 本轮复核范围

已逐项核对七条意见、ADR 正文与矩阵的主体、顺序、失败后状态和生效条件；
所有语义 family 来自既有 Draw2D／GEF 账本。
本轮使用技术文档清晰度与好好说话复核逻辑和中文表达。
这属于文档自审，不宣称独立 reviewer 已复核或架构已获接受。
文档链接和差异检查结果保存至本轮证据目录。
