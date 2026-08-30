# Zed 响应式设计分析

类型：`reference-analysis`

本文档整理 Zed 在 Rust 中实现响应式机制的关键设计，重点关注：

1. 状态变化如何传播
2. 监听者模式与订阅如何建模
3. 事件与状态失效为何分离
4. 这套机制对 Novadraw 实现 draw2d 等价通知体系有何启发

> 参考源码路径：`/Users/bytedance/Documents/code/GitHub/zed`

---

## 1. 先说结论

Zed 的响应式机制并不是一个单一的“观察者模式实现”，而是四个协议的组合：

1. `Entity<T>`：状态与身份的统一载体
2. `notify`：无 payload 的“状态已变化”通知
3. `emit`：typed event 的语义事件分发
4. `effect flush`：将通知、事件、视图刷新统一延迟到更新结束后执行

从第一性原理看，Zed 解决的不是“如何让回调跑起来”，而是：

- 如何避免在更新过程中立即重入
- 如何区分“状态失效”和“领域事件”
- 如何让订阅者回到自己的受控上下文中执行
- 如何把状态变化与视图失效精确关联

这与 draw2d 的通知问题高度相关，因为 draw2d 也不是单一 listener，而是多类通知并存：

- `figureMoved`
- `coordinateSystemChanged`
- `propertyChange`
- `UpdateListener`

所以如果未来 Novadraw 要实现 draw2d 等价通知机制，最值得借鉴的不是某个具体 API，而是这种“多协议分层”的思想。

---

## 2. Zed 的响应式最小模型

### 2.1 `Entity<T>` 是响应式根节点

在 Zed 中，真正参与响应式系统的不是裸数据，而是 `Entity<T>`。

可以把它理解为：

- 它是状态对象的 owner
- 它是被观察和被订阅的目标
- 它是视图依赖跟踪的最小单位
- 它也是生命周期管理的边界

这意味着 Zed 的响应式图不是“变量依赖图”，而是“实体依赖图”。

### 2.2 `Context<T>` 是唯一的受控入口

实体不会随意向外暴露可变引用。对外部来说，状态修改、事件发射、通知观察者，基本都要经过 `Context<T>`。

这相当于把“状态修改”和“副作用发射”放到了同一个事务边界里。

换句话说，Zed 的设计不是：

```text
拿到状态 -> 随便改 -> 顺手发通知
```

而是：

```text
进入上下文 -> 修改状态 -> 记录 effect -> 更新结束后统一 flush
```

---

## 3. `notify` 与 `emit` 为什么必须分离

### 3.1 `notify` 的语义

`notify` 表达的是：

> “这个实体的状态已经发生变化，依赖它的观察者和视图需要重新处理。”

它不关心具体 payload，也不试图表达业务语义。

这类通知更接近：

- invalidation
- changed
- dirty
- moved but payload not required

### 3.2 `emit` 的语义

`emit` 表达的是：

> “这个实体发出了一个带明确类型的语义事件。”

比如：

- pane activated
- worktree updated
- project reloaded

这类事件面向“业务协作”，不是面向“视图失效”。

### 3.3 为什么不能混成一种机制

如果把 `notify` 和 `emit` 混成一个系统，就会出现几个问题：

1. 视图刷新会被迫理解领域事件类型
2. 业务层会滥用事件来表达普通状态变化
3. 同一个变化既想触发刷新又想表达语义时，边界会变乱

Zed 的关键决策是：

- `notify` 负责“状态变化”
- `emit` 负责“语义事件”

这对 Novadraw 尤其重要，因为 draw2d 的通知本来就是分层的：

- 几何变化不等于属性变化
- `coordinateSystemChanged` 不等于 `figureMoved`
- repaint/update 也不应直接复用业务事件通道

---

## 4. 监听者模式在 Zed 中是怎么落地的

### 4.1 `observe`

`observe(entity, callback)` 监听的是某实体是否调用了 `notify()`。

它的特点：

- 不带事件 payload
- 本质是“状态变化观察者”
- 更适合刷新、同步、派生状态更新

### 4.2 `subscribe`

`subscribe(entity, callback)` 监听的是某实体发出的 typed event。

它的特点：

- 有明确事件类型
- 需要实体实现 `EventEmitter<E>`
- 更适合 pane/workspace/project 这种业务层协作

### 4.3 最关键的一点：回调会回到订阅者自己的上下文

Zed 并不是简单把事件丢给一个裸回调，而是让订阅者重新进入自己的 `Context<T>` 再处理。

这意味着：

- 回调执行时仍处于受控上下文中
- 订阅者更新自己的状态时仍遵守同一套事务规则
- 响应式传播不会退化成“全局随意修改共享对象”

这个设计非常值得借鉴。因为如果 Novadraw 以后实现 listener 系统，最危险的不是“没有 listener”，而是 listener 回调拿到太多裸状态，最终破坏 SceneHost / UpdateManager 的边界。

---

## 5. `Subscription` 的真正价值

Zed 的 `Subscription` 不是一个装饰品，而是生命周期协议。

它承担三件事：

1. 订阅关系可显式持有
2. `drop` 自动解绑
3. 订阅目标或订阅者释放时可自动清理

这让系统避免了很多经典观察者模式问题：

- 忘记解绑
- 悬挂回调
- 已释放对象仍被通知

Zed 还通过 `WeakEntity` 让订阅回调每次执行前先尝试 upgrade：

- 能 upgrade，说明订阅者还活着，继续执行
- 不能 upgrade，说明订阅者已释放，自动失效

这套模式对 Rust 尤其自然，也很适合 Novadraw：

- `FigureBlock` 或未来的 runtime object 都可以被弱引用观察
- listener 生命周期可以跟随宿主对象，而不是依赖手工管理

---

## 6. effect flush 是 Zed 最关键的稳定性设计

### 6.1 问题背景

如果状态一变化就立即广播，会产生经典重入问题：

- 观察者在回调里再次修改状态
- 新订阅在当前分发轮次就生效
- 一个实体还没更新稳定，其他实体就开始读取它

这类问题在图形引擎里尤其危险，因为它会直接污染：

- 更新事务顺序
- dirty 语义
- 坐标闭包
- 渲染与命中测试一致性

### 6.2 Zed 的解决方法

Zed 把通知、事件、刷新等动作都先放入 `pending_effects` 队列，等当前 update 结束后统一 `flush_effects()`。

这相当于把响应式系统变成：

```text
状态更新阶段
  -> 只记录 effect

事务结束
  -> flush notify / emit / refresh / defer
```

### 6.3 这对 Novadraw 的价值

如果未来 Novadraw 要做 draw2d 等价通知机制，这一点几乎是必须的。

原因很直接：

- `figureMoved`
- `coordinateSystemChanged`
- `repaint`
- `addInvalidFigure`
- `repairDamage`

这些动作并不适合在任意时刻同步乱触发，而应该受 UpdateManager 事务边界约束。

所以对 Novadraw 来说，最值得借鉴的不是“做一个订阅表”，而是：

> 所有通知都必须有明确的事务落点，不能直接穿透更新边界。

---

## 7. Zed 如何把状态变化连接到视图失效

### 7.1 访问跟踪

Zed 在 render/prepaint 期间会记录“这次视图访问过哪些实体”。

于是它建立了一张关系：

```text
view/window  ->  accessed_entities
```

### 7.2 精准失效

当某个实体 `notify(entity_id)` 时，系统并不是全局刷新，而是只让真正依赖该实体的视图失效。

这意味着 `notify` 不只是“调观察者”，它还是视图缓存失效协议的一部分。

### 7.3 对 Novadraw 的启发

Novadraw 现在最接近的对象不是传统 DOM diff，而是：

- damage repair
- RenderSubmission
- SceneHost redraw scheduling

所以如果未来要借鉴这一点，不一定直接照搬“访问跟踪渲染树”，但至少可以抽出一个原则：

> 通知系统不只是 listener 系统，它还应服务于精准失效和更新调度。

也就是说，未来的通知体系不应该只是：

- 注册监听器
- fire 事件

还应该考虑：

- 哪些变化只需要逻辑监听
- 哪些变化需要触发 dirty
- 哪些变化需要触发 validation
- 哪些变化需要触发坐标域变化传播

---

## 8. Zed 与 draw2d 的可对照关系

### 8.1 相同点

两者都不是单一通知通道。

它们都在区分：

- 状态变化
- 语义事件
- 生命周期事件
- 渲染/更新相关副作用

### 8.2 不同点

draw2d 的通知是传统 OO 风格，主要体现为：

- 多类 listener 接口
- 多个 `fireXxx()` 方法
- `UpdateManager` 作为更新事务载体

Zed 的通知则更偏现代 Rust/响应式风格：

- `Entity<T>` 作为统一节点
- `Subscription` 管理生命周期
- `notify` / `emit` 分流
- effect queue 统一结算

### 8.3 对 Novadraw 的建议

Novadraw 不应机械复制 draw2d 的 Java 监听器接口，也不应机械复制 Zed 的 API 形式。

更好的路径是：

1. 保留 draw2d 的语义分层
2. 借鉴 Zed 的事务和生命周期设计
3. 用 Rust 方式重写协议边界

---

## 9. 面向 Novadraw 的最小借鉴方案

如果未来要在本项目实现 draw2d 等价通知机制，建议最少拆成三层，而不是先做一个大而全的 listener 框架。

### 9.1 第 1 层：状态失效通知

建议引入类似 `notify(block_id)` 的无 payload 机制，用于：

- 观察某个 figure/runtime object 状态变化
- 触发局部刷新或派生状态更新
- 承接视图失效或编辑器 UI 同步

它不承担领域语义，只承担“这个对象已经变化”。

### 9.2 第 2 层：typed 语义事件

建议引入类似 `emit(Event)` 的 typed event 机制，用于：

- `FigureMoved`
- `CoordinateSystemChanged`
- `ChildAdded`
- `ChildRemoved`
- `LayoutInvalidated`

这里需要注意：不是所有 draw2d 的 fire 方法都应该被压成一个枚举，但 typed event 这一层必须存在。

### 9.3 第 3 层：更新事务 effect 队列

建议不要在 figure 操作里直接同步广播，而是进入 UpdateManager / SceneHost 控制的 effect 队列。

这层负责保证：

- Validation 先于 Repair
- 通知不会重入破坏坐标闭包
- dirty / invalid / event 的落点稳定

---

## 10. draw2d 等价通知机制应如何映射

下面是一个更接近本项目未来设计的语义拆分建议。

| draw2d 语义 | Novadraw 建议机制 | 是否需要 payload |
|---|---|---|
| `figureMoved` | typed event | 需要 |
| `coordinateSystemChanged` | typed event | 需要 |
| `propertyChange` | typed event 或属性通道 | 需要 |
| `repaint/addDirtyRegion` | effect queue 中的 render invalidation | 通常不直接暴露给业务 listener |
| `addInvalidFigure/revalidate` | effect queue 中的 validation invalidation | 通常不直接暴露给业务 listener |
| “对象状态变了” | notify | 不需要 |

这个拆分和 Zed 很接近：

- `notify` 对应“状态变化但不关心业务语义”
- `emit` 对应“有类型的语义事件”
- effect queue 对应“副作用统一结算”

---

## 11. 不应直接照搬的部分

Zed 的设计很强，但也不是所有部分都适合直接搬进 Novadraw。

### 11.1 不应先复制完整的视图依赖跟踪

Zed 的“访问过哪些实体 -> 精准失效哪些视图”非常强，但它依赖自己的视图缓存架构。

Novadraw 当前更紧迫的是：

- 坐标闭包稳定
- damage repair 稳定
- UpdateManager 事务稳定

所以这部分应列为后续增强，而不是通知体系的第一阶段。

### 11.2 不应让 listener 直接修改任意核心状态

一旦 listener 回调绕过事务边界，`dirty`、`invalid`、`bounds`、`capture/focus` 等状态就会重新失去一致性。

所以如果引入类似 Zed 的订阅机制，回调也应回到受控上下文中执行，而不是散落成“到处可写的回调”。

---

## 12. 最终判断

Zed 提供给本项目的最大启发，不是“观察者模式怎么写”，而是：

1. 响应式节点必须有明确 owner 和生命周期
2. 状态变化通知与语义事件必须分离
3. 副作用必须延迟到事务边界统一结算
4. 通知系统最终要服务于更新与失效，而不只是 listener API

如果未来 Novadraw 要做 eclipse draw2d 等价通知机制，建议路线不是：

```text
先做一套 listener 注册接口
```

而是：

```text
先定义通知语义分层
-> 再定义 effect 落点
-> 再实现生命周期安全的订阅机制
-> 最后再暴露 listener API
```

这条路线更符合：

- draw2d 的语义分层
- Rust 的所有权模型
- 当前 Novadraw 先稳定坐标和更新事务、再进入通知体系的主线策略
