# Draw2D / GEF 核心语义后续审计

类型：`verification`

状态：`in-progress`

日期：2026-09-20

## 审计边界

- 范围：`novadraw-core`、`novadraw-geometry`、`novadraw-render`、
  `novadraw-scene`、`novadraw-editor` 的 96 个 Rust 源文件，约 53,762 行；
- 语义基线：
  [`Draw2D API 覆盖账本`](../../parity/draw2d/api-coverage.md)、
  [`GEF API 覆盖账本`](../../parity/gef/api-coverage.md) 与当前设计文档；
- 参考源码仅限 `org.eclipse.draw2d` 和 `org.eclipse.gef`；
- `org.eclipse.zest` 不属于需求、语义或实现依据；
- 只把项目已声明为 `verified` / `partial`，但 Rust 实现未满足的行为计入缺陷；
  未承诺能力不作为缺陷。

本报告是
[`2026-09-16 语义审计整改`](draw2d-gef-semantic-remediation-2026-09-16.md)
完成 17/17 后的独立后续批次，不修改上一批次的历史结论。

## 结论

- 分组审查原始候选：25 条，其中 2 组为重复根因；
- 去重后候选：23 条；
- 提升为本批次优先整改项：5 条 P1；
- 已修复：2 条；
- 待修复：3 条；
- 其余 18 条保留为后续候选，需先补定向测试或重新拆分根因，暂不计入
  当前 P1 关闭分母。

## 优先整改账本

| ID | 发现 | 对等语义 | 状态 | 证据 |
|---|---|---|---|---|
| F01 | `NdCanvas::rotate` 将 Draw2D 的 degrees 当作 radians | `Graphics.rotate(float degrees)` | fixed | `rotate_uses_draw2d_degree_units`；`novadraw-render` 全量测试：PASS |
| F02 | Vello 静默丢弃公开的 Dash/Dot line style | `Graphics.setLineStyle` / SWT 内置 Dash、Dot | fixed | `line_styles_map_to_width_scaled_vello_dash_patterns`、`vello_scene_expands_dash_and_dot_into_multiple_path_segments`、Viewport guide 契约：PASS |
| F03 | `XYLayout` 自动轴测量丢失固定轴 hint | `XYLayout.layout` 将 constraint width/height 传入 `getPreferredSize` | open | `novadraw-scene/src/layout/xy_layout.rs::layout` 固定调用 `preferred_size(child, -1, -1)` |
| F04 | 失效 gesture target 导致同一 session 重定向 | scroll/zoom session 固定 target；失效后结束而非重新命中 | open | `InteractionState::reconcile_non_focus` 删除整个 session，后续 Update 进入 missing-Begin 恢复路径 |
| F05 | Policy 激活失败破坏 activate/deactivate 成对生命周期 | GEF EditPart/EditPolicy 激活与停用必须成对 | open | 创建 Part 时激活中途失败会停用未激活 Policy，Viewer Drop 再次停用 active Part 的全部 Policy |

### F01：rotate 角度单位

修复后 `NdCanvas::rotate` 明确接收 degrees，并在进入
`Transform::from_rotation` 前转换为 radians。测试以 `rotate(90.0)` 作用于单位向量
`(1, 0)`，要求结果为 `(0, 1)`。

### F02：Vello Dash / Dot

Vello 0.10 原生接受 `Stroke::with_dashes`，当前映射采用 SWT 内置样式的线宽比例：

```text
Dash = [3w, w]
Dot  = [w, w]
```

`StrokeRect`、`Line`、`Polyline`、`Ellipse` 与 `StrokePath` 统一通过同一
`vello_stroke` 构造器提交。Vello Scene 编码测试确认 Dash/Dot 会展开为多段路径，
不再退化为 Solid。

### F03：XYLayout 测量 hint

触发条件：

```text
constraint.width = 80
constraint.height = -1
```

当前实现以 `(-1, -1)` 请求 preferred size，宽度敏感 Figure 会按无限宽测量高度。
修复应把 constraint 的两个轴原样传入测量，只用返回值替换值为 `-1` 的自动轴。

### F04：gesture target 生命周期

当前错误链：

```text
Begin 固定 target A
-> A 被 remove / hide / disable
-> reconcile 删除整个 GestureState
-> 同 session 的 Update 被当作 missing Begin
-> 重新 hit-test 并投递 target B
```

目标语义：

```text
Begin 固定 target A
-> A 失效时保留 session tombstone，并可选发送 Cancel
-> 后续 Update 不投递
-> End / Cancel 清除 session
```

定向回归必须使用两个 Viewport，确认 A 失效后同一 session 不改变 B 的 scroll/zoom
状态；新 session 才允许命中 B。

### F05：Policy 激活回滚

当 Policy A 激活成功、Policy B 激活失败时，只允许逆序停用已成功激活的前缀 A。
当前实现会停用 A/B/后续未激活 Policy，并因 Part 已标记 active 而在 Viewer Drop 时
再次停用。修复需要统一创建回滚事务，保证 Behavior、Policy、Part active flag 和
注册资源严格成对。

## 后续候选

以下候选的静态触发路径成立，但未进入本批次前五。它们必须先建立定向回归，再决定
是否提升为正式整改项。

| 领域 | 候选 | 代码入口 | 后续要求 |
|---|---|---|---|
| Geometry / Hit-test | Rectangle right/bottom inclusive 与 Draw2D 不一致；原审计需拆分 standalone geometry 与默认 `precise_hit` 两个根因 | `novadraw-geometry/src/rect.rs`、Figure 默认命中 | 分别补相邻矩形和 Figure 共边测试 |
| Graphics | `NdCanvas::arc` 忽略 `anticlockwise` | `novadraw-render/src/context.rs::arc` | 补顺/逆时针与跨 0° 测试 |
| Vello | `PathOp::Arc` 在 FillPath/StrokePath lowering 中被跳过 | `novadraw-render/src/backend/vello/mod.rs` | 补 path IR 到 Vello Scene 编码测试 |
| Figure | 零面积 Triangle 可命中父容器任意位置 | `novadraw-scene/src/figure/triangle.rs::precise_hit` | 补零宽、零高与 inset 耗尽测试 |
| Border | `BorderBuilder` 默认 zero insets 绕过具体 Border 构造器不变量 | `novadraw-scene/src/figure/border/mod.rs` | 补 builder 与直接构造的盒模型等价测试 |
| Property | `set_opaque` 未发送 typed property event | `novadraw-scene/src/graph/mod.rs::set_opaque` | 补 false/true/no-op listener 测试 |
| Property | Tooltip 通知把继承与显式关闭折叠成同一值 | `FigureStyle::tooltip` property emission | 补 `None <-> Some(None)` old/new payload 测试 |
| Figure | `Polygon::add_point` 对两点退化多边形加入 stroke envelope | `novadraw-scene/src/figure/polygon.rs` | 覆盖 0/1/2/3 点 bounds 与 paint |
| Layout | BorderLayout 未维护每个 region 的唯一 child | `novadraw-scene/src/layout/border_layout.rs` | 补重复 North/Center 与移除测试 |
| Layout | GridLayout equal columns 被单列 grab 拆成不等宽 | `novadraw-scene/src/layout/grid_layout.rs` | 补两列、单列 grab 测试 |
| Layout | ToolbarLayout 非 stretch 分支可低于 child minimum | `novadraw-scene/src/layout/toolbar_layout.rs` | 补过小容器的横纵测试 |
| Update | Full frame 绕过或错报 Painting/Painted | `Runtime::prepare_submission_state_inner` | 补 full redraw、resize、partial promotion 通知测试 |
| Tooltip | 同 source 内移动不断延后自动隐藏 | `novadraw-scene/src/runtime/tooltip.rs` | 补移动期间仍按初始 hide deadline 隐藏 |
| Accessibility | traversable-only 节点声明 focusable，但 Focus action 失败 | accessibility snapshot/action | 统一快照能力与 action 资格 |
| Widget | 挂入 disabled parent 的 Clickable 保留 enabled 视觉 | attachment/reparent + clickable snapshot | 补 disabled parent attach/reparent 测试 |
| Auto-expose | feedback 重建后沿用旧 range 的停止判定 | `novadraw-editor/src/domain.rs::autoexpose_tick` | 补 feedback 扩展 range 的连续 tick 测试 |
| Selection | 指针选择返回的 `SelectionDelta` 遗漏同次 focus 变化 | `GraphicalViewer::dispatch_mouse_pressed` | 补首次点击与 focus-only 变化测试 |
| Tool | Viewer refresh 注销 source 后 active Tool 持有失效 `EditPartId` | Viewer refresh / EditorDomain Tool | 四类 gesture 分别补 source retirement 测试 |

## 验证状态

截至 2026-09-20，F01/F02 已通过：

```text
cargo test -p novadraw-render --features vello
cargo test -p node-editor-demo viewport_guide_uses_light_background_and_dashed_outline
cargo check -p novadraw-render --features vello
cargo xtask check --quick
cargo xtask docs
```

`cargo xtask check --full` 不在本次窄修复边界重复执行；最终提交、推送、合并或本批次
全部关闭时再运行一次。

