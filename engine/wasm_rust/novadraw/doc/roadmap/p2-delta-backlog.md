# P2 Delta Backlog

类型：`roadmap`

本页记录 Draw2D Core 1.0 与当前 Editor roadmap 之外、已确认但尚未进入实施的
能力缺口。每项进入实施前必须先补充对应的 design/parity delta、自动验证和完成
证据；本页不定义运行时契约。

## Rendering

### P2-R01: PointList Figure 的 miter visual bounds

状态：`not_started`

`PolylineFigure` 和 `PolygonFigure` 当前从点列表计算 bounds 时，只按
`stroke_width / 2` 向外扩展。渲染后端对 `LineJoin::Miter` 使用
`DEFAULT_STROKE_MITER_LIMIT`，尖锐拐角的实际描边范围可能超过该 bounds，并被
默认 Figure child-bounds clip 截断。

这与 Connection Figure 的既有几何契约不同：Connection 已将 miter limit 纳入
prepared geometry 的 visual outset。普通 PointList Figure 尚未具有等价的 visual
envelope 计算，不能将该能力标记为已完成。

实施 delta 至少需要：

1. 让 PointList Figure 的 bounds/visual envelope 与 `LineJoin`、stroke width 和
   miter limit 使用同一受控契约；
2. 保持 point 编辑、Runtime damage、精确命中和 Figure clip 的坐标域一致；
3. 覆盖 Polyline、Polygon、锐角 miter，以及 Round/Bevel join 的定向测试；
4. 更新 Draw2D parity 账本和相关 rendering 设计文档，明确与 Draw2D 的差异或
   合理增强。

当前证据：

- `novadraw-scene/src/figure/polyline.rs::normalize_points`
- `novadraw-scene/src/figure/polygon.rs`
- `novadraw-scene/src/connection/figure.rs::route_visual_outset`
- `novadraw-render/src/command.rs::DEFAULT_STROKE_MITER_LIMIT`
