# Core 公开 API 语义与命名审计

类型：`verification`

状态：`P0 implemented`

实施状态：`Batch A/B complete; Batch C/D unscheduled`

日期：2026-09-22

P0 实施日期：2026-09-24

## 1. 审计信息

- Novadraw 基线：`d90f90b9bf263dc72db2e563bf877df5d2b9a32d` 加审计时工作区；
- 审计范围：`novadraw-core`、`novadraw-geometry`、`novadraw-render`、
  `novadraw-scene` 及聚合 crate `novadraw` 的公开 API；
- 重点入口：`FigureTree`、`FigureTreeBuilder`、`Runtime`、`LayoutManager`、
  `LayoutSnapshot`、`NdCanvas`、公开事件和基础值类型；
- 明确排除：`novadraw-editor` 的 GEF 层 API、性能优化、功能缺口实现；
- 参考语义：Draw2D `IFigure`、`Figure`、`LayoutManager`，以及
  [`Draw2D API 语义覆盖账本`](../../parity/draw2d/api-coverage.md)；
- 当前设计约束：
  [`总体架构`](../../design/architecture/overview.md)、
  [`静态架构`](../../design/architecture/static-architecture.md)、
  [ADR-009](../../adr/adr-009-runtime-dynamic-mutation-contract.md) 与
  [ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)。

本报告第 4 节保留 2026-09-22 审计时的事实和迁移前名称。P0 目标已由
[ADR-017](../../adr/adr-017-core-public-api-boundary.md) 裁决，并于 2026-09-24
完成 Batch A/B；实施证据见第 9 节。P1/P2 项仍未排期，不改变 Core 1.0、roadmap
或 parity ledger 状态。

## 2. 总体结论

审计时 Core 的行为能力已经形成，但公开 API 尚未达到稳定发布所需的简洁性和一致性：

1. `BlockId/FigureBlock/FigureGraph` 已退出规范模型，公开表面仍残留 `block`；
2. 构建期、运行期、内部 primitive 和只读查询仍集中暴露在 `FigureTree`；
3. 同一能力存在隐式目标、显式目标、吞错 convenience 和结构化错误多套入口；
4. Layout、Graphics、Geometry 各自保留了迁移期兼容方言；
5. 聚合 crate 顶层重导出过多内部协议，稳定 API、扩展 API 和底层 API 没有分层。

`set_block_layout_manager` 应删除，但不能只机械改名为 `set_layout_manager`。当前
`FigureTree` 已有同名一参数方法，且它会根据 `contents` 是否存在隐式选择 contents
或 synthetic root。目标 API 必须先消除这种隐式作用域，再统一名称。

2026-09-24 更新：上述 P0 术语、阶段边界、显式目标和失败模型已完成收口。剩余风险
集中在 Layout measurement、Graphics、Color、Geometry 和 crate root 分层等 P1/P2
项目。

## 3. 审计原则

本次判断遵循以下原则：

- 继承 Draw2D 行为语义，不机械迁移 Java 方法名；
- `FigureId` 是唯一公开节点身份，公共语言统一使用 Figure、node、container；
- 构建期 mutation 由 `FigureTreeBuilder` 表达；
- 运行期 mutation 只经 `Runtime` 的事务入口；
- `FigureTree` 对 Runtime 用户主要提供只读查询；
- 非幂等失败使用结构化 `Result`，不使用 null handle 或 `false` 隐藏错误；
- 使用 `Rectangle`、`Dimension`、`MeasureConstraints` 等领域类型表达参数；
- 一个概念只保留一套规范命名和一个权威行为入口。

## 4. 审计项

### API-01：公开 `block` 术语未完成迁移

优先级：P0

状态：已整改（2026-09-24）

事实：

- 公开身份已经是 `FigureId`，节点类型已经是 `FigureNode`；
- `FigureTree::{set_block_layout_manager,get_block_layout_manager,get_block,block_depth}` 仍
  暴露旧术语；
- `ViewportHandle::block_id`、`ScaleHandle::block_id` 仍返回 `FigureId`；
- `FigureEvent`、`AncestorEvent`、`PropertyChangeEvent`、`ActionEvent` 和
  `NotificationEffect::Notify` 的公开字段仍为 `block_id`；
- `PropertyValue::Block` 仍把 Figure 引用称为 Block。

结论：

`block` 在当前模型中没有独立领域语义，是旧 `FigureBlock` 模型的兼容残留。继续保留
会让用户误以为 Block 和 Figure/Node 是不同抽象。

目标：

| 当前名称 | 目标名称 |
|---|---|
| `get_block` | `node` |
| `block_depth` | `depth` 或 `node_depth` |
| `ViewportHandle::block_id` | `figure_id` 或 `viewport_id` |
| `ScaleHandle::block_id` | `figure_id` 或 `scalable_id` |
| 事件字段 `block_id` | `figure_id` |
| `PropertyValue::Block` | `Figure` |

内部局部变量、字段和注释也应在同一批次统一，以免旧术语重新进入公共 API。

### API-02：LayoutManager 存在隐式与显式两套作用域

优先级：P0

状态：已整改（2026-09-24）

事实：

```rust
FigureTree::set_layout_manager(manager)
FigureTree::get_layout_manager()
FigureTree::set_block_layout_manager(container, manager)
FigureTree::get_block_layout_manager(container)
Runtime::set_layout_manager(container, manager)
Runtime::clear_layout_manager(container)
```

`FigureTree::set_layout_manager(manager)` 和 `get_layout_manager()` 会把操作目标隐式解析为
`contents.unwrap_or(synthetic_root)`。调用方无法从方法签名判断实际容器，且树状态变化
会改变同一调用的目标。

目标：

```rust
FigureTreeBuilder::set_layout_manager(
    container: FigureId,
    manager: Box<dyn LayoutManager>,
) -> Result<bool, BuildError>;

FigureTree::layout_manager(
    container: FigureId,
) -> Option<&dyn LayoutManager>;

Runtime::set_layout_manager(
    container: FigureId,
    manager: Box<dyn LayoutManager>,
) -> Result<bool, RuntimeMutationError>;

Runtime::clear_layout_manager(
    container: FigureId,
) -> Result<bool, RuntimeMutationError>;
```

处理要求：

- 删除隐式作用于 contents/root 的 setter 和 getter；
- 删除 `block` 版本；
- 构建期入口补到 `FigureTreeBuilder`；
- 运行期继续以 `Runtime` 为唯一事务入口；
- 内部 replacement primitive 可保留为 `pub(crate)`，统一命名为
  `replace_layout_manager`；
- builder 安装 manager 时也必须校验当前 parent-owned constraints。

### API-03：FigureTree 的公开职责超过设计边界

优先级：P0

状态：P0 mutation 边界已整改；low-level/headless 表面待 P1 设计

事实：

当前 `FigureTree` 同时公开：

- topology/query；
- bounds、visible、enabled、style 和 size mutation；
- layout manager、constraint 和 validation mutation；
- `UpdateManager` 协作；
- render 和 frame recording primitive；
- notification queue drain；
- debug printing。

这与当前设计中“`FigureTree` 保存拓扑、`Runtime` 原子协调交互和更新、
`FigureTreeBuilder` 承载构建期写入”的职责划分不一致。虽然 `Runtime::tree()` 只返回
不可变引用，类型本身仍向外承诺了多套生命周期阶段的 API。

目标：

- `FigureTreeBuilder`：只承载 pre-Runtime 构建；
- `FigureTree`：稳定只读查询和必要的不可变算法；
- `Runtime`：全部运行期 mutation、validation、damage、render publication；
- internal modules：低层 primitive、UpdateManager 协作和通知队列操作。

是否继续公开 `FigureTree::render`、`perform_update`、`notification_effects` 等低层入口，
需要在实施前单独确认 headless/custom-host 的真实扩展需求。

### API-04：失败模型混合 null、bool、panic 和 Result

优先级：P0

状态：已整改（2026-09-24）

事实：

- `FigureTreeBuilder::add_child_to` 失败时返回 `FigureId::null()`；
- `Runtime::add_figure` 失败时返回 `FigureId::null()`；
- `Runtime::{remove_figure,reparent}` 把结构错误折叠为 `false`；
- `Runtime::set_contents` 对错误 panic，同时另有 `try_set_contents`；
- 多个 `FigureTree` mutation 对 unknown Figure 静默返回；
- 坐标转换分别使用静默 no-op、`bool` 和 `Option`。

这违反 ADR-009 中“非法参数返回结构化错误，`Ok(false)` 只表示幂等无变化”的约定。

目标：

- fallible 公共操作以 `Result` 为规范入口；
- 删除返回 null handle 的 convenience；
- `FigureId::null()` 不作为正常控制流结果；
- `false` 只表示合法目标已经处于目标状态；
- 坐标查询统一返回 `Option<Transform>` 或结构化 `Result`，调用方再决定是否原地应用。

### API-05：`FigureTree::validate` 名称与行为不一致

优先级：P0

状态：已整改（2026-09-24）

事实：

`FigureTree::validate()` 当前只把 contents/root 的 `is_valid` 设置为 `true`，不运行
LayoutManager，不递归验证 children，也不执行 Draw2D `Figure.validate()` 对应流程。
真正的立即验证入口是 `try_revalidate` / `validate_with_update`，稳定运行期入口是
Runtime 的两阶段更新。

结论：

该方法不是命名风格问题，而是公开行为契约错误。应删除，不应简单重命名为另一个
validation 概念。需要直接设置 valid bit 的 primitive 必须保持私有。

### API-06：布局测量仍暴露标量 tuple 与 `-1.0` sentinel

优先级：P1

状态：待设计

事实：

- `LayoutManager::{get_preferred_size,get_minimum_size}` 接受
  `w_hint: f64, h_hint: f64`；
- `-1.0` 表示无约束；
- 返回值是 `(f64, f64)`；
- Figure 层已经存在 `MeasureConstraints` 和 `FigureMeasurement`；
- geometry 层已经存在 `Dimension`。

目标方向：

```rust
fn preferred_measurement(
    &self,
    container: FigureId,
    constraints: MeasureConstraints,
    snapshot: &LayoutSnapshot<'_>,
) -> FigureMeasurement;
```

最低限度也应使用 `MeasureConstraints` 和 `Dimension`，消除裸 tuple 与负数 sentinel。
是否合并 preferred/minimum 两个方法，应以 Draw2D 回退顺序和现有自定义 LayoutManager
迁移成本为依据，不在本报告中直接裁决。

### API-07：查询命名和权威数据源不统一

优先级：P1

状态：部分整改（P0 查询命名已完成，其余 P1）

可直接按 Rust 习惯收敛：

| 当前名称 | 目标名称 |
|---|---|
| `get_contents` | `contents` |
| `get_block` | `node` |
| `get_constraint` | `layout_constraint` 或 `constraint_as` |
| `get_layout_manager` | `layout_manager` |
| `get_points` | `points` |

`FigureNode::{get_preferred_size,get_minimum_size,get_maximum_size}` 不能只做机械重命名。
它们只读取 override/current bounds，不包含 LayoutManager、Figure intrinsic measurement
和 Border 合成，语义弱于 `FigureTree::{preferred_size,minimum_size,maximum_size}`。
应删除这些非权威查询，避免同名概念返回不同结果。

### API-08：聚合 crate 顶层暴露过多内部协议

优先级：P1

状态：待设计

`novadraw/src/lib.rs` 直接重导出大量类型，其中包括：

- `FigureNode`、`NodeState`、`LayoutState`；
- `EventDispatcher`、`NotificationQueue`、`PendingMutations`；
- `RootFigure`、`GraphMutationError`；
- backend session、resource journal 和低层 render command 类型。

这使普通用户无法区分：

- 应用级稳定入口；
- 第三方 Figure/Layout/Router 扩展入口；
- host/backend 集成入口；
- 引擎内部实现类型。

目标方向：

- crate root 只重导出最常用稳定入口；
- `figure`、`layout`、`connection`、`host`、`render` 等模块保留专业扩展类型；
- 建立明确的 prelude 时，只包含高频无歧义类型；
- 内部状态快照若仅供 inspector 使用，提供专门只读 snapshot，而不是重导出运行时内部
  struct。

### API-09：NdCanvas 同时暴露两套绘图方言

优先级：P1

状态：待设计

当前重复或近似入口包括：

- `fill_rect` 与 `fill_rectangle`；
- `stroke_rect` 与 `draw_rectangle`；
- `fill_style` 与 `set_background_color`；
- `stroke_style` 与 `set_foreground_color`；
- `line_width` 与 `set_line_width`；
- `line_style` 与 `set_line_style`；
- `global_alpha` 与 `set_alpha`。

前一组部分方法显式携带颜色/描边参数，后一组依赖 Graphics state。名称差异不足以表达
“低层立即参数”与“Draw2D 状态式绘图”两种语义，调用方容易选错。

处理方向：

- 只保留一套公开 stateful Graphics API；
- 显式携带完整 paint 参数的 command recorder primitive 下沉为 crate-private，或放入
  明确命名的低层类型；
- `commands()` 返回 `&[RenderCommand]`，而不是 `&Vec<RenderCommand>`；
- `damage` 字段与 `damage()/damage_mut()` 不应同时公开。

`NdCanvas::arc` 还存在独立行为问题：参数单位未在签名中表达，`anticlockwise` 未使用，
并固定为八段折线近似。该方法在语义补齐前不应作为稳定公共 API。

### API-10：Color 构造没有兑现公开不变量

优先级：P1

状态：待整改

事实：

- `Color` 文档声明分量范围为 `[0.0, 1.0]`，但字段公开且 `rgba/with_alpha` 不校验；
- `Color::hex` 对长度不足的字符串执行切片并 panic；
- 非法十六进制分量被静默替换为 0 或 255；
- 非 6/8 位的较长字符串可能被部分接受。

目标：

```rust
Color::from_rgba(...) -> Result<Color, ColorError>
Color::from_hex(...) -> Result<Color, ParseColorError>
```

若保留无失败构造器，应明确采用 clamp 语义并在名称或文档中写清。字段是否私有化需结合
序列化兼容性决定，但不能继续声称类型始终满足范围不变量。

### API-11：Geometry 兼容别名扩大了表面但没有新增语义

优先级：P2

状态：待清理

以下别名当前主要由存在性测试维持：

- `PrecisionPoint = Point`；
- `PrecisionRectangle = Rectangle`；
- `PrecisionDimension = Dimension`；
- `AffineTransform = Transform`；
- `Size = Dimension`。

`Precision*` 尤其容易让用户误以为存在不同精度模型。Core 使用 `f64` 时，应保留一个
规范名称并删除无行为差异的兼容名称。当前设计以 `Affine2D` 为规范词；实施时应决定
让 struct 本身采用该名称，还是统一保留 `Transform`，不应长期同时暴露三个名称。

### API-12：公共几何参数泄漏多套表示

优先级：P2

状态：待设计

Geometry 提供 `Point`/`Vec2`，但 `NdCanvas::{line,polyline}` 和 `RenderCommandKind`
直接暴露 `glam::DVec2`。同时 `Point` 本身只是 `Vec2` alias，`Vec2` 又公开内部
`DVec2` 字段。

这不是当前行为错误，但会把 glam 表示固化为跨 crate 公共契约。应选择：

- public scene/render API 使用 `novadraw_geometry::Point` 和 `PointList`；
- backend adapter 内部转换为 glam/kurbo/Vello 类型；
- 若 `RenderCommand` 明确是低层 backend protocol，则将该定位写入模块边界，不再从
  应用级 crate root 无差别重导出。

## 5. 建议目标表面

### 5.1 应用级

```text
Runtime
FigureId
FigureTreeBuilder
Figure + builtin Figures
LayoutManager + builtin Layouts
FigureStyle
Point / Rectangle / Dimension / Insets / Affine2D
PlatformHost / RenderBackend
```

### 5.2 扩展级

```text
LayoutSnapshot / LayoutOutput / LayoutConstraint
FigureEventHandler / FigureLifecycle / AccessibleFigure
ConnectionAnchor / ConnectionRouter / ConnectionLocator
typed component update protocol
```

### 5.3 不应默认位于 crate root

```text
FigureNode / NodeState / LayoutState
NotificationQueue / PendingMutations
EventDispatcher / UpdateManager
RootFigure
raw RenderCommand internals
```

这只是公开表面分层建议。实际可见性变更必须先验证 inspector、custom host、外部 Figure
和自定义 backend 的真实消费路径。

## 6. 建议实施批次

### Batch A：术语和无争议命名（已完成）

- `block` 全面改为 `figure/node/container`；
- 删除 `get_*`；
- 删除无调用点的隐式 LayoutManager getter/setter；
- 更新 book、rustdoc、demo 和测试名称；
- 不改变运行时行为。

### Batch B：构建期与运行期 API 收口（已完成）

- 为 `FigureTreeBuilder` 补齐 layout、constraint、bounds 等构建入口；
- FigureTree mutation primitive 改为 `pub(crate)`；
- 删除 null handle 和吞错 convenience；
- 统一 `Result`、幂等 `Ok(false)` 与 error 类型；
- 删除错误语义的 `FigureTree::validate`。

### Batch C：领域类型与扩展协议（未排期）

- Layout measurement 改用结构化约束和尺寸类型；
- 收口 `LayoutContext` 是否需要公开实现；
- 清理 Geometry 兼容别名；
- 统一 Point/Vector/Transform 表示。

### Batch D：Graphics 与聚合导出（未排期）

- 选择 NdCanvas 唯一公开方言；
- 分离 stateful Graphics 与低层 command recording；
- 明确 `arc` 语义或暂时撤出公开 API；
- 缩减 `novadraw` crate root 重导出。

每个批次均为 breaking API change，应独立提交，不能与 Editor G6 功能开发或其他行为
修复混合。

## 7. 验证要求

实施时至少需要：

1. 编译期 API existence/absence 测试，确认旧名称不再导出；
2. 外部测试 crate 实现自定义 Figure、LayoutManager、Router 和 RenderBackend；
3. builder 与 Runtime 对相同布局场景产生等价树和几何；
4. unknown/foreign/detached Figure 的结构化错误测试；
5. 不兼容 layout constraint 在构建期和运行期均原子失败；
6. measurement 的 bounded/unbounded、baseline 和缓存 key 回归；
7. NdCanvas state stack、角度单位、path 和 backend command 等价测试；
8. `cargo xtask docs`；
9. `cargo xtask check --quick`；
10. 最终 breaking change 批次关闭前执行一次 `cargo xtask check --full`。

## 8. 后续推进门禁

启动 Batch C/D 前应完成以下决策：

1. 确认 `FigureTree` 最终是否只保留 query，或仍支持显式 low-level/headless API；
2. 确认聚合 crate root 的稳定 API 白名单；
3. 确认 Layout measurement 的目标类型，不把 `-1.0` sentinel 带入新接口；
4. 确认 NdCanvas 采用 Draw2D Graphics 方言还是独立 Rust 命名；
5. 为下一批 breaking change 再次确认兼容策略；Batch A/B 已按 `0.1.0` 直接迁移，
   未保留 `#[deprecated]` 双入口。

完成上述设计门禁后，再创建 roadmap delta 或实施计划。ADR-017 只授权了 Batch A/B，
不构成 Batch C/D 的实施授权。

## 9. P0 实施证据

2026-09-24 完成：

- 公共身份术语统一为 `FigureId`、Figure、node 和 container；查询采用
  `contents`、`node`、`depth`、`layout_manager`、`layout_constraint`，句柄和事件
  payload 采用 `figure_id`，属性值采用 `PropertyValue::Figure`；
- 构建期写入集中到 `FigureTreeBuilder`，包括显式 container 的 layout manager、
  constraint、bounds、节点状态、顺序和 `validate_subtree`；
- 运行期结构操作只保留
  `Runtime::{set_contents,add_figure,remove_figure,reparent}` 的 `Result` 入口，
  setter 使用 `Result<bool, RuntimeMutationError>` 区分幂等 no-op 与非法输入；
- 删除隐式 contents/root LayoutManager API、旧 `try_*` 双入口和
  `FigureTree::validate`；
- Builder 和 Runtime 在替换 LayoutManager 前均验证已有 constraints，失败时保持原
  manager 不变；
- 新增/更新
  `d3_runtime_mutation::runtime_node_mutations_distinguish_noop_invalid_bounds_and_foreign_figures`
  与 `m5_layout_contract::builder_rejects_wrong_constraint_type_without_mutating_layout_state`
  等契约测试。

验证结果：

- `cargo test -p novadraw-scene`：通过；
- `cargo test -p novadraw-editor -p novadraw-inspector`：通过；
- `cargo xtask docs`：通过；
- `cargo xtask check --quick`：通过；
- `cargo xtask check --full`：通过。
