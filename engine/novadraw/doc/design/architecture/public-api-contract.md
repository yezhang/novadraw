# 图形框架公共 API 契约

类型：`normative-design`

规范效力：`accepted`（2026-10-12，五项架构修订已获确认）

实现状态：既有入口以 ADR-031/033/035 和完成记录为准；本次补充契约的验证边界见
[修订记录](../../verification/reviews/framework-architecture-contract-revision-2026-10-12.md)。

本文是公共 API 的当前规范入口。本文定义角色、权限和
组合规则，不冻结全部 Rust 签名；具体方法由领域规范和公开 API snapshot 维护。

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
[ADR-035](../../adr/adr-035-explicit-domain-imports-and-root-boundary.md)。

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

Query 是短生命周期只读借用；Snapshot 是可保存的 owned revision；Output 是待校验完整
候选；Effects 是 callback 后按因果顺序提交的意图。Context 和 Query 不返回 Runtime、
FigureTree、FigureNode 或同步 mutation closure。完整决策见
[ADR-036](../../adr/adr-036-context-phase-authority-projection.md)。

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

[ADR-056](../../adr/adr-056-detached-input-invariants.md) 的样式边界使用
`graphics::Opacity` 与 `text::FontDescriptor`，原始输入在构造时受检。
`Graphics::set_opacity` 安装同一个 Opacity；动画通道也消费此值，旧
`animation::Opacity` 路径退出。GraphicsInputError 保留非有限／越界原因，
动画原始输入入口经 AnimationError 的 source 传播。

查询的目标合同以 [ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)
为准。P2-S07 的布局测量、五个坐标入口和 Route 身份切片已经迁移 Result；
Tree 结构、状态、关系、专用可选查询与 workspace 消费者也已迁移。
Node 非权威尺寸 getter 已退出公开表面；行为与最终门禁登记在
[实施核验](../../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)。

## 5. 计算、发布与开放策略

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
[组件更新协议](component-update.md)，生命周期见 [ADR-019](../../adr/adr-019-composable-api-and-scoped-editors.md)。
