# ADR-055 容器编辑与父坐标协议草案评审

类型：`architecture-review`

日期：2026-10-29。
评审对象：[ADR-055 草案](../../adr/adr-055-container-editing-protocol.md)，
关联[补充方案](../../design/editor/core-capability-increment.md)与
[容器编辑验证矩阵](../plans/editor-container-editing.md)。
行号指本次读取的 162 行草案；后续修订以章节和评审 ID 对应。

## 结论与范围

**建议修订后复核，暂不冻结公开 API。**
开放 policy、受检父域映射、单个应用 Command 原子换父和投影失败保留 history
的方向可以保留。需要先补齐候选分派、第三方计划校验、混合父批次、选择恢复
和嵌套创建这五项 P1 契约，再澄清四项 P2 边界。

这里的 P1 表示可能使合法操作无法表达，或使无效计划通过的设计缺口；
P2 表示影响行为确定性、数值边界或成本判断的缺口。不是已复现的实现缺陷。
本次只读项目规范与 GEF 源码，未扫描 Novadraw Rust 实现，未执行交互或性能测试。
草案对当前 SelectionTool、Viewer 实现的描述作为输入，未独立核验。
本文记录建议，不改变 ADR 的规范效力、实现状态或执行队列。

| ID | 优先级 | 草案位置 | 修改主题 |
|---|---|---|---|
| CE-DR01 | P1 | §2，63–70 行 | 候选容器分派必须明确是否经过既有 target 解析 |
| CE-DR02 | P1 | §1、§4、§7 | 第三方 typed 计划与通用 Tool 之间需要公开的校验合同 |
| CE-DR03 | P1 | §4，108–111 行 | 同批本地 source 与外部 source 分别查询资格 |
| CE-DR04 | P1 | §2、§5，52–54、128–131 行 | 完整选择快照与操作集合分开；focus 独立保存 |
| CE-DR05 | P1 | §3、§4，90–102 行 | 创建没有已有 source，不能直接套用换父计划 |
| CE-DR06 | P2 | §3，78–89 行 | 几何映射公式、XY 可表示性与数值分类 |
| CE-DR07 | P2 | §4、§5，113–135 行 | revision 失效、释放重算与反馈失败的处理顺序 |
| CE-DR08 | P2 | §6，145–146 行 | 插入参考方向、边界与 source 引用规则 |
| CE-DR09 | P2 | §6，139–151 行 | policy 调用、批次映射与宽序列扫描成本 |

## 评审依据

项目契约以以下已接受文档为准：

- [ADR-034 §1–3](../../adr/adr-034-editor-tool-request-handle-protocol.md)：
  request 的关联 Payload/Contribution、统一 metadata 和 typed handler 注册。
- [Editor 架构 §7–8](../../design/editor/architecture.md)：
  selection 最后一项是 primary；EditPart focus 与 Figure keyboard focus 独立；
  dispatch 先解析 target，再在 target 聚合，不能递归重定向。
- [ADR-032 §1–2](../../adr/adr-032-viewer-edit-session-and-drive-boundary.md)：
  命令执行前同步；commit/reject/fault 分别关闭模型事务。
- [ADR-037 §5–6](../../adr/adr-037-editor-incremental-failure-isolation.md)：
  Overlay 完整预检、实际生命周期失败、成功模型/history 与投影 fault 分开。
- [ADR-038 §4–5](../../adr/adr-038-query-identity-and-failure-semantics.md)：
  owned snapshot 不能证明其中 ID 仍 live；不可逆与数值计算失败必须区分。
- [ADR-054](../../adr/adr-054-template-creation-and-marquee.md)：
  旧模板使用 contents 域；点击默认尺寸、拖动规范化矩形、工具结束行为已有合同。

GEF 参考仓库为 `/Users/bytedance/Documents/code/GitHub/gef-classic`；
已核对 HEAD 为 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
源码位置均相对该仓库，只读 `org.eclipse.gef`，未使用 Zest：

| 源码位置 | 核对结果与使用限制 |
|---|---|
| `org.eclipse.gef/src/org/eclipse/gef/tools/ToolUtilities.java`，`getSelectionWithoutDependants`，47–49 行 | 返回过滤后的新列表；用于支持操作集合与 Viewer 选择分离 |
| `org.eclipse.gef/src/org/eclipse/gef/tools/DragEditPartsTracker.java`，`getCommand`，301–334 行 | MOVE 与 ORPHAN/ADD 分流；组合命令不能证明 Novadraw 模型事务原子性 |
| 同上，`getExclusionSet`，366–381 行 | 保存操作源主 Figure 和连接层；草案要求整个视觉子树，实施时仍需单独证明排除语义 |
| `org.eclipse.gef/src/org/eclipse/gef/editpolicies/ConstrainedLayoutEditPolicy.java`，`getConstraintFor(CreateRequest)`、`translateToModelConstraint` | 创建坐标先转布局相对域，再转模型约束；不提供 Novadraw 仿射数值合同 |
| `org.eclipse.gef/src/org/eclipse/gef/editpolicies/OrderedLayoutEditPolicy.java`，`getAddCommand`、`getInsertionReference`、`getMoveChildrenCommand`，90–124 行 | 插入参考交给应用命令；同文件注释对 null 的首尾含义存在矛盾，不宜直接照搬 |

以下反例是对草案的逻辑检验，修改内容是评审建议。未将 GEF 行为当作
Novadraw 新 API、错误分类或事务保证已经存在的证据。

## P1 修改建议

### CE-DR01：候选 metadata 并不保证在候选容器上聚合

草案说把候选容器作为 metadata source，即可在该容器上分派。
但现行 Editor 架构 §8 规定，source 的 policies 会先解析一次 target。
若容器 B 的 handler 把普通请求重定向到父容器 A，探测 B 就可能实际聚合 A，
跳过 B 的明确拒绝。此时探测循环记录的候选 B 与实际聚合 host A 也不一致。

**建议**：冻结“在已校验 host 上聚合”与“按 source 解析 target”之间的边界。
可以公开一个通用、受检的 typed host 聚合入口，复用相同 registry、reducer、
生命周期和错误通道；也可以用开放协议描述该分派模式。不能按内置 marker
类型匹配建立特权路径。候选探测与源父资格查询都需要这项语义。

建议补文：

> 容器探测在当前候选 host 的 policies 上聚合，不运行普通请求的 target 重定向。
> 调用边界校验 host、真实操作 source 和 revision。普通 dispatch 的一次 target
> 解析规则不变。第三方 request 可通过同一受检入口获得相同聚合语义。

验证补充：扩展 CE-04/CE-16，安装会重定向普通请求的内层 policy；
证明容器探测仍在内层执行，其明确拒绝不能被父容器的接纳覆盖。

### CE-DR02：通用 Tool 如何校验第三方计划尚未定义

ADR-034 允许每个 request 自定义 Contribution。草案同时要求第三方自定义 request、
payload、计划类型，并由框架检查全部 source、原父关系、cycle、资格和 revision。
但尚未说明通用容器 Tool 如何读取这些未知类型，也未指定这些校验发生在哪里。
仅把 source 放进自定义 payload，不会自动获得 envelope 的身份校验。

反例：第三方返回一个可构建 Command 的计划，但漏掉一个 source，或把源父 P
颁发的资格用于另一个父 Q。若 Tool 只持有构建 Command 的闭包，就无法验证完整批次。

**建议**：在 API 冻结前给出公开的最小 typed 适配合同，覆盖：

- 原始输入、去重后的操作 source、目标、原父关系和基线 revision 的只读公共视图；
- 框架能检查的资格关联信息，包括哪一批 source、哪一个父、哪种操作；
- 第三方模型约束与 preview 的扩展方式；
- 预览、重新规划、构建单个 Command 的调用阶段及错误返回。

具体可使用关联类型、trait 或带通用校验头的 typed 计划；无需预设公共类型名。
外部 opaque 约束可以由应用解释，身份、批次覆盖和拓扑检查必须有统一入口。
构建 Command 后，应用仍在 execute 内重新检查完整模型合法性。

验证补充：扩展 CE-02/CE-10/CE-16，分别返回漏项、重复项、换目标、
复用另一 revision/原父的资格。内置和第三方都应在写模型前拒绝。
独立消费者还应证明：自定义关联类型能由同一个容器 Tool 消费，而不是另写工具绕开合同。

### CE-DR03：混合批次必须按 source 与目标的关系分类

草案区分同父与跨父，却未明确整个批次不是二选一。
例如选择目标 B 中的节点 b，以及容器 A 中的节点 a，再一起拖入 B：
b 需要重排或约束修改；a 需要 A 的移出和 B 的移入。按“每个原父都须允许移出”
处理，会错误地要求 B 允许 b 移出。

**建议**：先按原父分组，再相对目标分类。原父等于目标的子批只查重排/约束修改；
其他原父分别查实际移出子批。目标计划接纳完整有序操作集合，并区分保留项与移入项。
目标 child 序列先移除已在目标内的本批 source，再一次性插入完整操作批次。
任一必需资格拒绝，整批拒绝。

验证补充：在 CE-08/CE-09/CE-10 增加 A→B 与 B→B 混合批次，
以及 A、C→B 的多原父批次。验证查询参数、相对顺序、无重复 child、
单个 history entry 和 undo/redo；B 拒绝移出不应阻止合法的本地重排。

### CE-DR04：恢复完整选择；focus 不从操作集合推断

§2 只要求记录去重后的 source ModelId；§5 的“保存的有序 ModelId 列表”
没有指明是完整选择还是操作集合。选择父 p 和后代 c 时，操作集合只有 p，
但换父会重建 p 的子树。若只恢复操作集合，c 会从选择中消失。
focus 也可能位于非选中节点，不能从 selection 列表推断。

**建议**：开始手势时分别保存完整有序 selection 的 ModelId、primary 对应项、
独立 EditPart focus 的 ModelId，以及去重后的操作快照。成功投影后批量映射完整选择，
按原顺序保留可映射项；primary 若失去映射，明确采用既有“最后一项”规则。
EditPart focus 单独恢复。Figure keyboard focus 是否恢复及如何找到新 Figure，
应显式声明，不能用 ModelId 暗示二者已经统一。

“取消和拒绝不改原选择”应限定为工具不主动修改选择；外部删除触发的既有
reconcile 仍生效，不能恢复已删除对象。

验证补充：扩展 CE-01/CE-12/CE-13，覆盖后代为 primary、非选中节点 focus、
连接被选择但不参与 containment 操作，以及外部删除后取消。
矩阵还要求 undo/redo 的选择恢复：应明确是在每次历史操作成功投影后映射
该次操作前的当前选择，还是只有首次 execute 恢复；不能隐含由模型 Command 保存 Viewer 状态。

### CE-DR05：嵌套创建需要独立的计划输入与工具合同

换父计划要求已有 source、原父、移出资格和原索引。新对象尚无这些事实。
§3 只说新增 typed marker，没有明确创建如何使用同一 targeting/映射服务、
空 source 如何解释、模板数据在哪里进入 policy，以及由哪个 Tool/入口消费。
仅增加 marker 不能说明它已与 ADR-054 的 CreationTool/drop 接上。

**建议**：共享候选查询、父域映射、反馈与事务通道，但分别定义
“已有对象移动”与“新对象创建”的 typed 输入。创建计划包含模板意图、受检目标、
父域几何/应用约束、revision 和单个 Command 构建能力，不要求移出资格。
空 source 不是移动成功计划；创建 payload 的身份与几何仍必须受检。

同时冻结父域创建的启用入口、默认尺寸所在坐标域、点击与拖动矩形规则、
drop 抓取点，以及成功后的工具切换行为。采用新工具或开放 typed 适配均可，
旧模板的 contents 含义保持不变。

验证补充：扩展 CE-11/CE-16/CE-17，独立第三方模板在缩放且有 inset 的
嵌套容器中完成点击、拖动和 drop；记录默认尺寸与抓取点的预览/提交结果。
拒绝和取消不能把待创建对象加入模型或产生 history；模板预先持有业务身份
不等于对象已经提交到模型。

## P2 修改建议

### CE-DR06：冻结映射依据与 XY 数值边界

“独立映射几何”尚未指明输入来自模型约束矩形还是视觉包围盒。
装饰、阴影或子 Figure 的 surface AABB 不一定等于可写回的模型约束。
“旋转/skew 不接纳”也需要判断组合映射的结果：源域与目标域可能有相同旋转，
组合后仍能准确表示为目标域矩形。

**建议**：将输入定义为应用约束对应的几何及其角点/仿射信息。
设源父域到 surface 的映射为 S、目标父域到 surface 为 T、拖动 surface 平移为 D，
目标候选几何为 `T⁻¹ · D · S` 作用于源约束几何。判断最终结果能否保持几何语义，
不以链路中曾出现旋转为由一律拒绝，也不以 AABB 代替不可表示结果。
明确反射、轴交换、零尺寸及浮点误差的处理；如允许误差，需声明单位和界限。

直接引用 ADR-038：只有数学奇异返回无映射；非有限输入、结果不可表示与
数值计算失败返回几何错误，不能吞为“无候选”。源和目标变换、全部角点、
最终约束与 feedback 投影都需校验。第三方策略不能接纳非有限成功值。
视口变化后还需明确抓取点的坐标域和重算依据，保证静止指针时 auto-expose
也能更新拖动位置，而非重复使用开始时的旧 surface 映射。

验证补充：CE-06/CE-07/CE-14 增加源/目标相同旋转、反射、极小可逆尺度、
有限输入派生溢出，以及指针不动时滚动与缩放。比较角点和模型约束，不只比较 AABB。

### CE-DR07：明确释放时序与失效状态

“新 revision 取消旧计划”可能解释为整段手势结束，也可能只是移除候选；
“释放时重新校验”未明确是否用 release 的最新位置重新规划。
模型不变时，视口或 policy 配置也可能改变接纳与映射。
ADR-032 的命令入口还会再次同步，不能只依赖 release 前的一次检查。

**建议**：列出简短状态转移表，明确 revision 变化是否取消整个手势并释放 capture；
建议首批采用整段取消。释放按最新 logical surface point 重算候选与资格，
再清理反馈并构建 Command；execute 在既有编辑会话同步后再次核对基线。
policy 停用、替换或相关配置变化应取消或重新规划，不能继续使用旧接纳结果。

反馈清理的 prepare 拒绝与 lifecycle panic 按 ADR-037 分类；
清理失败阻止后续正常提交，保留原始错误和实际清理进度。成功模型产生的
新 revision 不应被当成外部 revision，让提交后的选择恢复步骤被取消。

验证补充：CE-13–CE-15 增加 move 与 release 位置不同、release 前视口变化、
命令会话同步发现 revision 变化、policy 换装，以及 feedback 清理失败。
验证模型/history、capture、fault 与选择恢复的先后事实。

### CE-DR08：有序插入参考要有确定的方向和边界

“移除 source 后解释插入参考”仍未说明 reference 是 before 还是 after，
首尾如何表示，以及 reference 本身属于操作集合时如何处理。
已核对的 GEF `OrderedLayoutEditPolicy` 注释对 null 的首尾解释并不一致，
不能把该注释当作现成规范。

**建议**：统一为带方向的 ModelId 参考或显式首尾位置；如果使用索引，
标明它属于移除本批 source 后的序列。对指向已移除 source 的参考，
明确在规划阶段规范化为剩余相邻位置，或结构化拒绝。
稳定顺序采用草案已经承诺的操作集合顺序，明确它不同于原 child 顺序。
所有 reference 身份、目标 child 关系和 revision 在提交前重新校验。

验证补充：CE-09 增加首尾、空目标、反向选择非连续 child、
reference 属于 source、reference 被删除，以及混合父批次。

### CE-DR09：复杂度需要把每个候选的批次工作算进去

`O(h)` 可以描述祖先候选数量，不能代表完成 h 次 policy 查询的总成本。
每次查询若映射 k 个 source、调用多个 policy，或为 Ordered 扫描 n 个 child，
整体成本会随 k、policy 数和 child 数增长。排除源子树也需要说明是否枚举全部后代。

**建议**：分别记录候选遍历成本、source 几何映射数、handler 调用数、
Ordered child 扫描数，以及 feedback 更新/清理成本。框架控制开销可写为
`O(v + h)`，批次映射与应用 policy 成本单列；不要给任意第三方 handler
承诺总复杂度。优先复用源快照、目标域一次变换和排除根判定，
说明缓存随模型、视口和 policy 变化的失效条件。

CE-18 增加多选、多 policy slot、宽有序容器和大源子树场景。
产物比较同时记录未启用能力的裁剪结果、构建配置与 feature；
Web 实际压缩传输和首次加载成本沿用现有计划，不由 Native 差值推算。

## 建议修订与复核顺序

先处理 CE-DR01/02，提交最小公开签名或可编译消费者设计，证明统一 registry 能
表达受检 host 聚合与第三方计划。再处理 CE-DR03–05，明确批次与创建输入、
选择/focus 所有权。最后补齐 CE-DR06–09，并同步验证矩阵。

修订时在 ADR 增加评审 ID 对应表；当前设计、GEF 账本和 API snapshot
的同步清单应一起明确。矩阵已有 CE-01–CE-18 不等于上述新增断言已获覆盖，
本次没有执行任何容器验收。

中文交付复核已单独完成：保留原始术语、行号和模态；问题、假设反例与建议分开；
未把建议改写成已接受合同，也未添加实现或性能通过结论。
