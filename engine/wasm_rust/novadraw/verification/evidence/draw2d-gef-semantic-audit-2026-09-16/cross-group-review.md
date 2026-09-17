# Sep16 语义审计最终跨组校验

类型：`verification`

复核日期：2026-09-17。职责：bits-code-guard 通用工作流 Step 4 委派的最终跨组校验，不重新执行六组审计。

## 结论

- 读取全部 7 份分组 JSONL 和 `platform.jsonl`，共 17 条候选；保留 16 条快照 P1，Native 离窗 capture 原报告降为 P2/6 的待验证风险，不作为已复现的平台 P1。
- 17 条 JSONL 之间没有应删除的同根因重复；MD 中的重复表现归并到对应候选。没有新增缺陷，不按最终 5 张卡片上限截断本清单。
- 当前整改状态单独采用[整改 SSOT](../../../doc/verification/reviews/draw2d-gef-semantic-remediation-2026-09-16.md)：写入前复读为 **5 项已关闭、12 项待关闭**。快照评级调整不改写该账本的历史 17 条口径。
- “待关闭”仅表示 SSOT 尚无关闭记录，不断言并发修改后的当前产品仍存在原问题；本次没有复验修复代码或运行任何 cargo、探针、GUI、测试。

## 范围与证据身份

已按顺序读取 `AGENTS.md`、`CLAUDE.md`，读取项目记忆、bits-code-guard 技能及通用工作流、评级、七维规则。本次仅复核现有证据，无产品或第三方源码新扫描。参考语义仅采纳分组报告中 Draw2D/GEF 官方证据，不采用 Zest。

- `SKILL_ROOT`：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- `REPO_ROOT`：`/Users/bytedance/Documents/code/GitHub/drawjs`；项目根为其下 `engine/wasm_rust/novadraw`。
- `WORK_DIR`：本文件所在 `verification/evidence/draw2d-gef-semantic-audit-2026-09-16`，也是本次唯一产物目录。
- 输入：[reviewer-brief.md](reviewer-brief.md)、[review_groups.md](review_groups.md)、全部 `group/*.jsonl` 及对应 MD、[platform.jsonl](platform.jsonl)、探针源码/元数据/日志、Web 构建日志和 additional-checks 元数据。
- 本次是 `full_file` 历史证据复核，不按 diff 过滤。JSONL 的 `file` 相对 Git 根；下表位置为 Novadraw 相对路径，**全部行号只指 Sep16 分组快照，不链接当前源码行**。
- `baseline.json` 记录 HEAD `e8d54ac063d05c63abb9f2850365a837ec4e8c9d` 和脏工作区；不能把 HEAD 等同于完整审计内容。16 条分组候选的行号均落在 baseline 记录的对应文件行数内；平台文件不在该 inventory 中，其 950-953 行由 `web.build.log` 编译器诊断直接支持。这不是当前源码行号校验。
- event 与 editor 探针记录的 scene rlib 哈希分别以 `406e61d6`、`d5b2a577` 开头，不能宣称两次运行来自同一二进制或同一原子工作区快照。各自结果按各自元数据使用。
- event 探针元数据引用 Sep15 的源文件，脚本及 build/exit=0 记录支持 Sep16 重新编译执行；复用源文件不等于沿用旧二进制结果。本次未追读已迁移的历史源路径。日志中的旧 doc 路径只是当时记录，不是现行产物位置。

## 逐项裁定

置信度表示对原快照结论的把握，不代表当前仍未修复。“静态”表示分组已有完整函数、直接调用方及具体输入推导，不能改写为本次运行复现。除明确调整外保留原等级。

| 候选与原快照位置 | 裁定 | 证据边界与跨组判断 | 当前状态（SSOT） |
|---|---|---|---|
| F-EVENT，`novadraw-scene/src/runtime/event/mod.rs:377-410` | 保留 P1/10 | `event-probe.log` 实证交互父 P/无 handler 子 C：首次只有 Moved，C 到 P 空白区多出 Entered。MouseHover、capture 下交叉节点属于同根因静态范围，日志未逐项复现。 | 已关闭 |
| G1-S1，`novadraw-scene/src/runtime/runtime.rs:2485-2488` | 保留 P1/10 | 关闭 debug assertions 时 scope 写入不执行，dispose 无 owner 索引可退订；八类注册合并一项。SSOT 另有修复前 FAIL、修复后 PASS 的 release 定向验证记录，不由原 debug 门禁推断。 | 已关闭 |
| G1-S2，`novadraw-apps/src/app.rs:488-495` | 降为待验证 P2/6，原 P1/9 | 给定 exit/release/reenter 顺序可静态推出残留；原证据没有 macOS 实际事件投递/capture 顺序。部分依赖外部平台契约，置信度减 3；详见下节。 | 已关闭；原平台复现局限仍保留 |
| G2-F1，`novadraw-scene/src/runtime/update/repair.rs:116-119` | 保留 P1/10 | 静态：溢出 child 从 `(60,0)` 移到 `(80,0)`，erase 以父为 dirty owner，起点 visual clip 丢掉旧区域。限 retained partial 且无附带全重绘；不是所有后端都必见残影。 | 待关闭 |
| G2-F2，`novadraw-scene/src/graph/mod.rs:4162-4164` | 保留 P1/10 | 静态：100x100、inset=10、StackLayout 将 node-local client origin 当 child-content origin，再经 child transform 加一次 inset。不扩张为所有 scale/viewport 组合已复现。 | 待关闭 |
| G2-F3，`novadraw-scene/src/layout/border_layout.rs:279-287` | 保留 P1/10 | 静态：200 高、South=150，Center 预留到 50，South 实摆高 100，出现 50 空带；East 同模式。接受固定厚度策略也不能消除内部不一致。 | 待关闭 |
| G3-F01，`novadraw-scene/src/graph/mod.rs:3061-3063` | 保留 P1/10 | 静态闭环：Ready 图片 remove -> UnknownResource 被跳过 -> Figure 继续旧引用 -> Vello Remove 后缺资源 -> Retry。限定可见 Figure 继续生成旧命令；未做 GPU 复现，不推为所有 backend 行为。 | 待关闭 |
| G3-F02，`novadraw-scene/src/figure/border/compound_border.rs:92-98` | 保留 P1/10 | 静态：合法 Compound+TitleBar 缺 owner-scoped 子快照，动态 inset、背景和 glyph 均丢失；普通 Compound 测试、直接 TitleBar 测试不能互相替代。 | 待关闭 |
| G3-F03，`novadraw-scene/src/figure/label.rs:581-589` | 保留 P1/10 | 静态：North/South 与 text 相对 icon 的 enum 语义相反，presentation 被 paint 直接消费且 demo 已使用。无截图，不另声称字体像素错误；保留为明确方向功能错误。 | 待关闭 |
| G4-F1，`novadraw-scene/src/connection/runtime.rs:650-658` | 保留 P1/10 | 静态：首次零长 Direct route 被 PathFractionLocator 拒绝后 observations 未合并，移动 owner 无反向依赖可命中。计算失败和预检失败不是同一分支；不重报已修复的提前提交 Resolved。 | 待关闭 |
| G4-F2，`novadraw-scene/src/connection/router.rs:428-436` | 保留 P1/10 | 静态代入 A=(0,0)、B=(100,0)、separation=16：反向连接 index 与法向同时翻转，中点均 `(50,-8)`。无向分组本身正确；不是 Editor 顺序问题。 | 待关闭 |
| G4-F3，`novadraw-scene/src/runtime/runtime.rs:1638-1643` | 保留 P1/10 | 静态：Connection 自身换到平移 100 的 parent，absolute bendpoint 从 surface 50 跳到 150。依据本地已接受的“迁移或原子拒绝”契约；不是 Java 原生事务承诺，也不是 owner reparent 用例。 | 待关闭 |
| G5-F01，`novadraw-editor/src/viewer/mod.rs:2691-2695` | 保留 P1/10 | `editor-probe.log` 实证 `activated=[1] deactivated=[]`。探针记录回调、不真的注册外部 listener；遗漏停用已复现，订阅残留是遵循生命周期契约的调用方后果。与 G1-S1 不同根因。 | 待关闭 |
| G5-F02，`novadraw-editor/src/viewer/mod.rs:2174-2181` | 保留 P1/10 | 静态操作计数：逐边 reorder 校验复制/扫描 E 项，另逐边 resolve 重建 routing order，构成平方级工作量，违反批次线性契约。没有耗时/卡顿实测；不能用 adjacency 增量修复关闭此项。 | 待关闭 |
| G5-F03，`novadraw-editor/src/viewer/mod.rs:1768-1779` | 保留 P1，9 提升至 10 | `editor_probe.rs` 先改 bounds 再 panic；日志实证外层捕获后 `viewer_faulted=false`。revision 未提交及后续 refresh 行为仍为静态边界，探针没有直接断言它们。不提升 P0，不宣称正常 demo 必崩溃。 | 待关闭 |
| G6-F1，`novadraw-editor/src/viewer/mod.rs:1351-1355` | 保留 P1/10 | 探针实证 target=model 3、实际 command host=model 2。command 回调返回 None，未执行错误模型命令；证明的是错误归属路由。feedback 与 foreign/stale 接受属于同根因静态证据，未分别运行。 | 已关闭 |
| PLATFORM-WEB，`apps/web/web-validation/src/lib.rs:950-953` | 保留 P1/10，校正调用名 | `web.build.log` 实证 E0308/退出 101。实际是 `dispatch_scroll/dispatch_zoom` 的 match 返回 DispatchOutcome 而要求 `()`；原 JSONL 写 `dispatch_wheel_changed` 不准确，但不影响构建失败成立。 | 已关闭 |

## Native Capture 专项裁定

原增补证据证明的是：如果 Native 先清掉 cursor_position、未终止 capture，并在结束事件得到处理之前重入，后续 move 会被判 Dragged。它没有证明 macOS/winit 在真实捕获状态下一定产生这条顺序，也没有窗口系统复现记录。不能把确定的条件分支推导等同于已经确认的平台触发。

因此原快照项保留为 **P2/6 待验证风险，需结合业务场景确认**，不是直接删除为误报；不确定性主要在平台可达性而非残留状态下的引擎行为。若后续真实记录确认该序列，可恢复 P1；原快照报告不可倒填复现结果。

写入前 SSOT 已将该项关闭，记录 pointer leave 清 capture/pressed、向原 mouseTarget 发送一次 Exited，并引用 `pointer_exit_clears_capture_and_exits_the_mouse_target`、`event-app:pointer_leave_cleanup` 为 PASS。本报告接受该整改状态，但只转述账本，不声称亲自执行，也不将其外推为 macOS 离窗/失焦的全部原生输入序列均已人工验收。失焦路径的原描述同样不具备独立平台复现，不再拆成新 P1。

## 共享契约与去重

| 跨组边界 | 最终判断 |
|---|---|
| Runtime dispatch 返回值 -> Native/Web adapters | Web 是已证实的直接调用方返回类型未同步；host quality 通过不覆盖 wasm32 cfg。保留平台项，修复显式丢弃 outcome 不应被改写为已经实现新的浏览器默认事件策略。 |
| EventDispatcher -> InteractionState -> widget/tooltip -> Editor press 仲裁 | F-EVENT 负责事件接收者与几何来源混用，G1-S2 负责平台结束事件/清理闭环，触发和修复责任不同，不能互相去重。G3 widget 正常拖出拖回、G6 press 仲裁成立，不足以证明两项无缺陷。 |
| Runtime scoped listeners -> UpdateManager retirement；Viewer -> Part lifecycle | G1-S1 是 build-profile 下 owner 登记缺失；G5-F01 是初始化失败未遍历已激活 Part。虽都可能残留订阅，不能合并成“生命周期泄漏”后少算一项。八类 scoped listener 只算 G1-S1 一项。 |
| LayoutContext -> LayoutSnapshot -> LayoutOutput -> child transform/paint | G2-F2 在查询边界，G3 的 paint transform 顺序成立不构成反证；不应逐布局或渲染主循环补偿。G2-F1 的旧 damage 起点裁剪另有根因，不能以“坐标问题”合并。 |
| Border snapshot -> layout client metrics -> Compound paint | G3-F02 与 G2-F2 可以叠加，但一个缺动态子快照、一个返回错误坐标域，分别保留。普通静态边框正确不能证动态组合正确。 |
| ResourceRegistry -> ImageFigure snapshot -> Vello submission -> Retry | G3-F01 属跨层失效遗漏；不能以各层单独 Remove/Retry 测试通过关闭，也不另报一个 backend Retry 缺陷。 |
| Label presentation -> icon named geometry -> LabelAnchor | G3-F03 可影响派生 icon 位置；G4 的 named-region 查询/跟踪正确不意味着输入位置正确。此传播归 G3-F03，不另造 Anchor 算法缺陷。 |
| SceneQuery observations -> route preflight -> dependency invalidation；Editor resolve | G4-F1 与已落地三阶段提交兼容，缺的是失败恢复依赖。Editor 显式 resolve 可能遮住反例，不否定仅 Runtime mutation + prepare_frame 的公共路径。 |
| Viewer connection reconciliation -> Runtime reorder/resolve -> PartTree adjacency | G5-F02 保留两个逐边全局工作来源，不把已修复的全表 adjacency 重建再报一次。G4 的 Fan 偏移、reparent 域迁移各自独立。 |
| Policy target -> PartId registry -> command/feedback host | G6-F1 是身份结果被布尔化，command/feedback 归同项。Group 5 的 namespace/registry 校验存在，不代表这里实际消费了校验；只换 host 而沿用源 policy 也不满足归属契约。 |
| CommandStack/Runtime guard -> Viewer projection -> Domain 调用方 | G5-F03 的 Viewer fault 缺口不能由底层 guard 或 CommandStack guard 覆盖。G6 引用同一 projection panic 归并此项；未复现的任意 Tool/Policy callback panic 不据此扩成全域确认缺陷。 |

MD 重复描述仅作为支撑合并：Group 1 supplement 的 F-EVENT 复述、Group 6 的初始化失败/panic、平台失败在其他组的转述均不新增计数。三个独立路由问题、两个独立生命周期问题及各布局/绘制问题不能为了卡片限额强行合并。当前 17 条 JSONL 不删除任何一条原始记录。

## 误报排除与未决差异

不把“不同于 Java”自动判 bug：disabled 普通几何命中门禁、remove=dispose、严格 viewport chain、Locator 保持现有 child 尺寸、单 primary pointer，以及明确后置的 persistence/connection children 等，按各组的批准收窄或后置记录保留。`event-probe.log` 中 disabled hit 不一致是差异实证，不是新增偶然实现缺陷。

不把已落地的三阶段路由、Runtime Locator 接入、稳定 Anchor descriptor、增量 adjacency 重新报为缺失。G5-F03 是项目显式增强的 fault 契约缺失，而非要求 Java 提供同等保证。

分组限额之外的真实证据边界也不能消失，继续由原 MD 追踪，**本次不新增定级或确认为已修复**：

- Group 2：layout 外框测量/inset、XY/Freeform/Toolbar/Viewport hints、Grid equal/grab/压缩/换行、布局 output 数值校验、validation budget 与有限大批输入。
- Group 3：公开 LineStyle 被后端忽略、Path Arc lowering/方向参数、外部动态 Border snapshot 能力；不能全部用“高级能力后置”覆盖已暴露的静默行为。
- Group 4：真实 rounded shape 自动供给、Manhattan 完整法向分支、跨 layer binding 归属、跨连接依赖环、decoration reference/orientation 消费范围。
- Group 5/6：动态内部 visual 更新、activation 注释与实际阶段、外部 Tool/Request/selection/root 注入、统一 key/focus/cancel、拖动中 modifier、回到阈值内 feedback。

现有 passing suites/replay 不覆盖这些所有组合。日志中 editor panic 是探针故意触发并捕获，元数据 exit=0 表示反例探针完成，不是产品正确或 panic 已修复；其反向断言不能原样成为正确性回归期望。

## 交接

本报告是保留全部差异的 Step 4 证据，不替代主审的最终人读报告、快照差异说明或整改 SSOT。主审最终卡片可按技能最多展示 5 条，但需引用本表全部 17 条裁定及各组未决映射；展示未入选不等于丢弃或关闭。

当前已关闭的五项分别是 F-EVENT、G1-S1、G1-S2、G6-F1、PLATFORM-WEB；其余 12 项仍按 SSOT 待关闭。并发后续更新以 SSOT 为准。没有改产品/测试、原始 JSONL 或 SSOT；仅新增本文件，未生成额外 HTML、未运行 cargo 或重新执行六组扫描。
