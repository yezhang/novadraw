# ADR-050：按实际字体准备下划线

类型：`architecture-decision`

状态：Accepted，2026-10-27 用户批准通用下划线实施。

`api_semantics`：`text.flow`、`text.interaction`、`paint.protocol`、`damage.repaint`。

## 决策

首个切片提供 `text::TextUnderline` 逻辑范围及颜色，
`MeasureContext::prepare_underlines` 将其与 StyledTextLayout 结合。
范围按行、bidi run、实际 fallback face 拆分；端点必须是可见的 grapheme 边界。
StyledGlyphRun 公开实际基线和 advance bounds，字形位置不用于猜测基线。
每个逻辑范围只查询一次选择几何，按行合并重叠区间，再与实际 run bounds 相交，
避免重复片段和逐 run 重复遍历完整文本。

TextLayoutEngine 的 `resolved_font_metrics` 接收已解析 GlyphRun，
返回完全匹配 face/revision/index、normalized variation 和 size 的 FontMetrics。
默认引擎直接读取注册字体；缺失或替换的字体、指标缺失及不匹配结果明确拒绝。
外部引擎使用相同方法，没有请求字族替代实际字体的分支。

结果为 owned 不可变 PreparedTextUnderlines，包含 layout revision、实际字体指标、
片段矩形和颜色。未完成的准备不发布部分结果；无需可变 source 状态。
Graphics 与 PaintContext 的 `fill_underlines` 绘制相同结果并恢复状态。
`ink_bounds` 供外部 Figure 与文字 ink 合并后提交 visual bounds，
旧新 damage 由既有 FigurePresentation 更新机制负责。
逻辑文本高度保持原值；字体变化、宽度变化或 detach 通过既有 preparation 失效。

PathExample 替换样例中的单字体下划线算法，保留原始选择和两次文字绘制语义。
本切片不新增文本树、装饰枚举或 TextFlow 专属路径；后续装饰可复用相同准备原则。

## 外部消费者证据与成本

独立引擎及 Figure 使用同一指标、片段和 Graphics 接口。
覆盖 fallback、不同字号、换行、RTL、组合字符、字体替换、错误和旧新像素擦除。
方案记录的 Draw2D/SWT 源码只读；像素回归证明本机更新一致性，
不替代其他平台或上游原生栅格验收。

没有新增依赖、字体或 GPU 格式。准备按相交 run 输出片段，绘制按片段数线性执行；
相同字体实例在一次准备内复用指标。静态帧复用结果。
长文窄宽重排及相同 release 产物成本归本批验证记录。
