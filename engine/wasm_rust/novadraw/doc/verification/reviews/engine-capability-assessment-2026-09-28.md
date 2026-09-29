# 引擎能力与 API 稳定化评估

类型：`verification`

状态：`complete`

日期：2026-09-28

实施更新：E1 与 E2 已由
[ADR-020](../../adr/adr-020-engine-value-and-render-contract.md) 完成并验证；E3 的聚合
facade 与 backend feature 边界已由
[ADR-021](../../adr/adr-021-public-facade-and-feature-boundary.md) 收口。本文第 3.3 与
第 4.1-4.3 节保留整改前证据，后续工作从 Figure capability 与内部模块拆分继续。

## 1. 范围

本评估只覆盖可复用图形引擎与 Editor framework：

- `novadraw-core`
- `novadraw-math`
- `novadraw-geometry`
- `novadraw-render`
- `novadraw-scene`
- `novadraw-editor`
- `novadraw-inspector`
- 聚合入口 `novadraw`

产品 document schema、serializer、业务模型、产品 UI、Native/Web 产品壳和发布流程属于
独立产品包，不作为本仓库引擎完成度或后续增强的门禁。引擎只保证外部模型重新注入后，
Runtime、EditPart 与 Figure 投影可以按公开契约确定性重建。

## 2. 当前结论

- Draw2D Core M1-M10 已完成；
- Editor framework G0-G5 已完成，检查点 A、B、C 均已通过；
- ADR-017、ADR-018 与 ADR-019 已完成构建期、查询期、挂载期和驱动期 API 分层；
- Draw2D/GEF 两批共 22 条 P1 语义问题已经关闭；
- 当前主线应从功能里程碑转为引擎 API 稳定化和定向能力增强。

当前实现的主要优势是：

1. arena、代际 ID 与 Runtime namespace 提供稳定身份隔离；
2. Runtime 是挂载后 source mutation、派生状态、通知和 damage 的唯一提交权威；
3. FigureTreeBuilder、scoped editor、只读查询和 Runtime drive 已按生命周期分层；
4. Figure、LayoutManager、Router、Anchor、RenderBackend 和 typed component update
   已形成可替换扩展边界；
5. Layout、事件、Viewport、Connection、文本、资源和 Editor 编辑闭环均已有自动证据。

## 3. 模块边界评估

### 3.1 当前 crate 拆分总体合理

`geometry -> render protocol -> scene runtime -> editor` 的依赖方向成立；
`novadraw-editor` 与 `novadraw-inspector` 独立于产品应用也是正确边界。

现阶段不建议继续拆分 `novadraw-scene` crate。FigureTree、Runtime、UpdateManager、
InteractionState 和 ConnectionRuntime 共享高内聚事务状态，提前拆 crate 会制造大量
facade 转发和内部协议公开化。

### 3.2 应先执行内部模块拆分

当前高集中度文件包括：

- `novadraw-scene/src/graph/mod.rs`：约 6,400 行；
- `novadraw-scene/src/runtime/runtime.rs`：约 6,100 行；
- `novadraw-editor/src/viewer/mod.rs`：约 2,800 行。

建议保持所有权不变，只按职责拆分内部模块：

- graph：topology、query、layout、mutation、presentation；
- runtime：editors、frame、connections、listeners、resources；
- viewer：containment projection、connection projection、targeting、refresh transaction。

### 3.3 当前 crate 中的弱边界

- `novadraw-math` 只包含未被引擎消费的 `Mat3` 和 `Vec3`，但被多个 crate 声明依赖；
- `novadraw-core` 当前基本只承载 `Color`，名称与稳定职责尚未匹配；
- 聚合 crate `novadraw` 重导出 `FigureNode`、`NodeState`、`UpdateManager`、
  `PendingMutations` 和 raw `RenderCommand` 等低层协议；
- `novadraw-scene` 与 `novadraw-render` 对大量公共类型关闭了 `missing_docs` 门禁。

## 4. API 风险

### 4.1 公开渲染契约存在可构造但不执行的状态（已整改）

- 审计时 `PathOp::Arc` 可以公开构造，但 Vello lowering 忽略该操作；
- 审计时 `NdCanvas::arc` 忽略 `anticlockwise`，并固定使用八段折线；
- 审计时 `RenderCommandKind::Path` 为公开 variant，但没有实际 producer 或 Vello consumer；
- 审计时 `Image.src_rect` 可在公开 command 中表达，但 Vello 遇到非空 source rectangle 时
  静默跳过绘制；
- backend capability 只区分 Glyph、Image 和 Projective Composition，不能阻止上述
  子能力静默降级。

公共 Render IR 必须满足二选一：所有启用后端完整消费，或在提交前返回明确的
unsupported capability。不得依赖 wildcard match 静默忽略。

Arc、冗余 Path command 与当时未实现的 image source rect 已按 ADR-020 收口；
2026-09-29 的 `core.image-source-rectangle` 后续切片已恢复完整 source rectangle
协议、结构化输入失败和 Vello destination clip，以上静默降级风险均已关闭。

### 4.2 Graphics 表面存在两套方言

`fill_rect/stroke_rect` 使用显式 paint 参数，
`fill_rectangle/draw_rectangle` 使用 Graphics state；颜色、线宽、线型和 alpha
也有重复命名。应保留一套规范的 stateful Graphics API，并将显式 recorder primitive
改为明确命名的低层接口或限制可见性。

### 4.3 Geometry 与基础值仍有迁移期表面

- `Point` 是 `Vec2` alias，类型系统不能区分位置与向量；
- `Vec2` 的 tuple 字段公开，RenderCommand 直接暴露 `glam::DVec2`；
- `Transform`、`Affine2D`、`AffineTransform` 同时公开；
- `Dimension`、`Size` 与 `Precision*` alias 没有新增语义；
- `Insets` 已存在，但 Figure、Border 和 size override 仍使用 tuple；
- `Color` 字段公开，构造器不能兑现 `[0, 1]` 不变量，`hex` 对短输入可能 panic。

### 4.4 Figure capability 仍有闭集倾向

`Figure` 基础 trait 当前知道 `connection`、`point_list`、`bordered`、`label` 和
`clickable` 等具体能力；`FigureEditor` 同时承载通用节点与多种内置 Figure mutation。
这会让新增内置能力继续修改基础 trait 或扩大通用 editor。

目标应是：

- `FigureEditor` 只保留 NodeState、拓扑关系和 typed component update；
- 通用横切能力保留小 trait；
- 内置或第三方 Figure 私有状态通过 typed update 或专用 capability editor 修改；
- capability 获取阶段完成类型校验，不把错误延迟到每个 setter。

## 5. Draw2D 对等能力

### 5.1 建议近期补强

1. Connection decoration：
   `RotatableDecoration`、`PolygonDecoration`、`PolylineDecoration` 和 endpoint
   orientation。当前只有 centerline decoration inset，没有实际 decoration Figure 协议。
2. 原子 child add：
   等价 `add(child, constraint, index)` 的具名复合操作，一次完成 admission、constraint
   校验、顺序和 update publication。
3. PointList miter visual bounds：
   让普通 Polyline/Polygon 与 Connection 使用一致的 stroke envelope。
4. Graphics 基础扩展：
   custom dash、miter limit 和 path clip。
5. Editor framework：
   marquee selection、专用 CreationTool 和 reparent Command。

### 5.2 按真实需求后置

- ShortestPath obstacle routing；
- 完整 TextFlow、fragment、Bidi 和 caret；
- 任意多矩形 child clipping provider；
- ButtonGroup、Slider 和 repeat firing；
- PrinterGraphics、ScaledGraphics、Animation 和 XOR。

这些能力应作为独立 delta 进入，不以机械复制 Draw2D 类名作为完成标准。

## 6. 建议执行顺序

| 批次 | 目标 | 约束 |
|---|---|---|
| E1 | 修复 Color、Path/Arc 与 Render IR 的公开正确性 | 先消除静默不执行 |
| E2 | 收口 Geometry、Insets、Dimension、alias 与第三方类型泄漏 | 直接 breaking，不留双入口 |
| E3a | 收口 crate root 与 backend feature | 已由 ADR-021 完成 |
| E3b | 收口 Figure capability 与 scoped service | 不改变 Runtime 单一所有权 |
| E4 | 拆分 Scene/Runtime/Viewer 内部模块 | 不提前新增 crate |
| E5 | Connection decoration、原子 add 与 miter bounds | 每项独立 parity delta |
| E6 | TextFlow、ShortestPath 等需求驱动增强 | 不进入默认主线 |

## 7. 文档一致性问题

- `doc/parity/draw2d/api-coverage.md` 的 Viewport/Scale/ScrollPane 行仍把 handle mutator
  写作公开入口，与 ADR-019 scoped editor 实现不一致；
- `doc/00-index.md` 仍称 2026-09-16 语义整改“进行中”，而整改报告状态已为
  `complete`；
- Editor roadmap 的 G6 仍包含产品 schema、serializer 和产品级 Native/Web 场景，
  应移出引擎里程碑。

## 8. 验证基线

- `cargo check -p novadraw-core -p novadraw-geometry -p novadraw-render
  -p novadraw-scene -p novadraw-editor -p novadraw-inspector`：PASS；
- `cargo check -p novadraw --no-default-features`：PASS；
- ADR-019 实施后的 `cargo xtask check --full`：PASS。
