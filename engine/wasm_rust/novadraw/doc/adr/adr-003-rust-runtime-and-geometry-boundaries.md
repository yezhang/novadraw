# ADR-003: Rust Runtime 所有权与二维几何边界

类型：`architecture-decision`

## 状态

已通过

## 背景

早期理想架构保留了 Draw2D 的行为语义，但仍混入若干 Java 对象模型和当前实现选择：

- FigureGraph 同时拥有树和交互状态；
- Figure trait 聚合几何、验证和大量事件回调；
- bounds 继续使用最近坐标根域并在普通 parent 移动时传播到后代；
- 核心 trait 默认要求 `Send + Sync`，部分服务使用 `Arc`；
- RootFigure 可以读取平台参数；
- 所有核心概念倾向于预先抽象为 trait；
- DisplayList 二进制 ABI 被当作规范架构；
- 以 4x4 矩阵和类型别名作为未来 3D 扩展方案。

这些选择并非 Draw2D 行为正确性的必要条件，也不符合 Novadraw 长期的 Rust 所有权、
跨平台和可替换后端目标。

## 决策

1. `FigureTree` 只拥有节点与拓扑；pointer、focus、hover、capture 和 gesture
   session 归独立 `InteractionState`。
2. `FigureNode` 组合通用 `NodeState`、容器 `LayoutState` 和
   `Box<dyn Figure>`；Figure 只表达具体图形差异化行为。
3. bounds 统一存储在 parent content domain，父子关系使用同一 `Affine2D` edge
   transform；parent 移动不改写后代 bounds。
4. Runtime 核心默认单线程独占，不为 Figure、LayoutManager、EventContext 等基础
   trait 添加 blanket `Send + Sync`，不使用通用 `Arc<Mutex<_>>`。
5. Root Figure 是树内虚拟根和继承属性根，不读取或持有平台对象。
6. 只在真实替换边界使用 trait；FigureTree、InteractionState、默认 dispatcher、
   UpdateManager 和 Runtime 默认采用具体类型。
7. 二进制 DisplayList 降级为 proposal；核心只规范
   `RecordingCanvas -> RenderSubmission -> RenderBackend`。
8. 二维布局使用 `Affine2D`；2.5D 通过可选 projective composition 扩展；真正 3D
   使用独立 Scene3D、camera、depth、3D bounds 和 ray hit-test。

同时采用 effect-based callback：

```text
callback
→ record effects
→ release component borrow
→ apply effects in causal order
→ apply structural mutations at transaction boundary
```

该决策细化 ADR-002 的 flush owner：Runtime 是 effect 和 mutation 的提交边界；
UpdateManager 只负责 Validation、Damage Repair 和 frame preparation 的阶段协议。

## 后果

### 正面

- 树拓扑、交互 session 和更新队列具有清晰所有权；
- 避免自引用、重入可变借用和普遍锁；
- 二维 layout、hit-test 和 damage 更易统一；
- 平台与渲染后端可以独立替换；
- Web 单线程环境无需承担无意义的线程安全约束；
- 2.5D 和真正 3D 都有明确扩展路径；
- DisplayList 可以依据真实性能数据独立演进。

### 负面

- 与当前实现可能存在较大迁移差距；
- parent-local bounds 与 Draw2D 的内部存储方式不再逐字段对应；
- effect queue 要求为回调查询准备稳定 snapshot；
- 2.5D 投影命中和真正 3D 需要独立协议；
- 旧文档和派生 Wiki 中的 FigureGraph/FigureBlock 术语需要逐步标明为实现快照。

## 不采用的方案

### 继续扩大 FigureGraph

不采用。交互状态与树拓扑生命周期不同，未来多 pointer 和无输入场景会持续增加耦合。

### 完整复制 IFigure

不采用。保留行为语义，但用小 trait、节点状态和 Runtime facade 表达。

### 所有核心组件均为 trait object

不采用。没有真实替换需求的动态分发会增加对象安全、借用和 API 演进成本。

### 全面使用 4x4 矩阵

不采用。它不能解决 camera、depth、ray hit-test、3D bounds 和布局语义，只会增加二维
核心成本。

### 现在固定 DisplayList ABI

不采用。缺少第二 consumer、跨进程需求、benchmark 和版本兼容证据。

## 参考

- `doc/design/architecture/overview.md`
- `doc/design/architecture/static-architecture.md`
- `doc/design/architecture/dynamic-architecture.md`
- `doc/design/coordinates/coordinate-system.md`
- `doc/design/rendering/update-manager.md`
- `doc/adr/adr-002-notification-effect-queue.md`

## 日期

2026-08-30
