# Group 3: Graphics、几何、文本、资源与可复用 Figure 语义审计

日期：2026-09-15。范围：`full_file`，不是仅审查 diff。

## 结论与证据边界

- 本组提交三项可证实的条件性功能缺陷：复合边框丢失 TitleBar、Label North/South 反向、Vello 丢弃公开线型。均为 P1、置信度 10/10，无 P0。
- 状态栈、矩形 clip 回放、绘制阶段顺序、glyph IR 与资源因果主链路有实现和已有测试断言支持；不能据此宣称全部 Graphics API、全部 Border 组合或原生无障碍已等价。
- 本组没有运行 cargo、GPU、截图、浏览器或性能测试，没有修改实现、测试、账本或路线图。
- 主 Agent 告知 workspace 测试 **644 通过、2 忽略**。这是主 Agent 转述的执行结果，本组未独立复跑或核对日志；不等于以下缺口场景已经被测试覆盖。
- 已知并行改动涉及 `novadraw/src/lib.rs`、`connection/{runtime,locator}.rs`、`runtime/runtime.rs`、M9 tests。三项缺陷均定位到本组稳定文件；连接具体渲染、路由、locator 由 Group 4 负责。

### 基线与路径

- Novadraw：`6b83ac0fa980dc60fd1284607a51460858fa883b` 加当前工作区。
- Draw2D/GEF：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
- 下文 `J/` = `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/`。
- 下文 `G/` = `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef/src/org/eclipse/gef/`。
- Rust 路径相对 Novadraw 根；JSONL 的 `file` 相对 Git 根，包含 `engine/wasm_rust/novadraw/`。
- 设计入口：ADR-014、ADR-015，`doc/parity/draw2d/api-coverage.md`，`doc/design/architecture/{text-layout,resource-lifecycle,reusable-shape-border,basic-widgets,tooltip-accessibility}.md`。
- 只使用 Draw2D/GEF 指定包作为参考；没有读取 Zest 或 `doc/archive/`。

### 证据等级

`S`：实际读取目标方法、实现及直接调用链后静态确认。
`T`：实际读取已有测试断言，**本组未运行**。
`U`：尚无此次执行证据或缺少针对性断言。
“合理变体”只限该行明确描述的范围，不豁免同一 family 的其他缺口。

## 三项确认缺陷

### G3-01：CompoundBorder 内的 TitleBarBorder 不测量、不绘制

- 类型：条件性功能缺陷；P1；10/10；边框语义。
- 位置：`novadraw-scene/src/figure/border/compound_border.rs:36-47,82-101`；`title_bar_border.rs:110-119`。
- 触发：注册 Inter 后，把 `CompoundBorder::new(LineBorder::new(Color::BLACK, 2.0), TitleBarBorder::new("Title", Color::BLUE))` 挂到 RectangleFigure，准备正常首帧。
- 根因：Runtime 的 `refresh_title_bar_borders` 只查询最外层 `Border::title_bar()`。CompoundBorder 没有递归测量协议，返回默认 None；其 metrics 只调用子 Border 的无上下文方法。TitleBar 的这些方法返回零指标，`paint` 是空实现，普通 `paint_with_insets` 最终仍调用该空实现。
- 结果：只有外层线框，缺少标题背景和 glyph；top inset 只剩线框宽度，布局也没有为标题保留高度。outer 为 TitleBar 或多层嵌套也存在同类问题。
- 参考：`J/CompoundBorder.java:69-95,122-134` 将 owner 传递给子边框；`J/TitleBarBorder.java:69-71,105-107,140-163` 依赖 owner 字体测量并绘制标题。
- 已有证据：`shared_title_bar_border_keeps_metrics_per_owner` 只测两个 owner 直接共享 TitleBar；`compound_border_isolates_outer_state_and_offsets_inner_paint` 只组合 LineBorder。没有覆盖 TitleBar 的组合。
- 建议：建立可组合、owner-scoped 的 Border 测量/绘制快照，递归组合 metrics 与累计 insets；不要只透传一个 TitleBar 指针，那无法表达多个标题和嵌套偏移。验收 outer/inner/nested TitleBar、不同字体双 owner、字体替换和边框替换。

### G3-02：Label 的 North/South 文本位置与契约相反

- 类型：条件性功能缺陷；P1；10/10；文本布局语义。
- 位置：`novadraw-scene/src/figure/label.rs:614-626`。
- 触发：Label 有 Ready 图标和非空文本，调用 `Runtime::set_label_text_placement(..., TextPlacement::North)` 或 `South`，再准备帧。
- 根因：`positions` 的 North 分支令 `text_y = y + icon.height + gap`、`icon_y = y`，实际把文本放在图标下方；South 恰好反过来。East/West 分支按文本相对图标方向实现，不能解释成另一套方向约定。
- 参考：`J/Label.java:177-207` 中 North 为文本在上、图标在下；South 为图标在上、文本在下。
- 已有证据：`label_icon_gap_placement_and_typed_mutations_are_transactional` 虽设置 South，但仅断言 Image/Glyph 命令存在和图标尺寸，未比较二者 y 顺序。
- 建议：修正垂直 placement 的两个偏移分支。验收四方向、非零 gap、不同图标/文本高度、border inset 和各 alignment；检查命令 origin 或布局快照，而非只检查命令存在。

### G3-03：公开 Dash/Dot 线型在 Vello 被静默绘成实线

- 类型：条件性功能缺陷；P1；10/10；Graphics 命令消费。
- 位置：`novadraw-render/src/backend/vello/mod.rs:662-693`；相同模式还在 Line、Polyline、Ellipse、StrokePath 分支。
- 触发：自定义 Figure 在 paint 中 `set_line_style(LineStyle::Dash)` 或 `Dot`，随后调用 `draw_rectangle`、`line`、`polyline` 或 `stroke`。
- 根因：NdCanvas 将线型写入命令，但 Vello 解构为 `line_style: _`，构造没有 dash 配置的 Stroke。`required_capability()` 也不区分该能力，正常提交不会以 Unsupported 拒绝。
- 结果：请求非实线却得到实线；`m1_graphics_shape_and_style_entries_emit_commands` 已证明 Dash 是公开可达输入，但只断言 IR 字段，不能证明 backend 消费正确。
- 参考：`J/SWTGraphics.java:319-328,1293-1348` 把线型/dash 应用到 GC。
- 边界：自定义 dash 数组、dash offset 等已明确延后，不要求本轮实现；**已经公开的 Dash/Dot 静默失效**不能按“未提供高级 API”豁免。
- 建议：明确 Solid/Dash/Dot 语义并实现一致的 stroke lowering，或在提交前结构化拒绝不支持的线型；补默认 Vello 的实际编码/像素断言，覆盖全部 stroke primitive。

## 语义映射

| # / Family ID | Java 类/方法与源码行 | 不可丢失语义 | Rust 公共入口与源码行 | 判定 | 测试名称与证据等级 |
|---|---|---|---|---|---|
| 01 `graphics.context` | `J/Graphics.java:779-793` push/pop/restore；`J/SWTGraphics.java:887-916,966-1008` | restore 不弹栈；恢复 transform、clip、颜色和线参数 | `novadraw-render/src/context.rs:84-109` `NdCanvas::{push_state,restore_state,pop_state}`；backend `mod.rs:549-578` | 合理变体：颜色/线参数在录制时冻结，backend 栈只保存 transform/clip；空栈操作是安全 no-op | S+T：`graphics_state_stack_restores_nested_clip_transform_and_stroke_state`；U：空栈与所有 backend 组合 |
| 02 `graphics.context` | `J/SWTGraphics.java:373-382,1206-1208,1424-1462` | clipRect 取交集，setClip 替换；已有 clip 不随之后 translate 漂移 | `context.rs:493-507` `clip_rect/set_clip/reset_clip`；backend `mod.rs:190-204,598-615` 保存 clip 当时 transform、按公共前缀重放 | 矩形范围合理变体；getClip/path clip 后置 | S+T：`clip_restore_plan_replays_saved_outer_clip_after_reset`、`clip_restore_plan_keeps_the_common_prefix`；U：旋转 clip 像素 |
| 03 `graphics.context` | `J/Graphics.java:795-824` rotate/scale；`J/geometry/Transform.java:56-93` | 明确变换顺序和角度单位 | `context.rs:114-153`；`novadraw-geometry/src/transform.rs:91-125` `post_concat/pre_concat` | 合理变体：NdCanvas rotate 使用弧度，Graphics.rotate 使用度；parent * local | S+T：`concat_transform_uses_parent_times_local_order`、`post_concat_keeps_parent_scale_outside_child_translation` |
| 04 `graphics.context` | `J/Graphics.java:827-834` setAlpha | alpha 随状态恢复且只应用一次 | `context.rs:76-78,611-700`；`novadraw-core/src/color.rs:10-125` | 合理变体：0..1 浮点，颜色/图像 alpha 已烘焙；SetGlobalAlpha 在 Vello 不再二次乘 | S+T：`global_alpha_is_scoped_and_applied_to_shapes`、`canvas_records_positioned_glyph_runs_with_scoped_alpha` |
| 05 `graphics.context` | `J/SWTGraphics.java:319-328,1293-1348` | 显式线型应被实际绘制或拒绝 | `context.rs:603-609` `set_line_style`；backend `mod.rs:662-693,704-781,932-958` | **缺口 G3-03**；cap/join 和本次新增 miter=4 已消费，不代表 dash 已消费 | S+T：`m1_graphics_shape_and_style_entries_emit_commands` 仅 IR；U：dash/dot backend |
| 06 `graphics.context` | `J/SWTGraphics.java:460-463,603-606` drawPath/fillPath | 路径段与 fill/stroke 不可静默丢失 | `context.rs:355-491`；`command.rs:153-188,261-329` `Path`、`Path::arc_to`；backend `mod.rs:855-1034,1084-1085` | 部分：Move/Line/Quad/Cubic/Close 有消费；通用 `Path` variant 和 `PathOp::Arc` 落空分支。详见缺口清单 | S；U：全命令集覆盖。RoundedRectangle 走 Quad，不证明 Arc |
| 07 `graphics.context` | `J/Graphics.java:146-159` drawImage source/destination | source crop、destination scale、资源身份 | `context.rs:668-690` `draw_image/draw_image_with_size`；backend `mod.rs:1042-1081` | 部分：完整 source 有消费；`src_rect: Some` 直接跳过。裁源产品 API 后置，但公开 IR 需显式拒绝 | S+T：`draw_image_records_destination_and_alpha_snapshot`；U：裁源像素 |
| 08 `paint.protocol` | `J/Figure.java:1250-1269,1328-1353` | self -> children -> border；self state 不污染后两阶段 | `novadraw-scene/src/figure/mod.rs:399-407,472-496`；`graph/render_recursive.rs:93-143,159-192` | 核心顺序等价；固定引擎遍历替代 Java 可覆盖 paintClientArea 是有意收窄 | S；U：本次未重跑 paint 顺序契约 |
| 09 `clipping.strategy` | `J/Figure.java:1296-1316` paintChildren | child order、visible、clip policy、兄弟隔离 | `figure/mod.rs:117-127` `ChildClippingStrategy`；`render_recursive.rs:215-262` | 部分：三种 enum policy；没有任意多矩形 provider；持续保留 parent client clip 是保守变体 | S；U：多矩形 provider、复杂溢出像素 |
| 10 `geometry.primitives` | `J/geometry/Rectangle.java:145-185,737-782` | contains、空矩形、intersection 的边界必须明确 | `novadraw-geometry/src/rect.rs:199-244` `contains/intersects/intersection` | 部分：contains 改为闭边界且有测试；intersects 缺显式空尺寸排除，不能称完整等价 | S+T：`test_contains`、`test_intersects`；U：零宽/负宽相交 |
| 11 `geometry.primitives` | `J/geometry/Rectangle.java:935-943` scale；`J/geometry/Transform.java:81-93` | 精度策略、旋转后的外包矩形 | `geometry/lib.rs:28-52` `Precision*`；`translatable.rs:163-207`；`transform.rs:153-162` inverse | 合理变体：统一 f64、四角 AABB、不可逆返回 None；不是 Java integer rounding 的逐像素复制 | S+T：`m1_precision_aliases_keep_approx_eq_contract`、`rectangle_affine_transform_returns_conservative_aabb` |
| 12 `geometry.primitives` | `J/geometry/PointList.java:113-130,287-305,487-503` | 有序点、变换、空列表、polyline/polygon 算法 | `geometry/point_list.rs:16-126` `PointList`；`figure/polyline.rs:312-330`；`figure/polygon.rs:131-160` | 部分：基础存储为自有 Vec；empty bounds=None；距离/包含下沉到 Figure helper，公共 PointList 未覆盖 Java 算法族 | S+T：`bounds_returns_minimum_rectangle_containing_all_points`、`empty_bounds_returns_none`、`transformed_returns_new_point_list` |
| 13 `geometry.primitives` | `J/geometry/Transform.java:56-93` | 点/向量旋转必须有一致坐标约定 | `geometry/vec2.rs:78-92` `Vec2::rotate`；`novadraw-math/src/mat3.rs:107-131`；`vec3.rs:52-69` | 部分：Mat3/Transform 为标准列向量；Vec2.rotate 正角符号相反，不能作为同义适配使用；3D 向量是额外能力 | S+T：Mat3 `test_rotation`；U：Vec2 与 Transform 互操作、零向量 normalize |
| 14 `geometry.primitives` | `J/SWTGraphics.java:460-463` 保留完整 path 绘制 | 用于 damage 的 bounds 至少覆盖曲线 | `novadraw-render/src/command.rs:342-390` `Path::bounding_box` | 缺口：只累积曲线端点，不含极值；目前读到的直接调用为直线三角形测试，不升级为线上 damage 缺陷 | S+T：`triangle_uses_draw2d_resize_and_centering_geometry` 仅直线；U：曲线包围盒 |
| 15 `builtin.figures` | `J/Shape.java:113-155`；`J/RectangleFigure.java:29-50` | fill 先于 outline；stroke 向内；颜色来自统一 style | `figure/mod.rs:789-857` `Shape`；`figure/rectangle.rs:154-169,236-264` | 有限等价：构造色决定 fill/outline 开关，绘制色消费 NdCanvas；通用 Shape mutation 未齐，disabled emboss 未迁移 | S+T：`reusable_shapes_consume_runtime_figure_style_as_color_truth` |
| 16 `builtin.figures` | `J/Ellipse.java:39-90` | 椭圆精确命中；fill/outline 共用 optimized bounds | `figure/ellipse.rs:213-222,286-301` `EllipseFigure::precise_hit` 与 Shape 实现 | 合理变体：浮点内缩替代 floor/ceil，不声明 SWT 光栅一致 | S+T：`ellipse_fill_and_outline_share_optimized_bounds_and_preserve_stroke_width` |
| 17 `builtin.figures` | `J/RoundedRectangle.java:42-63,73-86` | 二维圆角、边界约束与 stroke 内缩 | `figure/rounded_rectangle.rs:129-135,197-224,342-417` | 部分：二维尺寸与精确椭圆角命中已实现；paint 用角点控制的二次曲线，未证明与椭圆角命中逐点一致 | S+T：`rounded_rectangle_precise_hit_rejects_clipped_corner`；U：大圆角 paint/hit 轮廓一致 |
| 18 `builtin.figures` | `J/Polyline.java:44-77,94-95,140-144` | 点为真值，distance hit、路径包围盒和 mutation 同步 | `figure/polyline.rs:243-260,376-438`；`figure/mod.rs:576-581` | 部分：segment 距离和退化段可处理；normalizer 只扩 stroke/2，未计 cap/join/miter；不把连接组修复当本类已修 | S+T：`polyline_precise_hit_uses_segment_distance`、`runtime_point_mutations_commit_bounds_points_damage_and_notification_atomically`；U：锐角 miter envelope |
| 19 `builtin.figures` | `J/Polygon.java:38-67` | 闭合 fill/outline、interior/edge 命中 | `figure/polygon.rs:131-160,257-300` | 部分：显式 Close 与 edge 命中；hit 用奇偶规则而 FillPath 固定 NonZero，自交多边形未证明一致 | S+T：`polygon_precise_hit_uses_closed_interior` 仅普通三角形；U：自交/重复绕行 |
| 20 `builtin.figures` | `J/Triangle.java:99-150` validate | client box、resize(-1,-1)、方向与居中 | `figure/triangle.rs:229-331,417-423,454-464` | 合理增强：精确三角形 hit 替代矩形；退化共线输入的 hit 仍需验证 | S+T：`triangle_uses_draw2d_resize_and_centering_geometry`；U：零面积与极粗描边 |
| 21 `border.protocol` | `J/Border.java:19-67` | border 可复用；insets、preferred、opaque 独立 | `figure/border/mod.rs:72-134` `Border`；`line_border.rs:70-119`、`margin_border.rs:130-151` | 合理变体：Arc + Send/Sync，只读配置；Line 内描边、Margin 不绘制。plain Border 缺 owner-aware measure 扩展 | S+T：`border_metrics_and_compound_formula_match_draw2d`、`border_preferred_size_and_effective_opacity_join_figure_protocol` |
| 22 `border.protocol` | `J/CompoundBorder.java:69-95,114-134` | 累加 inset；max preferred；outer state 隔离 | `figure/border/compound_border.rs:36-102` `CompoundBorder` | 普通无文本组合等价；缺任一边 opaque=false；**文本组合缺口 G3-01** | S+T：`compound_border_isolates_outer_state_and_offsets_inner_paint` |
| 23 `border.protocol` | `J/TitleBarBorder.java:69-71,105-107,140-163` | owner font 驱动 top inset/preferred，paint 使用同一测量 | `figure/border/title_bar_border.rs:59-105`；`border/mod.rs:27-65` `BorderSnapshot`；Runtime `refresh_title_bar_borders` | 部分：直接挂载、双 owner 隔离成立；快照 enum 封闭且不可组合；**G3-01** | S+T：`title_bar_border_uses_resolved_font_metrics_and_glyph_commands`、`shared_title_bar_border_keeps_metrics_per_owner` |
| 24 `border.protocol` | `J/Border.java:45-67` opaque 与 border ring | 新装饰应复用通用命令，不改 renderer | `figure/border/bevel_border.rs:44-128`、`etched_border.rs:17-60` | 合理变体：明暗线配色显式注入；未机械复制平台主题；RectangleBorder 是 LineBorder 类似产品 API | S+T：`etched_and_bevel_borders_expose_product_metrics_and_commands` |
| 25 `text.flow` / `graphics.context` | `J/TextUtilities.java:22-57,92-126`；`J/Label.java:230-232,344-369` | 测量和绘制同源；真实字体，不伪造字符平均宽 | `novadraw-render/src/text.rs:479-496,565-700` `TextLayoutEngine/ParleyTextEngine`；Runtime `with_text_layout_engine/layout_text` | 合理变体：Runtime 独占 object-safe 服务；系统字体禁用；字体注册独立于 backend | S+T：`proportional_font_measurement_is_not_character_count_estimation`、`registered_cjk_font_falls_back_when_the_requested_font_lacks_glyphs` |
| 26 `text.flow` / `graphics.context` | `J/text/TextFlow.java:641-653` paintText | 完成 shaping 后的布局可交付给绘制层 | `text.rs:178-220,233-340` `FontFaceRef/GlyphRun/TextLayout::from_parts`；`context.rs:611-665`；backend `mod.rs:133-178` | 合理变体：自有 IR，无 Parley/Vello 公开 layout 字段；不可变受校验构造；非空第三方引擎可接入 | S+T：`external_text_engine_constructs_non_empty_backend_neutral_layout`、`external_layout_parts_reject_invalid_metrics_and_utf8_ranges` |
| 27 `graphics.context` | `J/SWTGraphics.java:460-463` 路径消费；`J/text/TextFlow.java:645-653` text layout 消费 | glyph ID、定位、font revision、fill/stroke 必须消费 | `backend/vello/mod.rs:133-178,1015-1035` `append_glyph_run`；`text.rs:224-230` `GlyphPaint` | 静态闭合：Vello 不重新 shaping；font_size/origin 按 DPI 映射；缺 font 返回路径另见资源行 | S+T：`positioned_glyph_run_is_encoded_into_the_vello_scene` 断言 encoding 非空，非像素证明；U：stroke glyph 视觉、bidi/variable font 全量 |
| 28 `builtin.figures` | `J/Label.java:125-139,297-333,344-369` | text/icon/gap、preferred/minimum、grapheme-safe ellipsis | `figure/label.rs:207-329,473-548` `LabelFigure`；Runtime label setters | 部分：双缓存、真实 shaping、grapheme 前缀；空文本 icon-gap 和全部 Unicode 场景未验，不据 ASCII 断言判全覆盖 | S+T：`label_uses_runtime_shaping_for_measurement_truncation_and_paint`、`label_cache_reshapes_only_when_measurement_inputs_change` |
| 29 `builtin.figures` | `J/Label.java:177-207` calculatePlacement | placement 表示文本相对图标的方向 | `figure/label.rs:574-642` `positions`；Runtime `set_label_text_placement` | **缺口 G3-02**：North/South 反向；East/West 对应 | S+T：`label_icon_gap_placement_and_typed_mutations_are_transactional` 不验证相对 y |
| 30 `text.flow` | `J/text/TextFlow.java:557-653,696-701` | fragment、baseline、bidi、换行高度参与 layout | `figure/mod.rs:320-355,423-436` `MeasureConstraints/FigureMeasurement`；`text.rs:593-700` | 部分/后置：有约束 layout 与 glyph 基础，不等于完整 TextFlow、富文本、caret/IME | S+T：`width_constraint_breaks_text_into_multiple_lines`；U：本组未独立复核 D4 parent arrange 扩展测试 |
| 31 `resource.lifecycle` / `builtin.figures` | `J/ImageFigure.java:20-28,84-116,145-160` | image 尺寸改变 revalidate/repaint；资源由明确 owner 管理 | `figure/image.rs:26-91,118-160` `ImageFigure`；`runtime/resource.rs:216-269` | 合理变体：typed ID、Ready revision、Pending/Failed 确定性视觉；图片只引用 resource。alignment 仅同一 Start/Center/End 同时作用两轴，非 Java 任意横纵组合 | S+T：`image_figure_tracks_pending_ready_and_failed_resource_states`、`decodes_png_and_svg_into_rgba` |
| 32 `resource.lifecycle` | `J/ImageFigure.java:145-160`；`J/Graphics.java:869-929` 引用期间资源有效 | 因果顺序、复用身份隔离、retry 不倒置 | `runtime/resource.rs:240-310,325-362` `ResourceRegistry`；`submission.rs:172-230` `ResourceDelta/ResourceSync`；backend `mod.rs:1477-1556` cache sync | 合理扩展：namespace + generation、Arc payload、有序 ops、snapshot replacement；不存在 Java 同名 session API | S+T：`ready_failed_ready_preserves_backend_operation_order`、`ready_snapshot_is_stable_and_supersedes_pending_ops`、`resource_snapshot_replaces_a_partially_applied_cache` |
| 33 `render.backend_session` | `J/SWTGraphics.java:388-396` dispose；`J/ImageFigure.java:20-28` owner 契约 | backend 重建后必须重建资源/场景；旧确认不可接管新 session | `submission.rs:31-78,247-280` `BackendSessionGate/Id`；Runtime `reset_backend_session/prepare_submission_state/complete_submission` | 合理扩展：单 consumer/in-flight，Snapshot 首帧、同 namespace generation；gate 自身不验证 Full，依赖 Runtime 前置保证；并发 handoff 明确后置 | S+T：`backend_session_gate_requires_snapshot_baselines_and_rejects_stale_generations`；U：本组未复跑 reset/retry 集成、真实 device loss |
| 34 `widgets.basic` / `notification.action` | `J/ClickableEventHandler.java:56-131`；`J/ToggleModel.java:29-32` | press/release-inside、drag-out/back、matching key、toggle 先 selected 后 action | `figure/widget.rs:174-246,271-315,406-433` `ClickableModel/Behavior/ButtonFigure/ToggleFigure` | 合理变体：持久 selected 与 Runtime transient 分离，Figure visual 为快照；基础模型扩展限定 Push/Toggle；repeat、ButtonGroup 后置 | S+T：`drag_out_cancels_action_and_drag_back_rearms_it`、`keyboard_activation_requires_the_matching_release_key`、`toggle_changes_selection_before_emitting_action` |
| 35 `accessibility.bridge` | `J/AccessibleBase.java:19-40`；`J/LightweightSystem.java:378-524`；`G/AccessibleEditPart.java:20-114` | identity、name/role/state/bounds/children/focus/action | `figure/mod.rs:745-770` `AccessibleFigure`；`runtime/accessibility.rs:18-88,157-205,291-370`；Runtime `perform_accessibility_action` | 合理变体：stable snapshot/delta、namespaced ID、装饰节点提升；并非完整原生 AT 或 GEF accessible provider | S+T：`snapshot_promotes_accessible_children_through_decorative_figures`、`focus_and_default_actions_reuse_runtime_widget_transactions`、`dispose_removes_accessibility_identity_in_the_next_delta` |
| 36 `accessibility.bridge` / `builtin.figures` | `J/ImageFigure.java:20-31`；`G/AccessibleEditPart.java:80-114` 语义 adapter | 图像有明确 name 时可进入无障碍树 | `figure/image.rs:17-23,109-160` 没有 accessible-name 字段或 AccessibleFigure 实现；`figure/label.rs:432-440` 用完整源文本 | 部分/缺口：Label/Button/Toggle 有内置能力；命名 ImageFigure 的设计承诺没有对应内置 public API，不能用 role enum 存在证明完成 | S；U：图片 name/action；原生 VoiceOver provider 后置 |

表中 `context.rs`、`command.rs`、`text.rs` 均在 `novadraw-render/src/`；`geometry/` 指 `novadraw-geometry/src/`；`figure/`、`graph/`、`runtime/` 指 `novadraw-scene/src/`。`backend/mod.rs` 行号在表中均明确写为 `backend/vello/mod.rs` 或以上下文的 Vello backend 指代。

## 未纳入三项缺陷上限的差异、风险与后置

### 静态已见的其他语义缺口

以下不是“测试通过所以不存在”的项目，也没有因此自动获得合理迁移豁免。本组 JSONL 按 brief 限制仅保留上述三项，合并审计时仍应保留这些映射差异：

- 通用 `RenderCommandKind::Path` 没有 Vello 消费分支；`PathOp::Arc` 在 fill/stroke 中被忽略。`NdCanvas::arc` 又是另一实现：固定八段折线、忽略 anticlockwise，并非解析曲线。没有运行曲线像素复现。
- `Image.src_rect: Some` 被跳过。裁源公开产品入口明确后置，但 IR 已有字段；建议实现前先定义 Unsupported，避免对可构造 IR 假成功。
- `Path::bounding_box` 不覆盖 Bézier 内部极值。例如 `(0,0) -> quad control (0,100) -> (10,0)` 返回的 y 范围为零，实际曲线中点 y=50。当前所读直接调用只有直线三角形测试，**未证实它进入运行期 damage**。
- `Rectangle::intersects` 对内部零宽矩形可以返回 true，与自身 `is_empty` 和 Java 正尺寸前置语义不一致；`Vec2::rotate(PI/2)` 将 X 变为 -Y，而 Transform/Mat3 将 X 变为 +Y。均未据此推断主渲染链已受影响。
- Point-list normalizer 只扩展 stroke/2，不读取 cap/join；RoundedRectangle 的二次角曲线与椭圆 hit、Polygon 的 NonZero fill 与奇偶 hit 均需专项一致性验收。未运行相应像素或命中反例，不能把普通三角形测试当覆盖。
- LineBorder/RectangleBorder 的 `with_style` 保存 `BorderStyle`，paint 没有消费它；这与 G3-03 的 backend 丢弃发生在不同层，修 Vello 不会自动修复 Border 层。
- BorderSnapshot 只有内部 TitleBar variant；第三方 owner-font-dependent Border 没有与 TextLayoutEngine 等价的受校验快照构造/测量入口。接口对象安全不等于这类扩展已闭合。
- `ImageFigure` 未提供设计中承诺的 accessible name/capability。用户可包装自定义 Figure，不等于内置 ImageFigure 已交付该能力。

### 理论风险与尚未验证

- 公共 Color/geometry/IR/payload 可构造非有限值、无效尺寸或错误字体引用。`Color::hex` 对短字符串可能切片 panic，`ImageData::from_rgba` 不校验字节数；本组未发现由当前真实调用方输入坏值的证据，不上升为安全漏洞或 P1。
- Vello 对缺失资源统一返回 Retry，错误类型不足以解释永久 missing revision；BackendSessionGate 不接收 damage，无法自行验证 Snapshot+Full。当前 Runtime 正常提交提供对应前置保证，不能把手工破坏 submission 的情况写成正常路径必现。
- TextLayout 校验数值有限和 UTF-8 边界，不证明 glyph id、font collection index 或 layout key 与请求语义匹配；这是第三方引擎责任边界，需补外部引擎错误输出验收。
- Accessibility 当前每次 publish 都构造候选树再比较语义，而非设计描述的 semantic-dirty 快速跳过；存在可定位的额外工作，但没有基准数据，不宣称已造成性能退化。
- Vello 命令热路径仍有 `tracing::debug!`。这是与项目无热路径日志约束的偏差；实际过滤配置、耗时未测，不作为性能缺陷定级。
- 未验证 native/web 像素一致、变换/裁剪与 alpha 的全部组合、宽度受限外部 Figure arrange、复杂 bidi、字体 synthesis、glyph stroke、device loss、截图尺寸约束、第三方 panic 和资源内存预算。

### 明确后置，不作为现存 bug

- getClip 查询、path clipping、任意多矩形 ClippingStrategy、gradient、自定义 dash 数组/offset、可配置 miter、antialias/XOR 和打印 adapter。
- 完整 TextFlow fragment/inline/block、富文本编辑、caret/selection/IME；有 Parley line breaking 不等于 TextFlow 已完成。
- repeat firing、ButtonGroup/radio、Slider 和完整 widget toolkit。
- 富文本/交互 Tooltip、完整原生 AT provider、accessibility text range/table/selection 和 Editor/GEF accessibility。
- 多 backend consumer、并发 Runtime handoff activation token、跨进程资源协议、真实异步 loader token 与解码/缓存预算。

## Rust 迁移与扩展性评价

1. **所有权与身份**：Runtime 是 text/resource/session 组合根；Figure 保存 typed ID、不可变布局和资源 revision，避免 SWT payload/GC 生命周期泄漏到 Figure。Arc payload 与命令快照适合异步消费；单 in-flight 的限制必须持续写明。
2. **替换边界**：`TextLayoutEngine` 与 `RenderBackend` 分离且对象安全；非 Parley 外部测试构造了非空 IR，而非 default 空占位。Vello 不接收 raw text/family/width，不二次 shaping。这一结论不等于外部引擎和 Vello 的端到端像素已经验证。
3. **不完整扩展点**：Border 是 `Send + Sync` trait，但 owner-dependent snapshot 仍封闭；Figure 的 label capability 返回具体 LabelFigure，widget model 限于 Push/Toggle。这些是实际扩展边界，应明确可覆盖范围，不统称“组合已替代继承”。
4. **不可丢失语义**：parent-local、f64、typed mutation、延迟稳定观察属于可解释迁移；North/South 反向、组合后空画、公开参数静默忽略则改变用户可观察行为，不是所有权迁移的必要代价。
5. **错误模型**：命名 mutation 使用 Result 和 prepared/commit 思路；低层 IR 仍开放可构造，backend 的 catch-all no-op 与粗粒度 Retry 未完全落实“支持、暂不可用、永久不支持”的区分。

## 建议顺序与验收

1. **先补可执行反例**：Group 3 的三项缺陷分别增加有图标四方向位置断言、复合标题栏 metrics/glyph 断言、Dash/Dot backend 消费断言。测试由主 Agent 统一执行，不并行运行 cargo。
2. **优先修复通用消费契约**：对全部 RenderCommandKind/PathOp 建立生产者、能力检查、消费者矩阵。只忽略“alpha 已烘焙”这类有明确语义理由的冗余状态，不能把未知 draw 指令当成功。
3. **评审 Border 扩展协议后实现**：owner-scoped 测量快照要能组合且不让 Border 拥有 Runtime。先定义 trait/快照/失败原子性，再实现 G3-01；不改递归 renderer 来识别每一种边框。
4. **几何一致性专项**：统一 path 几何来源，验证曲线包围盒、fill rule、旋转方向、退化矩形、stroke envelope 和 hit/paint；不要用连接 Figure 的独立修复替代普通 Polyline/Polygon 验收。
5. **跨平台验收最后进行**：命令断言之后再跑 Vello scene encoding、native/web 像素与真实资源/session 恢复；native AT 是独立后置交付，不由 engine snapshot 测试替代。

七维度覆盖：逻辑/领域语义为三项主缺陷；健壮性记录公开边界风险；安全未发现有实际利用路径的缺陷；并发审查以单 Runtime/单 consumer 契约为边界；性能只记录静态复杂度/额外工作，不给无测量定性；质量不报告纯风格问题。

## 实际阅读文件

以下“阅读”包含完整小文件、目标方法及直接调用上下文、测试断言；不表示对每个大型文件逐行证明所有行为。只有搜索命中而未读方法正文的文件不计入。

### Rust

- `novadraw-core/src/{lib.rs,color.rs}`。
- `novadraw-math/src/{lib.rs,mat3.rs,vec3.rs}`。
- `novadraw-geometry/src/{lib.rs,point_list.rs,precision.rs,rect.rs,transform.rs,translatable.rs,vec2.rs}`。
- `novadraw-render/src/{lib.rs,command.rs,context.rs,submission.rs,text.rs,traits.rs,backend/mod.rs,backend/vello/mod.rs}`。
- `novadraw-scene/src/figure/{mod.rs,rectangle.rs,ellipse.rs,rounded_rectangle.rs,polyline.rs,polygon.rs,triangle.rs,root.rs,label.rs,image.rs,widget.rs}`。
- `novadraw-scene/src/figure/border/{mod.rs,line_border.rs,margin_border.rs,rectangle_border.rs,compound_border.rs,bevel_border.rs,etched_border.rs,title_bar_border.rs}`。
- `novadraw-scene/src/graph/{render_recursive.rs,mod.rs}`：后者限 preferred/measurement、border replacement/snapshot 与本组相关 diff。
- `novadraw-scene/src/runtime/{resource.rs,accessibility.rs,runtime.rs}`：最后一项限 text/title/resource/session/frame 直接调用链及相关 diff。
- `novadraw-geometry/tests/m1_product_existence.rs`。
- `novadraw-render/tests/{m1_product_existence.rs,m10_text_extension_contract.rs,r8_extension_boundaries.rs}`。
- `novadraw-scene/tests/{m10_label_contract.rs,m10_reusable_shape_border_contract.rs,m10_widget_contract.rs,m10_accessibility_contract.rs,m10_tooltip_contract.rs}`。

### Java

- `J/{Graphics.java,SWTGraphics.java,Figure.java,Shape.java,RectangleFigure.java,Ellipse.java,RoundedRectangle.java,Polyline.java,Polygon.java,Triangle.java}`。
- `J/{Border.java,CompoundBorder.java,TitleBarBorder.java,Label.java,ImageFigure.java,TextUtilities.java,ClickableEventHandler.java,ToggleModel.java}`。
- `J/geometry/{Rectangle.java,PointList.java,Transform.java}`；`J/text/TextFlow.java`。
- `J/{AccessibleBase.java,LightweightSystem.java}`；`G/AccessibleEditPart.java`。

当前 diff 已读：本组 `command.rs` 新增 miter 常量及导出、Vello 五个 Stroke 分支应用该常量；共享 graph/runtime 的连接批次变化只用于识别边界，不重复对连接几何下结论。报告没有把删除的旧逻辑当当前缺陷。
