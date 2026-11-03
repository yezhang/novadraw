# Core Rust 能力状态复核

类型：`verification-record`

日期：2026-11-02。能力归属：Core（`novadraw`）；backend／platform、Inspector、
Editor 和官方样例分别列出依赖与验收边界。
本轮以当前 Rust 公开路径、调用方和既有测试／实施证据核对状态，
没有重跑全部窗口和像素验收。执行状态由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 已实现能力与源码入口

| 能力 | 当前 Rust 入口 | 既有验证 | 完成范围 |
|---|---|---|---|
| 状态栈、路径、变换、clip、gradient、dash | [graphics](../../../crates/novadraw/src/graphics/context.rs)、[StrokeStyle](../../../crates/novadraw/src/render/stroke.rs) | [Graphics 证据](p2-g01-graphics-evidence.md) | Core／Vello 已实现；P2-G01 surface 验收未闭合 |
| 逐操作图形和文字抗锯齿 | `Graphics::set_antialias`／`set_text_antialias`、`RenderCapability::PerOperationAntialias` | [AA 实施](per-operation-antialias-implementation-2026-10-26.md) | AA-1～4 已实现和验证，完整上游栅格及平台边界保留 |
| 低 miter、零宽 profile | `StrokeStyle::effective_join`、样例 Cocoa 映射 | [低 miter/profile](../../design/rendering/low-miter-and-cocoa-sample-profile.md) | 低 miter 已实现；Core 逻辑零宽不绘制，通用设备 hairline 未实现 |
| 继承排版方向、结构化文字流 | [flow](../../../crates/novadraw/src/figure/flow/mod.rs)、[styled text](../../../crates/novadraw/src/text/styled.rs) | [方向及文字增量](official-example-core-increment-2026-10-27.md) | nested inline／block、真实 Figure、bidi、caret／selection 已实现 |
| 跨字体／跨行下划线 | [PreparedTextUnderlines](../../../crates/novadraw/src/text/underline.rs)、`prepare_underlines`／`fill_underlines` | 同上、`text.underlines` | 实际字体、字号、baseline 与逻辑范围统一准备，Path 已消费 |
| owner-aware Locator 与路由 | [connection](../../../crates/novadraw/src/connection/mod.rs) | [K03 计划与记录](../plans/draw2d-official-examples-core-plan.md) | 端点／标签、visibility／组路由已实现；P2-C03 跨 viewport 未实施 |
| ScrollBar 与共享 range | [scroll_bar](../../../crates/novadraw/src/container/scroll_bar.rs) | [ScrollBar](../../design/architecture/scrollbar-and-figure-activity.md) | 五部件、横纵向、range、repeat／取消已实现；完整 widget family 仍 deferred |
| 图布局与 Tree 动画 | [graph](../../../crates/novadraw/src/layout/graph/mod.rs)、[animation](../../../crates/novadraw/src/animation/mod.rs) | ADR-043／046、[Tree 实施](tree-layout-presentation-implementation-2026-10-26.md) | Directed／Compound 与 Tree 批次完成；M01-D 仍待平台采证 |
| 共享场景与背景 | [scene](../../../crates/novadraw/src/scene.rs)、Runtime Host background | ADR-044／045、K07-1～6 | Thumbnail、透明 Canvas 已实现及 Native 验证；跨平台背景／DPI 仍待验收 |
| 精确文字缓存与共享 IR | [FlowLayoutService](../../../crates/novadraw/src/figure/flow/snapshot.rs)、共享 GlyphBuffer／GlyphRange | [长文本首批](long-text-scroll-resize-implementation-2026-10-28.md) | 段落、来源索引与共享存储已落地；性能目标和默认原生字形剪枝未闭合 |
| 连接注册与观测故障 | [service facades](../../../crates/novadraw/src/runtime/runtime/service_facades.rs) | S09-A／B／C | Core 注册／fault 与观测合同已实现；不据此关闭 Editor S09-D |

`adapted` 表示有当前 Rust 对等机制，不能单独替代完整平台验收。
本轮将长期能力账本中的高级 Graphics 和 Animation 从“目标 pending”更新为
“已实现 adapted”，P2-G01／M01 的总体状态仍保持 in_progress。

## 真正未闭合的增量

长文本仍需减少纯滚动与 resize 的重复贡献读取／稳定化，验证宽度分阶段复用，
并为默认原生字形的可见输出建立与实际 lowerer、字体版本、变换和 AA 对应的覆盖依据。
已有轮廓索引可剪枝不代表默认原生 glyph 已安全剪枝。
worker、渐进文档几何、紧凑来源和 glyph atlas 保持独立评审。

设备 hairline、P2-C03、P2-O01、完整 widget family 和 live unmount 是独立候选，
不能用已有 API 名称或可运行样例宣称完成。SWT Printer、原生控件和产品持久化
不属于本轮 Core 能力分母。

## 样例状态与历史证据

catalog 保持 25 个运行工厂、17 complete／8 in_progress；本轮不改变入口状态。
[剩余入口清单](../plans/draw2d-official-examples-next-slices-2026-10-23.md)主要记录
字体、主题、原生栅格、跨平台和输入验收，不再将 AA、下划线或 ScrollBar 列为缺失。
TextFlowLarge 的入口完成事实与长期性能目标分别保留，不能相互替代。

2026-10-26 的[剩余审计](draw2d-official-examples-remaining-audit-2026-10-26.md)是历史快照，
其“AA／低 miter／混合字体装饰未实现”已由后续实施替代。原审计、日志和 JSON
保持原字节；本轮更新当前计划、账本和导航，不重写历史结论。
