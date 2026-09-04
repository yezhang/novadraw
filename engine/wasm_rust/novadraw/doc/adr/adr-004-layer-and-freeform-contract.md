# ADR-004: Layer 与 Freeform 范围契约

类型：`architecture-decision`

## 状态

已通过

## 背景

M9 ConnectionLayer 和大型编辑画布需要稳定的 Layer、Freeform、负坐标范围与
Viewport/Zoom 组合语义。Draw2D 通过 Java 继承、Figure 内部 listener 链和
`setFreeformBounds()` 递归同步容器 bounds 实现这些能力。

Novadraw 已由 ADR-003 确立 parent-local bounds、Runtime 事务入口、FigureTree
拓扑职责和统一 Affine2D 坐标链。直接复制 Draw2D 的对象布局会重新引入多重几何真源、
回调重入和 bounds 批量改写。

## 决策

1. Freeform extent 从后代几何派生；nested freeform 传播 extent/envelope，但不递归
   改写普通 child 或 freeform host 的 NodeState.bounds。
2. LayerKey 在单个 LayeredPane 内唯一；重复 key 返回结构化错误。
3. FreeformLayout 保留正负坐标，不提供运行时 positive-coordinates 模式；非负归一化
   位于导入、导出或算法适配边界。
4. LayeredPaneState 是 Runtime 私有关系状态，不属于 LayoutConstraint；FigureTree
   children 顺序仍是唯一 Z-order。
5. FreeformState 是 LayoutState 的派生缓存；具体 Figure 不持有 bounds、child IDs
   或 listener 对象链。
6. Freeform 使用 ChildClippingStrategy::OverflowVisible；paint、hit-test 和 damage
   共享同一有效 clip，最终由 ancestor 或 Viewport 截断。
7. RangeModel 的 minimum、maximum、extent 和 value 统一使用 content domain；
   ScalablePane 是 scale 的唯一真源。
8. ScalableFreeformLayeredPane 在同一节点组合 LayeredPane、Freeform 和
   ScalableFigure capability，并复用现有 scale state。
9. 公开 topology 写入统一经过 Runtime；FigureTree 仅公开只读查询，底层 mutation
   primitive 限于 crate 内事务实现。

完整语义、Draw2D 差异及逐项收益/代价见
`doc/design/architecture/layer-and-freeform.md`。

## 后果

### 正面

- bounds、extent、scale 和 range 各自只有一个真源；
- 负坐标不会因新增 child 被隐式归一化或批量移动；
- Layer membership、topology、extent、damage 和通知可以原子提交；
- paint、hit-test、event point 和 damage 共享坐标与裁剪协议；
- nested freeform extent 可按 generation 自底向上线性重算；
- ConnectionLayer 可复用稳定 layer key 和 freeform canvas，而不污染 FigureTree。

### 负面

- 不兼容依赖 `setFreeformBounds()` 或 `freeform.getBounds() == extent` 的调用方；
- 不支持重复 LayerKey；
- D2 不提供 Draw2D 的运行时 positive-coordinates 模式；
- RangeModel 在 scale 不为 1 时与 Draw2D 的公开数值可能不同；
- Runtime 成为 topology 写入的强制入口；
- OverflowVisible 和 extent 收敛增加 UpdateManager 与命中协议复杂度。

## 不采用的方案

### 递归同步 freeform bounds

不采用。它会让 bounds 同时表达布局位置和内容 envelope，并在 parent-local 模型下
产生双重平移与批量几何改写。

### LayerKey 复用 LayoutConstraint

不采用。Layer 身份不应随 LayoutManager 替换而丢失。

### 外层 ScalablePane 隐式查找后代 extent

不采用。Viewport 只查询 direct contents capability，隐式向下搜索会形成隐藏协议和
第二条范围解析路径。

### 允许直接修改 FigureTree

不采用。LayeredPaneState、FreeformState、interaction cleanup 和 damage 无法在绕过
Runtime 时保持原子一致。

## 参考

- `doc/adr/adr-003-rust-runtime-and-geometry-boundaries.md`
- `doc/design/architecture/layer-and-freeform.md`
- `doc/design/architecture/tree-search-and-focus.md`
- `doc/design/coordinates/coordinate-system.md`
- `doc/design/architecture/static-architecture.md`
- `doc/design/rendering/update-manager.md`
- Eclipse GEF Classic commit `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 日期

2026-09-04
