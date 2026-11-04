# ADR-025: 统一 Graphics API 与 Glyph 预处理边界

类型：`architecture-decision`

## 状态

已接受，2026-10-05 用户确认采用。
实施与验收状态见 [P2-G02](../roadmap/p2-delta-backlog.md)，不以接受推断实现完成。

## 背景

调用者需要在同一个 API 集合中完成字体指标查询、文字测量、文字绘制与图形绘制。
底层分别采用 Parley 和 Vello，不应迫使调用者协调两套字体状态或理解 glyph run。

未来自研字体后端将使用片段着色器，消费布局后的 glyph 位置和经分网格等预处理的
贝塞尔曲线数据。仅提供 TextLayoutEngine 替换或 Vello Path 绘制不能完整表达这条
链路，必须把布局、轮廓、后端预处理和每次绘制实例的归属明确区分。

## 决策

接受 [文字与图形整合设计](../design/rendering/text-graphics-integration.md)：

1. Graphics facade 同时提供 `font_metrics`、`measure_text`、`layout_text`、
   `fill_text/stroke_text` 和图形绘制。它组合借用服务和录制状态，不拥有另一个 Runtime。
2. MeasureContext/PaintContext 是同一体系按阶段提供的受限视图；
   Figure 在布局阶段准备不可变 TextLayout，paint 不重新 shaping。
   独立录制 Graphics 可以先测量后绘制。
3. TextLayoutEngine 决定精确字体实例、glyph ID、advance/offset、换行与交互几何；
   GlyphOutlineProvider 从该实例和 glyph ID 提取受检中立曲线。两者独立替换。
4. glyph 预处理位于布局之后，按实际使用的 glyph 获取或缓存曲线，
   由字体后端完成分网格、曲线系数、索引等处理。
5. PreparedGlyph 是可复用几何资产；GlyphDrawInstance 保存位置、变换和绘制状态。
   不将每次布局位置烘焙进共享曲线数据，也不在 shader 前重新累加 advance。
6. Core 保留 backend-neutral 字体/glyph/曲线语义；预处理格式、GPU buffer 和
   fragment 算法属于 backend-local 合同。支持后端显式导入自己的预处理资产，
   不向通用布局结果塞无类型 blob，不传递跨后端 GPU handle。
7. 字体身份、变体、revision、单位和坐标转换贯穿布局/曲线/预处理；实际缓存依赖
   必须显式。布局尺寸、ink bounds、paint bounds 分开，不用曲线包围盒替代文本测量。
8. 图文共用有序合成、paint、clip、transform、opacity、damage 和 submission 确认。
   预处理/上传失败不部分接受帧，缓存成功不等于呈现成功。

## Vello 与自研路径

- 默认路径继续允许 Vello `draw_glyphs` 使用其内部 Skrifa 与 glyph cache。
- 替换轮廓时显式走 provider → Path → Vello，不能假定 Vello 的
  `draw_glyphs` 会调用外部 provider。
- 自研字体后端消费“定位 glyph + 原始曲线”或自身认可的 PreparedGlyph，
  在同一 renderer 中完成有序执行/合成；不强制把预处理数据重新转换为路径。
- 当前不实现尚未提供的片段着色器算法，不冻结网格格式、shader ABI 或设备资源格式。

## 失败与资源合同

- 字体替换、变体和度量变化须使对应布局/轮廓缓存失效；位置变化通常只更新实例。
- 若预处理依赖 ppem、误差容限、变换或描边参数，必须纳入 key。
- 导入资产校验字体内容、face/glyph、算法/格式版本、单位、参数和索引/数值有效性。
- device/session 变化使 GPU handle 失效；允许保留匹配的 CPU 预处理资产后重传。
- 原始轮廓与预处理资产都必须给出可用的几何包络；后端不得私自改变布局指标。
- 非法输入、缺资源、不支持的字形格式和合法空轮廓分别处理。

## 关系与交付范围

- 保持 ADR-014 的 Runtime 文字服务所有权与 measure/arrange/paint 顺序。
- 扩展 ADR-007 的后端无关 glyph 合同；布局不依赖 GPU。
- 保持 ADR-023 六包边界与 ADR-024 的 Paint/Stroke/clip/能力预检。
- 接受 API 体验提案的文字整合部分；其他领域的大范围改名不因本 ADR 自动获批。
- 交付以 P2-G02 为单位：统一 API、默认实现、外部曲线/预处理 consumer、原子失败、
  Native/Web 图文像素证据。自研片段着色器实现是未来独立后端工作。
- `api_semantics`：`graphics.context`、`text.flow`、`text.interaction`、
  `paint.protocol`、`damage.repaint`、`render.backend_session`。
