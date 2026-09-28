# 4. 布局、校验收敛、重绘区域与帧提交

## 4.1 更新不是立即绘制

**帧**是引擎为一次屏幕呈现准备并提交的完整结果。修改发生时不立即绘制，而是先
记录需要重新计算和重新绘制的事实。

一次修改通常只声明两类事实：

- **失效**（invalid）：几何、布局或派生状态已过期，需要重新计算；派生状态是由
  源状态计算得到、可以重新生成的状态；
- **脏区**（dirty）：某个节点本地区域的可见像素可能变化，需要纳入重绘。

失效描述“数据需要重算”，脏区描述“像素需要重画”，两者不能混为一谈。

它们被延迟合并，在帧边界执行：

```mermaid
flowchart LR
    A[源状态修改] --> B[收集失效项与脏区]
    B --> C[派生状态收敛]
    C --> D[校验与布局]
    D --> E[计算重绘区域]
    E --> F[录制绘制命令]
    F --> G[生成渲染提交包]
    G --> H[后端提交]
    H --> I[完成或重试]
```

严格顺序是关键：如果在布局稳定前计算重绘损伤区域（damage），脏区会基于旧几何
传播，造成漏绘。

## 4.2 布局管理器的读写隔离

**布局管理器**（`LayoutManager`）负责根据容器、子节点和布局约束计算几何结果。
它不能直接修改 `FigureTree`。`Runtime` 先建立只读的**布局快照**
（`LayoutSnapshot`），布局器再把候选变更写入**布局输出**（`LayoutOutput`）：

```rust
pub trait LayoutManager {
    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError>;
}
```

`LayoutOutput` 可记录子节点边界、可见性、失效请求和少量封闭的内建效果。`Runtime`
先完整校验输出，再统一提交。

```mermaid
sequenceDiagram
    participant Runtime
    participant Snapshot as 布局快照
    participant Layout as 布局管理器
    participant Output as 布局输出
    participant Tree as 图形树

    Runtime->>Snapshot: 冻结只读视图
    Runtime->>Layout: 计算布局
    Layout->>Output: 写入子节点边界
    Runtime->>Runtime: 校验完整输出
    Runtime->>Tree: 统一提交变更
```

代码锚点：

- [`LayoutSnapshot`](../../novadraw-scene/src/layout/mod.rs)
- [`LayoutOutput`](../../novadraw-scene/src/layout/mod.rs)
- [`LayoutManager`](../../novadraw-scene/src/layout/mod.rs)
- [`FigureTree::validate_layout_output`](../../novadraw-scene/src/graph/mod.rs#L1991-L2036)

## 4.3 布局约束属于父子关系

**布局约束**（constraint）描述父容器如何放置某个子节点，因此不是子节点的固有
属性，而是父节点与子节点之间的关系：

```text
LayoutState(container)
├── LayoutManager
└── child FigureId -> LayoutConstraint
```

因此删除子节点或更换父节点时，必须原子清理旧父节点中的约束。约束使用受控类型
擦除，布局管理器必须显式验证类型；错误类型不能被静默忽略。

`XYLayout` 的示例：

```rust
fn validate_constraint(
    &self,
    container: FigureId,
    child: FigureId,
    constraint: &dyn LayoutConstraint,
) -> Result<(), LayoutError> {
    if constraint.as_any().is::<XYConstraint>()
        || constraint.as_any().is::<Rectangle>()
    {
        return Ok(());
    }
    Err(LayoutError::ConstraintTypeMismatch { /* ... */ })
}
```

代码锚点：
[`XYLayout`](../../novadraw-scene/src/layout/xy_layout.rs)。

## 4.4 当前具体布局管理器

`LayoutManager` 是可替换的布局策略接口。父容器持有一个具体布局器，布局器只管理
该容器的直接子节点。当前产品代码共有 10 个实现：8 个位于 `layout` 模块，另外
2 个由视口和滚动面板容器专用。测试中的故障注入布局器不属于产品能力。

进入运行期后，应通过借用 `Runtime` 的 scoped editor 安装布局器和子节点约束。例如：

```rust
runtime
    .container(panel)?
    .set_layout_manager(Box::new(GridLayout::new(2).with_spacing(8.0, 8.0)))?;
runtime
    .figure(field)?
    .set_layout_constraint(GridConstraint::fill())?;
```

构建新树时通过 `FigureTreeBuilder::set_layout_manager` 显式指定容器；场景进入运行期
后只能使用 `Runtime::container(panel)?.set_layout_manager(...)`，因为替换布局器还
需要触发约束校验、失效传播和重绘。`ContainerEditor` 只借用 Runtime，不形成第二套
状态所有权。

### 六个 Draw2D 对标布局器

以下六个布局器构成产品交付清单中的通用布局基线：
这里的“对标”表示职责和主要使用方式对应，不表示每个边界行为已经与 Draw2D 完全
等价；精确语义状态仍以
[`Draw2D API 语义覆盖账本`](../../doc/parity/draw2d/api-coverage.md) 为准。

| 布局器 | 子节点约束 | 实际作用 | 典型用途 |
|---|---|---|---|
| `XYLayout` | `XYConstraint` 或兼容的 `Rectangle` | 按每个子节点的显式 `(x, y, width, height)` 放置；宽或高为负数时使用首选尺寸；没有约束的子节点保持原位 | 图形编辑器画布、需要精确坐标的节点 |
| `StackLayout` | 不接受约束 | 让每个子节点都占满容器客户区；多个子节点按照图形树叠放顺序相互覆盖 | 分层面板、多个互相覆盖的内容层 |
| `BorderLayout` | `BorderConstraint` 或兼容的 `Rectangle` | 把客户区分成上、下、左、右、中五个区域；边缘区域可指定厚度，中心使用剩余空间 | 主内容区加标题栏、状态栏或侧栏 |
| `FlowLayout` | 不接受约束 | 按子节点顺序沿水平或垂直主轴排列；空间不足时自动换行或换列；可配置元素间距和行列间距 | 标签组、按钮组、可换行项目列表 |
| `GridLayout` | `GridConstraint` | 按固定列数自动分配网格单元；支持跨行跨列、边距、间距、对齐、填充、尺寸提示、缩进、等宽列和剩余空间分配 | 表单、属性面板、规则对齐的控件矩阵 |
| `ToolbarLayout` | 不接受约束 | 只生成一行或一列；空间不足时按各子节点可压缩量，从首选尺寸收缩到最小尺寸；副轴可拉伸或按起点、居中、终点对齐 | 工具栏、菜单条、单行或单列操作区 |

这些布局器的代码入口：

- [`XYLayout`](../../novadraw-scene/src/layout/xy_layout.rs)
- [`StackLayout`](../../novadraw-scene/src/layout/stack_layout.rs)
- [`BorderLayout`](../../novadraw-scene/src/layout/border_layout.rs)
- [`FlowLayout`](../../novadraw-scene/src/layout/flow_layout.rs)
- [`GridLayout`](../../novadraw-scene/src/layout/grid_layout.rs)
- [`ToolbarLayout`](../../novadraw-scene/src/layout/toolbar_layout.rs)

### 两个引擎辅助布局器

| 布局器 | 子节点约束 | 实际作用 | 与相近布局器的区别 |
|---|---|---|---|
| `FillLayout` | 不接受约束 | 只让第一个子节点占满客户区，其余子节点保持原位 | `StackLayout` 会让所有子节点占满；`FillLayout` 适合只有一个有效内容节点的简单容器 |
| `FreeformLayout` | `FreeformConstraint` | 按自由坐标放置有约束的子节点，宽高可省略并回退到首选尺寸；无约束子节点保持当前边界；测量结果取全部子节点边界的并集 | `XYLayout` 关注显式位置和尺寸；`FreeformLayout` 还负责从自由分布的子节点推导整体内容范围 |

代码入口：

- [`FillLayout`](../../novadraw-scene/src/layout/fill_layout.rs)
- [`FreeformLayout`](../../novadraw-scene/src/layout/freeform_layout.rs)

`FillLayout` 是 Novadraw 的本地便利策略，不属于六个 Draw2D 对标布局器。
`FreeformLayout` 与自由范围容器协作，但“自由范围”不表示跳过视口等祖先裁剪。

### 两个容器专用布局器

| 布局器 | 所属容器 | 实际作用 |
|---|---|---|
| `ViewportLayout` | `ViewportFigure` | 安放唯一内容节点，计算可滚动范围，并让当前滚动位置始终可用 |
| `ScrollPaneLayout` | `ScrollPaneFigure` | 同时排列视口、水平滚动条和垂直滚动条；根据“总是显示、从不显示、按需显示”策略求解滚动条可见性，并扣除滚动条厚度后确定最终视口大小 |

这两个布局器由对应容器在构造时安装，通常不由应用单独创建：

- [`ViewportLayout`](../../novadraw-scene/src/container/viewport.rs)
- [`ScrollPaneLayout`](../../novadraw-scene/src/container/scroll_pane.rs)

滚动条会互相影响可用空间：显示垂直滚动条可能导致水平空间不足，继而需要水平
滚动条。因此 `ScrollPaneLayout` 会执行两轮可见性求解，让两个轴得到一致结果。

#### ViewportLayout 到底做什么

可以把 `ViewportFigure` 看作一块固定大小的观察窗，把它的唯一直接子节点看作窗外的
内容。`ViewportLayout` 在每次 layout 时完成四件事：

1. 决定内容在内容坐标域中的宽和高；
2. 求出内容中哪些坐标可以被这扇窗看到；
3. 将该范围写入水平和垂直 `RangeModel`；
4. 若窗口变小或内容范围缩小，修正已经无法看到完整窗口的旧滚动位置。

这里的“唯一”不是便利约定，而是 Viewport 的结构约束：它只变换、裁剪和滚动一个
contents。多个需要重叠的图层应先组合成一个 LayeredPane 或其他内容根，再作为这个
唯一 contents 放入 Viewport。

**跟踪视口宽高**指内容是否随观察窗的可用尺寸重新测量和分配：

- `tracks_width = true`：把视口客户区宽度作为内容的宽度 hint。普通内容会占用这份宽度，
  但不会被压到小于自己的最小宽度。典型用途是表单、文本流或纵向列表，希望窗口变宽时
  内容跟着变宽，通常不需要水平滚动。
- `tracks_width = false`：内容按自己的首选宽度计算；若首选宽度大于视口，就保留这个
  更宽的内容，从而产生水平滚动范围。典型用途是画布、表格或不能随窗口压缩的图形。
- 高度的 `tracks_height` 使用同一规则。两个开关彼此独立，例如常见的纵向列表会跟踪
  宽度但不跟踪高度。

对于普通内容，设视口客户区宽度为 `W`、内容首选宽度为 `P`、最小宽度为 `M`：

```text
跟踪宽度：内容宽度 = max(W, M)
不跟踪宽度：内容宽度 = max(W, P)
```

因此“跟踪”不等于无条件拉伸，也不等于关闭裁剪；它只改变内容采用视口尺寸还是首选尺寸
作为布局依据。

**结合内容缩放和自由范围**只发生在 contents 明确同时具有 Freeform 与 Scalable
能力时。普通内容不需要这条规则。

- 自由范围（`freeform_extent`）是内容子树实际占据的内容坐标范围，可能从负坐标开始，
  也可能大于 contents 的 presentation bounds。
- 缩放比例 `scale` 属于 contents。视口在屏幕上宽 `client_width`，在内容坐标中实际能
  看到的宽度是 `client_width / scale`；例如屏幕窗口宽 400、`scale = 2` 时，窗口只
  能看到 200 个内容单位。
- 因此布局器先构造一个以内容原点 `(0, 0)` 为起点的“视口基线”，再与
  `freeform_extent` 求并集。这个并集就是可滚动内容范围。基线确保内容比窗口小时仍有
  一个完整窗口大小的范围；并集确保负坐标和越过右下角的自由内容也能滚到。

```text
视口基线 = Rect(0, 0, client_width / scale, client_height / scale)
可滚动范围 = union(freeform_extent, 视口基线)
```

**滚动原点的合法区域**是：原点表示视口左上角正在看的内容坐标，不能滚到窗口右侧或
下侧没有任何内容的位置。对每个轴，`RangeModel` 保存：

```text
minimum  = 可滚动范围起点
maximum  = 可滚动范围终点
extent   = 当前视口在内容坐标中的可见长度
origin   = 当前视口左上角坐标

合法 origin = [minimum, maximum - extent]
```

例如水平方向的内容范围是 `[-100, 600]`，可见窗口宽为 `200`，则原点只能在
`[-100, 400]` 之间。请求滚到 `500` 会被钳制为 `400`，因为此时窗口右边缘正好到
`600`；继续向右只会显示空白。若内容或窗口尺寸变化使原来的 origin 超出新区间，
`ViewportLayout` 会在同一 layout 提交中钳制它，并触发坐标变换更新和视口重绘。

### 如何选择

1. 子节点位置来自业务模型中的明确坐标：使用 `XYLayout`。
2. 子节点自由分布，并且容器需要从其几何推导内容范围：使用 `FreeformLayout`。
3. 容器只有一个内容节点需要铺满：使用 `FillLayout`。
4. 多个语义层需要完全重叠：使用 `StackLayout`。
5. 界面天然分成上、下、左、右、中：使用 `BorderLayout`。
6. 子节点按顺序自然排列，并允许换行或换列：使用 `FlowLayout`。
7. 子节点需要规则的行列、跨格和对齐：使用 `GridLayout`。
8. 子节点必须保持单行或单列，并在空间不足时压缩：使用 `ToolbarLayout`。
9. 需要滚动内容：使用 `ViewportFigure` 或 `ScrollPaneFigure`，由容器安装专用布局器。

布局约束只属于对应布局器与直接子节点之间的私有协议，不能跨布局器复用。没有约束
并不等于“使用默认约束”：例如 `XYLayout` 和 `FreeformLayout` 会跳过无约束子节点，
而 `GridLayout` 会为无约束子节点使用默认的 `GridConstraint`。所有布局器仍只向
`LayoutOutput` 写入候选结果，不直接修改图形树。

## 4.5 Demo 中的实际用法

`apps/native/*` 负责窗口、输入和场景切换，具体可复用场景集中在
`novadraw-demo-scenes`。例如 `layout-app` 的入口只加载
`novadraw_demo_scenes::layout::suite()`，布局器的安装代码实际位于
[`novadraw-demo-scenes/src/layout.rs`](../../apps/scenes/src/layout.rs)。

### Demo 覆盖情况

| 应用 | 场景 | 使用的布局器 | 证明的行为 |
|---|---|---|---|
| `layout-app` | `xy-layout` | `XYLayout` | `Rectangle` 兼容约束控制每个子节点的位置与尺寸 |
| `layout-app` | `fill-layout` | `FillLayout` | 第一个子节点铺满客户区，后续子节点保持原位 |
| `layout-app` | `flow-layout` | `FlowLayout` | 水平顺序排列、元素间距和自动换行 |
| `layout-app` | `nested-layouts` | 外层和区域容器均使用 `XYLayout` | 父子两级布局依次生效，子节点约束只由直接父容器解释 |
| `layout-app` | `constraint-update` | `XYLayout` | 多个子节点使用独立位置约束，并通过重新校验得到布局结果 |
| `layout-app` | `grid-layout` | `GridLayout` | 三列等宽网格、边距、间距和填充对齐 |
| `layout-app` | `toolbar-layout` | `ToolbarLayout` | 水平单行、主轴压缩、副轴拉伸与最小尺寸 |
| `layout-app` | `stack-layout` | `StackLayout` | 所有子节点占用同一客户区并按叠放顺序覆盖 |
| `layout-app` | `border-layout` | `BorderLayout` | 使用兼容的 `Rectangle` 约束表达上、下、左、右、中五区 |
| `layout-app` | `root-viewport-resize` | `BorderLayout` | 使用 `BorderConstraint`，窗口变化后重新分配五区 |
| `update-app` | `validation` | `XYLayout` | 布局失效后重新校验显式坐标约束 |
| `update-app` | `stress` | `GridLayout` | 32 列、1,024 个图形的布局与更新事务 |
| `clip-app` | `responsive_nested_clip` | `BorderLayout` | 根尺寸变化后重排五区，并继续验证嵌套裁剪 |

以下 demo 通过容器构造间接使用专用布局器：

| 应用 | 构造入口 | 自动安装的布局器 |
|---|---|---|
| `viewport-app` | `add_viewport_to` | `ViewportLayout` |
| `scroll-pane-demo` | `add_scroll_pane_to` | `ScrollPaneLayout`，以及内部视口的 `ViewportLayout` |
| `node-editor-demo` | `GraphicalViewer::new` | 编辑框架根图层拓扑中的多个 `StackLayout` |

当前没有 demo 直接安装 `FreeformLayout`。`scroll-pane-demo` 中的
`FreeformLayerFigure` 用于自由范围和溢出语义，不等同于使用 `FreeformLayout`。
这是阅读 demo 时需要明确区分的两项机制。

下面的代码均从当前 demo 提炼，省略颜色、图形构造和错误处理，只保留布局协议相关
调用。

### 示例一：显式坐标布局

`layout-app` 的 `xy-layout` 场景先给容器安装 `XYLayout`，再给每个直接子节点设置
矩形约束：

```rust
let mut builder = scene.builder();
builder.set_layout_manager(container_id, Box::new(novadraw::XYLayout::new()))?;
let child_id = builder.add_child(container_id, Box::new(rect))?;
builder.set_layout_constraint(
    child_id,
    novadraw::Rectangle::new(x, y, width, height),
)?;
builder.validate_subtree(container_id)?;
```

这里的 `Rectangle` 是 `XYLayout` 为兼容 Draw2D 接受的约束类型，不是子节点当前
边界的第二份独立真源。重新校验时，布局器读取约束并生成新的子节点边界。

### 示例二：网格与工具栏

`grid-layout` 场景创建三列等宽网格，并让每个子节点填满其单元格：

```rust
let mut builder = scene.builder();
builder.set_layout_manager(
    container_id,
    Box::new(
        novadraw::GridLayout::new(3)
            .with_equal_column_widths(true)
            .with_margins(40.0, 40.0)
            .with_spacing(20.0, 20.0),
    ),
)?;
let child_id = builder.add_child(container_id, Box::new(rect))?;
builder.set_layout_constraint(child_id, novadraw::GridConstraint::fill())?;
```

`toolbar-layout` 场景则强调空间不足时的压缩行为：

```rust
let mut builder = scene.builder();
builder.set_layout_manager(
    container_id,
    Box::new(
        novadraw::ToolbarLayout::horizontal()
            .with_spacing(16.0)
            .with_stretch_minor_axis(true),
    ),
)?;
builder.set_minimum_size(child_id, Some((100.0, 40.0)))?;
```

两者的差别是：网格布局先求行列轨道，再把子节点放入单元格；工具栏布局始终保持
单行或单列，并在主轴空间不足时参考最小尺寸压缩。

`fill-layout` 场景不设置子节点约束。`FillLayout` 不接受约束，实际铺满行为只由
“第一个子节点”这一顺序决定；向它写入 `Rectangle` 会由 Builder 或 Runtime 在提交前
拒绝。

### 示例三：五区布局与窗口变化

`root-viewport-resize` 使用类型明确的 `BorderConstraint`。根客户区变化时，不需要
应用手工重算每个子节点：

```rust
let mut builder = scene.builder();
builder.set_layout_manager(
    contents,
    Box::new(novadraw::BorderLayout::with_sizes(
        HEADER_HEIGHT,
        FOOTER_HEIGHT,
        SIDEBAR_WIDTH,
        SIDEBAR_WIDTH,
    )),
)?;
builder.set_layout_constraint(
    child,
    novadraw::BorderConstraint::with_size(
        novadraw::BorderRegion::North,
        HEADER_HEIGHT,
    ),
)?;
builder.validate_subtree(contents)?;
```

同一模式也用于 `clip-app` 的 `responsive_nested_clip`：布局器负责尺寸变化后的五区
重排，裁剪测试继续验证重排后的父子几何。

### 示例四：容器自动安装专用布局器

`scroll-pane-demo` 不直接构造 `ScrollPaneLayout`。构建器创建滚动面板时会同时建立
视口、两个滚动条及其专用布局器：

```rust
let pane = graph
    .builder()
    .add_scroll_pane_to(
        root,
        Rectangle::new(PANE_X, PANE_Y, PANE_WIDTH, PANE_HEIGHT),
    )?;

pane.set_contents(&mut graph, &mut updates, Box::new(contents))?;
graph.builder().validate_subtree(pane.pane_id())?;
```

因此应用只表达“创建滚动面板并设置内容”，`ScrollPaneLayout` 和 `ViewportLayout`
负责滚动条可见性、视口尺寸、内容范围和滚动范围的一致更新。

相关入口：

- [`layout-app`](../../apps/native/layout-app/src/main.rs)
- [`layout` demo 场景](../../apps/scenes/src/layout.rs)
- [`update` demo 场景](../../apps/scenes/src/update.rs)
- [`clip` demo 场景](../../apps/scenes/src/clip.rs)
- [`viewport` demo 场景](../../apps/scenes/src/viewport.rs)
- [`scroll-pane-demo`](../../apps/native/scroll-pane-demo/src/main.rs)
- [`node-editor-demo`](../../apps/native/node-editor-demo/src/main.rs)

## 4.6 测量顺序

测量（measure）回答“一个 Figure 希望占多大空间”，排列（arrange，即
`LayoutManager::layout`）回答“它最终放在哪里、实际占多大空间”。两者不能颠倒：
父布局必须先测量子节点，才能计算轨道、换行和对齐；随后才把最终边界写入
`LayoutOutput`。

### 4.6.1 一次尺寸查询包含什么

布局系统有三类尺寸查询：

| 查询 | 含义 | 主要 API |
|---|---|---|
| 首选测量 | 空间充足时希望获得的尺寸与可选 baseline | `preferred_measurement` |
| 最小尺寸 | 布局压缩时仍应保留的尺寸 | `minimum_size` |
| 最大尺寸 | 布局拉伸时允许达到的上限 | `maximum_size` |

首选和最小尺寸都接受 `MeasureConstraints`。父布局可以分别约束宽轴和高轴：

- `Some(value)`：该轴可用的测量上限，必须有限且大于等于 `0`；
- `None`：该轴无约束；
- constraint 本身通常不是最终尺寸。布局器可以把它仅用于换行测量，也可以像
  `GridLayout` 的显式尺寸约束一样把它解释为确定尺寸。

约束通过校验构造器创建：

```rust
let unbounded = MeasureConstraints::UNBOUNDED;
let width_bounded = MeasureConstraints::width(72.0)?;
let bounded = MeasureConstraints::bounded(320.0, 180.0)?;
```

首选测量返回 `FigureMeasurement { width, height, baseline }`，最小和最大尺寸返回
`Dimension`。`baseline` 是从 border-box 顶边到文本基线的距离，没有基线语义的
Figure 返回 `None`。

### 4.6.2 首选尺寸的解析链

首选尺寸按以下优先级解析：

```text
显式指定的尺寸
-> 布局管理器测量
-> 图形自身的内在尺寸测量
-> Figure 默认内在尺寸回退
```

```mermaid
flowchart TD
    A["preferred_measurement(id, constraints)"] --> B{FigureId 存在?}
    B -- 否 --> X["返回 None"]
    B -- 是 --> C["转换容器自己的测量约束"]
    C --> D{存在 preferred override?}
    D -- 是 --> E["投影到父布局坐标域并返回"]
    D -- 否 --> F{节点安装了 LayoutManager?}
    F -- 是 --> G{generation + constraints<br/>命中缓存?}
    G -- 是 --> H["返回缓存尺寸"]
    G -- 否 --> I["LayoutManager::preferred_measurement"]
    I --> J["投影、写缓存并返回"]
    F -- 否 --> K["Figure::intrinsic_measurement"]
    K --> L["叠加 owner-scoped Border 尺寸"]
    L --> M["返回内在尺寸"]
```

每个阶段的具体含义如下。

**阶段一：显式尺寸覆盖**

`Runtime::figure(figure)?.set_preferred_size(size)` 写入节点状态中的显式 override。
它表达的是调用方明确指定的首选尺寸，因此直接终止后续解析：

```rust
runtime.figure(panel)?.set_preferred_size((320.0, 180.0))?;
// 后续 preferred_measurement(panel, 任意 constraints) 都先得到 (320, 180)

runtime.figure(panel)?.clear_preferred_size()?;
// 清除后重新委托 LayoutManager 或 Figure 内在测量
```

首选、最小和最大 override 是三个独立值。设置最小尺寸不会替代首选尺寸，设置首选
尺寸也不会自动改变最大尺寸。Runtime 会拒绝负数和非有限值，并使该节点到根节点的
validation 路径失效。

**阶段二：布局管理器测量**

容器安装了 `LayoutManager` 时，容器尺寸通常由直接子节点聚合得出，而不是由容器
外观决定。例如 `XYLayout` 对每个有约束的子节点计算：

```text
子节点宽 = constraint.width >= 0 ? constraint.width : child.preferred_width
子节点高 = constraint.height >= 0 ? constraint.height : child.preferred_height

容器首选宽 = max(child.x + 子节点宽)
容器首选高 = max(child.y + 子节点高)
```

假设两个子节点的解析结果分别是：

```text
A: x=10,  y=12, width=72, height=48  -> 右下角 (82, 60)
B: x=100, y=20, width=30, height=20  -> 右下角 (130, 40)
```

则 `XYLayout` 测得容器首选尺寸为 `(130, 60)`。这一步只计算容器希望的尺寸，不会
修改 A、B 的边界。

布局管理器只能通过只读快照递归查询子节点：

```rust
pub trait LayoutManager {
    fn preferred_measurement(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> FigureMeasurement;

    fn minimum_size(
        &self,
        container: FigureId,
        constraints: MeasureConstraints,
        snapshot: &LayoutSnapshot<'_>,
    ) -> Dimension;

    fn layout(
        &mut self,
        container: FigureId,
        snapshot: &LayoutSnapshot<'_>,
        out: &mut LayoutOutput,
    ) -> Result<(), LayoutError>;
}
```

`preferred_measurement` / `minimum_size` 是测量算法 API；`layout` 是排列算法 API。
测量方法必须无树写入副作用，排列方法也只能把候选边界写到 `LayoutOutput`。

**阶段三：Figure 内在尺寸**

没有布局管理器的叶子 Figure 通过以下扩展点描述自身：

```rust
fn intrinsic_measurement(
    &self,
    constraints: MeasureConstraints,
) -> FigureMeasurement;

fn intrinsic_minimum_measurement(
    &self,
    constraints: MeasureConstraints,
) -> FigureMeasurement;
```

图片可以返回资源像素尺寸，文本可以根据 `max_width` 换行并返回高度与基线，几何
Figure 可以返回自己的自然包围尺寸。存在 owner-scoped Border 时，引擎先测量内容，
再加上 border insets，并保证结果不小于 border 自身的首选尺寸；基线也会向下偏移
top inset。

**阶段四：默认内在尺寸回退**

若 Figure 没有覆盖内在测量方法，默认实现读取 `initial_bounds()` 的宽高。它是
Figure 构造时声明的自然尺寸，不是布局后 `NodeState` 中不断变化的当前边界。这样
可以避免“本轮排列结果成为下一轮首选尺寸输入”的反馈环：

```rust
fn intrinsic_measurement(&self, _constraints: MeasureConstraints) -> FigureMeasurement {
    let (width, height) = self.intrinsic_size(); // 默认基于 initial_bounds()
    FigureMeasurement::new(width, height, None)
}
```

因此，需要随内容变化的 Figure 应显式实现内在测量，不能依赖上一次 layout 的边界
充当内容尺寸。

这里有两种不同的 `None`：

- 节点状态中的 `preferred_size: None` 表示“没有显式 override”，继续进入下一阶段；
- `FigureTree::preferred_measurement(...) -> None` 表示 `FigureId` 不存在。

当前 `LayoutManager` 和 `Figure` 的具体测量方法都返回确定结果，不使用
`Option<Dimension>` 串联阶段。`Dimension::ZERO` 是合法结果，不能把它当作
sentinel（用特殊值表示“无结果”）。例如显式首选尺寸为零时，布局管理器不会再被调用。

### 4.6.3 调用 API 与算法 API

应用代码只负责表达策略和约束，通常不直接驱动递归测量：

```rust
runtime
    .container(panel)?
    .set_layout_manager(Box::new(XYLayout::new()))?;
runtime
    .figure(label)?
    .set_layout_constraint(XYConstraint::at_size(10.0, 12.0, 72.0, -1.0))?;
runtime.figure(label)?.set_minimum_size((24.0, 16.0))?;
```

其中 `height = -1.0` 仍是 `XYConstraint` 自身的“自动高度”约定，但它不会进入测量
扩展协议。`XYLayout` 会在边界把固定宽度转换为 `MeasureConstraints`：

```rust
let constraints = MeasureConstraints::new(
    (constraint.width >= 0.0).then_some(constraint.width),
    (constraint.height >= 0.0).then_some(constraint.height),
)?;
let preferred = snapshot.preferred_measurement(child_id, constraints).size();
let width = if constraint.width < 0.0 {
    preferred.width
} else {
    constraint.width
};
let height = if constraint.height < 0.0 {
    preferred.height
} else {
    constraint.height
};
out.set_child_bounds(child_id, Rectangle::new(x, y, width, height));
```

布局算法通过 `LayoutSnapshot` 使用以下只读 API：

| API | 何时使用 |
|---|---|
| `preferred_measurement(child, constraints)` | 查询首选宽高与可选文本基线 |
| `minimum_size(child, constraints)` | 计算压缩下限 |
| `maximum_size(child)` | 计算拉伸上限 |
| `container_bounds(container)` | 排列阶段取得当前客户区 |
| `children(container)` / `constraint(child)` | 枚举直接子节点并解释父子约束 |

`FigureTree` 中首选尺寸解析的核心逻辑可简化为：

```rust
fn preferred_measurement(
    id: FigureId,
    constraints: MeasureConstraints,
) -> Option<FigureMeasurement> {
    let node = blocks.get(id)?;
    let constraints = node.layout_constraints(constraints);

    if let Some(explicit) = node.preferred_size {
        return Some(node.project_preferred_measurement(
            FigureMeasurement::new(explicit.0, explicit.1, None),
        ));
    }

    if let Some(layout) = node.layout.manager.as_deref() {
        if let Some(measurement) = cache.get(node.generation(), constraints) {
            return Some(measurement);
        }
        let snapshot = LayoutSnapshot::new(self);
        let measurement = node.project_preferred_measurement(
            layout.preferred_measurement(id, constraints, &snapshot),
        );
        return Some(cache.store_and_return(measurement));
    }

    let content = node.figure.intrinsic_measurement(constraints);
    Some(apply_border(content, node.border_snapshot()))
}
```

最小尺寸使用同一结构，但读取 `minimum_size` override、调用
`LayoutManager::minimum_size` 和 `Figure::intrinsic_minimum_measurement`。最大
尺寸当前不经过布局管理器：显式 maximum override 优先，否则返回无限上限。

### 4.6.4 约束文本测量样例

约束文本是最能体现“先测量、后排列”的例子。假设文本自然尺寸为
`(156, 20)`，但父容器只提供 `72` 的宽度；换行后可能得到 `(72, 48)`，基线为
`15`。父容器必须使用 `48` 作为最终高度。

```mermaid
sequenceDiagram
    participant Parent as 父 LayoutManager
    participant Snapshot as LayoutSnapshot
    participant Child as 文本 Figure
    participant Output as LayoutOutput
    participant Paint as 绘制阶段

    Parent->>Snapshot: preferred_measurement(child, max_width=72)
    Snapshot->>Child: intrinsic_measurement(max_width=72)
    Child-->>Snapshot: FigureMeasurement(72, 48, baseline=15)
    Snapshot-->>Parent: 同一测量结果
    Parent->>Output: set_child_bounds(..., 72, 48)
    Paint->>Child: 使用 72 宽对应的不可变文本布局
```

自定义 Figure 的核心实现形态如下。实际文本整形应在 validation 的派生状态阶段生成
不可变 `TextLayout`，测量和绘制复用同一份 glyph IR：

```rust
impl Figure for WrappedTextFigure {
    fn intrinsic_measurement(
        &self,
        constraints: MeasureConstraints,
    ) -> FigureMeasurement {
        let layout = self.selected_layout(constraints);
        FigureMeasurement::new(
            f64::from(layout.width()),
            f64::from(layout.height()),
            Some(f64::from(layout.baseline())),
        )
    }

    fn paint_figure_in_bounds(&self, canvas: &mut NdCanvas, bounds: Rectangle) {
        let constraints = MeasureConstraints::width(bounds.width)
            .expect("Figure bounds are valid geometry");
        canvas.fill_text_layout(self.selected_layout(constraints), 0.0, 0.0);
    }
}
```

需要基线的布局器应调用 `snapshot.measurement`，而不是只调用
`snapshot.preferred_size`：

```rust
let area = snapshot.container_bounds(container);
for (child, _) in snapshot.children(container) {
    let constraints = MeasureConstraints::width(area.width)?;
    let measured = snapshot.preferred_measurement(child, constraints);
    output.set_child_bounds(
        child,
        Rectangle::new(area.x, area.y, area.width, measured.height),
    );
}
```

约束文本测量必须在布局阶段完成：

```text
父级宽度约束
-> 子节点按约束测量
-> 得到高度、基线和不可变布局快照
-> 排列子节点
-> 放置最终显示内容
```

如果把换行推迟到仅绘制阶段的显示逻辑，父布局无法得到正确高度，只能依赖第二次
全量重绘修补，破坏单帧收敛。

### 4.6.5 缓存与失效

容器的布局测量可能递归访问大量子节点，因此首选和最小尺寸分别缓存。缓存键包含：

```text
(layout generation, MeasureConstraints)
```

相同 generation 和相同 constraints 的重复查询直接复用结果。结构变化、约束变化、
显式尺寸变化或相关几何变化会增加 generation，同时清空首选和最小尺寸缓存；下一次
查询才重新执行布局器测量。缓存只是已计算结果，不是显式 override，也不能跨不同
constraints 复用。

代码锚点：

- [`FigureTree::preferred_measurement` 与尺寸解析](../../novadraw-scene/src/graph/mod.rs)
- [`MeasureConstraints` 与 `FigureMeasurement`](../../novadraw-scene/src/figure/mod.rs#L320-L367)
- [`Figure` 内在测量默认实现](../../novadraw-scene/src/figure/mod.rs#L409-L461)
- [`LayoutSnapshot` 查询 API](../../novadraw-scene/src/layout/mod.rs#L92-L151)
- [`LayoutManager` 算法 API](../../novadraw-scene/src/layout/mod.rs#L301-L354)
- [`XYLayout` 测量与排列](../../novadraw-scene/src/layout/xy_layout.rs#L81-L201)
- [约束文本测量契约测试](../../novadraw-scene/tests/d4_constrained_measurement.rs)

## 4.7 校验阶段如何收敛

**校验**（validation）是把失效的布局和派生状态反复重算，直到没有待处理工作的
过程，并不只是遍历一次。布局提交可能产生新的失效项、自由范围或视口范围更新，
因此 `Runtime` 使用固定优先级工作列表（worklist）：

```text
内在尺寸
-> 布局
-> 依赖失效传播
-> 连接路由
-> 路由后的几何更新
-> 最终显示状态
```

后置阶段产生高优先级工作时，调度器回到高优先级继续处理。全部队列排空才形成
稳定版本。

实际调度见
[`Runtime::stabilize`](../../novadraw-scene/src/runtime/runtime.rs#L3533-L3614)。

为避免错误扩展无限失效，单个阶段有反馈预算。超过预算返回
`FramePreparationError::DidNotConverge`：

- 不生成只完成一部分的渲染提交包；
- 保留待处理工作；
- 请求后续诊断或重试；
- 不在渲染热路径打印日志。

## 4.8 两阶段更新管理器

**更新管理器**（`UpdateManager`）汇总失效项和脏区，并按“先校验、后计算重绘区域”
的两阶段协议准备一帧。它是 Runtime 的内部协作者，不与公开 `FigureTree` 组合成
另一套宿主入口。应用通过 `Runtime::figure(figure)?.{revalidate,repaint}` 请求工作，
通过 `Runtime::{prepare_submission,prepare_frame,record_full_frame}` 驱动帧。

内部更新事务先完成校验，再冻结脏区快照：

```rust
self.perform_validation_phase(graph)?;
self.update_queued = false;
let snapshot = self.take_dirty_snapshot();
let damage = prepare_damage_set(graph, canvas, snapshot.iter());
if damage.is_some() {
    graph.render_to(canvas);
}
```

实际实现见
[`UpdateManager::perform_update_transaction`](../../novadraw-scene/src/runtime/update/deferred.rs#L517-L550)。

冻结快照的意义是：计算重绘区域或绘制期间新产生的脏区不会被当前遍历意外消费，
而会保留到下一事务。

## 4.9 重绘区域如何传播

一个图形对象的脏矩形位于节点本地域。最终重绘区域的计算如下：

```text
本地脏区
-> 与当前视觉边界求交
-> 应用节点定位
-> 应用父节点的子内容变换
-> 与父级有效裁剪区求交
-> 重复直到根节点
-> 归一化多个区域
-> 得到逻辑表面域的 DamageSet
```

概念伪代码：

```rust
for step in parent_chain {
    dirty.transform(step.transform);
    if let Some(clip) = step.clip {
        dirty = dirty.intersection(clip)?;
    }
}
```

实际实现：
[`propagate_damage_through_parent_chain`](../../novadraw-scene/src/runtime/update/repair.rs#L63-L84)。

这条链必须与绘制和命中测试使用同一子内容变换与裁剪策略。

## 4.10 重绘区域合并与正确性

`DamageSet` 是本帧需要重绘区域的集合，有三种状态：

```text
None
Full
Partial { union, regions }
```

`regions` 是用于优化的分散区域，`union` 是包围所有区域的单一矩形。区域过多时可以
安全退化为 `union`；后端无法可靠保留重绘区域外像素时必须升级为全量重绘（`Full`）。

真正的不变量是：

> 帧提交后，重绘区域外的可见像素必须与提交前等价。

它不强制所有后端使用同一种局部呈现（partial present）技术。

代码锚点：

- [`DamageSet`](../../novadraw-render/src/submission.rs)
- [`normalize_damage_regions`](../../novadraw-scene/src/runtime/update/repair.rs)

## 4.11 帧准备状态机

**帧准备状态机**用明确状态表示当前能否产生一帧。`prepare_submission_state`
不用 `Option` 混淆所有情况，而是区分：

```rust
pub enum FramePreparation {
    Ready(RenderSubmission),
    Idle,
    Suspended,
    AwaitingCompletion,
    Error(FramePreparationError),
}
```

完整顺序：

```mermaid
flowchart TD
    A[开始准备提交] --> B{是否已故障锁定?}
    B -->|是| E1[返回错误]
    B -->|否| C{前一帧仍在提交?}
    C -->|是| W[等待完成]
    C -->|否| D[应用待处理修改]
    D --> S{绘制表面可用?}
    S -->|否| P[暂停并保留全量重绘请求]
    S -->|是| T[收敛派生状态]
    T --> A11[发布无障碍信息]
    A11 --> R[录制增量或全量命令]
    R --> C1[检查后端能力]
    C1 --> RS[冻结资源快照或增量]
    RS --> F[分配会话与帧 ID]
    F --> READY[准备完成]
```

实际实现见
[`Runtime::prepare_submission_state_inner`](../../novadraw-scene/src/runtime/runtime.rs#L3653-L3745)。

## 4.12 后端会话与资源基线

**后端会话**（backend session）表示一段连续使用同一后端资源基线的提交序列。新
会话必须先收到资源全量快照（`Snapshot`），之后才能消费资源增量（`Delta`）。提交
携带：

- `BackendSessionId`
- `FrameId`
- `ResourceSync`

后端入口可以拒绝旧代数或缺失快照的切换。提交失败时 `Runtime` 恢复资源增量，或
重新要求全量快照，并将下一帧提升为全量重绘。

这解决了“旧 `Runtime` 的迟到帧污染新绘制表面”与“新后端不知道已有资源”的问题。

## 4.13 通知为什么最后发布

内部变化按因果顺序写入通知日志（notification journal），但监听器只在稳定版本形成
后收到记录。否则监听器可能观察到：

- 边界已改但连接尚未重新路由；
- 路径已改但定位器尚未更新；
- 内容范围已改但视口范围尚未钳制到合法区间；
- 文本度量已改但最终显示状态仍旧。

通知记录保留 `source_epoch` 与 `sequence`，查询上下文指向发送通知时最新的稳定场景。

## 4.14 失败模式

| 错误 | 后果 |
|---|---|
| 重绘区域计算先于校验 | 重绘区域基于旧几何，产生残影 |
| 布局管理器直接改树 | 可变借用重入，无法原子校验 |
| 绘制阶段反向改变布局 | 帧无法稳定，依赖第二次重绘 |
| 脏区不记录来源图形 | 无法正确应用变换与裁剪 |
| 局部重绘后端不保留旧像素 | 重绘区域外内容丢失 |
| 资源切换没有全量快照 | 后端缓存缺少基线 |
| 监听器在中间状态执行 | 外部观察到半提交场景 |

## 4.15 验证入口

- [`m5_layout_contract.rs`](../../novadraw-scene/tests/m5_layout_contract.rs)
- [`d4_constrained_measurement.rs`](../../novadraw-scene/tests/d4_constrained_measurement.rs)
- [`d4_component_update.rs`](../../novadraw-scene/tests/d4_component_update.rs)
- [`d4_notification_epoch.rs`](../../novadraw-scene/tests/d4_notification_epoch.rs)
- [`runtime_resize_contract.rs`](../../novadraw-scene/tests/runtime_resize_contract.rs)
- `cargo xtask run core.runtime`
- 规范 SSOT：
  [`update-manager.md`](../../doc/design/rendering/update-manager.md)
