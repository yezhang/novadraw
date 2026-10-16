# ADR-018: Runtime 驱动与结构化测量 API

类型：`architecture-decision`

## 状态

已接受；第 1 节中的公开帧入口已由
[ADR-031](adr-031-graphics-recording-and-publication-boundary.md) 收紧

2026-10-12 同步 ADR-033：驱动经 Runtime 子 facade；测量与坐标值合同保持有效。

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

生产宿主通过 `runtime.frames().prepare_submission(...)` 驱动帧，完整 backend
结果交回 `runtime.frames().complete_submission(...)`。旧
`prepare_frame` / `record_full_frame` 返回内部录制状态的入口已由 ADR-031 删除。
需要显式请求局部更新时，通过
`Runtime::figure(figure)?.{revalidate,repaint}` 提交，unknown、foreign 或 disposed
Figure 在 scoped mutable facade 获取阶段返回 `RuntimeMutationError`。

`UpdateManager` 仍可作为公开扩展协议类型供 listener 等能力使用，但不再与公开
`FigureTree` 组合成第二套运行时。

### 2. 坐标 API 只返回变换

公开坐标查询统一为：

- `local_to_parent_transform`;
- `parent_to_local_transform`;
- `local_to_surface_transform`;
- `surface_to_local_transform`;
- `child_content_to_surface_transform`。

本节的失败合同已由 [ADR-038](adr-038-query-identity-and-failure-semantics.md) 替换：
三个正向查询返回 `Result<Affine2D, CoordinateQueryError>`，两个逆向查询返回
`Result<Option<Affine2D>, CoordinateQueryError>`。先校验身份，再执行计算；逆向的
`Ok(None)` 仅表示有限输入在数学上奇异。非有限输入、逆结果不可表示和数值计算失败
保留结构化分类。调用方自行把变换应用到几何对象。
五个查询及内部受检求逆已在 P2-S07 坐标切片实施；当前行为验收直接运行
`p2_coordinate_query_failure`、`m4_coordinate_contract` 与内部 `--lib s07_` 测试，结果见
[实施核验](../verification/reviews/p2-s07-query-identity-completion-2026-10-16.md)。

### 3. Layout 测量使用领域类型

`MeasureConstraints` 表达每个轴的可选上限；`UNBOUNDED` 表示两个轴均无约束。
Layout 扩展协议不再接受 `-1.0`：

```rust
fn preferred_measurement(
    &self,
    container: FigureId,
    constraints: MeasureConstraints,
    snapshot: &LayoutSceneQuery<'_>,
) -> Result<FigureMeasurement, LayoutError>;

fn minimum_size(
    &self,
    container: FigureId,
    constraints: MeasureConstraints,
    snapshot: &LayoutSceneQuery<'_>,
) -> Result<Dimension, LayoutError>;
```

保留 preferred 与 minimum 两条查询，以维持 Draw2D 的回退顺序。Preferred 使用
`FigureMeasurement` 传播 baseline；minimum 和 maximum 使用 `Dimension`。
`FigureTree` 与 `LayoutSceneQuery` 使用同一组类型，缓存键由结构化 constraints 构成。
ADR-038 补充身份校验与测量准入：尺寸和 baseline 在组合、投影及成功缓存写入前
受检；非法 callback 成功值与派生溢出通过 LayoutError 返回，普通失败不自动成为
Runtime fault。内置布局器与外部布局器使用相同通道。

`XYConstraint` 等布局约束自身的自动尺寸语义不属于本决策；布局器必须在调用测量 API
前把它们转换为 `MeasureConstraints`，不得继续传播 sentinel。

### 4. 本批次直接 breaking

项目版本仍为 `0.1.0`，已知调用点均在 workspace 内。本批次直接迁移，不保留
deprecated 双入口。

## 错误处理

- Runtime 更新请求对无效 Figure 返回 `RuntimeMutationError`；
- 坐标查询按 ADR-038 区分身份、数学奇异与几何计算失败，不修改调用方数据；
- `MeasureConstraints` 只接受有限、非负上限，非法值在构造边界返回错误；
- LayoutManager 通过 Result 返回测量或错误；零尺寸是合法值，不作为 sentinel。

## 扩展点与稳定性

- 自定义 Figure 继续通过 intrinsic measurement 扩展；
- 自定义 LayoutManager 只依赖 `LayoutSceneQuery` 和 `LayoutOutput`；
- custom host 通过 Runtime 帧边界工作，不依赖 FigureTree 内部队列；
- 本决策不处理 Graphics、Color、Geometry alias 或聚合 crate 导出。

## 验证

- 外部测试 crate 无法调用 FigureTree 低层驱动方法；
- Runtime 的局部重绘、重验证、panic recovery 和完整帧录制通过；
- 坐标父链、逆变换和 unknown Figure 查询通过；
- bounded/unbounded、baseline、固定轴 hint 和测量缓存键通过；
- `cargo xtask docs`、workspace fmt/check；
- 批次交付前执行一次 workspace fmt、Clippy 与 test。

## 关系

- 延续 [ADR-003](adr-003-rust-runtime-and-geometry-boundaries.md) 的 Runtime 所有权边界；
- 完成 [ADR-017](adr-017-core-public-api-boundary.md) 的 API-03/API-04 剩余项；
- 公开更新请求的命名空间由
  [ADR-019](adr-019-composable-api-and-scoped-editors.md) 调整为 scoped mutable facade，
  Runtime 仍是唯一提交权威；
- 落实 Core API 审计 API-06 的领域类型迁移；
- 对应 `api_semantics`：
  `figure.tree`、`figure.geometry.bounds`、`layout.manager`、
  `validation.protocol`、`notification.figure`。

## 日期

2026-09-24
