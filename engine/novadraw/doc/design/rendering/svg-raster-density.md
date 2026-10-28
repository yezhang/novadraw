# SVG 栅格密度与资源替换

类型：`normative-design`

状态：已实现，承接已批准的 K08 与 ADR-030 纯解码边界。

`api_semantics`：`resource.lifecycle`、`graphics.context`。

## 保留矢量源，稳定自然尺寸

ZoomExample 的三个 SVG 图标在固有尺寸只栅格化一次。放大或提高宿主 DPI 后，
后端只能放大旧像素。当前 `ImageData::decode_svg(bytes, scale)` 的 `scale` 表示
像素与自然逻辑尺寸的比例，不表示 SVG 栅格化倍率；本批保留该 API 的既有行为。

增加 owned、不可变的 `SvgImageSource`，保存已解析的 SVG。它只负责纯解析与栅格化，
不持有 Runtime、资源 ID、文件路径或 URL，也不执行异步任务。
第三方图片来源继续通过相同的 `ImageData` 和资源请求 token 提交，不增加 SVG 专用
Registry 状态、Render command 或后端特权路径。

## 公开接口

新值通过现有 `render` 领域路径导入：

```rust,ignore
let source = SvgImageSource::decode(bytes)?;
let options = SvgRasterOptions::try_new(pixel_density, pixel_budget)?;
let data = source.rasterize(options)?;
```

- `SvgImageSource::logical_size()` 返回 `Dimension`，与旧解码器的固有整数尺寸一致。
- `pixel_density` 是正整数；像素宽高为固有尺寸分别乘以密度。
- `pixel_budget` 是正整数像素数上限，调用者必须明确提供。
- 结果 `ImageData::scale` 等于密度。因此更新像素密度不改变图标的自然逻辑尺寸，
  不因小数倍率和像素取整导致 Label 布局漂移。
- 密度或预算为零、像素宽高溢出、像素数超过预算，分别返回结构化错误。
  预算校验发生在 Pixmap 分配之前。
- 解析失败沿用 `InvalidSvg`；栅格结果继续使用普通 straight-alpha RGBA。

`ImageData::decode_svg` 可以复用同一解析实现，但旧 `scale` 的意义和错误行为保持。
纯解码器不创建资源 revision，不发出 Figure 通知，也不接触 backend session。
usvg 默认的外部图片路径 resolver 会读取本地文件。本批关闭该隐式读取路径，
新源与旧解码入口都只消费提供的 bytes 和 data URI 内嵌图片；外部 href 不解析，
由宿主在提供 SVG 前完成资源内嵌。原始官方图标不依赖外部图片。

## 宿主选择密度，Runtime 提交资源

密度选择由持有 source 的消费者负责：

```text
已提交的局部到 surface 变换 + surface DPI
→ 选择覆盖最大放大倍率的整数密度
→ 同一密度不重新栅格化
→ begin_request(ImageId)
→ source.rasterize(显式预算)
→ complete_image_request(token, data)
→ 既有资源依赖失效与帧提交
```

多个 Figure 共用一个 ImageId 时，消费者选择这些实例所需的最大密度。
原始 SVG 与密度缓存由消费者持有。Runtime 不新增 provider、全树逐帧扫描或文件 I/O。
迟到完成、replacement、Retry、reset 与释放继续遵循 ADR-030 和资源因果协议。

Zoom 的宿主消费同一组图标 source，读取缩放后的已提交变换与实际 DPI。图标的
ImageId 保持不变，资源 revision 仅在密度变化时更新。资源更新不得改变类布局、
滚动条尺寸或缩放公式。

## 验证边界

- 密度 1／2／3 的像素尺寸与自然尺寸；保留源重复栅格化不重新解析。
- 零值、溢出和预算失败在分配前拒绝；合法低密度结果不受失败请求影响。
- 旧 `decode_svg(..., scale)` 的像素尺寸及自然尺寸保持。
- Zoom 缩放与 DPI 变化更新资源 revision；重复同一密度不更新，恢复后布局不漂移。
- 资源 replacement／迟到完成使用既有 token；Native GPU 确认新版图像进入绘制。
- Core Native／Wasm 公共 API、文档、严格 Clippy 与完整测试。

本批不改变 SVG 的字体解析策略，不建设 SVG DOM 编辑器或 GPU 矢量资源格式；
逐 Shape AA 的 Graphics／IR／backend 增量需另行评审。

依据：[ADR-030](../../adr/adr-030-resource-provider-and-request-token-boundary.md)、
[资源因果协议](../architecture/resource-lifecycle.md)、
[官方示例 Core 计划](../../verification/plans/draw2d-official-examples-core-plan.md)。

实现与验证记录见 [K08 SVG 证据](../../verification/reviews/k08-svg-density-evidence-2026-10-23.md)。
