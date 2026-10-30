# Editor 容器编辑验证矩阵

类型：`verification-plan`

能力归属：Editor（`novadraw-editor`），验收目标为 GEF 容器编辑行为。
Core revision 耗尽属于基础依赖故障，验证其进入 Tool 后的事务与清理行为；
Native／Web capture、DPI 与取消属于平台／宿主输入链路验收。
业务模型与 Command 由应用消费者提供，各层证据分别记录。

日期：2026-10-29。修订 2 已获批准，无窗口实施与回归已执行；
Native/Web 实际输入、跨显示器及窗口外 capture 仍待验收。
2026-10-30 已补隔离源码副本中的真实 revision 耗尽 Tool 链路、
Web DPR 1／2 输入与 Native Retina 的部分输入组合，
见[补验记录](../reviews/editor-container-followup-2026-10-30.md)。
macOS 两种 Tool 的窗口外释放已通过；跨显示器、系统 DnD 与完整组合仍保留未完成。
目标契约：[ADR-055](../../adr/adr-055-container-editing-protocol.md)。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 观测与夹具

独立模型包含根、两层容器、不同 content pane、节点和跨容器连接。
每次操作记录模型 revision、父/child 顺序与约束、ModelId/PartId/FigureId、
selection/focus、feedback owner、history/dirty 和 fault。
所有拒绝场景比较前后模型与历史，不用截图替代事务事实。
本轮按[首次评审](../reviews/adr-055-draft-review-2026-10-29.md)补齐断言。
修订 2 再补[修订 1 复核](../reviews/adr-055-revision-1-review-2026-10-29.md)
的 R1-01～03，保留其余已解决合同的验证范围。
本轮续验映射见[契约与成本复核](../reviews/adr-055-followup-2026-10-29.md)；
提前退休反馈的清理准入拒绝不替代 Core revision 耗尽的端到端场景。
这些场景是退出条件。实际覆盖与尚缺证据见
[实施验证记录](../reviews/adr-055-implementation-2026-10-29.md)，不得由单项正例推断全矩阵完成。

| ID | 输入 | 必须验证 |
|---|---|---|
| CE-01 | 祖孙混选、后代 primary、两种选择顺序、选择连接 | 操作集合去除后代且不含连接；完整选择快照独立，保留 primary 与原顺序 |
| CE-02 | duplicate、foreign、retired source/target；第三方 header 错身份、目标或父 | host/真实 source 全批校验；拒绝错误 header，不执行合法前缀，不产生历史 |
| CE-03 | 源节点覆盖目标、Handle/Feedback/连接层覆盖落点 | 排除源子树与非容器层；按 Core 命中找到真实容器 |
| CE-04 | 内层普通 handler 重定向到父；内层无 handler/明确拒绝 | 普通 dispatch 保持原规则；aggregate_at 不调用 target callback，内层拒绝不被父接纳覆盖；只有无贡献探测祖先 |
| CE-05 | 拖入自身或后代 | cycle 被拒绝；模型、投影和 history 不变 |
| CE-06 | 不同 inset/content pane、DPI、源与目标相同旋转、反射、轴交换；多选异源父变换 | 比较各项 `T⁻¹ · D · Sᵢ` 约束角点；同一批次共用 D；声明容差下映射/规范化与反馈提交一致 |
| CE-07 | 数学奇异、极小可逆尺度、不可表示旋转/skew、非有限输入及派生溢出、零尺寸 | 奇异与计算错误区分；有限但溢出不能成为无候选；自定义约束不能输出非有限成功值 |
| CE-08 | A→B 与 B→B 混合；A、C→B；同父 XY 多选；非 primary 按下、被覆盖后代、反向选择、程序化/零尺寸锚点 | 断言唯一 anchor/D、确定选择规则及 XY 间距保持；G 不因零尺寸奇异；程序化缺失起点/无效锚点拒绝；资格精确分组、单 history entry |
| CE-09 | 首尾、空目标、反向选择非连续 child、Before/After、本批 source reference、无变化 | 移除本地 source 后插入完整操作序列，无重复 child；无变化无历史；source reference 明确拒绝 |
| CE-10 | 缺项、重复项、错目标；另一父/revision/generation/候选/活动协议的回执；资格 metadata 与 query 子批不符 | 校验 provenance、精确子批与覆盖；多个 contribution 在 reducer 前拒绝；资格不足时计划不受检、不发布反馈；任一必需资格失败全批不执行 |
| CE-11 | 有 inset/scale 的嵌套点击、拖动与 drop，第三方模板 | 创建独立输入、不查询移出；默认尺寸为父域，drop 显式偏移；空移动拒绝；取消/拒绝无模型对象或历史 |
| CE-12 | 换父、undo/redo、后代 primary、非选中 focus、选中连接 | 退休旧 Part；execute 恢复完整快照；undo/redo 恢复各次操作前当前选择；缺失 primary 取最后一项；不自动恢复 Figure keyboard focus |
| CE-13 | 外部 revision、删除、换 Tool、失焦；policy 换装/配置失效 | 整段取消、释放 capture；外部删除 reconcile 生效，不复活对象；不提交旧资格 |
| CE-14 | move/release 位置不同，release 前视口变化；多选静止指针下 scroll/zoom/resize/双轴 auto-expose | 每次更新只求一次锚点 D，全体 source 共用；release 最新位置重算，各项 Sᵢ/T 映射正确；不跳 Viewer |
| CE-15 | 会话同步发现新 revision；模型失败、投影失败；反馈 prepare 拒绝/lifecycle panic | 清理失败阻止 build/execute；保留原因与进度；成功命令 revision 不触发外部取消，成功 history 不回滚 |
| CE-16 | 两个不同 Request/Payload/Plan 消费者；同 Viewer XY→Ordered 共同活动 P；只注册另一 family 的容器 | 分别证明泛型开放与同手势跨布局；真实注册 CheckContainerPermission，公共 Tool 查询 host/kind/精确子批并签发回执；不兼容 family 为未支持；不得直接构造回执或另写 Tool 代替 |
| CE-17 | 原平面 SelectionTool、CreationTool、模板与连接回归 | G1–G5、直接文本编辑、Shapes 原有行为不变 |
| CE-18 | 多选、多 policy slot、宽有序容器、深树、大源子树、连续输入 | 分记候选、角点映射、handler、child 扫描、排除展开、反馈及同步；记录缓存失效、p50/p95、feature 和能力关闭后的同配置产物 |

第三方消费还需覆盖独立 `ContainerCreationTemplate` 的关联 Payload/Plan，
通过同一创建 Tool/drop，不能用业务代码直接改模型证明 CE-11/16。
所有计划可读取字段与资格回执都需要编译级构造边界验证；
测试不能仅校验第三方遵守约定的正例。

## 实施验证顺序

1. 先运行受影响库的 check 与单个容器契约 target。
2. 功能切片完成后运行既有 `editor.contracts` / `extensions`，
   检查 suite 内容，避免重复执行其已包含的命令。
3. 最终边界运行 workspace fmt、严格 Clippy 和 test；纯文档批次运行 docs 与 diff check。
4. Native/Web 分别记录实际输入、DPI、capture 和取消场景，未执行的平台保持未完成。

跨显示器、窗口外 capture 和系统 DnD 不由 headless 成功推断。
性能记录包括完整模型同步，不能只用候选计算时间代表整个输入响应。
