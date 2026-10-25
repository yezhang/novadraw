# Tree 动画需要表现几何

类型：`normative-design`

状态：修订 3，已批准实施，T-1 进行中。承接 K06 与 ADR-026 的领域消费者目标。

2026-10-24 按 R01–R10 补齐实施契约，处理结果见文末“评审响应”。
首轮方案与 Rust 实现核对的原始判断、源码证据和验证结果见
[评审记录](../../verification/reviews/tree-layout-presentation-review-2026-10-24.md)。
本稿不覆盖已接受的 ADR；设计复核通过不表示实现或平台验收完成。
2026-10-24 用户明确批准落地，决策固化为
[ADR-046](../../adr/adr-046-sampled-geometry-and-animation-reservations.md)；
实施状态与退出证据见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

修订 2 的复核结论与 F01/F02 原始判断见
[修订 2 复核记录](../../verification/reviews/tree-layout-presentation-revision-2-review-2026-10-24.md)。
修订 3 按这两项意见统一时间快照与容量预留规则，处理结果见文末“修订 2 复核响应”。

2026-10-24 修订 3 的设计复核通过，F01/F02 已收口；结论与实施验证范围见
[修订 3 复核记录](../../verification/reviews/tree-layout-presentation-revision-3-review-2026-10-24.md)。

`api_semantics`：`animation.presentation`、`animation.timeline`、
`layout.manager`、`coordinate.conversion`、`viewport.scroll_zoom`、`damage.repaint`、
`figure.geometry.bounds`、`figure.visibility.enabled`、`figure.box.client_area`、
`clipping.strategy`、`paint.protocol`、`frame.preparation`。

## 上游按 bounds 重放布局

官方 Tree 动画只在开启 Use Animation 后用于展开与折叠，默认关闭。上游
`examples/tree/Animation.java` 捕获每个容器的子节点 bounds，按 210 ms 进度分别
插值位置和尺寸；`TreeBranch.animationReset` 把后代初始位置收拢到被操作节点。
连接线根据当前节点和内容区几何绘制；每步按被标记节点的位置差更新 viewport，
使操作节点在屏幕上保持稳定。

上游通过改写 Figure bounds、重复 validation 和同步 while 循环实现。这些做法
不能直接搬进 Novadraw：源状态必须一次提交，布局只计算最终结果，Host 驱动
Timeline，表现状态不写 source bounds、通知、撤销历史或选择状态。

现有 `BoundsTransition` 用仿射变换把最终 Figure 内容映射到旧 bounds。它能完成
普通位置/尺寸过渡，却会同时缩放文字和描边，不能表达 Tree 的子节点 bounds
重放；零尺寸折叠还会失去可逆映射。仅对现有 branch 调用该 API 不足以完成 K06。

## 增加 Runtime 拥有的表现几何快照

Runtime 拥有 bounds、表现可见性和 viewport origin 的 typed channel。采样结果叠加
在同一棵已提交 FigureTree 上，形成帧内只读几何视图；不复制可变树、不重放布局。
普通 FigureTree 查询、输入与 accessibility 仍读取 committed 状态。

视图冻结 namespace、源依赖版本、Runtime 采样 epoch 与表现版本 E，并提供以下数据：

| 数据 | 所属坐标域或来源 |
|---|---|
| sampled bounds | parent content domain，允许有限零尺寸 |
| local border/client box | sampled size 与本次冻结的 insets |
| local/child transform | 有效 typed channel 值或冻结的 committed 值 |
| effective visibility | 下文定义的祖先传播规则 |
| local-to-surface、relative transform、effective clip | 统一 edge-transform 协议的派生结果 |
| self-content 与 visual envelope | 冻结的文字绘图、派生 provider 输出及真实描边外扩 |

公开几何查询返回 `Result`，携带身份、未声明依赖、版本或数值失败原因；求逆沿用
坐标 SSOT 的奇异结果契约。provider 不取得 Runtime/Figure 的可变引用。
这些角色描述尚未冻结 Rust 签名，但 T-1 必须交付外部消费者可调用的 typed 入口，
不能只增加私有 snapshot 字段。

### 完整绘制协议的读取点

| 读取点 | 修订后的取值规则 |
|---|---|
| 可见性短路与绘制剔除 | effective presentation visibility 与该帧 effective clip |
| node placement | sampled bounds.origin 与有效 local transform |
| opaque 背景、自绘尺寸、border | sampled local border box，使用冻结的样式与 border 输入 |
| client area | sampled size 扣除四侧 insets；零可用区保持合法 |
| child-content 映射 | client origin、有效 child transform 与 viewport origin |
| child bounds clip | sampled child bounds，保持原 child clipping strategy |
| self-content provider | 同一 epoch 的几何视图与冻结内容输入 |
| damage | 同一组矩阵、可见性、clip 和 self-content envelope |

真实 bounds 不继承旧 affine 过渡绕过 child bounds clip 的规则。普通策略与 sampled
client box 相交；OverflowVisible 保留进入容器的 ancestor clip，并跳过本层 client
和 child bounds 限制。两者都不能绕过最近普通裁剪祖先或 Viewport。
零尺寸裁剪不被强制扩大，折叠终点通过显式非零收拢包围域避免提前裁掉标题。

渲染仍走当前递归协议，只把上述几何读取集中到统一视图。没有动画时直接走原
committed 读取路径，不分配表现视图，也不增加派生扫描。

### bounds 与 transform 的组合

矩阵采用列向量，继续遵守坐标 SSOT：

```text
node_to_parent = Translate(sampled_bounds.origin) * effective_local_transform
child_to_node  = Translate(insets.left, insets.top) * effective_child_transform
world(child)   = world(parent) * child_to_node(parent) * node_to_parent(child)
```

bounds.size 只决定 border/client 几何，不额外生成宽高比缩放矩阵。显式 local transform
仍可以缩放或旋转整个内容，其效果与 bounds 变化分别表达。Tree 首批没有字号或描边
缩放轨道；祖先本来存在的 scale/rotation 仍按正常坐标协议应用。

Viewport 的 sampled origin 在该 Viewport 的 child transform 中替换 committed origin
项，不再给 contents Figure 追加一次滚动 transform。ScalablePane 的 scale 保持原 owner。

### channel 身份与兼容范围

新协议以 subject identity 与语义类型共同标识 channel。建议用 `LayoutBounds`、
`PresentedVisibility`、`ViewportOrigin` 等受检语义值承载 Rectangle、bool、Point；
外部消费者可用自己的语义类型包装同一底层值，并通过相同 typed descriptor 注册。
同一语义只允许一个 owner；不同语义可以在一个计划中同时运行。

诊断名称不参与身份。同一 Figure 的两个 Rectangle 语义必须使用不同语义类型，
不能仅换 `PresentationBinding<Rectangle>` 的字符串名称。provider 的查询同样携带
语义身份；注册、查值、冲突检查与 family 合成必须保持一致。

现有 `BoundsTransition` 保留 affine 语义和既有回归，不静默改成真实 bounds。
新增真实 bounds 使用单独的 typed 构造入口。旧 affine bounds 过渡仍占用 local
transform channel；它与真实 bounds 同时运行时按上式组合，显式表现为额外 transform。
Tree 只使用新 bounds 入口，不把旧 affine 过渡混入同一布局动画。

### 文字与内容输入冻结

在 source 稳定后完成最终文字排版和 glyph 准备，计划持有该版本的不可变内容与资源
lease。sampled bounds 改变标题的 placement 和 clip，不改变字号、glyph advance、
baseline 或换行结果，不逐帧 shaping。容器变化只派生几何与线条 drawing。

字体、文本、insets、border、provider 配置变化的处理见下文版本表。任意 Figure
若需要按每个 sampled width 重新排版，不属于 Tree 首批契约，不能在 sample 中隐式
调用测量或 LayoutManager。

## 捕获范围、版本与原子替换

Tree 消费者一次性声明目标和依赖，Core 负责校验；它们不是同一个集合：

| 集合 | Tree 首批范围 |
|---|---|
| 动画目标 | 操作 branch 的收拢后代；受 contour 重新排布的兄弟及其相关容器/title；viewport origin |
| 只读依赖 | 这些目标的父链、稳定 child order、线条端点、insets/transform、字体/绘图资源与 provider 配置 |

首批以 Tree 的布局根为影响范围，显式收集所有存活 branch、contents、title 和线条
依赖，覆盖左右兄弟。目标只为有差异的几何分配轨道；只读依赖不因被引用而获得轨道。
收集发生在 capture/retarget 稳定边界，不发生在每帧。

capture 是 Runtime-scoped、one-shot 的 owned 值。它冻结目标 generation、父关系、
子顺序、源 epoch，以及 bounds/layout、visible、insets/transform、文字/绘图资源、provider
配置的依赖版本。实现可用统一 revision 加依赖指纹表达，不要求为每项增加公开计数器。
捕获同时保存最后一次成功 Runtime 采样所对应的完整表现版本 E：几何、effective
visibility、world/clip、envelope、节点跟踪位置与 sampled origin。尚未为 E 录制帧时，
capture 在相同时间补齐派生几何；有活动计划时，这些值是新计划的起点。
不能用旧 committed 终值替代当前表现，也不能通过 capture 查询偷偷推进时间。

事务顺序为：稳定并捕获 → source mutation → 稳定最终布局 → 构造收拢/展开端点
与内容输入 → 完整预检候选 → 同一边界转移 owner。首帧发布候选的起点，不能先
录制 committed 终值再开始动画。尚未获得时间时沿用 Scheduled 锚点规则。

| 版本变化 | reconcile 规则 |
|---|---|
| 本次展开/折叠及由其产生的布局变化 | 作为声明的 after 版本准入；原子 Replace 从捕获表现值开始 |
| 同一拓扑下的再次布局或快速反向 | Tree 消费者显式在稳定边界 capture/retarget；未声明的布局变化取消旧组，不自动安装 Trigger |
| reparent、子顺序/成员变化、目标或依赖 dispose | 旧拓扑不再适用，取消关联计划；新操作需重新收集，禁止逐帧猜测依赖 |
| 字体、文本、资源 revision、insets、border 或 provider 配置变化 | 取消关联计划并使用最新静态内容；新计划重新准备 glyph/输入，不逐帧重排 |
| 外部 visible/hide 版本变化 | 更新遮挡来源，按 suspension 表处理；不能复用旧操作的可见性保留 |
| 与声明依赖无关的源变化 | 继续原计划，不因全局 epoch 增长而无条件取消 |
| 新候选 foreign/stale、数值、provider 或预算预检失败 | 旧依赖仍有效时保留旧计划、表现起点、owner 和 wakeup；不发布半个新候选 |

retarget 事务内的 source mutation 不回滚。候选失败时旧计划继续使用原先 owned 的
冻结输入，完成后清除覆盖并显示最新 committed 状态。dispose/reparent 等独立依赖
失效必须取消旧计划，不能把这一事件伪装成“retarget 失败且旧计划继续”的保证。

## 采样阶段、派生依赖与错误传播

Motion 只读取冻结输入和单调时间；几何派生 provider 只读取缓存的 Motion 结果及
声明的冻结依赖。两者通过不同角色进入公开协议，不把跨 Figure 查询塞进 Motion。

### 当前表现由 Runtime 采样时刻确定

本方案选择最后一次成功 Runtime 采样作为当前表现，不以已录制或已呈现帧为锚点。
将时间采样与完整几何冻结分开命名：

| 身份 | 含义与更新条件 |
|---|---|
| Runtime 采样 epoch S | 最近一次成功 `advance_time(t)` 的单调时刻及缓存 Motion 结果；同时间、同输入重复 advance 幂等 |
| 表现版本 E | `(S, active-set revision, 合法冻结源依赖版本)`，关联一个完整只读几何快照 |
| submission/session/frame | 从某个 E 录制的不可变提交包身份，不决定当前 S |
| backend 呈现确认 | 后端结果与帧恢复证据，不推进或回退 S |

基础值缓存以 `(S, active-set revision)` 标识。新准入/retarget 在相同 S 初始化新
owner 的起点缓存，形成新的活动版本，不改写旧版本的采样或已录制包。

`advance_time(t)` 唯一负责接受新时刻、计算基础 Motion 值并缓存采样结果。成功后，
旧几何缓存标记为不再匹配当前 E；不得继续把它当作当前表现。非单调拒绝保留旧 S、
采样值、owner 和 wakeup。同时间发生源/活动集合变化时，S 的时间不变，由 E 的
依赖或 active-set revision 区分新结果。

完整几何物化读取 S 的缓存，不调用 Motion 在一个新时刻采样：

```text
稳定源与版本 reconcile
→ 选定当前 S 与 active-set revision，读取缓存基础值
→ 按依赖顺序派生 viewport origin 等几何输出
→ 派生线条 self-content，校验 drawing 与 envelope
→ 校验并冻结完整表现版本 E
```

`capture`、retarget/输入接管的预检以及帧准备都复用这个物化过程。它可以稳定源、
执行只读几何派生和缓存 E，但不能推进时钟、创建一个新的时间采样，也不能暗中
分配 frame ID、排空资源增量或启动提交。普通 pull query 只读取缓存并返回其版本；
完整快照尚未匹配当前 E 时返回未准备结果，不能静默返回旧时刻的几何。

| 入口 | 责任 |
|---|---|
| `advance_time` | 更新时间与基础 Motion 缓存；不要求有可提交的 surface 或 frame |
| 源稳定化 | 产生合法依赖版本并 reconcile；不推进时间 |
| `capture` | 稳定后物化当前 E，整包冻结起点与版本；不查询系统时间 |
| retarget admission | 先物化旧计划当前 E，候选起点整体来自它；在相同 S 原子替换 active set，并初始化候选起点值 |
| 帧准备 | 物化/复用当前 E，再计算修复范围、录制并冻结提交包；不再次推进 Motion |
| completion/Retry/reset | 处理对应提交包的资源、damage 与 session；不把其旧 E 写回当前 S |

retarget 的候选初始化只在已确定的同一 S 上使用捕获起点，不重新采样旧计划，不
伪造另一个时刻。新 active-set revision 产生新的 E，仍不改变时钟。
同时间的合法源变化依前述版本表处理：继续组使用同一 S 重新物化，失效组清除后
重建静态视图，声明的 retarget 使用完整 before/after 输入。任何快照不得混合两次
物化的 bounds、visibility、world/clip、marked position 与 origin。

### 未录制、在途与失败场景

| 场景 | 当前表现与锚点 |
|---|---|
| 已录制 t=0，成功 advance 到 t=50，尚未 prepare | capture/接管物化 t=50 的 E；bounds、位置和 origin 都取 t=50，不能取 t=0 的几何 |
| 多次 advance 而未 prepare | 取最后成功 S，允许跳过未录制的中间时刻，不逐帧补播 |
| 同时间源变化 | 时间保持；按合法新依赖版本生成完整 E，不拼接新 source 与旧派生缓存 |
| 首次 Scheduled，尚无成功 advance | 使用 admission 已解析的初始值，S 标记为 Unanchored；不虚构 t=0 或读取墙钟；第一次 advance 才建立时间锚点 |
| AwaitingCompletion | 可以物化当前 E 并 capture/接管；旧提交包保持不可变，新录制仍服从既有 in-flight 门禁 |
| Skipped/Retry | 保留当前 S 与活动状态及修复前缀；重试使用届时当前 E，不复活旧帧的采样值 |
| 当前几何物化普通 provider 失败 | 按失败表清除该组，在同一 S 重建静态 E；不得返回部分几何作为锚点 |
| 共享容量或瞬时分配不可用 | 返回结构化准备失败，保留当前 Motion 缓存与活动组；本次 capture/retarget/接管不成立，不改用旧 E |

后两种失败必须区分，预算归属见下文。query 不物化、不重新采样、不产生 damage；
显式 capture/准备允许按已缓存的时刻补齐几何，这项工作受已预留容量约束。

provider 在 prepare 时声明 typed 输入、只读 Figure 依赖、坐标查询截止域及唯一输出。
Core 补齐声明查询所需的祖先依赖，建立有向依赖图；同层按捕获的稳定注册顺序执行，
不依赖 HashMap 或 arena slot 顺序。输出不能有两个写 owner；自循环与间接循环在
admission 拒绝。查询未声明对象或未就绪输出返回结构化错误。

Tree 的几何与派生输出属于同一个原子计划。派生 provider 读取的活动几何 owner
必须纳入同一原子组并预检，或明确返回冲突；首批不支持未声明的跨计划派生依赖。
外部 provider 和内置 provider 使用同样的 prepare、依赖、失败与预算规则。

| 失败阶段 | 结果 |
|---|---|
| capture、prepare、依赖图或候选 admission | 返回 Result；不激活新 ID/owner/覆盖，retarget 保留有效旧计划 |
| 基础 Motion 或逐帧派生普通失败，包括超出自身声明上限 | 关联计划进入 Failed，清除整组基础/派生覆盖及 wakeup；同帧重建静态视图和清理 damage |
| 共享容量或瞬时分配不可用 | 整次物化/准备返回结构化失败，丢弃候选；不终止仍在自身准入上限内的活动组 |
| provider panic | 沿 Runtime fault 边界处理，丢弃候选，不能伪装成普通失败或发布部分帧 |
| 录制或 backend Retry | 沿既有帧恢复协议；不把后端失败误记为 provider 失败 |

普通失败后的静态重建不得再次调用失败 provider 的动画路径；静态线条走正常最终
数据。无关 active plan 的值和 owner 保留，最终视图仍要重新组合受影响祖先几何。

## 线条使用同一个几何视图

提取 Normal/Hanging 的纯端点算法，输入是 owner-local 的 title、contents、子标题
几何与冻结的方向/间距/样式，输出线段及描边 envelope。静态 BranchLayout 和动画
provider 共用它：静态 layout 仍可更新最终 Lines；动画只发布 owned、不可变 drawing。

TreeBranchFigure 通过普通 typed capability 注册几何派生 provider，声明实际端点
依赖。provider 从最终几何视图将子标题转换到 owner-local 域，不混用 source bounds
或手工累加 surface 偏移；整组线条与标题必须属于同一个完整表现版本 E。

Core 不匹配 Tree 名称或 BranchStyle，不调用示例端点算法。provider 不修改共享 Lines、
不调用 LayoutManager、不保存 timer。样式、方向和间距切换按静态最终结果处理并取消
当前展开/折叠计划；首批不隐式扩张为这些属性的自动动画。

## 折叠保留显示，交互立即使用终值

必须分别保存 committed final geometry 和 presentation terminal geometry。
折叠后的静态 contents 可以是零尺寸，但它不是后代收拢终点。

在最终稳定布局的 viewport 内容域中，取操作标题的 border-box 中心为收拢点 C。
所有本来可见的收拢后代标题保留冻结的宽高，终点中心映射到 C。设 P 为该标题
parent-content 到共同内容域的终点矩阵，L 为该标题的有效 local transform，则：

```text
terminal_origin = inverse(P)(C) - L(local_title_center)
```

P 使用终点祖先几何，经统一受检坐标查询取得；奇异或不可表示的 inverse 在候选
预检拒绝，不使用宽高比或固定 epsilon。零尺寸本身不使 placement 变换奇异。

终点构造一次完成：先用冻结的线性变换计算各层同中心 child border boxes 的包围域，
自内向外加回 insets 得到 contents/branch 所需尺寸；再自外向内确定 placement 和上述
标题 origin，将 child 包围域归一到父内容域。各层采样端点保持既定 clipping strategy，
不能为收拢临时关闭裁剪。此终点构造不是逐帧 LayoutManager，也不改写 source。
预检必须验证每个收拢标题中心、非负尺寸和所需 client 包围域。

| 操作 | 起点 | 表现终点 | committed 结果 |
|---|---|---|---|
| 首次展开 | 显式收拢几何 | 最终展开布局 | expanded=true |
| 折叠 | 当前表现几何 | 后代标题中心收拢到 C；兄弟转到新布局位置 | expanded=false，contents 隐藏 |
| 中途反向 | 新捕获的当前表现几何与可见性 | 新操作的显式终点 | 新操作一次提交的终值 |

首次展开的收拢点取操作前标题中心；折叠终点取最终布局中的操作标题中心，viewport
另按标题位置差跟踪。中途反向不重置为“全展开”或“全收拢”状态。

折叠期间，计划只对显式声明的存活节点提供本地可见性保留，包括静态隐藏的 contents。
父链上的每个隐藏节点都必须由本计划声明保留；外部隐藏的祖先不能被后代覆盖恢复。
保留名单还必须证明节点在捕获表现中有效可见，且 after 隐藏由本次折叠造成；原本
未展开的嵌套 branch 不因祖先动画而展开或显示。
线条参与相同规则。到达终点或取消时，同一边界清除 bounds、可见性、线条与 origin
覆盖，立即显示 committed 场景，不多显示一帧收拢终点。

### 表现有效可见性与 suspension

effective presentation visibility 是每层“committed visible 或该计划合法保留”的
祖先合取。合法保留仅适用于本次操作造成的隐藏版本；新的外部 hide 是依赖变化，
不能被旧保留标记屏蔽。绘制与 target_hidden 判断使用同一规则，clip 使用同一采样几何。

| 事件 | 行为 |
|---|---|
| 本计划造成 committed hidden，但合法保留为 visible | 不触发 target_hidden，折叠整组继续运行 |
| 外部祖先隐藏或目标 hide 版本改变 | 不能被保留覆盖；按该计划 Advance/Pause/Finish 处理 |
| surface suspend | 独立 Host 条件，始终参与 suspension，不因表现保留而取消 |
| Advance/Pause/Finish | 分别继续绝对时间、暂停整组并平移锚点、清除整组覆盖；保持已有定义 |
| Disabled/ReducedMotion、dispose | 沿既有模式/身份规则终止整组；切回 Enabled 不复活 |

交互资格不使用这个表现可见性规则。命中、焦点、选择和 accessibility 只看 committed
状态；source 折叠时由所属输入/选择域清理失去资格的焦点/选择，不因继续绘制后代
而延迟。计划不保活
dispose 的 Figure，不复用旧身份，不创建动态 source 节点模拟 temporary visual。

## Viewport 跟踪与布局共用一个进度

每次 capture/retarget 先物化当前表现版本 E，从同一个 E 冻结 sampled origin Oa 和
操作标题左上角在 viewport 内容域中的位置 Xa，并将 E 记录在跟踪锚点中。
位置查询截止于该 Viewport 的 child-content 域，包含内部正常
scale/transform，但不包含该 Viewport 自身的 scroll 和 client-origin 映射。
标题中心 C 用于收拢，左上角 X 用于跟踪，两者不能混为一个锚点。

本次稳定 after 布局的 RangeModel minimum/maximum、contents extent 与 viewport client
extent 形成冻结的双轴合法区间 K。取计划终点位置 Xe，先将最终 origin
`clamp_K(Oa + Xe - Xa)` 提交到 RangeModel；RangeModel 仍是唯一源真值。
每个样本由同一个进度派生：

```text
sampled_origin(t) = clamp_K(Oa + sampled_marked_node_position(t) - Xa)
```

两轴独立 clamp 后一起发布，不逐帧改 RangeModel。第一帧使用 Oa/Xa；区间收缩使 Oa
越界时，第一帧直接夹到 K，允许必要的边界位移，但不能闪现 committed 终值再回退。
末帧先统一清除覆盖，使用已提交终点。反向操作重新取当前 Oa/Xa 和新的 K，不沿用
首次操作的锚点。依赖中的 extent/client/scale 再次变化按版本表 reconcile。

### 引擎层直接输入接管

wheel、viewport drag、scrollbar 或公开 origin 写入，必须进入同一个引擎 origin
写入仲裁入口；输入会话固定 target 的机制继续使用，但它不代替 owner 接管。
接管先物化当前 E，读取其中的 sampled origin，基于该值计算增量与最新 committed 区间的
clamp 结果，预检新源写入，再原子取消整组并提交新 origin。公开绝对写入以请求值
为准；增量输入不能基于动画终点 RangeModel 值跳算，也不能取旧录制帧的 origin。
物化失败时不取消组、不执行源写入；bounds、marked position 和 origin 不混用两个 E。

写入预检失败保留有效旧计划。接管成功后不得恢复旧 origin 或单独留下 Tree 跟踪
owner；后续输入使用新 committed origin。独立 cancel 未携带输入时仍直接显示已经
提交的最终 origin，不新增 KeepCurrent 取消模式。

新的 origin channel 与 contents local transform 是不同语义。若旧 viewport transition
正占用 contents transform，Tree 候选须明确接管其整个关联计划或返回冲突，不叠加
两次 pan。同一个新计划可以组合合法的 contents transform 和 origin，依上文矩阵
各应用一次。首批 Tree 不使用旧 viewport affine transition 模拟跟踪。

## 取消与失败使用既有组语义

bounds、表现可见性、线条与 viewport 共用一个原子计划和终止结果；prepare/admission
与逐帧失败分别遵守上文错误表。静态零尺寸 fallback 的既有回归继续保留。
独立取消、自然完成、模式切换和直接输入接管都产生清理 damage；Source mutation
及外部副作用不纳入回滚。

## Damage、遍历预算与帧恢复

每个已录制提交包保留其 E 对应的 surface envelope、effective clip 与 surface 版本，
in-flight/恢复前缀保留尚未确认的修复范围。仅 capture 或物化几何缓存不建立“已绘制”
基线；它可以比最后录制帧更新，但不能让旧帧修复证据丢失。新旧
envelope 分别保留自己的祖先矩阵和 clip；旧包围盒不能用新 viewport 或祖先几何
重新解释。子节点越出旧 bounds、border/line 外扩及 OverflowVisible 都纳入 envelope。
surface/DPI 不可比较时请求 Full；该降级不代替正常 retained-partial 的正确性证明。

新的 envelope 使用最终表现视图计算，与实际录制共享可见性、矩阵与 clip。viewport
变化同时覆盖旧/新可视区域；清除动画后再计算 committed envelope，修复残留像素。
Retry 保留尚未呈现的 damage；完成确认前不能丢掉恢复前缀或资源 lease。

捕获阶段一次性归并目标根和依赖闭包。每帧在归并后的受影响区域走一次现有递归
几何协议，缓存 edge/world/clip，再自子向父汇总 envelope；祖先和后代同时作为 target
不会分别扫描整个后代集合。它是当前递归协议的准备数据，不是第二套渲染主线。
只读祖先路径去重，普通最终录制仍按现有树遍历执行。

### 准入容量与单组上限分别记账

Runtime 统一配置 capture/staging、活动容量、快照/资源 lease 存储和几何物化工作容量，
不在示例中散落阈值。几何物化工作容量对应一次完整 E 的生成，供帧准备或 capture
使用；不是按某个尚未呈现的 UI 帧累计多次 advance 的消耗。

每个候选在 prepare 时声明并证明自身上限 B，包括节点、祖先和依赖边数、
轨道/provider 调用数、线段/其他几何项、drawing 大小与矩阵/clip/envelope 工作量。
冻结后的目标闭包、去重调度与受检输出 builder 必须能在任何合法采样时刻满足 B。
字段可构成多维容量向量，各维独立校验；不能仅预估平均帧消耗。

准入在激活前预留该组容量，运行期间在自己的预留范围内计数，不从一个按执行顺序
递减的共享余额借用额度。共存的旧动画通过兼容适配证明并登记工作上限，不能免费
占用这个池。无法给出有界输出/依赖的 provider 不准入新几何协议。

活动组当前快照与下一物化候选共存所需的峰值存储，也在准入时预留。capture 或
submission 若要独立保留某个快照，须先取得其留存期的容量，不能占用活动组后续
物化的保证额度。留存容量不足只拒绝该新捕获/提交，不能撤回活动组预留；只保留
Arc 引用也必须登记其留存责任，避免旧缓冲不能复用时才发现容量被占满。

| 容量类别 | 生命周期与不足处理 |
|---|---|
| capture/staging | 为一次捕获或候选预检提供独立临时容量；不足只拒绝本次新工作 |
| 活动组与物化工作容量 | 活动期间持有预留；有限完成、取消或受检失败后释放，不按实际少用量让其他组超额准入 |
| owned 快照与资源 lease 存储 | 按实际存活的缓存、capture、提交包与恢复前缀计账；引用未释放时不能因计划结束提前释放额度 |
| 静态修复 | 与失败动画 provider 的输出/工作额度隔离，沿既有 Full/恢复路径执行 |

同一 E 的缓存复用不重复执行 provider，也不重复收费。新 E 在相同的活动预留内
物化；需要同时保留的旧快照仍计入存储池，不能用“每帧重置”绕过存活字节上限。

### 新准入与 retarget 的容量检查

设 Q 为共享活动容量，R 为所有活动组预留之和。新 plan 必须证明 `R + Bnew <= Q`；
预算恰满时既有合法组继续运行，新 plan 被拒绝。比较逐维执行，与 provider 顺序无关。

retarget 用净替换检查 `R - Bold + Bnew <= Q`，不先取消旧计划再尝试分配。
候选预检与旧计划共存时，另占 capture/staging 容量；临时存储不足同样保留旧计划。
成功边界一次转移 owner、活动预留和当前 E，候选临时额度随后释放。旧 capture、
in-flight/恢复包的存储 lease 按各自生命周期继续计账，不能混入净活动容量折抵。

### 单组越限与共享不可用的失败归属

| 情况 | 归属与结果 |
|---|---|
| admission/capture/staging 容量不足 | 拒绝新工作；有效旧组的 owner、采样值、预留与 wakeup 保留 |
| provider 产生超过自己声明的节点/依赖、线段、drawing 或工作上限 | 受检 provider/plan 失败；错误携带组、provider、限额项及声明/实际计数，清除该关联组 |
| 合法组在自身 B 内，但共享池或瞬时分配不可用 | 几何物化/帧准备失败；不归罪于最后执行的 provider，不驱逐合法活动组 |
| 运行期间请求降低共享容量到已有预留或存活存储以下 | 原子拒绝配置更新，保留旧配置和全部合法活动组；需先由调用方显式取消/释放或等自然完成 |
| 实际共享不足与预留账不一致 | 作为共享容量/内部准备错误报告，丢弃整次候选；不能用单组越限清理掩盖记账问题 |

只有能证明越过自身冻结上限的组才因限额失败进入 Failed。实现应将这类错误明确
归为 provider/plan 受检失败，不把所有 `BudgetExceeded` 都映射为活动组终止。
循环、未声明查询或非法输出仍按正常 provider 失败处理。

共享不可用时保留当前 S 的 Motion 缓存、活动 owner、预留与 wakeup，准备失败不将
旧 E 冒充当前表现，不生成部分提交包或消耗资源同步前缀。再次准备可以复用当前 S
的 Motion 缓存；下一次正常 advance 仍可推进时间或自然完成，这不属于预算驱逐。
retarget/输入的当前 E 若无法物化，则本次操作返回失败，不能先取消再用静态值接管。

本修订不改变 ADR-026 和公开失败合同的预算原则：共享容量耗尽只拒绝新工作，
不驱逐仍合法的活动组。单组违背声明属于自身受检失败。静态 Full 修复不再次调用
失败 provider；正常渲染/设备分配失败继续走原帧恢复边界，不承诺物理内存永不失败。

### 工作量与退出证据

T-1 交付容量预留、单组声明/实际计数与错误归属；T-4 给出实际测量：

```text
V = 去重的受影响节点与祖先
D = provider 依赖边
G = 派生线段数
每次 E 物化的几何与派生工作量 = O(V + D + G)
```

该式约束 Tree 及 Core 调度的实际节点访问、矩阵/clip/envelope 和线段工作量。
通用 provider 的其他 drawing 几何项也要计入派生工作预算；输入、调用次数与输出
规模均须受检。这里不承诺限制任意外部函数内部的 CPU 时间或使任意扩展代码终止。

树深仍限制 10,000，使用既有递归深度防护；不恢复迭代渲染。新增诊断为有界 pull
计数，不在渲染热路径打印。预算验证必须交换两个合法组的执行顺序，证明 owner、
值和 wakeup 不依赖顺序；再分别核验预算恰满、新 plan 拒绝、净容量替换、配置下调
拒绝、单组输出越限与静态 Full 修复，不能只测“某个 provider 报错后组被清除”。

## 实施与退出证据

| 顺序 | 切片 | 必须证明 |
|---|---|---|
| T-1 | typed bounds/可见性、捕获与完整几何视图；派生协议和预算 | 首帧无终值闪烁；零尺寸；嵌套 translation/scale/rotation 与四侧 insets/border；普通 clip/OverflowVisible；同底层值多语义；循环/查询/prepare/sample/panic/预算失败；retarget 原子性；无动画零新增扫描 |
| T-2 | Tree 静态/动画同源端点与内容冻结 | Normal/Hanging、多层标题端点同 epoch；字号、描边与 glyph 缓存不变；不逐帧写 Lines、shaping 或布局；派生失败清除整组 |
| T-3 | 显式 Viewport 组合、收拢终点与引擎输入接管 | 将 19 节点树放入普通 Viewport；左右兄弟重排；所有后代中心收拢；中途反向重新锚定；双轴 clamp/extent 收缩；wheel/drag/scrollbar/公开写入；旧 transform owner 冲突；取消/完成保留正确 origin |
| T-4 | 默认关闭的真实 Use Animation 面板与完整验收 | 动态增删/释放和外部隐藏；Pause/Advance/Finish、suspend/resume 与模式切换；Native GPU 固定时间中间帧、retained-partial 旧像素擦除；Retry/reset、资源释放和最终静态等价；嵌套目标工作量 |

修订 3 为切片增加以下退出证据，须由公开入口断言版本与失败主体：

| 复核项 | 必须覆盖 | 断言 |
|---|---|---|
| F01 / T-1、T-3 | 录制 t=0 后 advance 到 t=50，未 prepare 即 retarget/增量滚动；连续 advance；同时间源变化 | 使用最后成功 S 的 E；bounds、visibility、world/clip、marked position、origin 与锚点版本一致；时钟不因 capture/接管推进 |
| F01 / T-1、T-4 | 首次 Scheduled、AwaitingCompletion、Skipped/Retry、reset | Unanchored 不伪造时间；在途包不变；当前采样不回退到旧提交包；资源/修复前缀保留 |
| F02 / T-1、T-4 | 两个合法 active group 交换执行顺序；容量恰满；新 plan 与配置下调被拒 | 合法组的 owner、当前采样值、预留和 wakeup 不变；拒绝主体与运行顺序无关 |
| F02 / T-1、T-3 | retarget 净容量恰好可替换；staging 不足；自身上限内的共享不可用 | 成功时一次转移 owner/预留/E；失败保留旧组；不能按共享失败终止合法组 |
| F02 / T-2、T-4 | provider 超出自身声明上限；随后静态 Full 修复 | 错误归属明确，只清除违约关联组；无关组保留；静态路径不再次调用失败动画 provider |

T-3 的 Viewport 是明确新增的消费者组合，不能把当前白色 host Figure 宣称为已有
Tree Viewport。T-4 增加默认关闭的 Use Animation checkbox；关闭时继续执行原静态
展开/折叠。SimpleTree 静态消费者保持同一端点算法，不被迫安装动画能力。

独立测试 crate 通过同样的 typed 注册接口实现两个同底层 Rectangle 的语义 channel、
自定义容器和几何派生线条 provider。测试须覆盖完整读取点、依赖拒绝与全部错误通道，
不能依靠 Tree 私有名称、内部 hook 或不同的 typed/erased 路径。

固定时间 Headless 核验 source/表现分离、中心/端点和工作量；Native GPU 核验 clip、
字号/描边、中间帧与擦除。比较有动画和 Disabled/ReducedMotion 的最终 committed
几何、glyph、线条、origin 和像素。编译或 active track 数量不替代这些证据。
稳定验证入口依 `verification/README.md` 和当前 xtask 建立，未实现前不新增通过声明。

该设计涉及渲染使用的几何协议。实施前需要评审，不直接扩张现有 affine bounds
实现。渲染主循环如需调整，必须限定为从统一只读几何视图取值；不得增加第二套
遍历或为 Tree 增设快路径。

## 评审响应

以下“已纳入”仅表示本次设计修订处理了意见，不表示复核通过或实现问题关闭。
原评审记录的判断与测试快照保留，复核另行记录。

| 编号 | 处理方式 | 修订位置与复核重点 |
|---|---|---|
| R01 | 已纳入：独立 committed/表现终点；标题中心受检转换与容器包围域；同帧清除 | “折叠保留显示，交互立即使用终值”；收拢、零尺寸、反向和静态等价 |
| R02 | 已纳入：完整读取表、edge 组合、普通/OverflowVisible clip；不沿用 affine 绕过 | “增加 Runtime 拥有的表现几何快照”；嵌套、insets、border 与全路径几何一致 |
| R03 | 已纳入：Motion/派生角色分开、稳定依赖顺序、循环拒绝、Result 与阶段失败表 | “采样阶段、派生依赖与错误传播”；外部 provider、整组清理与 panic fault |
| R04 | 已纳入：目标/依赖闭包、兄弟影响、版本表、当前表现起点；明确旧计划有效条件 | “捕获范围、版本与原子替换”；反向、拓扑/字体/insets 变化与候选失败 |
| R05 | 已纳入：合法保留的祖先合取，外部 hide/surface suspend 独立，交互只用源状态 | “表现有效可见性与 suspension”；三种 policy、模式切换与释放 |
| R06 | 已纳入：每次 Oa/Xa 锚定、内容域截止、冻结 clamp 区间、首末帧与输入原子接管 | “Viewport 跟踪与布局共用一个进度”；双轴、反向、输入与 owner 冲突 |
| R07 | 已纳入：保留旧帧 envelope、统一 clip/world、合并区域遍历、分阶段预算与统计 | “Damage、遍历预算与帧恢复”；retained-partial、Retry 与线性工作量 |
| R08 | 已纳入：提取共用端点算法、typed provider、显式新增 Viewport 和真实 checkbox | “线条使用同一个几何视图”“实施与退出证据”；19 节点与静态消费者 |
| R09 | 已纳入：语义类型身份、bounds/transform/insets 次序、旧 affine API 保留与文字冻结 | “channel 身份与兼容范围”“文字与内容输入冻结”；外部同底层值多语义 |
| R10 | 已纳入：补齐六个 family 与 T-1–T-4 证据，不把旧基础回归当新实现证据 | `api_semantics` 与“实施与退出证据”；按账本逐项核验 |

## 修订 2 复核响应

以下响应记录设计处理，F01/F02 是否通过须由后续复核确认。原复核的“暂不通过”
结论和验证限制保留，不改写为已实现或验收通过。

| 编号 | 处理方式 | 修订位置与复核重点 |
|---|---|---|
| F01 | 已纳入：以最后成功 Runtime 采样 S 为当前表现；E 区分源/活动版本；几何物化不推进 Motion 时钟，提交与呈现独立 | “当前表现由 Runtime 采样时刻确定”“未录制、在途与失败场景”及 viewport 锚点；advance 后未 prepare、同时间变更和在途/Retry |
| F02 | 已纳入：准入预留自身上限 B、净容量替换与独立 staging；共享失败不驱逐合法组；配置下调低于存活预留时拒绝 | “准入容量与单组上限分别记账”“单组越限与共享不可用的失败归属”；顺序交换、恰满、违约组与静态修复 |

依据：[ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)、
[领域能力矩阵](capability-integration.md)、
[坐标规范](../coordinates/coordinate-system.md)、
[语义覆盖账本](../../parity/draw2d/api-coverage.md)。
