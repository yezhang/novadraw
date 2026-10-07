# UML 复杂样例扩展能力验证

类型：`verification`

状态：`complete`

日期：2026-10-05

## 1. 目标与范围

本验证通过 `examples/native/uml-demo` 的订单域类图检查框架能否在不修改 Core 分支的
前提下组合复杂业务图形。参考对象限定为 Draw2D 自带样例：

- `org.eclipse.draw2d.examples.uml.UMLClassDiagram`
- `org.eclipse.draw2d.examples.uml.UMLClassFigure`
- `org.eclipse.draw2d.examples.zoom.CompartmentFigure`

未使用 `org.eclipse.zest` 的 UML 类型或布局逻辑。

样例包含 6 个类、6 条关系，以及 association、dependency、realization、composition
四类 UML 表达。每个类由外部 `Figure`、header、attribute compartment、method
compartment 和 Label 组成；关系使用 Connection layer、Chopbox anchor、Direct /
Bendpoint router、Locator 与可旋转 decoration。class 支持 Native/Web 共用的 pointer
capture 拖拽，bounds 提交后由 Runtime 自动失效并重路由关联 Connection。

## 2. 自动与视觉证据

稳定 suite：`example.uml-extension`

自动结果：PASS。

| 指标 | 结果 |
|---|---:|
| 外部复合 class Figure | 6 |
| Connection Figure | 6 |
| 已提交 route point | 14 |
| Glyph run | 41 |
| Connection polyline command | 6 |
| 单次拖拽失效并重路由的 Connection | 6 |

Native/Vello 截图结果：PASS。确认项目标题、全部 class member、compartment separator、
虚实线型、开放箭头、实现三角、组合菱形和 connection label 均可见，未出现空帧、文字
裁剪或端点方向错误。

交互结果：PASS。按下 class header 后 capture 固定到 class；拖动提交 parent-content
bounds，release 清除 capture；相关 Connection 在下一稳定帧使用新端点重新生成 route，
无 deferred mutation error。

## 3. 能力结论

| 能力 | 结论 | 证据 |
|---|---|---|
| 外部自定义 Figure / Container | PASS | examples crate 定义 `UmlClassFigure`、`UmlCompartmentFigure`，Core 无分支修改 |
| 复合 Figure 与样式继承 | PASS | class/header/compartment/Label 多层树和统一字体、前景色 |
| Layout 扩展组合 | PARTIAL | class/header 的 `ToolbarLayout` 可用；更深层组合存在收敛问题，见 F-01 |
| Connection 扩展 | PASS | Layer、Anchor、Direct/Bendpoint Router、typed constraint、Locator |
| Decoration 扩展 | PASS | examples 自定义 diamond/triangle template，无需新增 Core Figure |
| Figure 直接拖拽 | PASS | EventContext deferred bounds mutation、capture 生命周期与 Connection 自动重路由 |
| Headless 到 Native/Vello | PASS | 同一共享 scene 同时通过结构探针和真实截图 |

结论：当前公开 API 足以承载中等规模、可直接拖拽的单坐标根 UML 图。扩展点总体成立，但
“任意深度复合布局”和“自动图布局”尚不能视为完成。

## 4. 缺失能力

### F-01：深层复合布局与文字 presentation 未稳定收敛

优先级：P0。

初版让 attribute/method compartment 自身再安装 `ToolbarLayout`。稳定帧中各 Label
bounds 已分离，RenderSubmission 也包含全部 41 个 glyph run，但 Native/Vello 只显示
每个 compartment 的部分成员。将成员改为 compartment-local 固定 bounds 后全部显示。

这说明 `layout -> Label presentation -> paint clip/transform` 的派生状态组合缺少覆盖，
不能仅凭 layout bounds 或命令数量判定正确。应新增最小契约，要求两层及以上由父布局
改变 bounds 的 Label 容器在同一稳定帧使用最终几何；修复应落在 Runtime 派生状态收敛，
不能在应用层增加重复刷新。

### F-02：缺少 Directed / Compound graph layout adapter

优先级：P1。

6 个 class bounds 和两个人工 bendpoint 由样例显式给出。框架可以消费布局结果，但不能
从 class/relationship 图模型自动计算层次、rank、间距和边路由。该缺口已在长期能力
分母中登记为 `CAP-GRAPH-LAYOUT`，对应 `P2-L01`。

退出条件应保持现有定义：独立图模型、确定性输出、外部算法可替换，输出通过公开
Figure/Layout/Connection API 提交，不把第三方图类型暴露到 Core。

### F-03：Label/TextFlow 缺少 fragment style

优先级：P2。

UML member 只能用单一 Label 样式表达，无法在同一签名中分别渲染 visibility、名称、
类型和 stereotype emphasis。该缺口属于 `P2-T03`，不应通过拆成字符级 Figure 或在
样例中手绘 glyph 绕过。

## 5. 未覆盖边界

以下能力不是本次 PASS 结论的一部分：

- 跨 divergent viewport 的 connection clipping，仍由 `P2-C03` 负责；
- command-backed class move/resize、关系创建、reconnect、undo/redo 等 Editor 交互；
- UML 模型持久化、schema 与产品级文档生命周期；
- 大图自动布局和性能基线。

## 6. 建议顺序

1. 先将 F-01 提炼为独立最小复现与 Core 契约测试，再修复派生状态收敛。
2. 按当前 roadmap 推进 `P2-L01`，用本 UML 样例作为第一个外部消费者。
3. fragment style 继续随 `P2-T03` 推进，不阻塞静态 UML 样例使用。
