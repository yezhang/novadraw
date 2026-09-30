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
| 直接文本编辑 | direct text edit | 在图形原位置用临时草稿编辑业务文本，并在接受时生成模型命令的会话 |
| 插入光标 | caret | 由文本布局计算、表示当前插入位置的可视几何 |
| 预编辑 | preedit/composition | 输入法候选确认前，可更新或取消的临时组合文本 |
| 文本输入租约 | text-input lease | 用 session 身份约束平台文本焦点、事件和候选窗效果的所有权 |
| 文本输入快照 | TextInputSnapshot | 平台文本缓冲区提交给 Editor 的完整草稿、选择与组合范围 |
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
| `novadraw::render` | 后端无关的提交、资源和文本协议 |
| `novadraw::advanced` | 诊断与深度集成所需低层状态 |

Editor、Inspector、Vello backend 和 Winit/Web platform adapter 使用独立 crate。
应用不应为了取得一个方便类型而直接依赖 `advanced`。

## C. Rust 包地图

| Rust 包（crate） | 主要职责 | 关键入口 |
|---|---|---|
| `novadraw` | 平台无关 Core：几何、渲染协议、Figure、布局、树与 Runtime | [`src/lib.rs`](../../novadraw/src/lib.rs) |
| `novadraw-editor` | 模型投影、选择、工具、策略、命令历史 | [`src/lib.rs`](../../novadraw-editor/src/lib.rs) |
| `novadraw-inspector` | 只读诊断与观测 | [`src/lib.rs`](../../novadraw-inspector/src/lib.rs) |
| `novadraw-backend-vello` | Vello 渲染后端 | [`src/lib.rs`](../../novadraw-backend-vello/src/lib.rs) |
| `novadraw-platform-winit` | Winit 输入与宿主适配 | [`src/lib.rs`](../../novadraw-platform-winit/src/lib.rs) |
| `novadraw-platform-web` | Web 输入与宿主适配 | [`src/lib.rs`](../../novadraw-platform-web/src/lib.rs) |

## D. 图形核心代码地图

### 几何

- 矩形与尺寸（Rectangle/Dimension）：
  [`novadraw/src/geometry/rect.rs`](../../novadraw/src/geometry/rect.rs)
- 仿射变换（Affine2D/Transform）：
  [`novadraw/src/geometry/transform.rs`](../../novadraw/src/geometry/transform.rs)
- 点列表（PointList）：
  [`novadraw/src/geometry/point_list.rs`](../../novadraw/src/geometry/point_list.rs)

### 图形对象与树

- 图形能力（Figure）：
  [`novadraw/src/figure/mod.rs`](../../novadraw/src/figure/mod.rs)
- 图形节点与图形树（FigureNode/FigureTree）：
  [`novadraw/src/graph/mod.rs`](../../novadraw/src/graph/mod.rs)
- 命中与树搜索：
  [`novadraw/src/graph/search.rs`](../../novadraw/src/graph/search.rs)
- 递归绘制：
  [`novadraw/src/graph/render_recursive.rs`](../../novadraw/src/graph/render_recursive.rs)

### 场景运行时与更新

- 场景运行时组合根（Runtime）：
  [`novadraw/src/runtime/runtime.rs`](../../novadraw/src/runtime/runtime.rs)
- 事件上下文与效果队列：
  [`novadraw/src/runtime/context.rs`](../../novadraw/src/runtime/context.rs)
- 事件分发器（EventDispatcher）：
  [`novadraw/src/runtime/event/mod.rs`](../../novadraw/src/runtime/event/mod.rs)
- 修改事务（Mutation）：
  [`novadraw/src/runtime/mutation/mod.rs`](../../novadraw/src/runtime/mutation/mod.rs)
- 更新管理器（UpdateManager）：
  [`novadraw/src/runtime/update/deferred.rs`](../../novadraw/src/runtime/update/deferred.rs)
- 重绘区域计算（Damage repair）：
  [`novadraw/src/runtime/update/repair.rs`](../../novadraw/src/runtime/update/repair.rs)

### 布局与容器

- 布局协议：
  [`novadraw/src/layout/mod.rs`](../../novadraw/src/layout/mod.rs)
- 精确坐标布局：
  [`novadraw/src/layout/xy_layout.rs`](../../novadraw/src/layout/xy_layout.rs)
- 单内容填充与多层堆叠布局：
  [`fill_layout.rs`](../../novadraw/src/layout/fill_layout.rs)、
  [`stack_layout.rs`](../../novadraw/src/layout/stack_layout.rs)
- 五区、流式、网格与工具栏布局：
  [`border_layout.rs`](../../novadraw/src/layout/border_layout.rs)、
  [`flow_layout.rs`](../../novadraw/src/layout/flow_layout.rs)、
  [`grid_layout.rs`](../../novadraw/src/layout/grid_layout.rs)、
  [`toolbar_layout.rs`](../../novadraw/src/layout/toolbar_layout.rs)
- 自由范围布局：
  [`novadraw/src/layout/freeform_layout.rs`](../../novadraw/src/layout/freeform_layout.rs)
- 布局 demo 场景：
  [`novadraw-example-scenes/src/layout.rs`](../../examples/scenes/src/layout.rs)
- 更新与裁剪中的布局场景：
  [`update.rs`](../../examples/scenes/src/update.rs)、
  [`clip.rs`](../../examples/scenes/src/clip.rs)
- 图层（Layer）：
  [`novadraw/src/container/layer.rs`](../../novadraw/src/container/layer.rs)
- 视口（Viewport）：
  [`novadraw/src/container/viewport.rs`](../../novadraw/src/container/viewport.rs)
- 滚动面板专用布局：
  [`novadraw/src/container/scroll_pane.rs`](../../novadraw/src/container/scroll_pane.rs)
- 范围模型（RangeModel）：
  [`novadraw/src/container/range_model.rs`](../../novadraw/src/container/range_model.rs)
- 可缩放容器（Scalable）：
  [`novadraw/src/container/scalable.rs`](../../novadraw/src/container/scalable.rs)
- 滚动面板（ScrollPane）：
  [`novadraw/src/container/scroll_pane.rs`](../../novadraw/src/container/scroll_pane.rs)

### 连接

- 公共导出与 ID：
  [`novadraw/src/connection/mod.rs`](../../novadraw/src/connection/mod.rs)
- 锚点（Anchor）：
  [`novadraw/src/connection/anchor.rs`](../../novadraw/src/connection/anchor.rs)
- 场景查询（SceneQuery）：
  [`novadraw/src/connection/query.rs`](../../novadraw/src/connection/query.rs)
- 路由器（Router）：
  [`novadraw/src/connection/router.rs`](../../novadraw/src/connection/router.rs)
- 连接运行时（ConnectionRuntime）：
  [`novadraw/src/connection/runtime.rs`](../../novadraw/src/connection/runtime.rs)
- 连接图形与几何：
  [`novadraw/src/connection/figure.rs`](../../novadraw/src/connection/figure.rs)
- 定位器（Locator）：
  [`novadraw/src/connection/locator.rs`](../../novadraw/src/connection/locator.rs)

### 渲染

- 命令画布（NdCanvas）：
  [`novadraw/src/render/context.rs`](../../novadraw/src/render/context.rs)
- 渲染命令（RenderCommand）：
  [`novadraw/src/render/command.rs`](../../novadraw/src/render/command.rs)
- 渲染提交包（RenderSubmission）：
  [`novadraw/src/render/submission.rs`](../../novadraw/src/render/submission.rs)
- 渲染后端（RenderBackend）：
  [`novadraw/src/render/traits.rs`](../../novadraw/src/render/traits.rs)
- Vello 后端：
  [`novadraw-backend-vello/src/lib.rs`](../../novadraw-backend-vello/src/lib.rs)

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
- 直接文本编辑会话：
  [`novadraw-editor/src/direct_edit.rs`](../../novadraw-editor/src/direct_edit.rs)
- 平台无关文本输入协议：
  [`novadraw-editor/src/text_input.rs`](../../novadraw-editor/src/text_input.rs)
- 边缘自动滚动：
  [`novadraw-editor/src/autoexpose.rs`](../../novadraw-editor/src/autoexpose.rs)
- 编辑域（EditorDomain）：
  [`novadraw-editor/src/domain.rs`](../../novadraw-editor/src/domain.rs)

### 文本输入宿主

- Winit 输入法桥：
  [`novadraw-platform-winit/src/text_input.rs`](../../novadraw-platform-winit/src/text_input.rs)
- Web 输入法桥：
  [`novadraw-platform-web/src/text_input.rs`](../../novadraw-platform-web/src/text_input.rs)
- Web 隐藏输入宿主：
  [`novadraw-platform-web/src/dom_text_input.rs`](../../novadraw-platform-web/src/dom_text_input.rs)
- Web EditContext 宿主：
  [`novadraw-platform-web/src/edit_context.rs`](../../novadraw-platform-web/src/edit_context.rs)

## F. 示例与验证入口

```bash
# 运行基础图形、布局和滚动示例
cargo run -p shape-app
cargo run -p layout-app
cargo run -p scroll-pane-demo

# 运行完整节点编辑器
cargo run -p node-editor-demo

# 查看并执行验证套件
cargo xtask list
cargo xtask run core.runtime
```
