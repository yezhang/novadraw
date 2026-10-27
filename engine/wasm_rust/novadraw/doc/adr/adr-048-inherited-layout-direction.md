# ADR-048：继承的布局方向与文字片段定位

类型：`architecture-decision`

状态：Accepted，2026-10-27 用户批准官方样例能力补充方案。

`api_semantics`：`figure.properties`、`text.flow`、`text.interaction`、
`viewport.scroll_zoom`、`damage.repaint`。

## 决策

`figure::LayoutDirection::{LeftToRight,RightToLeft}` 属于普通 FigureStyle，
未设置时继承，根默认 LeftToRight。构造、子树描述和 Runtime scoped style mutation
共用既有协议；通知使用 `LAYOUT_DIRECTION: PropertyKey<Option<LayoutDirection>>`。
它独立于段落方向和几何变换，不反射字形轮廓。

LayoutSceneQuery 与 EventSceneQuery 提供同一已解析方向。
方向修改使子树布局及文字准备失效，旧新像素由既有发布路径擦除、重绘。
相同 style 写入不改变版本。准备源前缀已包含解析样式，外部 Figure 无特权分支。

StyledTextRequest 保存布局方向并纳入 revision。Flow 按段落 owner 的解析样式
提供方向。镜像行交换水平对齐与片段位置；片段内的字形保持正常朝向和顺序。
同一位置映射用于 glyph、inline box、caret、selection 与 hit testing；
位置变化不得只更新 paint。外部文字引擎消费相同请求并返回受检不可变结果。

ScrollPane 按解析方向安排纵向滚动条侧别；横向滚动输入消费同一方向。
纵向 RangeModel 的值与共享语义保持一致。普通容器可以覆盖祖先方向。

## 源码与采用边界

Draw2D 提交 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`：
`LightweightSystem.RootFigure.isMirrored`、`BlockFlow.getOrientation`、
`BlockFlowLayout.addBelowPreviousLine`、`LineRoot.bidiCommit/contiguousCommit`。
上游在 mirrored control 域反向放置片段；本引擎在布局准备阶段发布可消费的位置，
不引入 SWT 类型或另一个输入坐标域。Auto 保留既有文字方向自动检测；
官方 Bidi 消费者显式提供 LTR/RTL。

## 外部消费者与成本

独立消费者覆盖继承、局部覆盖、方向 mutation、两种段落方向与五种对齐、
glyph 不反射、换行、组合字符、caret affinity、选择与共享滚动。
外部 LayoutManager 和文字引擎读取相同方向。实施记录须区分自动验证与平台呈现。

方向增加一个样式值和请求字段，不增加依赖或字体。镜像准备按片段数线性生成
映射，静态帧沿用准备结果；禁止逐帧重建。命中按所在行片段查询，
选择输出按相交片段拆分。连续窄宽重排与同配置产物成本在本批收尾记录。
