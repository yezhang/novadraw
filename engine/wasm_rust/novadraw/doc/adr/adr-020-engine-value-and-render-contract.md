# ADR-020: 引擎基础值与公开渲染契约

类型：`architecture-decision`

## 状态

已接受，实施中

## 背景

Draw2D Core 与 Editor framework 主干完成后，当前风险已从能力缺失转为公共契约不一致：

- `Color` 声明通道位于 `[0, 1]`，但公开字段、构造和反序列化均可破坏该不变量；
- `NdCanvas::arc` 忽略方向参数，公开 `PathOp::Arc` 又未被 Vello backend 消费；
- `RenderCommandKind::Path` 可以由外部构造，但当前没有 producer 或 backend lowering；
- Render IR 直接暴露 `glam` 类型，使第三方实现绑定内部数学库；
- Geometry 同时公开多组无语义差异的兼容 alias；
- `novadraw-math` 的 3D 类型没有被当前 2D 引擎消费，却进入多个 crate 的依赖图。

同时，Editor roadmap 原 G6 的 document schema、serializer 与产品级 Native/Web 场景属于
独立产品包，不应驱动本仓库的引擎 API。

## 决策

### 1. 引擎与产品边界

本仓库的 Editor framework 以 G0-G5 为完整引擎闭环。产品 document schema、
serializer、业务模型和产品 UI 由下游产品包定义。

引擎只承诺：

- 外部模型身份和事件可以重新注入；
- Runtime、EditPart 和 Figure 投影可确定性重建；
- 不在引擎 crate 中定义产品文档格式或持久化兼容策略。

### 2. Render IR 必须可执行或显式拒绝

公开 Render IR 不允许存在 backend 静默忽略的命令或 path operation：

- 删除没有 producer 和 consumer 的 `RenderCommandKind::Path`；
- Vello backend 必须完整 lower `PathOp::Arc`；
- backend 对命令和 path operation 使用穷尽匹配，不保留 wildcard 忽略分支；
- 不能实现的能力必须在 submission capability 检查阶段返回结构化错误。

### 3. Arc 使用明确且一致的语义

- `NdCanvas::arc` 保持 Draw2D 风格的角度单位：degree；
- `anticlockwise` 必须参与 sweep 计算；
- 圆弧转换为受容差控制的 cubic Bezier，不使用固定段数折线；
- SVG endpoint arc 和 center arc 最终进入同一 backend-neutral Path IR；
- path bounds 必须覆盖 curve control geometry，不能只统计 segment endpoint。

### 4. Graphics 只保留一套规范命名

公开 `NdCanvas` 以 stateful Graphics API 为规范入口。显式携带 paint 参数的 recorder
primitive 必须使用清晰的 `*_with_*` 命名，或仅作为内部 lowering API；不再让
`fill_rect/fill_rectangle` 等近义名称表达不同状态模型。

该命名迁移独立成后续小批次，避免与 Arc 正确性修复混合。

### 5. Color 兑现值不变量

- Color 分量字段私有；
- `Color::rgba` 是显式饱和构造，所有分量规范化到 `[0, 1]`；
- `Color::try_rgba` 用于需要拒绝非法输入的边界；
- `Color::from_hex` 与 `FromStr` 返回结构化错误；
- 反序列化复用同一校验，不允许绕过不变量；
- 提供只读 `red/green/blue/alpha` accessor 和 `to_rgba_array`。

### 6. Geometry 使用唯一领域词汇

- `Point` 与 `Vec2` 必须成为不同类型，分别表示位置和值向量；
- 运算语义固定为 `Point - Point -> Vec2`、`Point +/- Vec2 -> Point`；
- 公开 scene/render API 使用 `Point`、`Vec2`、`Rectangle`、`Dimension`、`Insets`
  和 `Affine2D`；
- 删除 `PrecisionPoint`、`PrecisionRectangle`、`PrecisionDimension`、`Size`、
  `Vector` 和 `AffineTransform` 等无独立语义 alias；
- `glam` 与 `kurbo` 只留在实现和显式 interop 边界，不出现在默认 RenderCommand API。

### 7. 当前不扩大 crate 数量

- `novadraw-scene` 先执行内部模块拆分，不拆成更多共享状态 crate；
- `novadraw-math` 从 2D 引擎依赖图移除；若未来出现真实 3D 使用方，再按独立能力恢复；
- `novadraw-core` 的命名与最终归属在基础值迁移后单独评估，本批次不制造中间 facade。

## 失败处理

- 非法 Color 文本和非有限 RGBA 输入返回结构化错误；
- 饱和构造必须确定性处理 NaN 和无穷值；
- 非法或退化 arc 不得生成非有限 path；
- backend 不支持某项公开命令时返回 unsupported，不得静默跳过；
- 本批次是 `0.1.0` 直接 breaking migration，不保留无语义差异的兼容 alias。

## 验证

- Color 非法长度、非法字符、非有限和越界分量；
- Color serde 不能构造非法值；
- 顺时针、逆时针、小弧、大弧和完整圆；
- `PathOp::Arc` 在 Vello lowering 中产生曲线；
- curve/arc bounds 覆盖实际几何；
- RenderCommand 公开字段不再出现 `glam`；
- 删除 geometry alias 后 workspace 调用方全部迁移；
- `novadraw --no-default-features` 保持可编译；
- `cargo xtask check --quick`，最终交付执行 `cargo xtask check --full`。

## 后续

以下内容不混入本批次：

- `Bounded` 兼容 trait 与 Figure capability 收口；
- Runtime service 和大文件内部拆分；
- Connection decoration；
- custom dash、path clip、image source rectangle；
- TextFlow、ShortestPath 和完整 widget toolkit。

## 关系

- 延续 [ADR-017](adr-017-core-public-api-boundary.md) 的公开 API 失败契约；
- 延续 [ADR-018](adr-018-runtime-driving-and-measurement-api.md) 的领域值类型方向；
- 延续 [ADR-019](adr-019-composable-api-and-scoped-editors.md) 的生命周期 API 分层；
- 落实
  [引擎能力与 API 稳定化评估](../verification/reviews/engine-capability-assessment-2026-09-28.md)；
- 对应 `api_semantics`：`graphics.context`、`geometry.primitives`。

## 日期

2026-09-28
