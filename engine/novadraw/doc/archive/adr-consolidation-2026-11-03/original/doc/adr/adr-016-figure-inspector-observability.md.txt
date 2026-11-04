# ADR-016: FigureInspector 可观测性边界

类型：`architecture-decision`

## 状态

已接受

## 背景

FigureTree、Runtime 更新事务和 Editor Viewer 各自维护不同但相关的身份与状态域。排查
命中、输入消费、布局失效、damage 或 EditPart 投影问题时，开发者需要同时查看：

- 当前稳定 Figure 树的层级、几何与节点状态；
- 已提交 notification effect 的因果顺序；
- Figure 与 Editor visual owner 的关联。

直接暴露 Runtime 内部可变状态或在渲染主循环记录调试日志，会破坏稳定事务边界并污染
渲染热路径。

## 决策

1. 新建独立 `novadraw-inspector` crate，只依赖 `novadraw` Core 的公开只读协议。
   Inspector 不属于 Runtime、FigureTree 或 RenderBackend 的所有权范围。
2. Inspector 通过 `ObservationListener` 订阅提交后的 `NotificationRecord`，保留
   `source_epoch`、`sequence` 和原始 typed effect。它不得订阅或制造未提交 mutation。
3. 树快照只能由 `Runtime::stable_query` 成功后捕获；快照包含 Figure identity、
   parent/child 顺序、深度、Figure diagnostic name、bounds、可见/启用/valid 状态。
4. 事件历史使用有界 FIFO 环形缓冲；容量由创建者显式提供。首版不保存逐 effect 的完整
   FigureTree 历史快照，事件的 old/new 事实只来自 payload。
5. 可视化由 Native/Web 开发工具宿主实现，默认布局为：
   - 可虚拟化 outline tree，保持真实 parent/child 和 Z-order；
   - 选中节点的属性面板；
   - 按 `source_epoch`、`sequence` 排序且可过滤的时间线。

   不采用力导向或自动布局图作为主视图。它们适合关系网络，不适合保持 FigureTree 的
   containment、顺序和深层定位。
6. Editor 对 Figure 到 `VisualOwner`、`EditPartId`、`ModelId` 的补充映射由后续
   adapter 提供；`novadraw-inspector` 不依赖 `novadraw-editor`，避免将通用引擎
   诊断能力绑定到 GEF 层。

## 失败与资源边界

- Runtime faulted 或存在未发布 work 时，快照捕获返回既有 `StableQueryError`；
- Inspector listener 只观察已提交 effect，不能恢复 Runtime 或重放未完成事务；
- 事件容量满时只淘汰最旧记录，不影响 Runtime 事务；
- Inspector 不在 paint、validation、damage repair 或 render submission 热路径打印日志。

## 后果

### 正面

- 诊断数据与 UI 宿主解耦，Native、Web、headless 测试可以共享同一事实模型；
- 时间线保持 Runtime 已定义的稳定 epoch 和 sequence 语义；
- Tree UI 可以按需虚拟化，不受完整图布局成本约束。

### 代价

- 当前时间线不能回放任意历史树状态；
- Editor 关联需要显式 adapter，不能从 FigureId 推断业务模型。

## 关系

- 延续 [ADR-002](adr-002-notification-effect-queue.md) 的 effect queue；
- 受 [ADR-010](adr-010-runtime-listener-lifecycle.md) 的 listener 生命周期约束；
- 使用 [ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md) 的稳定发布边界；
- 具体协议见
  [`../design/architecture/figure-inspector.md`](../design/architecture/figure-inspector.md)。

## 日期

2026-09-22
