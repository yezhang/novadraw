# 开放吸附协议、内置策略与编辑计划实施记录

类型：`implementation-verification`

能力归属：Editor（`novadraw-editor`）。Core 只提供既有查询与坐标契约；
平台输入、应用模型 Command 和 Guide 持久化仍由各自 owner 验收。

日期：2026-11-02。状态：E04 实施中，协议、内置策略和移动／创建／resize 计划已通过定向验证。
用户授权“如果复核通过，则推进方案”；[修订 2 复核](../evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/adr-058-revision-2-review-2026-11-02.md)
通过，故 [ADR-058](../../adr/adr-058-open-editor-snapping.md)修订 2 已接受并进入实施。
本记录不关闭 E04；平台输入、完整成本与最终门禁仍待验收。

## 已实施的契约

新增公开领域 `novadraw_editor::snap`，不增加根目录平铺导出。
Viewer 持有注册表，手势持有 SnapSession。策略、几何 provider、准入及对齐／布局
求值器通过 typed key、关联 payload 与实例身份注册；同一 family 可有多个配置实例。
替换返回新的代际，旧实例及其声明失效；注册不执行 prepare、query 或 Command。
payload、prepared 和 Error 均没有新增 Clone／Send／Sync 要求。

Editor 从 E03 MoveCandidate 构造不可伪造的 SnapInput，保留原始控制点、最新位置、
完整 source／target 映射、模型／projection revision、policy generation 与视口快照。
受检 sibling 查询使用 committed Figure bounds、有效可见性和真实四角；
合法不参与与查询失败分开。完整 sibling 成员及可见几何在回调前后重新核验。

ThresholdCapture 与 ExactProjection 使用普通 EvaluationRule 注册。后者委托外部关系
求值器检验冻结输入；不存在 Grid 类型免检分支。
候选按策略顺序、surface 距离、provider 顺序、稳定特征及候选次序排序。
仲裁保护已采用事实的写入集和依赖集，每次暂定合成都重验早先事实；
合法冲突跳过较晚候选，非法输出和 Error 拒绝整次求解。

来源、特征、候选和反馈使用显式有限预算。sink 保留第一次拒绝，
扩展忽略 push 的错误仍不能取得部分成功。
每次求解尝试先使同一 session 的旧结果失效，即使使用同一个候选、
模型和配置版本未改变，且本次尝试最终失败，也不能恢复旧结果。
最终几何复验保留所有外部依赖与事实，失效或布局改写破坏命中时拒绝，不重新 query。

所有求值回调保留原始 Error source；panic 记录诊断、fault Viewer 并沿用既有传播规则。
会话计数区分 provider、prepare、query、规则求值、version 和完整 owner 检查。
耗时由宿主显式提供单调时钟；没有时钟时记录 `None`，不报告零成本。
默认路径没有依赖 Native 的 Instant，也没有新增平台或计时依赖。
诊断记录实际进入的 provider version／features、策略 version／prepare／query／
prepared 复用检查和规则 version／evaluate 边界，panic 不再只定位到整次 solve。

## 内置策略与 prepared 复用

Grid、Geometry、Guide 使用普通 provider 和 SnapStrategy 注册，没有额外内置分派。
provider 可以通过 `push_typed` 提供 owned 类型化特征；外部 provider 使用相同入口。
Grid 使用目标域正间隔和有限 origin，最近格点按 ties-to-even 求取。
GridCapture 复用公共 surface 距离准入；
ExactProjection<NearestGrid> 重验冻结原始输入的最近格点关系。
独立轴分别捕获，有共享自由度的旋转或斜向网格贡献一个耦合候选。
操作控制点的线性影响用于求修正，全批次保留唯一锚点的同一操作修正。

有界 sink 的 `push` 表示策略已断言准入成立；
`propose` 表示请求框架先执行其获授权的准入过滤。
后者的合法 false 为不参与，错误、panic、越权身份及非法字段仍拒绝整次求解。
这样可以过滤阈值外的正常网格位置，并把准入求值、依赖检查和统计留在框架内。

GeometryProvider 从 committed 四角贡献真实边及中心平行线；
GuideProvider 消费应用提供的稳定身份和目标域端点，不创建持久关联。
Geometry／Guide 是同一个公开 LineSnap 协议的类型化实例；
独立第三方 line marker 和 provider 已使用同一路径验证斜向耦合对齐。
surface 轴线建立排序索引，斜向真实边走一般线查询，没有用 AABB 替代。
比例 resize 的两个网格方程退化为一个自由度时，先求非零方程，
再检查两轴残差。一致的投影保持固定控制点；不一致则合法不参与，
不分别改写两轴造成比例漂移。该策略自由度现已接通下述 resize 工具。

SnapStrategy 提供默认返回 false 的 `reuse_prepared` 受检回调。
未声明可复用依赖的外部策略仍每次 prepare。
Grid 对比全部 GridSpec 身份和配置；LineSnap 对比全部线身份、点与法向量，
包括新增与移除成员。策略代际或 version 变化不能复用旧资产。
仅指针变化或 provider 排序变化时，已通过比较的资产可直接 query；
provider 替换或线几何变化则重新 prepare。失败的求解不保留本次部分缓存。
该实现每次仍贡献并核验完整特征集合，不能据此声称整次输入处理为 O(log N)。

特征身份检查和候选来源查找改用 HashSet／HashMap，
排序仍使用显式顺序和稳定键，哈希遍历不参与平局判定。
反馈预算在接收贡献时检查，合法冲突或准入过滤不能使超预算贡献获得部分成功。

## 实际移动计划与反馈

此前 MovePlan 只有 header、feedback 与 Command 构造，无法证明布局最终采用的位置
仍满足吸附事实。现新增 `actual_geometry`：应用声明目标 content 域中、
与 header 源集合同序的完整模型约束。普通未吸附计划允许返回 None；
启用吸附时缺少该声明会拒绝，不假定任意平移都合法。
应用 Command 必须消费同一组约束，这仍是开放应用计划的扩展合同。

ToolContext 的 `resolve_snapped_container_move` 先通过普通 typed policy 请求
MoveLayoutConstraints，取得允许位置的只读 RuleCall，再创建原始输入并求解一次。
唯一修正进入全部 source 的同一 surface 平移；原始 pointer、targeting 和候选身份保持。
随后取得 E03 完整计划及真实父权限，检查其实际角点能否表达为全批同一平移，
并以实际角点求值全部布局和已采用事实。不能只核对调用者提供的标量修正，
也不能以容差内的重建角点代替计划实际角点。
布局允许的改写若保留全部事实则使用实际几何；破坏事实则拒绝，不重新求解。

`SnappedMovePlan` 保存上述 checked plan、吸附事实与实际几何。
ContainerMoveTool 可通过 `with_snapping` 消费相同 Viewer 注册表，
每个手势拥有独立 session。反馈发布前再次核对 plan 声明和所有依赖；
release 按本次最新位置重走完整流程，清理成功后最后检查事实，再构造一次 Command。
错误不降级为未吸附提交；取消、工具切换和成功提交释放 session。
工具暂存计划使用一个 Box，增加的分配必须计入后续完整事件成本。

FeedbackRenderer 通过 typed payload、实例身份与同一个注册表安装。
每类反馈 payload 只有一个 renderer；重复安装、foreign 和 retired 身份返回错误。
替换或移除使旧结果失效。所有采用声明在发布前渲染成 owned Overlay visuals，
渲染错误保留原始 source；panic 定位到 `snap.feedback.render` 并 fault Viewer。
渲染超预算不发布部分反馈。
完整发布批次同时计入应用计划和吸附 renderer 的 visuals，合计超出预算也整体拒绝。
Geometry／Guide 贡献普通 ReferenceLine 声明，ReferenceLineRenderer 用实际几何
计算线段范围，样式、线宽与延伸距离由消费者显式配置。
配置在回调中保持不变，修改必须使用受检替换；隐藏捕获状态的修改不属于可检测行为。

## 创建与 drop 的实际计划

CreationPlan 新增 `actual_bounds`，由应用声明 Command 实际消费的目标父域矩形。
普通创建允许返回 None；吸附创建要求有声明，并把受检快照保存在
CheckedCreationPlan 中。每次发布或提交前重新核对 header、实际矩形和全部吸附依赖，
回调改写声明或最终布局破坏已采用事实时拒绝，不重试、不提交旧预览。

创建输入不借用移动源的资格。click／drop 只有完整模板的父域平移自由度，
保持模板尺寸；拖拽创建只有当前两个可变边，保持原始 press。
线性影响使用完整父域到 surface 映射；Grid 捕获拖拽端点，固定点不参与改写。
修正不能跨过固定起点而翻转已声明的边。最终检查使用实际矩形的真实四角，
而非仅核对调用者传入的修正标量。

CreationLayoutConstraints 是普通 typed policy 请求；目标可贡献允许位置的 RuleCall。
ContainerCreationTool 的 `with_snapping`、ToolContext 的受检创建／drop 服务，以及
EditorDomain 的 `drop_into_container_with_snapping` 共用 Viewer 注册表。
drop 保留未吸附 pointer 的 targeting 与目标父域 grab offset，没有移出权限查询。

创建起点现在持有独立手势身份。一次手势内的 pointer event revision 继续递增，
不会使连续预览误判为另一手势；每次候选计算仍有唯一 CandidateId。
stationary zoom refresh 重建当前输入和反馈，release 按最新端点重新计划。
release 的清理失败使 session 失效，并阻止构造 Command；移动直接提交入口在构造失败时也取消 session。

独立非 cfg(test) 消费者 `checked_plan_snap_probe` 使用第三方 Move／Creation
协议及普通 ReferenceLineRenderer，执行实际计划复验、反馈发布、清理与提交入口。
该探针的合法计划返回 None，不产生模型 Command；模型变更与 undo／redo 由容器
integration 验证，两个证据范围分别统计。

## resize 的实际计划与工具

既有 ChangeBounds 入口没有实际计划几何，不能证明布局改写后吸附事实仍成立。
本轮新增开放 `ResizeProtocol`／`ResizePlan`，通过普通 typed request 在原父 policy
聚合计划。捕获的 ResizeStart 保留源身份、原父关系、模型 revision、双方 policy
generation 和应用声明的 ResizeShape；resize 不查询移动或换父权限。
ResizeCandidate 与 SnappedResizePlan 均由框架构造，外部只能读取。

ResizeShape 明确几何域中的矩形、几何到父域的映射、最小尺寸及可选宽高比。
单源八方向按允许边开放自由度，固定边的线性影响为零。
比例角拖拽使用一个宽度修正，同时按比例修正高度；
原始指针位移先投影到该 surface 方向，之后仍经同一策略仲裁。
比例必须与非零源矩形一致；比例侧边拖拽结构化拒绝。
居中及多源 resize 未在本切片实现，不宣称覆盖 GEF 的这些变体。

ResizeLimits 通过普通 registry 安装，以 owned 参数求值最小尺寸。
ResizeTool 也接受相同 payload 的第三方求值器；外部消费者在最小尺寸之外增加
最大宽度限制，未增加内置专用 hook。
原父 policy 可通过 ResizeLayoutConstraints 返回额外布局 RuleCall。
实际计划必须返回 Command 消费的矩形。最终检查使用完整仿射映射的真实四角，
从几何恢复自由度并验证固定边、比例、布局和全部采用事实；
实际布局改变允许边却破坏 Guide 命中时仍拒绝。

ResizeTool 的 Handle 与程序化入口共用捕获、预览、refresh 和 release 流程。
stationary zoom 使用当前映射重新计算，不累加旧修正；
release 使用本次最新位置，经过一次完整计划和最终校验，清理成功后构造至多一个
带模型 revision 守卫的 Command。取消或模型变化不能被后续指针恢复。
resize、移动和创建的取消路径均先取消 session，再清理 Overlay；
即使清理本身失败，旧 receipt 也已失效。

公开非 cfg(test) 的 `checked_resize_snap_probe` 使用第三方 ResizePlan、
第三方 limits 求值器、Guide 和普通 ReferenceLineRenderer，
验证 release A→B、一次 Command、实际模型矩形及 undo／redo。
额外 integration 覆盖八方向、旋转映射、比例、最小尺寸、实际改写、
stationary zoom、typed Handle、foreign／retired rule、取消与释放回调失效。

## 移动与创建组合补验

本轮增加五项容器 integration，分别验证混合父缩放／旋转后的一致 surface
平移、静止指针的 scroll／zoom／resize 重算、移动和创建的取消／切换／模型发布、
创建布局规则拒绝网格平移，以及 release query 内配置变化。
创建的原始位置满足布局规则时，拒绝网格候选后仍可采用该合法位置；
这与已采用事实被实际计划破坏时的整次拒绝不同。

混合父测试通过真实 ContainerMoveTool release 执行一次模型 Command；
静止指针测试核对最新预览、release 一次计划及 undo／redo。
release query 修改策略 version 时，移动和创建均只 query 一次，
不构造 Command，不重新建立手势基线。
显式取消入口代表宿主调用契约，尚不代表 Native／Web capture lost 已验收。

## 定向证据

环境：macOS，`rustc 1.99.1 (58cf8f9c3 2026-10-13)`，debug。

| 验证 | 结果 | 覆盖边界 |
|---|---|---|
| Editor `--lib snap::tests` | 32 项通过 | 协议、注册生命周期、旧结果失效、网格中点与溢出、比例退化方程、固定控制点、唯一锚点、真实边、斜向第三方线、缓存增删和 renderer 生命周期 |
| 公开 integration target `p2_e04_open_snapping_contract` | 20 项通过 | E03 实际候选、内置和外部消费者、八类回调 Error／panic 矩阵、最终规则失败零重查、视口及模型失效；移动／创建／resize 实际计划、ReferenceLineRenderer、resize 固定边／比例／最小尺寸与生命周期 |
| `p2_e03_container_editing_contract` | 46 项通过，3 项性能测试默认忽略 | 既有容器契约，加上吸附 release 最新位置、单次历史与撤销／重做、实际布局破坏事实、缺失几何、多源失真、清理失败、renderer Error／panic、开放布局规则；创建固定起点／尺寸、最新端点、zoom refresh、缩放／旋转 drop 与整组反馈预算；混合父、静止指针重算、取消和 release query 配置失效 |
| `--doc snap::` | 4 项编译失败断言通过 | 外部不能修改受检输入、采用事实、resize 基线或实际计划 |
| Editor `--lib --no-deps` Clippy | 通过 | 本包新增实现 |
| 两个公开 integration target `--no-deps` Clippy | 通过 | 外部 API 消费代码 |
| 独立消费者 Native／Wasm `--lib` check | 本轮两组通过 | 编译完整 `open_snap_probe`、`builtin_snap_probe`、`checked_plan_snap_probe` 和 `checked_resize_snap_probe`，覆盖移动／创建／resize 实际计划、第三方 limits 与 renderer |

integration fixture 同时由独立 `editor-open-protocol` crate 编译；
两处引用属于同一组行为测试，不重复累加测试数量。
Native／Wasm check 仅证明已消费的公开 API 可编译，没有执行真实 Web capture 或 Native 输入。
本轮文档门禁检查 494 个 Markdown 文件通过，差异空白检查通过。

本轮 workspace fmt 和 check 通过。最终 workspace Clippy 再次被下述三项既有
Core 告警阻断，按分层门禁不继续启动更宽的 workspace test。
日志见[workspace check](../evidence/p2-e04-2026-11-02/workspace-check.log)和
[workspace Clippy](../evidence/p2-e04-2026-11-02/workspace-clippy.log)。

未限制依赖的 Clippy 首次在既有 Core 代码报出新版工具链的三项告警：
`render/svg.rs` 的 `chunks_exact_to_as_chunks`，
`runtime/update/deferred.rs` 与 `listener.rs` 的 `drain_collect`。
本记录不把 `--no-deps` 通过写成 workspace Clippy 通过。
另一次消费者编译遇到共享工作区正在写入的 Core staged registry 文件，
该阶段完成后消费者编译通过。

## 尚未完成

SnapSolution 仍只证明候选仲裁，公共 `validate_snapping_geometry` 仍只验证标量几何。
移动、创建／drop 和 resize 已通过各自不可伪造计划连接实际模型几何与事务；
单源八方向及比例角 resize 已通过定向验证，居中与多源变体不在本切片。
创建与 resize 的 zoom refresh、创建的缩放／旋转 drop，以及 ReferenceLine
实际发布和整组反馈预算已有定向证据；移动端 stationary refresh、旋转／混合父、
释放回调配置失效和创建布局规则已有联合验证，平台 capture、DPI 与其余矩阵组合仍保留。

已增加无窗口正式采样入口，区分 candidate、冷／热实际计划、
反馈清理和整次 release，使用真实 Instant 与线程分配计数。
完整 owner 扫描及独立版本检查发生在每个求值回调周围，
不能把候选索引的复杂度当作完整事件成本。
复制字节没有独立埋点；分配请求字节不等于实际复制量或留存内存。
采样结果与限制在下节记录，平台耗时仍待测。
本记录已按 technical-writing-clarity 和好好说话回读状态、依据及中文；
没有独立 reviewer，不把自审作为整体实现验收。

注册生命周期、失败位置与预算已补验，完整矩阵仍需逐项核对；
SN-14／20 的完整成本，以及 E04-C 剩余项／D、
workspace 最终门禁和平台输入保留未完成。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 成本续项与评审入口

正式 release 采样程序已完成六组 N／源数场景；八次预热、十六次正式样本，
使用真实 Instant 和线程 allocator。环境为 macOS 14.7.8、MacBookPro18,1，
Rust 1.99.1；共享主机同期有编译活动，数据不是安静机器基线。
目标 sibling 中一个与当前锚点接近，其余位于捕获范围外；
下表不能代表密集候选最坏情况。单位为毫秒。

| sibling N | 移动源数 | 冷实际计划 P50／P95 | 热实际计划 P50／P95 | 完整 release P50／P95 |
|---:|---:|---:|---:|---:|
| 64 | 1 | 12.10／12.74 | 12.47／30.44 | 11.50／11.82 |
| 64 | 64 | 23.58／25.28 | 23.83／25.84 | 25.99／26.87 |
| 1024 | 1 | 166.55／181.37 | 165.14／178.19 | 173.74／175.28 |
| 1024 | 64 | 255.21／313.13 | 252.30／273.85 | 268.49／641.31 |
| 10000 | 1 | 1741.87／2810.69 | 2029.01／2114.33 | validation 预算阻塞，0 个正式样本 |
| 10000 | 64 | 2797.49／3053.99 | 2721.91／2904.89 | validation 预算阻塞，0 个正式样本 |

冷／热场景和完整 release 独立采样，不按阶段相加解释完整 release。
各组最后一次已采样求解均记录 24 个仲裁候选、2 个采用事实及 2 条反馈声明，
59 次规则求值、468 次 owner 检查和 6368 次版本回调。
热场景 prepare 为 0、复用命中为 1、query 为 1，特征数为 6N。
这证明 prepared 复用工作，但完整来源检查和依赖校验仍是关键成本。
空候选的先前样本只有 40 次 owner 检查，不能代替实际命中结果。

10000 sibling 的单源热计划每次请求约 426.6 万～458.7 万次分配，
累计请求约 17.20 亿～18.49 亿字节；这些是累计请求，不是同时留存内存。
同组反馈发布／清理 P95 为 131.93 ms，64 源为 204.97 ms。
复制字节、坐标查询次数和各阶段 owner 扫描耗时仍未完整埋点，
现有计数不能代替这些指标。
原始[正式日志](../evidence/p2-e04-2026-11-02/cost-final.log)、
[阶段汇总](../evidence/p2-e04-2026-11-02/cost-summary.json)和
[采样范围与复现](../evidence/p2-e04-2026-11-02/README.md)已保留。

prepared 命中没有消除完整集合校验成本，当前数据不支持交互性能毕业。
10000 sibling 的完整 release 触发
`NonConvergingValidation { budget: 10000, .. }`，
Runtime 使用固定 validation 工作上限，本轮未改 Core 或绕过 stabilization。
这不是 preparation 预算，也不能解释为递归深度超过上限。

后续建议见[校验成本整改提案](../../design/editor/snapping-validation-cost-proposal.md)。
提案待评审，先保持现有检查边界并减少 owned 集合重复分配；
减少扫描次数需要另外证明全部相关变化均可检测。
该方案不授权 Core 改动，不把性能采样通过记为 E04 平台或整体毕业。
