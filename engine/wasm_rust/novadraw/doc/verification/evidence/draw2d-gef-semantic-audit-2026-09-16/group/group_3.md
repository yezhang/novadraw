# Group 3: Geometry / Graphics / Paint / Backend / Resources / Text / Widgets

审计日期：2026-09-16。范围：当前工作区完整文件语义，不按 diff 过滤。
结论：确认 3 条 P1 条件性功能缺陷，详见 G3-F01 至 G3-F03；不能据现有
`verified` 标签推断所有组合场景已闭合。本报告不修改 parity 状态。

本组仅静态阅读，未运行 Cargo、构建、测试、应用探针或截图验证。
主审告知 `workspace.quality`、全部应用探针和 Editor replay 已通过；这属于主审
验证结果，不替代本组指出的缺失场景。Web 构建 E0308 由主审归入平台缺陷，
本组未重复检查、未重复报送。

## 1. 范围与方法

遵循 `reviewer-brief.md`、`review_groups.md` Group 3、`review_files.md`，
以及 analyzing-gef-code / bits-code-guard 的官方源码优先、完整函数和直接调用方核对、
七维度评估、具体触发路径定级规则。完成启动文件及项目记忆读取。

实际阅读范围：

- `novadraw-core/src/{lib,color}.rs`。
- `novadraw-geometry/src/{lib,point_list,precision,rect,transform,translatable,vec2}.rs`。
- `novadraw-math/src/{lib,mat3,vec3}.rs`。
- `novadraw-render/src/{lib,traits,command,context,submission,text}.rs`、
  `backend/mod.rs`、`backend/vello/mod.rs`，包括文件内测试。
- `novadraw-scene/src/figure/` 下本组列出的全部文件，包括七类边框实现、
  border 协议、基础 Figure、shape、Label、Image、widget。
- `novadraw-scene/src/graph/render_recursive.rs` 全文，仅阅读；
  `runtime/resource.rs` 全文；`scene/src/lib.rs`。
- 共享 `graph/mod.rs` 按本组职责阅读：NodeState/FigureNode 几何及 child transform、
  Figure 测量、render 入口、点列表及边框变更、Label/Image 刷新、
  BorderSnapshot、clickable 模型变更和 action 通知。
- 共享 `runtime/runtime.rs` 按本组职责阅读：文本服务和 Label/TitleBar/Image 刷新、
  style/shape/label/widget/resource 事务、widget 派生视觉状态、稳定化、
  prepare/complete submission。没有将共享文件其他职责声称为已完整审计。
- 静态阅读所有本组列明外部测试：geometry `m1_product_existence`；
  render `m1_product_existence`、`m10_text_extension_contract`、`r8_extension_boundaries`；
  scene `m10_label_contract`、`m10_reusable_shape_border_contract`、
  `m10_widget_contract`、`m10_accessibility_contract`、`m10_tooltip_contract`。
- 交叉核对 `doc/parity/draw2d/api-coverage.md`、
  `doc/design/architecture/text-layout.md`；定向核对 demo 的 Label placement 消费。

下文 Rust 路径相对 Novadraw 根；JSONL 的 `file` 相对 Git 根
`/Users/bytedance/Documents/code/GitHub/drawjs`。
行号针对审计时工作区；其中 `runtime.rs` 有并行工作区修改，关键链路已再次定位。
没有回滚或修改这些已有改动。

## 2. 官方基线

参考根：`/Users/bytedance/Documents/code/GitHub/gef-classic`。
只使用 `org.eclipse.draw2d/src` 及其官方 `org.eclipse.draw2d.doc.isv/guide-src`，
未使用 Zest、归档设计或历史缺陷作为当前结论。

- 全文阅读 `guide-src/painting.adoc`、`coordinates.adoc`：
  Figure 的位置、client area、子坐标域与累计裁剪必须一致；
  不能把 Java 默认 inherited coordinates 机械等同于 Rust 的 node-local 绘制。
- `Figure.java:1250-1379` 的 `paint/paintFigure/paintClientArea/paintChildren/paintBorder`：
  local style -> self -> restore -> client area/children -> border；
  children 按列表顺序绘制，每个子绘制后的状态恢复。border 不是先于 children。
- `Figure.java:2045-2085` 与坐标指南：父子坐标传播必须区分位置、inset 和局部变换。
- `Graphics.java` 的 state、clip、translate/scale/rotate Javadoc；
  `SWTGraphics.java` 的 `clipRect/pushState/restoreState/setClip/translate`
  实现（373、896、966、1206、1424 行附近）。
- `geometry/Rectangle.java` 的 contains/intersects/scale；
  `PrecisionRectangle.java` 的 getBounds/contains/scale：
  整数半开包含、正面积相交、浮点与整数外包围转换是不同契约。
- `Shape.java:113-196`、`Ellipse.java`、`RoundedRectangle.java`、
  `Polyline.java`、`Polygon.java`、`Triangle.java`、`ImageFigure.java` 的完整相关方法。
- `Label.java` 的 placement/alignment、preferred/minimum、截断、失效与 paint；
  `CompoundBorder.java`、`TitleBarBorder.java`；
  `Clickable.java` 的模型和绘制方法、`ButtonModel.java` 状态变更、
  `ClickableEventHandler.java`、`ToggleModel.java`。

## 3. 确认缺陷

### G3-F01 [P1] 删除 Ready 图片后，ImageFigure 保留旧资源引用并使提交持续 Retry

- 类别：`LOGIC`；置信度：10/10。
- 主定位：`novadraw-scene/src/graph/mod.rs:3061-3063`。
- 触发：注册图片并附到可见 ImageFigure，完成图片资源加载和一次正常提交；
  随后调用 `Runtime::remove_resource(image.resource_id())`，保持该 Figure 可见，
  再向 Vello 提交后续帧。无需非法数据或并发。
- 根因：`ResourceRegistry::remove` 删除 entry 并生成 Remove
  （`runtime/resource.rs:260-269`）。但 `refresh_image_figures` 遇到
  `status` 返回 UnknownResource 时直接 continue，没有清除 Figure 缓存的
  Ready 状态和 `ImageResourceRef`。
- 结果：`figure/image.rs:120-147` 继续生成旧 `(id, revision)` 的 Image 命令。
  Vello 先同步 Remove，再由 `has_required_resources`
  （`backend/vello/mod.rs:294-304`）发现该引用缺失，`submit:1121-1127`
  返回 Retry。`Runtime::complete_submission_inner:3767-3780` 恢复 delta、
  请求全重绘；下一次刷新仍跳过已删除 entry，因此重试本身无法恢复。
  在该可见 Figure 仍生成旧命令期间，受影响的是整帧提交，不只是图片显示。
- 官方与本地契约：Java `ImageFigure.setImage(null):145-157` 清空图像及尺寸并触发
  revalidate/repaint；Java 要求调用者管理 SWT 图像寿命。Rust 可以改为注册表管理，
  但已提供的 remove 操作必须使派生绘制快照失效，不能把已删除资源无限请求回来。
- 静态测试缺口：`m10_label_contract.rs:408-482` 只覆盖 Pending/Ready/Failed；
  该文件的 remove 用例针对字体，不覆盖 Ready ImageFigure 的 remove。
  registry/cache 各自的 Remove 测试也不能证明 Figure 到 backend 的闭环。
- 建议：为资源不存在的刷新分支定义明确的不可绘制状态，原子清除旧引用并更新
  测量、damage；不要让 backend 通过忽略资源错误掩盖失效问题。
  增加 Ready -> Removed -> prepare/submit -> 后续无旧引用且可完成呈现的验证；
  同时检查共享该资源的多个 ImageFigure。

### G3-F02 [P1] CompoundBorder 无法组合 TitleBarBorder 的测量与绘制语义

- 类别：`BUSINESS_SEMANTICS`；置信度：10/10。
- 主定位：`novadraw-scene/src/figure/border/compound_border.rs:92-98`。
- 触发：将 `CompoundBorder::new(LineBorder::new(...), TitleBarBorder::new(...))`
  安装到 Figure，注册可用字体并执行正常 Runtime prepare。把 TitleBar 放在
  outer 或进一步嵌套也不能解决。
- 根因：`Runtime::refresh_title_bar_borders:447-502` 仅检查顶层
  `get_border().title_bar()`。CompoundBorder 不传播 owner-scoped 派生测量，
  `get_insets/preferred_size` 只读取子 border 的静态指标，绘制也仅调用
  `paint_with_insets`。而 TitleBar 的静态 insets/preferred 均为零，普通 paint
  为空（`title_bar_border.rs:107-116`）；真正指标和 glyph 绘制依赖 snapshot。
- 结果：组合边框中的标题文字及背景消失，标题高度不计入 client area/preferred
  size，子内容也不为标题预留空间。这是两个已公开 builtin 的合法组合失效，
  不是要求尚未提供的新边框类型。
- 官方契约：`CompoundBorder.java:69-92,122-134` 将同一 owner 传给内外边框的
  测量与 paint，并累计 outer inset；`TitleBarBorder.java:98,130` 使用 owner
  的字体、尺寸绘制标题。组合不应丢失子边框的动态语义。
- 静态测试缺口：`m10_reusable_shape_border_contract` 覆盖普通 Compound 的
  inset/尺寸公式和 outer 状态隔离；`m10_label_contract:226-304` 覆盖直接、
  共享 TitleBar。没有 Compound + TitleBar 的组合用例。
- 建议：让 owner-scoped 测量与绘制快照支持边框组合树，累计动态 inset 并把
  相应子快照传给子 border；保留共享 border 的 owner 隔离。
  不在 recursive renderer 中增加针对某类组合的特殊分支。

### G3-F03 [P1] Label 的 North/South 将文字和图标的上下位置写反

- 类别：`BUSINESS_SEMANTICS`；置信度：10/10。
- 主定位：`novadraw-scene/src/figure/label.rs:581-589`。
- 触发：非空文字、Ready 图标，设置 `TextPlacement::North` 或 `South`；
  Runtime 刷新 presentation 后正常绘制。
- 根因与结果：North 分支使 `text_y = y + icon.height + gap`、`icon_y = y`，
  实际把文字放到图标南侧；South 恰好相反。East/West 则按文字相对图标的方向
  计算，因此同一 enum 的方向语义内部也不一致。
- 官方契约：`Label.java:179-204` 的 `calculatePlacement` 明确规定 North 的
  text 在上、icon 在下；South 的 text 在下、icon 在上。
  `setTextPlacement` 表示文字相对图标的位置，不是图标相对文字的位置。
- 直接链路：`LabelFigure::refresh_presentation:302-310` 调用 positions，
  `paint_with_icon:331-342` 直接消费这些 origin；Runtime 的 typed placement
  setter 会触发刷新。demo `novadraw-demo-scenes/src/text.rs:310-311` 也使用
  North/South，不依赖潜在的新调用方。
- 静态测试缺口：`m10_label_contract.rs:148-223` 设置 South，但只检查命令存在、
  指标和事务，没有比较 glyph origin 与 image bounds 的相对位置。
- 建议：修正两个纵向分支，并以四方向的 glyph origin、图标矩形和 gap 做断言；
  同时检查 icon named geometry 跟随 presentation，避免仅修视觉坐标。

## 4. 详细语义映射

状态说明：下列“静态一致”仅表示当前阅读没有发现该列明路径的矛盾，
不是运行验证结果，也不是整个 Java 类的完全等价声明。

### G3-M01 浮点几何与 Precision

- Java：`Rectangle.contains/intersects/scale`、`PrecisionRectangle.getBounds/scale`。
  Java 整数 Rectangle 的右下边界不包含，Precision 类型保留精度并提供整数外包围。
- Rust：`geometry/rect.rs:187-232`、`precision.rs`、`translatable.rs:152-192`；
  Point/Rectangle/Dimension 采用 f64，Precision 名称是别名，ApproxEq 显式提供容差。
- 状态：合理迁移，但不是整数几何逐方法等价。Rust contains 明确包含边界；
  intersection 需要正面积；不能仅凭类型别名就给出 Java Precision 全覆盖结论。
- 证据/局限：M1 产品测试验证导出名称及 approx-eq；Rectangle 自测明确接受边界。
  `intersects` 未显式排除零/负面积矩形，仍需补充分离正常/退化输入的契约，
  本组未建立更高层误判链路，不另报 P1。

### G3-M02 Affine 组合、点和向量

- Java：坐标指南、`Figure.translateToParent/translateFromParent` 和 Graphics 变换契约。
  局部坐标先变换到父域，点包含平移，方向/尺寸不能直接套用点的平移。
- Rust：`geometry/transform.rs:95-165` 的 multiply/post_concat/pre_concat、
  transform_point/transform_vector；Rectangle transform 取四角的保守 AABB。
- 状态：列向量 `parent * local` 的显式 API 是合理迁移；
  `then_*` 与 concat 的调用顺序应遵循各自文档，不能按名称猜测。
- 证据/局限：同文件有组合顺序、逆、点/向量测试；Mat3/Vec3 是通用数学补充，
  不是 Draw2D 3D 框架实现。奇异矩阵返回 Option，公开底层数学值不全局保证有限。

### G3-M03 Graphics state 的三种操作

- Java：`Graphics.pushState/popState/restoreState`；restore 不弹栈，pop 恢复并弹栈。
- Rust：`render/context.rs:84-111` 保存颜色、stroke、alpha、transform、clip depth；
  后端 `RenderState` 保存解释命令所需的 transform/clip。
- 状态：静态一致；样式进入绘制命令快照，后端无需重新维护全部前端样式字段。
- 证据/局限：`graphics_state_stack_restores_nested_clip_transform_and_stroke_state`
  静态覆盖上述区别；路径构建缓存不属于 state，不能声称等同于任意 Canvas 路径协议。

### G3-M04 clip 的累计、替换和恢复

- Java：`SWTGraphics.clipRect/setClip/restoreState`；clipRect 取交集，setClip 替换，
  保存状态的恢复必须能够恢复先前被替换的 clip。
- Rust：`context.rs:493-507` 的 clip_rect/set_clip/reset_clip；
  `backend/vello/mod.rs:194` 的 clip_restore_plan 及 restore_clip_layers，
  以公共前缀比较决定 pop/replay，clip 同时保存建立时的 transform。
- 状态：矩形裁剪路径静态一致，不是仅按 clip 数量恢复。
- 证据/局限：后端静态测试覆盖 reset 后恢复 outer clip、保留公共前缀；
  getClip/path clip 未提供，账本标 partial，不能外推为任意路径裁剪已支持。

### G3-M05 recursive paint 顺序和状态隔离

- Java：`Figure.paint:1250-1379`；self、client area/children、border 依次执行，
  self 临时状态不得泄漏到 children，兄弟绘制互相隔离。
- Rust：`graph/render_recursive.rs:93-143,215-263`；
  self 用内层 push/pop，children 正序，border 最后执行，节点退出恢复父状态。
- 状态：静态一致。用配对 push/pop 代替 Java 同栈帧上的 restore 是合理迁移。
- 证据/局限：本文件全文只读，没有更改主循环。完整 paint 扩展面不等于任意重写
  child traversal；Rust 刻意由引擎固定树遍历、坐标和裁剪。

### G3-M06 client area 与绘制坐标域

- Java：`coordinates.adoc`、`Figure.paintClientArea/paintChildren`；
  默认 inherited 和启用 local coordinates 的 Figure 有不同转换过程。
- Rust：`render_recursive.rs:116-124,159-192`；
  self 收到 node-local bounds，父 client clip 在 child transform 前建立；
  `FigureNode::child_transform/client_area` 提供共享几何。
- 状态：统一 node-local/parent-content 是合理迁移，不能直接复制 Java 的
  translate 调用次数。OverflowVisible 只改变指定层的裁剪策略。
- 证据/局限：外层 clip 仍由状态栈累计；本组不替代 Group 1/2 对 hit-test、
  viewport/freeform 坐标链路的完整审计。

### G3-M07 alpha、颜色与设备缩放

- Java：Graphics 的 foreground/background、alpha 和变换影响后续绘制。
- Rust：`context.rs:76-81,611-699` 在 shape/glyph/image 命令中固化 alpha；
  `backend/vello/mod.rs:138-175,1086-1107` 缩放坐标、glyph 字号与 affine 平移项。
- 状态：静态一致；后端不再次消费 SetGlobalAlpha 不是漏实现，
  重复乘 alpha 反而错误。设设备缩放为 S，当前路径等价于 `S * T * p`。
- 证据/局限：alpha 与 glyph 命令有静态测试。没有本次 GPU 截图，
  不宣称字体栅格化、抗锯齿或色彩像素完全等同 SWT。

### G3-M08 Shape 的 fill/outline 与路径

- Java：`Shape.paintFigure:113-196` 先背景色 fill，再前景色 outline；
  每类 shape 决定实际几何，Shape 本身不机械调用 Figure opaque 背景填充。
- Rust：Rectangle/Ellipse/Polygon 等消费 Runtime FigureStyle，
  NdCanvas 的 fill/draw 生成分离命令，fill_and_stroke 支持同一构建路径两次消费。
- 状态：核心 solid fill/outline 静态一致；Rust 没有完整复制 Java Shape 的
  XOR、所有 line attributes、disabled 双重浮雕绘制。
- 证据/局限：`m10_reusable_shape_border_contract` 验证 Runtime 颜色真源。
  `set_line_style(Dash/Dot)` 虽进入命令，Vello 多个分支以 `line_style: _`
  忽略它；与“高级 stroke 后置、不暴露静默 no-op”的账本说明存在边界不一致，
  不能将 M1 命令存在测试算成虚线后端验证。

### G3-M09 Ellipse、RoundedRectangle、Triangle

- Java：Ellipse 共用 optimized bounds；RoundedRectangle fill 与 outline 区分
  stroke inset；Triangle validate 依据 bounds、insets 和 direction 生成顶点。
- Rust：相应 `figure/{ellipse,rounded_rectangle,triangle}.rs` 提供绘制和几何命中；
  Runtime corner/direction 变更使用类型化事务。
- 状态：合理浮点迁移；Ellipse 的 fill/outline 使用同一优化区域，
  RoundedRectangle 补充了圆角区域精确命中，不能声称 Java 所有像素细节相同。
- 证据/局限：M10 shape 测试检查椭圆命令半径及 stroke width、圆角外点、
  三角形 bounds、负 corner metric 拒绝和错误 capability。

### G3-M10 PointList shape 的模型几何和命中

- Java：`Polyline.getBounds/setPoints/containsPoint` 从点列表派生 bounds，
  `Polygon.shapeContainsPoint` 使用闭合内部语义；不是任意拖动 bounds 的普通矩形。
- Rust：`figure/{polyline,polygon}.rs`、`geometry/point_list.rs`；
  Runtime 点编辑同步 points、bounds、damage、property notification。
- 状态：静态一致于列明正常路径；Rust 持有 Vec/值而非 Java 可变列表别名，
  通过 Runtime 发布树内修改是合理所有权迁移。
- 证据/局限：M10 测试覆盖线段距离、闭合区域、点编辑及非有限/越界失败后状态不变；
  不将几何容差或整数舍入视为逐像素等价。

### G3-M11 普通 Border 的盒模型与组合

- Java：`CompoundBorder.getInsets/getPreferredSize/paint` 累加 inset，
  preferred 为 outer 与加上 outer inset 的 inner 的逐轴最大值。
- Rust：`border/compound_border.rs:36-99` 对静态子边框实现同一公式，
  outer 绘制用 push/pop 隔离；Line/Margin/Etched/Bevel 分别提供指标与绘制。
- 状态：普通静态 border 路径一致；opaque 是 border 自身覆盖性质，
  effective opacity 还需要 Figure alpha。
- 证据/局限：M10 border 测试覆盖公式、两个 LineBorder 的偏移、
  effective opacity 与 alpha。不能从这些测试推广到动态 TitleBar，见 G3-F02。

### G3-M12 owner-scoped TitleBar 指标

- Java：`TitleBarBorder.getPreferredSize/paint` 从 owner 字体和 bounds 得出标题区域。
- Rust：`runtime.rs:447-502` shaping 后生成 `BorderSnapshot`；
  `title_bar_border.rs:59-103` 只消费 layout/metrics 绘制。
- 状态：对直接挂载、共享实例的多 owner 是合理迁移，避免把字体缓存写入共享 border。
  组合边界不完整，G3-F02 是 confirmed defect。
- 证据/局限：shared TitleBar 测试覆盖不同 owner 字体；snapshot 内部 enum
  仅有 TitleBar，不能证明外部动态边框拥有等价扩展能力。

### G3-M13 ImageFigure 与异步资源

- Java：`ImageFigure.getPreferredSize/paintFigure/setImage`；
  空图不绘制，有图时以实际图片尺寸和 alignment 定位。
- Rust：`figure/image.rs:25-155`、`graph/mod.rs:3035-3078`；
  Pending/Ready/Failed 状态和 ImageResourceRef 替代 SWT Image 对象。
- 状态：异步生命周期与资源逻辑尺寸是合理迁移；单个 Alignment 同时影响两轴，
  比 Java 可组合的水平/垂直 alignment 更窄。
- 证据/局限：已有状态测试不能覆盖 Removed，见 G3-F01；
  source rectangle 虽存在于 command 字段，产品入口只提交完整 source，账本已后置。

### G3-M14 资源 revision、backend session 与 Retry

- Java：ImageFigure Javadoc 要求 client dispose Image；Graphics 消费已持有的资源。
  Draw2D 没有等同于此处 GPU session/delta 的接口。
- Rust：`runtime/resource.rs`、`render/submission.rs:115-281`；
  namespace/id/revision、Snapshot/Delta 和 BackendSessionGate 明确隔离资源身份。
- 状态：合理平台迁移。后端按精确 `(id, revision)` 查找字体/图片；
  新 session 要 Snapshot，过期 generation 不可覆盖新基线；
  Runtime 重试把先前 delta 放回新 delta 前，保留操作顺序。
- 证据/局限：资源、submission、后端文件内测试覆盖缓存同步/重试顺序。
  分层协议自身正确仍不足以弥补 Figure 的旧快照，G3-F01 展示了这一闭环缺口。

### G3-M15 文本测量与引擎替换

- Java：`Label.calculateTextSize/getPreferredSize/getSubStringText` 使用同一字体度量，
  改变文本或字体必须使尺寸和截断缓存失效。
- Rust：`render/text.rs:479-738` 的 TextLayoutEngine/ParleyTextEngine；
  `LabelIntrinsicKey` 包含 text revision、font、engine revision、icon、placement、gap。
- 状态：合理迁移；布局在 Runtime 稳定阶段完成，paint 不重新 shaping。
  内置字体需显式注册，不引入全局字体状态；字体替换/删除重建引擎字体上下文并更新 revision。
- 证据/局限：文本文件内测试覆盖比例字体、fallback、注册失败保持旧状态；
  Label 测试覆盖缓存命中及字体失效。未运行这些测试。

### G3-M16 后端无关 Glyph IR

- Java：Label paint 通过 Graphics 绘制测量后的文本；底层平台拥有字体资源。
- Rust：`TextLayout::from_parts:297-340` 验证指标和 UTF-8 范围，
  `NdCanvas::draw_text_layout:611-667` 生成 DrawGlyphRun；
  Vello adapter 消费 face revision、glyph id/position、font size，不重新解析 raw string。
- 状态：合理迁移且具真实外部引擎入口，不只是 trait 名称。
- 证据/局限：`m10_text_extension_contract` 的外部 TextLayoutEngine 构造非空 layout，
  RecordingBackend 消费 glyph 命令。该测试不验证 GPU 呈现；
  外部实现仍需保证 glyph id 与所注册字体实际匹配。

### G3-M17 Label 测量、截断、alignment 与 placement

- Java：`Label.calculatePlacement/calculateAlignment/getSubStringText`；
  placement 是文字相对图标的方向，alignment 是组合和横向/纵向内部对齐。
- Rust：`figure/label.rs:208-320,456-603`；
  intrinsic 与 presentation 分开，宽度不足时按 grapheme 前缀加省略号。
- 状态：缓存/Unicode 边界是合理迁移；North/South 存在 G3-F03。
  enum 仅保留四个轴向 placement，不能宣称覆盖 Java 全部 position 组合。
- 证据/局限：静态测试验证初帧经过父布局后 shaping、缓存、截断 metadata；
  缺少四方向相对位置断言。完整 TextFlow fragment/bidi 是明确后置，
  不意味着 Parley 完全不具备文本 shaping 能力。

### G3-M18 Clickable 输入与激活事务

- Java：`ClickableEventHandler`、`ButtonModel.setArmed/setPressed/setEnabled`、
  `ToggleModel`；有效的按下/释放序列产生 action，toggle 先变更 selected。
- Rust：`figure/widget.rs:173-247`、`graph/mod.rs:3200-3231`；
  pointer 与 keyboard pressed 分开，匹配 key release，失焦取消，
  activate 检查 effective enabled，并先发 selected property 再发 action。
- 状态：核心输入事务静态一致；Runtime 持有 capture/focus 等输入状态，
  Figure 消费派生 visual snapshot，是明确的所有权迁移。
- 证据/局限：widget 测试覆盖 drag-out/back、错配键/鼠标释放、disabled、
  toggle 顺序及事务边界通知。本组不重复审计通用事件分发器。

### G3-M19 widget 绘制与模型扩展

- Java：`Clickable.paintBorder/paintClientArea:293-325` 用 focus ring、
  armed/selected 偏移表现状态，允许替换 ButtonModel 和 event handler。
- Rust：`figure/widget.rs` 的 ClickableModel + LabelFigure 组合，
  Button/Toggle 提供边框、偏移、selected/focus/disabled 视觉；
  `runtime.rs:3435-3452` 派生视觉状态。
- 状态：已选 builtin 语义是合理迁移；ClickableBehavior 返回具体 ClickableModel，
  并不等价于任意替换 Java ButtonModel 子类。程序化激活统一通过 Runtime 事务，
  也不是机械复制 Java `Clickable.doClick` 仅直接发 action 的实现。
- 证据/局限：按键、模型和 action 静态测试有覆盖；repeat firing、ButtonGroup、
  radio/checkbox/Slider 明确后置。截图美术等价不是本次静态结论。

### G3-M20 平台能力、可访问性与 tooltip 边界

- Java：Graphics 是平台绘制边界；Figure/Clickable 提供可访问与交互语义，
  不能仅凭原生平台控件名称推断渲染后端功能。
- Rust：`RenderBackend/BackendCapabilities` 声明 glyph/image/projective 能力；
  R8 测试验证 projective 显式拒绝和将外部 3D frame 作为普通 Image 嵌入。
  widget/Label 提供 AccessibleFigure 信息，不直接调用平台服务。
- 状态：能力检查和图像嵌入是合理边界，不等于已实现原生 3D 合成。
- 证据/局限：本组静态读了 accessibility/tooltip 测试，覆盖语义快照、
  action 复用、移除、祖先 tooltip 和 press-hide；
  对应控制器完整实现归 Group 1，本组不将测试意图当作平台桥接已验证。

## 5. 合理迁移、收窄与后置

### 合理迁移

- f64 几何、值类型/ID 引用、显式 affine 次序，替代 Java 整数与对象别名；
  浮点边界语义应独立定义，不按同名类型认定等价。
- 固定的递归 traversal 与分层 push/pop 保持 paint 协议；
  Renderer 负责坐标/clip，Figure 负责具体绘制，不能为修复 builtin 添加主循环特例。
- Runtime shaping + 不可变 Glyph IR + revision 资源快照；
  直接 TitleBar 的 owner-scoped snapshot 避免共享实例相互污染。
- 输入状态归 Runtime、模型 selected 归 widget、程序化激活走相同事务。

### 明确收窄或需要明确的边界

- 不开放 Figure 任意替换 child traversal；Label 的单行和四向 placement、
  ImageFigure 单一 Alignment、具体 ClickableModel 均不等于 Java 全部可替换面。
- BorderSnapshot 只有私有 TitleBar 变体；外部 border 可以绘制和给静态指标，
  但没有同等通用的 Runtime 文本测量快照协议。G3-F02 不是单纯能力收窄，
  因为现有公开组合入口已经产生错误结果。
- `LineStyle` 进入 public command 却被 Vello 忽略，与账本后置声明不吻合；
  应收敛公开契约或提供显式不支持结果，不能称为已验证的虚线。
- `Path::arc_to` 暴露 Arc 操作，而当前 Vello path match 没有对应 lowering；
  NdCanvas 的 arc 是固定分段近似，方向参数没有被消费。通用路径“verified”
  不应外推到任意 arc。这里作为覆盖限制记录，不扩展本组最多三条 JSONL。

### 已声明后置，不伪装为当前缺陷

- clip 查询/path clip、gradient、高级 dash/miter/antialias/XOR；
  其中已暴露但静默忽略的部分必须与上一节区分。
- 图片 source rectangle 产品入口、完整 TextFlow fragment/bidi；
  不能据此否定当前 Glyph IR/Parley 已提供的 shaping 与宽度约束。
- repeat scheduler、ButtonGroup、radio/checkbox/Slider、projective composition。

## 6. 扩展性、复杂度与 Rust API

- 可替换边界有实际证据：TextLayoutEngine、RenderBackend、Figure/Border 等；
  外部文本引擎可以构造经过验证的非空 TextLayout，后端不依赖 Parley 类型。
- 仍有具体类型限制：Figure 的 label capability 返回 LabelFigure，
  Image 刷新按 ImageFigure downcast，ClickableBehavior 返回 ClickableModel，
  BorderSnapshot 特化 TitleBar。它们是目前的扩展范围，不应对外承诺与
  IFigure/Border/ButtonModel 继承扩展面完全相同。
- 普通 paint 树遍历为 O(N)，本地 style 继承不要求每个节点反查祖先。
  但后端 push/restore 保存 clip 向量，累计复制量可能随
  `sum(node depth)` 增长；没有本次 benchmark，不把深树成本猜测升级为性能缺陷，
  也不建议借此改回迭代渲染。
- Label/Image 刷新扫描节点，TitleBar 刷新遍历 descendants 并解析 owner style；
  layout cache 避免重复 shaping，不消除扫描及祖先解析成本。
  应区分“paint style 路径已线性化”与“整个准备阶段都是 O(N)”。
- 截断按 grapheme 分割后进行二分，最多 O(log G) 次候选 shaping，
  每次还包含前缀构造与实际文本布局成本，不能把总成本直接写成 O(log G)。
- typed Runtime mutation、Result/Option、私有 TextLayout 字段与显式 revision
  降低非法状态传播；公开几何字段和独立构造期 builder 不保证输入全局有限。
  无具体调用路径的非法数字、颜色字符串 panic 等仅属防御性边界，不报高等级缺陷。

## 7. 七维度判断与验证缺口

| 维度 | 判断 |
| --- | --- |
| 逻辑 | G3-F01 为资源派生状态失效遗漏，导致合法 remove 后整帧持续 Retry。 |
| 业务语义 | G3-F02 破坏动态 Border 组合；G3-F03 破坏文字相对图标的方向契约。 |
| 并发 | 当前阅读链路以 Runtime 可变借用和提交身份管理时序，未建立新的数据竞争缺陷；不等于审计了 GPU/平台内部同步。 |
| 健壮性 | 已有 typed mutation、字体校验和 UTF-8 检查；缺少前置条件但无现有触发链路的情况不升级为 P1。 |
| 性能 | 记录 clip 栈复制、刷新全扫描及多次 shaping 的成本边界；未做基准，不声称性能门禁通过或失败。 |
| 安全 | 在本组路径未确认可利用安全缺陷；没有对字体/图片第三方解码器做完整安全审计。 |
| 质量 | 静态测试较系统，但缺 Removed Image、Compound+TitleBar、四方向 Label 几何断言；纯风格问题不列缺陷。 |

优先补充的验证由主审或后续修复阶段执行：

1. Ready ImageFigure 的资源移除，检查下一 submission 不再引用旧 revision，
   并可结束 Retry；同时验证多个 owner、资源移除与待提交 delta 的顺序。
2. TitleBar 作为 Compound 的 inner/outer、嵌套组合、同一共享组合的不同字体 owner，
   检查动态 inset、preferred、clip、背景和 glyph 命令。
3. Label 四向 placement 的 text/image 相对坐标及 gap；同步检查 icon named geometry。
4. 后置或部分覆盖的 style/path API 先明确支持边界，再选择后端像素验证，
   不以“生成了命令”作为 Vello 实现语义的替代证据。

本组没有写产品或测试文件，没有执行 Cargo，没有提交 Git，
没有修改 `render_recursive.rs`，仅产出 `group_3.md` 和 `group_3.jsonl`。
