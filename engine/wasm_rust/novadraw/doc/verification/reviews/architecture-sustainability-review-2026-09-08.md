# Novadraw 长期架构与可扩展性分析

类型：`verification`

日期：2026-09-08

## 1. 结论与范围

**总体架构方向合理，不建议推倒重写。但目前还不能把“核心机制已存在”等同于
“长期运行可靠、外部可扩展、后端可完整替换”。主要风险是跨模块协议没有完全闭合，
其次才是模块体积和性能。**

建议保持 FigureTree、NodeState、Runtime、LayoutSnapshot/LayoutOutput、
RenderSubmission 这些主干，通过有边界的架构增量修复，而不是迁移 ECS、恢复迭代
渲染主线、泛化全部 manager，或者提前实现二进制 DisplayList。

本报告是当前实现的分析与建议，不覆盖设计 SSOT，也不自动改变 milestone 状态。
建议中的新接口和阶段均未获批、未实现。

### 分析基线

- 开始分析时 Novadraw HEAD：`49283a7102c9d9a86b6177138f22c431ceab28ae`。
- 分析对象包含当前工作区，开始时已有 Runtime/listener、apps 和测试等未提交改动。
- 分析期间这些工作形成 `5e1849b`、`2b85469` 两个提交；已复查变更范围，
  主要为 D3 listener 公共面和测试，不改变本报告的核心发现。
- 磁盘规则及路线图显示 D3.2 已完成、D3.3 正在推进；不重复列举旧审计中已补齐的
  layout mutation、child order 和 M9 shared reservation 缺口。
- Draw2D：`gef-classic`，提交
  `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
- 先阅读架构、动态协议、UpdateManager、迁移计划、Draw2D 公理和既有审计，
  再检查当前实现；直接复核了 Draw2D `Figure.paint/paintChildren/remove/primTranslate`
  与 `DeferredUpdateManager.performValidation`。
- 本次未修改产品源码。独立探针位于 `target/architecture-review-probe.rs`。

### 证据级别

- **已复现**：通过独立 crate 视角的公共 API 探针验证。
- **源码确认**：存在明确数据流或接口边界证据，但未运行对应完整平台场景。
- **待量化风险**：算法或结构有明确成本，尚未测量真实业务负载。
- **后续能力**：当前明确延后，不把尚未承诺的功能当作现有 Bug。

## 2. 最重要的发现

P1 表示应在稳定核心或相关替换能力交付前解决；P2 表示下一阶段架构改进。
不是安全漏洞分级，也不表示所有问题都需要同一批修改。

| 编号 | 优先级 | 发现 | 证据 |
|---|---|---|---|
| A01 | P1 | route dirty 没有进入正常帧的自动重路由闭环 | 已复现 |
| A02 | P1 | Label 的约束布局发生在容器布局之前，首帧可能使用旧宽度 | 已复现 |
| A03 | P1 | ResourceDelta 丢失 add/remove 之间的时序 | 已复现数据形态，源码确认后端后果 |
| A04 | P1 | detach 后没有公开销毁/回收闭环，旧节点持续留在 arena | 已复现 |
| A05 | P1 | FigureId 代际身份不隔离不同 Runtime | 已复现 |
| A06 | P1 | 外部 TextLayoutEngine 无法直接构造非空 TextLayout | 源码确认 |
| A07 | P1，后端替换前 | 全量重绘不重发已确认资源，后端重建缺少资源同步协议 | 已复现提交内容 |
| A08 | P1，性能门禁 | 递归绘制重新出现逐节点祖先样式扫描 | 源码与 release 测量 |
| A09 | P2 | Runtime/Tree 对具体 Figure 和共享容器状态了解过多 | 源码确认 |
| A10 | P2 | 输入取消、多 pointer、IME、accessibility 尚非完整跨平台协议 | 源码确认；部分已规划 |
| A11 | P2 | 错误模型混合 None、bool、null ID、Retry，难以区分故障与无工作 | 源码确认 |

## 3. 应当保留的设计

### 3.1 Figure 树与行为/状态分离

`SlotMap<FigureId, FigureNode>`、有序 children、`NodeState` 和 `Box<dyn Figure>`
适合异构图形编辑场景。代际 key 避免槽位复用造成陈旧引用，children 同时表达
绘制顺序和命中优先级，不需要再建立一棵权威“渲染树”或“交互树”。

与 Draw2D 的区别是合理的 Rust 所有权迁移，不是对标失败：

- Draw2D 通过对象引用和 GC 管理树；Novadraw 通过 arena 和 ID 管理。
- Draw2D 的 IFigure 很宽；Novadraw 将通用状态与差异化行为拆开。
- editor selection 已外移，不应重新塞入二维引擎。

需要补的是身份域与销毁协议，而不是替换 SlotMap。

### 3.2 局部二维坐标

统一 parent-content bounds、node-local 绘制和 child transform，比机械复制
Draw2D 分段坐标域及后代 bounds 平移更容易维护。

`FigureNode::client_area/child_transform`、坐标转换、命中和 damage 使用同一组
基础几何入口，这个方向正确。`f64` 二维几何有利于缩放与大画布精度。

长期保持二维 layout，2.5D 放到合成层，真正 3D 使用独立模型。不要将 Point 改成
Vec3，也不要为了 Vello 的类型选择反向改动领域坐标。

### 3.3 Runtime 与 effects

单 Runtime 独占树、InteractionState、UpdateManager、资源和连接状态，回调写 effect，
借用释放后提交，这比共享可变对象网络更符合 Rust。

保留具体 Runtime、EventDispatcher、UpdateManager。它们承载时序不变量，没有必要
因为“可扩展”就分别变为可任意替换的 trait。

### 3.4 布局快照与输出

LayoutSnapshot 是只读借用视图，LayoutOutput 是待提交变化。父容器拥有 constraint，
删除/reparent 清理关系约束，尺寸 override 与缓存分离，这些设计合理。

注意当前 Snapshot 不是可跨线程使用的深冻结快照；不能直接据此宣称支持并行布局。
只有真正需要 worker 时才建立独立 immutable job input。

### 3.5 渲染、文字与资源分离

命令使用项目自有类型，文字降低为 GlyphRun，图像通过 ResourceId/revision 引用，
Figure 不持有 Vello 对象。这已经建立了真实的后端中立基础。

RenderSubmission 将 commands、damage、resources、surface、frame_id 集中提交，
并通过 complete_submission 确认，是比直接调用 GPU 更可测试的边界。

### 3.6 路由的纯计算边界

Anchor、Router、SceneQuery、RouteOutput 分离合理。依赖反向索引、组级提交、
Fan 的 anchor-pair scope、Manhattan 的 routing-domain scope 值得保留。

shared lane reservation 不等于通用避障路径规划。当前没有必要为“长期发展”
立即实现 ShortestPath，优先接通自动 validation。

## 4. 核心更新协议

### A01：连接没有自动完成 dirty -> reroute -> paint

证据入口：

- `novadraw-scene/src/runtime/runtime.rs:379`：显式 `resolve_connection_route`。
- 同文件 `:443`：dirty 查询；`:1443`：has_pending_update；
  `:1966` 附近：prepare_submission。
- `doc/design/architecture/connection-routing.md:439`：正式规定的 validation 顺序。

公共 API 探针：

1. 建立两个矩形和已解析连接。
2. 完成首帧。
3. Runtime::set_bounds 移动 source。
4. 正常 prepare_submission/complete_submission。
5. 连接仍为 `Dirty { revision: 3 }`，路径 bounds 不变，
   `has_pending_update()` 已为 false。

这不是 Manhattan 算法缺陷，而是调度缺口。现有连接测试验证了 dirty 标记与显式
resolve；demo 也显式 resolve，因此尚未证明应用只修改模型即可正确更新整条链路。

此外，LayoutOutput 的 bounds 提交直接走 FigureTree，不经过 Runtime::set_bounds；
callback context 也有自己的写入路径。只补一个 prepare_submission 中的 reroute
调用不足以保证所有变更都正确失效依赖。

建议：

- 各种 geometry/topology/style/resource mutation 产出内部 typed change set；
  直接 API、callback、layout commit 消费同一套失效规则。
- 路由工作纳入 Runtime validation，dirty connection/group 参与 pending-work 判断。
- owner geometry 与 named anchor region 稳定后计算 route，接着更新 locator、
  decoration、extent、range，最后 damage 和录制。
- 显式 resolve 可保留为诊断或同步入口，但不能是正常动态连接正确性的必要条件。
- 失败提交 unresolved 并清除旧路径，而不是无限保留脏状态或旧线。

验收必须覆盖：直接移动、布局移动、callback 移动、端点删除、layer 重排，
均不显式调用 resolve，也不让 apps 手工排列这些更新阶段。

### A02：文本派生状态没有与布局共同收敛

`Runtime::prepare_submission` 先 refresh Label，再 perform_update/layout；
`FigureTree::refresh_label_layouts` 使用此时的 client area，
`LabelFigure::refresh_layout` 将 width/height 纳入缓存 key。

探针使用 StackLayout 和默认零 bounds 的 `LabelFigure::new("Hello world")`：

- 首帧提交后 Label 最终宽度已经是 500；
- 首帧记录 3 个 glyph，下一次显式 full redraw 记录 11 个 glyph；
- 首帧完成后 pending 为 false，没有自动补正下一帧的请求。

因此“文字有真实测量”尚不等于“本帧布局和绘制一致”。简单地在首帧后强制多画一帧
属于掩盖问题，不应采用。

建议分清两种派生计算：

1. 不受最终容器宽度限制的 natural measurement，用于父布局。
2. 得到最终 client area 后的截断、对齐与 constrained text snapshot。

将文本、owner geometry、路由、locator、freeform、viewport 纳入同一受预算保护的
收敛过程。输入没变的阶段跳过，不能每轮全树重建。

这里需要统一调度，不一定需要实现一个通用 DAG 框架。先把固定阶段与依赖关系写清，
再以 dirty reason/generation 驱动即可。

### 容器共享状态的事务风险

`container/viewport.rs:346-430` 在 layout 计算期间直接写共享 ViewportRuntime 和
RangeModel，再通过 LayoutOutput 记录通知。`range_model.rs:147-174` 的默认模型会
同步调用 RangeListener。

这意味着“所有布局变化先留在 LayoutOutput 中”并不覆盖容器模型；将来接入自定义
range listener/model 时，存在观察中间状态、失败后部分模型已变更、外层锁重入的风险。
当前外部自定义模型注入面较窄，本次没有复现用户可达死锁，不将风险写成已发生故障。

建议把 range/content-scale 更新作为内部布局提交的一部分，提交后再发通知。
先消除跨事务共享写入，再决定是否把 Arc<Mutex> 换成更符合单线程语义的存储。
单纯替换为 RefCell 不能解决重入与原子性。

## 5. 身份、内存与资源生命周期

### A04：区分 detach 与 dispose

证据：`graph/mod.rs:1049` detach_child、`:1232` remove_child_internal、
`:752` set_contents。当前移除清理 parent relation，但不从 blocks 删除 FigureNode；
整个图模块没有 blocks.remove/retain/clear 的销毁路径。

探针移除带 Drop 计数的 Figure 后：

```text
attached=false, node_still_stored=true, drops=0
drops_after_runtime_drop=1
```

Draw2D.remove 本来也是 detach；Java 中外部不再引用的对象可由 GC 回收。Rust arena
始终强持有对象，因此不能只迁移“摘树”操作而忽略回收机制。

建议显式定义：

- detach：允许保留子树时，谁拥有它、怎样重新挂载、何时释放。
- dispose_subtree：销毁节点和后代，使 ID 失效。
- contents replacement：调用方选择保留旧树或销毁，不能隐式无限保存。
- remove/dispose 同事务清理 interaction、constraint、resource dependency、
  connection、layer lookup、listener association 和缓存。

销毁前捕获旧 visual envelope，再处理 damage。不能为了回收而破坏旧区域擦除。
撤销历史应由 editor/document command 层明确拥有，而不是 arena 隐式保留所有旧节点。

这个问题还会放大 refresh_label_layouts/image_figures 的全 arena 扫描：
已脱离场景的对象继续消耗刷新成本，甚至参与不应再影响当前画面的文本准备。

### A05：代际 key 不等于 Runtime 身份域

`graph/mod.rs:49` 的 FigureId 是普通 SlotMap key；
`ResourceId` 则已有 namespace + generation_key。

探针建立两个相同顺序的 Runtime，得到相等的 root ID。将第一个 Runtime 的 ID
传给第二个 Runtime::set_bounds，操作成功并修改第二棵树。

这是 API 逻辑隔离问题，不是 Rust 内存安全漏洞。多文档、多个视图、异步结果返回和
插件都会增加混用 ID 的机会。

建议公开句柄携带 Runtime 身份域，内部继续使用紧凑的 local generational key；
或采用可证明隔离的 branded handle。不要简单给每个内部遍历都增加 UUID 查表。
同样复查 AnchorId、RouterId 和 listener ID 的跨实例策略。

FigureNode 目前还无条件保存 UUID 和 uuid_map，但 UUID 并没有用于这项身份校验。
应区分运行时 namespace 与可持久化业务身份；后者宜由上层可选映射管理。

### A03：ResourceDelta 必须表达有序状态变化

证据：

- `render/submission.rs`：ResourceDelta 分成 added 和 removed 两个 Vec。
- `runtime/resource.rs:244-260,318-345`：fail 追加 removed，complete 追加 added。
- `render/backend/vello/mod.rs:1406-1446`：先消费全部 added，再消费全部 removed。

探针在同一待提交窗口执行 Ready -> Failed -> Ready：
Registry 最终为 `Ready { revision: 2 }`，但 delta 同时包含该 ID 的 added 和 removed。
Vello 按当前顺序先上传新版本再将其删除。若命令引用该资源，
has_required_resources 会失败并返回 Retry；重复同一 delta 不会纠正这个顺序。
本次验证了公共 delta 内容及后端消费逻辑，没有运行 GPU 故障场景。

建议二选一并明确契约：

- 有序 ResourceOp 序列，如 Upsert/Remove，按真实发生顺序回放。
- 对每个 ID 归约为最终状态，但必须证明 retry、in-flight 和版本引用语义成立。

不应继续通过两个集合的各自 append 顺序声称保留完整因果关系。

### A07：后端重建需要资源重新同步

首帧资源被确认后，request_full_redraw 只重录 commands，下一帧 resource additions
数量为 0。Registry 仍拥有 CPU payload，但没有面向新 backend 的全量 ready snapshot
公共协议。

因此 fresh backend、device 重建、同一 Runtime 切换渲染器，并不等价于 surface resize。
旧资源 delta 的 retry 只覆盖未确认提交，不能恢复早已确认的资源。

建议引入 backend/session epoch 或明确的 attach/reset 事务：

```text
backend 建立或重建
-> 同步当前 Ready 资源快照
-> Full damage
-> 提交当前完整场景
-> 确认新的资源同步基线
```

单后端、单 in-flight 当前足够，不需要立即设计复杂多消费者资源系统。
先支持同一 Runtime 无损重建一个后端，再谈同时多后端或异步多帧。

### 异步加载的后续约束

Resource revision 当前按完成调用的到达顺序递增，不代表请求的新旧顺序。
在真正接入异步网络/worker 前，需要 request token 或 expected generation，
使过期完成消息不能覆盖更新的加载结果。

ImageData::decode 及 SVG rasterize 是同步 CPU 工作。适合作为工具函数，不应把大图
解码默认放进交互线程；Native worker、Web worker 只返回资源消息，不修改 FigureTree。
同时定义解码尺寸、字节数和缓存预算。

## 6. 扩展点与模块边界

### A06：TextLayoutEngine 的外部替换尚不完整

`render/text.rs:233-345` 的 TextLayout/TextLayoutKey 字段全部私有；
外部只有 Default、getter 和 with_visibility，没有从 glyph runs、line metrics、
font、revision 构建真实布局的公开入口。

因此外部 crate 可以实现 trait 并返回 Default，或者再委托 Parley，但不能直接返回
自己的非空布局结果。现有 StubTextEngine 测试返回 Default，只证明可注入，不证明
可替换真实排版。

建议提供受校验的 TextLayout builder/from_parts：

- 校验字体资源引用、有限坐标、度量和 UTF-8 range。
- 保持最终布局不可变。
- 不公开 Parley 类型，不要求 alternative engine 使用 Parley。
- 用外部 crate 构造非空 glyph run，并经 Figure 测量及独立 backend 消费来验收。

这是稳定接口前的实际缺口，优先级高于把 text.rs 单纯拆成几个文件。

### A09：模块边界仍受内置产品类型牵引

当前 `graph/mod.rs` 约 5,832 行，其中约 3,800 行为测试模块之前的实现；
`runtime/runtime.rs` 约 2,897 行，后半部分包含测试。行数本身不是缺陷，
真正问题是模块同时承担太多变化原因：

- FigureTree 知道 LabelFigure、ImageFigure、RoundedRectangleFigure、TriangleFigure
  及 clickable 的具体 mutation 和缓存更新。
- Figure::label/label_mut 返回具体 LabelFigure，不是独立文本内容能力。
- Image refresh 通过具体类型 downcast，定制图像 Figure 不能自然复用完整资源刷新链。
- Tree 直接依赖 UpdateManager，通用拓扑、变更提交与运行时调度耦合。
- 新增内置产品能力，往往同时修改 Figure trait、Tree、Runtime 和 facade。

建议先进行 crate 内按职责分组，不要一次拆成大量 crate：

1. topology/node/state/query。
2. geometry/layout validation 与 typed change output。
3. text/image/connection 的派生状态管理。
4. Runtime orchestration、input、frame preparation。
5. built-in Figures 与相应受控 mutation adapter。

Figure 的长期扩展面应让第三方图元能够：
提供本地 paint/hit/measurement，申请资源或文字布局，接收结果，通过受控事务更新
专属状态。可以使用小 capability 或类型化 Figure handle，但不要把 &mut FigureTree
开放给插件，也不要引入一个能执行任意副作用的万能回调来绕过不变量。

`Bounded`、paint_figure 与 paint_figure_in_bounds 等兼容双入口应在计划中的 capability
消融复查中收口。保留一条清楚的“构建数据 -> NodeState -> 本帧只读上下文”路径，
避免第三方误读构造期 bounds。

### 编译依赖与真正可替换性

`novadraw-render` 虽然能不启用 Vello，但仍无条件依赖 Parley、PNG/SVG 解码器；
macOS 的 objc2-quartz-core 也没有随 Vello feature 门控。facade 默认启用 Vello，
Native Vello feature 同时带入 winit。

这没有把 Vello 对象泄漏进 Figure API，却说明“运行时接口解耦”和“编译依赖隔离”
不是同一件事。

建议优先形成以下边界，是否独立 crate 由编译/发布收益决定：

- render protocol：Canvas、Command、Submission、资源描述和后端中立 glyph IR。
- text adapter：Parley 默认实现，可按 feature 或独立包选择。
- codec：PNG/SVG 等解码工具，可选。
- backend-vello：GPU 编码及内部资源缓存。
- platform-winit/web：输入、窗口、DOM 和 surface 创建适配。

先证明 `no-default-features` 的外部消费者能运行，再按稳定依赖方向拆包。
不建议把 Math/Geometry 的命名整理放在这些实际边界问题之前。

## 7. 算法与性能

### A08：已经复现的深树性能回归

`graph/render_recursive.rs:24-38` 的 resolved_style 每次分配祖先链并向根回溯，
`:113` 在每个节点 paint 时调用它。

对深度为 H 的 N 节点树，样式解析成本为所有节点深度之和；
链形树就是 O(N²)，10,000 层约有五千万级祖先访问。

这与性能文档中“D1 已改为 local override + Graphics state inheritance”的记录不一致。
Draw2D Figure.paint 直接应用 local 属性，随后依靠 Graphics 状态栈继承。

本次已运行 release 基准，初测 deep-tree median 为 765.199 ms，
command 数仍为 129,997。历史 D1 记录为 10.713 ms；
同为 Apple M1 Pro、rustc 1.94.1，但历史系统负载不可复现，不能将比值当作精确回归量。
追加三次标准复测及结果见第 10 节。

推荐恢复线性的样式继承或逐边传播的 resolved state，保留 self/children/border
和兄弟隔离语义。这是主循环保护区，需先评审 Draw2D 对标与文本继承测试；
本报告不直接修改，也不建议恢复迭代渲染。

### 其他算法评估

| 环节 | 当前成本/结构 | 建议 |
|---|---|---|
| 节点查询 | SlotMap 近似 O(1)，children Vec | 保留；不要先改 ECS |
| reverse-Z 命中 | 最坏 O(N)，逐边逆变换 | 中小场景合理；大场景按证据增加 broad phase |
| 坐标到 surface | 单次 O(H)，重复构建祖先链 | 高频跨节点查询可用 generation transform cache |
| invalid 入队 | Vec.contains，K 个不同节点入队可到 O(K²) | 顺序 Vec + membership set 或代际标记 |
| validation root 归约 | 上溯、按深度排序、dedup | 先量化；预算应区分工作量与真正不收敛 |
| dirty region | 每 Figure 合并，父链传播，区域两两合并 | 小 K 合理；阈值应在昂贵合并前限制成本 |
| partial recording | 仍遍历整棵可见树 | clip 不等于 CPU culling；按 visual envelope 剪枝 |
| Label/Image refresh | 全 arena 扫描，Label 再逐个解析样式 | 按附着状态、dirty reason 和资源依赖索引更新 |
| clickable 同步 | 每次 dispatch 扫全部 clickable | 只更新旧/新 hover、focus、press 及失效子树 |
| Manhattan lane | 每条扫描既有 group 路径，逐候选查 lane | 大 group 再引入事务内 lane index，不改 scope |
| glyph/command | Vec 中拥有命令与 glyph 数据，克隆可能复制 | 测量 allocation 后考虑 Arc slice/不可变共享块 |

`normalize_damage_regions` 在区域数量最终降至 8 之前已进行了两两比较；
输出数量上限不是输入计算成本上限。

Vello 当前先渲染 surface 尺寸 scratch texture，再复制 damage regions 到 retained
texture，最后 present。这个策略保证局部保留，但不能推断 GPU 工作量严格与脏区面积
成比例。需分别测 command encode、raster、copy 和 present，不能仅看 damage 矩形面积。

### 深度契约需要全链路验证

10,000 层限制不能只验证 render：

- render 使用 stacker::maybe_grow。
- hit_test_from_with 与 revalidate_with_update 仍直接递归。
- 现有 deep benchmark 调用 tree.render，不覆盖 Runtime validation 和深层输入。

本次没有运行到 stack overflow，也没有验证浏览器的 10,000 层实际执行能力。
需要 Native 默认线程栈、测试线程栈、WASM 的完整
build -> validate -> render -> hit -> mutate -> dispose 门禁。
支持递归是合理选择，但不能用单条渲染测试代表所有递归路径安全。

### 建议补的基准

- 1k/10k/更大节点，flat、deep、balanced 三类结构。
- 单节点移动、10% 节点变化、全量 layout、纯重绘分别测量。
- 大量 Label 的 style/font/width 变化与字体 fallback。
- 连接 group 增删、端点持续拖拽与 reroute。
- 长时间增删/换 contents 的 live node、资源和内存峰值。
- Web CPU 时间、wasm 大小、初始化耗时、缺 GPU 时的明确行为。

性能数据应来自完整 Runtime 路径。当前 R8 TextProbe 已预先 shaping，
不能代表真实 Label 的更新成本。

## 8. 跨平台与后端替换

### A10：平台适配层方向正确，输入语义尚不完整

Native/WASM/Headless 的逻辑像素约定、host 与 engine 分离值得保留。
但 PointerId 的数据类型存在不等于多 pointer 已实现：

- InteractionState 的主要 mutator 固定写 PointerId::PRIMARY。
- Runtime 公开输入主要是 dispatch_mouse_*。
- Web pointer handler 丢弃具体 pointer identity，down/up 固定作为 Left button。
- Web 注册了 pointerdown/move/up，但没有完整 pointercancel/lostpointercapture 路径。
- IME 目前主要是宿主允许状态和 cursor area，不是 composition/preedit/commit 协议。

建议先定义统一 InputEvent，再逐平台适配 pointer kind/id/buttons、cancel、失焦、
键盘逻辑/物理身份及 IME composition。桌面单鼠标兼容方法可以封装该入口，
不必删除所有已有 API。

Tooltip timer 和 accessibility 已在 M10.5 规划，保持该边界：
Runtime 拥有语义状态与 deadline，Host 提供单调时间/wakeup 和平台桥接。
无障碍树应是场景的派生语义投影，可过滤纯装饰节点，不必逐个 Figure 映射原生控件。

### RenderBackend 不应把永久不支持表示为 Retry

现有 RenderOutcome 只有 Presented/Skipped/Retry。扩展后端需要区分：

- 可重试 surface 获取失败；
- 资源尚未同步；
- DeviceLost，需要重建；
- Unsupported command/capability；
- 非法 submission 或不可恢复错误。

Canvas2dBackend 位于 Web 验证 app，已实现部分命令；
对 glyph/image 返回 Retry，其余未覆盖命令存在 `_ => {}`，最后返回 Presented。
这可以作为有限场景验证器，但不是语义完整的可替换后端。

不能把“存在两个 RenderBackend 实现”作为后端可替换验收。应明确 Core Graphics
必需子集及 optional capabilities，缺必需命令明确失败，不允许静默成功。

建议先实现独立、严格的命令验证后端，再选一个小规模 CPU/第二光栅后端验证：
状态栈、clip、transform、alpha、path、stroke、glyph、image 和资源重建。
验证后端负责协议检查，不等于像素等价验证；两类测试都要有。

### 后端中立的完整范围

稳定协议还需明确 surface background/透明度、颜色空间、alpha 类型、图片采样、
DPI rounding、非有限值、路径填充规则和文字合成语义。
不要让这些由 Vello 的当前默认值悄悄决定。

平台对象不进入 Figure 是已经做到的目标；可创建不同目标 surface、
异步初始化、suspend/resume、资源恢复与错误传播则是下一层验收。
Apps 中直接使用 VelloRenderer 作为 demo 没问题，不要把它当成通用嵌入接口。

## 9. 错误模型与实施顺序

### A11：错误应可区分、可恢复、可定位

具体证据：

- prepare_submission 在文本准备失败时返回 None，与无帧、surface 不可绘制、
  in-flight 未完成混在一起。
- add_child_to 失败返回 null FigureId；多项旧 Runtime mutation 返回 bool。
- 新 D3 mutation 已提供结构化 RuntimeMutationError，是更合适的方向。
- GPU render_to_texture 仍 expect；不能假定所有失败都会转换成 RenderOutcome。
- prepare_frame/record_full_frame 与 prepare_submission 形成不同完整性的录制入口。

建议：

- 统一规范的 frame preparation outcome，区分 Ready、Idle、Suspended、
  AwaitingCompletion 与具体 Error。
- 同一失败的直接调用和 deferred callback 具有一致语义；deferred 错误携带来源。
- 不要求所有实现细节都有独立错误枚举，优先把应用需要采取不同动作的情况分开。
- 只保留一个产品提交主路径；低层录制入口明确为诊断/测试或要求稳定快照。
- trace 使用可选、事务级、按因果顺序的诊断数据；不要在渲染热路径输出日志。
  当前 debug_render 仍直接调用 tracing::debug，开启日志时会逐节点执行，应一并治理。

### 建议推进顺序

**第一批：核心正确性与已有承诺收口。**

完成当前 D3.3/D3.4，同时将本报告已复现问题作为新增审计输入评审，不自动重编号。
优先修复资源时序、route 调度、text/layout 收敛、detach/dispose 与 ID 隔离。
性能回归可独立原子修复，但需要主循环修改评审。

验收：不靠手动 resolve/二次 redraw 的组合场景、同帧资源失败恢复、多 Runtime
误用拒绝、长时间增删内存回收、各阶段通知顺序和失败恢复。

**第二批：用真实消费者证明替换边界。**

补 TextLayout 构造协议、backend epoch/resource snapshot、明确错误和 command
能力契约；完成外部 Figure、外部 Layout、非 Parley engine、独立 backend 的最小样例。
这些样例应从外部 crate 只使用稳定 facade，不访问 crate-private 实现。

验收：同一 Runtime 换后端后首帧包含文字和图片，原有 Figure/布局不修改，
错误可解释，命令语义一致。

**第三批：平台发布面。**

继续既定 M10.5，补齐 pointer cancel、实际 IME/无障碍桥及 Web Text/Image/Widget。
macOS/Web/Headless 先闭环，Windows/Linux 依据现有 roadmap 做发布资格验证；
不把本次 WASM 编译通过说成全部平台完成。

**第四批：按负载做模块与性能增量。**

在正确性用例固定后，拆内部职责模块，隔离 adapter/codec 依赖；
再用完整 Runtime benchmark 决定 dirty set、transform cache、culling、资源预算、
command cache 和 lane index 的投入。

暂不优先：ECS 全面迁移、全域 Send+Sync、全套 manager trait、二进制/远程
DisplayList ABI、GPU picking、通用 3D、完整 widget toolkit、ShortestPath。

## 10. 验证记录

本次执行：

```bash
cargo test -p novadraw-scene -p novadraw-render --lib --tests --quiet
cargo build -p novadraw-scene -p novadraw-render --quiet
cargo check -p novadraw --no-default-features --target wasm32-unknown-unknown -q
rustc --edition=2024 target/architecture-review-probe.rs \
  -L dependency=target/debug/deps \
  --extern novadraw_scene=target/debug/libnovadraw_scene.rlib \
  --extern novadraw_render=target/debug/libnovadraw_render.rlib \
  -o target/architecture-review-probe
target/architecture-review-probe
```

结果：首次 398 项库/集成测试通过；D3 listener 新提交后再次运行，402 项全部通过。
核心构建与无默认后端 WASM check 通过。
探针输出：

```text
identity: equal_ids=true, foreign_mutation=true, second_x=12
remove: attached=false, node_still_stored=true, drops=0
remove: drops_after_runtime_drop=1
resource: status=Ready { revision: 2 }, delta_add=1, delta_remove_same_id=true
backend_replacement: full_redraw_resource_count=0
routing: after_frame=Dirty { revision: 3 }, old_bounds_unchanged=true, pending=false
label: final_width=500, first_glyphs=3, second_glyphs=11, pending_after_first=false
```

性能初测原始数据：
`target/performance/architecture-review-20260908.json`。
标准复测使用 `cargo run --release -q -p r8-perf -- --report=<path>`，
每轮 1 次预热、7 次采样，三轮顺序执行，不并行运行 benchmark。

| 场景 | 复测 1 Median | 复测 2 Median | 复测 3 Median | Median-of-medians |
|---|---:|---:|---:|---:|
| flat render 4096 | 2.473 ms | 2.006 ms | 2.055 ms | 2.055 ms |
| flat hit 4096 | 29.167 us | 29.042 us | 27.583 us | 29.042 us |
| deep render 10000 | 638.301 ms | 639.175 ms | 601.346 ms | 638.301 ms |
| text recording 1000 | 0.441 ms | 0.403 ms | 0.369 ms | 0.403 ms |
| viewport render 1024 | 0.427 ms | 0.409 ms | 0.377 ms | 0.409 ms |

原始数据：`target/performance/architecture-review-repeat-{1,2,3}.json`。
三轮所有场景输出 command 数与历史基线一致。深树三轮结果均远高于历史约 10 ms，
并有 O(N²) 源码路径对应，值得单独修复；不应通过放宽 15% 阈值接受。
本次没有重新 checkout 历史版本做严格 A/B，故不报告精确性能回归百分比。

没有执行 GPU 像素对比、浏览器交互、Windows/Linux 运行和全 workspace
fmt/clippy/test 发布门禁。没有以未运行的验证作为已完成依据。

## 11. 最终判断

Novadraw 已具备值得继续投入的架构主干，尤其是二维几何、树组织、Runtime
组合根和中立渲染 IR。长期风险不在于没有足够多的抽象，而在于有些抽象只完成了
类型层面，没有覆盖真实消费者、派生状态调度、资源因果顺序和生命周期。

下一阶段应把验收问题从“某个 trait/算法是否存在”改成：

> 一个外部应用只通过 Runtime 发起变化后，能否在一次完整更新中得到一致画面；
> 多个 Runtime 是否隔离；旧对象是否能释放；后端重建是否能恢复；
> 第三方实现是否能不改核心就提供真实非空结果。

这些边界闭合之后，模块拆分、性能索引、平台扩展和未来 GEF 层才有稳定基础。
