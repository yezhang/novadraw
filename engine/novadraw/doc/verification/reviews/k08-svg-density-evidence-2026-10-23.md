# SVG 密度更新验证

类型：`verification`

日期：2026-10-23

范围：K08 SVG 纯解码与 Zoom 消费者；附 K06 静态折叠 fallback 修复。

`api_semantics`：`resource.lifecycle`、`graphics.context`、
`animation.presentation`、`layout.manager`。

## 已实现的能力

`render::SvgImageSource` 保存解析后的不可变树；`SvgRasterOptions` 要求调用者
明确给出正整数密度和像素预算。宽高乘法、RGBA 地址空间和预算在分配前检查。
结果使用普通 straight-alpha `ImageData`，scale 与密度相同，自然尺寸保持不变。
旧 `ImageData::decode_svg` 的固有像素尺寸、scale 含义和错误优先级保持。
同时关闭 usvg 默认 resolver 的本地文件读取。外部 href 忽略，data URI 内嵌图片
继续解码；宿主负责外部图片内嵌。这项修复落实已有纯解码边界。

Zoom 的三个共享 ImageId 保持不变，消费者根据两个类的已提交变换和宿主 DPI
选择最大密度，只在密度变化时重栅格化。三个结果全部准备成功后，才通过普通资源
请求 token 替换。消费者只访问已知的两个类，不扫描全树，不安装私有 timer。

K06 的原失败发生在 Disabled/ReducedMotion 判断之前：零尺寸折叠布局先尝试
构造仿射缩放，返回 `IncompatibleBoundsTransition`。现在先检查捕获身份和变化，
静态模式直接呈现已提交布局。Enabled 保留原有受检变换契约。

## 确定性与像素证据

| 验证入口 | 结果 |
|---|---|
| `svg_raster_density_contract` | 4 项通过：密度 1/2/3、重复栅格、自然尺寸、straight alpha、零值/溢出/预算、失败后恢复、旧 API 行为与无文件读取 |
| Zoom 库级消费者回归 | 4 项通过：原始类结构与范围、箭头/拖拽公式、重复缩放布局、共享 revision 与 DPI/预算恢复 |
| `p2_animation_transition_contract` | 修复后定向记录为 15 项通过；包含静态零尺寸和已释放目标拒绝 |
| `layout_submission::native_gpu_zoom_uses_replaced_svg_pixels_and_restores_original_layout` | Native GPU 通过：同一 zoom 下更新像素后画面变化，恢复密度与 scale 后整幅图像逐字节一致 |

GPU 图像位于 `target/verification/svg-density/`，包括初始、低密度放大、
高密度放大和恢复共四幅。已查看高密度图像，确认两个 UML 类与图标进入绘制。
图像由相同 Runtime、资源协议与 Vello 后端生成；没有通过截屏后处理模拟密度。

最终 workspace 测试共 1,416 项通过、6 项忽略、0 项失败；严格 workspace Clippy、
fmt 和差异空白检查通过。`core.facade` 通过，包括架构边界、Core doctest、
Native/Web backend feature；新增两个 render 类型已进入 Native/Wasm 相同 API 快照，
Runtime 方法仍为 28/28。关闭外部 href resolver 后再次完成 Wasm Core 编译。
文档校验通过；中文半角标点与破折号机械扫描无命中。

全量测试仍输出既有测试中未消费 `SubmissionAcknowledgement` 的警告；
严格 Clippy 按仓库规定的 workspace 默认目标执行，不宣称所有测试目标无警告。

## 完整入口仍保留未完成状态

catalog 继续为 16 complete、8 in_progress、1 blocked，24 个可运行。
本切片不关闭 Zoom 全部视觉对等或跨显示器系统 DPI 验收，不关闭 K08 AA、
hairline/低 miter 与混合字体装饰，也不关闭 Tree 的 branch/lines/viewport 动画。
完整契约见 [SVG 栅格密度](../../design/rendering/svg-raster-density.md)，
执行状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。
