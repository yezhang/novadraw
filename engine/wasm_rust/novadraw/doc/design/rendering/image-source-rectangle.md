# 图像源区域绘制契约

类型：`normative-design`

## 1. 目标

Novadraw 支持把图像资源的一个矩形区域缩放绘制到 Canvas 的目标矩形。该能力对应
Draw2D `Graphics.drawImage(Image, source, destination)`，并保持 backend-neutral：

```text
ImageResourceRef + source rectangle + destination rectangle
→ RenderCommand
→ RenderBackend
```

后端不得静默忽略非完整 source rectangle。

## 2. 坐标域

- source rectangle 位于图像资源的物理像素域，边界为
  `[0, image.width] × [0, image.height]`；
- destination rectangle 位于当前 `NdCanvas` 逻辑坐标域，继续参与 Graphics transform、
  clip、global alpha 与 surface scale；
- 图像资源的 `scale` 只决定完整图像的自然逻辑尺寸，不改变 source rectangle 的像素域；
- source 使用 `f64`，允许后端执行亚像素采样，这是相对 Draw2D 整数 source 坐标的合理
  扩展。

## 3. Render IR

每个 Image command 都携带显式 source rectangle，不使用 `Option`：

```text
Image
├── image: ImageResourceRef
├── source_rect: Rectangle
├── dest_rect: Rectangle
└── alpha: f64
```

完整图像绘制使用
`Rectangle::new(0, 0, image.width, image.height)`，从而使所有 backend 消费同一协议。

## 4. 输入与失败

`NdCanvas::draw_image_region` 返回 `Result`：

- source 或 destination 含非有限值：返回对应的 non-finite 错误；
- source 或 destination 宽高为负：返回对应的 negative-extent 错误；
- source 任一边超出图像像素边界：返回 source-out-of-bounds 错误；
- source 或 destination 任一宽高为零：成功但不记录命令；
- global alpha 为零：成功但不记录命令。

失败和 no-op 都不得改变 command stream 或 damage。普通 `draw_image` 与
`draw_image_with_size` 继续是完整 source 的 convenience API。

## 5. Backend lowering

Backend 将 source rectangle 仿射映射到 destination rectangle：

```text
scale_x = destination.width / source.width
scale_y = destination.height / source.height
translate_x = destination.x - source.x * scale_x
translate_y = destination.y - source.y * scale_y
```

绘制完整图像前必须把输出裁剪到 destination rectangle，防止 source 之外的像素泄漏。
临时 image clip 嵌套在现有 Graphics clip 内，不修改持久 RenderState。

Vello adapter 使用 `push_clip_layer → draw_image → pop_layer` 实现该协议。其他 backend
必须提供等价行为；若未来 backend 无法支持，应在提交前通过 capability 返回结构化
unsupported，不得在 command lowering 中静默跳过。

## 6. 验证

- 完整 source 和局部 source 的 command 记录；
- source/destination 非有限值、负尺寸、零尺寸和越界；
- source 到 destination 的缩放和平移；
- destination clip 在 transform 与 surface scale 下保持正确；
- global alpha 与资源 revision 保持现有语义；
- native Vello、Web Vello 与 backend-neutral 构建门禁。

## 7. 参考

- Draw2D `Graphics.drawImage(Image, Rectangle, Rectangle)`；
- Draw2D `SWTGraphics.drawImage(...)`；
- SWT `GC.drawImage(...)` source containment 与负尺寸失败契约；
- [ADR-020](../../adr/adr-020-engine-value-and-render-contract.md)。
