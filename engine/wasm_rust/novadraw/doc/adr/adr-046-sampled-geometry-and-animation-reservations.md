# ADR-046: 表现几何与动画容量预留

类型：`architecture-decision`

## 状态

已接受。2026-10-24 用户明确批准按 Tree 修订 3 实施。
T-1 至 T-4 的实施状态只在 [P2 backlog](../roadmap/p2-delta-backlog.md)维护。

## 决策

接受 [Tree 表现几何修订 3](../design/animation/tree-layout-presentation-proposal.md)。
该方案已经过首轮、修订 2 和
[修订 3 复核](../verification/reviews/tree-layout-presentation-revision-3-review-2026-10-24.md)，
原始发现与各轮结论保留，不以设计通过代替实施证据。

- Runtime 拥有真实 bounds、表现可见性、viewport origin 与派生 drawing。
  FigureTree、RangeModel、布局、输入和 history 的源真值保持 committed 语义。
- 完整表现版本 E 关联最后成功 Runtime 采样 S、活动集合和合法源依赖版本。
  capture/准备复用 Motion 缓存补齐几何，不推进时钟；录制、在途和呈现确认独立。
- 绘制的 placement、背景、自内容、border、client area、child transform、clip、
  可见性和 damage 共用只读几何视图。真实 bounds 不生成宽高比缩放，也不继承
  旧 affine 过渡绕过 child bounds clip 的规则。
- typed channel 与几何派生 provider 使用统一身份、注册、依赖、预检与错误通道。
  依赖图按稳定顺序执行，循环拒绝；provider 不获得 Runtime 可变引用。
- capture 显式冻结目标、兄弟影响、祖先和 provider 依赖。反向从当前完整 E 开始；
  折叠收拢终点与静态隐藏终值分开。取消、完成或普通派生失败清除整个关联计划。
- 每个计划在准入时声明并预留自身上限，retarget 按净活动容量替换，另计 staging
  和快照留存责任。自身违约与共享不可用分别归因；合法活动组不因执行顺序被驱逐。
- viewport 跟踪与布局共用一个 E，双轴 clamp；引擎统一预检并原子接管直接输入。
  origin 在正常 child transform 中应用一次，不另给 contents 叠加 pan。

## 兼容与边界

保留旧 `BoundsTransition` 与 viewport affine transition 的现有语义。Tree 使用新的
typed 真实几何入口，不通过改变旧方法含义获得支持。
本 ADR 扩展 ADR-026 的领域消费者和读取协议，保持其时间、源/表现分离及预算失败
原则。具体签名在实施时落到既有领域 facade，不扩张 Runtime 本体或根导出。

渲染继续走 `render_recursive.rs` 的原有 paint 顺序。批准的修改范围是从统一几何
视图读取值；不增加第二套遍历主线、Tree 快路径或全局状态。递归深度防护保持。
没有动画时不构造表现快照，不进行新增派生扫描。

## 外部消费者证据

T-1 必须在独立测试 crate 中注册同底层 Rectangle 的不同语义 channel、几何派生
provider 和自定义容器。它们与框架提供的 channel 共用 typed 注册、完整几何查询、
循环/失败/预算和 owner 语义，不使用内部 hook 或具体 Figure 名称。
公开入口已落到 `animation` 领域：`GeometryBinding<V>`、`GeometryProvider<V>`、
`GeometryDrawingBuilder` 与 `GeometryCapture`。外部 Figure 通过 `BOUNDS_SNAPSHOT`
提供有界自绘，Border 通过同一 bounds snapshot 协议提供冻结绘图。
独立 integration target `p2_animation_geometry_contract` 注册两个 Rectangle 语义、
自定义容器、内容/边框快照和派生线条，并覆盖循环、未声明查询、panic 与容量拒绝。
Tree 示例 crate 使用这些公开入口，不依赖 Core 私有类型或名称判断。
执行证据由 `animation.tree` suite 和后续实施复核记录维护。

## 验证与关系

`api_semantics`：`animation.presentation`、`animation.timeline`、`layout.manager`、
`coordinate.conversion`、`viewport.scroll_zoom`、`damage.repaint`、
`figure.geometry.bounds`、`figure.visibility.enabled`、`figure.box.client_area`、
`clipping.strategy`、`paint.protocol`、`frame.preparation`。

T-1 验证 S/E、完整几何、typed 扩展、容量预留与失败；T-2 验证同源端点与内容冻结；
T-3 验证收拢、Viewport 和输入接管；T-4 验证真实面板、Native 中间帧、retained-partial
擦除、Retry/reset 与资源释放。详细退出证据以方案为准。

保持 [ADR-025](adr-025-unified-graphics-and-glyph-preparation.md)、
[ADR-026](adr-026-animation-and-presentation-plane.md)、
[ADR-028](adr-028-typed-property-identity.md) 与
[坐标规范](../design/coordinates/coordinate-system.md)的所有权和身份边界。
