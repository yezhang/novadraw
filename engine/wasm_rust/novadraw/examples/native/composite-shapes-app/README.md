# Composite Shapes

独立组合样例：两张卡片由普通 Figure、Label 和 child tree 构成标题区与内容区。
下方展示仅上两角圆角的缺底边矩形、逐角三角形及凹多边形。
标题区域的结构、配色和尺寸都在样例中，Core 只提供通用圆角轮廓与分边描边。

```bash
cargo run -p composite-shapes-app
cargo run -p composite-shapes-app -- --screenshot
```

矩形半径顺序：左上、右上、右下、左下；边顺序：上、右、下、左。
`RectangleCorners` 使用真实半径，旧 `corner_dimensions` 仍使用圆角宽高。
分边配置第一批共用线宽并使用实线。相邻两边都开启时保留圆角接合；
相邻边关闭时，描边在直线切点使用 Butt 平齐端帽。下方两角为零时，左右描边
完整延伸到底部，以水平截面结束。
布局 insets 和 child clipping 按原协议单独配置。

```rust
use novadraw::{
    figure::{EdgeStroke, RectangleCorners, RectangleEdges, RoundedRectangleFigure},
    graphics::Color,
};

let rectangle = RoundedRectangleFigure::new(0.0, 0.0, 220.0, 150.0, 0.0)
    .with_stroke(Color::BLACK, 3.0)
    .with_corners(RectangleCorners::circular([20.0, 12.0, 0.0, 0.0])?)?
    .with_edges(RectangleEdges([
        EdgeStroke::Inherit, // 上
        EdgeStroke::Inherit, // 右
        EdgeStroke::None,    // 下
        EdgeStroke::Inherit, // 左
    ]))?;
```

已挂载 Figure 使用 `runtime::SetRectangleCorners`、`SetRectangleEdges`、
`SetTriangleCornerRadii`、`SetPolygonCornerRadii` 或 `ReplacePolygonGeometry`，
通过 `FigureMut::update_component` 更新。Polygon 点数变化时应一次替换点和半径。

验证：`cargo xtask verify shape.independent-corners`；需要本机 GPU。
