# Draw2D 通知机制设计分析

类型：`parity-contract`

本文档分析 Eclipse Draw2D 源码中的通知机制设计，重点回答以下问题：

1. Draw2D 中有哪些主要通知类型
2. 这些通知各自解决什么问题
3. 它们是如何在源码中实现的
4. 它们之间如何与 Figure、UpdateManager、EventDispatcher 协同工作
5. 这些设计对 Novadraw 后续实现 draw2d 等价通知机制有何启发

> 参考源码路径：`/Users/bytedance/Documents/code/GitHub/gef-classic`

---

## 1. 先说结论

Draw2D 的通知机制并不是一个统一的大总线，而是多条并行协议：

1. Figure 级变化通知
2. 坐标系统变化通知
3. 属性变化通知
4. 祖先链变化通知
5. 输入事件监听与回调
6. UpdateManager 更新周期通知

从第一性原理看，Draw2D 解决的不是“如何注册回调”这么简单，而是：

- 如何区分不同语义层的变化
- 如何避免把几何、属性、输入、更新事务混成一种事件
- 如何让更新周期中的 repaint / validate 也成为可监听协议的一部分
- 如何让坐标系统变化被显式建模，而不是误当成普通 moved

因此，Draw2D 的通知体系本质上是一组**语义分层的协议**，而不是单一的 observer pattern。

---

## 2. 通知体系的总览

在 `IFigure` 接口中，Draw2D 直接暴露了多类 listener 注册方法，包括：

- `AncestorListener`
- `CoordinateListener`
- `FigureListener`
- `FocusListener`
- `KeyListener`
- `LayoutListener`
- `MouseListener`
- `MouseMotionListener`
- `MouseWheelListener`
- `PropertyChangeListener`

这说明 Draw2D 的设计从一开始就明确承认：

> Figure 运行时会产生多种完全不同的“变化”，这些变化不能压扁成同一种通知。

与现代响应式框架相比，Draw2D 的形式偏传统 OO；其源码明确区分了这些通知协议。

---

## 3. Figure 自身的通知面

### 3.1 `FigureListener`：几何变化通知

`FigureListener` 的核心语义是：

> Figure 的 bounds 变化了。

在 `Figure` 中，`fireFigureMoved()` 会通知所有 `FigureListener`：

- 位置变化会触发
- 尺寸变化也会触发
- 只要 bounds 变化，就属于 figure moved 的语义范围

这说明 Draw2D 的 `figureMoved` 不是“单纯平移”，而是“几何边界变化”。

### 3.2 `CoordinateListener`：坐标系统变化通知

`CoordinateListener` 的核心语义是：

> 此 Figure 的局部坐标系统发生变化，并且影响其子树的绝对边界。

这不是普通几何变化，而是更高层的“坐标域映射变化”。

典型触发点是 `Figure.primTranslate()`：

- 先改当前 figure 的 `bounds.x/y`
- 如果 `useLocalCoordinates()` 为 `true`
- 则不向子节点传播 translate
- 而是调用 `fireCoordinateSystemChanged()`

这非常关键，因为它说明在 draw2d 中：

> 坐标根移动，不等价于把整棵子树的 bounds 一起平移；它更接近“局部坐标域映射发生变化”。

所以 `coordinateSystemChanged` 绝不是 `figureMoved` 的重复版本，而是另一条独立语义。

### 3.3 `fireMoved()`：历史兼容层

Draw2D 中还有一个已废弃的 `fireMoved()`，它会同时触发：

- `fireFigureMoved()`
- `fireCoordinateSystemChanged()`

从当前源码的 deprecated 注释可以确定，`fireMoved()` 是为兼容旧调用而同时触发两类
通知；至于历史演进动机，源码本身没有给出更多证据。

这个兼容层本身就是一个重要设计信号：

> Draw2D 逐步认识到“几何变化”和“坐标系统变化”不是同一个协议。

---

## 4. `PropertyChangeListener`：属性变化协议

Draw2D 同时保留了 Java Bean 风格的 `PropertyChangeListener`。

它与前面的区别在于：

- `FigureListener` / `CoordinateListener` 更偏图形系统语义
- `PropertyChangeListener` 更偏通用对象属性变更语义

在 `Figure` 中，属性变化通过 `PropertyChangeSupport` 实现，并提供多个重载的 `firePropertyChange(...)`：

- `boolean`
- `int`
- `Object`

这让 Draw2D 可以对外表达一些不属于纯几何通知的变化，比如：

- `parent` 改变
- 视口位置
- 滚动条 range model 的属性变化

这套机制的意义在于：

> Draw2D 并没有试图用 Figure 专用 listener 覆盖一切，而是保留了更通用的属性变化通道。

---

## 5. `AncestorListener`：祖先链变化的桥接通知

`AncestorListener` 并不是 Figure 直接 fire 的一个原生事件，而是由 `AncestorHelper` 维护的一层桥接逻辑。

`AncestorHelper` 会做两件事：

1. 沿父链给所有祖先挂上 `FigureListener`
2. 同时监听祖先的 `"parent"` 属性变化

这样就能把以下变化转成祖先通知：

- 某个祖先 moved -> `ancestorMoved`
- 某个祖先从父链上脱离 -> `ancestorRemoved`
- 某个祖先重新挂接 -> `ancestorAdded`

这类通知非常重要，因为很多 Figure 的有效位置并不只取决于自己，而是取决于整条祖先链。

所以 `AncestorListener` 的根本用途是：

> 把“父链结构与祖先几何变化”转成一个可观察协议。

这和 `CoordinateListener` 有一定重叠，但关注点不同：

- `CoordinateListener` 更强调坐标系统语义
- `AncestorListener` 更强调结构与祖先变化对当前 figure 的影响

---

## 6. 输入事件监听不是普通观察者，而是分发协议

### 6.1 事件不是通过 fire/listener 对外广播的

Draw2D 的鼠标、键盘、焦点事件虽然也有 `MouseListener`、`KeyListener`、`FocusListener` 等接口，但它们的来源不是 Figure 自己主动 fire。

它们的来源链是：

```text
SWT 事件
-> LightweightSystem
-> EventDispatcher / SWTEventDispatcher
-> 命中测试
-> figure.handleXxx(event)
-> figure 内部 listener 被调用
```

也就是说，输入事件不是“状态通知”，而是“平台事件分发”。

### 6.2 `EventDispatcher` 的职责

抽象 `EventDispatcher` 定义输入分发契约；默认实现 `SWTEventDispatcher` 保存并维护：

- 焦点 owner
- mouse target
- capture
- 键盘事件路由
- 鼠标事件路由
- hover / enter / exit / drag 的状态维护

所以这里的 listener 不是一个普通 observer 容器，而是事件分发系统的末端 hook。

### 6.3 `SWTEventDispatcher` 的关键逻辑

`SWTEventDispatcher` 通过 `receive(me)` 完成：

1. 更新鼠标下 figure
2. 若已 capture，则沿用当前 `mouseTarget`
3. 否则用 `root.findMouseEventTargetAt(x, y)` 重新找目标
4. 处理 enter/exit 切换
5. 构造 Draw2D `MouseEvent`
6. 调用 `handleMousePressed/Released/Moved/...`

因此从设计上看：

> Draw2D 输入 listener 的本质不是“对象状态变化监听”，而是“事件分发系统中的回调端口”。

这和 `FigureListener` / `CoordinateListener` / `PropertyChangeListener` 不是同一层概念。

---

## 7. `LayoutListener`：布局机制的插桩点

`LayoutListener` 的实现方式也很特别。

它不是通过独立的 listener list 存在，而是由 `Figure.LayoutNotifier` 适配：

- 如果 figure 已有 `layoutManager`
- 则用 `LayoutNotifier` 包一层
- 在真正调用布局前后插入 listener 回调

这意味着 Draw2D 对布局通知的理解是：

> 布局通知不是 figure 通用事件，而是布局协议内部的插桩点。

因此 `LayoutListener` 更像“layout lifecycle hook”，而不是传统领域事件。

---

## 8. `UpdateListener`：更新事务通知

### 8.1 它属于 `UpdateManager`，不属于 `Figure`

这是 Draw2D 通知设计中最容易被忽略，但最关键的一点。

`UpdateListener` 并不是 Figure 上的 listener，而是挂在 `UpdateManager` 上。

它提供两个钩子：

- `notifyValidating()`
- `notifyPainting(Rectangle damage, Map<IFigure, Rectangle> dirtyRegions)`

这意味着它监听的不是某个对象的属性，而是整个更新事务的阶段变化。

### 8.2 `UpdateManager` 的作用

`UpdateManager` 负责：

- 收集 dirty region
- 收集 invalid figure
- 驱动 validation
- 驱动 damage repair / repaint
- 在关键时刻通知 `UpdateListener`

换句话说，`UpdateListener` 不是 UI 交互 hook，而是更新系统 hook。

### 8.3 `DeferredUpdateManager` 的两阶段事务

默认实现 `DeferredUpdateManager` 非常清楚地体现了这一点：

1. `addDirtyRegion()` 收集脏区
2. `addInvalidFigure()` 收集 invalid figure
3. `queueWork()` 异步排队
4. `performUpdate()` 执行：
   - `performValidation()`
   - `repairDamage()`

在这个过程中：

- `performValidation()` 前会 `fireValidating()`
- `repairDamage()` 计算出最终 damage 后会 `firePainting(...)`

因此可以说：

> Draw2D 把“更新周期本身”也设计成可观察协议。

这对架构非常重要，因为它让像 `Thumbnail` 这样的功能，不必侵入核心更新流程，就能感知 repaint/validation 的发生。

---

## 9. 通知体系与 damage / 坐标协议的关系

Draw2D 的通知机制不是孤立的，它与几何、坐标、update 紧密耦合。

一个最典型的例子就是：

- 基础 `Figure.primTranslate()` 在 `useLocalCoordinates() == true` 时触发
  `fireCoordinateSystemChanged()`，并停止修改后代 bounds
- `repairDamage()` 则沿父链做 `translateToParent()` 和裁剪

这说明：

1. 通知并不是附加功能
2. 它是几何语义的一部分
3. 它承担着把“坐标域变化”显式暴露出来的职责

如果没有这层通知，外部系统只能看到某个 figure moved，却无法知道：

- 子树绝对位置是否整体发生了变化
- 命中测试语义是否需要重新评估
- viewport / scroll / coordinate root 是否发生了映射变化

因此，`coordinateSystemChanged` 是 draw2d 的一个关键设计点，而不是边缘细节。

---

## 10. Draw2D 通知机制的主要作用

可以把 Draw2D 的通知机制总结成六个主要作用。

### 10.1 把不同语义层的变化显式分层

Draw2D 明确区分：

- 几何变化
- 坐标系统变化
- 属性变化
- 祖先链变化
- 输入事件
- 更新事务变化

这是它最大的优点。

### 10.2 为外部扩展提供 hook

很多扩展能力并不在 Figure 主链里实现，而是通过 listener 附着，例如：

- 缩略图
- 辅助视图
- 结构联动
- 祖先链感知

### 10.3 让更新系统可被观察

`UpdateListener` 说明 Draw2D 不只是让对象变化可观察，也让“更新阶段”本身可观察。

这对于性能工具、缓存、镜像视图都很有价值。

### 10.4 显式表达坐标域变化

`CoordinateListener` 让 Draw2D 能把坐标系统变化当成一等公民。

### 10.5 将平台事件路由到 Figure 树

输入 listener 的存在使 Figure 不直接依赖 SWT Control，而是通过 EventDispatcher 间接接收事件。

### 10.6 维持向后兼容

像 `fireMoved()` 这样的兼容 API，反映出 Draw2D 在演进过程中保持老监听器可工作的努力。

---

## 11. Draw2D 这套设计的优点与代价

### 11.1 优点

- 语义分层清晰
- 扩展点多
- 与 Java OO 模型天然贴合
- 更新系统和对象系统都可被观察
- 坐标变化被显式建模

### 11.2 代价

- listener 类型很多，学习成本高
- 某些通知边界存在重叠，例如 moved 与 coordinate changed
- 历史兼容 API 增加理解负担
- 输入 listener 与对象变化 listener 容易被误认为同一类机制

这些是该设计的可观察权衡；“先进”或“混乱”都不是源码事实。

---

## 12. 对 Novadraw 的直接启发

### 12.1 不要做单一 listener 系统

如果未来 Novadraw 要实现 draw2d 等价通知机制，不应先做一个统一的：

```text
add_listener(event_kind, callback)
```

然后试图把所有语义塞进去。

更合理的做法是像 draw2d 一样分层。

### 12.2 至少要区分三层

建议最少区分：

1. 几何/坐标变化通知
2. 输入分发回调
3. 更新事务通知

否则后面会很容易出现：

- 几何变化 listener 误承担 update 语义
- 输入事件系统误承担对象状态通知
- repaint / invalid / damage 失去稳定边界

### 12.3 `coordinateSystemChanged` 必须被保留

这是最不能丢的一层。

因为当前 Novadraw 正在收口的主线，正是：

- 坐标域分段
- `prim_translate`
- damage repair
- `translate*` 协议

如果通知系统里没有与 `coordinateSystemChanged` 等价的概念，坐标闭包就很难被完整表达。

### 12.4 更新事务通知应挂在 UpdateManager / SceneHost 层

这一点不应挂在 Figure 上。

因为 draw2d 已经证明：

> repaint / validation / repair 是事务级事件，不是单对象属性变化。

所以未来 Novadraw 如果做更新通知，更合理的位置应在：

- `UpdateManager`
- 或 `SceneHost + UpdateManager` 边界

而不是直接做成 Figure listener。

---

## 13. 与 Zed 的关系

Zed 与 Draw2D 的机制形式不同，但它们在核心原则上是接近的：

- 都不是单一通知总线
- 都区分状态变化与其他语义
- 都重视更新边界
- 都把生命周期/事务看得比“回调写法”更重要

可以这样理解：

- Draw2D 提供了**语义分层**
- Zed 提供了**现代 Rust 事务与生命周期管理方式**

因此 Novadraw 更合适的路线不是“照搬 Draw2D 接口”或“照搬 Zed API”，而是：

1. 保留 Draw2D 的通知语义分层
2. 借鉴 Zed 的订阅生命周期和 effect 队列
3. 用 Rust 方式重写通知协议

---

## 14. Novadraw 当前落地

当前引擎已按语义拆分以下监听端口：

- `FigureListener`
- `CoordinateListener`
- `AncestorListener`
- `PropertyChangeListener`
- `LayoutListener`
- Figure input callbacks
- UpdateManager 级 `UpdateListener` / `ValidatingListener`

监听器注册返回 `ListenerId`，由 `SceneUpdateManager::remove_listener()` 统一解除，
不依赖全局状态。所有变化先写入 `NotificationQueue`，再在更新事务末尾按发生顺序
flush；validation 中产生的 Figure/Layout effect 位于 `Validating` 和 `Validated`
之间。

兼容入口 `UpdateListener::{on_figure_event,on_notify}` 暂时保留，但新增能力应优先
使用对应 typed listener，避免重新形成单一总线。

---

## 15. 最终判断

Draw2D 的通知机制可以概括为一句话：

> 它不是一个 listener 框架，而是一组围绕 Figure 树、坐标域、输入分发和更新事务的语义协议。

对本项目最重要的结论有四条：

1. `figureMoved` 和 `coordinateSystemChanged` 必须分开理解
2. 输入 listener 与对象状态通知不是同一个系统
3. `UpdateListener` 说明更新事务本身必须可观察
4. 通知机制应服务于几何、坐标、damage 和调度，而不只是“给外部注册回调”

如果未来 Novadraw 要实现 eclipse draw2d 等价通知机制，正确路线应是：

```text
先定义通知语义分层
-> 再定义事务边界与落点
-> 再定义 Rust 风格的订阅/生命周期模型
-> 最后实现具体 listener / event API
```

而不是反过来先设计一堆接口。

---

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
