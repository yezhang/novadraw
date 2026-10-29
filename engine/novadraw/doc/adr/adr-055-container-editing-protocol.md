# ADR-055：容器编辑与父坐标协议

类型：`architecture-decision`

能力归属：Editor（`novadraw-editor`），对应 GEF 容器编辑协议。
Editor 持有操作集合、目标容器解析、父域映射编排与编辑事务协议；
Core 的命中、坐标与 Runtime 提交作为基础依赖，
应用 Command 持有业务模型的换父和顺序变更。
Core 基础故障由 Editor 端到端测试验证其传播，不计为新增 Core 能力。

## 状态

已接受，修订 2，2026-10-29。设计复核通过后，用户明确授权
“评审通过，按 ADR-055 执行”，进入接口落定与实施验证。
依据见[修订 2 复核](../verification/reviews/adr-055-revision-2-review-2026-10-29.md)。
总体范围见 [Editor／GEF 框架能力补充方案](../design/editor/core-capability-increment.md)。
本决策不替代已接受的 ADR-032/034/037/054。

本轮采纳[首轮评审](../verification/reviews/adr-055-draft-review-2026-10-29.md)的
CE-DR01～09。修订 2 继续采纳
[修订 1 复核](../verification/reviews/adr-055-revision-1-review-2026-10-29.md)的
R1-01～03；其余已解决合同保持。接口已落定于
`crates/novadraw-editor/src/container/`，结果以 P2 backlog 与验证记录为准。

## 问题与参考事实

模型换父后，Viewer 已能退休旧 Part 并在新父节点下重建。
但默认 SelectionTool 直接使用完整选择集合，在 contents 域计算统一增量；
CreationTool 和 typed drop 也按 contents 域创建。它们没有闭合跨容器编辑。

GEF Classic 基线 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
中的以下源码提供参考：

| 类 / 方法 | 可采用的行为 |
|---|---|
| `tools.ToolUtilities.getSelectionWithoutDependants` | 操作集合去除被选祖先覆盖的后代，保持剩余选择顺序 |
| `tools.DragEditPartsTracker.getExclusionSet` | 查找落点时排除拖动源 Figure 与连接层 |
| `tools.DragEditPartsTracker.getCommand` | 同父移动；换父时组合源侧 ORPHAN 与目标侧 ADD |
| `editpolicies.LayoutEditPolicy` / `ContainerEditPolicy` | 容器贡献创建、移入、移出和布局命令 |
| `editpolicies.ConstrainedLayoutEditPolicy` | 将视觉约束转换成应用模型约束 |
| `editpolicies.OrderedLayoutEditPolicy` | 根据指针位置生成插入参考并保持多元素顺序 |

Novadraw 保留行为分工，但不直接执行两个可能产生非法中间态的移出/移入命令。
完整换父计划由应用一次性提交；普通 CompoundCommand 不等于任意模型事务。

## 1. 所有权与扩展

Editor 拥有手势、候选容器、受检坐标映射、临时反馈和提交时机。
应用拥有 containment、约束转换、连接合法性、模型事务与撤销数据。
Command 不保存 EditPartId/FigureId；保存 ModelId、父关系、原顺序和模型约束。

容器资格由注册的 typed request policy 表达，不根据 Figure 类名、
是否有 children 或特定内置布局判断。内置 XY/Ordered 策略与第三方策略
共用 RequestProtocol、typed contribution、PolicyHandlerBuilder 和 ToolContext。
禁止增加仅服务于内置容器的 EditPolicy hook。

首批提供可选的容器拖动 Tool，显式通过现有 `set_active_tool` 安装；
同父、跨父和重排都走其统一计划流程。保留 SelectionTool 的平面编辑语义。
后续默认工具组合必须先有独立消费者证据，不能根据是否内置建立快捷分支。

一个 `ContainerMoveTool<A, P>` 实例在整个手势中固定 P，不随候选布局切换 request。
参与该手势的源父须注册统一资格 handler，目标须注册 `P::Request` 计划 handler；
同父 source 的资格由其目标容器提供。不同布局可以注册同一个开放 P，
在 policy 内将共同约束输入转换为自己的 XY 或有序插入结果。
只注册另一 family 的容器对当前 P 返回未支持，依既定无贡献规则探测祖先；
这不表示该容器拒绝了所有编辑协议。首批不建设自动多协议组合或布局类型分支。

## 2. 操作集合与目标

按顺序完成以下步骤：

1. 同步模型通知，校验全部输入身份，拒绝重复 source；
2. 去除具有被选祖先的后代，保留剩余 source 顺序；不修改 Viewer selection；
3. 记录 source ModelId、原父 ModelId、原索引和模型 revision；
4. 查找目标时排除操作源的整个视觉子树、Handle、Feedback 与连接层；
5. 从命中 Part 沿 containment 祖先向上探测容器 policy，包括合法 contents；
6. 首个明确接纳的容器成为本次 target；无贡献可继续向上，显式拒绝不变成回退。

框架拒绝 foreign、退休对象、synthetic root、以连接为容器、
目标位于 source 子树中，以及形成 containment cycle 的计划。
隐藏、禁用及裁剪规则沿用 Core 命中和有效状态查询，不另建几何命中模型。

target 探测使用专门的开放 request family。普通 request 的
“source 解析一次 target，再在 target 聚合”规则保持不变，
不能通过递归重定向改变所有既有请求。

候选与源父资格查询使用通用受检 host 聚合入口，直接在指定 host 的 policies
上聚合，不运行普通 dispatch 的 target 重定向。metadata 保留真实操作 source，
不把候选容器伪装成 source。第三方 request 使用完全相同的入口：

```rust,ignore
// GraphicalViewer 的公开签名；ToolContext 返回 ToolError。
pub fn aggregate_at<R>(
    &mut self,
    host: EditPartId,
    metadata: &RequestMetadata,
    payload: &R::Payload,
    limit: ContributionLimit,
) -> Result<RequestResolution<R::Contribution>, ViewerError>
where
    A: 'static,
    R: RequestProtocol<A>;
```

入口先校验 host 与 metadata 全部 source 的 namespace、live membership、
activation 和模型绑定，再按既定 slot 顺序聚合 handler，调用相同 reducer。
不调用 handler 的 target callback，不按 marker 类型建立分支。
`ContributionLimit` 由调用者显式选择 `All` 或 `AtMostOne`；
容器计划及资格查询选择后者，在 reducer 之前拒绝多个非空 contribution，
因此第三方 reducer 无法隐去重复接纳。该限制同样适用于任意外部 request。
空 source 可用于无已有对象的请求，但不表示一个合法移动批次。
InteractionRevision 只标识交互顺序，不证明模型或 policy 仍处于原状态。
容器协议另外用下面的受检批次核对 ModelRevision 和 policy generation。

候选 host B 的普通 request 即使会重定向到 A，容器聚合仍在 B 执行；
B 的明确拒绝终止本次候选搜索。无 handler/无贡献才可探测上一级。

### 2.1 第三方计划的最小适配合同

选用关联类型协议与不可伪造的受检批次头。候选公共路径为
`novadraw_editor::container`；复用的 host 聚合入口属于 Viewer/ToolContext。
下表是签名中各类型的合同，字段通过只读 getter 暴露：

| 公开类型 | 所有权与可读内容 |
|---|---|
| `MoveBatch<I>` | 框架创建；Viewer namespace、ModelRevision、InteractionRevision、完整原始输入、非空有序操作项、唯一锚点 source 与抓取点；每项含 PartId、ModelId、原父 PartId/ModelId、原索引 |
| `MoveCandidate<I>` | 框架创建；批次、当前 host/目标、策略 generation、映射几何、本地项/移入项分组；每次重算获得新的候选身份 |
| `PermissionQuery<I>` | 框架创建；候选身份、活动协议身份、颁发 host、操作 kind、精确有序子批；本地修改与各原父移出分别查询 |
| `PermissionDecision` | policy 返回同意或显式拒绝；无贡献表达未支持，不自带可复用授权 |
| `PermissionReceipt<I>` | 框架在对应 host 聚合成功后签发，私有构造；绑定实际 host、活动协议、kind、子批、revision、generation 和候选身份 |
| `MovePlanHeader<I>` | 第三方通过 `from_candidate` 复制声明；目标与精确有序操作项、原父、候选身份；字段只读，声明不等于校验结果 |
| `CheckedMovePlan<A, P>` | 框架完整比对 header 与 candidate、资格回执后创建；私有构造，仅当前候选与事务边界内有效 |

操作 kind 是本协议的有限语义值（本地修改/移出），不是 Request、Tool 或
布局类型的分派枚举。未知业务操作通过其他 typed request 扩展。
应用约束留在关联类型，框架不 downcast 或读取其私有业务字段。

```rust,ignore
pub trait MovePlan<A: ModelAdapter>: 'static {
    fn header(&self) -> &MovePlanHeader<A::ModelId>;
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError>;
    fn build_command(
        self,
        model: &A,
    ) -> Result<Option<Box<dyn Command<A>>>, ContainerEditError>;
}

pub trait ContainerMoveProtocol<A: ModelAdapter>: 'static {
    type Payload: RequestPayload;
    type Plan: MovePlan<A>;
    type Request: RequestProtocol<A, Payload = Self::Payload, Contribution = Self::Plan>;

    fn payload(candidate: &MoveCandidate<A::ModelId>) -> Self::Payload;
    fn source_geometry(
        model: &A,
        source: &MoveSource<A::ModelId>,
    ) -> Result<ConstraintGeometry, ContainerEditError>;
}

// 两类消费者使用同一个泛型 Tool，不各自实现一套状态机。
pub struct ContainerMoveTool<A: ModelAdapter, P: ContainerMoveProtocol<A>> { /* private */ }
```

`MoveSource` 是批次中的只读操作项。`ConstraintGeometry` 表达应用约束的有限
角点/仿射信息，来源与数值合同见 §3。`ContainerEditError` 区分身份、
批次/资格不匹配、明确拒绝、几何失败及既有 Viewer/Policy 错误；
不把不可恢复错误变成无贡献。

Tool 构造候选，通过 `P::payload` 与
`aggregate_at::<P::Request>(..., ContributionLimit::AtMostOne)` 聚合；
然后逐字段校验第三方 header，禁止漏项、重复、换目标或更换原父。
资格查询使用下面的统一 typed request 与同一 registry；一个子批恰好需要
一个同意结果，多个互斥结果拒绝。回执来源由聚合服务签发，不能相信 plan 自报。
Tool 将精确资格集合交给框架校验器；计划不保存伪造回执，也不能自行选择免校验。

header 的重复读取也不能成为绕过路径：校验器持有一次读取的 owned header，
当前 checked wrapper 有效期间只调用其对应计划。源几何、反馈和最终约束的
数值均需校验；不因 header 合法而信任任意模型副作用。
返回零个计划为未支持；原始 contribution 与 reducer 最终结果都须至多一个，
第三方 reducer 不能把多个互斥 target 偷换成单个成功计划。
框架检查 reducer 最终结果与实际 host/当前候选匹配，不承诺检测任意恶意扩展代码。

这组接口允许不同 request、payload、policy slot 与 plan 关联类型；
底层 host 聚合和容器计划验证各有唯一入口。`feedback` 只在计划受检后调用；
释放时重新生成计划，完成校验和清理后才调用 `build_command`。
`Ok(None)` 表示经比较确认无变化；Command execute 仍须再次检查完整模型合法性。

### 2.2 统一资格 request 与调用阶段

选择公开统一 marker，内置与第三方源父 policy 注册同一个资格 handler，
不为每个 P 复制移出协议：

```rust,ignore
pub struct CheckContainerPermission;

impl<A: ModelAdapter + 'static> RequestProtocol<A> for CheckContainerPermission {
    type Payload = PermissionQuery<A::ModelId>;
    type Contribution = PermissionDecision;
    // key/reduce 沿用普通 RequestProtocol 注册与聚合合同。
}

// 在普通 EditPolicy::register_handlers 内：
builder.handle::<CheckContainerPermission>(Self::check_permission);
```

每次调用 `aggregate_at::<CheckContainerPermission>` 的 metadata.source_parts
恰好为当前资格子批，保持操作集合中的相对顺序；不使用完整批次或颁发 host 代替。
query 指向完整候选，并包含同一精确子批与 host/kind。框架在调用前检查
metadata 与 query 一致，调用后签发绑定实际 host 的回执。
活动 P 的 request 身份由框架写入 query，回执不能用于另一活动协议；
policy 可通过 typed `is_for<R>()` 查询资格所服务的 family，
诊断字符串不作为身份。query 与 receipt 都不能由第三方伪造构造。

一个候选的调用顺序固定为：

1. 从同一批次与唯一平移构造当前候选，调用 `P::payload`；
2. 在候选 host 聚合 `P::Request` 的只读原始计划；无贡献则探测祖先，
   明确拒绝或重复 contribution 终止搜索；
3. 比对原始 plan header 的目标、source、父关系和候选身份；
4. 按本地子批、各原父移出子批查询 `CheckContainerPermission`，
   校验 metadata/query，收集该候选的回执；
5. 全部资格齐全后创建 `CheckedMovePlan`，再调用 feedback；
6. release 使用最新位置重新走此流程，清理后构建单个 Command。

步骤 2 的计划不拥有授权，步骤 5 之前不得发布 feedback 或构建 Command。
目标接纳由计划 handler 表达；资格 request 检查本地修改与实际源父移出。
第三方消费者须注册真实资格 handler，由公共 Tool 触发查询；
直接构造回执不能作为外部消费者证据。

## 3. 坐标合同

平台输入始终为 logical surface point。父坐标明确指目标 Part 的
content pane 的 child-content domain；它不一定等于主 Figure local domain。
模型若使用其他约束表达，由应用 policy 转换。

Viewer 和 ToolContext 提供受检的 surface ↔ part child-content 映射，
复用 Runtime 的变换、insets、scroll、scale 与逆矩阵。
无效身份返回领域错误；只有数学奇异返回明确的无映射结果。
非有限输入、有限输入派生溢出、逆矩阵计算失败与不可表示结果返回几何错误，
不能吞为无候选。极小但可逆尺度不能仅因固定 epsilon 被归类为奇异。
Tool 不退回 contents 坐标或使用零坐标，遵循 ADR-038。

Core 公开 `Affine2D::checked_inverse() -> Result<Option<Affine2D>, InverseFailure>`。
`None` 只表示数学奇异；`InverseFailure` 区分非有限输入、逆值不可表示和数值失败。
Viewer 的 `surface_to_child_content` 沿用这一区分，容器 Tool 保留原错误原因。
Core 公开符号快照相应增加 `geometry::InverseFailure`，不暴露第三方几何类型。

对每个 source 独立映射几何，不能向不同父域直接加同一个模型增量。
输入由应用提供约束几何，不从视觉 AABB 推导模型约束；阴影、装饰和子 Figure
extent 不属于该输入。设源父域到 surface 的映射为 S、目标父域到 surface 为 T、
surface 拖动平移为 D，目标几何为 `T⁻¹ · D · S` 作用于源约束几何。
判断组合结果，而不是检查链路中是否曾出现旋转。相同源/目标旋转可互相抵消。

默认 XY 策略接纳组合映射后仍为轴对齐矩形的角点集合。反射和轴交换可以接纳，
规范化 origin 与非负尺寸；不保留方向性的模型须由应用资格明确确认。
零宽/零高是合法退化约束，模型最小尺寸由应用决定；不可表示的旋转/skew
不以 AABB 替代。第三方可表达更丰富几何，但不能接受非有限成功值。

XY 可表示性容差由显式 `GeometryTolerance` 配置给出，单位为目标父域逻辑单位，
默认绝对容差为零；不以未声明的 epsilon 纠正几何。消费者若启用非零容差，
记录数值与允许的角点残差，反馈和提交使用同一规范化结果。
全部输入变换、角点、中间运算、最终约束与反馈投影均检查有限性。

### 唯一锚点与批次平移

一个手势只有一个 anchor source 和一个抓取点，不为每个 source 各求平移。
实际按下对象在操作集合内时，它就是锚点，不要求其为 primary。
若按下后代被选中祖先覆盖，则按 containment 链选择覆盖它的操作 source，
把按下点转换到该 source 的约束域。祖孙去重保证该 source 唯一，
不得用 HashMap 遍历顺序或“最后处理的对象”选择锚点。

程序化启动可显式提供操作集合中的锚点及 logical surface 起点；
未提供锚点时使用有序操作集合第一项，仍须提供起点。
空操作集合或不在批次中的显式锚点结构化拒绝。选择顺序与锚点选择独立。

抓取点保存在锚点的原约束局部域，Gₐ 将该域映射到锚点源父域。
默认 XY 的局部域以逻辑长度计量，Gₐ 只平移约束 origin，
不把 width/height 编码成单位矩形缩放；因此零宽/零高不使 Gₐ 奇异。
自定义约束的 Gₐ 若数学不可逆，抓取点明确改存源父域，
以 identity 映射表达（而不是假造局部逆变换）。
源父到 surface 的逆映射若不可得则不能建立该手势；
数值计算失败仍返回几何错误，不走数学奇异的合法缺失分支。

每次更新取当前锚点源父映射 Sₐ 与最新 logical surface pointer p，
只计算一次 `D = Translate(p − Sₐ · Gₐ · grab_local)`；
源父域抓取点形式使用 `D = Translate(p − Sₐ · grab_parent)`。
所有 source 共用同一个 D，第 i 项使用自己的当前 Sᵢ，
候选几何为 `T⁻¹ · D · Sᵢ` 作用于该项的源父域约束几何。
共用 surface 平移不会向不同父域直接加同一个模型增量。
在变换未改变时，同父 XY 多选保持间距；不同父域也逐项验证同一个 D。
Ordered 策略仍可重排，不承诺保持 XY 间距。

指针不动而 scroll/zoom/auto-expose 变化时重算锚点映射和唯一 D，
不复用旧 surface 变换；模型 revision 改变则依 §4 取消整段手势。

换父默认保持拖动后的 surface 几何；同父移动使用同一映射。
有序布局可将指针映射为插入参考，而不承诺保持 XY。
创建和模板 drop 使用同一目标解析、父域映射与应用约束转换。
现有模板契约保持 contents 域；新增父域创建协议另用 typed marker，
不改变旧 payload 的坐标含义。

### 3.1 独立的父域创建输入

新增 `ContainerCreationTool<T>` 与受检 `drop_into_container<T>` 宿主入口，
分别消费同一个 `ContainerCreationTemplate<A>`，不改旧 CreationTool/drop_template：

```rust,ignore
pub trait ContainerCreationTemplate<A: ModelAdapter>: 'static {
    type Payload: RequestPayload;
    type Plan: CreationPlan<A>;
    type Request: RequestProtocol<A, Payload = Self::Payload, Contribution = Self::Plan>;

    fn default_size(&self) -> Dimension; // 目标父 child-content 域
    fn payload(&self, candidate: &CreationCandidate<A::ModelId>) -> Self::Payload;
}

pub trait CreationPlan<A: ModelAdapter>: 'static {
    fn header(&self) -> &CreationPlanHeader<A::ModelId>;
    fn feedback(&self) -> Result<Vec<FeedbackVisual>, ContainerEditError>;
    fn build_command(
        self,
        model: &A,
    ) -> Result<Option<Box<dyn Command<A>>>, ContainerEditError>;
}
```

创建候选/header 绑定 namespace、目标、两类 revision、策略 generation、
候选身份和父域几何。模板意图由关联 payload 携带；待创建业务 ID 的冲突由
应用计划与 Command 再检查。创建没有 source、原父、原索引或移出资格。
创建与移动使用不同受检头，空 MoveBatch 始终拒绝。
共有目标查询、host 聚合、坐标与反馈服务不因此复制。

press 的 logical surface 点先受检映射到 root contents 域并固定为创建起点；
更新时用当前 root contents→surface 变换重新投影，再映射到当前候选父域。
点击以该起点为 origin，尺寸使用目标父域 default_size；
拖动以重投影后的起点和最新指针映射到当前候选父域，生成规范化矩形。
点击/拖动判定阈值在 logical surface 域，复用既有工具配置，
不把缩放后的父域距离与 surface 阈值直接比较。
目标随最新指针解析；目标或视口变化时两个端点都按当前变换映射，
不混合两个父域，也不以旧 press surface 点抵消 auto-expose 的位移。

drop 使用显式模板抓取偏移（目标父域单位，默认零），以落点减去偏移作为 origin，
使用同一 default_size。创建预览不分配已提交模型对象；release/drop
重新校验，清理后提交一次 Command。成功或合法无变化后回到 SelectionTool；
拒绝不提交，取消/失焦清理并结束手势。旧模板语义与工具切换规则保持。

## 4. 计划与事务

开放容器协议的 typed contribution 表达一个只读编辑计划。计划包含：

- 固定 source 集合与原父关系；
- 接纳目标、插入位置或应用约束；
- 源侧移出资格与目标侧接纳资格；
- 基线模型 revision；
- preview feedback，以及提交时构建单个应用 Command 的能力。

每批必须覆盖全部 source；零个计划表示未支持，多个互斥接纳计划结构化拒绝。
源侧资格通过同一 policy registry 的 typed request 查询，不能绕开源容器约束。
所有查询与 preview 都不得修改模型。

每项 source 按原父相对目标分类，而不是对整个批次二选一。
原父等于目标的本地子批只查重排/约束修改；其他原父分别查询实际移出子批。
目标计划接纳完整有序操作集合并区分本地项与移入项；任一必需资格失败即整批拒绝。
A→B 与 B→B 混选时，B 拒绝移出不影响合法本地修改；A、C→B 各取自己的移出资格。
连接可以保留在完整选择中，但不进入 containment 操作集合。
关系合法性由应用计划检查；删除和连接编辑仍走原协议。

释放主键时使用最新 logical surface 位置重新同步、求候选与资格，
校验身份、revision、父关系、policy generation 与全批计划；
清除反馈后构建并提交一个 Command。没有变化的计划不产生历史。
应用 Command 先完成全批合法性检查，再原子修改父关系、顺序与约束；
undo/redo 恢复相应完整模型状态。通知发布只描述稳定结果。

新模型 revision 到达时取消整段手势并释放 capture，不只是丢弃候选。
视口变化不等于模型变化：scroll/zoom/resize 与 auto-expose 重新计算坐标、
target 和反馈，保留 source 与抓取点。手势不得切换到另一个 Viewer；
跨窗口或跨 Viewer transfer 属于后续独立合同。

模型命令成功但投影失败时，遵循 ADR-037：保留成功模型与 history，
Viewer faulted，不自动回滚模型。unknown-state 或扩展 panic 遵循现有 fault 通道。

| 当前状态 / 触发 | 处理与下一状态 |
|---|---|
| Idle / press | 同步并保存快照；启动 Tracking，不修改模型 |
| Tracking / move、视口变化 | 校验基线，按最新指针重算候选/计划/反馈；仍 Tracking |
| Tracking / 外部模型 revision、相关 policy 停用/替换/配置变化 | 整段 Cancel；清理、释放 capture，返回 Idle |
| Tracking / release | 同步、最新位置重算、完整校验 → 清理 → 构建 Command → Committing |
| Committing / 编辑会话再次同步 | Command execute 校验稳定模型基线；变化则原子拒绝，不沿用旧计划 |
| Committing / 模型与投影成功 | 本次命令 revision 是预期结果；恢复选择/focus，再结束 |
| 清理 prepare 拒绝 / lifecycle panic | 按 ADR-037 保留原因和实际进度，阻止构建/提交；按既有 fault/隔离规则结束 |
| Command 或投影失败 | 按 ADR-032/037 处理，不恢复未知状态，不重试提交 |

generation 属于相关 policy 安装及配置版本；配置更新必须通过显式受控失效入口，
任意扩展私有状态变化不能被框架自动观察，扩展须遵守此通知合同。
成功提交产生的 revision 不被外部变化取消分支再次处理。

## 5. 身份、选择与反馈

手势开始分别保存完整有序 selection 的 ModelId、primary 对应项、
独立 EditPart focus 的 ModelId，以及去重后的操作集合。后代为 primary、
被选择的连接、非选中的 focus 都不会因操作集合裁剪而丢失。

换父重建后旧运行时身份失效；execute 成功投影后批量映射完整选择，
保留仍可选择项的原顺序，primary 为剩余列表最后一项；原 primary 缺失时
采用同一规则。EditPart focus 独立映射，缺失时清空。
Figure keyboard focus 不由 ModelId 自动恢复，退休清理与宿主显式焦点请求沿用原合同。

每次 undo/redo 在同步外部通知后、操作模型前保存该时刻的当前选择/focus，
成功投影后按同样规则恢复。历史不保存旧 Viewer 状态，也不恢复最初 execute 的
选择快照。快照与恢复属于 EditorDomain 的编辑会话收尾，不写入应用 Command；
为需要重建的命令统一使用此行为，不按具体内置命令类型分支。
仅成功投影后恢复；恢复本身失败按 Viewer 一致性错误处理，保留已成功 history。

取消/拒绝表示工具不主动改变选择；外部删除的既有 reconcile 仍生效，
不得复活已删除对象。普通外部变更仍沿用既有规则。

source 与 target 反馈分别归实际 owner。target 更换、失活、取消、换 Tool、
失焦、历史操作和提交均清理。父域预览必须先投影到既有 scaled 或 unscaled
feedback 域；不能把目标父坐标直接写入 root feedback layer。

## 6. 性能与包体积

设 k 为选择数量、h 为祖先深度、v 为命中遍历规模。
操作集合使用选中身份集合，最坏 O(kh)，深度遵守 10,000 上限；
源父资格按身份索引分组，保留首次出现与批次顺序，避免反复扫描已有分组。
候选命中遵循 Core，最坏 O(v)，O(h) 只表示候选个数。
源快照在手势开始时构建，指针更新只替换候选与反馈。
不得每次移动复制整个模型或重建所有容器描述。

有序插入统一为 `Start`、`End`、`Before(ModelId)`、`After(ModelId)`，
是插入位置的语义值，不承担开放能力分派。先移除目标中的本地 source，
再解释位置并一次插入完整操作集合，顺序使用操作集合顺序，可能不同于旧 child 顺序。
空目标的 Start/End 均为零位置。Before/After 只能引用剩余目标 child；
引用本批 source、foreign/缺失模型或非目标 child 均结构化拒绝，不自动猜测相邻项。
释放与 execute 再次校验 reference、父关系和 revision。
无变化检测同时比较父关系、完整顺序和约束。
应用模型命令完成后的现有全快照同步成本单独采样，不宣称 O(k) 总提交。

框架候选遍历 O(v+h) 之外，单列各候选 k 项几何映射、policy handler 调用、
Ordered 的 n 项 child 扫描及 feedback 创建/清理。总成本是这些工作的总和，
不为任意第三方 handler 承诺复杂度。资格回执校验同样计入批次开销。
排除集合保存源视觉根；命中遍历按根/祖先关系裁剪，避免每次枚举全部后代。
如果实际 Core 入口需要展开，须记录展开成本，不能仍记为 O(k)。
共享 source 约束快照与当前目标一次逆变换；模型变化取消快照，
视口变化失效映射、policy generation 变化取消资格；记录每类缓存更新次数。

不新增运行依赖和内置资源。实施后比较同配置消费者产物体积，
测量小场景、宽树和深层嵌套的目标查询、反馈更新及提交 p50/p95，
并记录分配、扫描数量和构建配置。Web 传输预算仅由实际 Web 产物判断。
比较同时记录 feature、未启用能力的裁剪结果、压缩传输及首次加载资源；
不得以模块拆分证明产物已经缩小。

## 7. 外部消费者证据

已落定两个独立消费者：XY 容器策略与第三方有序布局策略。
第三方自定义 request、payload、policy slot 和计划类型，不能修改 Editor。
二者验证相同的 targeting、坐标、取消、错误、反馈清理和 CommandStack 通道。
首批证据分为两类，不能互相替代：

- 两个独立消费者采用不同 Request/Payload/Plan 关联类型，分别实例化相同
  `ContainerMoveTool<A, P>`，证明泛型协议开放；均注册统一资格 handler。
- 一个同 Viewer 的 XY→Ordered 场景，两侧注册同一个活动 P 的 handler，
  由各自 policy 转换几何/插入位置，验证一次手势、同一 D、资格及单个历史操作。
  另放置只注册另一 family 的容器，确认当前 P 未支持的结果及祖先搜索行为。

这不证明一个手势自动组合多个 P；该需求出现后另行设计 typed 组合入口。
无窗口测试不替代 Native/Web capture、DPI 或系统 DnD 验收。

完整退出条件见 [验证矩阵](../verification/plans/editor-container-editing.md)。
消费者位于 `tests/consumers/editor-open-protocol/`，以独立 Cargo workspace 编译，
复用 `p2_e03_container_editing_contract` 夹具。公共移动与创建 Tool 均走
ToolContext 的 begin/candidate/resolve/show/submit 服务；外部 Tool 可复用同一服务。
`CreationPlan`、`ContainerCreationTemplate` 与 `CheckedCreationPlan<A, T>` 使用独立
Payload/Request/Plan，创建不查询移出资格。
私有批次、候选、查询、回执与 checked wrapper 通过 compile-fail doctest 固定构造边界。

## 8. 评审处置与同步清单

九项建议均采纳；具体实现选择为通用 host 聚合、私有受检凭证和关联类型计划。
保留原[评审报告](../verification/reviews/adr-055-draft-review-2026-10-29.md)作为首次草案证据，
不把其原结论改为已通过。

| 评审 ID | 修订位置 | 必须补充的验证 |
|---|---|---|
| CE-DR01 | §2，通用 aggregate_at 不运行 target callback | CE-04/16 普通重定向与 host 聚合对照 |
| CE-DR02 | §2.1，统一受检 header、资格凭证和关联 Plan | CE-02/10/16 漏项、重复、错目标/父/revision/generation |
| CE-DR03 | §4，本地子批与多原父移入分别取资格 | CE-08/09/10 混合批次与参数断言 |
| CE-DR04 | §5，完整 selection、独立 focus、每次 history 操作前快照 | CE-01/12/13 后代 primary、连接、外部删除 |
| CE-DR05 | §3.1，独立创建协议/Tool/drop/尺寸和抓取点 | CE-11/16/17 第三方嵌套创建 |
| CE-DR06 | §3，约束几何公式、组合可表示性与数值分类 | CE-06/07/14 相同旋转、反射、极小尺度、溢出 |
| CE-DR07 | §4 状态表与清理/事务顺序 | CE-13/14/15 release 重算、policy 更换、清理失败 |
| CE-DR08 | §6，显式首尾及 Before/After | CE-09/10 source reference、反向选择、多原父 |
| CE-DR09 | §6，完整工作计数和缓存失效 | CE-18 多选、多 slot、宽序列、大子树 |

修订 1 复核的剩余三项均采纳：

| 复核 ID | 修订位置 | 必须补充的验证 |
|---|---|---|
| R1-01 | §3 唯一锚点与批次 D，程序化启动和零尺寸抓取点 | CE-06/08/14 多 source 间距、异父映射、非 primary/被覆盖后代、静止指针 |
| R1-02 | §2.2 统一 CheckContainerPermission 类型、精确 metadata 子批与阶段 | CE-10/16 外部 handler 实际派发和回执来源 |
| R1-03 | §1/§7 固定活动 P、共同协议的同 Viewer XY→Ordered 消费 | CE-16 两个泛型消费者与同 Viewer 场景分别验收，不兼容 family 为未支持 |

原修订 1 复核结论保留为历史证据，本次修订不表示复核已通过。

本轮同步总体方案、验证矩阵、GEF 账本与 P2 的修订入口，
保持 `design-draft`。接受后再更新架构总览和 ADR-034 的 host 聚合补充；
实施时新增领域公开导出、消费者、对应 API snapshot 与错误契约测试，
通过后才提升 parity。设计修订不修改现行架构为“已实现”。
