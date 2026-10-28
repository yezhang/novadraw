# P2-S06 增量视图失败隔离验证计划

类型：`verification`

状态：`complete`，按修订 2 完成，限制见完成记录。

日期：2026-10-14

修订：2，2026-10-15 用户接受组合证据及 Native GPU 队列宿主关闭范围。
修订 1 的行为契约不变；修订 2 只调整下述证据方式和平台验收范围。

目标契约：[ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)。
本页是已接受的实施矩阵；执行结果须以独立证据记录为准。
基线缺口见[机制必要性复核](../reviews/p2-s06-mechanism-necessity-2026-10-14.md)。

## 1. 观测方法

使用 integration fixture 与独立外部 consumer；失败注入通过既有 factory、
behavior、policy、Router/binding 和 lifecycle 扩展点完成。只有公共契约无法观察
引擎登记时，才在对应内部测试读取登记；不得为测试新增公共 registry getter。

每个失败用例保存：

- 失败阶段、原始错误、Runtime fault 状态、cleanup 结果和存活资源身份；
- 模型 revision、CommandStack history/dirty、Viewer 接受的 revision 与 fault；
- 失败作用域内的 Part/Figure、复合 content pane、model/visual registry、
  policy/behavior、subscription、anchor、endpoint 和 Overlay owner；
- activation 完成、deactivation 已尝试/已完成的次数及调用顺序；
- dispose 尚未提交、结构已退休、callback 未完成的分阶段事实；
- 既有父节点、兄弟和连接的身份与资源；
- Runtime 通知、damage 及已删除 ID 的失效结果。

预检用例比对全部相关状态前后相同；创建后失败用例核验资源清理与真实通知，
不比对整个 scene 快照完全相同。计数器仅属于测试 fixture，不进入渲染热路径。
panic 用例只验证现有 unwind fault 边界，不声明覆盖进程 abort。
无 Result 的 `EditPolicy::deactivate` 只注入 panic，不为测试新增 fallible API。
Core fault 后断言普通 mutation 拒绝及隔离退出，不要求借普通提交继续清除全部资源。
prepare 拒绝的未退休资源与退休后未完成 callback 分开报告。

修订 2 的证据例外仅限 P04a/C05c/O07a：接受 Core 真实 dirty revision
耗尽测试、Editor 真实拒绝分派测试和三条清理调用路径的静态复核作为组合证明。
完整 Editor 计数耗尽集成未执行，不将组合证明记为集成测试。
任一连接耗尽后全局 disposal 预检均会拒绝；继续独立清理表示继续有权限的尝试，
不保证其他 Figure 删除成功。接受依据及可达性分析见
[剩余验收复核](../reviews/p2-s06-remaining-evidence-review-2026-10-15.md)。

F03b/F03c 的本批宿主门禁使用真实 Native/Winit/Vello GPU 队列退出证据。
retained texture 渲染不依赖 WindowServer drawable；不代表 surface 呈现
或 Web 异步队列退出通过。这两项继续登记为独立平台待验收项。

## 2. 故障矩阵

| ID | 注入或输入 | 必须验证 |
|---|---|---|
| S06-P01 | 新父创建成功，第二层后代 factory 拒绝 | 本次完整子树及复合 Figure 清除；既有兄弟不变；Viewer fault |
| S06-P02 | configure/refresh 在部分登记后拒绝 | Part/model/visual registry、订阅、selection/focus/capture 无本次残留 |
| S06-P03 | 父子均已激活，后续 policy/behavior activation 拒绝 | 成功前缀各尝试一次 deactivate；Part/policy 与 Figure retire/lifecycle 分别记录顺序，不改变正常 removal 规则 |
| S06-P04a | 原始创建 Err 后 dispose prepare 拒绝 | 该项结构尚在、ID 仍有效；Runtime 健康时继续独立清理；两个原因保留，不重试失败项 |
| S06-P04b | 原始创建 Err 后 Figure 退休，lifecycle panic | Figure 旧 ID 失效、相关引擎登记已清除，damage 保留；Core fault；不把 callback 未完成当结构残留 |
| S06-P04c | 显式清理后关闭旧 Viewer/Runtime | 已退休 Figure 不再 dispose，已尝试 callback 不再调用；失败资源按隔离退出规则释放，不依赖 Drop 回滚 |
| S06-P05a | Viewer configure/activation panic，Runtime 仍健康 | Viewer fault；不再驱动 panic 实例；其他已知资源只在现有权限内清理；不声称外部状态恢复 |
| S06-P05b | Core Figure activation panic | Core 与 Viewer fault；普通提交拒绝；按实际阶段记录已取得/已退休资源及隔离退出 |
| S06-P06 | 先成功同步一个兄弟，再创建另一子树失败 | 保留成功前缀；目标 Viewer revision 未接受；仅清理失败作用域 |
| S06-P07 | 重建后再次创建原模型对象 | 分配新 Part/Figure 身份；旧 ID 失效，无旧 registry 再利用 |
| S06-C01 | source anchor 已登记后 target 初始化拒绝 | 本次 anchor、endpoint、Part/Figure/registry 均清理 |
| S06-C02a | 两端已登记后 binding admission 拒绝，Runtime 健康 | 清理此前新增 anchor、Part/Figure/registry；不误清理既有端点与连接 |
| S06-C02b | Connection 已接受，非法 route 输出经 Runtime 预检后提交 unresolved | 保留 Part 与 anchor/dependency；该批 points/reservation 清空；其余同步成功时 Viewer 健康；Router 输入修正后恢复 resolved |
| S06-C03 | Connection activation 部分完成后拒绝 | 成功前缀各清理一次；与既有 activation 回归同一路径 |
| S06-C04 | self-loop 晚期失败 | 两端登记清除，不重复释放共享资源，不误删其他连接 |
| S06-C05a | Connection 的 Viewer-only callback panic | Viewer fault、Runtime 仍健康；只沿健康清理权限处理其他已知资源 |
| S06-C05b | 原始 Connection Err 后 Core cleanup lifecycle panic | Core fault；区分已退休结构与存活登记；普通 mutation 拒绝，不重复 callback |
| S06-C05c | Connection cleanup prepare 拒绝后关闭旧 owner | 保留原始与 prepare 错误、未退休身份；不自动重试，不误删其他连接或共享 anchor |
| S06-C06 | 合法 `UnresolvedConnection` | Part、anchor/dependency 保留，Viewer 健康；后续合法更新可恢复 |
| S06-C07 | 既有连接端点更新被候选校验拒绝 | 沿用既有单项提交契约，不以新建清理删除旧连接 |
| S06-O01 | 删除 `[a, a]` | 写入前稳定拒绝；a、owner、通知、damage 和生命周期计数不变 |
| S06-O02 | 删除 `[parent, child]` 及反向排列 | 均在写入前拒绝；不靠顺序避开祖孙重叠 |
| S06-O03 | foreign/stale/其他 owner 及包含其他 owner 后代的根 | 整批拒绝，合法前缀不被删除；错误指出输入位置及原因 |
| S06-O04 | 健康 Viewer 的空批次及合法互不重叠批次 | 空批次零变化；合法批次按既有生命周期清除全部目标 |
| S06-O05 | 新增批次后项 Figure/style/admission 非法 | 所有写入前拒绝，身份分配及 owner 无变化 |
| S06-O06 | 新增的安装 callback 晚期拒绝 | 清理该批新增资源，保留原始及清理错误；Viewer fault |
| S06-O07a | 删除批次后项 dispose prepare 拒绝 | 已删除前缀与未退休后项分别记录；Viewer fault；不恢复旧 ID |
| S06-O07b | 后项结构已退休，其 Figure lifecycle panic | 该项 Figure/owner 不再作为存活资源报告；Core fault；旧 ID 失效、damage 保留；关闭时不重复 callback |
| S06-O08 | 外部 typed Tool/Handle 操作上述 Overlay | 与内置实现共用 owner 校验、拒绝及清理路径 |
| S06-F01 | 模型命令成功后视图同步失败 | 成功 history 和模型 revision 保留；单独报告视图失败，不自动 undo |
| S06-F02 | 失败新建资源已经产生 attach/detach | 通知符合实际操作且无双重清理；damage 覆盖新旧区域；不倒退 epoch |
| S06-F03a | Viewer fault、Runtime 健康，已有 in-flight submission | 拒绝新输入/编辑/prepare；允许既有 completion 结清旧提交，不推进 Viewer 接受的模型 revision |
| S06-F03b | Core fault，已有 in-flight submission | completion 按既有规则返回 fault；宿主停止/排空或撤销旧提交，不解锁 Runtime |
| S06-F03c | 关闭旧 owner 并由新 Runtime 接管 backend | 旧生产者与资源退出后才接管；Snapshot + Full 建立基线；迟到旧 completion 被拒绝，重建成功后才恢复编辑 |

带 a/b/c 后缀的用例属于原 P04/P05/C02/C05/O07/F03 用例组，分别记录结果；
P04a/C05c/O07a 按修订 2 的组合证明执行，其他用例继续按既有注入或输入执行。
P04/C05/O07 的清理错误用例须区分可独立清理的资源和依赖失败步骤的资源。
测试不得把“继续清理”误写成“即使任意 cleanup 失败也保证无残留”。
P01/P02/C01/C03/C04/O06 的无残留断言以 Runtime 健康且显式清理完成为前提；
panic/prepare 拒绝变体分别由上述故障用例覆盖，不能静默跳过失败变体。

## 3. 现有门禁映射

| 用例 | 现有 suite | 用途 |
|---|---|---|
| P01–P07（含后缀）、F01、F03 | `g2.viewer-projection` | Part 生命周期、registry、同步与 Viewer fault |
| O01–O08、F02 | `g3.selection-targeting` | Overlay、Tool/Handle owner、输入及通知/damage |
| C01–C07 | `g5.1.connection-projection` | Connection 生命周期、关系、anchor 与 routing |
| 跨 Part/Connection 的 F01–F03（含后缀） | 上述受影响 suite | 结合各 fixture 断言失败结果，避免只验证一条创建路径 |
| F03b/F03c 的 Core completion 与接管协议 | `core.tc12-render-failure` | 沿用既有 fault acknowledgement 回归；S06 补充 Viewer 退出集成证据 |
| S06 增量实施与内部登记断言 | `editor.p2-s06-failure-isolation` | Core 批量准入、Editor 私有生命周期/registry 用例、G2/G3/G4/G5 与独立 consumer 的统一入口 |
| F03b/F03c 的 Native GPU 队列退出 | `editor.p2-s06-native-exit` | 真实 Winit/Vello 队列排空、旧 owner 释放、Snapshot + Full 接管及迟到 completion 拒绝；不代表 surface 呈现或 Web 异步队列验收 |

具体 integration target 和跨工具 suite 见 `tests/README.md`。
实现时把新增用例纳入对应 target；仅在现有 suite 无法表达新 target 时修改 manifest。
不预先注册空 suite，不把 baseline 或临时探针的 passed 当作整改通过。

## 4. 实施与关闭清单

- [x] ADR-037 经评审接受，回写 Editor architecture/G5.1 的保证范围。
- [x] 修订 1 按 S06-DR01–DR05 逐项复核；见修订 1 复核记录。
- [x] Part 反例先失败，再由最小清理修复通过；记录资源与生命周期证据。
- [x] Connection admission 清理与非法 route 转稳定 unresolved 分别覆盖；只修复用例证明的缺口。
- [x] Overlay 非法整批零写入与安装/删除 callback 晚期 panic 分别通过；按 Core fault 权限记录存活和退休身份。
- [x] 独立 consumer 不修改 Core，内置与扩展共用 API/错误/清理路径。
- [x] 模型/history、Viewer fault、notification/damage 断言通过。
- [x] dispose 分阶段、健康清理到 Core fault 的转换、关闭后不重复 callback，以及旧提交退出证据通过；P04a/C05c/O07a 为修订 2 组合证明。
- [x] 统一 S06 suite、quick、full 与文档门禁通过；完整故障矩阵仍按各项证据判定，不以全量回归代替未验收用例。
- [x] 更新规范、backlog、总清单和 completion record；按修订 2 实际证据关闭 R06/TC-08。

Design 阶段只执行文档门禁与 diff 检查；2026-10-15 接受设计，实施验证独立记录。

当前已通过的用例与限制见[实施证据](../reviews/p2-s06-implementation-evidence-2026-10-15.md)。
逐项组合证明与宿主关闭范围见
[剩余验收复核](../reviews/p2-s06-remaining-evidence-review-2026-10-15.md)。
用户已接受该调整，最终 full 门禁通过；
逐项证据和关闭结论见[完成记录](../reviews/p2-s06-failure-isolation-completion-2026-10-15.md)。
完整 prepare 拒绝集成、Web 异步退出与 surface 呈现未执行的限制不随本批关闭消失。
