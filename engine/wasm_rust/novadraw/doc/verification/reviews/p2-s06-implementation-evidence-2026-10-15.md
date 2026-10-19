# P2-S06 实施证据

类型：`verification`

日期：2026-10-15

状态：`complete`。目标契约为
[ADR-037](../../adr/adr-037-editor-incremental-failure-isolation.md)；
[验证矩阵修订 2](../plans/p2-s06-incremental-failure-isolation.md)已按用户接受的范围满足，
最终 full 通过。逐项证据、R06/TC-08 关闭结论及永久限制见
[完成记录](p2-s06-failure-isolation-completion-2026-10-15.md)。
以下按实施切片保留过程事实；各早期切片的未完成结论不代表当前状态。

已核验的增量：

- G2 反例首次运行时，新建后代 factory 拒绝后父 Part 仍在 registry。
  原因是增量同步调用递归创建入口，绕过单次创建清理。
- 初始与增量创建均已接入同一清理作用域；保留本次调用开始时的父级直接资源集合，
  只清理该次新增资源，保留其他同步成功前缀。
- 成功的 policy/behavior activation 记录进度，deactivation 调用前记录已尝试。
  清理拒绝与 panic 分别保留原始错误和后续失败；Core fault 停止普通清理。
- Overlay 删除入口已增加重复身份和拓扑重叠预检。
- 新增 `Runtime::add_figures` 即时批量入口，单项与批量共用 Figure 准入，
  capability descriptor 每项登记一次。Editor 在 Core 校验本批全部父节点、
  Figure 和 style 后安装；typed Handle 与反馈共用安装及故障记录。
- 已核验 configure/refresh/activation 拒绝、configure panic、Core attach panic、
  Figure 退休后的 detach panic、Overlay 后项退休 panic，及关闭不重复 callback。
- 旧 Viewer fault 且 Core 健康时允许已有 submission completion；Core fault 时
  completion 拒绝。新 Runtime 的首提交携带 Snapshot + Full，迟到旧 completion
  被拒绝；该测试不替代真实平台宿主的资源退出验收。
- 首次 S06 suite 在 Connection activation 回归失败：清理把独立 Connection
  Part 当成组合子树，`subtree_ids` 拒绝后跳过 binding removal，anchor 仍被占用。
  已按独立 Connection Part 作用域修正，原 activation 用例定向复测通过。
- Connection 登记前校验全部既有 dirty revision，避免 overflow 拒绝时留下 binding。
  内部测试以真实计数耗尽验证 admission 与 dispose prepare 拒绝均不写入结构或 damage。
- 新增 target provider 晚期拒绝、binding constraint 拒绝、Viewer activation panic
  和 self-loop 用例；健康 Runtime 下新增 Figure、Part 与 anchor 均完成清理，
  既有连接身份和端点保留。诊断分别记录存活与已退休的 Figure、Part、anchor，
  并保留嵌套作用域的清理诊断。
- 非法有限 route 输出在 Core 几何预检后转为 `InvalidGeometry` unresolved；
  清空无效几何，保留 Part、anchor 和 dependency，后续合法模型更新恢复 resolved。
  原合法 unresolved 用例也已补充恢复断言。
- policy deactivate panic 记录已尝试，继续独立 behavior/Figure 清理；关闭不重复回调。
  直接文本反馈清理不再忽略 dispose 失败；原始 configure panic 与文本反馈的
  detach panic 分别保留，旧 session 发出 Release，已退休反馈不再 dispose。
- F01 反例通过：模型命令成功后新 Part factory 拒绝，模型 revision、undo history
  与 dirty 状态保留，CommandStack 健康，Viewer fault 且不接受目标 revision。
- 独立外部 consumer 使用 typed Handle 创建 Overlay、由自定义 Tool 调用批量删除；
  重复身份返回同一 `InvalidOverlayBatch` 错误，随后合法批次仍可删除。
- Overlay 安装后项 attach panic 的用例通过：Core fault 后已挂载前缀保持存活，
  owner 尚未发布；报告真实身份，关闭不重复调用 detach，也不借健康 mutation 强行清除。

| Suite | 结果 | 已覆盖增量 |
|---|---|---|
| `g2.viewer-projection` | passed，24 tests | 新建父与后代拒绝清理、成功兄弟保留、目标 revision 不接受、关闭不重复 deactivate、重复/foreign/stale Overlay 不删除合法前缀 |
| `editor.p2-s06-failure-isolation` 的 Core 子项 | passed，3 tests | 整批拒绝无 topology/damage 写入；capability 单次登记；revision overflow 预检；真实 attach/detach 通知与 scoped subscription 退休 |
| 同 suite 的 Editor 内部子项 | passed，10 tests | 生命周期分阶段、拓扑/owner 预检、双反馈层准入、Overlay 安装 panic、policy/text cleanup panic、旧提交与重建基线 |
| 同 suite 的 G3 / G4 子项 | passed，8 / 19 tests | 既有交互与编辑回归；F01 模型/history 反例 |
| 同 suite 的 G5 子项 | passed，22 tests | 晚期 admission、self-loop、Viewer/Core panic、非法 route 与合法 unresolved 恢复 |
| 同 suite 的独立 consumer | passed，4 tests | 自定义 typed Tool/Handle 使用同一批量预检、错误与合法删除路径 |
| `editor.p2-s06-failure-isolation` | passed | 上述全部子项；最终日志 `verification/evidence/p2-s06-failure-isolation-final.log` |
| `full` | passed，exit 0 | workspace Clippy、全部 unit/integration/doctest、公开 API 与 Native/Web backend；日志 `verification/evidence/p2-s06-full-rerun.log` |
| 文档门禁 | passed | manifest、文档链接、索引分类、公开 API 探针；日志 `verification/evidence/p2-s06-docs.log` |
| diff 检查 | passed | 无空白错误 |

新增统一入口为 `editor.p2-s06-failure-isolation`。首次 suite 日志保存在
`verification/evidence/p2-s06-failure-isolation.log`；该次 suite 因 G5 失败未通过，
不得将已通过子项误记为整个 suite 通过。
中间复跑曾因测试 fixture 缺少 Tool 方法、返回身份元组未解构及局部 import
而编译失败，修正后最终复跑全部通过；中间日志保留为 `p2-s06-failure-isolation-r*.log`，
不作为 passed 证据。

此批 `quick` 门禁通过（exit 0）；日志为
`verification/evidence/p2-s06-quick.log`。它覆盖 workspace 编译、格式、
公开 API 类型与方法预算、分层依赖及 Native/Web backend 编译。
quick 本身不替代故障矩阵或最终 full；后续 full 结果见上表。
最终 S06 suite 共 90 tests 通过。首次 full 在 `workspace.clippy` 因内部反馈安装
四元组的 `type_complexity` 拒绝，日志保留为 `verification/evidence/p2-s06-full.log`。
已用私有类型别名修正，定向 Editor lib Clippy 通过后复跑 full，exit 0。
该修正没有新增公共类型或修改行为；full 同时覆盖既有 TC-12 提交恢复契约。
此前 quick 的通过记录属于较早切片，最终共享边界改动由此次 full 覆盖。

Core 的 dispose prepare 零写入证据已通过；它不代替 P04a/C05c/O07a 的
Editor 清理 prepare 拒绝集成证据。后者仍需核验原始错误与拒绝原因、未退休身份、
独立清理和关闭不重试。真实宿主的旧生产者退出与 backend 资源接管也未在本批验收。
上述切片结束时完整矩阵尚未关闭，R06/TC-08 当时保持开放。

## 后续剩余验收

同日补充后，`editor.p2-s06-failure-isolation` 共 92 tests 通过：
Core 子项 4 tests，Editor 内部子项 11 tests，其余子项数量不变。
日志为 `verification/evidence/p2-s06-failure-isolation-followup.log`。

新增 Core 用例从真实 dirty revision 的 `MAX - 1` 开始：
第一项 disposal 成功并消耗最后一次 revision，后项 prepare 拒绝，后项及连接仍存活；
拒绝不新增 topology/damage 写入。此时 Runtime 保持健康，但全局 disposal
预检同样拒绝其他 Figure，不能假定所有独立资源都可继续成功删除。

新增 Editor 用例使用真实 synthetic-root 拒绝，验证既有清理分派保留原始错误和
领域错误，继续处理前后独立 Figure，关闭不重复 callback。
synthetic root 不属于新建资源；该用例只证明拒绝分派，不替代
P04a/C05c/O07a 的完整 prepare 拒绝集成。

`editor.p2-s06-native-exit` 已通过。它使用真实 Winit window、Apple M1 Pro /
Metal renderer 和 GPU retained texture：Core attach panic 后停止新驱动，
旧 completion 返回 fault；有界排空 GPU 后释放旧 Viewer/Runtime，
保存模型与 CommandStack，新 Runtime 以 Snapshot + Full 接管；
迟到旧 completion 拒绝，基线完成后恢复编辑。日志为
`verification/evidence/p2-s06-native-exit-final.log`。

该结果证明 Native GPU 队列退出和资源接管，不证明 surface 呈现或 Web 异步退出。
正常 surface 提交返回 `Skipped` 的未通过日志、中间验收断言修正及证据方式调整
建议见[剩余验收复核](p2-s06-remaining-evidence-review-2026-10-15.md)。
上述后续切片结束时矩阵尚未变更，S06、R06/TC-08 当时仍保持开放。

本轮 Native bin Clippy、quick、文档门禁及 diff 检查通过。
日志分别为 `verification/evidence/p2-s06-native-clippy.log`、
`p2-s06-followup-quick.log` 和 `p2-s06-followup-docs.log`。
此前 full 的通过记录对应上一实施切片；该轮未关闭里程碑或提交 Git。
用户随后接受验收范围调整；最终关闭 full 门禁通过，
日志为 `verification/evidence/p2-s06-closure-full.log`。
状态回写后的文档门禁见 `verification/evidence/p2-s06-closure-docs.log`。
