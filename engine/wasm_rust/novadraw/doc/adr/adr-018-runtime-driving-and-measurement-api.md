# ADR-018: Runtime 驱动与结构化测量 API

类型：`architecture-decision`

## 状态

已接受

## 背景

ADR-017 已收口构建期和运行期 mutation，但 Core 公开表面仍保留两类阶段泄漏：

- `FigureTree` 公开驱动 `UpdateManager`、排空通知和直接录制帧的低层方法；
- Layout 扩展协议使用 `w_hint`、`h_hint`、`-1.0` 和 `(f64, f64)` 表达测量。

坐标 API 也同时存在纯变换查询和静默原地修改两套入口。后者对 unknown Figure
静默 no-op，无法区分恒等变换与查询失败。

这些接口会让宿主、自定义 LayoutManager 和 Editor 继续依赖内部生命周期与 sentinel，
因此应在后续引擎扩展扩大调用面前完成收口。

## 决策

### 1. Runtime 是更新和帧发布入口

`FigureTree` 保留构建器、只读查询和命中测试等必要不可变算法。以下能力限制为
crate 内部：

- notification queue 的读取与排空；
- `UpdateManager` 的 invalidation、damage 和 validation 协作；
- 直接执行 update；
- 直接录制 Figure tree。

生产宿主通过 `Runtime::{prepare_submission,prepare_frame,record_full_frame}` 驱动帧。
需要显式请求局部更新时，通过
`Runtime::figure(figure)?.{revalidate,repaint}` 提交，unknown、foreign 或 disposed
Figure 在 scoped editor 获取阶段返回 `RuntimeMutationError`。

`UpdateManager` 仍可作为公开扩展协议类型供 listener 等能力使用，但不再与公开
`FigureTree` 组合成第二套运行时。

### 2. 坐标 API 只返回变换

公开坐标查询统一为：

- `local_to_parent_transform`;
- `parent_to_local_transform`;
- `local_to_surface_transform`;
- `surface_to_local_transform`;
- `child_content_to_surface_transform`。

所有查询返回 `Option<Affine2D>`。调用方自行把变换应用到点、矩形或其他几何对象。
删除静默修改输入的 `translate_*` convenience，避免 unknown Figure 被解释为成功。

### 3. Layout 测量使用领域类型

`MeasureConstraints` 表达每个轴的可选上限；`UNBOUNDED` 表示两个轴均无约束。
Layout 扩展协议不再接受 `-1.0`：

```rust
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
```

保留 preferred 与 minimum 两条查询，以维持 Draw2D 的回退顺序。Preferred 使用
`FigureMeasurement` 传播 baseline；minimum 和 maximum 使用 `Dimension`。
`FigureTree` 与 `LayoutSnapshot` 使用同一组类型，缓存键由结构化 constraints 构成。

`XYConstraint` 等布局约束自身的自动尺寸语义不属于本决策；布局器必须在调用测量 API
前把它们转换为 `MeasureConstraints`，不得继续传播 sentinel。

### 4. 本批次直接 breaking

项目版本仍为 `0.1.0`，已知调用点均在 workspace 内。本批次直接迁移，不保留
deprecated 双入口。

## 错误处理

- Runtime 更新请求对无效 Figure 返回 `RuntimeMutationError`；
- 坐标查询失败返回 `None`，不修改调用方数据；
- `MeasureConstraints` 只接受有限、非负上限，非法值在构造边界返回错误；
- LayoutManager 返回确定测量结果；零尺寸是合法值，不作为 sentinel。

## 扩展点与稳定性

- 自定义 Figure 继续通过 intrinsic measurement 扩展；
- 自定义 LayoutManager 只依赖 `LayoutSnapshot` 和 `LayoutOutput`；
- custom host 通过 Runtime 帧边界工作，不依赖 FigureTree 内部队列；
- 本决策不处理 Graphics、Color、Geometry alias 或聚合 crate 导出。

## 验证

- 外部测试 crate 无法调用 FigureTree 低层驱动方法；
- Runtime 的局部重绘、重验证、panic recovery 和完整帧录制通过；
- 坐标父链、逆变换和 unknown Figure 查询通过；
- bounded/unbounded、baseline、固定轴 hint 和测量缓存键通过；
- `cargo xtask docs`、`cargo xtask check --quick`；
- 批次交付前执行一次 `cargo xtask check --full`。

## 关系

- 延续 [ADR-003](adr-003-rust-runtime-and-geometry-boundaries.md) 的 Runtime 所有权边界；
- 完成 [ADR-017](adr-017-core-public-api-boundary.md) 的 API-03/API-04 剩余项；
- 公开更新请求的命名空间由
  [ADR-019](adr-019-composable-api-and-scoped-editors.md) 调整为 scoped editor，
  Runtime 仍是唯一提交权威；
- 落实 Core API 审计 API-06 的领域类型迁移；
- 对应 `api_semantics`：
  `figure.tree`、`figure.geometry.bounds`、`layout.manager`、
  `validation.protocol`、`notification.figure`。

## 日期

2026-09-24
