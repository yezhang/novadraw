# 按属性依赖限定布局重验

类型：`proposal`

能力归属：Core Layout 定义父布局的依赖声明；Core Runtime／FigureTree 负责
失效范围、队列、预算和失败恢复；ScrollBar／ScrollPane 是普通消费者。
Editor、backend 和 platform 的职责不变。

日期：2026-11-05。状态：具体机制草案，尚未接受或实现。属于 FR-02 已授权的
纯投影分离目标；本文新增布局输入读取、队列失效范围与父布局依赖声明，
实施前需确认这组具体契约。
不将已有总体方案的授权写成本文已通过。

本轮定向复核补充了布局私有输入的读取接口。待确认范围为：布局输入如何进入
现有证明链、父布局声明何时允许停止传播、局部队列如何覆盖内部几何反馈和失败。
此前总体实施授权保持有效；活动交接与能力成员索引不依赖这组新接口。

2026-11-05 范围修正：纯投影分离的总体对象是整个 FigureTree。
总体设计由 [FigureTree 内容、布局与投影分离](figure-tree-projection-separation.md)维护；
本文仅是内部局部重验的辅助候选，不代表通用投影主入口，也不限定于文本 Figure。
下文 scrollbar／文本记录属于首个定位场景；候选 API 需按总体依赖边界重新复核。

`api_semantics`：`layout.manager`、`validation.protocol`、
`notification.layout_update`、`viewport.scroll_zoom`、`frame.preparation`。
现行语义由 [Draw2D 账本](../../parity/draw2d/api-coverage.md)维护。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 问题与证据

纯 viewport 位移目前没有直接使文字排版失效。
`ScrollBar::sync` 发现 range snapshot 变化后调用 `EventEffects::invalidate`；
FigureTree 随即使 scrollbar 及其父链失效。ScrollPaneLayout 再查询 viewport 的
首选尺寸，ViewportLayout 继续测量 contents，进入受检文字测量缓存。

Parley 0.12 细阶段基线中，每次拖拽和滚轮有两个 Flow measurement cache hit，
29 次资格读取和 27 次输入校验。完整分母及口径见
[本轮验证](../../verification/reviews/parley012-input-scope-and-projection-2026-11-05.md)。
这些次数确认了可删除工作的候选，尚未证明本文能降低耗时。

另一个传播点在 `FigureTree::perform_validation_cycle_measured`：
从 UpdateManager 取出 FigureId 后，它会再次调用
`mark_validation_path_invalid`。只改 scrollbar 的即时失效函数，
到排队验证时仍会恢复整条父链失效。

第三个传播点在布局输出提交：`apply_layout_output` 使用
`set_bounds_with_update`。thumb 移动时，两个 page 部件的长度会改变；
child resize 会调用普通 `mark_invalid`，再次使 scrollbar 祖先失效。
局部队列和内部布局输出必须一起处理，不能把初次排队的改善当作完成。

只读 Draw2D 依据是 `org.eclipse.draw2d.ScrollBar.revalidate()`：
只 invalidate 自身并加入 UpdateManager，理由是 range 改变不会改变首选尺寸，
orientation 改变除外。`Viewport.localRevalidate()` 也保留局部重验。
源码位于 `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/`；
不使用 Zest，也不运行 Java 验证代码。

Novadraw 需要保留未知自定义父布局的现有行为。例如自定义布局可能持有同一
RangeModel，并按 value 更新 sibling 位置；它不能仅因 child 的首选尺寸不变而
被跳过。因此本文采用显式依赖声明，缺失声明继续传播。

## 拟议行为与接口

保留 `EventEffects::invalidate()` 的完整父链语义。新增一种明确声明
“本次只改变该属性及 owner 内部布局”的请求，使用既有 `PropertyKey<V>` 和
`PropertyValueType`，按完整 typed key 匹配，不按裸字符串、Figure 名称或事件名称分支。

拟议调用面如下，签名在接受前均为候选：

```rust
EventEffects::revalidate_layout_for_property(key, old_value, new_value, guard);
LayoutRevalidationGuard::new(&input_registry, input_key, first_input);
LayoutDependencyDeclaration::independent_child_property(key);
LayoutDependencyDeclaration::independent_child_property_for(child, key);
LayoutManager::register_child_layout_dependencies(&self, out)
    -> Result<(), LayoutDependencyRegistrationError>;
```

`revalidate_layout_for_property` 记录已有 typed property payload 模型的 old/current
值及 source。请求者必须保证 owner 自身的外部几何、任意约束下的 preferred／minimum／
maximum、baseline、border／insets、结构和资源没有因该变化而改变。
请求可更新 owner 内部 child placements，不承诺这些内部几何保持不变。
若不能作出这个保证，请求者调用普通 `invalidate`。
本接口不自动发送 property notification，避免与当前通知路径重复。
old/current 必须是 owned 不可变值；`PropertyValueType` 的 Clone 和 PartialEq
本身不能证明这一点。这些值用于保留请求事实，不替代 Preparation 输入证明。

`guard` 绑定 source、注册实例和 owned 不可变的非目标输入首次值，复用
`PreparationInputKey<P>`／`PreparationInputRegistry`／`PreparationInputPayload`
的 typed reader 和完整比较模型。请求者在记录请求时交出已读取的首次值；
Runtime 在执行局部项前，以 source 范围和本轮预算重读实际输入。
注册身份与 reader 弱引用由 Core 保留，不能靠 raw 指针或字符串找到替代 reader。
候选 `LayoutRevalidationGuard::new` 接收 owner 已读取的完整 owned payload，
绑定已有注册并保留弱引用；未知注册按 `PreparationInputError` 返回。
此时的 pending payload 属于 deferred effect，尚无发布权，不伪装为已受检快照。
执行时在现有 PreparationScene／预算内准入首次值及证明 metadata，
调用原注册 reader 读取当前值；不在 EventContext 内另建无预算 reader。
注册／typed value 保持完整身份，snapshot admission 的原始错误继续传播。

首次值与当前值不等时升级为普通失效；reader 原始错误、注册退役和预算失败
仍作为错误传播，不解释成“没有能力”。请求之后的 consumer 或其他串行回调
可能改变 range extent 等输入，必须被这个执行前检查发现。
缺少 guard 的扩展只能调用普通 invalidate。
执行后有实际外部回调时，局部范围不跨越该回调；后续工作重新确认资格。
局部 layout 一旦开始，guard 进入该布局的普通 input proof 收集器。
manager、测量或 child callback 返回后及提交前，仍按现有消费／发布检查重验；
期间改变输入会拒绝该候选并回滚，不能重新读一个首次值来让旧候选通过。
“变化则升级普通失效”只适用于采用局部候选之前的资格判断。

`LayoutDependencyDeclaration` 是与 manager 实例一同准入的不可变声明表，按
`ErasedPropertyKey` 匹配 typed 身份。它声明：当直接 child 的上述外部输入保持不变时，
该属性引起的 child 内部布局变化不会影响本 manager 的 layout 输出、测量缓存、
派生数据或失败行为。可对任意直接 child 声明，也可限定实例中的具体 child。
声明查询仅查表，不调用用户判断函数，不缓存动态 payload 或 provider revision。

LayoutManager 默认不注册任何声明。注册仅在 manager 准入或替换时执行，
受检表由对应 LayoutState 持有；运行时查询不会再次调用扩展注册函数。
注册中的 child 身份和重复条目需在提交前校验，失败或 panic 保留 manager
准入阶段的错误与原旧状态，不能安装部分表。具体错误接入与容量预留随实现冻结。
外部 manager 可以用自己的 property key 和同一声明表加入，没有内置专用分支。
替换 manager 同时替换声明；声明不归应用或全局持有。
内置布局只为已确认独立的属性登记，不新增闭合的内置 cause 枚举。

Runtime 收到请求后沿当前 parent 链检查。每一级以直接 child 身份和原始 typed key
查询声明。每一级都同意时，该级的对外测量和派生状态保持不变，可继续检查上一级。
无 manager、无 lifecycle、无 preparation、无 freeform 状态的普通节点可以作为透明祖先；
存在未声明的 manager、lifecycle、preparation 或 freeform 状态时，
整次请求退化为普通父链失效。
这条首片规则较保守，后续不得用节点类型识别绕过。

声明链有效时，FigureTree 只使请求 source 的布局和派生数据失效，将它加入局部队列。
source 的生命周期仍按普通规则执行，children 仍获得真实几何更新、可见性与 damage。
祖先的有效标记和文字布局缓存保持。任何普通失效都优先于局部请求。

## 队列与回滚

UpdateManager 继续拥有待执行队列，FigureTree 继续拥有 valid 标记和布局数据。
队列内部必须保存 source 与已确认的失效范围，不能只保存 FigureId 后再猜测范围。
局部状态不是“节点已经 invalid”，也不能从当前 valid 标记推导。
若当前祖先已有普通失效，局部项不缩小它的范围；首片将该项一并升级。

同一 source 的请求按以下规则合并：

| 已有请求 | 新请求 | 执行范围 |
|---|---|---|
| 局部 | 普通父链 | 普通父链，局部原因可留作诊断 |
| 普通父链 | 局部 | 普通父链 |
| 局部 | 相同 typed 属性的局部 | 保留局部及必要首末值，不覆盖首次输入证明 |
| 局部 | 其他属性的局部 | 首片退化为普通父链 |

排队到执行之间可能发生 reparent、manager 替换或新失效。执行局部项前，
FigureTree 按当前 parent 链重新查询静态声明；不匹配即退化。
局部项不能调用现有的无条件父链标记。直接调用公开
`UpdateManager::add_invalid_figure` 的旧消费者继续得到普通父链行为。

合格局部项的 LayoutOutput 提交携带 source 作为内部重验边界。
输出中的 child bounds／visibility 变化继续使相应 child 失效和更新坐标，
但由这一份已验证输出引起的父链失效到 source 为止。
这是布局提交参数，不是整个 Runtime 的“正在滚动”开关。
source 自身外部 bounds 或测量输入变化、显式 `LayoutChange::Invalidate`、
非内部几何的 property／viewport effect 或后续外部回调，都升级为普通传播；
不能借局部边界屏蔽这些变更。所有旧 LayoutOutput 入口默认保持普通语义。
升级必须在完整 output 检查后、第一项几何写入前决定；不能先提交一部分局部输出，
再发现另一项需要普通传播。内部 child 的 visibility 改变也需要检查是否影响
owner 的对外测量或声明条件，不能仅因它来自同一个 output 就赋予局部资格。

checkpoint、队列清空、source 删除、drain、重试和失败恢复都必须处理范围 metadata。
布局或输入检查失败仍按现有事务回滚；恢复后允许保守扩大为普通父链，
不能丢失 pending source 或留下一个实际上无效的祖先。
本方案不引入第二个 UpdateManager、全局 epoch 或跨回调校验成功缓存。

## 定向复核：布局私有输入必须进入证明链

现行 `PreparationLayoutRead` 已记录部分几何和测量依赖，但它不是任意布局的
完整输入清单。`LayoutSceneQuery::children` 返回直接 children 的身份、顺序与
完整 bounds；`constraint_as` 也通过该查询确认 child 归属。即使某个布局当前
只使用返回值中的身份，Core 也不能自动将这次读取缩成拓扑依赖。
现行 `LayoutState::invalidate` 还会清除尺寸缓存并调用 manager 的
`invalidate(reason)`。依赖声明必须包含跳过这个 callback 的条件，不能仅声明
“首选尺寸不变”。未知 manager 的私有派生数据和原始错误路径继续保留。

另一个实际缺口在 `ScrollBarLayout::layout`：它直接通过共享 state 读取完整
RangeModel snapshot，计算 thumb 与 page 部件的位置。请求阶段的非 value guard
只能证明祖先隔离的前提，不能证明布局实际使用的 value 在消费与发布之间未变。
而且布局会调用可替换 thumb 的 minimum measurement；这个外部 callback 可以改变
共享输入。因此需同时保留两种证明：

| 证明 | 首次值绑定的时刻 | 检查什么 | 不一致时的行为 |
|---|---|---|---|
| 请求隔离 guard | 记录局部请求时 | 非目标输入及祖先隔离条件 | 采用局部候选之前升级普通失效；reader 错误仍传播 |
| 布局消费 proof | 该次布局首次读取实际输入时 | 完整实际输入，包含目标 value | 拒绝本次候选并按普通事务恢复，不用新值替换首次值 |

两份证明可以使用同一 registry 中不同 payload 类型的 reader，
但不能用同一份“去掉 value 的快照”代替完整消费证明。
reader 必须满足既有不可变值、纯读取、容量计费和禁止稳定化期间并发写入的合同。
所有读入的字段逐项比较，不用哈希、指针相等或诊断字符串替代值比较。
包含浮点字段的 payload 需先遵守 RangeModel 的有效值规则，并提供明确的完整
相等语义，不能为实现 `Eq` 而把非法数值视为有效输入。

拟议新增公共查询如下；这些签名尚未实现：

```rust
LayoutSceneQuery::input<P: PreparationInputPayload>(
    &self,
    registry: &PreparationInputRegistry,
    key: PreparationInputKey<P>,
    source: FigureId,
) -> Result<PreparationInputSnapshot, LayoutError>;
```

返回值沿用 `PreparationInputSnapshot`。实现将读取交给该次布局所属的
`PreparationScene::input`，保留 source 范围、注册实例弱引用、首次值、
预算和不可清除的失败记录。`LayoutError` 包装现有 input 错误并保留原始 source，
不另建字符串错误或静默返回空快照。manager 使用快照中的完整值计算输出，
不能先取受检快照再用另一份直接读取的可变 state 计算几何。

Runtime 驱动的布局和测量必须能提供这个受检 session，包括没有 preparation
capability 的普通 Figure。不能继续由“整树是否存在 preparation descriptor”
决定布局输入 API 是否可用。受检上下文的构造与复用由 Core 负责；
优先在实际需要时构造，不为每个普通节点另建 registry 或复制全树样式。
旧的纯几何查询保持有效。没有受检 session 的内部测试或兼容查询若调用新接口，
拟返回 `LayoutError::InputSessionUnavailable { source }`；不得直接执行一个无预算 reader。
正常 Runtime 路径不能以该错误作为长期回退方式。

首次完整读取应发生在可能读取相关私有输入的外部 child callback 之前。
callback 返回后，消费检查确认原首次值仍有效；若 callback 改变了 value，
即使 scrollbar 后来能读到新值，也不能让原候选继续通过。
发布前检查保留原 proof，失败候选的 child bounds、damage 和通知不晋升为稳定状态。
layout 自己写入的 `minimum_thumb_length` 是派生输出，不应混入 RangeModel source
证明；这类私有派生写入仍遵守现行 callback 副作用恢复边界，不能承诺 Core 能
回滚任意 manager 私有内存。

内置与外部布局使用同一个查询接口和 reader registry。外部容器至少用一个自有
payload 读取其共享模型，再通过自有 typed property 请求内部重验。
不为 ScrollBarLayout 添加 Core downcast 或专用 proof 通道。

## 下一片的执行顺序与边界

先接入通用布局输入证明，再接入失效范围，最后迁移首批消费者。三个步骤仍属于
唯一 FR-02 Execute；每一步的验证记录分别保留，不能由最终样例成功倒推前一步正确。

1. **布局输入读取**：冻结新查询及错误包装；验证无 preparation Figure 的 Runtime
   布局、preferred／minimum 查询、完整首次值、注册替换和退役、忽略错误、
   child callback 修改输入以及预算拒绝。此步不缩小任何失效范围。
2. **局部队列与提交边界**：统一处理即时标记、出队范围、重复请求、普通失效优先、
   当前父链复核和内部 output。完整父链仍是旧入口与未知扩展的默认路径。
   只对已确认边界停止传播，不从 `is_valid` 或事件名称恢复范围。
3. **普通消费者接入**：ScrollBar 与外部自有容器一起接入同一机制；ScrollPaneLayout
   和 FillLayout 仅登记本文确认的独立属性。单独评估新增证明成本是否抵消删除的
   上层重验成本，不能因“证明更完整”就宣称性能优化通过。

静态声明的准入要与 manager 安装或替换同事务完成：具体 child 可以是本次事务
预留、尚未发布的身份，但必须在提交时成为该容器的直接 child。
重复声明、非法身份、容量拒绝和 callback panic 都不能留下部分声明表；
旧 manager 与旧表按既有准入失败语义保留。全体 child 声明与指定 child 声明
重叠时按同一独立结论去重，不引入运行时优先级。
reparent 后重新核对直接 child；不能把 arena 槽位当作原 Figure 身份。

首片不从实际读取记录自动生成免重验声明，不将几何移动转换成 typed property
请求，也不批量声明内置布局与任意属性独立。固定尺寸节点移动继续从几何入口处理
投影；内部局部重验只删减该路径上确实多余的布局反馈。
Freeform extent、跨子树连接路由、scale 参与测量和未知 preparation 均保留真实依赖。

混合场景须同时包含 shape、image、text 和自有布局，至少对照以下两组：
固定 viewport 的 pan／内部滚动，及同一场景的 resize／reparent／Freeform 节点移动。
前者检查合格局部内容是否仍被生产，后者检查必要重验是否遗漏。
另外覆盖祖先已普通失效、局部项待执行时节点退休、不同属性合并、output 显式
invalidate、reader 失败和完整证明冲突。仅检查 thumb 位置和文字摘要不足以关闭
像素、命中、当前坐标和通知门禁。

计费拆开记录声明表准入与留存、请求 payload、祖先查表、guard 重读、完整 input
重读、proof 留存、队列 metadata 和失败恢复。每次请求的传播检查按实际祖先链
计费，不扫描所有兄弟或全树；合法长链继续服从既有深度和事务预算。
沿用匹配的关闭 tracing 三轮对照，并单独以 Perfetto 检查工作次数。
本片若没有稳定净收益，则撤回性能接入并保留证据，不能把通用输入接口的正确性
验证当作局部队列生产启用的依据。

## 首片消费者

ScrollBar 只在 snapshot 的 `value` 改变，且 minimum／maximum／extent／enabled
等其他输入保持时，发出以 `SCROLL_VALUE` 为 typed 身份的请求。
guard reader 重读同一 RangeModel 的非 value 字段及其他实际外部尺寸输入；
执行前变化不能因请求时的 snapshot 相等而被遗漏。
任何非 value 变化继续普通失效。config、orientation、border、部件替换和
最小 thumb 输入的更新不借用这个资格。

ScrollPaneLayout 只对其登记的水平和垂直 scrollbar role 声明独立于
`SCROLL_VALUE`。它的布局仍依赖 viewport 首选尺寸、容器 bounds、visibility
policy、thickness 和 layout direction；这些变化照常使布局失效。
FillLayout 可对 child 内部的 `SCROLL_VALUE` 变化声明独立，
但 preferred／minimum 和容器 bounds 的普通失效保持。
首片不为其他 manager 批量添加声明。

外部消费者至少覆盖：自有 typed property、自定义五部件或其他容器；
未声明的 parent 读取 range value 并移动 sibling；已声明 parent 的纯内部变化；
上层未知 manager、reparent、manager 替换以及同轮普通失效升级。
另需覆盖 value consumer 在请求记录后改变 extent：执行前 guard 必须升级普通失效；
该 reader 失败、注册退役、预算拒绝和忽略错误的状态仍保持。
这些消费者与 ScrollBar 使用同一请求、声明和队列路径。

## 文字和坐标的正确性

本方案只限制布局失效传播。既有 `PreparationInputSnapshot` 的首次值、
注册弱引用、约束绑定、动态 reader、观察依赖和最终发布检查保持。
如果 contributor／inline 或布局持有外部状态并实际依赖 value，
其未声明路径继续布局；已有证明或 provider revision 的变化继续拒绝旧结果。
局部请求不能给任意 reader 增加跨回调缓存资格。

viewport 的 translation、属性通知、坐标通知和 repaint 继续执行。
caret、selection、命中、focus、连接锚点与 router 使用当前坐标；
scrollbar thumb 继续 layout。宽度变化、滚动条出现或消失、
track policy 反馈、真实 inline 和 resize 不能归为 value-only 请求。

## 验证与性能门槛

先以精确测试验证队列范围、升级优先级、当前 parent 链复核、直接旧 API、
预算拒绝、失败恢复、page 部件 resize 不重新传播以及外部消费者。
随后运行 scrollbar／viewport／文字交互、
连接和布局 suite；不改像素比较阈值或删除反例。

成本采样复用本轮冻结的 Parley 0.12 release 基线，原生交替运行至少三轮，
每轮两个 cycle，保持 tracing 关闭、自然 damage、GPU wait 和纹理缓存配置一致。
逐操作匹配位移与文本摘要，比较 active／frame／latency 的 P50、P95、P99；
首次、Stable、拖拽、滚轮、追加和 resize 分别报告。

Perfetto 独立采集，核对有效拖拽和滚轮的 Flow measurement hit、
qualification、input validation、layout、assembly、shaping 和 source geometry 次数。
两个纯投影测量 hit 是应从合格路径中消失的目标，实际计数以 trace 为准；
不将两个 hit 直接换算成预期提速。
原生呈现不等同 WindowServer 或 scanout 延迟。

仅有局部 scope 次数或缓存命中增加，不能作为保留理由。
若动态检查没有减少，或匹配的整体耗时无净收益，撤回实现并保留证据。
任何显著 resize／首次回退必须解释；整体 FR-02 门禁继续独立维护。

## 取舍与接受范围

直接把所有 scrollbar 失效改成局部请求最短，但会改变未知父布局行为；
从节点 valid 标记推断队列范围会丢失普通失效优先级，两者均不采用。
完整追踪任意 layout callback 对外部私有状态的读取能提供更广资格，
但需要新的可靠写入协议，超出首片。

本文选择 static typed 依赖声明与未知回退。代价是新增队列范围 metadata、
沿祖先链的有界查表和更保守的消费者资格。好处是旧扩展保持默认传播，
且局部重验不会成为 ScrollBar 的特权路径。
依赖和状态 owner 不变，具体协议进入设计专题，不为本方案自动分配 ADR 编号。

接受本方案表示同意请求资格、父链声明、队列范围及回滚规则；
本轮还需确认通用布局输入读取与完整首次值的消费／发布规则。
实现完成和生产启用仍需上述验证。当前没有实施本文的新 API。
