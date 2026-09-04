# 核心收口与 Draw2D 功能迁移计划

类型：`roadmap`

状态：`in_progress`

本文定义 R8/R9 架构迁移完成后，Novadraw 从“核心运行时主路径已验证”推进到
“Draw2D 核心产品能力完整”的执行顺序。

本文不创建新的 milestone 编号。`M1-M10` 的编号与状态仍以
[`00-index.md`](00-index.md) 为唯一入口；本文中的 `D0-D2` 是跨 milestone 的
architecture delta，用于消除后续 M9/M10 实现会放大的公共协议缺口。

## 1. 当前判断

Novadraw 已具备完整的核心执行骨架：

- FigureTree、FigureNode 与代际 FigureId；
- parent-local bounds 与统一 Affine2D 坐标链；
- LayoutSnapshot/LayoutOutput 与 validation；
- Validation -> Damage Repair 两阶段更新；
- Runtime 事务、effect queue 与 mutation FIFO；
- mouse、capture、hover、focus、scroll/zoom gesture；
- Viewport、ScrollPane、RangeModel 与 ZoomManager；
- PlatformHost、RenderBackend 与 headless 验证边界。

但公共协议仍存在以下未收口项，因此当前结论是：

> 核心运行时完整，Draw2D 核心框架公共面与产品能力尚未完整。

主要阻塞项：

1. D1.1 启动时 selection 兼容状态仍位于 FigureTree，并由渲染主流程绘制；
2. D1.2 启动时 Bounded/Updatable 兼容 capability 尚未完成去留决策；
3. Figure style 尚未覆盖 font、cursor、tooltip 等继承属性；
4. 部分公开 Graphics API 是静默 no-op；
5. TreeSearch、Layer/Freeform 与资源生命周期仍缺少稳定协议；
6. M9 Connection 体系和 M10 文本/图像/控件尚未实现。

## 2. 执行原则

- 先修正横跨多个产品能力的公共协议，再实现具体 Figure。
- Runtime、FigureTree、InteractionState 和 UpdateManager 的职责边界不得倒退。
- selection、Tool、Command、Request 和业务对象映射属于 editor/GEF 层。
- 不为单个 demo 在 apps 层增加引擎语义旁路。
- 公开 API 不允许静默 no-op；未支持能力必须删除、私有化或返回明确错误。
- 每个 delta 先定义契约和测试，再实现，再执行自动与人工验证。
- macOS、Web 和 Headless 是当前门禁；Windows/Linux 延后到发布资格阶段。

## 3. D0：路线图与证据校准

状态：`complete`

目标：

- 完成 M8 macOS/Web 人工签收并决定是否提升为 `complete`；
- 同步 R8、R9 已完成事实；
- 重新审计 API 语义账本中的 `partial`、`missing` 和 `deferred`；
- 区分“类型存在”“行为已验证”和“产品面完成”；
- 为公开 no-op、兼容 API 和设计偏差建立明确处置项。

完成门禁：

- `00-index.md`、API 语义账本、产品清单和 demo 矩阵状态一致；
- 每个 P0/P1 `partial` 或 `missing` 都有 milestone 或明确排除理由；
- 不存在无 owner 的核心债务。

## 4. D1：核心公共协议收口

状态：`in_progress`

### D1.1 Selection 外移

状态：`complete`

目标：

- FigureTree 不再保存 selection；
- Runtime 不再暴露 editor-specific `set_selected`；
- selection model 由 editor/viewer 层拥有；
- selection outline、handles 和 drag feedback 由 editor-owned feedback composition 表达；
- 删除节点后，通过 mutation 结果或树查询清理失效 FigureId。

验证：

- FigureTree 渲染结果不依赖 editor selection；
- 单选、取消选择、节点删除和场景切换仍正确；
- selection feedback 使用 FigureTree 的规范坐标链，并在场景命令之后合成；
- 不修改 `render_recursive.rs` 主循环语义。

自动验证记录：

- FigureTree、Runtime 和 EventContext 已删除 selection-specific 状态与 API；
- editor `SelectionModel` 维护选择并在提交阶段合成 feedback；
- `cargo test -p novadraw-scene --lib --tests`：240 项通过；
- `cargo test -p editor`：10 项通过；
- `cargo check --workspace`：通过；
- `cargo test --workspace`：通过；
- `cargo clippy -p novadraw-scene --lib -- -D warnings`：通过；
- `cargo clippy -p editor --all-targets -- -D warnings`：通过。

人工验证：

- editor 场景 1、2、5 的初始黄色选择轮廓；
- 场景 9 点击选择、拖出释放和再次点击；
- 场景 5 按 `T` 后选择轮廓与目标同步移动；
- 场景切换后 selection 不泄漏。

人工验收记录：

- 日期：2026-09-03；
- 平台：macOS；
- 结果：PASS；
- 失败项：无。

### D1.2 兼容 capability 收口

状态：`complete`

目标：

- 审计并决定 Bounded、Updatable 的调用方；
- 运行时几何只以 NodeState 为真源；
- FigureLifecycle 只保留具体 Figure 派生状态的 validate/invalidate；
- 删除重复、不可达或仅用于旧调用方式的 public surface。

审计结论：

- `Bounded` 仍被内置 Figure、Viewport/Scroll/Scale 容器和独立图元 API 用于构造期
  几何、insets 与尺寸投影；D1.2 不强行删除，保留到 M10 后消融复查。
- `Updatable` 除 Triangle 派生几何缓存外均为空实现；Runtime validation 已通过
  `FigureLifecycle` 执行，因此删除 `Updatable`，Triangle 使用 inherent 方法并桥接
  `FigureLifecycle`。

执行结果：

- 删除 `Updatable` trait、公共导出及全部空实现；
- Triangle 保留 inherent `validate`/`invalidate`，并通过 `FigureLifecycle` 接入
  Runtime validation；
- `Bounded` 保留为构造期和独立图元 capability，不参与 FigureTree 运行时状态真源；
- `cargo check --workspace`：通过；
- `cargo test --workspace`：通过；
- `cargo clippy -p novadraw-scene --lib -- -D warnings`：通过；
- Rust 源码已无 `Updatable` 引用。

### D1.3 公开 API 真实性

状态：`complete`

目标：

- 审计 NdCanvas、Path 和 FigureTree 的公开占位 API；
- 可支持的能力补齐 command/backend 语义与测试；
- 暂不支持的能力删除、收窄或返回 Unsupported；
- 删除 `apply_layout`、`compute_layout_size` 等绕过正式布局事务的旧入口。

执行结果：

- 删除 `FigureTree::{revalidate_with_bounds,apply_layout,compute_layout_size}` 旧占位入口；
- 删除 NdCanvas 中 line dash/miter、text alignment/baseline、composite/shadow 和 path
  hit-test 等静默 no-op；
- 删除未由 NdCanvas 生成且 Vello 静默忽略的 `RenderCommandKind::Clear`；
- 删除未实现圆角语义的 `RectangleBorder::with_corner_radius`，圆角 Figure 的真实路径
  绘制能力保持不变；
- `cargo check --workspace`：通过；
- `cargo test --workspace`：通过；
- `cargo clippy -p novadraw-render -p novadraw-scene --lib -- -D warnings`：通过；
- 核心 Rust 源码不再包含 TODO、`todo!`、`unimplemented!` 或公开静默 no-op。

### D1.4 Figure 属性与资源

状态：`complete`

目标：

- 定义完整的 FigureStyle/ResolvedStyle；
- 补齐 foreground、background、alpha、font、cursor、tooltip、opaque 语义；
- 属性变化进入 notification、validation 和 damage；
- 定义 ImageId/FontId、ResourceRegistry、异步完成与依赖失效；
- Runtime 提供统一平台无关输入入口和资源完成入口。

执行批次：

1. D1.4a：FigureStyle/ResolvedStyle、继承、通知和绘制应用，
   `complete`；
2. D1.4b：cursor/tooltip 查询与 PlatformHost effect，
   `complete`；
3. D1.4c：ImageId/FontId、ResourceRegistry 和异步完成事务，`complete`。

D1.4a/b 执行结果：

- 新增 `FigureStyle` 和 `ResolvedStyle`，foreground、background、alpha、font、cursor
  与 tooltip 按最近祖先独立解析；
- tooltip 使用继承/显式关闭/本地文本三态；
- 递归绘制在 Figure paint 前应用 resolved style，并删除 `Figure::init_properties`
  双真源；
- style 变化产生 typed property event，font 变化使子树失效，视觉属性变化重绘子树；
- Runtime 提供 `cursor_icon()` 和 `tooltip()`，Winit/Web/editor 输入事务后同步 host
  cursor；
- `style-app` 新增 `Inherited FigureStyle` 场景；
- `cargo check --workspace`、`cargo test --workspace`（含 201 项 scene 单测）和核心
  Clippy 通过；
- 样式、通知、绘制和 cursor/tooltip Runtime 契约测试通过。

人工门禁：

- [`../verification/manual/d1-figure-style.md`](../verification/manual/d1-figure-style.md)
- 2026-09-03 macOS：PASS，无失败项。

D1.4c 执行结果：

- 新增 namespace + slot generation 组成的 `ResourceId`，并通过 `ImageId`/`FontId`
  提供 typed handle；
- 新增 `ResourceRegistry`，实现 Pending/Ready/Failed/Removed 状态、单调 revision
  与 Figure 依赖索引；
- 资源完成、失败和移除只通过 Runtime 事务进入，并对仍附着的依赖 Figure 执行
  invalidate + repaint；
- `ResourceDelta` 随 `RenderSubmission` 提交，支持无场景 damage 的 resource-only
  submission；
- submission retry 会把旧 in-flight delta 恢复到新 delta 之前，保持资源更新因果顺序；
- 图像与字体 payload 使用 `Arc` 共享，避免 Registry、submission 和 in-flight
  之间深拷贝；
- `FigureTree::set_contents` 现在会脱离旧 contents，资源依赖清理不再保留旧场景引用；
- 资源生命周期与 contents 替换定向测试共 11 项通过；
- `cargo fmt --all -- --check`、`cargo check --workspace` 和
  `cargo test --workspace` 通过；
- `cargo clippy -p novadraw-render -p novadraw-scene --lib -- -D warnings` 通过；
- `cargo check -p web-validation --target wasm32-unknown-unknown` 通过。

### D1.5 树查询与焦点遍历

状态：`in_progress`

目标：

- 引入可接受/剪枝的 TreeSearch 策略；
- 支持 exclusion search、ancestor/descendant 查询；
- 定义稳定、可配置的 focus traversal；
- 保持普通输入单 target，不引入 DOM 式通用冒泡。

候选契约：

- [`../design/architecture/tree-search-and-focus.md`](../design/architecture/tree-search-and-focus.md)

执行批次：

1. D1.5a：TreeSearch、ExclusionSearch 和共享 hit-test traversal，`complete`；
2. D1.5b：显式 focusable/focus_traversable 与 tree-order policy，`complete`；
3. D1.5c：Native/Web Tab traversal 和人工验收，`not_started`。

评审点：

- `accept` 只决定当前节点能否返回，`prune` 排除当前节点和整个子树；
- visibility、enabled、几何、裁剪和坐标转换仍是不可绕过的树不变量；
- direct focus 与 traversal focus 使用独立资格；
- 默认 traversal 按稳定前序/逆序且不 wrap；
- focus owner 失效必须发出 FocusLost，不能静默清除；
- traversal 到边界时交回平台，不把同一次 Tab 再投递为普通 key event。

D1.5a 执行结果：

- 新增只读 `TreeSearchContext` 及 `TreeSearch`、`IdentitySearch`、
  `ExclusionSearch`；
- 新增 `hit_test_with`、`hit_test_excluding`、`find_in_subtree`、
  `ancestor_ids`、`descendant_ids` 和 `is_ancestor_of`；
- 普通命中、mouse event target、cursor、tooltip 与 gesture 共用同一坐标、裁剪和
  reverse Z-order 遍历内核；
- `prune` 在访问子树前执行，`accept` 在所有可命中子节点失败后决定当前节点；
- 5 项内部搜索测试与 2 项 public API 契约测试通过；
- `cargo test --workspace`、核心 Clippy 和 Web target check 通过。

D1.5b 执行结果：

- `NodeState` 新增互相独立且不继承的 `focusable` 与 `focus_traversable`，默认均为
  `false`，变化产生 typed property event；
- 新增 `FocusTraversalPolicy`、`TreeOrderFocusTraversal`、方向/结果类型与结构化
  `FocusError`；
- Runtime 提供 `request_focus`、`clear_focus`、`traverse_focus` 和 policy 替换入口；
- 默认 policy 按 contents 子树稳定前序/逆序遍历，跳过 hidden/disabled 节点且不
  wrap；
- focus owner 先原子提交，再按 lost → gained 投递含 related target 的事件；
- remove、hide、disable 和 reparent 到无效祖先均通过同一事务发出一次 FocusLost，
  `InteractionState` 不再静默删除 owner；
- 鼠标 handled target 不具备 direct focus 资格时保留旧 owner；
- 10 项 focus 定向测试、1 项 public API 契约测试及 226 项 scene 单测通过；
- `cargo test --workspace`、核心/公共契约 Clippy 和 Web target check 通过。

D1 完成门禁：

- 核心 public API 无静默 no-op；
- FigureTree 只持有拓扑、节点与树级通知；
- style、resource、search 和 focus 能支撑 M9/M10，且无产品特例；
- R8 性能基线无超过既定阈值的回归。

## 5. D2：Layer 与 Freeform 基础

状态：`not_started`

目标：

- LayerFigure 与 LayeredPane；
- FreeformLayerFigure、FreeformLayout 和 freeform extent；
- 负坐标内容范围与 origin 变化通知；
- layer key、稳定 Z-order 与 layer 查询；
- viewport/zoom 嵌套下的 paint、hit-test、event point 和 damage。

D2 是 ConnectionLayer 和大型编辑画布的前置条件，但不引入 GEF EditPart 或 Tool。

完成门禁：

- 正负坐标内容均可达；
- extent 随 add/remove/move 原子更新；
- layer paint 顺序与逆序 hit-test 一致；
- viewport/zoom/partial damage 组合测试通过。

## 6. M9：Connection 分批交付

状态：`not_started`

执行顺序：

1. **M9.1 Anchor/Router 纯计算契约**
   - SceneQuery 只读输入；
   - Anchor owner/reference/location；
   - Router 输入、输出、constraint 和结构化错误。
2. **M9.2 Connection runtime**
   - ConnectionState；
   - owner -> connection 依赖索引；
   - route cache 与失效传播；
   - owner remove/reparent 的失效处理。
3. **M9.3 Connection Figure**
   - ConnectionFigure、PolylineConnection；
   - point list 同时驱动 paint、stroke-aware hit-test 和 visual bounds；
   - Direct、Chopbox、Ellipse、XY Anchor。
4. **M9.4 Router**
   - BendpointConnectionRouter；
   - ManhattanConnectionRouter；
   - FanRouter；
   - ShortestPathConnectionRouter 继续延后。
5. **M9.5 Locator、Decoration 与 Layer**
   - Endpoint/Midpoint/Connection Locator；
   - polygon/polyline decoration；
   - ConnectionLayer；
   - viewport/zoom/deep-tree 集成。
6. **M9.6 产品验收**
   - `connections-demo`；
   - anchor × router 组合矩阵；
   - 节点移动、resize、reparent、remove、滚动和缩放视觉断言。

## 7. M10：Reusable Figure 分批交付

状态：`not_started`

执行顺序：

1. **M10.1 Graphics、Shape 与 Border**
   - deferred shape 的精确 hit-test、visual bounds、style 和 update；
   - Border preferred size、opaque；
   - Compound、TitleBar、Etched、Bevel Border。
2. **M10.2 文本与 Label**
   - 真实字体测量；
   - LabelFigure 文本/图标、alignment、gap、truncate；
   - preferred/min/max size 与布局缓存联动。
3. **M10.3 ImageFigure 与资源**
   - ImageId；
   - pending/ready/failed fallback；
   - 图像完成后的 revalidate/repaint。
4. **M10.4 基础控件**
   - Clickable/Button/Toggle model；
   - pressed、rollover、selected 与 focus visual；
   - action/change notification。
5. **M10.5 Tooltip、焦点与 Accessibility**
   - hover delay 与 show/hide；
   - 键盘遍历；
   - AccessibleFigure 到 PlatformHost bridge。

M10 不扩张为完整 widget toolkit，也不实现富文本编辑器。

## 8. Draw2D Core 1.0 完成门禁

- M1-M10 全部达到 `complete`；
- P0/P1 API family 不存在未解释的 `missing`；
- public API 不包含静默 no-op 或占位成功；
- macOS、Web、Headless 自动与人工门禁通过；
- 关键组合场景覆盖 layout、event、viewport、connection、text 和 resource；
- API 文档、语义账本、产品清单和 demo 矩阵一致；
- 完成 R9.4 capability 消融复查。

满足这些条件后，才启动独立的 GEF roadmap，包括 EditPart、Viewer、Tool、Request、
EditPolicy、Command、SelectionProvider 和 undo/redo command stack。

## 9. 延后能力

以下能力不阻塞 Draw2D Core 1.0：

- 富文本 Flow 与编辑；
- Animation/Animator；
- PrinterGraphics、ScaledGraphics 和打印工作流；
- DirectedGraphLayout、CompoundDirectedGraphLayout；
- ShortestPathConnectionRouter；
- XOR、pattern 和平台特定高级 Graphics 模式；
- 完整 widget toolkit；
- Windows/Linux 发布资格验证。
