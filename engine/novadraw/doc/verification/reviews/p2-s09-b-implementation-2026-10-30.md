# S09-B Observations 与 Inspector 生命周期实施记录

类型：`implementation-verification`

日期：2026-10-30

能力归属：Core（`novadraw`）拥有 listener 准入、注销、通知发布和无 GPU
稳定化协议；Inspector（`novadraw-inspector`）拥有诊断缓冲、首 fault 和订阅
token；宿主拥有暂停、重建与恢复编排。本片不增加 Editor 能力。

S09-B 的实现、公开消费者及自动门禁已完成，依据用户已接受的
[ADR-057 修订 3](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)。
成本证据覆盖容量 64／4096，理论最大容量未验收。
Connections 和 Editor 仍归 S09-C／D，S09 整批保持 `in_progress`。
样例与公开消费者见 [service-failure README](../../../tests/consumers/service-failure/README.md)。

## 实施结果

| 所有者／入口 | 结果 |
|---|---|
| Core 八类 listener 简写与 scoped 注册 | 统一返回 `Result<ListenerId, RuntimeMutationError>`；检查 owner 健康与 scope 后使用 checked allocator；耗尽不发布身份、不改变已有登记及 owner 表 |
| Core `Observations::remove_listener` | 返回 `Result<bool, RuntimeMutationError>`；foreign 为 `ForeignListener`，实际移除为 true，本域已缺失为 false；faulted Runtime 上允许清理，不驱动 callback |
| Core `NotificationRecord` | 显式携带 namespace；sequence 只在对应 Runtime 内排序 |
| Core `Observations::checkpoint` | 无副作用地查询 namespace、stable epoch 和最后完整发布 sequence；未稳定、有待分发 journal 或正在分发时拒绝，fault 为 `Faulted`；无已发布记录为 `None` |
| Core 发布游标 | 每个 effect 的全部 observer／typed listener 完成后推进；零 listener、无匹配和 self-removal 均可推进；普通 panic 保留完成前缀，不把当前分配号算作已发布 |
| Core `Frames::stabilize_for_query` | 应用待提交 mutation、稳定必需派生场景、处理 Animation behavior 后分发通知；不记录或完成 GPU 帧，不消耗 presentation damage |
| Inspector `events`／`clear_events` | 健康时等待缓冲锁；poison 返回结构化错误并发布首 fault，不恢复 poisoned guard 或用空集合代替错误 |
| Inspector 首 fault | `Arc<OnceLock<InspectorFault>>` 保存一个完整胜出值；操作和来源一起发布，Clone 共用；读取／清空发现的来源为 None，observer 发现的来源携带 namespace／epoch／sequence |
| Inspector observer | 已 fault 或发现 poison 时 self-remove，不抛出本地 panic；当前 effect 的其他 observer 继续；普通外部 panic 仍 fault Runtime |
| Inspector 订阅 | fallible attach 返回不可 Clone／Copy 的 token，保存 owner／listener／scope；detach 借用 token，可在 foreign 失败后向正确 owner 重试；Drop 不操作 Runtime |
| 宿主恢复 | 清理旧 token，公开 CPU 驱动，checkpoint，新实例／订阅，快照，再次 checkpoint；两次完整值相同才交付新基线，失败清理部分登记并保留清理错误 |

`checkpoint` 与原 `stable_query` 的条件不同：在途帧、仅有 damage 或 backend
资源同步不单独拒绝通知基线。Inspector 快照使用该通知基线，原 presentation
相关查询保留原条件。backend `Terminated` 回执本身不等于 CPU Runtime fault；
实际 owner fault 后查询拒绝，重建 Inspector 不能恢复这个 Runtime。

恢复消费者只有在独占 Runtime 编排、输入和回执保持暂停时成立。
它不承诺扩展外部副作用回滚、事件重放或多 Runtime 全局事务。
消费端保留 `RecoveryCause` 的 Error source；这些编排类型不进入 Core 公开 API。

## 行为验证

对应 [S09 服务失败矩阵](../plans/p2-s09-service-failure.md)的
SF-03～08／17／18／20，以及 SF-13～15 的本片部分。

| 范围 | 证据 |
|---|---|
| 私有 Core | [准入与普通 panic](../evidence/p2-s09-b-2026-10-30/core-private.log) 2 项；[游标](../evidence/p2-s09-b-2026-10-30/cursor.log) 2 项；八类简写及 scoped 耗尽后逐项比较计数器、八个登记数量和 owner 表 |
| 私有 Inspector | poison 读取／清空、observer self-remove、固定首 fault 候选竞争、真实读取与 observer 竞争、健康读锁／observer 等待、FIFO 不增长、多 Runtime 首值和新实例基线 |
| 公开宿主消费者 | 无 GPU／有 damage 查询、foreign 重试、Figure dispose、延迟在途回执、Idle／Suspended、旧 journal、scope 部分失败、快照失败、两次 checkpoint 变化／拒绝、清理错误、多 Runtime 部分成功、普通 callback panic、backend 回执与实际 Runtime fault 分离 |
| 编译失败 | token Clone、Copy 和旧 attach 直接消费 ListenerId 的三个 doctest |

Inspector 的[测试日志](../evidence/p2-s09-b-2026-10-30/inspector-tests.log)包含
9 项私有测试、2 项既有公开契约、9 项宿主恢复用例和 3 项 compile-fail；
成本探针默认忽略，不混入行为门禁统计。
独立 workspace 的[Inspector 配置](../evidence/p2-s09-b-2026-10-30/consumer-inspector.log)
通过 9 项宿主测试。相同宿主测试在 Inspector integration target 和独立
消费者中执行，不把两次执行计为两套独立场景。
[默认 Animation](../evidence/p2-s09-b-2026-10-30/consumer-default.log)、
[关闭默认 feature](../evidence/p2-s09-b-2026-10-30/consumer-off.log)保留回归。
两项 feature [组合配置](../evidence/p2-s09-b-2026-10-30/consumer-combined.log)
通过 13 项测试，[独立 Wasm 消费者](../evidence/p2-s09-b-2026-10-30/consumer-wasm.log)
check 通过。
注册失败清理的公开分支使用真实 foreign scope；数值耗尽通过 Core 私有注入
单独验证，不增加 Inspector 或 Core 公共故障 hook。

## 成本与体积

成本探针为 `crates/novadraw-inspector/src/cost.rs` 的显式 ignored release 测试，
只编进测试二进制；计数 allocator 仅统计测量线程的 allocation／reallocation。
使用 Rust 1.99.1 的 Cargo 默认 release 优化配置；计时包含 allocator 计数开销。
构造、清空、线程与 channel 创建、预热和样本容器不计入入队／复制计时。
每个场景五个会话、每会话采样 201 次。入队、复制、Runtime 分发和首 fault
先预热 20 次；健康锁竞争不另设预热。批量入队与单次复制分开，
读取计时包含返回 Vec 的复制和释放。
健康竞争分别测锁获取与实际 observer callback，另一个线程持锁并在收到释放
指令后继续持有 100 μs。ready／释放握手在计时外，计时包含随后调度与锁等待，
不能解释为引擎固有的 100 μs 延迟；锁获取与 callback 来自各自独立采样，
不能相减推算同一次 callback 的成本。

容量参数保留原来的 `NonZeroUsize`，引擎没有有限的全局容量上限。
本次覆盖宿主选择的 64 和 4096 容量、未满及满缓冲；4096 是本探针的最大配置，
不是引擎的“最大允许容量”。分配可能受地址空间和 allocator 限制，未尝试
`usize::MAX`，不据此声称最大容量或任意 payload 的成本已验收。
固定诊断首 fault 没有 heap payload；实际事件的复制成本仍取决于 effect。

构建与 workspace 回归全部结束后单独运行探针；
[原始日志](../evidence/p2-s09-b-2026-10-30/inspector-cost.log)和
[分位数摘要](../evidence/p2-s09-b-2026-10-30/cost-summary.json)保留五个会话。
下表为各会话分位数的中位数，单位 ns／采样；批次数据不等同于单条事件分位数。

| 场景 | 采样工作量 | P50 | P95 | 每会话测量分配 |
|---|---|---:|---:|---:|
| 容量 64 未满入队 | 32 条 | 708 | 750 | 0 |
| 容量 64 满缓冲入队 | 32 条 | 792 | 834 | 0 |
| 容量 4096 未满入队 | 256 条 | 5,500 | 5,625 | 0 |
| 容量 4096 满缓冲入队 | 256 条 | 6,625 | 6,958 | 0 |
| 读取并复制容量 64 缓冲 | 64 条 | 542 | 542 | 201 |
| 读取并复制容量 4096 缓冲 | 4096 条 | 36,042 | 40,375 | 201 |
| Runtime，无 Inspector | 32 次移动／64 条通知 | 37,875 | 39,333 | 51,459 |
| Runtime，附加 Inspector | 32 次移动／64 条通知 | 42,083 | 44,667 | 57,891 |
| 首 fault 发布：读取候选 | 一次固定值发布 | 41 | 42 | 0 |
| 首 fault 发布：observer 候选 | 一次固定值发布 | 41 | 42 | 0 |
| 健康持锁竞争 | 一次锁获取 | 114,834 | 129,292 | 0 |
| 健康持锁竞争 | 一次实际 observer callback | 114,125 | 131,000 | 0 |

固定 payload 的直接入队每条复制一次 effect，满缓冲先淘汰再复制，五个会话均
没有分配或队列容量增长。`events()` 每次为返回 Vec 分配一次，复制所有保留事件；
容量 64 每次分配 15,872 bytes，容量 4096 每次分配 1,015,808 bytes。
后者复制期间持有事件锁，因此容量选择同时影响读取耗时、临时内存和 observer 等待。

完整 Runtime 批次包含 mutation、稳定化、通知分发和通用 listener 登记整理。
附加 Inspector 后 P50 增加约 11.1%，每个会话比无 Inspector 多 6,432 次分配、
694,656 bytes。这组数据描述当前版本两种配置的差异；S09-A／B 的产物比较另以
相同源码和同配置实施。直接入队的零分配不代表整个 Runtime 分发链路零分配。
大树、多 observer、动态 payload、GPU 与产品首次加载不在这组微基准覆盖内。

同配置体积比较使用 Git `27dc2274`（S09-A 完成）隔离 worktree。
[固定源码](../evidence/p2-s09-b-2026-10-30/cost-probe.rs)在两边一致：
关闭 Inspector、依赖启用但未使用、实际 attach／frame 通知／读取三种变体。
Native 为 stripped cdylib，Wasm 为原始 module；release 使用 `opt-level=s`、
LTO、单 codegen unit、abort panic、strip，gzip 固定 `mtime=0`。
[脚本](../evidence/p2-s09-b-2026-10-30/measure-sizes.py)保存命令、工具链和 SHA-256。
该消费者不含产品 JavaScript 胶水、字体、图像或首次加载资源。

[产物收据](../evidence/p2-s09-b-2026-10-30/sizes.json)如下，单位 bytes：

| 变体／平台 | 基线原始 | 当前原始 | 基线 gzip | 当前 gzip |
|---|---:|---:|---:|---:|
| Inspector 关闭／Native | 1,295,936 | 1,295,936 | 609,613 | 609,869 |
| Inspector 关闭／Wasm | 1,201,117 | 1,201,118 | 426,026 | 426,068 |
| 依赖启用但未使用／Native | 1,295,936 | 1,295,936 | 609,553 | 609,892 |
| 依赖启用但未使用／Wasm | 1,201,121 | 1,201,111 | 426,033 | 426,066 |
| Inspector frame 消费者／Native | 2,041,712 | 2,041,728 | 1,000,730 | 1,001,968 |
| Inspector frame 消费者／Wasm | 2,149,890 | 2,148,744 | 776,974 | 777,060 |

实际消费 Inspector 的 Native 原始增加 16 bytes、gzip 增加 1,238 bytes；
Wasm 原始减少 1,146 bytes、gzip 增加 86 bytes。
启用但未使用的依赖可以被裁剪。文件布局和压缩变化不用于逐条指令归因。
没有新增第三方依赖、默认 feature、字体、图片或启动资源；
完整产品传输与首次加载时间不由这组 module／cdylib 数字推断。

## 最终门禁与审计回填

[workspace check](../evidence/p2-s09-b-2026-10-30/workspace-check.log)、
[Rust 1.96.0 严格 Clippy](../evidence/p2-s09-b-2026-10-30/workspace-clippy-1.96.log)、
[workspace test](../evidence/p2-s09-b-2026-10-30/workspace-tests.log)、
[独立消费者严格 Clippy](../evidence/p2-s09-b-2026-10-30/consumer-clippy.log)、
[Native／Wasm API](../evidence/p2-s09-b-2026-10-30/api.log)和
[文档门禁](../evidence/p2-s09-b-2026-10-30/docs.log)通过。
API 检查覆盖公开路径、第三方类型、依赖方向、保护热路径和 Runtime 28／28 方法快照。
最终 workspace 测试共 178 组汇总，1,774 项通过、0 失败、25 忽略；
忽略项包括本片的显式成本探针和既有专项，不改变其原验收边界。
[workspace 格式检查](../evidence/p2-s09-b-2026-10-30/fmt.log)与
[diff 空白检查](../evidence/p2-s09-b-2026-10-30/diff-check.log)通过。
沿用 S09-A 的工具链范围：Rust 1.99 的三项既有 Clippy lint 不属于本片整改，
不声称 1.99 workspace 严格 Clippy 通过。

API-F10 的 Inspector 静默同步错误由本片整改；R07／TC-10／API-F08 只回填
Animation／Inspector 和 listener 生命周期已完成部分，Connections alias
与 Editor 扩展 Error／panic 诊断待 S09-C／D，不提前关闭整项。
Draw2D 账本回填既有通知 family；poison 与宿主恢复是本地／编排合同，
不计作新增 Draw2D 对等能力，不影响 M01 平台剩余验收。

终端样例的[顺序验证日志](../evidence/p2-s09-b-2026-10-30/sample.log)：
x=10 时保存 sequence 5／6；detach 依次 true、false；x=20 时旧缓冲仍为 5／6；
重建基线 sequence=8 且新事件为空；x=30 时新事件为 9／10。
这是 headless 编排验收，不替代 Native Inspector 窗口的人工交互验收。
