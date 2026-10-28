# Draw2D Text、Label 与 TitleBarBorder 源码语义

类型：`reference-analysis`

范围：M10.2

源码基线：Eclipse GEF Classic commit
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 1. 证据入口

- `org.eclipse.draw2d/Label.java`
- `org.eclipse.draw2d/TextUtilities.java`
- `org.eclipse.draw2d/AbstractLabeledBorder.java`
- `org.eclipse.draw2d/TitleBarBorder.java`
- `org.eclipse.draw2d/text/FlowFigure.java`
- `org.eclipse.draw2d/text/TextFlow.java`

## 2. Label

`Label` 同时承载文本和可选图标。其产品语义不是单次 `drawText`：

- preferred size 使用完整文本尺寸；
- minimum size 使用省略号尺寸与完整文本尺寸的逐维最小值；
- bounds 宽度小于 preferred width 时截断文本，并追加 `...`；
- 文本与图标仅在两者都存在时应用 gap；
- text placement 决定文本位于图标的 east、west、north 或 south；
- label alignment 控制文本与图标组成的整体在 bounds 内的位置；
- text alignment 和 icon alignment 控制两者在交叉轴上的位置；
- text、font、icon、gap 或 placement 改变会清理测量和位置缓存；
- alignment 变化只影响位置缓存与 repaint。

截断不是按字符平均宽度估算。`TextUtilities` 重复调用真实字体测量，寻找可放入可用
宽度的最大前缀。

## 3. TextUtilities

`TextUtilities` 是 Draw2D 对平台字体测量的统一入口：

- `getStringExtents` 测量单行字符串；
- `getTextExtents` 处理换行和 tab；
- `getAscent` / `getDescent` 提供 baseline 指标；
- `getLargestSubstringConfinedTo` 基于真实测量选择最大可见前缀。

Draw2D 使用全局 `INSTANCE`，因为 SWT 的字体和 Display 生命周期由平台控制。
Novadraw 不复制该 singleton；文本上下文必须由 Runtime 显式拥有。

## 4. TitleBarBorder

`TitleBarBorder` 的指标依赖 owner Figure 的有效字体：

```text
top inset = text height + vertical padding
preferred width = text width + horizontal padding
preferred height = text height
```

绘制时：

1. 将标题区域高度限制为 `text height + vertical padding`；
2. 裁剪到标题区域；
3. 填充背景；
4. 按 left、center 或 right 对齐文字；
5. 使用 owner 的有效字体或 Border 的显式字体绘制。

label、font 或 padding 改变会清除 text extents 和 insets cache。颜色和 alignment
只影响绘制。

## 5. TextFlow

`TextFlow` 不是 Label 的实现细节，而是段落流式布局：

- 一个文本可拆成多行 fragment；
- line break、leading word width、bidi 和 baseline 参与布局；
- bounds 是 fragment 布局的派生结果；
- text 改变需要重新执行 flow 与 bidi validation。

M10.2 首批只建立可支持后续 TextFlow 的 shaping、measurement、line breaking 和
glyph snapshot 基础，不复制 selection、caret 或富文本编辑能力。

## 6. 对 Novadraw 的约束

1. 测量、截断与绘制必须消费同一种 shaping/layout 结果。
2. 字体或文本变化必须使 intrinsic/preferred/minimum size 缓存失效。
3. alignment 不能改变 preferred size。
4. glyph shaping 不属于 Vello backend；backend 只消费 glyph id 和位置。
5. 文本上下文不得使用全局 singleton。
6. 自定义字体 Ready/Failed/Removed 必须通过既有资源事务使依赖 Figure 重新布局。
7. Label 与 TitleBarBorder 不得各自实现一套字体测量。
