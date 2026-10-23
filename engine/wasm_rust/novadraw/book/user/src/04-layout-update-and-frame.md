# 4. 布局与更新：从尺寸计算到稳定帧

> **本章解决的问题**：应用应如何选择和配置布局，状态变化后引擎又如何得到一份可
> 安全呈现的新帧。

核心顺序只有一条：

```text
源状态变化
-> 测量与布局
-> 派生状态收敛
-> 计算重绘区域
-> 录制绘制命令
-> 提交并确认一帧
```

应用开发者重点阅读 4.1-4.6。实现自定义 Figure、LayoutManager、平台宿主或后端时，
再继续阅读 4.7-4.12 的协议细节。

## 4.1 失效与重绘不是一回事

状态变化后，Runtime 记录两类事实：

- **失效**（invalid）：布局、连接或其他派生状态已经过期，需要重新计算；
- **脏区**（dirty region）：某个节点本地区域的像素可能变化，需要重新绘制。

失效回答“哪些数据要重算”，脏区回答“哪些像素要重画”。只有颜色变化时可能只需
重绘；文本或边框改变首选尺寸时通常还需要重新布局。

正确顺序必须是先收敛几何，再把脏区变换到逻辑表面域。否则重绘区域会使用旧位置，
造成残影或漏绘。

## 4.2 布局器只计算，不直接改树

**布局管理器**（`LayoutManager`）根据容器、直接子节点和布局约束计算位置与尺寸。
Runtime 向它提供只读的 `LayoutSnapshot`，布局器把候选结果写入 `LayoutOutput`：

```rust
pub trait LayoutManager {
    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError>;
}
```

```mermaid
sequenceDiagram
    participant Runtime
    participant Snapshot as 布局快照
    participant Layout as 布局管理器
    participant Output as 布局输出
    participant Tree as 图形树

    Runtime->>Snapshot: 建立只读视图
    Runtime->>Layout: 计算全部候选结果
    Layout->>Output: 写入子节点边界
    Runtime->>Runtime: 校验输出
    Runtime->>Tree: 原子提交
```

布局器不能直接持有或修改 `FigureTree`。这样 Runtime 才能在任何可见变化发生前检查：

- 目标是不是当前容器的直接子节点；
- 坐标和尺寸是否有限；
- 约束类型是否匹配；
- 一组输出能否整体提交。

## 4.3 布局约束属于父子关系

布局约束描述“父容器如何放置这个子节点”，不是子节点自身的固有属性：

```text
LayoutState(container)
├── LayoutManager
└── child FigureId -> LayoutConstraint
```

因此：

- 约束由直接父容器解释；
- 换父节点时必须清理旧约束；
- 更换布局器前必须验证已有约束是否兼容；
- 约束类型错误应被拒绝，不能静默忽略。

构建期配置：

```rust
use novadraw::figure::RectangleFigure;
use novadraw::layout::{XYConstraint, XYLayout};
use novadraw::tree::FigureTree;

let mut tree = FigureTree::new();
let root = tree
    .builder()
    .set_contents(Box::new(RectangleFigure::new(0.0, 0.0, 800.0, 600.0)));

tree.builder()
    .set_layout_manager(root, Box::new(XYLayout::new()))?;

let child = tree.builder().add_child(
    root,
    Box::new(RectangleFigure::new(0.0, 0.0, 120.0, 80.0)),
)?;
tree.builder().set_layout_constraint(
    child,
    XYConstraint::at_size(64.0, 48.0, 120.0, 80.0),
)?;
tree.builder().validate_subtree(root)?;
```

运行期配置：

```rust
runtime
    .container(root)?
    .set_layout_manager(Box::new(GridLayout::new(3)))?;
runtime
    .figure(child)?
    .set_layout_constraint(GridConstraint::fill())?;
```

两条路径使用同一布局契约，但只有 Runtime 路径会维护已挂载场景的失效、damage 和
通知。

## 4.4 如何选择布局器

应用通常只需要从意图出发选择：

| 需求 | 布局器 | 说明 |
|---|---|---|
| 业务模型给出明确坐标 | `XYLayout` | 支持固定尺寸或回退到首选尺寸 |
| 自由分布并推导整体范围 | `FreeformLayout` | 常用于大型自由画布 |
| 一个内容填满容器 | `FillLayout` | 只排列第一个子节点 |
| 多层内容完全重叠 | `StackLayout` | 所有子节点占满客户区 |
| 上下左右中五区 | `BorderLayout` | 主区、侧栏、标题栏 |
| 自然排列并允许换行 | `FlowLayout` | 标签组、按钮组 |
| 固定行列、跨格和对齐 | `GridLayout` | 表单、属性面板 |
| 单行或单列并允许压缩 | `ToolbarLayout` | 工具栏、菜单条 |
| 一个可滚动内容 | `ViewportLayout` | 由 Viewport 自动安装 |
| 视口加滚动条 | `ScrollPaneLayout` | 由 ScrollPane 自动安装 |

`ViewportLayout` 和 `ScrollPaneLayout` 是容器内部协议，应用通常不直接构造。滚动缩放
场景见[第 6 章](06-layers-and-viewport.md)。

完整布局示例集中在
[`examples/scenes/src/layout.rs`](../../../examples/scenes/src/layout.rs)。阅读时优先看场景构造
函数，不需要从 native 应用的窗口代码开始。

## 4.5 测量先于排列

**测量**（measure）回答“希望占多大”，**排列**（arrange）回答“最终放在哪里、占
多大”。父布局必须先测量子节点，再计算轨道、换行和对齐。

尺寸查询包括：

| 查询 | 含义 |
|---|---|
| `preferred_measurement` | 空间充足时的首选宽高与可选文本基线 |
| `minimum_size` | 压缩时仍应保留的尺寸 |
| `maximum_size` | 拉伸时允许达到的上限 |

首选尺寸按以下顺序解析：

```text
显式首选尺寸
-> 容器 LayoutManager 的测量结果
-> Figure 的内在测量
-> Figure 初始尺寸回退
```

`MeasureConstraints` 用 `Some(value)` 表示某一轴的有限上限，用 `None` 表示无约束。
零尺寸是合法结果，不是“没有结果”的特殊值。

### 应用何时需要关心测量

- 使用内置 Figure 和固定尺寸布局时，只需设置约束；
- 文本、图片或内容驱动尺寸时，应让 Figure 提供内在测量；
- 自定义容器布局时，通过 `LayoutSnapshot` 查询子节点，不读取上一次排列结果作为
  内容尺寸；
- 需要文本基线对齐时，读取 `FigureMeasurement`，不要只取宽高。

约束文本是典型例子：

```text
父级提供最大宽度
-> 文本按宽度完成换行测量
-> 返回高度和基线
-> 父布局安排最终边界
-> 绘制复用同一图形准备结果
```

如果到绘制阶段才决定换行，父布局已经拿不到正确高度，当前帧就无法一次收敛。

图形准备结果（`FigurePresentation`）把受检测量、视觉边界和不可变绘制数据保存为
一个值。它不专属于动画；普通文本、图像和边框准备也可以产生该结果。

自定义 preparation 需要在新宽度下重新测量时，通过
`PreparationScene::measure_figure(context, figure, constraints)` 查询。该入口分别解析
preferred 和 minimum：显式尺寸优先，其次是 LayoutManager，再选择注册的
`FigurePreparation::measure_scene`，最后使用普通 Figure 的内在测量。
即使还没有 prepared 快照，注册的 preparation 分支也需要支持显式约束；
默认实现返回 `UnsupportedConstraintMeasurement`。

`measure_scene` 返回内容域的 `PreparationMeasurement`，不修改 bounds，也不生成
绘制数据。Runtime 负责扣除物理边距、反投影约束、组合一次 Border 和正投影结果。
override 直接正投影，不再加 Border。测量失败通过 `PreparationMeasureError`
保留实际 Figure、outer/content 约束与原始错误，并沿
`TextError::PreparationMeasurement` 传到帧准备错误。

Anchor 和 Router 通过 `RouteSceneQuery::prepared_scene(owner)` 读取 Runtime 持有的
`PreparedSceneSnapshot`。快照包含不可构造的身份和不可变 presentation；旧快照可用于
诊断，但源输入变化后不能用于当前路由。producer 用
`PreparedScene::with_result_version` 声明稳定的领域结果版本。相同源前缀、结果版本、
约束与已观察依赖保留身份，普通 preparation 回调重跑不必使路由失效。

图投影必须同时发布布局和路径时，在候选中调用 `require_route_group`，显式提供
`ChildContent(owner)` 和完整 ConnectionId 集合。Runtime 在稳定帧晋升前检查组覆盖、
resolved 状态和快照依赖；失败通过 `TextError::PreparationRoute` 保留主体与原因。
声明本身是发布屏障，不是读取路径的计算依赖。

测量缓存以布局代数和约束为键。结构、约束、显式尺寸或相关内容变化时，Runtime
使缓存失效；应用不需要手工清理。

代码入口：

- [`MeasureConstraints` 与 `FigureMeasurement`](../../../novadraw/src/figure/mod.rs)
- [`LayoutSnapshot` 与 `LayoutOutput`](../../../novadraw/src/layout/mod.rs)
- [约束文本测量测试](../../../novadraw/tests/d4_constrained_measurement.rs)
- [外部 preparation 约束测量测试](../../../novadraw/tests/p2_preparation_measurement_contract.rs)
- [候选快照与必需路由组测试](../../../novadraw/tests/p2_prepared_scene_routing_contract.rs)

## 4.6 应用运行期如何触发更新

常见修改都通过 scoped mutable facade：

```rust
runtime.figure(node)?.set_bounds(new_bounds)?;
runtime.figure(label)?.set_label_text("Ready")?;
runtime.figure(node)?.set_visible(false)?;
runtime.container(panel)?.bring_child_to_front(node)?;
```

这些操作会按契约自动记录所需失效与重绘。只有自定义组件内部状态改变，而 Runtime
无法从操作类型推断影响范围时，才显式调用：

```rust
runtime.figure(node)?.revalidate()?;
runtime.figure(node)?.repaint(None)?;
```

应用业务代码不应直接访问 `UpdateManager`，也不应自己拼装 `DamageSet`。

## 4.7 派生状态如何收敛

一次源状态变化可能依次影响内在尺寸、布局、连接、自由范围、视口和最终呈现。
Runtime 使用固定优先级工作列表：

```text
内在尺寸与资源
-> 布局
-> 依赖失效传播
-> 连接路由
-> 路由后的几何
-> 最终显示状态
```

后置阶段产生新的高优先级工作时，调度器回到前面继续处理。全部队列排空后，
`stable_epoch` 才能晋升。

单个阶段有反馈预算。超过预算返回 `FramePreparationError::DidNotConverge`，不会
使只完成一部分的场景状态对外可见，也不会生成不完整的提交包。

实现入口：
[`Runtime::stabilize`](../../../novadraw/src/runtime/runtime.rs)。

## 4.8 重绘区域如何得到

一个脏矩形最初位于节点本地域。Runtime 沿父链逐级处理：

```text
与节点视觉边界求交
-> 应用节点位置
-> 应用父节点的子内容变换
-> 按裁剪策略收紧
-> 重复直到逻辑表面
-> 合并为 DamageSet
```

这条链与绘制和命中测试使用同一变换与裁剪规则。

`DamageSet` 有三种状态：

```text
None
Full
Partial { union, regions }
```

区域过多时可以安全合并为包围矩形；后端无法保留区域外像素时必须升级为全量重绘。
真正的不变量是：

> 帧提交后，重绘区域外的可见像素必须与提交前等价。

实现入口：
[`repair.rs`](../../../novadraw/src/runtime/update/repair.rs)。

## 4.9 帧准备状态

`Runtime::prepare_submission` 用明确状态表示当前能否产生一帧：

```rust
pub enum FramePreparation {
    Ready(RenderSubmission),
    Idle,
    Suspended,
    AwaitingCompletion,
    Error(FramePreparationError),
}
```

| 状态 | 宿主处理 |
|---|---|
| `Ready` | 渲染提交包已就绪；交给后端，并回传完整结果 |
| `Idle` | 当前没有像素或资源变化 |
| `Suspended` | 表面不可用，恢复后重试 |
| `AwaitingCompletion` | 等待前一帧完成，不能并发提交下一帧 |
| `Error` | 记录并按错误类型恢复或停止 |

宿主必须显式区分暂停、等待、无工作和错误，不能把它们统一折叠成“没有提交包”。

## 4.10 渲染提交、呈现与完成确认

稳定场景状态产生两条结果：

- `UpdateManager` 计算 `DamageSet`；
- `FigureRenderer` 递归遍历图形树，向 `NdCanvas` 录制 `RenderCommand`。

它们与资源同步、绘制表面、会话 ID 和帧 ID 组成 `RenderSubmission`。后端只消费
提交包，不能反向修改 FigureTree。

宿主的概念流程：

```rust
let submission = match runtime
    .frames()
    .prepare_submission(surface, backend.capabilities())
{
    FramePreparation::Ready(submission) => submission,
    FramePreparation::Idle => return,
    FramePreparation::Suspended => return,
    FramePreparation::AwaitingCompletion => return,
    FramePreparation::Error(error) => return handle_frame_error(error),
};

let session_id = submission.session_id;
let frame_id = submission.frame_id;
let outcome = backend.submit(&submission);
let acknowledgement = runtime
    .frames()
    .complete_submission(session_id, frame_id, outcome)
    .expect("prepared frame must match its completion");
handle_acknowledgement(acknowledgement);
```

完成反馈不能省略：

- `FramePreparation::Ready` 只表示提交包已就绪；
- Host 调用 `RenderBackend::submit` 后，提交才已经发起；
- `RenderResult` 为 `Ok(RenderDisposition::Presented)` 时，帧才已呈现；
- `Runtime::complete_submission` 返回 acknowledgement 后，资源与恢复状态才完成确认；
- `Retry`、`Skipped` 或 `Unsupported` 会触发相应恢复；
- 会话和帧身份防止迟到结果污染新绘制表面。

## 4.11 通知为什么最后发布

内部变化按因果顺序写入通知日志，但监听器只在稳定版本形成后收到记录。否则监听器
可能看到：

- 边界已改但连接尚未重新路由；
- 内容范围已改但视口尚未钳制；
- 文本度量已改但最终显示状态仍旧。

应用读取需要一致性的场景数据时使用稳定查询。不要把普通 Figure 回调或布局计算
当作“本帧已经完成”的通知。

## 4.12 常见问题

| 症状 | 原因 |
|---|---|
| 图形尺寸没有按内容变化 | Figure 没有内在测量，或父布局给了固定尺寸 |
| 换布局器后约束报错 | 旧约束类型与新布局器不兼容 |
| 修改后没有重排 | 绕过 scoped mutable facade，或自定义更新没有请求 revalidate |
| 移动后留下残影 | 旧视觉边界没有进入 damage |
| 绘制需要第二帧才正确 | 在绘制阶段才改变测量或布局事实 |
| 一直返回 `AwaitingCompletion` | 宿主没有回传前一帧的完成结果 |
| 一直返回 `Suspended` | surface 为零尺寸或当前不可呈现 |
| 返回 `DidNotConverge` | 扩展在布局或派生阶段形成循环失效 |

排查时沿本章开头的因果链从前向后检查，不要先修改渲染主循环。

## 4.13 有向图与复合图

图布局从 `novadraw::layout::graph` 导入。先用 `GraphRequestBuilder` 描述节点尺寸、
边、子图包含关系和算法选择，再创建对应 Figure 与 Connection，并构造完整的
`GraphProjectionBinding`。图中的包含关系与 FigureTree 父子关系分别维护：
官方 Compound 场景的子图边框和成员 Figure 都是同一容器的直接子节点。

将 `GraphProjection` 通过 `SetGraphProjection` 安装到 `GraphProjectionFigure`，
连接则使用同一 binding 创建的 Anchor 和 `GraphProjectionRouter`。
Runtime 会准备图结果、写入候选位置、计算整组路径，再提交稳定帧。
应用不应在场景构造中提前求解并逐项写 bounds，也不能保留旧快照作为当前路由权限。

显式输入尺寸保留原请求；需要根据 Figure 内容测量的实体通过
`GraphProjectionMeasurement` 声明。当算法改变候选宽度时，Core 会按新宽度
重新测量，直到请求需求与结果一致。测量振荡或预算耗尽会返回错误。
图容器也提供受约束测量，视口可以据此扩展内容范围；滚动改变视口位置，
不改变图拓扑和图布局坐标。

完整消费者见
[`graph_examples.rs`](../../../examples/native/draw2d-official-examples/src/examples/graph_examples.rs)。
运行后，Graph 菜单选择官方 fixture，Graph Direction 选择 SOUTH/EAST；
超出视口的内容通过宿主面板的 Horizontal/Vertical 控制查看。

```sh
cargo run -p draw2d-official-examples
cargo run -p draw2d-official-examples --example graph_verification
```

## 4.14 捕获共享场景

`Runtime::scenes()` 提供独立于 surface 帧的捕获服务。先登记 attached source，
再建立订阅；宿主检查 `needs_capture`，驱动初始捕获、输入变化后的捕获或显式重试。
捕获失败不会发布半个结果，也不会消费主窗口的 damage、资源同步记录或在途帧。

```rust,ignore
use novadraw::scene::{SOURCE_LOCAL_CAPTURE, SourceLocalCapture};

let source = runtime.scenes().register_source(source_figure)?;
let (subscription, config_revision, handoff) =
    runtime.scenes().subscribe(source, SOURCE_LOCAL_CAPTURE, SourceLocalCapture::default())?;
if runtime.scenes().needs_capture(subscription)? {
    let snapshot = runtime.scenes().capture(subscription)?;
}
```

默认捕获域是 source 的局部 border box。祖先颜色、字体与 alpha 会冻结为入口样式，
source 外部的位置、裁剪和视口变换不进入结果。`SourceLocalCapture::rectangle`
支持非零原点与零尺寸；零尺寸产生合法空结果。
自定义选择通过 `SceneCapturePolicyKey<P>` 与 `SceneCapturePolicy<P>` 注册，
与默认选择共用校验和错误通道。

`SceneHandoff` 每个订阅只保留最新完整结果、最近错误和终止事件。
宿主调用 `take()` 后拥有事件；source 释放后仍可读取 `Closed`。
这次交接不确认 GPU 呈现。取消订阅、删除 source 和释放 Runtime 都会结束订阅；
终止会清除未消费的普通结果，迟到捕获不能重新激活它。

跨 Runtime 共享视图还需要同一个 `SceneBindingGroup`。宿主在登记 source 前，
通过 `runtime.scenes().join_group(group.clone())` 登记每个参与者。
视图通过 `bind_view_dependency` 声明来源；默认 `SCENE_SOURCE_VIEW` 和外部
`SceneViewDependencyKey<P>` / `SceneViewDependencyPolicy<P>` 使用相同准入。
登记是结构声明，当前是否可见、是否在捕获矩形内、是否已有快照都不影响依赖边。

Core 在结构写入前拒绝直接和间接循环。重挂载、组件 Effect、回调延迟修改、
子树替换和批量插入都受检；失败保留此前已接受的输入。
`SceneSnapshot::dependencies()` 记录捕获时的结构来源，目标不能把旧记录当作当前
图的准入凭证。捕获根登记后保持固定；更换根应终止旧 source，再登记新身份。

目标通过 `create_view` 创建只读句柄，将它移入 `SceneViewFigure` 或外部 Figure。
句柄没有公开写入入口，绑定与安装由目标 Runtime 管理。

```rust,ignore
use novadraw::scene::{ASPECT_FIT_VIEW, AspectFitView, SceneViewFigure};

let handle = destination.scenes().create_view(ASPECT_FIT_VIEW, AspectFitView::default())?;
let figure = destination.figure(parent)?.add(Box::new(SceneViewFigure::new(handle)))?;
let binding = destination.scenes().bind_view(figure, source, subscription, config_revision)?;
destination.scenes().install_view(binding, snapshot)?;
```

默认展示保留 source 宽高比，以源逻辑尺寸作为自然 content size。无提示时返回自然
尺寸，单轴或双轴提示限定可容纳比例；Border insets 由 Core 扣除和加回。
调整目标 bounds 或 `configure_view` 只更新目标展示，不触发 source capture。
外部展示策略通过 `SceneViewPolicyKey<P>` / `SceneViewPolicy<P>` 注册，外部 Figure
通过 `SCENE_VIEW` descriptor 与 `SceneViewConsumer` 接入相同安装路径。

同 epoch 的新捕获配置需要先调用 `expect_view_config`，再安装新结果。
旧 binding、旧 source/config 和倒序 epoch 返回 `StaleSceneDelivery`；
完整身份相同但内容不同返回 `SnapshotConflict`。捕获或导入失败保留当前有效结果。
宿主收到 `Closed` 后调用 `close_view`；它移除绑定并请求完整清理帧，暂停时保留工作。

`installed_view` 返回可保留的 `InstalledScene`，其中 drawing 已改写为目标资源 ID。
旧、新来源 revision 使用不同 ID，可同时存活。保留该结果也保留 CPU 资源 lease；
reset 后目标用自身新 session 恢复，不要求 source 先呈现帧。
官方 Thumbnail 的 [Native 消费者](../../../examples/native/draw2d-official-examples/src/examples/thumbnail.rs)
展示了宿主如何拥有 source 和 destination：主视图转交值输入，源提交后捕获，
再将同一完整结果安装到主视图和 overview。overview resize 不触发源捕获。
生命周期和图像证据见 [K07-4 记录](../../../doc/verification/reviews/k07-4-thumbnail-evidence-2026-10-23.md)；
宿主背景继续按 [K07 执行队列](../../../doc/roadmap/p2-delta-backlog.md)推进。

## 宿主背景也走帧准备入口

透明 Canvas 需要宿主提供背景时，先建立 `host::BackgroundFrameInput`，
明确 surface、位置与 DPI 版本，以及宿主到 Canvas 的变换。
通过 `runtime.frames().begin_background(source, input, base)` 取得 request，
宿主再运行 `HostBackgroundRegistry` 中的 typed provider，并将结果交给
`complete_background`。Core 不负责 I/O 或轮询提供者。

初次 Pending 使用显式基础清屏值。加载失败保留已接受背景；
坐标失配时帧准备返回错误，直到宿主提供匹配的新候选。
`remove_background(base)` 清空背景并请求完整修复，迟到 request 不可恢复旧背景。
背景独立更新也产生 pending work，可通过 `set_background_waker` 通知宿主。

背景、Canvas 与现有 overlay 按此顺序合成。背景参与时使用完整 Canvas recording；
内容滚动不改变 `host_to_canvas`，移动 Canvas 则需要新的位置版本。
后端必须声明 `HostBackground` capability；不支持的后端在准备时明确拒绝。
公开调用反例见 [`p2_host_background`](../../../novadraw/tests/p2_host_background.rs)。

## 4.15 验证入口

- [`m5_layout_contract.rs`](../../../novadraw/tests/m5_layout_contract.rs)
- [`d4_constrained_measurement.rs`](../../../novadraw/tests/d4_constrained_measurement.rs)
- [`d4_component_update.rs`](../../../novadraw/tests/d4_component_update.rs)
- [`d4_notification_epoch.rs`](../../../novadraw/tests/d4_notification_epoch.rs)
- [`runtime_resize_contract.rs`](../../../novadraw/tests/runtime_resize_contract.rs)
- [`graph_projection_contract.rs`](../../../novadraw/tests/graph_projection_contract.rs)
- [`p2_scene_capture.rs`](../../../novadraw/tests/p2_scene_capture.rs)
- [`p2_scene_view.rs`](../../../novadraw/tests/p2_scene_view.rs)
- `cargo xtask verify runtime.apps`
