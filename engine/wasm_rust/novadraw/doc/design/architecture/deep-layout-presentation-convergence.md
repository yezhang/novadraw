# Deep Layout and Presentation Convergence

类型：`normative-design`

状态：`accepted / implemented`

适用增量：`P2-S05`

关联规范：

- [ADR-011：派生状态收敛](../../adr/adr-011-derived-state-convergence.md)
- [Runtime 派生状态收敛事务](derived-state-convergence.md)
- [Reusable Shape 与 Border](reusable-shape-border.md)
- [UML 扩展能力验证 F-01](../../verification/reviews/uml-extension-capability-2026-10-05.md#f-01深层复合布局与文字-presentation-未稳定收敛)

## 1. 问题与目标

P2-S05 处理布局容器自身带 insets 时的深层复合布局错误。当前 `LayoutManager`
聚合的是 child content measurement，`FigureNode` 却直接把结果暴露给父布局。父布局
因此把带 insets 的子容器安排得过小；子容器的 client area 再扣一次 insets 后会裁掉
后续 Label。Label bounds 和 glyph command 可以都存在，但 Native/Vello 的有效 clip
仍只覆盖部分文字。

目标是冻结以下契约：

1. 任意深度的容器 measurement 都明确区分 content box 与 outer box；
2. insets、约束和 scalable projection 使用固定顺序；
3. 一个 `prepare_submission` 在晋升 stable epoch 前完成布局、Label presentation、
   clip/transform 和 damage，应用不追加 refresh 或第二次 full redraw；
4. 内置与外部 `FigureContainer`、`LayoutManager` 使用同一投影路径。

## 2. 尺寸域

布局测量使用四个显式语义域：

| 域 | 含义 |
|---|---|
| parent outer constraint | 父布局分配给当前 Figure outer box 的约束 |
| unprojected content | 当前容器 child-content domain 中、尚未包含 NodeState.insets 的尺寸 |
| unprojected outer | content 加 NodeState.insets 后、尚未经过容器 scale 等投影的尺寸 |
| parent projected outer | 父布局最终观察到的 outer measurement |

`LayoutManager::{preferred_measurement,minimum_size}` 返回 `unprojected content`。
`FigureContainer::{project_preferred_measurement,project_minimum_size}` 接收
`unprojected outer`，返回 `parent projected outer`。

`FigureTree::preferred_measurement` 和 `minimum_size` 是盒模型组合的唯一权威入口；
具体 LayoutManager 不重复读取或添加容器 insets。

## 3. 规范投影顺序

设 `I` 为 `NodeState.insets`，`T^-1` 为容器的 constraint 反投影，
`T` 为 measurement 正投影。

### 3.1 约束

```text
layout_constraints =
    T^-1(shrink(parent_outer_constraints, I))
```

每个有界维度先减对应方向的 insets，结果下限为零；无界维度保持无界。扣除 insets
必须先于 scale 等容器反投影。

### 3.2 Layout measurement

```text
content = layout.measure(layout_constraints)
unprojected_outer = expand(content, I)
parent_measurement = T(unprojected_outer)
```

preferred baseline 从 content domain 平移到 unprojected outer domain：

```text
outer_baseline = content_baseline + I.top
```

随后由容器投影 baseline。minimum size 没有 baseline，但使用相同的尺寸顺序。

### 3.3 显式 override

`set_preferred_size` 和 `set_minimum_size` 的值是 `unprojected outer` override。它们
跳过 LayoutManager 和 insets expansion，但仍经过容器正投影：

```text
parent_explicit_size = T(explicit_unprojected_outer)
```

这保持现有 scalable 显式尺寸契约，也避免对调用方已经给出的 outer size 重复加
insets。

### 3.4 非 Layout Figure

没有 LayoutManager 的 Figure 继续使用其 intrinsic/prepared measurement 与
owner-scoped Border 组合规则。P2-S05 不改变 Figure 自身 intrinsic measurement 是否
包含构造期 padding，也不重复叠加 Border snapshot insets。

## 4. Scalable 组合示例

若 outer insets 为左右各 10，scale 为 2，父级宽度约束为 100：

```text
content constraint = (100 - 20) / 2 = 40
content measurement = 30
unprojected outer = 30 + 20 = 50
parent projected outer = (50 - 20) * 2 + 20 = 80
```

先 scale 再扣 insets 会得到 30 而非 40 的错误 content constraint；先投影 content
再加 insets则会把 insets 一并缩放。两种顺序都不符合 NodeState 盒模型。

## 5. 稳定帧契约

一次 source mutation 到一个成功 `RenderSubmission` 的因果顺序保持：

```text
intrinsic metrics
-> parent measurement
-> outer bounds commit
-> descendant layout
-> Label presentation using final client bounds
-> final clip/transform recording
-> damage publication
```

同一个 submission 中必须同时满足：

- 父布局观察到包含 insets 的最终 outer preferred/minimum size；
- 后代 Label bounds 位于最终 client area；
- Label presentation 使用这些最终 bounds；
- RenderCommand 中的 container client clip 能覆盖全部后代 bounds；
- glyph commands 与最终 geometry 属于同一 stable epoch；
- damage 覆盖 geometry/presentation 的 old/new visual envelope。

不得通过应用层 `refresh_label_layouts`、重复 `prepare_submission` 或
`request_full_redraw` 修正首帧。

## 6. Draw2D 对应与 Novadraw 变体

Draw2D `Figure.validate()` 在父 layout 后递归验证 children。
`ToolbarLayout.calculatePreferredSize()` 先按扣除 insets 的 hint 聚合 children，最后
`expand(container.getInsets())`，因此父布局接收 outer size。

Novadraw 保留该可观察语义，但把 insets 组合集中在 `FigureNode` measurement 边界：

- Rust `LayoutManager` 只处理 child-content domain；
- `NodeState` 是 attached Figure insets 的权威所有者；
- container projection 是独立扩展点，必须与 insets 以固定顺序组合；
- Runtime 仍统一提交布局与 presentation，不让 Figure 或 LayoutManager 访问
  UpdateManager。

这是所有 LayoutManager 共用的盒模型，不是 ToolbarLayout 特例。

## 7. 扩展与失败语义

外部消费者可以同时实现自定义 `FigureContainer` 和 `LayoutManager`，不修改 Core 即可
获得相同的 constraint/insets/projection 顺序。Core 不按具体容器或布局类型分支。

P2-S05 不新增公开错误类型：

- 合法有界约束扣除 insets 后饱和到零；
- 现有非有限 geometry、LayoutOutput 原子校验和非收敛错误保持不变；
- 失败不出版 partial frame，不 flush 未稳定通知；
- 渲染热路径不输出诊断日志。

## 8. 非目标

- 不修改递归渲染主循环；
- 不增加通用依赖 DAG 或额外稳定化扫描；
- 不改变 explicit preferred/minimum override 的 outer-size 语义；
- 不实现 graph auto-layout、fragment style 或新的文本 shaping 算法；
- 不把 UML 样例专用逻辑下沉 Core。

## 9. 完成判据

1. 独立契约覆盖 constraint shrink、insets expansion、baseline 和 container projection
   顺序。
2. 两层以上 Toolbar/Layout + inset container + Label 最小场景在一次 submission 中
   验证最终 preferred/minimum、bounds、client clip 与 glyph commands。
3. UML compartment 恢复 ToolbarLayout，不包含 refresh workaround。
4. `core.p2-s05-deep-layout-presentation` 与 `example.uml-extension` 通过。
5. Native/Vello 截图像素断言确认每个 compartment 的全部成员可见。
6. geometry/presentation 变化的 partial damage 覆盖 old/new visual envelope。
7. 定向门禁、quick gate 和最终 full gate 通过，并以 completion record 关闭 F-01。
