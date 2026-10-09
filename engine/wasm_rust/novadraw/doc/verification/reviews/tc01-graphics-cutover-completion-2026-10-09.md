# TC-01 Graphics Cutover 完成记录

类型：`implementation-verification`

日期：2026-10-09

状态：`complete`

## 交付范围

TC-01 按 ADR-031 将绘制和帧出版收口为：

```text
standalone caller -> CommandRecorder -> Graphics / PaintContext -> RecordedDrawing
Figure / Shape / Border -> PaintContext -> Result<(), GraphicsError>
Runtime -> prepare_submission -> RenderSubmission
```

完成内容：

- 删除 `Figure`、`Shape` 和 `Border` 对公开 `NdCanvas` 的依赖；
- 删除旧 `paint_figure` / `paint_figure_in_bounds` 双入口；
- `PaintContext` 提供 rect、ellipse、line、polyline、path、text、image、transform、
  clip 和 state-stack 的统一受检绘制语言；
- Figure、Border 与 presentation 绘制错误进入 Runtime 帧准备失败，不出版残缺帧；
- `CommandRecorder` 成为唯一公开 owned 命令录制入口；
- `NdCanvas` 降为 crate 内部 lowering 状态，退出 root、prelude、`graphics` 和
  `render` 公共表面；
- `Runtime::prepare_submission` 成为唯一公开帧出版入口，旧 `prepare_frame` /
  `record_full_frame` 退出公共 API；
- Native、Web、Editor、benchmark、示例场景与外部合同测试迁移到新入口。

## 所有权与失败

`PaintContext` 只借用录制状态，不能读取或修改 damage、清空既有命令、构造 submission
或设置 session/frame 身份。`CommandRecorder::finish` 只产生 `RecordedDrawing`，
不冒充 Runtime 已出版帧。

每个受检绘制操作失败时回滚本次操作，保留首个 `GraphicsError`。Figure 和 Border
临时 state 在错误路径也会恢复；Runtime 返回
`FramePreparation::Error(FramePreparationError::Graphics(_))`，修正 Figure 后可用
同一资源身份重试。

## 自动验证

统一 suite：`core.tc01-graphics-cutover`。

已通过的定向合同：

- `tc01_graphics_boundary`：2 项；
- `p2_graphics_contract`：23 项；
- `image_source_rectangle_contract`：4 项；
- `render_m1_product_existence`：2 项；
- `m10_text_extension_contract`：2 项；
- `m10_reusable_shape_border_contract`：16 项；
- `p2_f01_scalable_polygon_contract`：4 项；
- `m2_product_existence`：6 项；
- `p2_t01_text_flow_contract`：3 项；
- `p2_animation_transition_contract`：13 项；
- `p2_animation_behavior_contract`：7 项；
- `p2_animation_contract`：29 项；
- `cargo check -p novadraw --lib`；
- `cargo xtask verify core.tc01-graphics-cutover`；
- `cargo xtask verify core.p2-g01-graphics`；
- `cargo xtask verify core.p2-g02-text-graphics`；
- `cargo xtask docs`；
- `cargo xtask check --quick`；
- public API snapshot 已更新并确认 `NdCanvas` 从 `module.graphics` 和
  `module.render` 消失。

`cargo xtask check --full` 的 fmt、facade、第三方类型、API snapshot、hot-path、
Core I/O、Kurbo、Native/Web Vello 与 workspace Clippy 阶段全部通过；最终仅在
`workspace.test` 被下述既有问题阻断。

## 已知独立阻断

- workspace `cfg(test)` 仍受此前 Builder `Result` 迁移未完成影响，共 405 个编译错误；
- 部分 integration test 与 Editor test 仍使用 TC-13 收窄前的 root facade 导入。

这些问题不属于 TC-01；TC-01 定向合同和生产 workspace 编译不依赖旧调用面。

## 关系

- [ADR-031](../../adr/adr-031-graphics-recording-and-publication-boundary.md)
- [临时概念设计审计](temporary-concept-design-audit-2026-10-08.md)
- [统一 Graphics 与 glyph 准备](../../adr/adr-025-unified-graphics-and-glyph-preparation.md)
- [Graphics 扩展契约](../../design/rendering/p2-g01-graphics-extension.md)
