# S09 服务失败验证矩阵

类型：`verification-plan`

状态：已接受，实施中；2026-10-30。能力归属：Core／Editor／Inspector 与宿主分项验收。目标见
[ADR-057](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)。
本页定义退出条件；[S09-A](../reviews/p2-s09-a-implementation-2026-10-30.md)已完成，
SF-01／02／16 和 SF-12／13／14／15 的 Animation 部分通过；
[S09-B](../reviews/p2-s09-b-implementation-2026-10-30.md)的
SF-03～08／17／18／20 与本片公开 API／自动门禁已通过；
SF-14／15 的 Inspector 成本覆盖 64／4096 配置，未验收理论最大容量。
[S09-C](../reviews/p2-s09-c-implementation-2026-11-02.md)的 SF-09 与
SF-12／13 的 Connections 部分已通过。
[S09-D](../reviews/p2-s09-d-implementation-2026-11-02.md)已实现，
SF-10／11／12／19 的定向验证与 24 项 Native 消费者测试通过；
最新 Wasm、成本与最终门禁仍待补验，
本片与整批均未关闭。
SF-15 的体积比较按用户明确请求执行，不作为本片默认门禁。

当前为修订 3（2026-10-30），按[评审结论](../reviews/service-failure-review-status.md)
补齐无 GPU 恢复、已发布游标推进／持久性，以及首 fault 胜出值的来源规则。
[现状盘点](../reviews/p2-s09-design-inventory-2026-10-30.md)的基线保留。
已运行的九项旧测试只建立基线，不勾选本矩阵的新行为。

| ID | 场景 | 必须观察 |
|---|---|---|
| SF-01 | Animation mode／suspension 同值、异值与已 faulted 同值 | 未变化、变化、fault 分离；fault 检查先于同值短路 |
| SF-02 | behavior processing 普通无变化、拒绝、混合成功与 panic | 成功 bool 仅描述 presentation；普通拒绝进入有界失败队列，保留成功前缀与已消费事实；panic 遵循 owner fault |
| SF-03 | 读取／清空先成功发布 poison，之后无新 record 或到达 callback | 胜出操作必填、来源为空；Clone 看到同一 fault；返回操作错误；后续 observer 不覆盖／补填 |
| SF-04 | 健康锁竞争、poison 候选竞争与已 fault callback | 健康竞争不丢弃／fault；首次完整发布候选胜出，其操作和来源是一组；其他候选只读取首值；已 fault self-remove，其他 observer 完成 effect |
| SF-05 | 普通外部 observer panic | Runtime fault 保持；不能降级为 Inspector 本地错误 |
| SF-06 | Runtime／Figure scope attach、dispose／self-remove 后首次 detach、重复 detach | token 不 Clone／Copy；实际移除为 true，已自动移除为 false；scope 拒绝无登记 |
| SF-07 | foreign detach 后向正确 owner 重试、Runtime fault 后清理 | detach 借用并保留 token；foreign 结构化错误，正确 owner 可重试；清理不驱动 callback／稳定化 |
| SF-08 | 满缓冲、fault 实例 attach 拒绝、健康实例恢复 | 先淘汰最旧再入队；共享 fault attach 无 ID／登记；记录缺口，不补造或重放丢失历史 |
| SF-09 | faulted Runtime 拒绝注册；健康或重建后的 owner 接受对象 | 一个规范 fallible 入口；fault 下无登记／伪 ID；更换对象不能恢复原 faulted owner |
| SF-10 | 自定义 Command／Policy Error 的多层包装 | 原始 source 可追溯／downcast；分类与显示文字独立 |
| SF-11 | 文本／非文本 panic payload | 文本保持；其他只保存分类；不调用用户格式化器 |
| SF-12 | 失败前已提交前缀与未提交候选 | 沿用既有原子性层级，不承诺任意扩展副作用回滚 |
| SF-13 | 独立消费者与 Native／Wasm API | 内置与外部共用协议，旧 panic alias compile-fail |
| SF-14 | 同值动画、健康锁竞争、首 fault 发布与事件入队成本 | P50／P95、等待时间、分配、复制、满／未满缓冲；不以 poison 分支证明正常入队无等待 |
| SF-15 | 同配置 release 与可选能力关闭 | Native／Wasm 原始／压缩字节、依赖与首次资源 |
| SF-16 | 空 effects、无活动轨道和已有 fault 的私有 Animation 入口 | 健康检查先于空输入短路；suspension 配置变化与 presentation damage 分开 |
| SF-17 | listener 简写／scoped 准入、计数器耗尽及后续操作 | 注册拒绝只影响本次；Runtime 保持健康，计数／已有 listener／owner 表不变；既有订阅可继续观察／注销；不回收 sequence |
| SF-18 | 多个 attach、Clone、各 Runtime 的首 fault 与恢复边界 | Clone 共用首 fault，不覆盖首次值；token 独立注销且错误不消耗；逐 Runtime 保存 namespace／epoch／sequence／scope，不建跨 Runtime 全局顺序 |
| SF-19 | owned Error source 与旧 Eq／PartialEq 消费者 | 分类、位置、显式字段与 downcast 验证；不按字符串或指针伪造原因相等 |
| SF-20 | 无 GPU 的暂停恢复、旧 journal、游标与失败清理 | 公开稳定化驱动结清通知，不要求完成在途帧；两次 checkpoint 一致后恢复；完整分发才推进游标，错误清理已建 token |

SF-20 至少包含：空 journal／有待分发 record、在途提交、未稳定／fault query、
注册耗尽或 scope 失败、快照失败、两次 checkpoint 不一致、多 scope 部分成功、
清理错误以及多 Runtime 部分失败。注入暂停步骤间变化验证拒绝；
恢复驱动后的变化验证实际记录。旧 record 不能作为新基线之后的事件，
scope 过滤导致 sequence 跳号不解释为丢失或重放需求。
`checkpoint()` 是无副作用查询：有待发布 journal、正在分发或源／必需派生场景
未稳定时拒绝，不能偷偷 flush。在途帧或仅有 damage 不单独造成 NotStable。
从未完整发布 record 的游标为 None；已发布后队列变空仍保存先前游标。

SF-20 的公开消费者还须通过修订后的 `Frames::stabilize_for_query()` 验证：

- headless、无像素变化、Idle／Suspended 和 backend 暂不完成时，无 GPU 通知结清可成功；
  不产生伪造 render／presentation 事件，不改在途帧身份或未绘制 damage。
- 暂停期间延迟回执不被伪完成；恢复后按正常协议处理，其新事实位于基线之后。
  backend 失败尚未使 Runtime fault 时不预先拒绝查询，实际 fault 后明确拒绝并退出恢复。
- 稳定化／分发不能完成时返回错误或按已有 panic fault 边界退出，
  清理 token，不无限等待 backend；不把公开驱动成功等同于 checkpoint 已可读。
- 零 listener、scope 无匹配、self-removal 后正常完成的 record 均推进游标；
  读取／排空、Inspector clear、后续注册／注销不使其倒退。
- 编号分配、分发开始或当前 callback 未完成不计作完整发布；
  分发中查询 NotStable，普通 observer panic 后 Faulted，保留之前完成的前缀。
- 两次 checkpoint 按 namespace／stable epoch／最后完整发布 sequence 比较，
  多 Runtime 不共享游标；首 fault 两入口竞争时操作与来源始终对应同一胜出值。

首片 S09-A：

- SF-01、SF-02、SF-16 与直接帧调用方错误传播；
- SF-12 的成功前缀、已消费事实与 panic 基线回归；
- SF-13 的 Animation 外部调用、Native／Wasm API 和 set_mode 调用方迁移；
- SF-14／15 中 Animation 可达路径的成本记录。

Observations／Inspector 的 SF-03～08、SF-17／18／20 随第二片实施，
Connections 的 SF-09 已随第三片完成，Editor 的 SF-10／11／19 随第四片实施。
分片通过不等于 S09 整批关闭。

S09-B 的公开恢复注册拒绝使用真实 foreign scope，私有 Core 注入验证身份耗尽；
两者分开记录，不增加公共测试 hook。现有容量域是 `NonZeroUsize`，
没有有限全局上限；SF-14 的最大允许容量要求不据 4096 的采样结果勾选。
实测范围、分配限制与未覆盖的 payload 成本保留在实施记录。

每片对照 ADR-057 的 `api_semantics` 表回填既有 family；
Inspector poison／锁竞争是本地合同，宿主恢复是编排合同，
不计作新增 Draw2D 对等能力。listener 合同替代只在 ADR-057 接受后生效。

Execute 前先审阅现状，逐项列出受影响入口与错误类型。
不靠历史审计行号声称完成盘点，也不为 poison／revision 耗尽增加公共测试 hook。
私有故障注入、公开生命周期、外部消费者与 compile-fail 分开统计。

内环使用受影响 crate 的 lib 或单 integration target；最终运行 workspace
fmt、严格 Clippy、test、API／依赖边界及 docs。现有 check／has 契约做回归，
不把已完成能力重新定义为新增交付。
完成记录按实际覆盖关闭 R07、TC-10、API-F08、API-F10，
仅回填 API-F09 残余 register alias。
