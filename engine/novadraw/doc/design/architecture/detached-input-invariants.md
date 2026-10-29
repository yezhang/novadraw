# Detached 输入不变量设计

类型：`proposal`

状态：已接受；2026-10-29，S08-A 已获实施批准。决策入口为
[ADR-056](../../adr/adr-056-detached-input-invariants.md)。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 输入先受检，再进入可用对象

本专题定义 S08 的 Figure、Border 和 Style 输入边界。目标来自
[审计清单](../../verification/checklists/audit-remediation-execution.md#9-p2-s08detached-figure--border--style-输入不变量)
及 ADR-020／024 的受检值原则。现行样式、盒模型和组件更新契约继续生效；
S08-A 的现行正文随实现更新，其余切片按各自验收更新。

输入处理按发生顺序分为：

1. 原始配置构造受检值，或构造完整候选；失败返回领域错误。
2. 候选在挂载／更新前检查外部协议输出、身份、组合关系和本次派生计算。
3. 成功发布后更新 revision、validation、damage 和通知。
4. 帧提交按已有规则检查可执行 IR；后端独立检查设备精度与 capability。

拒绝原始输入不会生成可用非法对象。拒绝尚未提交的候选不会改变已发布状态。
外部回调 panic 仍按现有 lifecycle／frame fault 契约处理，不伪装成普通输入错误，
也不承诺撤销任意回调的外部副作用。

## 已有值与缺口

以下记录实施前的定向盘点，先确定目标契约，再核对实现；没有据此扩大到全部数值 API。
S08-A 的现行接口见下一节，验证结果见
[实施记录](../../verification/reviews/p2-s08-a-implementation-2026-10-29.md)。

| 领域 | 实施前事实 | 本次选择 |
|---|---|---|
| Color | 私有分量；`try_rgba` 拒绝；`rgba` 显式饱和；文本解析返回错误 | 复用，保留 ADR-020 的饱和语义 |
| Opacity | `animation::Opacity` 私有标量，有限且位于 `[0,1]`，输入错误为 AnimationError | 复用同一类型，迁到 graphics 领域，输入错误脱离动画服务 |
| Graphics opacity | 公共 `set_opacity(f64)` 已拒绝非法值；内部 canvas 仍有 clamp | 保留公共拒绝基线，改为安装同一 Opacity，收口内部重复入口 |
| StrokeStyle | 私有配置；检查非负宽度、非负 miter、dash 周期及描边外扩溢出 | Border 复用；Border 的正宽规则单独检查 |
| FontDescriptor | 私有公开访问面；构造检查非空 family、正有限 f32 size、`[1,1000]` weight | FigureStyle 保存 descriptor；沿用 f32 文字单位 |
| Rectangle／Dimension／Insets | 通用几何用途包含原始运算和领域特定约束，不能统一施加正值约束 | 在 bounds、border inset、测量等消费边界校验 |
| RectangleCorners 等轮廓值 | ADR-052 已定义受检参数和按目标几何适配 | 复用；本批不增加第二套圆角值 |
| FigureStyle | alpha 为公开 `Option<f64>`；解析覆盖时 clamp；font 为文本 | 改用已受检值，移除隐式修正和重复解析 |
| BorderBuilder | 接受原始宽度／insets，build 直接写结构字段；默认 inset 与直接构造不同 | builder 自身保持有效，build 复用相同构造规则 |
| Line／Rectangle／Margin Border | 存在构造 assert 与可直接写入的数值字段 | 受检构造／更新，保护数值配置 |
| Rectangle／RoundedRectangle Figure | 原始 bounds／stroke 数值可直接保存；旧圆角构造用 max 截掉负值 | 原始输入构造／更新受检，保护领域数值，保留目标几何适配 |
| Compound／TitleBar Border | 组合加法及文字加 padding 可从有限输入产生无穷值 | 在配置组合与 owner 计算分别拒绝溢出 |

盘点入口：
`crates/novadraw/src/style.rs`、
`crates/novadraw/src/animation/mod.rs`、
`crates/novadraw/src/render/stroke.rs`、
`crates/novadraw/src/render/text.rs`、
`crates/novadraw/src/figure/border/` 和
`crates/novadraw/src/graph/topology.rs`，以及定向核对的
`crates/novadraw/src/figure/rectangle.rs`／`rounded_rectangle.rs`。
Color 修复记录见[既有实施记录](../../verification/reviews/core-color-text-input-2026-10-29.md)。

## 样式保存领域值

S08-A 的公共形状如下：

```rust
pub struct FigureStyle {
    pub alpha: Option<graphics::Opacity>,
    pub font: Option<text::FontDescriptor>,
    // 其余独立覆盖项保持既有类型和继承规则。
}
```

`ResolvedStyle` 使用相同值类型，alpha 默认 OPAQUE，字体使用
`FontDescriptor::default()`。foreground、background、cursor、tooltip 和方向不变。
alpha 仍采用最近祖先覆盖，父子 alpha 不相乘。Color 自身 alpha 与 Figure opacity
在绘制时的组合继续遵循 Graphics 契约。

Opacity 的实现归 graphics，动画仅实现 AnimationValue 等消费协议。
`Opacity::try_new` 返回 GraphicsInputError，区分非有限与越界原因；不保留
`animation::Opacity` 公开 alias。动画入口用既有领域错误包装并保留 source。
公共 `Graphics::set_opacity` 接受 Opacity；内部录制层只接收它的有效标量，
不为原始 f64 再保留一个 clamp 入口。
Graphics 的现有 session／录制失败仍经 GraphicsError 返回；值已受检不消除这些失败。

FigureStyle 各字段可独立替换，因为其值已受检且没有跨字段不变量。
不为样式增加强制 builder 或全私有字段。接收原始文本／标量的应用入口先调用
`FontDescriptor::parse`／`Opacity::try_new`，成功后组装样式。

字体文本语法沿用本项目的 px 描述，不扩大到 CSS shorthand：
可选 style／weight token，接一个正有限的 `sizepx` token，随后为非空 family。
weight 可用 `normal`、`bold` 或 `[1,1000]` 数值；重复或冲突的 style／weight 拒绝。
解析按完整 token 识别字号，不能把 `700 12px Inter Variable` 的首个数字当作字号，
也不能丢弃负号、接受多余前缀或截断 Unicode。已有合法描述需进入迁移回归。
不存在于字体集合的 family 仍是合法请求，按既有 font matching／fallback 处理；
构造合法描述不保证资源就绪或排版成功。

## Border 构造保护真实配置

建议构造和更新保留一套名称。现有 `new` 若接受未受检输入则改为 Result；
不同时保留会 panic 的 new 和受检 try_new。以下为后续切片的目标签名：

| 对象／入口 | 返回与校验 |
|---|---|
| `LineBorder::new(color, width)`、`RectangleBorder::new(color, width)` | `Result<Self, BorderInputError>`；正有限宽度及 StrokeStyle 完整校验 |
| `from_stroke(color, stroke)` | 同上；受检 StrokeStyle 仍需满足 Border 正宽规则 |
| `with_insets(Insets)` | Result；四侧有限非负，横纵合计有限 |
| `with_stroke(StrokeStyle)` | Result；正宽检查；重新计算自动 inset |
| `BorderBuilder::new(color, width)`、`with_insets(Insets)` | Result；builder 的每个可观察状态有效 |
| `build_line`、`build_rectangle` | 直接返回；使用受检配置，删除无独立语义的 `build` alias |
| `MarginBorder::new(Insets)`、`with_margins(Insets)` | Result；边距与合计检查；删除兼容 color／width／style |
| `BevelBorder::new(style, highlight, shadow, width)` | Result；保留整数宽度，拒绝零值；转换与边距计算受检 |
| `TitleBarBorder::with_padding(horizontal, vertical)` | Result；有限非负，二倍 padding 有限 |
| `CompoundBorder::new`、`from_optional` | Result；静态 child 指标与组合计算受检 |
| 默认构造、EtchedBorder 的受检颜色构造 | 直接返回；固定有效配置，无原始非法数值入口 |
| SchemeBorder 的颜色序列构造 | 保留直接返回；Option 色带与空层语义不变，长度到指标的派生检查留在指标边界 |

Line／Rectangle Border 保存 Color、StrokeStyle 和 inset 配置，不重复保存 width、
dash、cap、join。数值配置私有，提供只读 getter；Color 等可独立替换的受检值
无需因本批而机械私有化。RectangleShapeBorder 继续复用 ADR-052 的轮廓与分边规则。
现有 BorderStyle 的迁移使用统一 DashPattern；历史 DashDot 映射须明确记录，
真实 dash-dot 对等能力另按 parity delta 处理，不能借类型迁移宣称已补齐。

迁移保持当前可见描边：Solid 转 Solid，Dash 转 Dash，Dot 转 Dot；
历史 DashDot 实际按 Dash 绘制，迁移继续使用 Dash。此映射是兼容决定，
不表示采用了上游完整 dash-dot 行为。

inset 配置区分“随宽度自动生成”与“调用方显式设置”。直接构造与 builder
默认都采用四侧等于宽度；更新 stroke 时自动 inset 跟随，显式 inset 保持原值。
只保存配置意图，自动 inset 从 stroke 派生，避免两份可写宽度状态。
显式 inset 可小于描边宽度；它控制 client area，描边仍在 owner border box 内绘制。
本批不新增 inset 大于等于 width 的限制。

MarginBorder 默认四侧为零。修改单侧也校验整个候选的横纵合计；
原对象不先写后检查。默认空 CompoundBorder 仍有效，optional 缺失保持空 Border
语义。任意嵌套的 child inset 加法与 preferred size 合成均检查溢出。

共享 Border 不保存 owner 的字体、bounds 或 Runtime。TitleBar 的配置先保证
二倍 padding 可表示；与实际文字宽高相加时再次检查。不同 owner 的快照独立，
失败不安装新快照，也不把旧字体快照标为当前成功结果。

Compound 构造只读取每个直接 child 的基础 inset 和 preferred 指标各一次，
检查同一组值的非负、有限与组合运算；不创建 owner，不解析标题字体，
也不保留一个供所有 owner 共用的标题测量快照。外部 child 的内部可变状态可能在
构造后改变，挂载与替换仍须重新准备当前指标。构造成功只证明当次基础候选合法。
owner 准备读取各 child 的当次快照或基础指标，检查与发布使用同一组值；
外部回调不能通过前后两次不同返回值绕过校验。

## 几何按用途校验

| 输入／输出 | 要求 | 合法边界 |
|---|---|---|
| Figure bounds | x、y、width、height 有限；尺寸非负；本次边界／变换计算可表示 | 负坐标、零尺寸合法 |
| Border／Figure 的初始 inset | 四侧有限非负；横纵合计有限 | 零边距、不对称边距合法 |
| preferred／minimum／owner border metrics | 沿用 S07 测量协议；尺寸与本次组合结果受检 | 空内容、零尺寸合法 |
| 约束 | 沿用领域协议 | `None` 无上限不伪装成非法无穷尺寸 |
| 圆角／viewport range | 原始配置先受检，再执行既有算法 | 圆角随目标缩放、range 截到可滚动区间合法 |
| 后端设备值 | 精度转换、下溢／溢出和 capability 独立检查 | 有限 f64 不意味着设备 f32 可执行 |

Insets 仍可作为通用几何运算值；Border 内保存已校验的私有 Insets，
外部 `initial_insets()`／`Border::get_insets()` 返回的原始值在消费时检查。
不新增 Alpha01、NonNegativeF64、全局“有效几何”标记。
BorderInsets 与通用运算 Insets 的意义也不靠类型别名伪装。

小 owner 扣除大边距后形成空 client area，或描边已无可画矩形时，保留现行截空行为。
检查必须先拒绝非有限输入和派生溢出，再执行几何截空；不能靠 max／min 将 NaN
或无穷输入变成合法结果。

## Detached Figure 同样保护数值

内置 Figure 接收原始 bounds、stroke 或 corner 参数的入口，也须在返回可用对象前
完成校验。Rectangle 的 new／from_bounds／new_with_color，以及 RoundedRectangle
对应构造按同一规则返回领域 Result。bounds／stroke 的原始数值字段不能继续
公开可写；数值更新准备完整候选再替换，颜色等独立受检字段可保持公开。
同类 Figure 沿用已有 ShapeGeometryError 和 StrokeStyle，避免每类重复校验。

Detached `translate`、`set_bounds` 等原始数值更新返回 Result，包括有限输入相加溢出。
Shape 的描边宽度零合法；安装 StrokeStyle 不额外施加 Border 的正宽限制。
接受外部 Border 的组合入口先检查静态指标，owner 相关指标随后按统一准备协议检查。
具体 Figure 的旧 Bounded setter 若能绕过这些检查，需同步收口，不能另留未受检 alias。

RoundedRectangle 旧 `corner_radius` 参数仍按既有“等宽高 corner dimensions”解释，
不借迁移改成半径；负值和非有限值改为拒绝。有效大圆角在目标矩形上适配仍合法。
实施前补齐同类 Figure 的入口清单和公开边界，按上述契约迁移，不能只修复两个
已核对的类就宣称所有 detached Figure 已通过 S08。

## 外部协议输出也要检查

第三方 Figure 的构造由下游负责，Core 无法保证任意 trait 实现的私有状态。
Core 保证它暴露的初始 bounds、insets、style、border 指标和组件候选在本次发布前
通过与内置对象相同的检查。不得按 concrete 类型跳过准入，也不新增内置专属 hook。

初始属性在 prepare 时读取并保存为本次候选；检查与发布消费同一份值。
独立调用回调两次、检查第一次却提交第二次，会留下绕过校验的窗口。
外部 Border 指标应在显式替换／失效前稳定；带内部可变状态的实现仍需在每次
指标准备时受检。构造检查不能替代后续动态 owner 测量检查。
现有 registry、typed identity 和 mutation 提交链复用，不建立第二套 Border registry。

`FigureMut::set_style` 安装已验证样式仍返回 RuntimeMutationError：
这是身份／fault／发布失败通道，不能因局部值已受检而删除。
更新失败保留旧值、源 revision、通知和 damage；同值成功返回未变化。
准入检查保留 namespace、拓扑、深度、资源和跨对象关系，不能由本批删掉。
Deferred mutation 的错误继续从现有 Runtime 失败通道观察。

## 错误保留字段与原因

BorderInputError 从 figure 的规范领域路径导出，区分非有限字段、负 inset、
非正宽度和组合溢出。Inset 字段有稳定 side 标识；嵌套错误保留 outer／inner
路径和底层 source。StrokeStyle／字体错误复用原类型，不仅转成 String。
这里只分类输入字段和错误，不以 enum 封闭第三方 Border 类型。

GraphMutationError／RuntimeMutationError／测量错误按阶段包装这些原因，
同时保留对象身份，不将所有失败压成不带原因的样式错误或默认尺寸。
后端错误继续独立分类。原始配置错误可恢复，恢复方式是提交新有效候选。
S09 的服务 fault、Inspector poison 和订阅所有权不混入本批。

## 迁移分片与验收

| 切片 | 修改范围 | 必须同时交付 |
|---|---|---|
| S08-A | Opacity 领域迁移、FigureStyle／ResolvedStyle、字体 token 解析、Graphics 输入 | 公共消费者、API snapshot、非法输入与继承回归 |
| S08-B | Border 构造、builder、Margin 配置、描边复用 | 构造／更新负测、直接构造与 builder 等价、原始字段 compile-fail |
| S08-C | Detached Figure 数值入口、初始 inset、组合／owner metrics、外部候选准入 | 同类入口清单、外部 Figure／Border、溢出反例、失败后旧状态与恢复 |
| S08-D | 全部下游调用与现行文档迁移 | 渲染／布局／damage、Native GPU、产物与最终门禁 |

每片先验证目标契约，再迁移调用点。关闭原审计项须具备全部切片的实施证据。
随实现更新 figure-style、reusable-shape-border、public-api-contract、文字专题、
ADR-026 的 Opacity 路径及 parity 失败语义。
产品 schema、serializer 与持久化兼容不进入 Core 验收。
具体反例和检查范围见[验证矩阵](../../verification/plans/p2-s08-detached-input-invariants.md)。

## 性能与包体积

预期标量校验为常数时间，custom dash 为配置长度线性时间，Compound 指标准备为
组合节点数线性时间。校验放在构造、候选和失效后的指标准备，不在每次 paint
重复扫描未变化的配置，也不扩大树失效范围。

Opacity 保持一个 f64 的值语义，动画默认 feature 不因领域迁移而被启用。
FontDescriptor 与样式源共用已有字体缓存和 revision 机制，不添加资源副本。
StrokeStyle 含共享 dash 数据，Clone 应复用共享存储；迁移原 Copy 边框后须检查
快照复制与缓存分配。错误路径可携带来源，正常渲染路径不格式化错误或打印日志。

实施前后以同一工具链、release 配置、消费者和场景测量：
标量构造批次、实际频率的 style 更新、共享／嵌套 Border 指标准备、稳定帧、
连续 resize 和输入响应。报告 P50／P95、分配与复制次数、缓存命中和数据规模。
复用代表性样例，不新增按每个 f64 建类型的微基准体系。

同配置 Native／Wasm 探针记录原始及压缩产物，分别比较默认与
`--no-default-features` 消费边界，核对字体等首次资源没有新增。
不预设未经测量的性能或体积阈值，各切片在实施记录中登记成本数据和覆盖范围。

## Draw2D 语义参考

仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`。
提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
只读 `org.eclipse.draw2d`，未编写或运行 Java 验证。

| 源码／方法 | 源码事实 | Novadraw 选择 |
|---|---|---|
| `Border.getInsets/getPreferredSize/paint` | 可共享 Border；inset 影响 client area；在 owner 内按累计 inset 绘制 | 保留协议与 owner 隔离；增加受检输入和指标 |
| `LineBorder.setWidth/getInsets` | 非正宽度保留旧值；四侧等于宽度 | 非正宽度返回错误；自动 inset 同步，显式 override 为既有扩展 |
| `MarginBorder` 构造／paint | 只保存 Insets；不绘制 | 移除无行为的兼容颜色／宽度／样式 |
| `CompoundBorder.getInsets/getPreferredSize/paint` | outer／inner 累计指标和有序绘制 | 保留组合；f64 加法显式检查溢出 |
| `Figure.getFont/setFont` | 局部字体覆盖，未设置则继承父级 | 使用同一 FontDescriptor 值，字体解析提前 |

alpha 范围和继承以本项目现行样式契约为准，不从上游未出现的 Figure alpha
接口推导。覆盖账本见
[Draw2D API family](../../parity/draw2d/api-coverage.md#api-family-id-约定)。
