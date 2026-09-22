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

## Developer tooling

### P2-D01: FigureInspector 开发期可观测性

状态：`in_progress`

FigureInspector 以独立 crate 消费 Runtime 的稳定场景查询与提交后 notification journal，
为 Native、Web 和 headless 工具提供统一诊断事实。它不属于 Draw2D Core 的运行时语义，
也不改变 GEF Editor 里程碑。

首期范围：

1. `FigureTreeSnapshot` 保留 containment、child order、节点状态和 Figure diagnostic name；
2. 有界 FIFO event timeline 保留 `source_epoch`、`sequence` 和 typed effect；
3. outline tree、属性面板、时间线作为宿主 UI 标准布局；
4. 不进入 render/validation 热路径，不保存逐 effect 的完整历史场景；
5. Editor Figure -> VisualOwner -> EditPart -> Model 关联由可选 adapter 后置。

规范与决策：

- `doc/design/architecture/figure-inspector.md`
- `doc/adr/adr-016-figure-inspector-observability.md`

## Editor

### P2-E01: 高级 Self-loop 路由策略

状态：`not_started`

Draw2D/GEF 核心只提供同源同目标 Connection、Anchor、Router 与 routing constraint
扩展点，不规定 self-loop 的开口方向、折点数量或障碍避让。Native node editor 当前用
两个显式 absolute bendpoints 实现节点右侧的三段正交 self-loop；该策略只承担 demo
验收，不进入 Draw2D/GEF parity。

Core 与当前 G5 必须继续保证：

1. self-loop 只投影一个 ConnectionPart，并同时进入 source/target relation；
2. 两个 endpoint handle 保持独立，重连保留 Connection 身份与 history 原子性；
3. 应用可通过 Anchor Descriptor、Router 和 typed constraint 生成非退化回环；
4. demo self-loop 跨 owner 重连时在新 owner 坐标域重建折点，节点移动时整体平移。

后续产品 delta 可按真实需求补充：

1. 根据可用空间选择上、下、左、右开口；
2. 多条 self-loop 的 lane 分配与稳定排序；
3. 节点、端口、标签和其他连接的障碍避让；
4. resize、端口迁移与自动布局后的形状保持；
5. 对应基准、视觉回归和可序列化 routing descriptor。
