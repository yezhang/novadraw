# ADR-057：服务失败与 Inspector 订阅生命周期

类型：`architecture-decision`

能力归属：跨包整改，Core／Editor／Inspector 分项负责；不属于单一 GEF 能力批次。

| owner | 本决策范围 | 验收归属 |
|---|---|---|
| Core（`novadraw`） | Animation、Anchor／Router 注册与 Observations 基础协议 | 服务结果、身份与故障契约 |
| Editor（`novadraw-editor`） | Command／Policy 扩展错误与调用位置 | 编辑事务边界及错误 source |
| Inspector（`novadraw-inspector`） | 本地 poison、首 fault 与订阅生命周期 | 观测与故障隔离 |
| 宿主／消费者 | attach／detach 配对和生命周期编排 | 各包公开 API 的接入证据 |

各包改动分别记录，不因 Editor／GEF 推进授权而启动 Core 或 Inspector 整改。

状态：已接受；2026-10-30。用户批准修订 3 并授权按切片推进 P2-S09。
执行状态见 [P2 backlog](../roadmap/p2-delta-backlog.md)。

当前为修订 3（2026-10-30），已按[修订 2 复核](../verification/reviews/adr-057-revision-2-review-2026-10-30.md)
补清 checkpoint 与 backend 完成的分层、发布游标和首 fault 胜出值的来源。
逐项处理见[修订 3 回应](../verification/reviews/adr-057-revision-3-response-2026-10-30.md)。
修订 2 的[回应](../verification/reviews/adr-057-revision-2-response-2026-10-30.md)与
[原始文本](../verification/evidence/p2-s09-design-2026-10-30/adr-057-revision-2.txt)保留。
修订 1 的[源码盘点](../verification/reviews/p2-s09-design-inventory-2026-10-30.md)与
[原始文本](../verification/evidence/p2-s09-design-2026-10-30/adr-057-revision-1.txt)保留。
修订 3 的[设计复核](../verification/reviews/adr-057-revision-3-review-2026-10-30.md)已通过。
评审时的[修订 3 原文](../verification/evidence/p2-s09-design-2026-10-30/adr-057-revision-3.txt)
按复核记录的 SHA-256 保留，后续状态回填不改写该快照。
首个实施片 S09-A Core Animation 已完成，见[实施记录](../verification/reviews/p2-s09-a-implementation-2026-10-30.md)；
下一片为 S09-B Observations／Inspector，后续切片按执行队列串行推进。

## 问题与范围

R07、TC-10、API-F08 和 API-F10 指出，部分服务把故障解释为未变化或空集合。
API-F09 的资源读取整改已完成，本批仅承接残余 connection 注册别名。
CommandStack 的 check／has 分离已有完成证据，不重新列为缺口。

本决策从既有公开契约和审计证据定义目标，未扫描当前 Rust 实现。
实施前再核对 S03 迁移后的调用点，形成逐入口清单；历史行号不作为现状证据。

以上为首稿的形成方式。修订 1 已盘点当前入口、注册分配器与直接调用方；
具体证据和现状缺口见本轮复核，不把旧审计行号当现状证据。

## 决策

### 服务结果

保留领域错误，不建立横跨 Core、Editor 和 Inspector 的万能错误类型。
可能失败的 mutation 返回 Result；布尔值只表达已完成操作的变化事实。

| 结果 | 含义 | 状态与恢复 |
|---|---|---|
| `Ok(true)` | 操作成功并改变服务状态 | 按领域契约发布必要失效 |
| `Ok(false)` | 操作成功，目标状态已满足 | 不生成变化 revision、damage 或重复通知 |
| 领域拒绝 | 输入、身份或阶段不允许 | 保留该操作之前的状态，可提交有效输入重试 |
| 服务已 faulted | owner 已不允许执行该操作 | 原样返回 fault，不能解释为 false |
| 扩展 panic | 扩展违反执行契约 | 按既有 owner fault 边界处理，保留诊断 |

Animation 的 set_mode、surface suspension 和 behavior processing 均须逐入口核对。
processing 若返回布尔值，只表示本次处理是否产生变化，不能兼指成功与否。
remove 的 unknown／foreign 处理继续遵循其领域身份合同，不机械套用幂等 false。
健康状态检查必须先于同值短路，已 faulted 的同值请求仍返回错误。
纯配置读取不驱动动画、不修改 fault；需要 callback 的判断必须显式 fallible。

公开 `AnimationMut::set_mode` 使用 `Result<bool, AnimationError>`，
`Ok(false)` 表示健康状态下模式已满足。私有 surface suspension 也先检查健康；
返回值核算 suspension 服务配置变化，presentation damage 仍单独比较。
直接帧调用方显式传播领域错误到已有 `FramePreparationError::Animation`，
并保留其 Error source；不能把错误转为 Idle 或正常 Suspended。

behavior processing 是批次处理的例外：它的成功 bool 只描述 presentation 变化。
普通工厂或计划拒绝继续记录到已有有界 `AnimationBehaviorFailure` 队列，
不把这些拒绝提升为整个服务调用失败；已成功计划、失败诊断和已消费事实保留。
`Ok(false)` 可以伴随新增失败记录。fault 则在空 effects 等短路前返回领域错误；
工厂 panic 继续清理 presentation、标记 Runtime fault 并向原有捕获边界传播。
不增加整批模型或动画回滚承诺。

### Inspector 本地故障

events、clear_events 和 observer 入队只要发现事件缓冲锁 poison，
都先尝试发布共享实例的首 fault，再返回或传播对应的操作结果。
“首个”指首次成功完整发布的 fault 值，不按发现时间或调用开始时间推测。
操作类别与来源属于同一个胜出值：observer 入队路径成功发布时，
包含当前 record 的 Runtime namespace、epoch 和 sequence；
读取／清空路径成功发布时来源为空。
竞争失败或后续发现只读取既有值，不覆盖或补填来源。
首 fault 只发布一次，Clone 读取同一值，不使用最新 scene 伪造位置。

读取／清空返回的结构化 Inspector 错误同时描述本次失败操作与已保存首 fault，
不能报告为空或成功清空。不使用 poisoned guard 继续读写，不自动清除 poison。
历史快照是已保存的 owned 值，不证明故障后的 Inspector 仍在采集。
实例 fault 后 attach／attach_scoped 拒绝，且不消耗 listener ID 或留下新登记；
恢复使用新建的健康 Inspector，旧实例及其 Clone 不自动恢复。

Inspector 自己的缓冲区故障与一般用户 observer panic 分开处理：

1. 正常 observer 将 committed record 追加到有界 FIFO。
2. 入队发现本地 poison 时，尝试通过共同路径发布包含当前 record 来源的候选 fault。
   只有候选胜出时该来源才成为首 fault；否则读取已有值。
   当前 record 未入队的事实不改变首 fault 的既有操作或来源。
3. callback 请求 self-removal；当前 effect 的其他 observer 继续按注册顺序执行。
4. 宿主从 Inspector 状态入口读到 fault，并显式 detach、重建 Inspector 后再 attach。

本地 poison 处理不抛出 panic、不等待宿主恢复，也不改变 Runtime 的 source revision 或稳定场景。
已经 fault 的后续 callback 不再访问事件缓冲或入队，直接请求 self-removal。
真正的任意 observer panic 仍按 ADR-010／014 使 Runtime faulted；
不能借本地缓冲故障规则吞掉所有扩展 panic。
断采期间没有事件重放承诺；恢复按下节建立新快照／订阅边界，并标明时间线缺口。

单独 fault 状态须具有有界存储和不依赖事件锁的发布路径。若实施核对证明现有
ObservationListener 无法表达 self-removal，则先回到设计，不暗中增加第二套通知总线。

### 锁竞争与故障发布

本决策选择“不等待宿主恢复”，不承诺严格不等待锁或实时延迟上限。
健康状态下 observer 入队与宿主读取／清空可以为事件缓冲的短临界区等待；
正常竞争不是 poison，不因此丢弃 record、发布 fault 或产生时间线缺口。
事件锁不跨 Runtime 驱动、订阅／注销、快照捕获或宿主恢复步骤持有；
API 不公开可由宿主长期持有的缓冲 guard。

首 fault 使用独立、容量为一且不会 poison 的发布单元，值为固定诊断字段，
不调用用户格式化器或恢复回调。竞争发布只允许一个完整值胜出，
其余调用读取已发布值；不得循环等待宿主动作。
实现可以为该固定值的完整发布进行短暂同步，本决策不据此宣称 lock-free。
若实现需要新的可 poison 锁、无界分配或恢复等待，须回到设计复核。

SF-04／14 同时测试健康锁竞争和 poison：记录等待、入队及复制时间，
覆盖满／未满缓冲和最大允许容量。poison 后的停止采集会产生缺口，
健康锁竞争只影响耗时；两者的诊断不得混为一谈。

### 订阅所有权

优先复用 Observations 的 namespaced ListenerId、scope、remove 和 self-removal。
attach 返回明确的订阅 token，保存 Runtime 所有者和 ListenerId；不持有可变 Runtime。
宿主拥有 token，负责在销毁、断开会话或替换 Inspector 时通过对应 Observations 注销。
Figure-scoped attach 随 Figure dispose 自动注销，Runtime-scoped attach 必须显式配对。

detach 在登记实际存在并被移除时返回变化；不是仅因“第一次调用”就返回变化。
Figure dispose 或 self-removal 已移除登记时，第一次显式 detach 也返回未变化。
重复 detach 返回未变化；foreign token 返回身份错误，不能解释为本域未找到。
本批只修订 listener 的条款，不修改其他服务的合法缺失语义。
faulted Runtime 上的 detach 作为资源清理，允许移除订阅，不执行 callback 或稳定化；
一般注册和 mutation 仍拒绝 fault。

现状中 Runtime-scoped listener 简写会 expect scoped 版本的错误，
共享 listener ID 分配器也在 checked_add 耗尽时 panic。
因此各类 listener 的简写和 scoped 入口均需传播同一分配／准入失败，
在 ID 发布、listener 和 owner 表登记前完成预检。
两类入口的参数语义不同，可以保留，不能当同语义 alias 删除。
注销在检查 namespace 后移除登记；本域已缺失返回未变化。
fault 后清理不驱动回调、布局或事实重放。
同一机制用于内置及外部 observer，Inspector 不建立特权注销通道。

Drop 不能借用 Runtime，因此不声称 token Drop 会即时完成注销，也不在 Drop
执行提交或 panic。首期不增加共享取消表、全局注册表或带隐藏 Runtime 的 RAII guard。
只有真实消费者无法可靠配对、且既有 token 不能解决时，才另行评审 guard。
Inspector 本地故障导致 self-removal 后，token 仍可安全重复 detach。

不隐式限制现有多次 attach 或 Clone 共享行为。每个 token 独立携带 Runtime owner；
首个本地 fault 影响共享 Inspector 实例，后续回调不再入队并请求各自 self-removal。
首 fault 仅在 observer 入队候选成功发布时包含 Runtime namespace；
读取／清空胜出时来源为空，后续 observer 不补填。多 Runtime 的记录必须能识别来源，
只承诺各自 stream 内的 epoch／sequence，不建立跨 Runtime 全局顺序。
尚未收到新 record 的其他登记不会即时注销，宿主仍负责显式 detach。

拟议公共接口如下；类型名在实施前 API 核对时固定，行为不以更名放宽：

| 入口 | 参数与结果 | 失败与保留 |
|---|---|---|
| `FigureInspector::attach` | 借用 Inspector 与可变 Runtime；Runtime scope；`Result<InspectionSubscription, InspectorError>` | 共享实例 fault 拒绝；Core fault／分配耗尽保留在 registration 错误 source 中 |
| `FigureInspector::attach_scoped` | 上述参数加 `ListenerScope`；同一结果 | 另传播 foreign／stale Figure scope 拒绝；无登记时不返回 token |
| `InspectionSubscription::detach` | 借用 token 与可变 Runtime；`Result<bool, RuntimeMutationError>` | foreign 返回 Core 身份错误，token 保留，可向正确 owner 重试；fault 后允许清理 |
| `FigureInspector::events` | `Result<Vec<InspectionEvent>, InspectorError>` | 本次读取错误与首 fault 分开保存；不返回假空集合 |
| `FigureInspector::clear_events` | `Result<(), InspectorError>` | 发现 poison 先发布首 fault；不报告成功清空 |
| `FigureInspector::fault` | 返回首 fault 的只读值或固定 owned 副本 | 不接触事件锁、不驱动 Runtime，不把普通竞争报告为 fault |
| `FigureInspector::capture_tree` | 沿用 `Result<FigureTreeSnapshot, StableQueryError>` | 只读稳定查询；Core fault／未稳定仍拒绝，不由 Inspector 清除 |

InspectionSubscription 不实现 Clone／Copy，不包含 Runtime 借用；可移动，
读取 owner／ListenerId，不公开构造或伪造身份入口。
detach 不消耗 token，重复调用和失败后重试可通过同一个 token 完成。
FigureInspector 的 Clone 仍共享缓冲与首 fault，和 token 的所有权是两个独立合同。
正常多次 attach 仍各自生成独立 token；token Drop 不执行注销。

### 宿主恢复与事件边界

每个 Runtime 独立恢复。宿主先暂停输入、时钟推进和其他新驱动，
在整个衔接期间串行编排该 Runtime；到达的 backend 回执暂存为宿主任务，
完成衔接后仍通过正常 completion 协议处理。
checkpoint 不要求所有在途帧已经完成，也不要求创建或成功呈现一个 GPU 帧。
宿主在整个衔接期间保持 Runtime 的独占编排权，按以下顺序执行：

1. 对该 Runtime 显式 detach 旧实例的已知 token；已自动移除的登记视为未变化。
   foreign 失败不消耗 token，先回正确 owner 清理。Runtime 已 fault 时只做清理，
   后续稳定化不能在原 owner 上强行继续，须由宿主重建健康 Runtime。
2. 新实例尚未 attach。宿主使用 `Frames::stabilize_for_query()` 完成源状态与必需派生工作，
   并按下述 S09-B 增量完成稳定通知分发，取得健康、无待发布 journal、
   且不在分发阶段的 checkpoint。没有像素变化或 backend 回执延迟不阻止这一步。
   这是正常结清，不重放已送达或断采丢失的 record；其他现有 observer 仍按原规则执行。
   若不能结清或回调导致 fault，恢复在此停止。
3. 新建健康 Inspector，为需要的 scope 建立新 token；在不驱动 Runtime 的期间
   捕获稳定树快照，再次读取 checkpoint，必须与步骤 2 相同。
   attach 不发布场景变更或历史通知，新订阅不会收到步骤 2 已结清的旧 record。
4. 保存每个 Runtime 的恢复基线：namespace、稳定 epoch、最后已发布 record sequence、
   该 checkpoint 对应的树快照及 token scope。此后才恢复驱动。
   新时间线只包含该边界之后实际观察到的 record，不承诺收到所有通知 family 的每个 sequence。

上述 checkpoint 是 S09 的 Core Observations 基础增量，不能仅从现有 epoch 猜出：
拟议 `Observations::checkpoint() -> Result<ObservationCheckpoint, StableQueryError>`
返回 Runtime namespace、stable epoch 与 `Option<最后已发布 sequence>`，
该查询不 flush、不推进布局或动画。fault 返回 Faulted；
源／必需派生场景未稳定、待发布 journal 或正在分发时返回 NotStable。
仅有待绘制 damage 或在途 submission 不构成拒绝条件；
未来 completion 产生的事实在正常处理时属于后续边界的 record。
checkpoint 标明 stream 已发布边界，不恢复旧全树，也不创建通知回执队列。

**无 GPU 的结清路径。** S09-B 明确扩展现有公开驱动
`Frames::stabilize_for_query()`：稳定化成功后，在同一 Runtime 编排中将合法的
既有通知事实按正常顺序完整分发；不要求 surface、RenderSubmission 或 backend completion。
它不伪造 Painting／Prepared／Submitted 事实，不录制像素，不清除未绘制 damage，
不消费在途帧身份。单纯 checkpoint 查询仍然无副作用。
当前入口尚未提供这项完整分发保证，因此这里是待实施的行为增量，
不能以现有调用成功宣称 S09-B 已验收。

该驱动适用于 headless、无像素变化、Idle 和 Suspended；
backend 无法完成但 Runtime 尚健康时，场景／通知恢复也可以继续。
稳定化拒绝返回 `FramePreparationError`，普通 observer panic 沿用既有 fault 规则，
宿主停止此次恢复并报告，不无限等待 backend 或伪造回执成功。
实际 backend 失败到达后由正常 completion 路径决定是否 fault；
若已使 Runtime fault，checkpoint 返回 Faulted，宿主清理并重建 owner。
宿主若另有“快照必须对应已呈现帧”的产品需求，可独立等待特定回执并设置
失败／超时退出；这不是 Core checkpoint 的准入条件。

### 已发布游标

每个 Runtime 的通知 stream owner 持有一个持久的
`Option<最后完整分发结束的 record sequence>`。它描述正常通知分发已完成的前缀，
不是某个 Inspector 或每个 listener 的送达确认。
record 编号分配或开始 callback 不推进该游标；
该 record 的适用 callback 与当前 effect 的完整处理结束后才推进。
零 listener、scope 无匹配或本 effect 中发生 self-removal 时仍推进，
因为该 record 的正常分发处理已经结束。

读取／排空 journal、Inspector clear、attach／detach 和队列变空不重置游标。
同一 Runtime 内游标单调向前；None 只表示从未完整发布 record，
不表示当前队列为空。重建 Runtime 使用新的 namespace 与自己的初始游标。
listener ID 分配序号不是 record sequence，不能混用。

分发期间 checkpoint 返回 NotStable，不暴露 callback 只完成一部分的边界。
普通 observer panic 令 Runtime fault 后返回 Faulted；当前未完整分发的 record
不计入游标，之前完成的前缀保留，不承诺未送达事实重放。
在途帧之后的正常成功或失败回执若产生合法通知，随其实际分发推进游标；
没有通知事实时不人为制造新 sequence。
恢复的两次 checkpoint 比较完整字段：Runtime namespace、stable epoch、
最后完整发布 sequence，和每条 record 的来源身份使用同一命名域。

### 恢复失败与多 Runtime

attach、snapshot 或第二次 checkpoint 失败，或两次 checkpoint 不一致时，
宿主清理本次已经建立的全部新 token，丢弃未接受的新基线，保持暂停并报告失败。
多 scope 或多 Runtime 的部分建立成功也按各自 token 清理，不恢复驱动掩盖部分失败。
清理错误单独保留，不把恢复失败伪装为成功 detach。
没有新 record 时，读取／清空发现的首 fault 仍可触发这套恢复；
起始缺口没有 record 位置时明确标为未知，不补填 epoch／sequence。

多个 Runtime 各有独立 checkpoint 和恢复结果，不提供跨 Runtime 原子快照或全局顺序。
若宿主在步骤间违反暂停约束，checkpoint 差异拒绝基线；
步骤 4 后的正常变化及暂存回执的后续事实由新订阅处理。
SF-08／18／20 必须分别验证这两种情况。

### ADR-010 的局部替代关系

本决策已接受，局部替代 [ADR-010](adr-010-runtime-listener-lifecycle.md) 的以下条款。
listener 新合同的实现与验收归 S09-B，接受决策不表示该切片已完成：

| ADR-010 原条款 | ADR-057 的 listener 合同 |
|---|---|
| 第 1 节注册入口直接返回 ListenerId | 简写和 scoped 入口均返回领域 Result；健康／scope／分配预检后才登记 |
| 第 1 节未知或已移除 ID 返回 false | 本域合法缺失或已移除返回 `Ok(false)`；foreign 返回身份错误 |
| 错误与边界：重复注销返回 false | 返回 `Ok(false)`；fault 不阻止无 callback 注销清理 |
| 错误与边界：ID 溢出为不可恢复 invariant | 显式 listener u64 分配耗尽只拒绝本次注册，Runtime 保持健康，已有登记／owner／计数不变，已有订阅仍可观察或注销 |

耗尽不会 wraparound，也不会通过注销回收已使用的 sequence。
callback 的当前 effect 完整、自注销、历史 record／最新 stable query 分离、
普通 observer panic 的 fault 规则继续沿用 ADR-010／014。
上述变更不替代 notification record sequence 耗尽或其他 arena 的已有合同。

### 注册入口

对 S03 后的 Anchor、Router 和其他本批相关注册逐项记录：
输入、失败原因、返回身份、namespace、替换／注销和 fault 行为。
有失败可能的同语义入口只保留一个 fallible 的规范入口，删除 `.expect` panic alias。
规范入口使用领域动词 register，不用同一语义的 register／try_register 双入口。
已有 infallible 默认资源访问、不同语义的 probe／try 操作不按名称批量删除。

prepare 拒绝不增加注册记录、不消耗可观察 revision、不返回伪有效 ID。
耗尽的身份分配不得通过 wraparound 复用活身份；已有不可恢复 invariant 的策略
须单独记录，不能将其藏在普通输入拒绝中。
Core root、Facade 与 typed registry 边界继续遵循 ADR-033／035。

当前 Anchor／Router 的对象注册没有 validate 回调，真实失败准入是 Runtime fault。
不得为负向测试虚构输入拒绝或混入 route 阶段的错误。
listener 使用显式 u64 checked allocator，耗尽应在登记前结构化拒绝；
RuntimeArena 的通用 SlotMap 容量与代际策略另列现有底层限制，
不在本批重写所有 arena，也不宣称所有内存耗尽均可恢复。

### 扩展错误与 panic 诊断

Command、Policy 和本批调用链的 extension rejection 保留原始 Error source，
外层另存领域分类与操作位置。字符串仅用于显示，不代替 source；
如果需要 Clone，用共享 owned error 保留原因，不要求外部错误实现 Clone。
线程约束沿用所属扩展协议，不为显示错误额外要求 Send／Sync。

CommandError／PolicyError 当前的 Eq／PartialEq 不能比较任意外部 Error；
支持 owned source 后移除受影响类型的不真实相等承诺，
不按 message 或指针身份伪造原因相等。消费者改为断言分类、操作位置、
显式字段和 source downcast。既有文本构造保留，多层外部错误链继续传播原始原因。

panic payload 的策略独立于普通 Error：

- `String` 和 `&'static str` 保存原文本；
- 其他 payload 保存“非文本 payload”的结构化分类与调用位置，
  不 Debug 任意 Any，不调用用户格式化器；
- 不伪造 Error source，不承诺恢复任意 payload 或外部副作用；
- `panic=abort`、析构二次 panic 继续不承诺可恢复诊断。

Command／Policy／Tool 各自原有 commit、rollback、fault 和已提交前缀规则保持。
保留 source 不意味着应用模型在失败后可任意继续写入。

## 对标与合理变体

Draw2D／GEF 的观察与命令生命周期说明注册、注销和命令失败属于不同职责；
Java 同步异常和 Draw2D Animation 的静态调度模型不直接迁移。
Novadraw 使用显式 owner、短借用 Facade 和提交后 journal。
本批按切片映射既有语义 family：

| 切片／owner | `api_semantics` | 验证与回填 |
|---|---|---|
| S09-A／Core Animation | `animation.timeline`、`animation.presentation`、`frame.preparation` | SF-01／02／12／16 的模式、表现与帧错误传播 |
| S09-B／Core Observations | `notification.figure`、`notification.coordinate`、`notification.ancestor`、`notification.property`、`notification.action`、`notification.layout_update` | SF-05～08／17／18／20 的统一注册、注销、发布边界和普通 panic |
| S09-B／Inspector 与宿主 | 使用上行通知与稳定查询协议 | SF-03／04／06～08／14／18／20；poison、锁竞争与恢复属于本地／编排合同 |
| S09-C／Core Connections | `connection.anchor`、`connection.router` | SF-09 的真实注册准入、身份与 panic alias 删除 |
| S09-D／Editor | `command.protocol`、`command.stack`、`policy.protocol` | SF-10／11／12／19 的扩展 source、panic 诊断与既有事务边界 |

ID 均来自 [Draw2D 账本](../parity/draw2d/api-coverage.md)与
[GEF 账本](../parity/gef/api-coverage.md)；没有新增 `notification.observation` 等 family。
只回填实际变更与证据，不因本批通过关闭 Animation 的剩余平台验收。
Inspector poison 是 Rust 本地同步合同，不冒充 Draw2D 对等能力。

## 外部消费者证据

独立 crate 使用公开领域导入完成 Animation 同值／变化／fault 对照、
Inspector attach／observe／detach／重复 detach／foreign token、
faulted Runtime 的外部 Router 注册被拒绝且无登记或伪 ID；
另在健康或重建后的 Runtime 注册有效对象，以及带自定义 Error 的 Command／Policy。
换一个 Router 不会恢复同一个 faulted owner。
检查 source 可 downcast 回消费者自定义错误，普通拒绝与文本／非文本 panic 分离。
锁 poison 的私有注入验证与公开消费者的完整生命周期分别证明不同边界；
不为注入故障增加公共后门。

退出用例见 [S09 验证矩阵](../verification/plans/p2-s09-service-failure.md)。

## 性能、体积与实施切片

成功路径只增加错误分支检查；source 保存只在失败时分配。
Inspector 每条 record 维持一次有界入队，不扫描全树，不创建事件的历史树快照。
首个 fault 状态容量为一，不形成无限错误日志。
不引入 GUI、transport、新字体、默认 feature 或第三方依赖。

实施后比较相同 release 消费者的 Native／Wasm 原始与压缩产物；
记录不消费 Inspector、消费 Inspector 与消费动画 API 的可达成本。
测量每批 observer 分发、环形缓冲满／未满、动画同值／变化的 P50／P95、
分配与 record 复制次数。设计假设不能代替实际测量。

按 Animation 结果 → Inspector 生命周期 → 注册收口 → extension source 顺序实施。
每片完成定向验证，最终 API、workspace 和文档门禁通过后才关闭来源审计项。
2026-10-30 用户批准后启动 S09 Execute，首先实施 S09-A。

修订 1 将第二片明确为 Observations 基础改动与 Inspector 生命周期一起交付，
避免 Inspector 宣称已 detach 而 Core 仍在 fault 后保留登记。
S09-A 先覆盖 `set_mode`、私有 suspension／processing、直接帧调用方及 Animation
错误 source；不同时启动 Connections 与 Editor 改造。
