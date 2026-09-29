# 附录：术语、公开 API 与代码地图

## A. 术语

本表用于快速回查。正文仍须在概念首次出现处给出定义，不能要求读者先阅读附录。

| 中文名称 | 英文原名或代码名 | 定义 |
|---|---|---|
| 唯一事实来源 | Single Source of Truth，SSOT | 某项事实唯一允许被写入和判定的权威位置，其他表示都由它派生 |
| 不变量 | invariant | 系统在任何合法操作前后都必须成立的约束 |
| 因果链 | causal chain | 一次输入或状态变化按发生顺序触发的处理过程 |
| 事务 | transaction | 一组相关变化作为整体接受校验和提交的操作边界 |
| 原子提交 | atomic commit | 要么完整提交全部变化，要么不留下部分结果 |
| 图形对象/图形行为 | Figure | 具体图形的差异行为，不拥有树拓扑或平台资源 |
| 图形节点 | FigureNode | `Figure`、`NodeState`、`LayoutState` 与树关系的运行时组合 |
| 图形树 | FigureTree | 图形拓扑、代际身份和叠放顺序的唯一所有者 |
| 场景运行时 | Runtime | 单个场景的事务边界和所有权组合根 |
| 图形核心 | Core | 负责图形树、布局、坐标、绘制、命中和基础输入语义的引擎层 |
| 编辑框架 | Editor | 把业务模型投影为图形，并把输入转换成可撤销模型修改的上层框架 |
| 代际 ID | generational ID | 由槽位和代数组成的身份；槽位复用后，旧 ID 不会误指向新对象 |
| 坐标域 | coordinate domain/space | 一组坐标数值所依附的参照空间 |
| 布局 | Layout | 根据容器规则计算图形位置和尺寸的过程 |
| 边界矩形 | bounds | 位于父内容域中的布局边框盒，记录节点位置与尺寸 |
| 本地边框盒 | local border box | 节点本地的 `(0, 0, width, height)` 矩形 |
| 内边距 | insets | 边框内侧为内容预留的上、左、下、右距离 |
| 客户区 | client area | 本地边框盒去除内边距后，供布局和子节点使用的区域 |
| 视觉边界 | visual bounds | 包含描边与视觉效果外扩的节点本地保守边界 |
| 投影边界 | projected bounds | 映射到逻辑表面后的保守轴对齐包围盒 |
| 子内容变换 | child transform | 从子内容域映射到节点本地域的变换 |
| 失效 | invalid | 几何、布局或派生缓存需要重新计算 |
| 脏区 | dirty region | 某个节点本地可见区域需要纳入重绘 |
| 源状态 | source state | 由应用、输入或资源直接写入，而不是从其他状态计算得到的事实 |
| 校验收敛 | validation | 反复计算失效的布局和派生状态，直到没有待处理工作的阶段 |
| 重绘损伤区域 | damage | 新帧可能与旧帧不同、需要重新生成像素的逻辑表面区域 |
| 派生状态 | derived state | 由源状态计算得到、可失效并重新生成的状态 |
| 稳定版本 | stable epoch | 所有必需派生工作闭合后，可以对外发布的场景版本 |
| 渲染提交包 | RenderSubmission | 携带命令、重绘区域、资源、绘制表面与帧身份的后端输入 |
| 帧 | frame | 引擎为一次屏幕呈现准备并提交的完整结果 |
| 命中测试 | hit-test | 根据坐标找出应该接收输入的最前方图形对象 |
| 指针捕获 | capture | 后续指针事件固定路由到某个图形对象的交互状态 |
| 悬停 | hover | 指针当前停留在某个图形对象上的交互状态 |
| 焦点 | focus | 当前接收键盘输入的对象 |
| 图层 | Layer | 参与图形树协议、通常不绘制自身的结构图形 |
| 自由范围 | Freeform | 允许子节点溢出，并从子节点派生内容范围的容器语义 |
| 视口 | Viewport | 用范围模型原点定义可见内容窗口的容器 |
| 范围模型 | RangeModel | 保存最小值、最大值、可见长度与当前位置的单轴滚动状态 |
| 边缘自动滚动 | auto-expose | 拖拽接近视口边缘时，自动滚动以露出更多内容的行为 |
| 连接 | Connection | 把两个图形端点关联起来，并能随端点变化自动更新路径的图形 |
| 锚点 | Anchor | 根据所属图形当前几何和另一端方向纯计算连接端点的策略 |
| 路由器 | Router | 根据锚点、约束和分组快照纯计算完整连接路径的策略 |
| 路由坐标域 | routing domain | 路由器读写连接路径点时统一使用的参照空间 |
| 场景查询 | SceneQuery | 向锚点和路由器提供只读几何，并记录所读取依赖的接口 |
| 连接运行时 | ConnectionRuntime | 连接绑定、依赖、代数与解析状态的所有者 |
| 未解析状态 | unresolved | 路径无法可靠计算时清除旧几何并等待依赖恢复的稳定状态 |
| 模型适配器 | ModelAdapter | 编辑框架读取应用模型及其有序通知的边界 |
| 编辑部件 | EditPart | 业务模型与图形对象之间的控制器 |
| 查看器 | Viewer | 模型投影、编辑部件树、注册表、选择状态与运行时的组合根 |
| 投影 | projection | 根据业务模型创建或刷新对应编辑部件与图形对象的过程 |
| 选择状态 | Selection | 当前被用户选中的有序编辑部件集合 |
| 编辑工具 | Tool | 把连续输入解释成编辑请求的状态机 |
| 编辑请求 | Request | 平台无关、模型无关且不直接修改模型的编辑意图 |
| 编辑策略 | EditPolicy | 为编辑请求提供目标、反馈和命令的角色化策略 |
| 可撤销命令 | Command | 只操作应用模型，并支持撤销与重做的操作 |
| 命令历史栈 | CommandStack | 执行、撤销、重做命令并记录保存位置的组件 |
| 反馈图形 | Feedback | 命令提交前用于预览结果的临时图形 |
| 故障锁定状态 | faulted | 因无法证明状态一致而拒绝继续提交的安全状态 |
| 全量快照/增量 | snapshot/delta | 分别表示一份完整状态和相对既有基线的变化 |

## B. 公开 API 地图

普通应用从 `novadraw` facade 开始：

| 入口 | 用途 |
|---|---|
| crate root | `Runtime`、`FigureTree`、常用 Figure、布局和基础值 |
| `novadraw::prelude` | 常规 Figure/Runtime 应用的一组便利导入 |
| `novadraw::geometry` | 点、向量、矩形、尺寸、内边距和仿射变换 |
| `novadraw::graphics` | `NdCanvas`、路径、线型和颜色 |
| `novadraw::figure` | Figure 扩展能力、内置 Figure、边框和样式 |
| `novadraw::layout` | 布局器、约束、测量快照和输出 |
| `novadraw::container` | 图层、自由范围、视口、滚动和缩放 |
| `novadraw::connection` | 连接、锚点、路由器和定位器 |
| `novadraw::event` | 输入、监听器、焦点、提示和无障碍 |
| `novadraw::runtime` | scoped editor、资源、稳定查询和帧准备 |
| `novadraw::editor` | 模型投影、选择、工具、策略和命令历史 |
| `novadraw::render` | 后端无关的提交、资源和文本协议 |
| `novadraw::backend` | feature 控制的具体后端 |
| `novadraw::advanced` | 诊断与深度集成所需低层状态 |

默认 feature 为空。桌面和网页 Vello 后端分别使用 `native-vello` 与 `web-vello`。
应用不应为了取得一个方便类型而直接依赖 `advanced`。

## C. Rust 包地图

| Rust 包（crate） | 主要职责 | 关键入口 |
|---|---|---|
| `novadraw` | 普通应用使用的聚合 facade | [`src/lib.rs`](../../novadraw/src/lib.rs) |
| `novadraw-core` | 基础颜色等值类型 | [`src/lib.rs`](../../novadraw-core/src/lib.rs) |
| `novadraw-math` | 通用数学类型 | [`src/lib.rs`](../../novadraw-math/src/lib.rs) |
| `novadraw-geometry` | 二维点、矩形、变换与点列表 | [`src/lib.rs`](../../novadraw-geometry/src/lib.rs) |
| `novadraw-render` | 命令录制、渲染提交与后端接口 | [`src/lib.rs`](../../novadraw-render/src/lib.rs) |
| `novadraw-scene` | 图形对象、树、布局、更新、输入、容器与连接 | [`src/lib.rs`](../../novadraw-scene/src/lib.rs) |
| `novadraw-editor` | 模型投影、选择、工具、策略、命令历史 | [`src/lib.rs`](../../novadraw-editor/src/lib.rs) |

## D. 图形核心代码地图

### 几何

- 矩形与尺寸（Rectangle/Dimension）：
  [`novadraw-geometry/src/rect.rs`](../../novadraw-geometry/src/rect.rs)
- 仿射变换（Affine2D/Transform）：
  [`novadraw-geometry/src/transform.rs`](../../novadraw-geometry/src/transform.rs)
- 点列表（PointList）：
  [`novadraw-geometry/src/point_list.rs`](../../novadraw-geometry/src/point_list.rs)

### 图形对象与树

- 图形能力（Figure）：
  [`novadraw-scene/src/figure/mod.rs`](../../novadraw-scene/src/figure/mod.rs)
- 图形节点与图形树（FigureNode/FigureTree）：
  [`novadraw-scene/src/graph/mod.rs`](../../novadraw-scene/src/graph/mod.rs)
- 命中与树搜索：
  [`novadraw-scene/src/graph/search.rs`](../../novadraw-scene/src/graph/search.rs)
- 递归绘制：
  [`novadraw-scene/src/graph/render_recursive.rs`](../../novadraw-scene/src/graph/render_recursive.rs)

### 场景运行时与更新

- 场景运行时组合根（Runtime）：
  [`novadraw-scene/src/runtime/runtime.rs`](../../novadraw-scene/src/runtime/runtime.rs)
- 事件上下文与效果队列：
  [`novadraw-scene/src/runtime/context.rs`](../../novadraw-scene/src/runtime/context.rs)
- 事件分发器（EventDispatcher）：
  [`novadraw-scene/src/runtime/event/mod.rs`](../../novadraw-scene/src/runtime/event/mod.rs)
- 修改事务（Mutation）：
  [`novadraw-scene/src/runtime/mutation/mod.rs`](../../novadraw-scene/src/runtime/mutation/mod.rs)
- 更新管理器（UpdateManager）：
  [`novadraw-scene/src/runtime/update/deferred.rs`](../../novadraw-scene/src/runtime/update/deferred.rs)
- 重绘区域计算（Damage repair）：
  [`novadraw-scene/src/runtime/update/repair.rs`](../../novadraw-scene/src/runtime/update/repair.rs)

### 布局与容器

- 布局协议：
  [`novadraw-scene/src/layout/mod.rs`](../../novadraw-scene/src/layout/mod.rs)
- 精确坐标布局：
  [`novadraw-scene/src/layout/xy_layout.rs`](../../novadraw-scene/src/layout/xy_layout.rs)
- 单内容填充与多层堆叠布局：
  [`fill_layout.rs`](../../novadraw-scene/src/layout/fill_layout.rs)、
  [`stack_layout.rs`](../../novadraw-scene/src/layout/stack_layout.rs)
- 五区、流式、网格与工具栏布局：
  [`border_layout.rs`](../../novadraw-scene/src/layout/border_layout.rs)、
  [`flow_layout.rs`](../../novadraw-scene/src/layout/flow_layout.rs)、
  [`grid_layout.rs`](../../novadraw-scene/src/layout/grid_layout.rs)、
  [`toolbar_layout.rs`](../../novadraw-scene/src/layout/toolbar_layout.rs)
- 自由范围布局：
  [`novadraw-scene/src/layout/freeform_layout.rs`](../../novadraw-scene/src/layout/freeform_layout.rs)
- 布局 demo 场景：
  [`novadraw-demo-scenes/src/layout.rs`](../../apps/scenes/src/layout.rs)
- 更新与裁剪中的布局场景：
  [`update.rs`](../../apps/scenes/src/update.rs)、
  [`clip.rs`](../../apps/scenes/src/clip.rs)
- 图层（Layer）：
  [`novadraw-scene/src/container/layer.rs`](../../novadraw-scene/src/container/layer.rs)
- 视口（Viewport）：
  [`novadraw-scene/src/container/viewport.rs`](../../novadraw-scene/src/container/viewport.rs)
- 滚动面板专用布局：
  [`novadraw-scene/src/container/scroll_pane.rs`](../../novadraw-scene/src/container/scroll_pane.rs)
- 范围模型（RangeModel）：
  [`novadraw-scene/src/container/range_model.rs`](../../novadraw-scene/src/container/range_model.rs)
- 可缩放容器（Scalable）：
  [`novadraw-scene/src/container/scalable.rs`](../../novadraw-scene/src/container/scalable.rs)
- 滚动面板（ScrollPane）：
  [`novadraw-scene/src/container/scroll_pane.rs`](../../novadraw-scene/src/container/scroll_pane.rs)

### 连接

- 公共导出与 ID：
  [`novadraw-scene/src/connection/mod.rs`](../../novadraw-scene/src/connection/mod.rs)
- 锚点（Anchor）：
  [`novadraw-scene/src/connection/anchor.rs`](../../novadraw-scene/src/connection/anchor.rs)
- 场景查询（SceneQuery）：
  [`novadraw-scene/src/connection/query.rs`](../../novadraw-scene/src/connection/query.rs)
- 路由器（Router）：
  [`novadraw-scene/src/connection/router.rs`](../../novadraw-scene/src/connection/router.rs)
- 连接运行时（ConnectionRuntime）：
  [`novadraw-scene/src/connection/runtime.rs`](../../novadraw-scene/src/connection/runtime.rs)
- 连接图形与几何：
  [`novadraw-scene/src/connection/figure.rs`](../../novadraw-scene/src/connection/figure.rs)
- 定位器（Locator）：
  [`novadraw-scene/src/connection/locator.rs`](../../novadraw-scene/src/connection/locator.rs)

### 渲染

- 命令画布（NdCanvas）：
  [`novadraw-render/src/context.rs`](../../novadraw-render/src/context.rs)
- 渲染命令（RenderCommand）：
  [`novadraw-render/src/command.rs`](../../novadraw-render/src/command.rs)
- 渲染提交包（RenderSubmission）：
  [`novadraw-render/src/submission.rs`](../../novadraw-render/src/submission.rs)
- 渲染后端（RenderBackend）：
  [`novadraw-render/src/traits.rs`](../../novadraw-render/src/traits.rs)
- Vello 后端：
  [`novadraw-render/src/backend/vello/mod.rs`](../../novadraw-render/src/backend/vello/mod.rs)

## E. 编辑框架代码地图

- 模型适配器（ModelAdapter）：
  [`novadraw-editor/src/model/mod.rs`](../../novadraw-editor/src/model/mod.rs)
- 编辑部件树与编辑部件（PartTree/EditPart）：
  [`novadraw-editor/src/part/mod.rs`](../../novadraw-editor/src/part/mod.rs)
- 图形查看器（GraphicalViewer）：
  [`novadraw-editor/src/viewer/mod.rs`](../../novadraw-editor/src/viewer/mod.rs)
- 选择状态：
  [`novadraw-editor/src/selection/mod.rs`](../../novadraw-editor/src/selection/mod.rs)
- 编辑请求：
  [`novadraw-editor/src/request/mod.rs`](../../novadraw-editor/src/request/mod.rs)
- 编辑策略：
  [`novadraw-editor/src/policy/mod.rs`](../../novadraw-editor/src/policy/mod.rs)
- 编辑工具：
  [`novadraw-editor/src/tool/mod.rs`](../../novadraw-editor/src/tool/mod.rs)
- 命令历史栈（CommandStack）：
  [`novadraw-editor/src/command/mod.rs`](../../novadraw-editor/src/command/mod.rs)
- 反馈与操作手柄：
  [`novadraw-editor/src/feedback/mod.rs`](../../novadraw-editor/src/feedback/mod.rs)
- 边缘自动滚动：
  [`novadraw-editor/src/autoexpose.rs`](../../novadraw-editor/src/autoexpose.rs)
- 编辑域（EditorDomain）：
  [`novadraw-editor/src/domain.rs`](../../novadraw-editor/src/domain.rs)

## F. 设计唯一事实来源地图

- [文档总入口](../../doc/00-index.md)
- [Core 公开 API 边界（ADR-017）](../../doc/adr/adr-017-core-public-api-boundary.md)
- [Runtime 驱动与测量 API（ADR-018）](../../doc/adr/adr-018-runtime-driving-and-measurement-api.md)
- [可组装 API 与 scoped editor（ADR-019）](../../doc/adr/adr-019-composable-api-and-scoped-editors.md)
- [引擎基础值与渲染契约（ADR-020）](../../doc/adr/adr-020-engine-value-and-render-contract.md)
- [公开 facade 与 feature 边界（ADR-021）](../../doc/adr/adr-021-public-facade-and-feature-boundary.md)
- [总体架构](../../doc/design/architecture/overview.md)
- [静态结构](../../doc/design/architecture/static-architecture.md)
- [动态协议](../../doc/design/architecture/dynamic-architecture.md)
- [坐标协议](../../doc/design/coordinates/coordinate-system.md)
- [更新管理器](../../doc/design/rendering/update-manager.md)
- [派生状态收敛](../../doc/design/architecture/derived-state-convergence.md)
- [连接路由](../../doc/design/architecture/connection-routing.md)
- [编辑框架架构](../../doc/design/editor/architecture.md)
- [编辑框架的视口与边缘自动滚动](../../doc/design/editor/g5-viewport-autoexpose.md)
- [Draw2D 语义账本](../../doc/parity/draw2d/api-coverage.md)
- [GEF 语义账本](../../doc/parity/gef/api-coverage.md)
- [图形核心路线图](../../doc/roadmap/00-index.md)
- [编辑框架路线图](../../doc/roadmap/editor/00-index.md)
- [验证清单](../../verification/suites.toml)

## G. 推荐检索

```bash
# 查类型或函数
rg -n "struct Runtime|fn prepare_submission_state" novadraw-scene

# 查某项语义的规范，不搜索 archive
rg -n "stable_epoch|Damage Repair" doc/design doc/adr doc/parity

# 查看统一验证入口
cargo xtask list

# 检查文档引用与账本
cargo xtask docs
```
