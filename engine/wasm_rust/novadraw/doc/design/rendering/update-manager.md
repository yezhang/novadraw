# UpdateManager 契约

类型：`normative-design`

本文是 Novadraw validation、damage repair 和 frame preparation 的 SSOT。Draw2D
机制事实见 `doc/reference/draw2d/rendering/update-manager.md`。

## 1. 目标

UpdateManager 将多次局部变化合并为一个确定的更新事务：

```text
collect invalid / dirty
→ Validation
→ Damage Repair
→ Paint Recording
→ RenderSubmission
```

Validation 必须先于 Damage Repair。UpdateManager 不决定平台何时 redraw，也不拥有
FigureTree。

## 2. 所有权

```rust
pub struct UpdateManager {
    invalid: InvalidSet,
    dirty: DirtySet,
    phase: UpdatePhase,
    generation: UpdateGeneration,
    listeners: ListenerRegistry,
}
```

它由 Runtime 独占持有，使用 `FigureId` 引用节点：

- FigureTree 保存节点及 valid 状态；
- UpdateManager 保存待处理集合和事务阶段；
- Runtime 保证修改 valid 状态与入队是同一个原子操作；
- PlatformHost 只观察“是否需要 redraw”；
- RenderBackend 只接收最终 submission。

UpdateManager 默认是具体类型。队列、region 合并和调试策略可以内部替换。

## 3. Invalidation

### 3.1 invalidate

```text
invalidate(id, reason)
→ mark node validity stale
→ find required validation root
→ insert root into InvalidSet
→ request redraw on empty-to-nonempty transition
```

要求：

- 重复 invalidate 幂等；
- parent layout 依赖 child measurement 时向上失效；
- container layout 结果改变时向下使受影响 child 失效；
- disabled 不影响 validation；
- invisible 节点可以推迟昂贵工作，但必须保留 stale 状态。

### 3.2 Validity

单个布尔值不足以支持长期缓存诊断时，可以内部使用 generation：

```rust
pub struct Validity {
    layout_generation: u64,
    transform_generation: u64,
    style_generation: u64,
}
```

公开语义仍是“当前节点是否满足本帧绘制所需的一致状态”，不要求调用方理解缓存字段。

## 4. Validation

```text
freeze invalid generation
→ remove redundant descendants when ancestor is a validation root
→ validate parent before descendants
→ measure container
→ calculate LayoutOutput
→ atomically apply child bounds
→ collect newly generated invalidations
→ repeat until stable
```

Runtime 先构造不可变 `LayoutSnapshot`，再调用 LayoutManager 把结果写入
`LayoutOutput`。布局器借用释放后统一提交，避免 LayoutManager 同时借用自身状态和
整棵树，也避免在遍历中重入修改树。

### 4.1 收敛

正常完成条件是 invalid set 为空。为防止错误 Figure/Layout 无限制造 invalid：

- 记录每个 generation 的处理次数和因果来源；
- 使用可配置事务预算作为诊断保护；
- 超出预算返回 `NonConvergingValidation`；
- 保留未完成工作，不伪装为成功；
- 调试信息按产生顺序报告 invalidation chain。

这不是静默截断算法，而是对不满足收敛契约的实现显式报错。

## 5. Dirty 收集

dirty source 由以下操作产生：

- repaint；
- bounds 或 transform 的 old/new visual area；
- visibility/style/border 改变；
- attach/detach/reparent；
- scroll/zoom；
- 资源完成；
- surface 恢复或 resize。

dirty region 必须带来源坐标域：

```rust
pub struct DirtySource {
    figure: FigureId,
    local_region: Rect,
    reason: DamageReason,
}
```

同一 Figure 的区域可以提前合并，但不能在 transform 或 clip 语义不同的节点之间直接
合并 local rectangle。

## 6. Damage Repair

Validation 稳定后冻结 dirty snapshot。每个 source：

```text
local region
→ expand by stroke/filter/effect
→ map through shared coordinate chain
→ intersect effective ancestor clips
→ project to logical-surface conservative AABB
→ merge
```

输出：

```rust
pub enum Damage {
    None,
    Full,
    Partial(DamageRegions),
}

pub struct DamageRegions {
    union: Rect,
    regions: Vec<Rect>,
}
```

- `union` 是所有 regions 的正确包围；
- `regions` 是减少 overdraw/present cost 的优化信息；
- region 数量阈值和合并算法是策略；
- region 过多时可退化为一个 union；
- surface 内容丢失、首帧和不确定效果必须使用 Full。

## 7. Damage 正确性

核心约束不是“后端必须按 union clip”，而是：

> frame 提交后，Damage 外的可见像素必须保持与提交前等价。

允许的后端策略：

- partial raster + retained surface；
- tile cache；
- full rerender + full present；
- full rerender + platform partial present；
- CPU/software clipped redraw。

后端若无法可靠保留 Damage 外像素，必须提升为 Full，不能忽略损坏区域造成残影。

## 8. Paint Recording

UpdateManager 在稳定 scene snapshot 上驱动绘制录制：

```text
Damage::None    → no frame unless platform requires one
Damage::Full    → record complete visible scene
Damage::Partial → backend/recorder policy chooses partial or full recording
```

Figure paint 只写入 RecordingCanvas，不访问平台 surface 或 GPU 对象。

绘制遍历保持：

- parent self before children；
- children forward Z-order；
- border/foreground after children；
- per-node state isolation；
- shared coordinate and clip protocol。

## 9. RenderSubmission

```rust
pub struct RenderSubmission {
    pub commands: CommandStream,
    pub damage: Damage,
    pub resources: ResourceDelta,
    pub surface: SurfaceInfo,
    pub frame_id: FrameId,
}
```

`RenderSubmission` 是进程内稳定语义信封。它不承诺：

- 固定内存布局；
- 跨语言 ABI；
- 零拷贝反序列化；
- chunk patch；
- 网络安全格式。

这些能力属于 DisplayList proposal。

## 10. 平台调度

```text
pending work: empty → non-empty
→ Runtime calls PlatformHost.request_redraw()
→ host coalesces requests
→ redraw callback calls Runtime.prepare_frame()
→ RenderBackend.submit()
```

UpdateManager 不依赖 winit、DOM 或 OS run loop。同步更新事务和异步 redraw scheduling
是两个不同概念。

## 11. Mutation 协作

顶层更新前必须先提交已冻结的结构 mutation：

```text
apply mutations
→ collect old/new damage
→ invalidate affected layout roots
→ Validation
→ Damage Repair
```

UpdateManager 执行期间不得提交新的结构 mutation。layout/lifecycle 回调产生的结构
请求进入下一事务。

## 12. 通知

通知按阶段分层：

```text
will_validate
did_validate
will_record(damage)
did_prepare(submission metadata)
did_submit(result)
```

- Figure 几何通知不混入 update listener；
- listener effect 延迟到内部可变借用释放后执行；
- listener 的发生顺序必须可观测；
- listener 不能重入当前 update transaction。

## 13. 失败恢复

| 失败 | 行为 |
|---|---|
| validation 不收敛 | 返回诊断错误，保留 invalid work |
| Figure paint 失败 | 放弃不完整 submission，下轮 Full |
| backend submit 失败 | 标记 surface state unknown，下轮 Full |
| surface lost | 暂停提交，恢复后 Full |
| invalid FigureId | 丢弃该 source 并记录其来源；不得访问复用节点 |
| callback panic | 恢复 phase guard；是否继续由 Runtime panic policy 决定 |

phase guard 必须使用作用域恢复机制，确保错误后 `is_updating` 不会永久为 true。

## 14. 性能扩展点

不改变语义即可替换：

- InvalidSet 去重结构；
- validation root 归约；
- dirty region coalescing；
- spatial index；
- transform/projected-bounds cache；
- retained command cache；
- tile cache；
- parallel command preparation；
- backend partial present。

并行化只能处理已冻结的只读 scene snapshot，结果通过确定性顺序合并。

## 15. 验证门禁

至少验证：

1. Validation 严格先于 Damage Repair；
2. 重复 invalid/dirty 合并；
3. layout 中产生的新 invalid 能收敛；
4. non-converging validation 可诊断且不挂起；
5. bounds 变更覆盖 old/new projected area；
6. nested transform、viewport 和 clip damage；
7. disabled 节点仍参与 validation；
8. invisible 节点恢复时完成过期 validation；
9. surface lost/resize 后 Full；
10. backend full-render 与 partial-render 结果等价；
11. panic/error 后 phase 状态恢复；
12. 通知保持因果顺序。
