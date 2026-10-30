# ADR-057 修订 3 复核回应

类型：`architecture-review`

日期：2026-10-30

能力归属：Core Observations 的 checkpoint／通知分发、Inspector 首 fault 与宿主恢复。
Animation、Connections 和 Editor 的既有切片范围不变。

状态：S09-R2-01～03 均合理，已全部修订并完成文档自审，待复核与接受。
未修改 Rust、运行新行为测试或启动 Execute。
原[修订 2 复核](adr-057-revision-2-review-2026-10-30.md)及
[修订 2 回应](adr-057-revision-2-response-2026-10-30.md)保留原文。
当前合同见 [ADR-057 修订 3](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)，
验证条件见 [S09 矩阵](../plans/p2-s09-service-failure.md)。

## 逐项处理

| 意见 | 判断与所选合同 | 修改位置／验证 |
|---|---|---|
| S09-R2-01 | 合理。场景和通知稳定不应被 backend 完成额外限制。取消在途帧独立拒绝，延迟回执的后续事实归新边界之后；公开 CPU 驱动结清通知，失败明确退出 | ADR“宿主恢复与事件边界”；SF-20 的 headless、Idle／Suspended、无像素变化、延迟完成与失败分支 |
| S09-R2-02 | 合理。最后已发布游标由每个 Runtime 通知 stream owner 持有，在 record 的完整分发处理结束后推进；编号分配或开始 callback 不算发布 | ADR“已发布游标”；SF-20 的空 stream／空队列、零 listener、scope 无匹配、self-removal 与 panic |
| S09-R2-03 | 合理。首 fault 以首次成功完整发布的值为准。操作与来源属于同一候选；读取／清空胜出时来源为空，后续 observer 不补填 | ADR“Inspector 本地故障”和“订阅所有权”；SF-03／04／18／20 的先后及竞争发布 |

## 不依赖 GPU 的恢复

修订 2 把无在途 submission 写成 checkpoint 的必需条件，
但既有 ADR-002／014 区分稳定场景与实际呈现，不能据此增加 GPU 准入。
修订 3 只检查源／必需派生稳定、待发布 journal、分发阶段和 Runtime fault。
backend completion 暂不发生时也可恢复场景诊断；
若产品要等某帧实际呈现，宿主独立设置等待及失败／超时策略。

为避免把“正常通知结清”写成无法调用的口号，本轮定向核对已有公开入口：
`RuntimeFrames::stabilize_for_query` 委托稳定化与 behavior processing，
当前不承诺完整外部通知 flush。因此在 S09-B 显式列出该现有入口的行为增量：
稳定化成功后按正常顺序完整分发已有合法事实，无需录制、submission 或完成回执。
不伪造 Painting／Prepared／Submitted，不消费在途身份或清掉未绘制 damage。
这是未来实现和公开消费者的退出要求，不宣称现有代码已满足。

宿主暂停期间暂存到达的回执任务，恢复后仍走正常 completion 协议。
延迟回执的新事实属于后续边界；实际失败若导致 Runtime fault，则查询明确 Faulted，
停止恢复并清理／重建 owner。稳定化或通知分发失败也明确退出，
不公开内部 queue、不绕过回执、不无限等待或伪造成功。

## 游标与首 fault 的统一含义

游标表示正常 record 分发已完成的前缀，不是每个 listener 的送达证明。
没有匹配 listener 也能完成分发并推进；队列清空或 Inspector clear 不使游标退回 None。
正在分发时查询拒绝，panic 后遵循 fault 合同；
未完成的当前 record 不推进，已有完成前缀保留，无重放承诺。

首 fault 来源从属于成功完整发布的候选，而非最近发现问题的入口。
observer 当前 record 未入队和首 fault 可能是两条不同诊断：
读取已先发布空来源 fault 时，之后入队发现不能将当前来源补进去。
正文与矩阵已消除无条件要求首 fault 包含 namespace 的旧句。

## 本轮检查与证据

修改前的修订 2 和矩阵已保存原始字节，
[输入快照](../evidence/p2-s09-design-2026-10-30/revision-3-inputs.json)中的 ADR 散列与
原复核基线 `4cb372c8e67c736937de5845cc11aff8e4a5721dcdd220521d978439af6a132f` 一致。
本轮检查只覆盖文档、静态合同和公开入口定位，没有重新开展实现审计。
使用技术文档清晰度与好好说话复核逻辑、状态和中文；
文档门禁不等于行为、性能、架构接受或独立复核通过。
