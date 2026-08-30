# M8-M9 Viewport 与 Connection 交付计划

类型：`roadmap`

本文记录 M8/M9 的交付范围和验证门禁，不定义理想架构。架构契约以
`doc/design/` 为准，里程碑状态以 [00-index.md](00-index.md) 为准。

## 1. 边界

### M8

- RangeModel；
- Viewport；
- ScrollPane / ScrollBar；
- ScalablePane / ZoomManager；
- wheel、trackpad scroll 和 pinch；
- viewport、scale、clip、hit-test 和 damage 的组合验证。

### M9

- Connection Figure；
- Anchor；
- Router；
- Locator / Decoration；
- ConnectionLayer；
- 节点变化后的 reroute、damage 和 hit-test。

GEF 的 EditPart、Tool、Request、EditPolicy 和 Command 不进入 M8/M9。

## 2. 架构前提

M8/M9 必须遵循当前设计：

- 运行时身份使用 `FigureId`；
- parent/child topology 只由 FigureTree 管理；
- InteractionState 不放入 FigureTree；
- bounds 使用 parent content domain；
- scroll/scale 通过统一 edge transform 接入；
- RangeModel 是 scroll origin 的唯一真源；
- ScalablePane 是 scale 的唯一真源；
- mutation 和更新通过 Runtime 原子事务；
- Figure、LayoutManager 和 policy 不默认要求 `Send + Sync`；
- 平台类型不进入 Figure、layout、input 或 update 协议。

## 3. M8 契约

### RangeModel

RangeModel 表达：

```text
minimum
maximum
extent
value
```

并始终满足：

```text
minimum <= value <= max(minimum, maximum - extent)
extent >= 0
```

同一操作中的多个字段变更必须原子提交并产生一次语义通知。

### Viewport

- 最多一个 contents；
- client area 是可视窗口；
- origin 来自 RangeModel；
- contents extent 来自 layout/intrinsic size；
- scroll transform、paint、hit-test、event point 和 damage 共用坐标协议；
- contents 小于 viewport 时的对齐由 ViewportLayout 策略定义。

### Zoom

- scale 由 ScalablePane 持有；
- ZoomManager 协调 scale、extent 和 origin；
- anchor zoom 保持同一 content point；
- zoom transaction 内完成 RangeModel clamp；
- 不使用“旧 bounds × scale ratio”推导内容尺寸。

### 输入

- 连续 gesture 固定 target 和 typed controller；
- target callback 一次；
- Scroll/Zoom fallback 不是普通事件冒泡；
- pointer capture 与 gesture session 独立；
- native event 只在平台 adapter 中出现。

## 4. M8 验证门禁

- RangeModel 原子 clamp；
- viewport 单 contents；
- border/insets/client area；
- nested scroll/scale transform；
- line 与 logical-pixel delta；
- gesture Begin/Update/End/Cancel；
- anchor zoom；
- 缩放后四边可达；
- scrollbar 与 extent 同帧一致；
- old/new damage；
- headless contract tests；
- 至少一个真实窗口人工验收。

## 5. M9 契约

### Anchor

- Anchor 使用 `FigureId` 引用 owner；
- location/reference point 通过只读树和坐标查询取得；
- owner 删除、reparent 或 transform 变化时引用可检测失效；
- Anchor 不长期持有 FigureTree。

### Router

- Router 是纯计算或显式缓存策略；
- 输入为 connection endpoints、constraints 和只读 scene query；
- 输出为 point list、route metadata 或结构化错误；
- Router 不直接修改 FigureTree 或 UpdateManager。

### Connection Runtime

connection-specific 关系状态属于专用 runtime component，不塞入通用 FigureNode：

```text
ConnectionState
├── source anchor
├── target anchor
├── router
├── routing constraint
└── route cache
```

Connection Figure 只负责绘制、精确 hit-test 和内在样式。节点或 anchor 变化通过依赖
索引使 route cache 失效。

### Locator / Decoration

- Locator 在 connection path 或 endpoint 上放置 child Figure；
- Decoration 是普通 Figure；
- 所有结果仍使用二维 parent-local bounds 和统一坐标转换。

## 6. M9 验证门禁

- Anchor location/reference；
- owner move、resize、reparent 和 remove；
- direct、bendpoint 和 orthogonal routing；
- route cache invalidation；
- connection projected damage；
- stroke-aware hit-test；
- locator 和 endpoint decoration；
- ConnectionLayer 的 paint/hit-test 顺序；
- deep tree 和 viewport/zoom 嵌套；
- demo 与视觉断言。

## 7. 推进顺序

1. 完成 M8 手工验收并更新唯一状态入口。
2. 定义 Anchor/Router 的 architecture delta。
3. 编写纯计算 contract tests。
4. 实现 connection runtime 与 Figure。
5. 补齐 damage、hit-test 和 viewport/zoom 组合测试。
6. 完成 demo、视觉验证和文档收口。
