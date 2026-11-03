# ADR-057 修订 2 复核

类型：`architecture-review`

日期：2026-10-30

能力归属：Core Observations、Inspector 与宿主恢复编排。
Animation、Connections 与 Editor 的原切片范围不变。

状态：上轮意见已有对应修订；建议补清下列三项后接受整篇方案。
本轮不表示 ADR 已获接受、S09 已获实施批准或新增行为已通过验证。

## 基线与审阅范围

对象为 [ADR-057 修订 2](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)，
文本 SHA-256 为
`4cb372c8e67c736937de5845cc11aff8e4a5721dcdd220521d978439af6a132f`。
下文行号仅定位本轮文本；后续按章节和原句核对。

本轮对照 [修订 1 审阅意见](adr-057-revision-1-review-2026-10-30.md)、
[修订 2 回应](adr-057-revision-2-response-2026-10-30.md)和
[验证矩阵](../plans/p2-s09-service-failure.md)，补充核对 ADR-002／010／014／016／018／031／044
及 Inspector、UpdateManager 的现行设计。未重新扫描 Rust 实现，也未运行行为或性能测试。
以下判断限于设计合同与文档一致性。

## 上轮意见的处理结果

| 原意见 | 复核结果 |
|---|---|
| S09-R1-01：所有 poison 发现入口发布首 fault | 主要合同已补齐：读取／清空／入队共用发布路径，来源可以为空，Clone 共享且 fault 后拒绝 attach；残留表述见 R2-03 |
| S09-R1-02：锁竞争合同 | 已解决原解释缺口：明确允许健康锁竞争等待，不等待宿主恢复，不宣称实时或 lock-free；性能仍待实施测量 |
| S09-R1-03：快照与新订阅衔接 | 已补暂停、旧 journal 结清、两次 checkpoint 和失败清理；新增查询的准入与游标定义仍需补清，见 R2-01／02 |
| S09-R1-04：token 公共合同 | 已解决：接口结果、scope、非 Clone／Copy token、借用式 detach、foreign 后重试及自动注销后的未变化均明确 |
| S09-R1-05：ADR-010 替代关系 | 已解决：条款替代与接受前效力明确，耗尽仅拒绝本次注册且不破坏已有登记 |
| S09-R1-06：Router 重试用例 | 已解决：拒绝与成功分别归于 faulted 和健康／重建后的 owner，不用换对象暗示原 owner 恢复 |
| S09-R1-07：api_semantics | 已按切片补齐既有 family，区分 Inspector 本地合同与 Draw2D／GEF 语义 |

上述“已解决”只表示原设计缺口已有明确回应，不代表实现或测试通过。
本轮没有新增 S09-A Animation 阻断意见。

## 剩余意见

### S09-R2-01（P1）：说明 checkpoint 等待 backend 完成的必要性，补齐无 GPU 的恢复路径

归属：Core Observations 查询；宿主负责 backend 回执编排。

位置：ADR L176–199，恢复要求无在途提交，且
`Observations::checkpoint()` 将在途提交归为 `NotStable`。

问题：这给稳定场景与已发布 journal 的查询增加了 backend 完成前提。
[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)区分 stable scene publication
与 presentation acknowledgement；
[ADR-002](../../adr/adr-002-notification-effect-queue.md)明确无像素变化的已稳定事实
不应要求产生 GPU 帧才允许观察。
修订 2 尚未说明已有场景和 journal 都稳定时，在途帧为何仍使 checkpoint 不可读，
以及 headless、Idle、Suspended 或 backend 无法正常完成时如何结清恢复边界。

后果：Inspector 的恢复准入可能额外受 GPU 完成时机约束。
“通过正常帧／通知边界结清”需要公开合同证明，不足以单独证明这些状态都能推进。
本轮没有运行实现，因此不断言这些场景已出现恢复失败。

建议：checkpoint 优先按场景稳定、待发布 journal 和分发阶段判断准入。
未来 backend completion 产生的事实可作为新边界之后的事件处理；
若宿主确实需要先等待某一帧回执，将其列为独立恢复策略并说明必要性。
若仍保留 Core 查询的在途拒绝，明确它与既有稳定／呈现分层的关系，
以及不依赖成功 GPU 呈现的结清或失败退出路径。
不为恢复公开内部 queue 或绕过正常回执协议。

复核条件：扩展 SF-20，明确无像素变化、headless、Idle／Suspended、
延迟 backend completion 和 backend 失败的结果。
公开消费者证明恢复所用驱动路径；没有办法结清时返回明确失败，不无限等待或伪造完成。

### S09-R2-02（P2）：定义“最后已发布 sequence”的推进点与持久性

归属：Core Observations。

位置：ADR L190–199，checkpoint 返回 namespace、stable epoch 与最后已发布 sequence，
空 stream 使用 None。

问题：“已发布”尚未明确对应 record 编号分配、稳定 journal 出版、开始 callback，
还是完整分发结束。零 listener、scope 无匹配、self-removal 或队列已排空时，
也未说明游标如何保持。恢复和独立消费者需要同一含义来比较边界。

建议：明确 stream 与游标的 owner、推进时机及单调合同。
区分已分配但待发布的编号与可作为 checkpoint 的已发布边界；
明确无匹配 listener 的 record 是否仍推进该边界。
读取／排空／Inspector clear、订阅与注销不应让既有边界倒退；
None 应表示从未发布 record，而不是当前队列为空。
分发正在进行或普通 observer panic 时应明确查询结果，继续遵守既有 fault 合同。

复核条件：SF-20 覆盖从未发布、已发布后队列为空、零 listener、scope 无匹配、
self-removal、后续注册及分发 panic。
两次 checkpoint 比较的字段与事件来源身份保持一致，不把编号分配当作送达证明。

### S09-R2-03（P2）：统一首 fault 来源的条件表述

归属：Inspector。

位置：ADR L74–79 已允许读取／清空发现时来源为空；
L90–91 仍写入队发现时保存首 fault 并携带当前 record 来源；
L152 仍无条件写“首 fault 的来源包含 Runtime namespace”。

问题：L152 与可为空的来源合同直接冲突。
竞争发布时，当前入队操作也可能不是首 fault 的胜出发布者，
不能用当前 record 位置覆盖另一个入口已发布的首次值。

建议：将来源规则统一为“首 fault 若由 observer 入队路径成功发布，则包含当前 record
的 Runtime namespace／epoch／sequence；读取或清空路径成功发布时来源为空”。
明确竞争场景中的“首个”以首次成功完整发布为准，后续发现读取既有值，不补填来源。
步骤 2 按此条件描述，避免把当前失败 record 与首次 fault 混为一条诊断。

复核条件：SF-03／04／18 覆盖读取先发布空来源、后续 observer callback，
以及两个入口竞争发布；首 fault 的操作和来源始终对应同一个胜出值。

## 交接与限制

下一轮回应 S09-R2-01～03，并同步必要的 SF-20 与首 fault 验证条件。
保留原修订回应和本记录的基线判断；本记录不修改 ADR 正文、执行状态或 Rust。
复核通过与用户接受、实施完成分别记录。

本记录已按技术文档清晰度与好好说话复核事实、逻辑和中文；
文档检查只验证链接、类型和格式，不证明上述行为或性能。
