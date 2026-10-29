# ADR-057：服务失败与 Inspector 订阅生命周期

类型：`architecture-decision`

能力归属：跨包整改，Core／Editor／Inspector 分项负责；不属于单一 GEF 能力批次。

| owner | 本草案范围 | 验收归属 |
|---|---|---|
| Core（`novadraw`） | Animation、Anchor／Router 注册与 Observations 基础协议 | 服务结果、身份与故障契约 |
| Editor（`novadraw-editor`） | Command／Policy 扩展错误与调用位置 | 编辑事务边界及错误 source |
| Inspector（`novadraw-inspector`） | 本地 poison、首 fault 与订阅生命周期 | 观测与故障隔离 |
| 宿主／消费者 | attach／detach 配对和生命周期编排 | 各包公开 API 的接入证据 |

各包改动分别记录，不因 Editor／GEF 推进授权而启动 Core 或 Inspector 整改。

状态：草案，待评审；2026-10-29。对应 P2-S09，尚未批准实施。
执行状态见 [P2 backlog](../roadmap/p2-delta-backlog.md)。

## 问题与范围

R07、TC-10、API-F08 和 API-F10 指出，部分服务把故障解释为未变化或空集合。
API-F09 的资源读取整改已完成，本批仅承接残余 connection 注册别名。
CommandStack 的 check／has 分离已有完成证据，不重新列为缺口。

本决策从既有公开契约和审计证据定义目标，未扫描当前 Rust 实现。
实施前再核对 S03 迁移后的调用点，形成逐入口清单；历史行号不作为现状证据。

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

### Inspector 本地故障

events 和 clear_events 的锁 poison 返回结构化 Inspector 错误，包含操作类别。
不返回空集合、不使用 poisoned guard 继续读写，也不自动清除 poison。
历史快照是已保存的 owned 值；它不能证明故障后的 Inspector 仍在采集。

Inspector 自己的缓冲区故障与一般用户 observer panic 分开处理：

1. 正常 observer 将 committed record 追加到有界 FIFO。
2. 发现本地 poison 时，在不依赖该锁的单独状态中保存首个本地 fault；
   保存源 epoch／sequence，明确该条 record 未记录。
3. callback 请求 self-removal；当前 effect 的其他 observer 继续按注册顺序执行。
4. 宿主从 Inspector 状态入口读到 fault，并显式 detach、重建 Inspector 后再 attach。

该路径不抛出 panic、不阻塞 Core，也不改变 Runtime 的 source revision 或稳定场景。
真正的任意 observer panic 仍按 ADR-010／014 使 Runtime faulted；
不能借本地缓冲故障规则吞掉所有扩展 panic。
断采期间没有事件重放承诺；重连需重新捕获稳定快照，并标明时间线缺口。

单独 fault 状态须具有有界存储和不依赖事件锁的发布路径。若实施核对证明现有
ObservationListener 无法表达 self-removal，则先回到设计，不暗中增加第二套通知总线。

### 订阅所有权

优先复用 Observations 的 namespaced ListenerId、scope、remove 和 self-removal。
attach 返回明确的订阅 token，保存 Runtime 所有者和 ListenerId；不持有可变 Runtime。
宿主拥有 token，负责在销毁、断开会话或替换 Inspector 时通过对应 Observations 注销。
Figure-scoped attach 随 Figure dispose 自动注销，Runtime-scoped attach 必须显式配对。

detach 首次移除返回成功，重复 detach 返回未变化；foreign token 返回身份错误，
不能被解释为本域未找到。若当前 remove 对 foreign 的合同不同，应在评审中明确
修订范围，不修改其他服务的合法缺失语义。
faulted Runtime 上的 detach 作为资源清理，允许移除订阅，不执行 callback 或稳定化；
一般注册和 mutation 仍拒绝 fault。

Drop 不能借用 Runtime，因此不声称 token Drop 会即时完成注销，也不在 Drop
执行提交或 panic。首期不增加共享取消表、全局注册表或带隐藏 Runtime 的 RAII guard。
只有真实消费者无法可靠配对、且既有 token 不能解决时，才另行评审 guard。
Inspector 本地故障导致 self-removal 后，token 仍可安全重复 detach。

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

### 扩展错误与 panic 诊断

Command、Policy 和本批调用链的 extension rejection 保留原始 Error source，
外层另存领域分类与操作位置。字符串仅用于显示，不代替 source；
如果需要 Clone，用共享 owned error 保留原因，不要求外部错误实现 Clone。
线程约束沿用所属扩展协议，不为显示错误额外要求 Send／Sync。

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
本批 `api_semantics` 为 `notification.layout_update`、`notification.property`、
`command.protocol`、`command.stack`、`policy.protocol`；后三项属于 GEF 语义账本。
Inspector poison 是 Rust 本地同步合同，不冒充 Draw2D 对等能力。

## 外部消费者证据

独立 crate 使用公开领域导入完成 Animation 同值／变化／fault 对照、
Inspector attach／observe／detach／重复 detach／foreign token、
外部 Router 注册拒绝后有效重试，以及带自定义 Error 的 Command／Policy。
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
本草案不启动 S09 Execute。
