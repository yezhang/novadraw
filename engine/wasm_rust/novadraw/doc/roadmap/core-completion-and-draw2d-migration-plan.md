# 核心收口与 Draw2D 功能迁移计划

类型：`roadmap`

状态：`in_progress`

本文定义 R8/R9 架构迁移完成后，Novadraw 从“核心运行时主路径已验证”推进到
“Draw2D 核心产品能力完整”的执行顺序。

本文不创建新的 milestone 编号。`M1-M10` 的编号与状态仍以
[`00-index.md`](00-index.md) 为唯一入口；本文中的 `D0-D4` 是跨 milestone 的
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

M1-M10 产品能力现已完成，当前结论是：

> Draw2D 核心框架公共面与产品能力完整，Core 1.0 尚待最终跨平台总审计。

当前剩余收口项：

1. D3 已关闭原 Draw2D Core 审计中的 M9、Runtime mutation 与 listener 公共面缺口；
2. D4 已关闭长期架构审计中的 A01-A08；
3. M1-M8 产品与人工验收差额已于 2026-09-12 收口；
4. M10 Text/Image/Widget Web 等价场景已于 2026-09-13 完成；
5. 当前执行 macOS/Web/Headless 总审计与 R9.4 capability 消融复查。

审计基线：

- [`../verification/reviews/draw2d-core-capability-audit-2026-09-08.md`](../verification/reviews/draw2d-core-capability-audit-2026-09-08.md)；
- [`../verification/reviews/architecture-sustainability-review-2026-09-08.md`](../verification/reviews/architecture-sustainability-review-2026-09-08.md)。

D1 启动时识别出的 selection、兼容 capability、Figure style、公开 no-op、
TreeSearch 和资源生命周期问题均已在 D1.1-D1.5 中收口。

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

2026-09-12 最终校准：

- M1-M6 与 M8 自动、Native 和 Web 验收已完成，状态提升为 `complete`；
- 验收记录见
  [`../verification/reviews/m1-m8-manual-acceptance-2026-09-12.md`](../verification/reviews/m1-m8-manual-acceptance-2026-09-12.md)；
- ScrollBar 按住 repeat firing 明确随 P2 widget scheduler 延后，不阻塞 M8。

## 4. D1：核心公共协议收口

状态：`complete`

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

状态：`complete`

目标：

- 引入可接受/剪枝的 TreeSearch 策略；
- 支持 exclusion search、ancestor/descendant 查询；
- 定义稳定、可配置的 focus traversal；
- 保持普通输入单 target，不引入 DOM 式通用冒泡。

规范契约：

- [`../design/architecture/tree-search-and-focus.md`](../design/architecture/tree-search-and-focus.md)

执行批次：

1. D1.5a：TreeSearch、ExclusionSearch 和共享 hit-test traversal，`complete`；
2. D1.5b：显式 focusable/focus_traversable 与 tree-order policy，`complete`；
3. D1.5c：Native/Web Tab traversal 和人工验收，`complete`。

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

D1.5c 自动验证结果：

- `novadraw-apps` 新增共享 `adapt_key_input`，Tab keydown 映射为 forward/backward
  traversal，Tab keyup 不再投递普通 key；
- 通用 Native app 与 editor 均通过 Runtime traversal 入口处理 Tab/Shift+Tab；
- Web 仅在 `FocusTraversalOutcome::Moved` 时调用 `preventDefault`，boundary 保留浏览器
  默认焦点移动；
- Native 与 Web 复用同一 `A → Group(B → Skip → C) → D` 验证树，实际有效顺序为
  `A → B → C → D`，可观察非聚焦容器下钻和 disabled candidate 跳过；
- Headless public contract 覆盖 forward、backward 和双向 boundary；
- `cargo test --workspace`、核心 Clippy、Web target check 与 release wasm-bindgen
  打包通过；
- 人工门禁：
  [`../verification/manual/d1-focus-traversal.md`](../verification/manual/d1-focus-traversal.md)。

D1.5c 人工验收记录：

- 日期：2026-09-04；
- 平台：macOS / Web；
- 结果：PASS；
- 失败项：无。

D1 完成门禁：

- 核心 public API 无静默 no-op；
- FigureTree 只持有拓扑、节点与树级通知；
- style、resource、search 和 focus 能支撑 M9/M10，且无产品特例；
- R8 性能基线无超过既定阈值的回归。

D1 最终验证结果：

- 修复渲染时逐节点回溯祖先解析样式造成的深树 O(n²) 回归，改为 Draw2D 一致的
  local style override + Graphics state inheritance；
- R8 benchmark command 数与基线完全一致；
- 三次追加复测的 median-of-medians 均未超过 15% 回归阈值；
- `cargo test -p novadraw-scene --lib --tests`：227 项单测及 49 项契约测试通过；
- `cargo clippy -p novadraw-scene --lib -- -D warnings`：通过。

## 5. D2：Layer 与 Freeform 基础

状态：`complete`

目标：

- LayerFigure 与 LayeredPane；
- FreeformLayerFigure、FreeformLayeredPane、ScalableFreeformLayeredPane、
  FreeformLayout 和 freeform extent；
- 负坐标内容范围与 origin 变化通知；
- layer key、稳定 Z-order 与 layer 查询；
- viewport/zoom 嵌套下的 paint、hit-test、event point 和 damage。

D2 是 ConnectionLayer 和大型编辑画布的前置条件，但不引入 GEF EditPart 或 Tool。

规范契约：

- [`../design/architecture/layer-and-freeform.md`](../design/architecture/layer-and-freeform.md)

`api_semantics`：

- `figure.tree`
- `hit_test.search`
- `clipping.strategy`
- `coordinate.conversion`
- `layout.manager`
- `update_manager.two_phase`
- `damage.repaint`
- `notification.property`
- `viewport.scroll_zoom`
- `layer.freeform`

执行批次：

1. **D2.0 契约接受**
   - 状态：`complete`；
   - 完成候选契约评审；
   - ADR-004 接受关键取舍；
   - 同步 tree-search、coordinate、static architecture 和 UpdateManager SSOT。
2. **D2.1 Layer 基础**
   - 状态：`complete`；
   - Layer capability、HitParticipation 与默认透明命中；
   - LayerKey、LayeredPaneState 和 LayeredPaneHandle 命名操作；
   - 构建期 FigureTreeBuilder、运行期 Runtime topology mutation；
   - key 唯一性、before/after 与原子失败契约。
3. **D2.2 Freeform extent**
   - 状态：`complete`；
   - LayoutState 中的可选 FreeformState 派生缓存；
   - 正负坐标、空容器和 nested freeform 范围；
   - bottom-up 失效传播与单 generation 线性重算；
   - FreeformLayeredPane 与 `ChildClippingStrategy::OverflowVisible`；
   - typed property extent notification。
4. **D2.3 FreeformLayout**
   - 状态：`complete`；
   - typed rectangle constraint；
   - intrinsic size fallback；
   - 保留负坐标的 LayoutSnapshot/LayoutOutput 提交。
5. **D2.4 Viewport / Zoom 集成**
   - 状态：`complete`；
   - freeform extent 与内容原点处 viewport baseline 合并；
   - content-domain RangeModel 与非零/负 minimum；
   - ScalableFreeformLayeredPane 复用现有 scale state；
   - scale、origin、range、damage 同事务更新。
6. **D2.5 Demo 与人工验收**
   - 状态：`complete`；
   - 新增共享 Native/Web 场景；
   - layer 顺序、透明命中、四方向滚动与缩放；
   - Headless 契约、macOS/Web 人工验收与文档收口。

D2.1 执行结果：

- 新增 `LayerFigure`、`LayeredPane`、`Layer` marker 与
  `HitParticipation::DescendantsOnly`；
- 新增非空 `LayerKey`、`LayerPlacement`、Runtime 私有 `LayeredPaneState` 和短生命周期
  `LayeredPaneHandle`；
- typed add/remove/move/reparent 原子维护 key/member 双向索引，children 顺序仍是唯一
  Z-order；
- 泛型 add/remove/reparent 无法绕过 LayeredPane key 与 child capability 门禁；
- LayeredPane 默认接入 StackLayout，作为 contents 或 nested layer 时均可注册；
- callback keyed layer mutation 已接入 effect queue，add/remove/move/reparent 与普通
  mutation 共享 FIFO 提交；
- 新增显式 `FigureTreeBuilder`，外部批量场景构建、Viewport、ScalablePane 和
  ScrollPane 构造统一经 builder；FigureTree 底层 topology mutator 已收窄为
  crate-private；
- editor、scroll-pane-demo、R8 benchmark 与全部 public contract tests 已迁移到
  builder API；
- `d2_layer_contract` 7 项通过，覆盖透明命中、key 唯一、before/after、逆序命中、
  泛型绕过拒绝、跨 pane reparent、remove、contents 注册和 callback FIFO；
- `cargo check -p novadraw-scene --tests` 与
  `cargo clippy -p novadraw-scene --lib -- -D warnings` 通过；
- workspace/full-test 门禁受当前沙箱禁止执行 Xcode clang 阻断，不是代码诊断失败。

D2.2 首批执行结果：

- 新增 `Freeform` capability、`FreeformLayerFigure` 与 `FreeformLayeredPane`；
- `LayoutState` 持有可选 `FreeformState`，缓存稳定 extent、generation 与 dirty 状态；
- direct child border-box 与 nested freeform extent 按完整 edge transform 自底向上归并；
- 空容器、负坐标、隐藏 child、presentation bounds 隔离和纯 translate 失效已覆盖；
- 新增 typed `freeform_extent` property 与区分 unknown/non-freeform/unvalidated 的查询错误；
- `ChildClippingStrategy::OverflowVisible` 已统一接入 paint、hit-test 与 damage；
- `d2_freeform_contract` 10 项、既有 clipping 回归和 14 项 damage repair 测试通过。

D2.3 执行结果：

- 新增 `FreeformConstraint` 校验构造器，区分 intrinsic fallback 与显式 zero；
- `FreeformLayout` 从 LayoutSnapshot 计算 prospective extent，不读取 extent cache；
- layout 保留负 origin，无约束 child 保持现有 bounds；
- constraint 类型错误在 LayoutOutput 提交前失败，不产生部分 child bounds 更新；
- `d2_freeform_contract` 扩展至 13 项并全部通过。

D2.4 执行结果：

- 新增 `ScalableFreeformLayeredPane`，在同一 Figure 上组合 Layer、Freeform 与
  ScalableFigure capability，并复用既有 `ScaleRuntime` / `ScaleHandle`；
- Viewport 在 direct contents 具备 Freeform capability 时，先验证 child extent，再以
  `union(freeform_extent, viewport_baseline)` 更新 content-domain RangeModel；
- RangeModel 的 minimum、maximum、extent 与 value 均保持未缩放 content units，
  Viewport child transform 统一执行 `(content_point - origin) * scale`；
- ZoomManager 在 Freeform 模式使用 content-domain anchor 公式，fit 操作读取派生
  extent；普通 Viewport 的既有 scaled-range 行为保持不变；
- extent 收缩导致 origin clamp 时，通过 LayoutOutput 在同一 validation 事务提交
  viewLocation property、coordinate-system change 以及 viewport/parent damage；
- `d2_freeform_contract` 扩展至 17 项，`m8_viewport_contract` 24 项和
  `d2_layer_contract` 7 项全部通过；
- `cargo check -p novadraw-scene --tests` 与
  `cargo clippy -p novadraw-scene --lib -- -D warnings` 通过；
- 全包测试仍受当前沙箱禁止执行 Xcode clang 阻断，不是 Rust 代码诊断失败。

D2.5 自动验证结果：

- 新增 `layer-freeform` 共享 suite，包含 layer order、negative origin、positive extent
  和 zoomed freeform 四个 Native/Web 共用场景；
- `scroll-pane-demo --verify` 新增 freeform range、透明层逆 Z 命中、四方向 clamp 与
  content-domain anchor zoom 三项 headless case；
- `novadraw-demo-scenes` 全量迁移到显式 `FigureTreeBuilder`，消除 D2.1 收窄原始
  topology mutator 后遗留的跨 crate 旧入口；
- 修复 Viewport 对 direct Freeform contents 追加 presentation-bounds clip 的问题，负/正
  extent 边界内容在 Native 截图中均可见；
- 空白 Viewport target 会把 zoom 路由到 direct scalable contents；场景增加坐标网格，
  使 pinch anchor 可观察；
- Native 截图改用 retained texture 离屏提交，不依赖 swapchain drawable，并统一输出到
  `target/visual-verification/screenshots/`；
- WebInputAdapter 将浏览器 pinch / `Ctrl+wheel` 映射为带 CSS 逻辑坐标锚点的
  `ZoomEvent`，普通 wheel 保持 scroll 语义；
- `novadraw-demo-scenes` 6 项测试、D2 Freeform 18 项、M8 Viewport 24 项及相关库级
  Clippy 通过；
- Native/Vello 与 Web 人工视觉/交互验收均通过，D2.5 和 D2 总阶段完成。

评审点：

- freeform extent 是 child content domain 中的派生状态，不是第二份 bounds；
- 不递归改写普通 child bounds，但保留 nested freeform extent/envelope 传播；
- LayeredPane 不建立平行于 FigureTree 的 layer topology；
- Layer membership 独立于 LayoutManager，children 顺序仍是唯一 Z-order；
- Freeform overflow 在 paint、hit-test 和 damage 中使用同一 clipping strategy；
- FreeformLayout 默认保留负坐标，不在运行时自动平移已有 children；
- extent 通知必须在 layout、viewport range 和 damage 稳定后发送；
- ConnectionLayer 只能在 M9 扩展 Layer capability，不能反向引入 Connection 状态。

完成门禁：

- 正负坐标内容均可达；
- extent 随 add/remove/move 原子更新；
- layer paint 顺序与逆序 hit-test 一致；
- viewport/zoom/partial damage 组合测试通过。

## 6. M9：Connection 分批交付

状态：`complete`

候选契约：

- [`../design/architecture/connection-routing.md`](../design/architecture/connection-routing.md)
- [`../adr/adr-005-connection-routing-contract.md`](../adr/adr-005-connection-routing-contract.md)

ADR-005 已通过；M9.1-M9.6 的当前产品基线、自动验证和人工窗口验收已完成。
2026-09-08 审计发现的 shared Manhattan reservation 与 nested viewport policy
缺口已由 D3.1 闭合，M9 自动、视觉和人工门禁均已完成。

执行顺序：

1. **M9.1 Anchor/Router 纯计算契约**
   - 状态：`complete`；
   - tracked SceneQuery 只读输入与 dependency token；
   - Anchor owner/reference/location、group key 与 named geometry；
   - RouterId/RouterRegistry、输入、输出、constraint 和结构化错误。
2. **M9.2 Connection runtime**
   - 状态：`complete`；
   - ConnectionState；
   - dependency token -> connection 反向索引；
   - inherited/explicit Router binding 与 RouterRegistry；
   - route generation、unresolved 状态与失效传播；
   - owner remove/reparent 与 dependency cycle。
3. **M9.3 Connection Figure**
   - 状态：`complete`；
   - ConnectionFigure、PolylineConnection；
   - Runtime 原子提交 local point list 与 path bounds；
   - point list 同时驱动 paint、stroke-aware hit-test 和自身 visual bounds；
   - locator child 通过独立 subtree visual envelope 参与 damage/freeform extent；
   - Direct Router；
   - XY、Chopbox、Ellipse、RoundedRectangle、Label Anchor。
4. **M9.4 Router**
   - 状态：`complete`；
   - BendpointConnectionRouter（完成）；
   - ManhattanConnectionRouter（单连接正交路由与 D3.1 shared reservation 完成）；
   - base router + Fan post-processor pipeline；
   - RouterId scope 下的 routing group state、稳定 snapshot 与批量原子提交；
   - ShortestPathConnectionRouter 继续延后。
5. **M9.5 Locator、Decoration 与 Layer**
   - 状态：`complete`；
   - Endpoint/Midpoint/Connection/PathFraction Locator；
   - polygon/polyline decoration；
   - ConnectionLayer inherited router 与 explicit override；
   - viewport/zoom/deep-tree 与 D3.1 strict viewport topology 已验证。
6. **M9.6 产品验收**
   - 状态：`complete`；
   - `connections-demo`；
   - anchor × router 组合矩阵；
   - 节点移动、resize、reparent、remove、滚动和缩放视觉断言。

M9.1 执行结果：

- 新增 `connection` 模块，公开 `ConnectionId`、`AnchorId`、`RouterId`、
  `CoordinateSpace`、`DependencySubject` 和 `SceneQuery` 纯计算边界；
- `SceneRead` 与 `TrackedSceneQuery` 分离只读数据源和 dependency tracking；
- 新增 XY、Chopbox、Ellipse、RoundedRectangle 与 Label Anchor；Label 通过 named
  `icon` geometry 解耦具体 LabelFigure；
- 新增 `AnchorSemanticKey`，为后续 Fan group 提供 value identity，自定义 Anchor
  缺省保留实例 identity；
- 新增 `RoutingConstraint`、`RouteRequest`、`RouteOutput`、endpoint metadata 与
  结构化错误，输出统一校验点数、有限性和首尾 endpoint 一致性；
- 新增确定性的 `DirectRouter`，保持 source/target 双向 reference 语义；
- `m9_connection_contract` 11 项通过，`novadraw-scene` 227 项库测试无回归。

M9.2 执行结果：

- Runtime 私有 `ConnectionRuntime` 统一持有 Anchor、Router、ConnectionState、
  layer default 和 dependency reverse index；
- `FigureTreeSceneRead` 负责 Figure local / child content / logical surface 映射，
  normal 使用 inverse-transpose；
- Runtime 公开 Anchor/Router 注册、Connection 状态绑定、constraint 设置、route
  resolve、state/error 查询接口；
- source/target 缺失进入 typed unresolved，重新绑定后回到 dirty；
- route 成功原子替换 dependencies，失败合并已读取 dependencies，避免恢复后无法调度；
- bounds、reparent、remove、contents replacement 和 deferred mutation 已接入失效与
  detached ConnectionState 清理；
- Router/Anchor 被引用时不可删除，Router 切换先校验 constraint，失败保持旧状态；
- dependency generation drift 与 Connection 自身/子树依赖环均结构化拒绝；
- `m9_connection_runtime` 6 项通过。

M9.3 执行结果：

- 新增 `ConnectionFigure` 与 `ConnectionFigureBehavior` capability；
- Runtime route 输出在 parent child content domain 计算 path bounds，再规范化为
  node-local points，与 NodeState bounds 同事务提交；
- ConnectionFigure 使用 Polyline render command，命中采用 segment distance +
  stroke/tolerance；
- path bounds 只包含 points 与 stroke，locator/decorations 仍保留独立 subtree
  envelope；
- unresolved transition 清空已提交 points 和 bounds，并 damage 旧区域；
- Connection Figure registration 强制 capability 检查，普通 Figure 不能伪装为
  Connection；
- `m9_connection_runtime` 增至 7 项，覆盖 paint command、local normalization、
  precise hit、unresolved clear 与状态清理。

M9.4a 执行结果：

- 新增 `BendpointConstraint`、absolute/relative Bendpoint 和
  `BendpointConnectionRouter`；
- 首尾 Bendpoint 作为 source/target Anchor reference，与 Draw2D 行为一致；
- RelativeBendpoint 使用 source/target reference、两侧 offset 和 `[0, 1]` weight；
- 新增无障碍 `ManhattanConnectionRouter`，根据 Anchor normal 选择正交方向并删除
  相邻重复点；
- `m9_connection_runtime` 增至 9 项；Fan group/batch 与 shared Manhattan
  reservation 留在 M9.4b。

M9.4b-M9.6 执行结果：

- FanRouter 使用 RouterId + 无向 AnchorGroupKey pair + routing-domain child order
  形成稳定分组，增删连接后重新居中；
- `ConnectionLocator::{Source,Target,Middle}`、MidpointLocator 和
  PathFractionLocator 已实现，保留 Draw2D middle 语义；
- 箭头 Decoration 作为 Connection 普通 child，由 Endpoint Locator 结果定位；
- 新增透明 `ConnectionLayerFigure`，验证 inherited Router 与 explicit override
  分层；
- 新增 `apps/native/connections-demo`，包含 anchor_matrix、bendpoint、manhattan、
  fan、moved_nodes、connection_layer 六场景；
- 六场景逐场截图成功并完成视觉复核，无空白帧、端点漂移、非正交段或视口裁剪；
- 六场景人工窗口验收通过，包含 anchor_matrix 箭头锐角和 bendpoint 转折点复核；
- `m9_connection_contract` 12 项、`m9_connection_runtime` 15 项通过；
- D3.1 已完成 shared Manhattan reservation、严格 viewport topology、两项新增
  Demo 场景的截图复核与人工窗口验收，M9 恢复为 `complete`。

## 7. M10：Reusable Figure 分批交付

状态：`complete`

M10.1 正式契约：
[`../design/architecture/reusable-shape-border.md`](../design/architecture/reusable-shape-border.md)。
由 ADR-006 接受。

M10.2 正式契约：
[`../design/architecture/text-layout.md`](../design/architecture/text-layout.md)。
由 ADR-007 接受，按 Text Core、Label、TitleBarBorder 三个原子增量执行。

M10.4 正式契约：
[`../design/architecture/basic-widgets.md`](../design/architecture/basic-widgets.md)。

M10.5 正式契约：
[`../design/architecture/tooltip-accessibility.md`](../design/architecture/tooltip-accessibility.md)。
状态：`complete`。M10.5a/b、M10.5c 自动门禁以及 macOS/Web 人工交互验收均已完成。

执行顺序：

1. **M10.1 Reusable Shape 与 Border 产品化收口**
   - 冻结 M1 已验证的 Graphics 状态栈、坐标、裁剪和基础命令契约，不做重新设计；
   - 以 RectangleFigure 为 baseline，补齐 deferred Figure 的精确 hit-test、
     visual bounds、style 和 update 证据；
   - 复用 FigureStyle 与具体 Figure 构造/更新入口，不默认扩张统一 Shape setter；
   - 补齐 Border preferred size、opaque 语义以及 Compound、Etched、Bevel Border；
   - 仅当具体 reusable Figure 无法基于现有 path/command 正确实现时，才增加经契约
     证明所需的最小 Graphics primitive；clipPath、shear、gradient、XOR 和高级
     stroke 不进入本批次。
2. **M10.2 文本与 Label**
   - M10.2a：可替换 TextLayoutEngine、真实字体测量、不可变 TextLayout、
     backend-neutral glyph IR 与 Vello adapter；
   - M10.2b：LabelFigure 文本/图标、alignment、gap、truncate；
   - M10.2c：TitleBarBorder 的文字测量、insets、preferred size 与绘制；
   - preferred/min/max size 与布局缓存联动。
3. **M10.3 ImageFigure 与资源**
   - ImageId；
   - pending/ready/failed fallback；
   - 图像完成后的 revalidate/repaint。
4. **M10.4 基础控件**
   - Clickable/Button/Toggle model；
   - pressed、rollover、selected 与 focus visual；
   - action/change notification。
5. **D3 Draw2D Core 审计收口**
   - 校准语义账本与 milestone 状态；
   - 恢复 M9 已接受契约；
   - 补齐 Runtime mutation 与 listener 公共面。
6. **M10.5 Tooltip、焦点与 Accessibility**
   - 先建立独立设计契约；
   - hover delay 与 show/hide；
   - Runtime 拥有 tooltip 语义状态与 deadline，Host 只提供单调时间和 wake-up；
   - 复用已验证的键盘遍历与 focus owner；
   - Runtime 生成平台无关 accessibility snapshot/delta，PlatformHost 只做桥接；
   - 最小 accessibility 数据包含 node id、name、role、state、bounds、children、
     focus owner 和 default action。

M10.5 在 D4 完成前不进入实现。M10 不扩张为完整 widget toolkit，也不实现富文本编辑器。

M10.1 自动执行结果：

- 六类 reusable Figure 统一消费 FigureStyle，并补齐 Ellipse、RoundedRectangle、
  Polyline、Polygon、Triangle 的精确命中和 Draw2D 绘制几何；
- Polyline/Polygon 使用 node-local points 与派生 NodeState bounds，Runtime 提供
  replace/insert/set/remove/clear 原子 mutation、失败回滚、damage 与 typed 通知；
- Border 协议新增 preferred size、ring opacity 和累计 inset 绘制，完成 Line、
  Margin、Compound、Etched、Raised/Lowered Bevel；
- Border replacement、corner dimensions 与 triangle direction 进入 Runtime typed
  transaction；
- `m10_reusable_shape_border_contract` 12 项通过，`novadraw-scene` 229 项库测试及全部
  集成测试无回归；
- `shape-app`、`border-app` 已加入 M10.1 场景；基础 Shape/Border 截图复核和全部
  M10.1 场景人工验收通过，其中 MarginBorder 通过非对称 inset 与 child client-area
  裁剪场景完成可视验证；
- M10.1 完成，下一阶段进入 M10.2 文本、Label 与 TitleBarBorder。
- M10.2 契约已由 ADR-007 接受；
- M10.2a 已形成 Parley/Vello 可运行原型，真实 measurement、line breaking、Runtime
  独立所有权和 Vello glyph encoding 已通过自动测试；
- ADR-007 已进一步收紧扩展边界：Parley 仅作为默认 `TextLayoutEngine` adapter，
  Command 必须使用 Novadraw 自有 glyph IR；raw-string `fill_text` / `stroke_text`
  不进入产品路径。
- 第一批 Command 收口已完成：Runtime 支持注入自定义 `TextLayoutEngine`，
  `DrawGlyphRun`、`FontFaceRef`、`GlyphPaint` 与 positioned glyph 均为 Novadraw
  自有类型，Vello 通过独立 adapter 消费；字体字节通过 resource delta 提交。
- 提供可显式注册的 Inter、Noto Sans SC 与 JetBrains Mono，覆盖 UI、CJK fallback
  与技术标注；Runtime 启动不自动注册字体，字体资产使用 Git LFS，Native/Web 构建
  共享同一字体输入。
- M10.2b 已完成：`LabelFigure` 在 Runtime 提交边界生成不可变 layout snapshot，
  支持图标、四向 placement、alignment、gap 与 grapheme-safe ellipsis；已补
  `text-app` 的字体/CJK fallback、截断、图标与 style inheritance 四场景截图复核。
- Vello backend 已消费 `RenderCommandKind::Image`；Parley font stack 使用显式注册的
  字体作为 deterministic fallback，CJK glyph 会从 Noto Sans SC face 输出。
- M10.2c 已完成：`TitleBarBorder` 保持不可变配置，owner-scoped `BorderSnapshot`
  保存派生 TextLayout/insets/preferred size；共享 Border 在不同 owner 字体下互不
  覆盖，`text-app` TitleBarBorder 场景验证标题与 child content 分离。
- M10.3 已完成：`ImageData` 支持 PNG/SVG 字节及 Native 文件路径解码，SVG 经 resvg
  rasterize 并统一为非预乘 RGBA；`ImageFigure` 公开输入使用 `ImageId`，支持自然
  尺寸、alignment、typed replacement 与 Pending/Ready/Failed 状态。`text-app`
  Image_Resources 场景已完成 PNG、SVG 和状态视觉复核。

M10.2/M10.3 完整性复审收口（2026-09-08）：

- `TextLayout` 已包含 source/key、UTF-8 visible range、truncated、engine revision、
  constraints 与首行 ascent/descent/baseline；Label cache 仅在测量输入变化时 shaping。
- raw-string `Text` / `FillText` / `StrokeText` command 与 NdCanvas API 已删除，规范
  文本 IR 唯一为 `DrawGlyphRun`。
- Label 与 ImageFigure 不再缓存 `ImageData`；Image command 使用
  `ImageResourceRef(ResourceId + revision + dimensions)`，Vello 按 revision 缓存、
  替换和删除。
- Label 已接入 Border client area、preferred/minimum size 与 `LabelAnchor` icon
  named geometry。
- `TitleBarBorder` 派生 metrics 已移至 owner-scoped sidecar，共享实例双 owner 测试通过。
- 字体 Failed/Ready/Removed 对 Label 与 TitleBar 的 engine revision 和重布局事务测试
  已通过。M10.2 与 M10.3 标记完成；完整 TextFlow/fragment/bidi API 仍按 P2 延后。

M10.4 自动执行结果（2026-09-08）：

- `ClickableFigure`、`ButtonFigure`、`ToggleFigure` 与 `ClickableModel` 已实现；
  Button/Toggle 组合现有 `LabelFigure`，继续复用文本、图标、ellipsis 与资源契约。
- pointer capture/pressed 与 keyboard pressed 在 `InteractionState` 内分源保存；
  rollover、pressed、focus、enabled 仅作为 Runtime 派生绘制快照进入 Figure。
- mouse release-inside、drag-out cancel、drag-back resume、Enter/Space、focus lost 与
  disabled 行为及 ActionListener 事务 flush 已由 `m10_widget_contract` 十项测试固定。
- Toggle 激活严格按 selected property change → typed `ActionEvent` 顺序进入同一
  notification queue；programmatic `do_click` 复用同一事务。
- `widgets-app` 的 Button_States、Toggle_States、Interactive_Widgets 三场景截图已
  生成并复核；2026-09-08 macOS 人工窗口验收通过，M10.4 标记 `complete`。
- Repeat firing、ButtonGroup/radio 互斥与完整 widget toolkit 保持延后，不进入
  M10.4 完成条件。

M10.5 自动执行结果（2026-09-10）：

- Runtime-owned `TooltipController` 使用 Host 单调时间，完成 waiting/visible/hidden、
  source 继承/显式关闭、内容替换、超时和输入取消状态机；
- Native 使用最终 overlay command 合成 Tooltip，Web 使用共享 placement 的 DOM
  popup；Winit/Web/Headless 共用 deadline、Show/Replace/Hide update；
- `AccessibleFigure` 已扩展 name/description/value/role/default action，Label、
  Button、Toggle 形成首批内置语义；
- Runtime 从 stable scene 发布 namespaced accessibility Snapshot/Delta，覆盖 logical
  bounds、层级提升、enabled/focus/pressed/selected 与 dispose removal；
- accessibility focus/default action 分别复用 `request_focus` 与 `do_click`，保持
  Toggle property change → ActionEvent 因果；
- accessibility 投影沿递归携带 effective flags 与 surface transform，10,000 层
  debug 全路径通过，未重新引入逐节点祖先扫描；
- `m10_tooltip_contract`、`m10_accessibility_contract`、Host adapter 与 Native overlay
  测试通过；workspace test、Clippy、WASM release build 通过；
- `widgets-app` 新增 Tooltip/Accessibility 场景，Native/Vello 边界上翻截图已复核；
  Web Vello 实测 Tooltip DOM、4 个 accessibility 节点、零 console error；
- 2026-09-11 macOS Tooltip、Accessibility/键盘和 Web 复核三组人工验收均通过，
  M10.5 标记 `complete`；
- 2026-09-13 Text/Image 六个共享场景与 Widget 五个共享场景完成 Web/Vello
  等价验证，覆盖字体/CJK、ellipsis、图标 placement、style inheritance、
  TitleBarBorder、图像资源四态、Button/Toggle、Tooltip placement 与 accessibility
  DOM；M10 总项标记 `complete`。

## 8. D3：Draw2D Core 审计收口

状态：`complete`

D3 是 M10.4 与 M10.5 之间的跨 milestone architecture delta。它不重写既有路线图，
也不创建新的产品 milestone；目标是把 2026-09-08 审计发现的“实现存在但公共不可达”
和“已接受契约与完成状态不一致”在继续扩展产品能力前收口。

审计输入：
[`../verification/reviews/draw2d-core-capability-audit-2026-09-08.md`](../verification/reviews/draw2d-core-capability-audit-2026-09-08.md)。

执行原则：

- 审计报告记录事实，不直接替代设计与 ADR；
- 已完成且证据仍有效的 D0-D2、M9、M10 实现不推倒重做；
- 发现契约冲突时，先明确保留、修订或延后的设计决策，再修改实现状态；
- 不通过开放 `&mut FigureTree` 绕过 Runtime 事务边界；
- 不为关闭账本 mechanically 复制 Draw2D 的全部方法名；
- 每个批次独立提交，先契约和测试，再实现，最后更新状态。

`api_semantics`：

- `graphics.context`
- `figure.tree`
- `figure.lifecycle`
- `figure.geometry.bounds`
- `figure.properties`
- `paint.protocol`
- `border.protocol`
- `layout.manager`
- `damage.repaint`
- `event.dispatcher`
- `notification.figure`
- `notification.coordinate`
- `notification.property`
- `notification.ancestor`
- `notification.layout_update`
- `connection.router`
- `clipping.strategy`
- `accessibility.bridge`

### D3.0 审计吸收与证据校准

状态：`complete`

目标：

- 将审计报告纳入 parity 与 verification 索引；
- 逐项校准 API coverage 中已实现、公共不可达、等价表达和明确延后的状态；
- 记录 M10.4 人工验收结果并保持 `complete`；
- M9 在已接受契约恢复前从 `complete` 回退为 `in_progress`；
- M7 保留 `behavior_verified`，但明确 UpdateManager 内部能力与 Runtime 公共面的差异。

必须校准的陈旧项：

- Figure remove/reparent/parent 查询；
- Border preferred size 与 opaque；
- text layout API 与已删除 raw-string API；
- Compound、Etched、Bevel、TitleBar Border；
- M7 listener 的 Runtime 可达性；
- M9 shared reservation 与 nested viewport policy；
- Accessibility 是否属于 M10 Core 1.0 门禁；
- Graphics 子集中的等价 primitive 与明确延后项。

完成门禁：

- 审计、路线图、产品清单、demo 矩阵和 API coverage 不再互相矛盾；
- 每个 P0/P1 `partial` 或 `missing` 都有执行批次、明确变体或延后理由；
- 状态校准不以删除已接受契约来适配现状。

### D3.1 M9 已接受契约恢复

状态：`complete`

补充决策：
[`../adr/adr-008-m9-contract-recovery.md`](../adr/adr-008-m9-contract-recovery.md)。

ADR-008 已通过，实现、自动验证、截图复核和新增场景人工验收均已完成。

决策：

- 继续遵守 ADR-005，不把已接受的 shared routing 和 viewport topology 语义静默降级；
- Manhattan scope 使用 RouterId + routing domain，Fan scope 才额外包含无向
  AnchorGroupKey pair；
- 实现 shared Manhattan row/column reservation、稳定 group snapshot、组级失效与
  批量原子提交；
- Core 1.0 对 nested viewport 采用最小严格策略：相同有效 viewport chain 可继续路由，
  不同 nested viewport chain 返回 `RouteError::UnsupportedViewportTopology`；
- nearest-common-viewport clipping 和 ShortestPath Router 保持后续增强。

验证：

- 同一 RouterId 与 routing domain 下 Manhattan reservation 稳定且无冲突；
- child order 变化、连接增删和 endpoint 变化触发整组确定性重算；
- 不同 RouterId 或 routing domain 不共享 reservation；
- divergent nested viewport topology 真实产生结构化错误并清除旧 route；
- 同一 viewport chain 的既有 route、locator、damage 和 viewport/zoom 行为无回归。

完成门禁：

- ADR-005、connection-routing、API coverage、实现和测试一致；
- M9.4/M9.6 恢复为 `complete`；
- 不以注释“后续增强”覆盖已接受且仍在 Core 1.0 范围内的契约。

自动执行结果：

- `RoutingGroupScope` 明确区分 independent、AnchorPair 与 RoutingDomain；
- Manhattan 按 RouterId + routing domain 的稳定 child order 计算完整 route batch，
  row/column lane 使用命名 spacing 与 minimum stub，任一成员失败时整组不提交；
- `ConnectionRuntime` 保存最近 routing space，targeted dependency invalidation 会扩展到
  同 scope 全部成员；
- topology 比较 connection parent、source owner、target owner 到公共树根的 viewport
  chain；divergent chain 进入 `UnsupportedViewportTopology` unresolved transaction；
- topology 恢复后整组重新 resolved，旧 route 在失败时已清除并 damage；
- `m9_connection_runtime` 从 10 项扩展到 15 项，覆盖 scope 隔离、不同 anchor pair、
  lane 回收、minimum stub、整组失败和 viewport 恢复；
- `connections-demo` 新增 `shared_manhattan` 与
  `unsupported_viewport_topology`，截图已生成并完成视觉复核；
- 全量 workspace、Clippy、WASM `novadraw` 与 `web-validation` 门禁通过。

人工执行结果：

- `shared_manhattan` 显示 4 条清晰分离的内部垂直 lane；相同 Anchor 的水平首尾
  stub 按契约重合；
- `unsupported_viewport_topology` 不绘制连接线、残留箭头或旧路径像素；
- 场景切换无崩溃和脏区残影。

### D3.2 Runtime 动态 mutation 公共面

状态：`complete`

目标：

- Runtime 提供 typed、update-aware 的 layout manager replacement；
- Runtime 提供 constraint set/remove；
- Runtime 提供 preferred/minimum/maximum size set/clear；
- Runtime 提供 child index、bring-to-front、send-to-back；
- Runtime 提供受控 clipping strategy replacement，不开放可变策略对象；
- 删除或私有化不参与真实 traversal 的 `Figure::paint_children` 公开 no-op；
- callback 内对应操作通过 `PendingMutation` 保持 FIFO，不允许重入 FigureTree mutation。

事务要求：

- 参数与 capability 校验在提交前完成，失败不产生部分状态；
- layout/constraint/size 变化正确触发 validation、ancestor extent 和 damage；
- child-order 变化同步 paint order、逆序 hit-test、layer/routing group 与 connection
  invalidation；
- remove/reparent 后的 constraint、interaction、resource 和 connection 清理保持现有
  Runtime 不变量；
- 不公开裸 `&mut FigureTree`。

验证：

- layout replacement 与 constraint replacement；
- size override set/clear；
- ordinary child Z-order mutation；
- callback deferred mutation FIFO；
- 非法 parent/child、constraint type 和 index 的原子失败；
- Runtime 公共路径与构建期 FigureTreeBuilder 的稳定结果一致。

完成证据：

- ADR-009 固定 Runtime mutation、constraint 预验证、NodeState clipping override
  与 callback FIFO 契约；
- `RuntimeMutationError` 和 deferred error queue 已形成公开失败模型；
- `novadraw-scene/tests/d3_runtime_mutation.rs` 的 6 项契约测试通过；
- M8 的 24 项 viewport/zoom 测试复验通过；
- workspace、Clippy、WASM `novadraw` 与 `web-validation` 门禁通过；
- 详细记录见
  `doc/verification/reviews/d3-runtime-mutation-2026-09-08.md`。

### D3.3 M7 listener Runtime 公共面

状态：`complete`

目标：

- Runtime 暴露 Figure、Coordinate、Ancestor、Property、Action、Layout 和 Update
  listener 注册入口；
- 所有注册入口返回 `ListenerId`；
- `remove_listener` 统一注销所有 listener 类型；
- 明确 callback 内注销策略：若不支持重入注销，则提供 deferred removal handle，
  不能发生借用冲突或静默失败。

验证：

- 每类 listener 经 Runtime 注册后可观察对应提交后事件；
- 注销后不再收到事件；
- 重复注销返回稳定结果；
- callback 请求注销时不破坏当前 effect 顺序；
- listener 不能在通知中观察未提交或部分提交状态。

完成证据：

- ADR-010 固定七类 Runtime listener、统一 `ListenerId` 与
  `ListenerDirective::{Keep, Remove}` self-removal 契约；
- Runtime 已公开 Figure、Coordinate、Ancestor、Property、Action、Layout 和
  Update listener 注册入口，全部返回 `ListenerId`；
- `remove_listener` 对全部类别统一生效，重复注销返回 `false`；
- `novadraw-scene/tests/d3_runtime_listener.rs` 的 4 项契约测试通过；
- 详细记录见
  `doc/verification/reviews/d3-runtime-listener-2026-09-08.md`。

### D3.4 D3 完成门禁

状态：`complete`

- D3.0-D3.3 全部完成；
- `cargo fmt --check && cargo check && cargo clippy -- -D warnings && cargo test` 通过；
- WASM `novadraw` 与 `web-validation` 构建通过；
- M9 恢复 `complete`，M7 的公共面证据写回 API coverage；
- Runtime 用户无需访问内部 FigureTree/UpdateManager 即可完成上述动态操作和订阅；
- 完成后才启动 M10.5 Tooltip 与 Accessibility 契约。

完成结果：

- D3.0-D3.3 全部完成；
- M9 已恢复 `complete`，M7 Runtime listener 公共面已写回覆盖账本；
- Rust workspace、Clippy、WASM `novadraw` 与 `web-validation` 门禁通过；
- D3 完成时已解除原审计阻塞；后续长期架构审计新增的 D4 现优先于 M10.5。

## 9. D4：长期架构正确性收口

状态：`complete`

D4 处理 2026-09-08 长期架构审计中已经复现、且会影响 Core 1.0 正确性或稳定
扩展面的 P1 问题。D4 不推倒 FigureTree/Runtime 主干，不恢复迭代渲染，也不提前
扩张完整 widget toolkit、ShortestPath 或通用 3D。

审计输入：
[`../verification/reviews/architecture-sustainability-review-2026-09-08.md`](../verification/reviews/architecture-sustainability-review-2026-09-08.md)。

### D4.0 审计证据校准

状态：`complete`

- 当前 HEAD 复跑独立 probe，A01-A07 输出与审计记录一致；
- A08 的递归渲染逐节点 ancestor style 扫描在校准时由源码确认，后由 D4.5 关闭；
- A09-A11 中不阻塞 Core 1.0 的模块拆分、完整输入协议与统一错误模型保留为后续
  architecture delta，不混入当前修复；
- D3.3/D3.4 已完成，不再作为本轮阻塞项。

### D4.1 派生状态收敛事务

状态：`complete`

正式契约：

- [`../design/architecture/derived-state-convergence.md`](../design/architecture/derived-state-convergence.md)
- [`../adr/adr-011-derived-state-convergence.md`](../adr/adr-011-derived-state-convergence.md)

目标：

- connection dirty 自动进入正常 frame 的 reroute，不要求应用显式 resolve；
- 统一直接 mutation、callback 和 LayoutOutput geometry change 的 connection 失效入口；
- 文本 natural measurement 先参与布局，最终 client area 确定后再生成 constrained
  glyph snapshot；
- viewport/range 等共享容器状态只在 layout output 校验通过后提交；
- 使用静态类型化依赖层级、稳定去重 worklist 与 generation 增量收敛；预算只兜底
  未知反馈环，不引入运行时可注册的通用 DAG 或无界重复帧。

`api_semantics`：`layout.manager`、`validation.protocol`、
`update_manager.two_phase`、`connection.figure`、`connection.router`、
`text.flow`、`viewport.scroll_zoom`、`damage.repaint`。

完成证据：

- ADR-011 已通过；Runtime 的三个 frame recording 入口统一执行静态优先级
  `DerivedWorkSet`，稳定化失败时不生成 submission；
- Label cache 已拆分为 intrinsic metrics 与 presentation snapshot，首帧先完成 parent
  layout，再按最终 client area 生成 glyph/ellipsis；
- dirty Connection 已进入 `has_pending_update`，正常 frame 根据 connection parent
  child-content domain 自动排空 routing group；
- dependency generation reconciliation 覆盖 direct mutation、callback、LayoutOutput
  geometry commit 与 routing-domain child order；
- Viewport layout 只计算 sealed `ViewportLayoutEffect`，完整 LayoutOutput 校验通过后
  才提交 RangeModel 与 content scale；
- 新增首帧文本、自动 route、LayoutOutput route invalidation 和非法 Viewport output
  无 side effect 回归测试；
- `cargo fmt --all -- --check`、`cargo check --workspace`、
  `cargo clippy --workspace -- -D warnings`、`cargo test --workspace` 与 WASM
  `web-validation` check 通过。

详细记录：
[`../verification/reviews/d4-derived-state-convergence-2026-09-09.md`](../verification/reviews/d4-derived-state-convergence-2026-09-09.md)。

### D4.2 资源因果与 Backend Session

状态：`complete`

正式契约：

- [`../design/architecture/resource-lifecycle.md`](../design/architecture/resource-lifecycle.md)
- [`../adr/adr-012-resource-causality-and-backend-session.md`](../adr/adr-012-resource-causality-and-backend-session.md)

目标：

- ResourceDelta 保留同一 ResourceId 的真实操作顺序，或在提交前归约为可证明正确的
  最终状态；
- Ready -> Failed -> Ready 不得被 backend 解释为最终删除；
- 定义 backend/session epoch 与 ready resource snapshot，支持同一 Runtime 的 backend
  重建；
- retry/in-flight 与资源 revision 继续保持原子确认；
- 异步加载 request token、解码预算只定义契约，除非实现依赖已出现，不提前扩张。

`api_semantics`：`resource.lifecycle`、`render.backend_session`、
`frame.preparation`、`graphics.context`。

完成证据：

- ADR-012 已通过；`ResourceDelta` 使用单一有序
  `ResourceOp::{Upsert,Remove}` 序列；
- Ready -> Failed -> Ready 保持 Upsert -> Remove -> Upsert，Vello 严格按序更新
  image/font cache；
- `BackendSessionId` 使用 Runtime namespace + 单调 generation；跨 Runtime 切换替换
  cache，同 Runtime 旧 generation 被拒绝；
- 新 session 首帧使用 Ready `ResourceSnapshot`、完整 commands 与 Full damage；
  Snapshot 每次应用前替换 cache，空 Snapshot 也保留清空语义；
- delta Retry 恢复精确因果前缀，snapshot Retry 从 Registry 当前状态重新冻结；
- completion 同时匹配 session/frame，session reset 后旧 completion 不影响当前工作；
- Native editor 在 backend 重建时显式 reset session；普通 resize/full redraw 不重发
  Ready snapshot；
- `cargo run -p update-app -- --verify` 六项通过，Rust workspace、Clippy 与 WASM
  `web-validation` 门禁通过。

详细记录：
[`../verification/reviews/d4-resource-causality-and-backend-session-2026-09-09.md`](../verification/reviews/d4-resource-causality-and-backend-session-2026-09-09.md)。

### D4.3 Figure 生命周期与 Runtime 身份域

状态：`complete`

正式契约：

- [`../design/architecture/figure-lifecycle.md`](../design/architecture/figure-lifecycle.md)
- [`../adr/adr-014-extensibility-and-lifecycle-boundaries.md`](../adr/adr-014-extensibility-and-lifecycle-boundaries.md)

2026-09-10：首批身份隔离/dispose 已进入实现；同树 Runtime 重包装曾确认
registry/session 身份复活，现已通过“场景工厂交付完整 Runtime、移除 into_tree”
关闭该所有权门禁。证据与修正见
[`../verification/reviews/adr014-implementation-gate-2026-09-10.md`](../verification/reviews/adr014-implementation-gate-2026-09-10.md)。
FigureTree builder 已改为只构建拓扑，Runtime 接管后 parent-before-children 激活；
动态 add/reparent 在结构与 sidecar 提交后 completion，dispose 在提取与清理后
descendant-before-parent 停用。普通与 LayeredPane lifecycle panic 均进入
Runtime faulted 边界；10,000 层最大深度 dispose 回归通过。
不能按旧 ADR-013 恢复编码。
原 D4.1/D4.2 complete 是原批次记录，不覆盖 ADR-014 的新增验证。

目标：

- 闭合 reparent、dispose_subtree 和 contents replacement；撤回通用活对象
  detach/reattach 自动迁移承诺，未支持能力明确拒绝；
- dispose 原子清理 tree slot、UUID、constraint、interaction、resource、connection、
  layer lookup 和缓存；
- 销毁前捕获旧 visual envelope，保证 retained surface 擦除；
- 公开 Figure handle 携带 Runtime 身份域，拒绝跨 Runtime 的偶然同 key 操作；
- editor undo/history 保存模型操作，跨 Runtime 通过显式描述/工厂重建；
- 清理按归属区分 Figure-scoped 与 Runtime-scoped listener、共享 Router/资源；
- 结构提交、必要 lifecycle、稳定发布和析构分层；扩展 panic 后默认 faulted；
- public/local ID、scope 与生命周期上下文接口评审通过后，再分步实现。

`api_semantics`：`figure.tree`、`figure.lifecycle`、`runtime.identity`、
`damage.repaint`。

### D4.4 外部替换与错误边界

状态：`complete`

2026-09-10 已完成：

- topology `try_*` API 保留 namespace、disposed、synthetic root、parent relation
  与 GraphMutationError 分类；
- `TextLayoutParts` / `TextLayout::from_parts` 支持非 Parley 引擎构造非空 IR；
- `FramePreparation` 区分 Ready、Idle、Suspended、AwaitingCompletion 与 Error；
- command capability 在最终录制后校验，永久 unsupported 与 Retry 已结构化区分；
- Vello/Web 使用统一 `BackendSessionGate`，首次或接管 Delta 因缺 Snapshot 被拒绝；
- DemoApp 验证跨 Runtime 串行切换与 Snapshot 基线；
- 既有 Manhattan group failure 契约测试已满足完整 unresolved batch 门禁。
- 外部 Figure 的 owned typed component update、私有派生快照、revision 与统一失效
  已通过独立集成测试，不增加 Runtime 具体 Figure 类型分支；
- 扩展 mutation panic 统一进入 Runtime faulted 边界，各公共错误域结构化拒绝后续写入；
- `FigureMeasurement` 将父级约束与 baseline 传入 LayoutSnapshot，外部 Figure/Layout
  已验证受约束测量、arrange 和同约束 Glyph IR；
- `NotificationRecord` 提供 source epoch 与全局 sequence，`StableSceneQuery` 明确只读
  最新稳定场景，未发布工作返回 NotStable。

验证记录：
[`../verification/reviews/adr014-d4.4-increment-2026-09-10.md`](../verification/reviews/adr014-d4.4-increment-2026-09-10.md)。

目标：

- 提供受校验的 `TextLayout` builder/from-parts，使外部 TextLayoutEngine 能构造
  非空后端中立结果；
- 用独立外部消费者验证非 Parley engine，而不是返回 Default 的占位实现；
- 明确 RenderBackend 必需 command/capability 与不可恢复错误，不把永久 unsupported
  伪装为 Retry；
- 收敛 frame preparation 的 Idle/Suspended/AwaitingCompletion/Error 可区分结果。
- 用外部自定义 Figure 验证 owned typed update、私有派生快照与统一失效，
  不增加 Runtime 具体类型分支；首批接口与验收已实现，契约见
  [`../design/architecture/component-update.md`](../design/architecture/component-update.md)，
  后续扩展继续遵守其 prepared/commit 与 faulted 边界；
- 验证受宽度约束的测量高度/baseline、arrange 与同约束 Glyph IR；
- 验证历史事件 revision/sequence 与最新 stable query 不混淆；
- 验证 route group 完整 unresolved 失败批次，不保留旧 reservation；
- 验证 Host 串行 session handoff 和 Snapshot 基线，拒绝无基线 Delta。

`api_semantics`：`text.flow`、`graphics.context`、`frame.preparation`、
`render.backend_session`、`accessibility.bridge`、`figure.properties`、
`layout.manager`、`notification.layout_update`、`connection.router`。

### D4.5 递归主线性能恢复

状态：`complete`

2026-09-10 已按 Draw2D local style + Graphics state inheritance 恢复线性样式传播，
validation 同步移除逐节点 ancestor visibility 扫描。1k/10k release 基准、
命令/结果计数等价和默认 debug 测试线程 10,000 层全链路均通过。证据见
[`../verification/reviews/adr014-d4.5-performance-2026-09-10.md`](../verification/reviews/adr014-d4.5-performance-2026-09-10.md)。

目标：

- 对标 Draw2D local style + Graphics state inheritance，移除每节点 ancestor style 扫描；
- 保持递归主线、self/children/border 顺序和兄弟状态隔离；
- 补 1k/10k deep tree Runtime 基准与完整 render/hit/validate 路径；
- 性能修改前后提供同机、同工具链、同 command count 的统计证据。

`api_semantics`：`paint.protocol`、`figure.properties`、
`update_manager.two_phase`。

### D4.6 D4 完成门禁

状态：`complete`

2026-09-10 A01-A08 关闭矩阵、10,000 层全链路、Rust/WASM/Headless/Native 门禁已通过。
完整记录见
[`../verification/reviews/adr014-d4.6-completion-2026-09-10.md`](../verification/reviews/adr014-d4.6-completion-2026-09-10.md)。

- A01-A08 均完成、被明确降级或由新的可验证契约替代；
- 正常 Runtime frame 不依赖应用手工 reroute 或第二次 full redraw；
- 资源重建、跨 Runtime ID、dispose 和外部 text engine 均有公共 API 测试；
- 深树性能恢复到新基线并通过 10,000 层全链路验证；
- Rust、WASM、Headless 和既有 Native/Web 场景门禁通过；
- 完成后进入 M10.5 Tooltip 与 Accessibility。

## 10. Draw2D Core 1.0 完成门禁

D4 完成后的固定收口顺序：

1. [x] 完成 M10.5 Tooltip 与 Accessibility bridge；
2. [x] 完成 M1-M8 产品与人工验收差额；
3. [x] 补齐 M10 Text/Image/Widget 的 Web 等价场景；
4. [ ] 在 macOS、Web、Headless 重新执行自动、视觉与人工总审计；
5. [ ] 仅在以下门禁全部满足后声明 Draw2D Core 1.0。

- M1-M10 全部达到 `complete`；
- P0/P1 API family 不存在未解释的 `missing`；
- public API 不包含静默 no-op 或占位成功；
- ADR-014 新增契约均有当前证据，不以旧批次 complete 代替验证；
- macOS、Web、Headless 自动与人工门禁通过；
- 关键组合场景覆盖 layout、event、viewport、connection、text 和 resource；
- API 文档、语义账本、产品清单和 demo 矩阵一致；
- 完成 R9.4 capability 消融复查。

满足这些条件后，才启动独立的 GEF roadmap，包括 EditPart、Viewer、Tool、Request、
EditPolicy、Command、SelectionProvider 和 undo/redo command stack。

## 11. 延后能力

以下能力不阻塞 Draw2D Core 1.0：

- 富文本 Flow 与编辑；
- Animation/Animator；
- PrinterGraphics、ScaledGraphics 和打印工作流；
- DirectedGraphLayout、CompoundDirectedGraphLayout；
- ShortestPathConnectionRouter；
- XOR、pattern 和平台特定高级 Graphics 模式；
- 完整 widget toolkit；
- Windows/Linux 发布资格验证。
