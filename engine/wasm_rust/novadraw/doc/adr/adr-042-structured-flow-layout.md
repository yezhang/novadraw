# ADR-042：结构化文字流与内嵌 Figure

类型：`architecture-decision`

## 状态

设计草案，待评审；未进入实现。
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
以上改变跨越公共文字 API 与布局契约，按 CLAUDE.md 的重大架构变更规则，
本草案获批后才进入实现。ScrollBar 切片已独立完成，不受本评审影响。
