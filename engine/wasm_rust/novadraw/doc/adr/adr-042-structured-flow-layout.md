# ADR-042：结构化文字流与内嵌 Figure

类型：`architecture-decision`

## 状态

已接受，实施中（2026-10-21，用户批准按 ADR-042 实施）。
承接官方示例 K05 与 P2-T03。`api_semantics` 为 `text.flow`、
`text.interaction`、`layout.manager` 和 `input.dispatch`。

## 来源与目标

只读参考 `org.eclipse.draw2d.text` 的 FlowFigure、FlowPage、BlockFlow、
InlineFlow、TextFlow、FlowAdapter、FlowBorder，以及六个官方文字示例。
不编写、编译或运行 Java，不参考 Zest。

TextFlowExample 包含六个 block：标题、自动换行、只按换行符分行的源码、
继承颜色的嵌套 inline、不同字号共享 baseline、逻辑尾侧对齐。
BidiBlockExample 组合 LTR/RTL 与 leading/trailing/physical left/right，
两个 Canvas 共享 vertical range；共享范围继续使用现有 RangeModel。
TextSurroundingFigureExample 包含普通 Toolbar/三 Shape 子树和嵌套 RTL page，
它们以真实 Figure 参与行布局，仍接受自己的布局、输入和生命周期。

FlowAdapter 先以当前行剩余宽度测量；当前行已占用且放不下时换行并重新测量。
其 baseline 位于盒底，descent 为零。InlineFlow 可跨行形成多个 fragment，
命中按 fragment，不能只检查包围矩形。FlowBorder 分片绘制，携带
leading/trailing 与方向；相邻和嵌套 block 的垂直 margin 允许折叠。

## 领域模型

保留现有 TextFlowFigure 的窄调用面；新增结构化 flow 容器与开放贡献协议，
不把所有文字节点和内嵌 Figure 压成一个失去身份的字符串。

每个 flow 节点注册同一 typed capability，其 payload trait 向布局器提供
带来源身份的文字、inline group、block 或可测量 Figure 贡献。
内置 page/block/inline/text/adapter 与外部节点走同一 registry；
不在 Runtime 按具体 Figure 类型分支，也不给内置节点保留专用 hook。
普通 Figure 子树通过 adapter 贡献占位盒，保留原有树身份。

拟定的公开协议骨架如下；名称在实现阶段可调整，权限边界需要保留：

```rust
trait FlowContributor {
    fn contribute(
        &self,
        context: &FlowContributionContext<'_>,
        output: &mut FlowContributionSink,
    ) -> Result<(), FlowError>;
}

trait FlowFragmentBorder {
    fn metrics(&self) -> Result<FlowBorderMetrics, FlowError>;
    fn paint_fragment(
        &self,
        fragment: &FlowBorderFragment,
        context: &mut PaintContext<'_>,
    ) -> Result<(), GraphicsError>;
}
```

ContributionContext 只读当前节点身份、解析样式和受检 measurement；
Sink 只追加 typed contribution，不允许修改树、访问平台或提交渲染。
公开 ContributionKey/payload trait/registry 定义贡献身份和处理策略，
标准 text/group/block/figure-box handler 也必须注册，不以闭合内置枚举分派。
服务处理 contribution 后产生受检 flow 快照，再由普通布局效果提交。

Block 拥有方向、alignment 和段落边界；inline 只组合贡献和继承样式。
leading/trailing 与 physical left/right 分开表达，方向继承从最近 block 取得。
字体、颜色和边框来自正常 Figure 样式与 typed 属性更新。

## 排版与快照

文字引擎增加 backend-neutral 的 styled request：UTF-8 源文本、受检 style
范围、段落方向与真实 Figure 占位盒。现有单样式 layout 作为该协议的适配入口；
扩展默认引擎与外部引擎使用同一输入和受检输出，Parley 类型保持私有。
同段文字整体 shaping 与 bidi；字体或 inline 边界不得破坏跨节点文字上下文。

flow 布局服务持有实例范围缓存，读取冻结 scene/style/measurement 前缀，
生成同一不可变快照：source mapping、line、baseline、fragment、
glyph run、内嵌 Figure placement、边框片段和交互几何。
布局、paint、hit、caret 与 selection 必须引用同一 revision。
UTF-8 与 affinity 是 Core 契约；Java UTF-16 和闭区间 selection 只在示例适配处转换。

内嵌 Figure 布局通过受检 LayoutOutput 提交；提交后才验证该子树。
测量或输出失败保留旧快照，不发布半段 fragment 或部分 child bounds。
文字、样式、字体资源、贡献树及内嵌测量 generation 变化统一使快照失效。
反馈使用既有稳定更新预算，禁止 paint 期间重排或修改模型。

## 边框与输入

开放 FlowBorder payload 接受 fragment rect、方向和首尾标记。
普通 Border 不被强制转换为 FlowBorder；block 的普通边框仍按盒绘制。
margin 折叠由 flow 布局负责，border 只返回受检度量并绘制。
选择高亮沿既有显式绘制策略，不恢复未支持的 XOR 指令。

文字命中先定位真实 fragment，再映射来源节点、UTF-8 offset 和 affinity；
跨行 fragment 间的空白不能命中 inline 内容。内嵌 Figure 输入复用正常命中、
坐标变换与事件分发；不引入示例私有事件点换算。

## 外部消费者与失败证据

必须有外部 flow contributor、外部 fragment border、替代文字引擎及自定义
内嵌 Figure 的独立 integration target。不修改 Core 即可注册、更新和销毁。
失败反例覆盖错误 style 范围、非有限盒指标、过期 revision、测量失败、
非法 placement、移除后悬空来源和不收敛反馈，拒绝时保留旧前缀。

行为验证覆盖嵌套样式、跨字号 baseline、自然换行/仅换行符分行、RTL 相对与
绝对对齐、fragment 空白命中、margin 折叠、边框首尾、内嵌 Figure 二次测量、
字体完成后重排及新旧 damage。六个官方入口按依赖逐个接入和截图。

## 实施顺序与评审点

先实现 styled request 与外部引擎证明，再实现开放贡献、block/inline 快照；
随后接入 border 与真实 Figure，最后补齐交互、范围共享与六个入口验收。
每步先定义失败契约，再实现消费者和定向验证。

评审需确认：结构贡献以 Figure capability 承载；样式范围与占位盒由文字引擎
统一 shaping/bidi；flow 服务拥有快照而 Figure 树保留节点身份。
以上改变跨越公共文字 API 与布局契约，已按 CLAUDE.md 的重大架构变更规则
完成评审批准。ScrollBar 切片已独立完成。

## 实施证据

文字引擎切片已提供 `StyledTextRequest` / `StyledTextLayout`、受检 UTF-8
style span、段落方向与对齐、内嵌盒，以及 `TextLayoutEngine::layout_styled`。
`RuntimeText` 和 `MeasureContext` 共用该入口；默认引擎的单样式入口适配到同一
排版路径。`Graphics::fill_styled_text` 与 `PaintContext::fill_styled_text`
同时支持 glyph 和轮廓绘制。分段颜色也参与交互 revision。

Parley 0.7 的显式方向由适配层维护私有方向标记与原文偏移映射。
内嵌盒产生负 leading 时，适配层统一修正行、glyph、盒与交互坐标，
保留相邻文字的 descent，不把修正交给示例。
独立消费者与失败反例见
`novadraw/tests/p2_t03_styled_text_contract.rs`。

通用 preparation 协议已增加 `PreparationScene` 和 `PreparedScene`。
前者只读 preparing Figure 的冻结子树、解析样式和受检测量；
后者将根 drawing、子节点 presentation 与普通 `LayoutOutput` 一起缓冲。
Runtime 先校验完整批次及输入 revision，再发布；非法 placement、测量失败、
输入 revision 漂移和错误的 parent-child 归属均保留旧 bounds 与旧快照。
`FigurePresentation` 同时承载不可变领域数据与片段命中区域，片段空白不会阻止
普通后代的输入遍历。独立外部消费者见
`novadraw/tests/p2_scene_preparation_contract.rs`。

开放贡献协议已增加 `ContributionKey<P>`、payload trait 和实例 registry。
标准 text/group/block/figure-box/fragment-border handler 与外部 contributor
使用同一注册入口。`FlowPageFigure`、`BlockFlowFigure`、`InlineFlowFigure`、
`StructuredTextFigure` 与 `FlowAdapterFigure` 通过正常 capability 接入，
Runtime 不按这些 Figure 的具体类型分派。

`FlowSnapshot` 保存段落整体 shaping、来源范围、片段、内嵌位置与边框片段。
绘制、片段命中、UTF-8 caret 和跨节点 selection 共用该快照；
scene 与文字引擎 generation 一起组成 revision。真实子树位置通过
`LayoutOutput` 提交，adapter 先测量行尾剩余宽度，必要时换行并按全宽重新测量。
普通子 Figure 保留绘制与输入身份。source 移出 flow 后，通用 preparation
按 owner 清除已退出快照的 descendant presentation。

fragment border 通过同一贡献协议注册，首尾占位盒参与段落布局。
边框收到来源、方向、owner style、分行矩形与首尾标记；需要 owner 字体测量
的边框可在 preparation 阶段生成不可变 renderer，不在 paint 中重排。
独立消费者见 `novadraw/tests/p2_t03_structured_flow_contract.rs`，
覆盖来源身份、嵌套样式、共享 baseline、相邻与嵌套 margin、RTL 对齐、
片段空白命中、二次测量、字体失效、来源迁移和非法边框指标拒绝。
styled 输出另增加跨 style source range 的拒绝校验。

`FlowLayoutService` 已开放，内置 page 与外部自定义 root 共用同一 preparation
入口和实例缓存。候选通过完整检查后才进入缓存；scene、文字引擎或排版区域变化
都会更新快照 revision。`PreparedScene` 允许组合根绘制并保留子节点和布局效果，
供 caret、selection 等显式绘制策略使用。

page/block 的普通 Border 通过正常边框 capability 接入；已解析的 owner insets
参与排版，嵌套 block 的盒边界与内容位置分别计算，各层内边距只计入一次。
通用 prepared measurement 按内容尺寸叠加普通 Border 或已解析的 owner Border；
flow 子节点发布内容尺寸，避免静态边框漏算和 owner 边框重复计入。
`SetFlowBlockStyle` 支持受检更新。inline 边框跨 block 时只在全局首尾预留标记，
仅含真实 Figure 盒的边框也参与布局。独立替代引擎见
`novadraw/tests/p2_t03_external_flow_engine.rs`，自行构造 styled 输出和 interaction
provider，覆盖来源映射、绘制、命中、caret、selection 与跨节点视觉移动。

内嵌子树反馈使用实例范围的派生检查点。失败时恢复旧 presentation、各节点几何、
viewport 范围、Connection route points 和装饰几何，丢弃未发布通知并重新失效布局。
活动提交的源属性、源 bounds、拓扑和连接绑定保留；恢复过程不克隆源组件。
连接的派生 generation 和反向依赖索引恢复后重新进入 Dirty，locator 测量缓存失效。
外部装饰与内置装饰共用 geometry capture/commit capability，外部消费者可通过受检
`PreparedDecorationGeometry::new` 构造几何。
`p2_scene_preparation_contract` 覆盖晚发测量失败、不收敛、源写入、viewport 以及
实际发生重路由后的外部装饰恢复。

`FlowBorderMetrics` 增加四侧 insets 和 margin；同一标准贡献 registry 提供
`BorderedBlockContribution`，外部 block 可复用，内置 block 不走特权路径。
block 边框间距进入既有 margin 折叠；inline 的首尾横向间距参与 shaping，
各行的垂直间距同步调整 glyph、轮廓实例、内嵌盒、caret 和 selection。
边框度量与不可变 renderer 在贡献遍历时一次准备，绘制阶段不测量。
跨段上下移动保留全局横向位置并支持空段落的来源身份。
独立契约测试覆盖非对称四侧间距、block 折叠、空段移动和轮廓坐标。

六个官方入口仍待后续实施和验收；
本 ADR 尚未完成全部验收。

结构化 flow 切片的 workspace fmt、check、Clippy 与 test 已通过
（1,191 项通过、1 项忽略）；文档门禁通过。命令退出码与日志保存在
`target/verification/adr042-flow/`。测试 target 的 Clippy 另行复查。
