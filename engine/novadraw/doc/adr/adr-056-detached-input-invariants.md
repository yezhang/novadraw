# ADR-056：Detached 输入不变量

类型：`architecture-decision`

状态：已接受；2026-10-29。用户批准按更新后的决策进入 S08-A。
S08-A 已完成公共消费者、API 边界、非法输入与继承回归，见
[实施记录](../verification/reviews/p2-s08-a-implementation-2026-10-29.md)；
后续切片尚未执行，整批 S08 未关闭。

本轮已补充 Compound 基础／owner 指标边界与历史描边映射，
见[设计复核](../verification/reviews/adr-056-design-review-2026-10-29.md)。
本次接受以更新后的设计和验证矩阵为准。

## 背景

P2-S08 承接 R03、TC-03、API-F04 和 API-10。已有受检颜色、描边和字体值，
但 Border 构造仍可能 panic，构建器和公开标量字段又能绕过校验。
FigureStyle 接受原始 alpha 和字体文本，错误可延迟到挂载、测量或绘制。

目标是在原始输入进入可用对象时拒绝领域非法值，并保留 Runtime 对跨对象关系、
派生计算和外部协议输出的检查。失败位置必须明确，默认对象仍应容易构造。

## 决策

采用[输入不变量专题](../design/architecture/detached-input-invariants.md)：

- 复用 Opacity、StrokeStyle、FontDescriptor 和 Color。Opacity 的唯一公开领域路径
  迁到 `graphics::Opacity`，动画实现继续消费同一个类型；输入错误归 Graphics。
- FigureStyle 的 alpha 和 font 分别使用 `Option<Opacity>` 与
  `Option<FontDescriptor>`。这些已受检、可独立替换的字段保留公开。
  最近祖先覆盖、默认值及同值无通知的语义保持。
- Border 使用私有受检配置。原始宽度、insets、padding 和组合输入的构造或更新
  返回结构化错误；默认构造、安装已验证值和受检 builder 的 build 保留直接返回。
  删除对应 panic 入口及无独立语义的 alias。
- 内置 Detached Figure 的原始 bounds、stroke、corner 和平移更新使用同一原则，
  收口可绕过校验的数值字段；Shape 零宽与旧 corner dimensions 单位保持。
- LineBorder／RectangleBorder 复用 StrokeStyle，同时保持边框宽度严格大于零的
  领域约束。Graphics 的零宽无绘制行为继续有效。
- MarginBorder 只保存实际边距，移除未参与边距和绘制的兼容颜色、宽度及样式。
  原始 Insets 按具体用途校验，不建设覆盖全部 f64 的包装体系。
- 构造检查配置自身的跨字段计算；挂载、替换和 owner 测量检查外部协议输出、
  组合派生溢出及身份／拓扑关系；后端检查精度和 capability。
  内置与外部 Figure／Border 共用这些检查点和错误通道。
- 删除隐藏非法输入的 alpha clamp。Color 的显式饱和构造、动画采样收敛、
  viewport range 收敛、小矩形内容区截空和圆角几何适配保留其既有契约。

公共 Figure mutation、typed component/capability registry、Runtime 单一提交权威
及递归渲染主循环保持原有职责。

## 取舍与迁移

这是一次公共类型和构造签名迁移。下游 alpha 从标量构造受检值，字体文本先解析，
边框原始配置显式处理 Result。Opacity 的旧动画路径退出，避免两个规范导入路径。
已有 AnimationError 的领域包装可保留，原始 Graphics 输入原因通过 source 追溯。

受检描边可能使原先 Copy 的边框变为 Clone；构造与快照的分配、复制成本须测量。
字体由文本改为 descriptor 后避免在查询／绘制时反复解析。没有新增依赖、字体资源、
crate 或默认 feature。产物与性能验证见专题和[验证矩阵](../verification/plans/p2-s08-detached-input-invariants.md)。

## Draw2D 对应

采用 Border 对 insets、preferred size、owner 内绘制和共享实例的协议，以及 Figure
字体继承行为。Draw2D 的 LineBorder 对非正宽度保留旧值；Novadraw 使用可观察的
结构化拒绝。Java 整数 Insets 的可变对象模型不直接移植到 Rust。
源码提交、方法和合理变体见专题的参考表。

`api_semantics`：`border.protocol`、`figure.properties`、`figure.geometry.bounds`、
`figure.box.client_area`、`geometry.primitives`、`graphics.context`。

## 外部消费者证据

实施时由独立 integration crate 创建第三方 Figure 和 Border，使用公开受检值、
普通 builder 和 typed mutation，不修改 Core。验证非法原始配置被构造拒绝，
外部回调返回非法几何被统一 admission 拒绝，成功对象与内置对象使用相同
measurement、paint、damage 和通知路径。具体用例见验证矩阵；
S08-A 先验证受检样式与第三方 Figure，其他场景由后续切片完成。

## 接受条件

先评审 Opacity 领域归属、Border 构造迁移、原始几何校验位置和外部输出边界。
接受后逐切片实施，更新现行专题、API snapshot 与消费者。只有完成记录具备构造、
更新、挂载、派生溢出、渲染和产物证据，才能关闭 S08。
本次颜色文本修复仍归 ADR-020，不作为 S08 完成证据。
