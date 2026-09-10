# ADR-002: 采用 Draw2D 语义分层与 Zed 式 effect queue 的通知机制

类型：`architecture-decision`

## 状态

已通过

后续细化：ADR-003 已确定 Runtime 是 effect 与 mutation 的提交边界。本文中将
`FigureGraph`、`SceneHost` 或未来 Context 列为候选 flush owner 的内容仅保留为历史
推导，不再代表当前选择。

## 背景

Novadraw 需要实现与 Eclipse Draw2D 等价的通知能力，用于支撑：

- Figure 几何变化通知
- 坐标系统变化通知
- 属性变化通知
- 祖先链变化通知
- UpdateManager 更新周期通知
- 后续 viewport、scroll、thumbnail、编辑器交互、布局联动等扩展能力

参考系统有两类：

- Draw2D：提供经过验证的图形领域通知语义，例如 `figureMoved`、`coordinateSystemChanged`、`propertyChange`、`UpdateListener`。
- Zed：提供现代 Rust/响应式系统中的 effect queue、typed event、订阅生命周期与延迟 flush 思路。

二者并不处在同一抽象层：

- Draw2D 更擅长定义“通知是什么”。
- Zed 更擅长定义“通知什么时候安全执行”。

因此，本项目不能简单二选一：

- 如果照搬 Draw2D 的同步 listener，listener 可能在 `prim_translate()`、`set_bounds()`、`revalidate()` 等核心状态修改过程中重入，读取半稳定的 bounds、dirty、invalid、damage 状态。
- 如果照搬 Zed 的完整 runtime，可能过早引入 `Entity/Context/Subscription` 等大规模机制，反而偏离当前正在收口的坐标闭包、damage repair 与 UpdateManager 两阶段主线。

## 决策

Novadraw 通知机制采用混合路线：

```text
Draw2D 决定通知“是什么”
Zed 决定通知“什么时候安全执行”
Novadraw 决定通知“在哪个事务边界 flush”
```

具体决策如下：

1. 保留 Draw2D 的通知语义分层
   - `FigureMoved`
   - `CoordinateSystemChanged`
   - `PropertyChanged`
   - `UpdateEvent`
   - 后续按需扩展 `AncestorEvent`、`LayoutEvent`、输入事件分发 hook

2. 不采用 Draw2D 的同步 fire 作为默认执行模型
   - 核心状态修改期间不直接执行外部 listener。
   - `prim_translate()`、`set_bounds()`、`revalidate()`、`repaint()` 等操作只记录 effect。
   - listener/subscriber 的执行必须发生在明确事务边界。

3. 借鉴 Zed 的 effect queue
   - 将通知拆成无 payload 的 `notify` 与 typed event 的 `emit`。
   - 将变化先记录到 effect queue。
   - 在事务稳定后统一 drain/flush。

4. 以 Novadraw 的更新事务定义 flush 边界
   - 默认边界应位于 `Validation -> Repair` 之后。
   - `UpdateManager` 保证布局、dirty、damage、RenderSubmission 已稳定。
   - `SceneHost` 或未来更高层 context 可负责最终 flush 与外部订阅执行。

5. 第一阶段只实现核心概念，不实现完整 listener 框架
   - 先实现 `NotificationEffect`、`FigureEvent`、`UpdateEvent`、`NotificationQueue`。
   - 先验证 `FigureMoved`、`CoordinateSystemChanged`、`Validating/Painting` 等关键 effect 是否正确记录。
   - 后续再实现 subscription 生命周期、弱引用、属性通道、祖先监听和布局监听。

## 设计公理

### 公理 1：通知语义必须分层

不能使用单一事件总线吞掉所有变化。

`figureMoved`、`coordinateSystemChanged`、`propertyChange`、`UpdateListener` 的语义不同，必须在类型层面区分。

### 公理 2：坐标系统变化是一等事件

`CoordinateSystemChanged` 不是 `FigureMoved` 的别名。

当 `use_local_coordinates == true` 的坐标根移动时，子树 bounds 不应被逐个平移，但子树的绝对映射已经变化。这个变化必须通过独立事件表达。

### 公理 3：核心状态修改期间不执行外部 listener

核心状态修改过程中，Figure 树、dirty 集合、invalid 队列、damage 集合可能处于中间状态。

外部 listener 不应在这个阶段同步执行，避免：

- 重入修改场景
- 读取半稳定状态
- 破坏坐标闭包
- 干扰 UpdateManager 两阶段顺序

### 公理 4：effect flush 必须有明确事务边界

通知不是不能执行，而是必须在稳定边界执行。

默认顺序为：

```text
Mutation Phase
-> 记录 bounds/property/coordinate/update effects

Validation Phase
-> 稳定布局和几何

Repair Phase
-> 稳定 damage 和 render submission

Notification Flush Phase
-> flush notify/emit/update listener
```

### 公理 5：listener API 晚于 effect 模型

项目应先稳定 effect 的记录、分类、事务边界，再暴露 listener/subscription API。

否则容易先得到一套看似完整的回调接口，但无法保证它们在正确时机执行。

## 当前最小实现方向

第一阶段的核心实现应保持很小：

- `NotificationEffect::Notify { block_id }`
- `NotificationEffect::EmitFigure(FigureEvent)`
- `NotificationEffect::EmitUpdate(UpdateEvent)`
- `NotificationQueue`
- `FigureGraph::drain_notification_effects()`
- `SceneUpdateManager::drain_notification_effects()`

这只是通知系统的“事件事实记录层”，不是最终订阅系统。

后续阶段再考虑：

- `Subscription` 生命周期
- 订阅者弱引用
- typed subscription
- 属性变化通道
- 祖先链监听
- 布局监听
- `SceneHost` 统一 flush

## 后果

### 正面

- 保留 Draw2D 的领域语义，不丢失 `coordinateSystemChanged` 等关键概念。
- 避免 Java 式同步 listener 在 Rust 核心状态修改过程中重入。
- 与当前 UpdateManager 两阶段更新、damage repair、坐标闭包主线兼容。
- 可逐步演进到 Zed 式 subscription，而不必一次引入完整 runtime。
- 为后续 `AD-006` 通知体系提供稳定决策基线。

### 负面

- 与 Draw2D 源码的执行时机不完全一致：Draw2D 多数 Figure listener 是同步 fire，本项目默认延迟到事务边界。
- 第一阶段只能记录 effect，不能立即提供完整外部 listener 能力。
- 需要后续明确 flush owner，可能是 `SceneHost`、`UpdateManager` 或未来的 `SceneContext`。
- effect 队列可能要求更多测试来验证事件顺序和事务边界。

## 不采用的方案

### 方案 A：完全照搬 Draw2D 同步 listener

不采用。

原因：

- 容易产生重入。
- listener 可能看到半稳定状态。
- Rust 借用与生命周期管理成本更高。
- 与当前“先稳定坐标闭包和更新事务”的主线冲突。

### 方案 B：完全照搬 Zed 的 runtime

不采用。

原因：

- 当前项目还没有 Zed 式 `Entity/Context` 运行时。
- 完整 subscription 生命周期会过早扩大架构面。
- 容易把图形引擎通知问题误建模成通用 UI 响应式框架问题。

### 方案 C：先做统一 event bus

不采用。

原因：

- 会抹平 Draw2D 的语义分层。
- `FigureMoved`、`CoordinateSystemChanged`、`UpdateEvent` 容易混在一起。
- 后续 damage、layout、viewport、property 通知都难以形成清晰边界。

## 参考

- `doc/parity/draw2d/notification-mapping.md`
- `doc/reference/zed/reactive-design.md`
- `doc/reference/draw2d/architecture/design-axioms.md`
- Draw2D/GEF: `/Users/bytedance/Documents/code/GitHub/gef-classic`
- Zed: `/Users/bytedance/Documents/code/GitHub/zed`

## 日期

2026-05-06
