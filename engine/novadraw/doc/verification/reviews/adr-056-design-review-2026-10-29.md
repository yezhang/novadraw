# ADR-056 设计复核与下一批边界

类型：`architecture-review`

日期：2026-10-29。范围为 [ADR-056](../../adr/adr-056-detached-input-invariants.md)、
[专题](../../design/architecture/detached-input-invariants.md)和
[验证矩阵](../plans/p2-s08-detached-input-invariants.md)。这是本 Agent 的定向复核，
不代表独立 reviewer 或用户接受。

## 结论

建议接受 S08 的方向：复用受检值，原始配置在构造／更新时拒绝，
动态外部输出在挂载／owner 准备时再检查。Opacity 唯一路径归 graphics，
FigureStyle 保存 Opacity 与 FontDescriptor；Border 使用同一 StrokeStyle。
保留值错误、发布错误与服务 fault 的区分。

本轮补清两处实施歧义：Compound 构造读取的是基础指标，不触发 owner 字体测量；
历史 DashDot 按 Dash 绘制，迁移保持该行为。验证矩阵增加 V11/V12。
S08 仍为设计草案，需用户接受后实施。

## 逐项判断

| 议题 | 判断与实施约束 |
|---|---|
| Opacity 归属 | 图形值不依赖动画服务；迁移旧路径，不能增加第二个公开 alias |
| 独立样式字段 | 已受检值可独立替换，无需机械私有化全部样式 |
| Border 正宽与 Shape 零宽 | 两种领域约束分别检查，不能把 Border 的正宽限制施加给 Graphics |
| Compound 构造 | 同一次基础候选检查；外部内部可变状态不由构造永久担保 |
| owner 快照 | 两个 owner 独立准备，失败不发布新成功快照；检查和发布不能读两次不同值 |
| 小矩形与圆角 | 非有限和溢出先拒绝，合法截空和轮廓适配继续有效 |
| 错误与成本 | 保留 source 和字段；正常 paint 不重复解析字体或扫描未失效的配置 |

参考基线为 GEF Classic `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
只读核对 `org.eclipse.draw2d.Border` 与 `CompoundBorder`：
`getInsets/getPreferredSize` 接收 owner，实例可共享，组合按 outer／inner 计算。
这支持区分配置和 owner 指标，不能据此推导“构造时已验证所有 owner”。
本轮未运行或修改 Java。

## 接受后的首步

本轮已按下表补做公开入口的首轮盘点。实施时逐项关联构造、builder、setter、
Bounded 与 typed component 更新，给每一项标记迁移或保留的理由；
该逐入口闭合仍是 S08-C 的执行门禁。

| 家族与源码 | 已定位的数值入口 |
|---|---|
| Rectangle／Ellipse：`figure/rectangle.rs`、`ellipse.rs` | new／from_bounds／new_with_color、with_stroke、translate、set_bounds、公开 bounds／stroke_width |
| RoundedRectangle：`figure/rounded_rectangle.rs` | 上述入口，加 corner dimensions／radius setter；保留旧单位 |
| Triangle：`figure/triangle.rs` | new／new_with_direction、公开 bounds、已受检 stroke／corner 与 Bounded setter |
| Polyline／Polygon：`figure/polyline.rs`、`polygon.rs` | 点坐标、set_points、with_width／with_stroke、hit tolerance、Bounded 缩放；不能只保护矩形字段 |
| ScalablePolygon：`figure/scalable_polygon.rs` | 受检 new、with_stroke、Bounded setter；模板与设备映射输出保留各自检查 |
| Root／GraphProjection：`figure/root.rs`、`graph_projection.rs` | 原始 bounds 构造、Root setter、FigureStyle 输入 |
| Label／Image：`figure/label.rs`、`image.rs` | with_bounds、Bounded setter；Label icon_text_gap 需按原有用途核对合法范围 |
| Clickable／Button／Toggle：`figure/widget.rs` | Clickable bounds 构造、Button／Toggle with_bounds、三者 Bounded setter |
| TextFlow／FlowPage：`figure/text_flow.rs`、`flow/nodes.rs` | bounds 构造／Bounded、with_style／with_border；已有受检 FlowBlockStyle 不重复造包装 |
| Layer／Scalable／Viewport：`container/layer.rs`、`scalable.rs`、`viewport.rs` | Figure 目录外的 bounds 构造与 Bounded setter；不能漏掉公开容器 |
| ScrollBar／ScrollPane：`container/scroll_bar.rs`、`scroll_pane.rs` | with_bounds／Bounded；RangeModel 既有受检与滚动收敛语义保持 |
| ConnectionLayer／SceneView：`connection/figure.rs`、`scene/view.rs` | 层 bounds 构造／Bounded 与 SceneView with_bounds；连接路由和场景 admission 不改成局部输入校验 |
| Border：`figure/border/` | Line／Rectangle／Margin 的公开原始字段与 builder；Bevel width、TitleBar padding、Compound 基础／owner 指标 |

路径相对 `crates/novadraw/src/`。这是方法和字段盘点，不代表这些类全部有同一种
缺陷。已受检入口保留；测量值、准备上下文和外部 Decoration 输出由对应消费边界
检查，不把所有公开 f64 一并私有化。

`Bounded::set_bounds` 当前不返回错误；Polyline 的实现还会按旧尺寸计算新点。
仅将 concrete setter 改为 Result 会留下 trait 调用的绕过路径。
实施前须同步确定 trait 的受检返回值和退化点缩放行为，并用外部消费者调用 trait
验证失败原状态保持，不能留下静默丢弃 Result 的兼容入口。

按 S08-A～D 单链推进，每片同时迁移公开消费者和 API 快照。
字体合法 token、重复／冲突、Unicode 和未知 family 的回归与 S08-A 一起落地。
使用同环境的构造、更新、Border 准备、连续 resize 和产物测量，不能将
本轮 E03 性能结果记为 S08 验收。

## S09 准备边界

S09 承接 R07、TC-10、API-F08、API-F10 和 API-F09 的残余注册 alias，
依据[审计清单](../checklists/audit-remediation-execution.md#10-p2-s09服务失败inspector-与错误-source)。
下一份 ADR 应先列出服务状态、可恢复错误、订阅所有者及注销时机，再定公开签名。

- Animation 的 fault 与 `Ok(false)` 幂等结果分开。
- Inspector poison 可观察；attach／detach 明确 Observations token 的所有权。
- 注册入口按实际行为盘点，删除 panic alias，保留一个有失败信息的入口。
- Command／Policy／扩展错误保留类型和 source；panic 诊断不伪装成可恢复输入错误。

S09 仍为 `not_started`；本页只准备范围，没有冻结新接口或新增实现。
吸附 Grid／Geometry／Guide 在这些 API 和失败边界收口后进入独立设计，
不混入本次受检值迁移。

中文交付已按好好说话技能回读；编号标题、B2 变体、元话语、
中文半角标点和长破折号机械扫描均为 0 命中，无豁免。
复核正文和新增矩阵用例已通过文档门禁，不据此宣称实现通过。
