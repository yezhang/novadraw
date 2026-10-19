# ADR-037 草案首轮评审

类型：`architecture-review`

日期：2026-10-14

结论：**暂不通过。三个 P1 契约问题需修订后复核。**

评审对象：[ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)
及其[验证矩阵](../plans/p2-s06-incremental-failure-isolation.md)，均为本轮开始时的
未提交草案。本报告行号指向该草案快照，不表示引擎已经实现这些行为。
P2-S06 继续保持 `design-draft`，不进入 Execute，不关闭 R06/TC-08。

本轮核对现行 ADR、生命周期和路由规范，并复用机制必要性报告的已有运行证据。
未扫描本项目 Rust 实现，未新增运行时探针；以下问题是设计契约冲突或缺口，
不是新的实现缺陷证明。

## 阻塞问题

### S06-DR01：dispose 失败必须区分结构提交之前与之后

优先级：P1。草案 §3、§5、§6，原行 80–93、137–138、155–158。

草案要求清理失败后报告残留，并禁止重复清理，但没有定义一次 dispose 已经执行到
哪一步。现行 [Figure 生命周期 §3](../../design/architecture/figure-lifecycle.md#3-dispose-的准备与提交)
明确规定：prepare 失败无写入；engine commit 移出节点、解除登记并记录旧 damage，
期间不调用用户代码且不能发生可恢复失败；之后才完成 deactivation 和释放。

因此，生命周期 callback 在结构提交后 panic 时，FigureId 已失效，节点和相关登记
已经从引擎移除。不能将“回调未完成”解释成“结构仍待删除”，否则后续 cleanup 会
重复 dispose 旧 ID，可能用 stale 错误覆盖原始 panic，或误报结构残留。
反过来，prepare 拒绝时不能把清理目标标成已删除。

修订要求：

- 分别记录未提交删除、结构已退休、生命周期已尝试/完成的事实；不要求新增公共类型。
- 原始错误、结构残留与 callback 未完成分别报告；不靠单个 cleanup 成功标志推断。
- 保持 Core 的结构提交保证，不把 callback failure 改造成可恢复的中途结构失败。
- 对已尝试的 callback 不自动重试；Viewer 关闭时也不得重复调用。

验证：拆分 P04/O07，分别注入 dispose prepare 拒绝与结构退休后的 lifecycle panic；
检查旧 ID、registry、damage 和 callback 次数。失败后关闭 Viewer/Runtime，
再次核验没有重复 dispose/deactivate。panic 后是否还有存活资源按实际阶段断言。

### S06-DR02：Runtime 已 fault 时不能沿用健康路径的清理权限

优先级：P1。草案 §2、§3、§6，原行 68–70、81–90、157–163。

流程图将拒绝与 panic 合并到“清理新增资源”，正文又要求继续清理独立资源。
但 [ADR-014 决策 2](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md#决策-2原子性分层不承诺任意用户代码回滚)
规定：Core 扩展 panic 后 Runtime faulted，拒绝新提交。
若 panic 发生在 Figure activation、dispose completion 或 Core extension，
此时正常 mutation/Connections/Observations 清理入口可能已不可用。
仅把 Viewer 标成 fault，无法解决 Runtime 拒绝后续清理的问题。

修订要求：

- 区分 Viewer 扩展拒绝且 Runtime 健康、Viewer-only panic、Core 已 fault 三条路径。
- Runtime 健康时使用既有显式清理；清理导致 Core fault 后停止要求普通 mutation 成功。
- 对 Core fault，明确哪些引擎清理已完成、哪些资源只能随旧 Runtime 退出；
  未证明存在安全退出权限前，不承诺继续清除全部资源。
- 不为 cleanup 临时解除 Runtime fault，不授予 callback 可变 Runtime，也不自动重试。

验证：将 P05/C05 分别覆盖 Viewer-only panic 与 Core lifecycle panic；
断言 Runtime fault 状态、普通提交拒绝、已完成清理和残留诊断。
再覆盖“原始 Err 后清理 panic”的状态转换，不能只测最初的 callback panic。

这项修订先定义健康清理与 fault 退出的边界；若必须新增 Core 退出机制才能满足
毕业条件，应回到 Design 评估范围，不能在实现中隐式扩张 S06。

### S06-DR03：非法 route 不能仅凭错误原因升级为 Viewer fault

优先级：P1。草案 §4，原行 109–115；矩阵 C02/C06。

草案写明“非法路径值、绑定拒绝和初始化错误不属于”可恢复 unresolved。
现行 [Connection routing §4.1](../../design/architecture/connection-routing.md)
在校验 point 有限性、route geometry 与 locator 后，要求任一 route batch 预检失败
都提交整批 unresolved、清除旧 route，并保留恢复依赖；参见该文原行 124–135。
[ADR-014 决策 6](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md#决策-6失败状态和-session-顺序显式化)
也规定运行期路由失败提交稳定 unresolved 结果。

这意味着 Router 的非法输出可以成为 Runtime 的 unresolved 原因。
如果新建 Connection 已完成结构登记、Runtime 也成功发布了 unresolved，
Viewer 不能因为原因包含非法 geometry 就删除该连接并 fault。
按当前 C02 的笼统断言实现，会扩大 S06 并破坏已有恢复语义。

修订要求：

- 以 Runtime 操作结果及提交阶段分类，而非仅以错误名称分类。
- 未成功取得 binding/结构资源的 admission 拒绝，走新建失败清理。
- Runtime 已接受连接并提交 unresolved，保留 Part 与恢复依赖，Viewer 健康。
- Runtime 自身提交失败或扩展 panic，遵守现有 fault 规则。

验证：将 C02 拆成 binding admission 拒绝与 route 预检后成功提交 unresolved；
后者检查 Part 保留、points 清空、依赖保留，并在 Router 输入修正后恢复 resolved。
C06 保留一般 unresolved 回归，不能代替非法输出这一具体用例。

## 次级修订

### S06-DR04：清理顺序应分别约束 Editor 与 Figure

优先级：P2。草案原行 81、87；矩阵 P03。

“反向依赖顺序”需要指明对象和 callback 阶段。现行 Figure dispose 要求先提交
引擎结构，再按 descendant-before-parent 完成 Figure lifecycle；Editor Part/policy
由 Viewer 单独协调。不能用一个反向列表把 Figure lifecycle 提前到 Core commit 之前，
也不能因失败清理顺序改写所有正常 Part removal 的生命周期。

建议增加父子均已激活、子节点后续拒绝的调用序列断言，分别记录
Part/policy deactivate 和 Figure retire/lifecycle。每条路径检查次数与相对顺序。

### S06-DR05：故障用例不要暗含新增 fallible deactivate API

优先级：P2。矩阵 P04，以及草案原行 137。

[ADR-034](../../adr/adr-034-editor-tool-request-handle-protocol.md) 的
`EditPolicy::deactivate` 返回 `()`，不是 `Result`。
P04 的“deactivate 或 dispose 再次拒绝”应按实际协议拆开：
可失败的 prepare/cleanup 返回 Err；无 Result 的 deactivation 验证 panic。
不要为实现测试中的“拒绝”而修改已有生命周期 API。

fault 关闭回归还应区分停止新 `prepare_submission` 与处置已有 in-flight submission。
依照 [ADR-029](../../adr/adr-029-structured-render-backend-failure.md)、
ADR-014 的旧生产者退出与新 Runtime `Snapshot + Full` 基线规则验证，
不要把新帧拒绝扩大成关闭流程无法执行。

## 复核与执行边界

保留现有 Viewer/Runtime 所有权、三个实施切片，以及不预设 Plan/Batch 的决定。
上述问题不证明需要新抽象；修订重点是明确操作阶段、权限和结果分类。

下一步先修订 ADR-037 与矩阵，再逐项复核 DR01–DR03。
ADR 接受前不修改引擎，也不将“评审完成”记成 S06 已完成。
文档门禁只能验证链接和格式，不能证明上述故障保证已实现。
