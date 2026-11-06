# 文字高性能方案采用情况与 Core 局部化缺口

类型：`verification`

能力归属：Core Text／Graphics／FigureTree／Runtime；richtext 拥有文档及段落布局；
Vello backend 拥有编码、设备资产和栅格；Editor 消费文档版本与交互几何。
日期：2026-11-06。状态：按当前工作区定向核对，包含尚未提交的并行改动；
本报告不接受新协议，不关闭 FR-02、巨段绘制或平台呈现门禁。

## 当前判断

当前主要缺口是默认原生文字在 Core 中仍完整生成命令，以及巨段编辑和变宽仍需
完整处理变化段落。Rope、共享 glyph、段落布局缓存和高度树已经采用，
但它们各自减少的成本不同。不能根据这些结构的存在推断全文记录或全文重排已消失。

对照范围是[高性能路线](../../design/architecture/text-performance-strategy.md)、
[长文本总方案](../../design/architecture/long-text-performance-proposal.md)和
[图文字形预处理合同](../../design/rendering/text-graphics-integration.md)。
采用状态以实际调用路径为依据；历史方案中拟定的类型名不算实现证据。
本次没有重新进行跨产品性能排名，也没有将归档补丁当作当前生产能力。

## 已有结构与剩余成本

| 技术或结构 | 当前采用范围 | 尚未解决的成本与源码 |
|---|---|---|
| 不可变 glyph backing、共享文本与行结果 | Core `GlyphBuffer`／`GlyphRange`、`TextLayout` 的 Arc；run 切片共享原资产 | `NdCanvas::draw_glyph_runs` 的 native 分支仍逐 run 生成命令；submission 仍是 Vec。见[布局与 glyph](../../../crates/novadraw/src/render/text.rs)、[记录](../../../crates/novadraw/src/render/context.rs) |
| Rope 文档来源 | richtext 每段持有私有 `ropey::Rope`，未变段落以 Arc 保留 | 编辑先调用 `paragraph.input()`／`text()` 物化 String，再替换并重建该段 Rope；段落表仍复制 Arc 列表。尚未采用直接 Rope delta 到布局的共享切片路径。见[文档](../../../crates/novadraw-richtext/src/document.rs) |
| 持久化高度摘要树 | richtext `ParagraphHeightIndex` 按树路径更新单个高度、查询前缀 y | 同顺序布局仍遍历全部段落组装快照；逻辑高度不能证明实际墨迹范围。见[索引](../../../crates/novadraw-richtext/src/index.rs)、[布局](../../../crates/novadraw-richtext/src/layout.rs) |
| 段落级增量布局 | richtext 缓存环境匹配时共享 Arc 身份未变的段落结果；变化段落重排 | 不是巨段内部的增量 shaping／断行。宽度、引擎、字体和方向改变时不复用旧段落布局。见[验证](richtext-incremental-feedback-2026-11-06.md)、[缓存](../../../crates/novadraw-richtext/src/cache.rs) |
| 层次绘制包络索引 | Core `EnvelopeIndex`、OutlinedText 的 glyph 索引与 FlowSnapshot 的段落索引 | 已知轮廓可以剪枝；native 的未知包络仍保留。richtext paint 仍遍历全部段落。见[包络索引](../../../crates/novadraw/src/text/outline/paint_index.rs)、[Flow 绘制](../../../crates/novadraw/src/figure/flow/snapshot.rs) |
| 测量／准备共享完整快照 | richtext 有界两条缓存；Core 已有完整 request 与段落资产缓存 | 减少同请求重复布局，不代表宽度改变时复用 shaping；也不免除输出版本、字体及输入检查。见[resize 验证](rich-text-figure-resize-optimization-2026-11-05.md) |
| 统一输入证明、共享 Flow 组装资产 | FR-02 已有共享输入、首次值一致性和整页采用开关 | 默认仍关闭，完整家族容量、生产反例与净收益尚未验收。见[整页采用](flow-assembly-adoption-implementation-2026-11-04.md)、[成本对照](flow-assembly-scroll-comparison-2026-11-05.md) |
| 后端 damage 选择与小纹理修补 | Vello 0.11 私有路径已在编码／位图解码前筛选，并只栅格化局部修补目标 | Core 的 native 命令量仍未减少；后端内部选择不提供 Core 录制前的公开覆盖证明。见[后端验证](richtext-local-damage-2026-11-06.md) |

本轮[Core 局部选择切片](core-local-damage-recording-2026-11-06.md)把已有轮廓索引
进一步限定到 damage 与 clip 的交集。它没有把 native 字形换成普通 Path，
也没有用段落高度裁掉 Emoji、图片或未知墨迹。

## 尚未采用或仅有实验的技术

| 优先级 | 技术与数据结构 | 用途与实施前提 |
|---|---|---|
| 当前优先 | native 静态墨迹摘要、原序段落／run 包络树、本帧覆盖绑定 | 在生成 DrawGlyphRun 前排除与 damage 不相交的已知分支；字体、变量坐标、尺寸、合成斜体、lowerer 数值域及 bitmap/COLR 分流必须一致。未知分支继续完整输出 |
| 当前优先 | 不可变命令段与浅引用 stream | 减少重复录制和命令容器复制；完整状态、资源、输入、栈和峰值留存均需证明。现有具体方案仍待独立复核 |
| 变宽热点优先 | 默认 Parley 的分析／shaping 资产与行结果分离 | Core 通用 `StagedTextLayoutEngine` 已实现，默认 Parley 没有覆写阶段能力；仍走完整 builder 与 break_all_lines。历史 B1 补丁已退役，不能记为当前采用 |
| 巨段编辑优先 | source delta、shaping 安全边界、断行检查点和后缀收敛证据 | 变化段落内部局部重排。UTF-8 或 Rope 叶边界不等于 shaping 安全边界；bidi、连字、组合字符、inline 及断行状态必须一起验证 |
| 来源复制热点后续 | 直接 Rope 编辑、共享段落切片、持久化段落表和位置映射 | 避免编辑一字先物化整段，再复制全部段落引用；布局要求连续文本时仍需局部物化，须计量实际复制。piece tree 是来源表示的备选，不必与 Rope 同时引入 |
| 前台阻塞后续 | 按帧请求合并、可取消 worker、候选版本和预算调度 | 连续 resize 时取消过期计算；不丢已消费的交互几何、不让 worker 持有附着树，也不把旧结果发布为当前版本 |
| 条件性后端研究 | 热块栅格复用、冷块区域查询与滚动复制条带 | 已有默认关闭的纹理实验；完整 IR 比较、冷块全文编码及组合像素门禁仍未关闭。可见 IR 与完整块身份必须联合验收 |
| 条件性后端研究 | PreparedGlyph 曲线系数、二维网格或水平／垂直条带索引、GlyphDrawInstance | 尚未采用自研字体 shader；Dobbie 网格和 Lengyel 双向条带是不同算法候选。数据归 backend，不进入 Core 排版结果 |
| 条件性后端研究 | 显式可替换 glyph raster atlas | Vello 自己已有字形缓存／图像资源机制，不能说“当前没有字形缓存”。Novadraw 尚未建设独立 atlas 后端；先证明局部编码之后仍有栅格热点 |

阶段与增量依据：
[阶段合同](../../design/architecture/staged-text-width-reuse.md)、
[当前阶段状态](text-stages-status.md)、
[增量候选](../../design/architecture/long-text-incremental-layout-proposal.md)。
绘制与后端依据：
[native 覆盖候选](../../design/architecture/long-text-native-coverage-boundary.md)、
[局部录制候选](../../design/architecture/figure-drawing-recording.md)、
[栅格实验](../../design/rendering/text-scroll-raster-reuse.md)、
[字体算法参考](../../reference/fonts/glyph-rendering-and-color-emoji.md)。

## 后续 Core 局部化的具体边界

默认 native 正文仍是当前缺口。先把静态资产与本帧状态分开：
静态资产持有原序范围、完整字体与 provider 身份，以及 Known／Empty／Unknown 墨迹；
Runtime 冻结实际资源、session、lowerer policy、DPI 和 SurfaceInfo；
Recorder 用当前 transform、clip、AA 与操作查询，输出共享 glyph 范围，
backend 在编码前核对绑定。已删除 glyph 的提交不能在 policy 变化后静默回退。

先查询段落／run 摘要，再下降到候选 glyph；没有证据的 sibling 保留原输出。
不能在录制阶段重新遍历全文提取轮廓来换取少量命令，也不能把后端固定两像素 padding
复制成任意 native 字体和变换都成立的 Core 证书。
静态构造、帧内查询、未知退化、选中数量和留存峰值分别计量。

不可变命令段解决存储与回调成本，覆盖索引解决输出范围，两项分别取得资格。
当前录制候选限制文字统计增量非零的段不取得回调省略权；
因此即使先迁移 stream，也不能宣称默认长文字 paint 已被省略。
需要改变 painter 可观察统计的语义时，必须另行评审。

本轮保留[总体方案](../../design/architecture/figure-tree-projection-separation.md)
规定的独立录制复核。候选 API、公共容器迁移和全部 backend 消费路径不能在
本轮内部 damage 选择修改中顺带接受。后续状态仍归入唯一 FR-02 Execute。

## 核对限制

上述代码事实是当前工作区的定向核对，不能据此证明跨平台行为或绝对延迟预算。
历史性能数字分别引用原报告，不把不同阶段分位数相加。
本报告已按技术文档检查和中文表达规则自审；没有独立读者复述或独立架构复核。
