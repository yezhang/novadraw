# 共享视图与宿主背景

类型：`normative-design`

状态：`approved`；修订 3 已通过设计复核，2026-10-23 用户批准实施，功能验收未完成。

日期：2026-10-23

修订：3；按修订 2 复核补齐结构依赖、Runtime 强制准入与继承 alpha，已批准。

范围：官方示例计划 K07；参考契约为 Draw2D，不扫描 Novadraw Rust 实现推导目标架构。
本文件提出两个独立决策：共享场景捕获与视图、宿主背景统一合成。两者复用不可变资源
闭包，分别验收，已由 [ADR-044](../../adr/adr-044-shared-scene-capture-and-view.md)和
[ADR-045](../../adr/adr-045-host-background-submission.md)接受。
与现行 ADR 的增量关系见
[与现行提交协议的关系](#与现行提交协议的关系)。

评审记录：[K07 方案评审意见](../../verification/reviews/k07-shared-scene-view-background-review-2026-10-23.md)。
修订 2 的处理见 [首轮修订响应](../../verification/reviews/k07-shared-scene-view-background-revision-2026-10-23.md)。
后续依据 [修订 2 复核](../../verification/reviews/k07-shared-scene-view-background-revision-2-review-2026-10-23.md)
继续修改，本轮处理见 [修订 3 响应](../../verification/reviews/k07-shared-scene-view-background-revision-3-response-2026-10-23.md)。
原评审结论保持不变；
[修订 3 设计复核通过](../../verification/reviews/k07-shared-scene-view-background-revision-3-review-2026-10-23.md)
只证明设计契约，功能和平台证据按实施切片提供。

## 需求来自两个独立消费者

`ThumbnailExample` 的两个窗口观察同一个可变 source。主窗口拖动两个 Rectangle，
overview 保持 source 的宽高比。`parts/Thumbnail.java` 通过 `setSource` 注册
UpdateListener，在 `notifyPainting` 中检查 dirty Figure 是否属于 source 子树；
source 改变或 `deactivate` 时注销监听并释放图像。`getSourceRectangle` 定义捕获区域，
`adjustToAspectRatio` 扣除 Border insets 后确定目标尺寸。分块 GC、SWT 定时器与
macOS 图像缓存修补属于上游栅格实现，不是 Core 的公共协议。

`InheritBackgroundExample` 将同一段多行 Label 放进两个 Canvas。一个 root 绘制
不透明背景，另一个 root 透明，由宿主父 Composite 的渐变背景透出。背景属于宿主，
其位置不随 Figure viewport 滚动。仅在 Figure 树中再画一个渐变，不能证明宿主
背景继承成立。

参考路径位于 `/Users/bytedance/Documents/code/GitHub/gef-classic/`：

- `org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/thumbnail/ThumbnailExample.java`
- `org.eclipse.draw2d/src/org/eclipse/draw2d/parts/Thumbnail.java`
- `org.eclipse.draw2d.examples/src/org/eclipse/draw2d/examples/swt/InheritBackgroundExample.java`

## 单一源状态与只读视图

source Runtime 独占 Figure 树和 attached-state mutation authority。overview 不接管、
挂载或复制这些活 Figure，也不调用 source 的可变能力。它消费一次稳定化成功后发布
的 owned、不可变 `SceneSnapshot`。宿主持有 source 与 destination，并按序传递快照；
不使用 Singleton、共享可变 Runtime、私有 Figure timer 或后台全树逐帧扫描。

建议公开领域为 `scene`，通过 Runtime-scoped facade 提供受检操作，不扩张 Runtime
本体或平铺 root。下列名称表示候选概念，具体 Rust 签名在批准后核对：

| 候选概念 | 所有者与职责 |
|---|---|
| `SceneSourceId` | producer Runtime namespace 与 source slot generation；标识一个已登记捕获源，不包含消费者 binding generation |
| `SceneSubscriptionId` | producer 管理的独立订阅身份及 generation；同源的多个订阅互不关闭或覆盖 |
| `CaptureConfigRevision` | producer subscription 管理的捕获配置版本；rect 或捕获策略变化时递增 |
| `SceneSnapshot` | 源、订阅、捕获配置版本、stable source epoch、捕获矩形、drawing 与资源闭包的完整不可变结果 |
| `SceneViewBindingId` | destination 管理的绑定身份及 generation；重绑后旧投递不可写入新绑定 |
| `SceneCapturePolicyKey<P>` / `SceneCapturePolicy` | producer 的 source rect、捕获坐标和失效选择；使用 typed identity + payload trait + registry |
| `SceneViewPolicyKey<P>` / `SceneViewPolicy` | destination 的 aspect fit、alignment、最终 clip 和测量；使用相同公开扩展模型 |
| `SceneViewFigure` | 普通 Figure 组件，消费 binding；默认 thumbnail 与外部 view 共用同一注册、测量和绘制路径 |

首先只发布已提交场景状态。presentation 动画不写回 source state；如后续 view 需要
观察动画帧，必须显式增加独立版本和选择策略，不让 source epoch 混入 presentation。
此次 Thumbnail 静态拖拽不依赖该扩展。

## 与现行提交协议的关系

[ADR-031](../../adr/adr-031-graphics-recording-and-publication-boundary.md) 第 3、4 节
目前将 `runtime.frames().prepare_submission(...)` 定义为唯一公开帧准备出口，
禁止 Runtime 返回未提交的内部命令容器。本修订明确提出一个受限例外：
独立 scene capture 只输出通过校验的 `SceneSnapshot`，不返回内部 recorder、
任意 `CommandStream` 或可变 drawing 容器，也不成为第二个 surface 提交入口。

批准共享场景决策后，需将该例外写入 ADR-031，并在
[派生状态收敛](derived-state-convergence.md)中定义 capture 与 frame preparation
复用的稳定化内核和订阅错误隔离。capture 使用 source 的稳定几何、文字与资源结果，
按捕获策略进行独立受检录制；不能直接截取已经被主窗口裁剪或加入 presentation override
的 surface 命令。source Figure 的绘制仍发生在 source Runtime 内；
destination 只回放不可变结果，不跨 Runtime 调用 paint。

capture 不分配或推进 source session/frame，不建立 source in-flight，也不消费 source
damage、资源 pending log、确认 cursor 或 render recovery 状态。稳定化产生的正常
source 通知和 damage 仍由原有 source 事务处理，capture 无权将其标为已提交或已呈现。
destination 更新不要求 source 先 Presented，也不构成 source 的第二个 backend consumer。

| source 状态 | capture 的结果与调度 |
|---|---|
| surface 暂停或零尺寸 | 捕获矩形有效且 CPU 资源可用时仍可捕获；不创建 surface、不请求 GPU 呈现 |
| `AwaitingCompletion` | 旧在途 source 帧保持冻结；从当前已稳定 source state 捕获，后续工作仍留在 source pending 中 |
| backend 暂时失败或重建 | CPU 场景可稳定时捕获不受 backend cache 影响；capture 不恢复或确认 source 提交 |
| source 本体 faulted 或派生状态未收敛 | 返回对应 source/stabilization 错误，保留上次成功快照，等待输入修复或显式重试 |
| 单个订阅的配置、录制或闭包校验失败 | 只拒绝该订阅的 candidate；其他订阅和 source 主窗口仍按各自协议准备 |

主窗口执行自身录制时若遇到同一个 Figure 错误，仍按 ADR-031 拒绝该帧；
订阅错误隔离不会吞掉主窗口原有错误。capture 的预算与重试属于订阅驱动，
失败不制造无上限 wakeup，需新输入、配置变更或显式重试后再尝试。
第三方回调 panic 或引擎结构不一致仍遵循现行 Runtime fault 边界，不被折叠为普通
订阅错误，也不通过本协议承诺继续使用已 faulted 的 source。

宿主背景决策另需修订 ADR-031 的统一输入冻结、
[资源因果与 Backend Session](resource-lifecycle.md)的背景 lease 与 session 恢复，
以及 [坐标契约](../coordinates/coordinate-system.md)的宿主到 Canvas 帧输入关系。
保持单个 destination in-flight 和原有 `prepare_submission` 提交权威，不改变
ResourceSync 中每个 destination ResourceId 仅保存一个当前 Ready revision 的规则。
上述增量已由 ADR-044/045 接受；实现完成范围和证据按执行切片记录。

## 身份与版本分别比较

snapshot 的实际内容身份由
`(SceneSourceId, SceneSubscriptionId, CaptureConfigRevision, stable_source_epoch)`
共同确定。投递 envelope 另携带目标 `SceneViewBindingId`，不将 binding 放进 source epoch。
source、subscription 和 binding 的 slot generation 分别由各自所有者管理；
配置 revision 仅在同一 subscription 内比较，stable epoch 仅在同一 producer 内比较。
跨 namespace 不建立时间顺序。所有 generation/revision/epoch 耗尽均返回结构化错误，
不得回绕、复用旧身份或发布候选结果。

destination 先检查当前 binding generation 和其期望的 source/subscription/config。
旧绑定、旧源或旧配置的投递返回明确 stale 错误。同一订阅的新配置可在同一 stable epoch
生成并安装不同快照；Host 需先受检更新绑定期望配置，后安装该配置的结果。
更新配置不会伪造 source mutation 或递增 source epoch。当前配置下的倒序 epoch 被拒绝。
同一有效 source 上切换配置也不能倒退已安装的 stable epoch；新配置从当前稳定场景捕获，
只有替换为新 source identity 才重建该比较基线。

相同完整身份和版本、相同不可变内容属于幂等投递；同一身份版本携带不同捕获矩形、
drawing 或资源闭包时返回 `SnapshotConflict`。不能只按 epoch 或未经核验的摘要判重。
展示策略和 destination resize 只重新测量、布局和绘制已有快照，不制造新源版本；
后端重建也保持快照内容身份不变。

## 按因果顺序捕获和安装

1. source mutation 提交，Core 执行现行 layout、route、text/resource preparation。
2. producer 固定 subscription/config revision 和 stable epoch，在捕获域录制 candidate。
3. 校验绘制、资源引用和完整闭包，再发布 owned snapshot；失败返回 `Result`，
   不发送半成品。内部 recorder、Figure、capability、LayoutManager 或回调不能进入快照。
4. Host 按绑定身份投递；destination 整批预检版本、依赖环、资源映射和命令，
   成功后一次安装完整 view。安装失败不部分改变 binding、导入表或当前可见结果。
5. destination 请求测量和完整旧/新 view damage；后续统一冻结帧输入并准备
   RenderSubmission。
6. Host 提交 backend；通过 destination 的 session、retry 与确认协议呈现和回收 lease。

capture/import 失败保留同一有效 binding 上次成功的 view，错误可查询并可显式重试。
source 或 binding 已终止时，旧图像不再作为 fallback，按终止协议清空。
绘制准备和资源映射复用现有通用机制，继续保护 `render_recursive.rs` 的主循环。

## 订阅关闭与慢消费者

每个订阅具有 `PendingInitial → Active → Closed` 状态。首次登记主动请求当前稳定
snapshot，不等待下一次 source mutation；尚未稳定时保留 PendingInitial，Host 安排
一次 capture drive，失败按前述重试规则处理。订阅配置改变清除旧配置的未消费 candidate，
在当前稳定 epoch 重新捕获。关闭为终态，不允许迟到 Ready 恢复该订阅。

producer 每个订阅仅保留一个最新完整 snapshot 槽、一个最近错误槽和一个终止事件槽。
普通更新可覆盖未消费旧更新，不能覆盖终止事件。关闭或源替换优先撤销普通更新，
发布携带 source/subscription generation 的 `Closed` 事件；事件保留到 Host 确认接管。
源替换通过关闭旧订阅、建立新身份完成，不能把两者合并成一次普通快照更新。
不同订阅互不共享关闭状态。

Host 持有可在 source dispose 后继续读取的 owned 交接句柄。source 删除、取消订阅、
Runtime dispose 在生命周期处理中写入终止事件并唤醒 Host，不依赖再绘制 source 帧。
Host 接管后负责保留事件，直到 destination 应用或目标明确销毁；该交接确认与
RenderSubmission 的 Presented/Retry 确认分开，不能互相推进 cursor。

destination 暂停时，Host 每个 binding 保留一个最新更新和终止状态，恢复后优先处理
终止，再处理有效新绑定的最新完整结果。source 不等待 destination 呈现或 GPU 释放，
不无限保留未消费历史。Host 若退出，必须显式取消订阅和 binding；本方案不承诺宿主
停止执行或进程崩溃后仍能呈现清理帧。

终止在 destination 中受检清空 view、释放 binding 引用、记录旧可视区域完整 damage，
并请求一帧；零尺寸或暂停 surface 保留清理工作直到恢复。旧在途帧的 lease 仍按提交
协议完成或失效，完成回调不能重新安装 view。首批每次安装和更新合并都修复完整
旧/新 view 区域，不用最后一个 epoch 的局部 dirty 推算被跳过更新的累计 damage。

## 资源闭包与多版本共存

snapshot 包含所引用 image/font 的不可变 payload 与 revision；不依赖 producer
RenderSubmission 是否已发送或确认。producer 的 resource ID 只作为 namespaced
来源身份，不能直接进入 destination 的资源注册表或 backend cache。

导入键为 `(producer namespace, source ResourceId, source revision, resource kind)`。
同一键去重，不同来源 revision 分配不同 destination typed ResourceId，允许同时 Ready。
这些导入 ID 的 payload 在存活期间不可被另一来源 revision 覆盖；未来 source 新版本
映射到新 ID。drawing 中所有 image/font 引用完整重写后才能安装 view。
source revision 与 destination Ready revision 是不同计数，不能直接互换。

CPU 导入表保留来源 payload，并追踪待安装候选、已安装 binding、保留快照和在途提交
lease；任一有效引用存在时不移除对应 destination Registry 资源。
多个 view 可共享同一导入键，也可分别使用旧、新来源 revision。
最后有效引用释放后才能按现行 Remove/确认协议回收；source 关闭或 mailbox 覆盖旧更新，
不强制回收仍被 destination 快照或提交持有的版本。

device reset 使旧提交 token 失效，但不能丢失其尚未完成转交的 CPU lease。
恢复准备必须先将仍需保留或重新提交的 lease 接回 CPU 导入表，再释放旧 token lease。
新 session 的 ResourceSync Snapshot 包含所有仍被引用的 destination ID，每个 ID
仍只有一个当前 Ready revision；因此旧、新来源版本都可恢复而不改变现行 cache 规则。

跨 backend/device 重建由 destination session 完成同步；source resize 和 target resize
不创建新 backend session。资源闭包遗漏、引用不匹配或导入失败返回受检错误，丢弃候选，
不部分替换当前 view。新 glyph 必须引用其冻结字体版本，旧 drawing 也不能读到新 payload。
外部副作用不纳入回滚承诺。

## 捕获坐标独立于主窗口

默认 Thumbnail 以 source 的 node-local border-box domain 捕获。
源捕获矩形可有非零原点，结果平移到该矩形原点；source placement、local-to-parent
变换及 source 外部 Viewport 的滚动/缩放、ancestor clip 和 surface clip 不自动带入。
resolved foreground/background/font/alpha 作为捕获根的继承输入冻结；
从 source 到其后代的内部变换、insets、可见性与裁剪仍按正常 Figure 协议应用。

继承样式在捕获入口解析一次，然后按
[Figure 样式契约](figure-style.md)沿子树递推。alpha 使用最近显式值：
祖先 alpha 为 0.5、source 未覆盖时，source 以 0.5 绘制；
子节点显式覆盖为 0.8 时，该分支使用 0.8，其未覆盖后代继承 0.8。
排除外部绘制路径不能丢掉入口继承 alpha，也不能再把祖先值与子节点值相乘。
捕获根的 resolved alpha 及内部 local override 都属于同一稳定快照，变化时重新捕获。

捕获域的外层 clip 固定为 capture rect；子树内部 clip 只能按现有绘制协议进一步约束，
不能越过捕获边界。source rect 与 drawing、资源、resolved style 属于同一稳定结果。
外部样式继承改变会使捕获失效；只改变主窗口 Viewport origin/scale/clip 不改变默认
capture 的比例或可见范围。实际 source geometry 改变则重新捕获。

若消费者需要主窗口可见区域，需显式选择独立 capture policy，声明其外部坐标、clip
和观察版本；首批默认 Thumbnail 不启用该策略。捕获转换复用
[坐标 SSOT](../coordinates/coordinate-system.md)，不复制父链绝对偏移算法。
K07 使用逻辑单位，DPI 属于目标帧输入；未定义 target DPI 驱动的源栅格重采样。
view 无新 snapshot 时不重新捕获；首批不承诺部分捕获或性能收益。

## 展示策略与测量由目标拥有

producer capture policy 决定 source rect、捕获域和失效条件。
producer subscription 管理前述有界合并；该生命周期规则不是可被 policy 绕过的选项。
destination view policy 接收不可变快照的逻辑尺寸、`MeasureConstraints`、Border 度量
与本地展示配置，提供 preferred/minimum measurement、aspect fit、alignment 和最终 clip。
调整目标展示策略无需修改 source subscription，错误归目标测量/展示 `Result` 通道。

默认 Thumbnail 没有显式 preferred size 时，以 capture rect 的逻辑尺寸为自然 content
size；无约束返回该尺寸，有轴向尺寸提示时，以自然尺寸和提示共同限定的 content box
做等比 fit，不向提示之外扩张。两个提示都有时取两轴可容纳比例；只有一个提示时，
由该轴限定比例，另一轴按 source 比例派生。所有输入先由 Core 扣除 Border insets，
policy 只返回 content `FigureMeasurement`，Core 最后加回 insets，各阶段仅应用一次。

显式 preferred `Dimension` 优先成为展示目标盒，再在该盒内等比 fit，并受
`MeasureConstraints` 限定；不保留 Draw2D 无提示时依赖当前 view bounds 的测量循环。
这是明确的 Rust 变体，不宣称所有 `Thumbnail.getPreferredSize` 分支逐字等同。
minimum content size 为 `Dimension::ZERO`，允许缩至空视图；最小外框仅保留 Border insets。
最大尺寸与显式大小仍通过现有 Figure 测量契约受检处理。

布局后的绘制使用扣除 insets 的真实 client area：
`scale = min(client_width / source_width, client_height / source_height)`。
默认 alignment 为 client 起点，外部 policy 可选择其他对齐方式；剩余空白不拉伸 source。
最终 clip 属于 destination 的正常 Figure/ancestor 协议。零尺寸 source/client 输出空
drawing 和零 content measurement，不除零；负值、非有限值和计算溢出返回受检错误。
source 比例或捕获配置尺寸改变使 view preferred measurement、layout 和 drawing
一起失效；展示配置、Border 或 destination resize 只使目标失效，并修复完整旧/新区域。

## 循环绑定在准入时拒绝

首批采用保守的结构依赖，拒绝直接和间接循环，不建设通用依赖调度器。
对已登记 source A，以其捕获根的整棵 Figure 子树为统计范围，包括根自身。
只要其中存在绑定到 source B 的 imported view，就登记 `A → B`；
不因 view 位于 capture rect 外、hidden、零尺寸、clip 截断或尚未 Ready 而删除边。
同一 view 位于多个已登记捕获根的子树中时，分别为这些 source 登记依赖。
矩形、可见性和裁剪只影响绘制，不决定依赖成员。

因此，移动或 LayoutOutput 将 view 从 rect 外移入、hidden view 显示、clip 放宽
均不会增加依赖边。反例中，A 子树里的 B view 即使当前在 rect 外，也已形成 `A → B`；
随后建立 `B → A` 时立即拒绝，不能先允许绑定再等 capture 报错。
该模型可能拒绝永远不可见 view 形成的结构环，这是首批明确接受的保守限制。

capture rect 和普通绘制 policy 调整不缩小结构统计范围。改变捕获根、topology、
view 来源或任何会改变 source 根/绑定关系的配置，均按下述提交协议预检。
policy 不能用“当前没有画到”或私有绘制路径隐藏结构依赖；外部 view 使用相同的
typed binding descriptor 登记来源。未知、跨组或无法核验的来源返回身份/依赖错误。

### Runtime 统一提交强制准入

composition root 创建 Core binding group，将其受限句柄交给参与的 Runtime。
group 保存已提交的 source/root/binding 身份、结构依赖和单调 `graph_revision`，
不持有 Runtime 可变引用、不使用全局状态。Runtime 通过私有提交权限更新它；
Host 只负责登记参与者与驱动，不持有可替代 Runtime 预检的任意“已通过”凭证。
多个 Runtime 的依赖提交由同一个 group 实例串行保护，不能各自维护互不一致的图副本。

直接 Runtime/facade API、callback 延迟 Effect 的每个原子批次、typed component update
和 derived output 都进入同一个 Runtime 提交器。
候选若会改变捕获根、子树成员或 view 来源，提交器根据候选完成后的结构计算整组边变更，
包含旧父和新父所属的全部捕获根；不能在写入后依赖 notification 或下一次 capture 补图。
callback reparent、subtree add/remove、view 重绑及 source/root/config 变更都受此约束。
新增公开 typed effect 若能改变上述关系，也必须进入同一路径，内置能力没有旁路。

仅输出 bounds/visibility/clip 的 LayoutOutput 不改变结构边，按现行完整输出预检和
原子提交执行；它无须应用手工提交 graph 候选。若 derived output 携带获准的关系变更
effect，Runtime 提交器必须执行与直接 API 相同的图准入，不能由 LayoutManager 自行
写共享图。整批候选一起校验，不能先接受部分 reparent 或资源写入再检查剩余关系。

### 图版本与候选在提交点核对

Core 内部准入候选固定 group identity/generation、`base_graph_revision`、
所用 Runtime 的 topology/binding/source-config generations 和完整待提交操作。
候选只能用于该次、该 payload 的提交，不能被 Host 或外部 capability 伪造、改写或复用；
RenderSubmission session/frame、source stable epoch 和 graph revision 各司其职。
实际提交顺序为：

1. 在写入前完成现行结构、资源和 derived output 全批预检，计算候选依赖增删。
2. 取得 group 的独占提交保护，重验图版本、本地相关 generations 及候选操作身份；
   从当前已提交图加候选增删检查直接/间接循环，并预留所需内部写入容量。
3. 版本过期返回 `StaleSceneAdmission`，循环返回 `CyclicSceneViewBinding`，
   耗尽或身份失败返回各自结构化错误；丢弃候选，不写源结构、依赖图、资源或 lease。
4. 校验成功后，在同一提交保护和 Runtime mutation scope 内安装已预检的本地结构/
   binding 与 group 边变更；提升相关 generations 和 graph revision，再产出 committed
   facts、通知与 damage。该段不执行用户回调或析构用户对象，不留可失败的外部步骤。

即使预检与提交之间其他 Runtime 已改变 group，旧候选也必须在第 2 步失效；
只有从当前图重新形成并校验的新候选可继续提交。候选通过时的图版本不能无条件沿用。
不相关的纯几何变化不提升 graph revision；关系或其准入配置的实际提交提升版本，
即使两条相同来源边的去重结果未变，也要使依赖旧成员关系的候选失效。
group 解除、身份和版本耗尽均不能复用旧凭证。

capture 冻结结构依赖元数据，destination import 在提交点依据当前 group/root/binding
关系重新核验；冻结的直接来源成员不能因当次 rect/clip 过滤而删边，传递闭包由当前
group 图计算，不能信任旧闭包。历史 graph revision 放在准入元数据中作来源记录，
不改变已定义的快照内容身份，也不当作当前准入票据；图已变化时须通过当前版本的新预检，
无法匹配当前源结构的结果被拒绝。
capture 后检查只是防止旧结果绕过准入，不能承担结构写入前拒绝的保证。

本协议延续 [ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)：
新 source operation 预检失败不修改该操作；FIFO 前缀已经提交的操作不回滚。
derived output 准入失败丢弃整个候选输出、保留 dirty work，不回滚此前接受的 source 输入、
不晋升 stable epoch 或发布半个结果。循环错误不产生新依赖边、部分资源引用或持续 wakeup，
恢复依赖新输入或显式重试。跨 Runtime 只串行提交依赖元数据，不提供跨 Runtime 模型事务。

## 宿主背景纳入统一受检提交

Host 的背景提供者输出候选 `HostBackgroundSnapshot`，包含背景 identity/generation、
request generation、revision、宿主覆盖区域、宿主到 Canvas 的变换与不可变资源闭包。
provider 在 Host 运行，Core 接收受检完成结果，不持有平台 I/O 或轮询 future；
遵循 [ADR-030](../../adr/adr-030-resource-provider-and-request-token-boundary.md)的边界。
背景适配能力使用 typed key + payload trait + registry；默认与外部实现使用同一
候选完成入口、资源闭包和错误通道。

提交权威仍是 destination Runtime。Host 在 frame preparation 前交入候选背景和
surface 输入版本；`prepare_submission` 统一预检并冻结背景、Canvas drawing、
所需资源 lease、surface/坐标版本，生成一个受检 RenderSubmission。
帧包表达背景与 Canvas 的组合输入，backend 只消费该帧包。
Host 不能在 Runtime 准备完成后追加未校验背景命令；宿主 overlay 保持其现行协议，
不借 overlay 绕过背景的预检、retry 或确认。

背景变换必须声明依赖的 surface/宿主位置版本。版本不匹配时拒绝该候选并保留 pending
work，不能把旧背景位置与新 surface 或资源组合成成功帧。
逻辑合成顺序为宿主背景、Canvas 场景、现有宿主 overlay。透明 root 保留 alpha，
opaque root 在自身覆盖区域遮住背景。宿主到 Canvas 的转换使用帧内冻结坐标，
不附加 Canvas viewport origin；移动 Canvas 与滚动内容是两个独立输入变化。

背景独立变化、移除、窗口位置、DPI 和 surface repair 都产生 destination pending
work 并请求 Host 唤醒；无需 Figure mutation。背景移除推进 generation，明确采用宿主
配置的基础清屏值，记录完整 surface damage；旧异步结果返回 stale 错误，不能恢复背景。
候选加载失败记录结构化错误，保留当前已接受背景；若其坐标版本与当前 surface 不匹配，
帧准备返回错误，不提交混合版本。初次背景 Pending 可使用显式配置的基础清屏值，
不得伪装成继承背景已完成。

Retry/Failed 按现有提交协议恢复 pending 前缀，并保留背景修复工作与 CPU lease；
下一次统一 preparation 从当前受检输入冻结一个内部一致的新帧，不将旧背景与新场景
随意拼接。Presented 只确认对应冻结前缀，期间产生的新背景更新继续 pending。
device reset 保留当前背景和资源 payload，新 session 用完整资源 Snapshot、背景、
Canvas drawing 与 Full damage 重建；失效 token 的迟到完成不得清除新工作。
不支持所需 alpha/组合命令的 backend 在提交前结构化拒绝；该平台验收仍未完成。
首批不读取操作系统桌面或父控件像素，只消费 Host 显式提供的背景。

## 逐项验收与外部消费者

独立消费者 crate 必须注册自定义 capture policy、view policy 和背景适配能力，
不改 Core，与默认组件共用测量、配置、资源、版本和失败通道。
一个 source 同时连接两个 destination，并在同一 destination 保留不同来源版本的
两个 view；验证 source mutation 仅发生一次、绑定互不覆盖、关闭一个不影响其他绑定。
不能用重复场景模拟共享源。

| 评审编号 | 实施验收反例 |
|---|---|
| K07-R01 | 主窗口滚动/缩放不截掉默认 overview；非零 source rect、内部嵌套 clip、继承字体与非单位 DPI 正确；祖先 alpha 0.5、source 无覆盖和子节点显式 alpha 0.8 按最近值绘制，不沿链相乘 |
| K07-R02 | source 暂停、旧帧在途、backend 失败时仍按表捕获；damage、pending resource prefix 和确认 cursor 不被 capture 消费；单订阅失败不阻断其他出口 |
| K07-R03 | 多 binding 独立；同 epoch 新 config 正确更新；旧源/配置/绑定迟到结果被拒绝；重复幂等、同版本异内容报错 |
| K07-R04 | 初始主动快照；暂停目标恢复获得最新值；不再绘制的 source 仍交出 Closed；终止不可被迟到更新覆盖；更新合并后完整擦除旧像素 |
| K07-R05 | 只变背景可提交；版本不匹配不提交混合帧；背景移除/失败/迟到、Retry/device reset、Canvas 移动与内容滚动分别验证 |
| K07-R06 | 旧、新 font/image 分别映射为共存 ID；旧帧在途再 reset 后仍保留所需 payload；导入失败不部分安装，旧 drawing 不读新资源 |
| K07-R07 | 无提示/单轴/双轴提示、显式 preferred、minimum、比例改变与零尺寸；Border insets 只扣加一次；外部 policy 错误可恢复 |
| K07-R08 | rect 外/hidden/clipped 的 view 已登记结构边；移动、LayoutOutput、显示及 clip 放宽不增加边；直接/间接环、callback reparent、subtree/root/来源变更由 Runtime 整批拒绝；过期 graph 候选被拒绝，跨 Runtime 交错提交仍无环；derived 失败不回滚先前源输入，没有部分图更新、资源引用或持续 wakeup |

上述为待执行验收，不是已通过证据。Native 分别提供 Thumbnail 拖拽与 overview resize、
opaque/transparent 背景、Canvas 移动/滚动及 DPI/repair 图像证据。
实际实现不得提前声明 Web/Windows/Linux 通过。

## 两个决策分别批准与实施

共享场景决策先固化 ADR-031 的 capture 例外、身份/终止与资源共存契约，再按单链推进
source capture → destination 导入与 view → Thumbnail 外部消费者和 Native 验收。
宿主背景决策随后独立固化统一帧输入、资源 lease 与恢复契约，再接入背景提供者、
backend 完整消费与 InheritBackground 验收。Thumbnail 通过不能替代背景合成验收。

每个决策实施前分别补 ADR、API 账本、外部消费者、书籍与验证入口。
执行队列仍只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护，
本页不建立第二份执行状态表。已批准按两个决策实施，验收前不宣称完整功能完成。
