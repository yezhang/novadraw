# Draw2D 核心能力差异审计

类型：`verification`

## 1. 审计信息

- 审计日期：2026-09-08
- Novadraw 基线：`7b3a033`
- Draw2D 参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`
- Draw2D 参考提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
- 审计范围：仓库定义的 Draw2D Core 1.0，即 M1-M10、P0/P1 API family 及其产品和验证门禁
- 明确排除：GEF 的 EditPart、Viewer、Tool、Request、EditPolicy、Command、
  SelectionProvider 和 undo/redo command stack
- 自动验证：`cargo test --workspace -q` 通过

本文记录 2026-09-08 时点的实现差异与证据，不定义新的架构。涉及取舍的项目必须先
更新对应设计或 ADR，再修改覆盖账本和实现状态。

## 2. 总体结论

Novadraw 已具备 Draw2D 核心运行时骨架，以下主链路已有实现和自动测试：

- Figure 树、盒模型、生命周期、Z-order 与精确 hit-test；
- Graphics 状态栈、递归 paint、client area 与 clipping；
- 坐标转换、事件点降域、capture、hover、focus 与键盘遍历；
- 六类布局、Validation -> Damage Repair、dirty region 与 retained submission；
- Viewport、ScrollPane、RangeModel、Zoom、Layer 与 Freeform；
- Connection、Anchor、Direct/Bendpoint/Manhattan/Fan Router、Locator 与 Decoration；
- reusable Shape、Border、Label、TitleBarBorder、ImageFigure 与基础控件。

当前结论仍是：

> 核心执行主链路已形成，但公共运行时能力、M9 部分正式契约、M10.5 和跨平台完成
> 证据尚未闭环，因此不能标记为 Draw2D Core 1.0 完成。

## 3. 阻塞差异

### 3.1 Runtime 未完整暴露动态布局与树操作事务

Draw2D 允许运行期修改 layout manager、constraint、preferred/minimum/maximum size
和 child index。Novadraw 的 `FigureTree` 已实现这些能力，但 `Runtime` 只公开只读
`tree()`，没有对应的 update-aware mutation API：

- `FigureTree::{set_block_layout_manager,set_constraint}` 已存在；
- `FigureTree::{set_preferred_size,set_minimum_size,set_maximum_size}` 已存在；
- `FigureTreeBuilder::{move_child_to_index,bring_child_to_front,send_child_to_back}` 已存在；
- `Runtime` 仅覆盖 add/remove/reparent、bounds、style 和少数具体 Figure mutation。

影响：

- Runtime 建立后，应用无法通过规范事务边界动态替换布局、修改约束或调整普通 child
  的 Z-order；
- 直接把 `FigureTree` API 记为 `layout.manager` 和 `figure.tree` 已验证，会掩盖产品
  公共面的不可达性；
- 若开放 `&mut FigureTree` 又会绕过 UpdateManager、资源、Connection 和交互状态清理，
  因此不能用裸可变访问器修补。

建议：

- 在 Runtime 增加 typed、update-aware 的 layout、constraint、size 与 child-order
  mutation；
- callback 内需要同类操作时，通过 `PendingMutation` 排队；
- 为运行期 constraint replacement、layout replacement 和 Z-order mutation 增加事务
  契约测试。

### 3.2 M7 listener 实现没有形成完整 Runtime 公共面

`UpdateManager` 已实现 Figure、Coordinate、Ancestor、Property、Action、Layout 和
Update listener 注册与统一移除，但 Runtime 当前只公开：

- `add_update_listener`；
- `add_property_listener`；
- `add_action_listener`；
- `remove_listener`。

其中 `add_update_listener` 未返回 `ListenerId`，调用方无法使用现有
`remove_listener` 注销；Figure、Coordinate、Ancestor 和 Layout listener 没有 Runtime
注册入口。

影响：

- M7 内部 effect queue 和分发行为已经验证，但对 Runtime 用户并非完整可用能力；
- `notification.figure`、`notification.coordinate`、`notification.ancestor` 和
  `notification.layout_update` 不宜继续无条件标记为 public API `verified`。

建议：

- Runtime 对所有已支持 listener 提供成对的注册与注销入口；
- 所有注册入口返回 `ListenerId`；
- 增加 Runtime 级注册、触发、注销和 listener 自注销测试。

### 3.3 M10.5 Tooltip 与 Accessibility 尚未实现

当前 Tooltip 只有 `Runtime::tooltip()`，其语义是根据 `hover_source` 立即查询继承后的
字符串。尚无：

- hover delay；
- show/hide 状态机和超时；
- tooltip placement、viewport/surface 边界处理；
- PlatformHost tooltip effect；
- Tooltip demo。

Accessibility 当前只有：

- `AccessibleFigure::accessible_name()`；
- `PlatformHost::update_accessibility(AccessibilityUpdate)`；
- 只携带 `revision` 的 `AccessibilityUpdate`。

尚无 Runtime 从 Figure 树生成 accessibility snapshot/delta 的路径，也没有 role、
state、bounds、focus、default action 和 child hierarchy 等平台可消费语义。

建议：

- 先为 M10.5 建立独立设计契约，明确 tooltip timer 归 host event loop 还是 Runtime
  scheduler；
- Accessibility 最小产品面至少包含稳定 node id、name、role、state、bounds、children、
  focus owner 和 action；
- Native/Web/Headless 共用同一引擎 snapshot，平台层只做桥接。

### 3.4 M9 完成状态与已接受契约不一致

`connection-routing.md` 和 ADR-005 把 nested viewport clipping、unsupported topology
以及 shared Manhattan reservation 纳入已接受契约和验证门禁；当前实现和执行记录则
明确保留以下缺口：

- `ManhattanConnectionRouter` 是不避障的单连接正交 Router；
- shared Manhattan obstacle reservation 未实现；
- nested cross-viewport connection clip 未实现；
- `RouteError::UnsupportedViewportTopology` 只有枚举与显示文本，没有实际产生路径。

这不一定要求立即实现 ShortestPath Router，但必须在以下两种处理之间选择：

1. 按已接受设计补齐 shared reservation 和 nested viewport policy；或
2. 修改设计/ADR，将其明确降级为 Core 1.0 之后的增强，再保留 M9 `complete`。

在完成该决策前，M9 的“正式契约全部完成”证据不成立。

## 4. 非阻塞但需记录的差异

### 4.1 ClippingStrategy 扩展性较 Draw2D 收窄

Draw2D `IClippingStrategy` 可针对 child 返回任意多个裁剪矩形。Novadraw 当前使用固定
枚举：

- `ClipToChildBounds`
- `DoNotClipChildBounds`
- `OverflowVisible`

并主要通过具体 Figure builder 在构造期设置。现有产品场景足够，但这不是可替换策略
的完整语义。若未来需要局部洞口、多矩形裁剪或 viewport-aware connection clipping，
应提升为只读策略对象或受控 clip provider，并提供 Runtime 替换事务。

### 4.2 Graphics 是经过裁剪的核心子集

当前缺失或明确延后的 Draw2D Graphics 能力包括：

- `getClip` 与 path clip；
- `drawRoundRectangle/fillRoundRectangle` convenience primitive；
- gradient；
- 自定义 line dash、miter limit；
- XOR 与平台特定 antialias mode；
- image source rectangle；
- focus drawing convenience API。

已有 path、glyph IR 和 resource-referenced image 足以支撑当前 Figure，不应为方法名
对齐机械增加 API。Core 1.0 前需要逐项写明“由现有 primitive 等价表达”或“明确延后”，
避免 `graphics.context` 长期保持无 owner 的 `partial/missing`。

### 4.3 明确延后的 Draw2D 能力

以下差异已有合理边界，不视为本次阻塞缺陷：

- TextFlow、FlowFigure、ParagraphTextLayout 与完整 bidi/fragment API；
- ShortestPathConnectionRouter；
- Animation、Animator 与 routing/layout animation；
- PrinterGraphics、ScaledGraphics 和打印工作流；
- DirectedGraphLayout、CompoundDirectedGraphLayout；
- ButtonGroup、radio/checkbox、repeat firing、Slider 等完整 widget toolkit；
- GEF 编辑器层能力。

## 5. 覆盖账本不一致

`doc/parity/draw2d/api-coverage.md` 存在以下陈旧记录：

- 仍称 remove/reparent/getParent 缺少 public API，但 Runtime 已公开
  `remove_figure/reparent`，FigureTree 已公开 `parent_id`；
- 仍称 `Border.getPreferredSize/isOpaque` 缺失，但 `Border` 已提供
  `preferred_size/is_opaque`；
- 文本行仍引用已删除的 raw-string `draw_string/draw_text/fill_text/stroke_text`，
  未记录当前 `draw_text_layout/fill_text_layout/stroke_text_layout`；
- concrete border 行未同步 Compound、Etched、Bevel 和 TitleBar；
- accessibility 在覆盖账本中写为“不进当前核心门禁”，但产品清单和 M10.5 明确将
  Accessible bridge 纳入 M10；
- M7 listener 状态没有区分 UpdateManager 内部可用与 Runtime 外部可达；
- M9 `verified/complete` 没有反映正式设计中的 nested viewport 和 shared reservation
  缺口。

覆盖账本应先按本审计重新校准，再用于 Core 1.0 完成判断。

## 6. 验证与发布证据差异

- `cargo test --workspace -q` 在本次审计基线上通过；
- M8 在路线图中仍为 `behavior_verified`；
- M10 为 `in_progress`；
- M10.4 自动测试和截图已通过，但人工窗口验收尚未记录；
- Tooltip demo 尚未创建；
- M9 Connection 已进入共享 demo catalog；M10 Text/Image/Widget 的 Web 端等价场景仍不足；
- Core 1.0 要求 M1-M10 全部 `complete`，并通过 macOS、Web、Headless 自动与人工门禁。

因此，自动测试通过只能证明当前实现没有已知测试回归，不能替代产品面与跨平台完成
门禁。

## 7. 建议收口顺序

1. 校准 API 覆盖账本，先消除已经实现但仍标为 `missing/partial` 的陈旧项。
2. 决定 M9 nested viewport clipping 与 shared Manhattan reservation 的契约归属。
3. 补齐 Runtime layout/tree mutation 与完整 listener 公共面。
4. 完成 M10.5 Tooltip、Accessibility bridge 和对应 demo。
5. 完成 M8、M10.4 人工验收及 M10 Web 验证。
6. 按 Core 1.0 门禁重新执行文档、自动测试、视觉和人工验收总审计。
