# FigureInspector

类型：`normative-design`

状态：`in_progress`

## 1. 目标

FigureInspector 为开发、测试和工具宿主提供对稳定 Figure 场景和提交后事件的只读观察。
它用于解释以下问题：

- 某个 Figure 在树中的 containment、Z-order、bounds 和节点状态；
- 一次 geometry/property/layout/update effect 的发生顺序；
- 为什么当前画面与上一个稳定事务不同。

它不负责修改场景、控制 Runtime、记录 GPU 命令流或替代应用日志。

## 2. 所有权与依赖

```text
Runtime
├── FigureTree
├── UpdateManager -> committed NotificationRecord
└── stable_query()
        |
        v
novadraw-inspector
├── FigureTreeSnapshot
└── bounded EventTimeline
        |
        v
Native / Web / headless host
```

`novadraw-inspector` 只依赖 `novadraw-scene`。它不依赖 `novadraw-editor`、RenderBackend
或平台 crate。Editor 关联属于宿主 adapter 的可选增强：

```text
FigureId -> VisualOwner -> EditPartId -> ModelId
```

不得把上述映射复制到 Runtime，也不得从 `FigureId` 推断业务模型身份。

## 3. 稳定快照

`FigureTreeSnapshot` 必须只在 `Runtime::stable_query()` 成功后创建，并携带该 query 的
epoch。每个 `FigureSnapshot` 至少包含：

- `FigureId`、parent、child count、tree depth 和直接 child order；
- Figure 的稳定 diagnostic name；
- local bounds；
- visible、enabled、valid、opaque、focusable 状态；
- 是否仍挂接到当前根。

快照使用 FigureTree 的真实 containment 顺序。它不展示连接依赖、Editor PartTree 或
渲染命令树为 containment child。

## 4. 事件时间线

Inspector 的 observer 只接收已经提交的 `NotificationRecord`，并按
`(source_epoch, sequence)` 保存原始 effect。历史记录是有界 FIFO：

```text
record arrives
-> append
-> if count > capacity: discard oldest
```

观察者不得取得 `&mut Runtime`，不得在 callback 中排入 mutation，也不得将当前
`StableSceneQuery` 误标为历史时刻的完整场景。事件发生时的旧值只来自 effect payload；
Inspector 属性面板显示的是当前稳定快照。

## 5. 可视化方案

主视图采用三栏诊断工作台，而非图自动布局：

```text
outline tree | selected Figure properties | event timeline
```

### 5.1 Outline tree

- 使用可虚拟化的展开/折叠树；
- 行顺序严格等于 FigureTree 的 parent/child 与 child order；
- 行首展示 Figure name、identity 短表示、状态标记和 bounds 摘要；
- 选中一行时展开 ancestor path，并在画布 overlay 高亮对应 bounds；
- 过滤命中时保留 ancestor context，不能只显示孤立节点。

树是主视图，因为 containment、Z-order 与深度是调试语义，而不是待优化的图布局。
面对 10,000 层树，虚拟列表只渲染已展开节点的可见行。

### 5.2 属性面板

显示选中 Figure 的完整 snapshot，并按稳定次序分组：

1. Identity 与 tree relation；
2. Geometry、visibility、enabled、validity；
3. Style、clip、transform 和 capability 的后续扩展字段；
4. 可选 Editor owner adapter 数据。

### 5.3 Event timeline

- 默认按 `source_epoch`、`sequence` 升序；
- 支持 effect 类别、FigureId、epoch 和文本过滤；
- 选中事件时高亮 payload 涉及的 Figure，并显示 old/new 数据；
- 时间线不能声称当前树就是该事件发生时的树。

首版不使用 force-directed graph、DAG layout 或完整历史回放。这些视图可以作为连接依赖
或事件因果的后续辅助视图，但不能取代 containment tree。

## 6. 性能与失败边界

- Snapshot 和 event buffering 不得置于 paint/validation/damage/render 热路径；
- Tree flattening 由宿主按展开状态执行；引擎 snapshot 不预先构造 UI 行；
- event capacity 必须显式且有界；
- Runtime 未稳定或 faulted 时捕获失败必须原样返回 `StableQueryError`；
- inspector 自身内存分配失败或宿主渲染失败不得影响 Runtime 事务。

## 7. 首期验证

1. 稳定场景捕获返回真实 parent/child 顺序和节点诊断属性；
2. 未稳定 Runtime 拒绝快照；
3. observer 按 sequence 保留提交后事件；
4. 缓冲区满时淘汰最旧事件；
5. 捕获与事件观察不改变 Runtime 的稳定 epoch、Figure bounds 或通知顺序。

## 关系

- [UpdateManager](../rendering/update-manager.md)
- [ADR-016](../../adr/adr-016-figure-inspector-observability.md)
- [P2 delta backlog](../../roadmap/p2-delta-backlog.md)
