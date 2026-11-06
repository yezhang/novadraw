# 图形框架公共 API 契约

类型：`normative-design`

能力归属：Core 公共协议与跨包权限；Editor、Inspector、backend、platform 按各自 owner 消费和实现边界。

规范效力：`accepted`（2026-10-12，五项架构修订已获确认）

实现状态：既有入口以 ADR-031/033/035 和完成记录为准；本次补充契约的验证边界见
[修订记录](../../verification/reviews/framework-architecture-contract-revision-2026-10-12.md)。

本文是公共 API 的当前规范入口。本文定义角色、权限和
组合规则，不冻结全部 Rust 签名；具体方法由领域规范和公开 API snapshot 维护。

Native 可选 `perfetto` feature 仅接入官方 SDK 的 CPU 埋点。
Core 不持有采集会话、聚合器或请求缓冲，宿主初始化 producer 并注册 category，
默认及 Web 构建不链接原生 SDK。领域签名不暴露 SDK 类型；
独立文字引擎使用相同阶段入口，详见[观测合同](perfetto-observation.md)。

## 1. 模块、所有者与发布包

模块表达领域职责，对象表达状态所有权，package 表达依赖与发布边界。
Runtime facade 是短借用权限视图，不是独立服务 owner。六包边界沿用
[ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md)。

| 领域 | 所有者或输入 | 输出与权限 | 禁止职责 |
|---|---|---|---|
| Geometry | owned 几何值 | 受检值、纯变换与算法 | Figure 身份、GPU、调度 |
| Graphics | 借用文字服务与 recorder | Measure/Paint 阶段的图文操作 | 拓扑、damage、frame 身份 |
| Text | Runtime 的文字服务；独立绘图由组合根拥有 | 不可变 TextLayout、metrics、轮廓 provider | Editor draft、GPU 数据格式 |
| Figure | 具体 Figure 私有内容与算法配置 | Figure 行为、typed update、owned descriptor | parent/children、全局服务 |
| Tree | Runtime 拥有节点与拓扑 | Builder、只读查询、身份校验 | 平台循环、attached 写权限逃逸 |
| Layout | 节点拥有 manager 和 parent-child constraint | Snapshot → checked placement/measurement | 直接修改树、通知或共享模型 |
| Connection | Runtime 的绑定、依赖和 routing service | RouteRequest → checked group output | 业务连线规则、Editor history |
| Runtime/Update | 单场景可变服务 | source commit、稳定化、damage、journal、帧出版确认 | 业务 schema、具体 GPU 实现 |
| Animation | Runtime 的 timeline/presentation plane | typed binding、可撤销表现覆盖 | 写回 committed source |
| Render | recorder、submission 与资源版本 | 后端消费的稳定信封 | 排版、命中、模型编辑 |
| Host/Platform | 窗口、输入、时钟和调度 | 归一化输入、IME/AT、平台生命周期 | Figure callback 和布局机制 |
| Editor | 模型、CommandStack、Part/Tool/Policy | 模型命令与视觉投影 | Core 提交旁路、强制业务 schema |
| Inspector | 稳定事实与订阅 | 只读 snapshot 和诊断 | 新的提交权威、隐藏 fault |

平台 adapter 管理 native window/canvas 的生命周期及可用性；backend 管理其 GPU
surface/device 配置、提交和恢复。组合根衔接二者。Core 只接受结构化 surface 信息与
确认结果，不拥有具体句柄；创建、resize、suspend、重建和释放的责任不能重复归属。

## 2. 唯一领域路径与对象角色

Core root 只公开领域模块，不提供库级 prelude 或类型平铺重导出，见
[原编号 035 的批准依据](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-035-explicit-domain-imports-and-root-boundary.md.txt)。
应用可以组织自己的 local prelude；API snapshot 必须反映真实公开路径。

| 规范路径示例 | 角色 |
|---|---|
| `geometry::{Point, Vec2, Dimension, Rectangle, Insets, Affine2D}` | 几何值 |
| `graphics::{Graphics, PaintContext, Paint, StrokeStyle}`；`text::MeasureContext` | 图文操作与阶段权限 |
| `figure::{Figure, FigureComponentUpdate}` | 具体行为与私有内容更新协议 |
| `tree::{FigureId, FigureTree, FigureTreeBuilder}` | 场景身份、查询和构建 |
| `layout::{LayoutManager, LayoutSceneQuery, LayoutOutput}` | 布局算法及受检结果 |
| `runtime::{Runtime, FigureMut, FramePreparation}` | 提交者、借用权限和帧 outcome |
| `text`、`connection`、`animation`、`event`、`container` | 各领域协议、身份和服务配置 |
| `render`、`host` | 后端与平台集成合同 |

值、实例身份、扩展行为身份、不可变结果和权限视图不互相替代。
`FigureId`/资源身份/EditPartId 不合并成裸 NodeId；`CapabilityKey<C>` 等 typed key
表达行为合同，字符串只作诊断。`Clone` 不授予 attached 写权限或 namespace 迁移权限。
共享 concrete model 不要求进入 registry；独立消费者需要发现可替换实现时才登记能力。

## 3. 生命周期与统一 Figure mutation

| 阶段 | 入口 | 权限 |
|---|---|---|
| detached | owned Figure/Layout/Border/Router 的构造与 `with_*` | 配置自己的值 |
| build | FigureTreeBuilder | pre-Runtime 拓扑和初始 NodeState |
| attached | `runtime.figure(id)?` → FigureMut | 节点/容器操作、typed component/capability update |
| query | FigureTree 或 stable query | 读现有事实，不隐式执行 layout |
| drive | `frames/resources/input/...` 等 Runtime 子 facade | 对应领域协调，仍由同一 Runtime 提交 |

Border 的原始配置构造与更新按
[ADR-056 输入不变量](detached-input-invariants.md#border-构造保护真实配置)返回领域错误。
受检值构造成功后仍需走 attached 候选发布协议。冻结描边入口的受限存储规则见
[Border 契约](reusable-shape-border.md#61-不可变可复用策略)。
Builder 拥有 pre-Runtime 树写权限，不垄断 owned 值的独立构造；
attached 领域错误不能由只验证、不提交的公开伪入口掩盖。

内置和外部 Figure 使用同一 identity、prepare、effect validation、commit、revision、
damage、notification 与错误通道。具体类型不获得 `runtime.label/image/viewport` 等
专用 mutation facade，也不获得闭集 adapter 快捷入口。具体私有属性用 typed component
update；跨类型行为用 typed capability update。

Viewports/Layers/Connections 等 facade 表达关系和服务职责，不能据此重建 concrete
Figure setter 集合。它们按领域身份及能力校验，不能要求目标属于内置类型闭集。
facade 获取和 Drop 都不提交操作，不拥有第二份状态。

prepare 必须读本次提交前缀的当前值。显式 `NoChange` 不执行 commit，不提升 revision，
不制造 derived work、journal、damage 或资源/结构 effect。相同值判断由 typed
实现表达；Core 不用浮点 epsilon 或全局字段比较猜测语义。
资源和结构 effect 只声明受限意图，其 namespace、目标权限及整体原子性由 Runtime
在任何写入前校验，详见 [组件更新协议](component-update.md)。

### 3.1 Context 阶段权限投影

Runtime、FigureTree、Viewer 与 CommandStack 是 owner，不进入扩展 callback。
callback 只接收当前阶段授权的 Context、Query、Services 与 Output/Effects/Commands：

```text
owner freeze causal prefix
→ construct phase query and effect/output sink
→ invoke callback
→ release callback borrow
→ validate and commit through owner
```

公开命名使用 `<Phase><Subject><Role>`。Event 使用
`EventContext + EventSceneQuery + EventEffects`；Layout 使用
`LayoutSceneQuery + LayoutOutput`；Connection routing 使用
`RouteSceneQuery + RouteOutput`。`StableSceneQuery` 查询最新 stable epoch。
`PaintContext` 与 `MeasureContext` 已直接表达阶段权限，不为了形式统一增加
`PaintSceneQuery` 或万能 Context。

`LayoutSceneQuery::input` 将布局实际使用的私有模型读入 Runtime 受检会话，
返回 `figure::PreparationInputSnapshot`；内置和外部布局共用
`PreparationInputRegistry`。完整首次值、注册弱引用、预算和不可清除的读取失败
沿用 preparation 输入证明。普通 Figure 不需要声明 preparation capability。
Runtime 的嵌套 preferred／minimum 测量保留同一查询，不能退回无证明的 manager
调用；缺少输入证明的旧测量缓存不能为这一调用提供发布依据。
经受检查询计算且没有准入私有输入的尺寸缓存，只在本轮 input scene 尚未准入时
保留旧的纯几何复用行为。准入以后继续真实测量回调与完整 input 检查，
不能由已存 proof 相等推断未知回调没有副作用。
没有受检会话的兼容查询返回 `LayoutError::InputSessionUnavailable`，
不会直接执行无预算 reader。实际输入错误由 `LayoutError::Input` 保留原始原因。
该接口不授予局部失效资格，具体队列合同见
[按属性依赖限定布局重验](property-scoped-layout-revalidation.md)。

输入证明内核按阶段参数化 reader 的 scene 与错误类型。preparation 保留现有公开
`PreparationInputRegistry`，受检投影使用独立的受限 reader；二者共用完整值比较、
注册弱引用、首次证明合并、活动读取环检测与留存 lease。阶段参数化不开放新的读取
权限，也不授予未知回调省略权。活动读取环返回
`PreparationInputErrorKind::DependencyCycle`；reader 在返回前检查本 scope 的锁存
失败，嵌套错误被忽略时仍拒绝外层成功值。验证 scope 的嵌套 proof 回传调用方，
新出现的 subject 加入本次最终检查，不跨 reader 持有收集器借用。
受检投影的公开入口和错误包装随
[通用投影实施](figure-tree-projection-separation.md#下一主线的公开合同候选受检-child-transform)
继续落定；独立坐标与命中／事件已采用 provider，Runtime 帧消费仍待接入。

`figure::FigureProjection::project(&self, &ProjectionScene<'_>)`
返回 `Result<ProjectionContribution, ProjectionError>`。
`ProjectionContribution::new(ChildTransform)` 只保存额外 child transform，
不包含 placement、insets 或 world transform。`ProjectionCapability::of::<T>()`
经 `PROJECTION` typed key 注册；只有匹配的 provider 与 `CONTAINER` adapter
同时解析成功才准入，否则返回
`FigureCapabilityRegistrationError::InvalidCapability { capability, reason }`。
缺失与非法注册不是同一种情况。

`runtime::PreparedFigureUpdate::with_capabilities(FigureCapabilityBuilder)` 把完整
descriptor 注册表作为更新候选；省略的 capability 表示显式移除，未调用此方法则
保留当前注册表。此方法使更新成为 changed，并要求 layout／geometry／paint 失效。
Runtime 在组件及 effects 提交前检查 projection／container 关联、现有 child policy
与附着角色。child policy 变更，以及 Connection、decoration、resource、border、
layer、Freeform、scale 等附着角色的新增／移除，继续使用对应子树事务；
本入口不创建或退休其他领域对象。resource 值变化仍须声明原有 resource effects。
检查失败经 `ComponentUpdateError::Effect`／`CapabilityUpdateError::Effect` 中的
`RuntimeMutationError::Graph` 保留 registration 原因，不写入组件、版本或注册表。
成功时组件和注册表共用一个 component revision，typed 成员索引同步更新，
旧 descriptor 在内部写入结束后释放；不在 commit 后重新调用注册回调。
projection 显式移除成功后才恢复实时 container 来源。

`ProjectionScene` 由 Core 建立，外部只接收借用。当前读取方法为
`root`、`budget`、`parent_id`、`children`、`origin`、`size`、`insets`、
`resolved_style` 和 `input`。任意 ID 先验证身份，再验证 owner 子树范围；
owner 的 parent 只返回关系身份。它不提供原始 tree／Figure、mutation、
坐标、命中、route、prepared drawing 或 measurement 入口。
Core 事实现记录完整身份、成员检查的 parent 路径、有序 children（含空集合）、
origin、size、insets、逐级原始样式与 provider owner 的 component revision。
origin 与 size 分开登记；相同解析样式下的继承来源变化仍可识别。
独立 reader 验证 scope 的 Core 事实也回传消费闭包。
独立坐标、命中与 Runtime 输入已接入；局部表现变换读取及 Runtime 帧消费仍待完成，
当前不可变借用不授予跨查询复用权。

`ProjectionInputKey<P>` 与 `ProjectionInputPayload` 复用 preparation 的 typed
key／payload 合同；`ProjectionInputReader<P>` 只接收 `ProjectionScene`，
返回 `Result<P, ProjectionError>`，不能直接绑定旧 preparation reader。
`ProjectionInputRegistry::new(retained_bytes)` 显式设置 owned snapshot／proof 的
逻辑字节留存限额；`register(key, reader)` 返回 `PreparationInputError`，
`retention_usage()` 返回 `Result<TextContentUsage, ProjectionError>`。
clone 共用注册身份与留存账本，旧 proof 只持有 reader 弱引用。
当前 `ProjectionError` 保留 `Identity`、`OutsideRoot`、`Input`、`Budget`、
`Retention`、`InvalidTransform` 与 `CoreChanged { figure, fact }` 原因；
CoreChanged 表示本次首次 Core 事实被冲突值替换，fact 仅为诊断文本。
字体 family、tooltip 与 children backing 在复制前计入本次预算。
坐标包装、来源替换和独立查询预算已经冻结；
整帧预算与其实际消费者继续在接入前冻结。此处不代表整片验收完成。

`LayoutManager::register_child_layout_dependencies` 在 manager 准入时接收
`layout::LayoutDependencyDeclaration`。外部和内置布局使用相同 typed property
声明入口；表由 `LayoutState` 持有，重复声明、非法 child 与预算错误经
`LayoutError::DependencyRegistration` 拒绝原子安装。
子树中预留的直接 child 可在整体发布前验证，所有 manager 共用准入预算。
声明只描述依赖，不直接改变失效范围；局部请求仍需按上述方案提供 guard，
并在执行前确认当前父链与完整输入。

`layout::LayoutRevalidationGuard::new(registry, key, source, first_input)` 记录
owned 首次输入和原注册的弱引用，构造时不调用 reader。
`EventEffects::revalidate_layout_for_property(key, old_value, new_value, guard)`
只记录内部布局请求，不自动发送属性通知。未知祖先依赖或 source 不匹配时
保持完整父链失效；普通失效优先，同属性合并保留首次 guard。
执行前在本轮预算内准入首次值并重读，值不同则升级普通失效，原始读取错误仍传播。
采用以后沿原 first proof 检查布局消费及发布，不能读新 first 来接受旧输出。
已接入真实事件入口及 ScrollBar／ScrollPane／Fill／Stack 消费者。
完整 range 输入在可替换 thumb 的测量回调之前读取；非 value 请求 guard 与实际
消费 proof 分开。重复可见值写入不扩大范围，实际可见性改变继续完整传播。
Runtime 的既有恢复机制覆盖普通布局的晚期输入冲突；
`FigureTreeBuilder::validate_subtree` 同样使用有预算的输入 session，
独立的无 session measurement 查询继续拒绝私有输入读取。
隐藏布局的旧 proof 不参与当前发布；可见父布局实际测量取得的 proof 仍检查。
生产净成本与混合 FigureTree 验收尚未完成，证据见
[实施记录](../../verification/reviews/figure-tree-projection-implementation-2026-11-05.md)。

`MeasureContext::cache_identity()` 只读返回文字引擎的可缓存实例身份，
沿用 `TextLayoutEngine::cache_identity()` 的 opt-in 合同。外部组件只有在身份存在、
`revision()` 相同且所有实际输入相同时才能复用布局；缺少身份时完整计算。
身份区分独立引擎和字体集合，不能仅用相同 revision 跨引擎复用。
该查询不授予修改文字服务、字体资源或阶段预算的权限。

Query 是短生命周期只读借用；Snapshot 是可保存的 owned revision；Output 是待校验完整
候选；Effects 是 callback 后按因果顺序提交的意图。Context 和 Query 不返回 Runtime、
FigureTree、FigureNode 或同步 mutation closure。完整决策见
[阶段权限投影](context-phase-authority.md)。

## 4. 命名、单位与失败

`with_*` 配置 owned value；`set/clear_*` 修改或移除配置；`take/into_*` 明示取走或
消耗；`*_later` 只排队；`measure/layout/prepare` 是显式计算，允许合法私有缓存变化。
`fill/stroke/draw` 追加命令，不代表 GPU 完成或窗口呈现。

Point 表示位置，Vec2 表示位移，Dimension 表示尺寸，Rectangle 表示矩形。
坐标域、角度单位、源图像像素与目标逻辑坐标必须出现在参数命名或合同中。
不要求所有坐标都引入泛型单位框架。

| 边界 | 失败或状态语义 |
|---|---|
| 受检值构造 | Result；非法值不产生有效对象 |
| 安装已验证的值 | 可无 Result，不添加没有失败含义的包装 |
| attached mutation | 领域错误；`Ok(false)`/NoChange 仅表示合法不变 |
| capability discovery | 无效身份为错误；合法缺失为 `Ok(None)` |
| 任意 FigureId 查询 | 先校验 owner 与 live membership；foreign/unknown 为错误，合法缺失才是 None |
| 坐标查询 | 按 ADR-038 区分身份错误、数学奇异与几何计算失败；正向 Result，逆向 Result<Option<_>> |
| 布局测量 | Result 保留查询和测量失败；owner 在组合、投影及缓存写入前校验尺寸与 baseline |
| deferred mutation | 入队不等于提交；错误从 deferred outcome 观察 |
| paint | GraphicsError 原因传播；失败帧不出版，状态栈仍复原 |
| frame/backend | 保留 Idle/Suspended/AwaitingCompletion/Error 和完整确认/恢复分类 |
| 扩展 panic | 进入 fault 边界；不声称回滚私有 interior mutation 或外部副作用 |

[Detached 输入不变量](detached-input-invariants.md) 的样式边界使用
`graphics::Opacity` 与 `text::FontDescriptor`，原始输入在构造时受检。
`Graphics::set_opacity` 安装同一个 Opacity；动画通道也消费此值，旧
`animation::Opacity` 路径退出。GraphicsInputError 保留非有限／越界原因，
动画原始输入入口经 AnimationError 的 source 传播。

Detached Figure 的原始 bounds、stroke、corner 和平移入口返回领域 Result，
Bounded::set_bounds 也执行相同检查。BorderedFigure::replace_border 使用
`figure::border::BorderCandidate`；它将共享 Border 和当次基础指标一起捕获，
Runtime prepare 与 commit 消费同一候选。owner 指标、动态外部输出和挂载关系仍由
各自的发布前检查负责，见 [Detached 输入不变量](detached-input-invariants.md)。

查询的目标合同以 [ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)
为准。P2-S07 的布局测量、五个坐标入口和 Route 身份切片已经迁移 Result；
Tree 结构、状态、关系、专用可选查询与 workspace 消费者也已迁移。
Node 非权威尺寸 getter 已退出公开表面；行为与最终门禁登记在
[实施核验](../../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)。

## 5. 计算、发布与开放策略

生产帧出口由 Runtime 的 frames prepare／complete 协调，FigureTree 不驱动更新队列。
测量使用结构化约束；preferred 返回尺寸与 baseline，minimum 返回 Dimension，
不使用 `-1` sentinel，失败按查询专题解释。

source/resource → intrinsic/constrained measurement → layout geometry → route →
post-route geometry → final presentation → damage/recording → submission。
阶段输入必须携带足以判断依赖有效性的版本；旧测量、旧约束或旧资源不能混进新稳定帧。
中间输出由 Runtime 校验后提交工作状态，最终表现完成后才晋升 stable epoch。
详细版本与发布合同见 [派生状态收敛](derived-state-convergence.md)。

固定的是 scheduler、身份/权限校验、完整输出提交、状态隔离、稳定通知和出版确认。
开放的是 LayoutManager、容器投影、child clip、精确命中/搜索与内容 presentation
策略。策略只能提供受检输入/输出，不能重入 Runtime 或重新排序阶段。

Draw2D 的 self → children → border 是默认兼容绘制模板；本次沿用现行递归模板。
未来真实消费者需要其他合成次序时，必须先形成受控、类型化的合成合同，证明 clip、
damage、hit 和状态隔离，再评审实现；它不是对安全任意 renderer hook 的承诺。
几何搜索与输入资格共享遍历内核，但 enabled 只约束输入/焦点策略，不是全部几何
查询的硬门禁。现有支持及新增验证要求见 [树查询规范](tree-search-and-focus.md)。

## 6. 组合与外部消费者门禁

Figure/Border/Paint/Layout/Router 可以独立构造并组合；attached 后由 Runtime 协调。
多个 facade 调用是按顺序提交的多个操作，不能用普通 closure 宣称整体回滚。
跨对象原子性仅由可完整预检的具名操作或受限 effect batch 提供。
Editor Command 改模型，Viewer 投影视觉；独立 CommandRecorder 只产生资源闭合的
RecordedDrawing，不分配 Runtime frame/session 身份。

外部消费者必须能在不修改 Core 枚举或类型分支的条件下：

1. 以显式领域路径构建场景，使用相同 FigureMut 更新内置与外部 Figure；
2. 验证 prepare 读取连续提交后的当前值及 NoChange 的零副作用；
3. 验证非法资源/结构 effect 在 commit 前拒绝且不部分修改；
4. 使用外部 Layout/Container/Presentation，证明同帧 measurement、bounds、clip、
   glyph 与 damage 版本一致；
5. 复用 Graphics/Figure 的 PaintContext，并完整处理 frame/backend outcome。

现有 suite 只证明其已覆盖的合同；新增要求须先冻结对应契约测试再实施。
规范效力和执行状态分别维护，本文不把这些要求全部标记为已验收。

## owned 与 scoped

owned 表示值具有自己的所有者和转交边界，不依赖外部借用生命周期；不表示必须
堆分配、绝对独占、不可 Clone 或永远可变。owned value 内部可共享不可变资源。
值移入 Builder/Runtime 后，authority 随所有权转移；修改其他 clone 不能反向改变
已录制命令或 attached state。

scoped facade 的有效范围受一次 Runtime 借用限制。它只持有受限权限，
不能比 Runtime 活得更久，不能保存第二份树或更新队列；获取及 Drop 都不提交操作。
借用结束后状态仍归 Runtime。具体组件提交顺序见
[组件更新协议](component-update.md)，生命周期见 [原编号 019 的批准依据](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-019-composable-api-and-scoped-editors.md.txt)。

## 受检投影的独立坐标查询增量（2026-11-06）

能力归属：Core FigureTree。五个 transform 返回类别保持不变，
`CoordinateQueryError` 新增 `Projection(Box<ProjectionError>)` 并保留 Error source；
错误支持 Clone，不再支持 Copy。ProjectionError 的 Capability 变体保留
描述符读取的原始 CapabilityQueryError。

`FigureTree::with_projection_budget(PreparationBudget, retained_bytes)` 配置每次独立
坐标查询的 work／allocation 累计限额和整树 contribution 留存字节上限。
默认 new() 使用 PreparationBudget 默认值，树留存上限使用默认 allocation 字节值。
input registry 仍有自己的显式 snapshot／proof 留存域。有限奇异正向合法，
逆向保持 Ok(None)；非有限和计算溢出继续区分数值原因。

命中及事件定位入口返回坐标 Result；DispatchOutcome 新增借用错误访问器，
不再实现 Copy，target／handled／capture 访问器改为借用。
一次命中搜索在全部搜索策略与 Figure 命中回调结束后复核实际消费证明。
一次事件的定位、投递与 Runtime 最终返回共享首次输入；失败输出不报告 target
或 handled，保留真实 capture，合法 source effects 仍提交。

Editor 的目标查询及无选择 press 包装相应返回 Result，并用已有 ViewerError
传播坐标原因；相关分发结果访问器返回借用。
LayoutError 和 FramePreparationError 新增 Coordinate 包装并保留 source，
FreeformError 和 AccessibilityError 也保留 Coordinate source，支持 Clone，
不再支持 Copy。动画几何读取沿 AnimationError::Coordinate 传播，
注册退休或预算错误不能解释为动画源自然失效／取消。

Runtime 帧准备、稳定查询、嵌套坐标消费及输入分发共享树实例内的临时 operation。
开始 source callback 前建立 scope，各不可变消费借用内重验 Core facts；
共享 input 首次值贯穿整个操作。派生状态采用、录制返回和 submission 采用
分别验证，只有最外层成功边界安装 contribution／world prefix。
忽略数值／输入错误不能恢复该 operation。失败保留脏区和旧无障碍快照，
不消耗 FrameId 或资源同步前缀，并请求完整重绘。
Prepared／Submitted 通知在 operation 结束后分发。

Freeform、动画 GeometryInputs／表现几何、无障碍、损伤修复和 scene capture
消费同一受检 child-content 边。GeometryInputs 仍剥离 Core insets，
GeometryPatch 按原次序恢复；动画 local transform 不被 identity 代替。
录制通过既有 PresentationSnapshot override 消费准入贡献；
未知 container 保留实时 callback。Viewport 与两类 ScalablePane 使用同一
公开 projection trait／registry，私有输入按完整浮点位值保存 origin／scale。
各内置 registry 的 snapshot／proof 留存上限为 64 KiB，独立于树的累计预算。
来源原子替换沿 PreparedFigureUpdate::with_capabilities 的完整候选表执行。

Runtime 的稳定投影另存，独立查询不能替换它；显式推进时，共享贡献变化也生成
必要坐标／后代事实并进入既有派生收敛。动画旧包络及一般绘制的输入敏感前缀身份
只在匹配 Presented 后晋升；后者仅用于保守 Full，不充当实际录制矩阵的证明。
source 变更前遇到上一显示帧的输入敏感前缀，不重新读取共享模型重建旧 damage。
完整录制／包络资格、净成本与混合像素验收尚未完成。
外部模型仍通过已批准的 full redraw／事件失效
入口请求 Runtime 推进，不新增自动轮询或信号 API；
上述领域接入不代表总体 FR-02 关闭。

## 历史批准依据

ADR-061 的公开编辑增量归属 Editor：`EditControllerKey<C>`、`EditController`、
`EditSessionDescriptor`、`EditSessionContribution`、`EditSessionPlan` 与
`StartEditSession` 通过既有 Request/Policy 注册接入同一 Viewer 活动槽。
`edit_session()` 按 typed key 查询领域状态，`edit_input_snapshot()` 在扩展失败边界
内生成 host 投影。plain 接口适配同一活动槽，不复制外部控制器的草稿。
`EditGeometryContext` 输入阶段限制为读取，挂载新反馈时允许目标限定的组件更新。
richtext 的 `RichTextEditController` 属于扩展包；精确行为见
[专用控制器设计](../editor/specialized-edit-controller.md)。

原编号 017／018／019／035 的批准范围、修订与原稿统一从
[归并记录](../../archive/adr-consolidation-2026-11-03.md)追溯。
当前行为由本页对应章节及其领域专题维护。
