# 9. 动画与表现状态层

> **本章解决的问题**：如何让属性、布局、连接和视口产生连续视觉过渡，同时保证模型、
> Figure、布局、命中、通知和撤销仍然只认同一份最终事实？

动画最容易破坏一致性的地方，不是插值公式，而是状态归属。若每一帧都修改 Figure
bounds、连接路径或业务属性，布局、通知、命中、撤销和重放都会把视觉中间值误认为
新的事实。

核心结论是：

> 权威状态立即提交最终结果；动画只在 Runtime 拥有的表现状态层
> （Presentation Plane）中生成可丢弃的表现覆盖值。

## 9.1 为什么逐帧修改源状态是错误模型

考虑把节点从 A 移到 B，并播放 300 毫秒动画。若每一帧都调用一次 `set_bounds`：

- 布局器会把中间位置当作新输入；
- 连接路由可能每帧重新计算；
- 属性观察者收到大量不代表业务操作的通知；
- CommandStack 无法判断一次移动对应多少次修改；
- 取消动画时必须猜测应该恢复哪个值；
- 不同刷新率会产生不同状态序列。

动画真正需要的是“最终事实已经是 B，但当前画面暂时显示在 A 与 B 之间”。这要求把
状态分成不同层次，而不是让一个字段承担两种时间语义。

## 9.2 已提交状态、表现状态与后端状态

这三类状态具有不同所有者和生命周期：

```mermaid
flowchart LR
    Source[已提交状态<br/>模型与场景最终值] --> Record[帧录制输入]
    Source --> Presentation[表现状态层]
    Presentation -.可选表现覆盖快照.-> Record
    Record --> Submission[RenderSubmission<br/>帧输入]
    Submission --> Backend[Backend state<br/>资源与像素]

    Source -.布局 / 路由 / 命中 / 通知.-> Semantics[语义消费者]
    Record -.绘制.-> Visual[视觉消费者]
```

### 已提交状态

这是模型和场景最终值的权威集合：

- Figure topology、bounds、style 和 content；
- layout 与 route 的最终结果；
- Viewport 的逻辑 origin 和 scale；
- model、selection 与 command history；
- accessibility role、state 和 action。

源事务必须先完成验证和派生状态收敛。动画不能延迟业务提交。

### 表现状态层

表现状态层是 Runtime 在稳定场景状态与帧录制之间维护的可丢弃状态边界。它包含：

- 当前 opacity、transform 和内容等表现覆盖值；
- 布局边界或连接路径的采样表现几何；
- 不进入 Figure 树的动画临时绘制项；
- 表现覆盖的所有者和生命周期信息；
- 计算重绘区域所需的保守视觉包围范围。

`PresentationSnapshot` 是当前帧的表现覆盖快照。它只保存活动 Figure 的稀疏覆盖和
动画临时绘制项，不是完整 Figure 树或另一份场景。没有活动表现覆盖时，
`AnimationService::snapshot()` 返回 `None`，帧录制直接读取稳定场景状态。

表现状态不产生 FigureMoved、业务属性通知、route notification 或 undo entry。清除
覆盖后，画面直接显示已经存在的已提交最终状态。

### 图形准备结果

`FigurePresentation` 是图形准备结果，包含受检测量、视觉边界和不可变绘制数据。普通
Figure 准备、动画内容覆盖和动画临时绘制项都可以复用它，因此它不等同于表现覆盖快照，
也不专属于动画。

`PresentationBinding<V>` 是表现准备绑定。它读取类型化当前值，并把同一家族的有效值
准备成图形准备结果；Runtime 仍负责通道采样和帧录制。

### 后端状态

后端只消费 RenderSubmission，并管理 GPU 或软件资源。后端可以优化表现值的降低方式，
但不能定义动画时间、取消语义或最终状态。

## 9.3 正交模型

动画由四个相互独立的问题组成：

| 维度 | 回答 | 不负责 |
|---|---|---|
| Target | 哪个 subject 的哪个表现通道变化 | 时间曲线 |
| Motion | 给定时间如何采样值 | 查找 Figure、提交帧 |
| Trigger | 何时创建或 retarget plan | 插值与组合 |
| Composition | Track 如何并行、顺序、错峰或重复 | 修改源状态 |

```mermaid
flowchart TD
    Fact[稳定的已提交变化] --> Trigger[Trigger / explicit start]
    Trigger --> Plan[AnimationPlan]
    Plan --> Target[Target]
    Plan --> Motion[Motion]
    Plan --> Composition[Composition]
    Target --> Track[Prepared track]
    Motion --> Track
    Composition --> Timeline[Timeline 调度]
    Track --> Timeline
    Timeline --> Sample[表现采样]
```

显式调用直接创建 Plan，不需要构造一个虚假的 ExplicitTrigger。Trigger 属于可选的
AnimationBehavior，用于观察稳定 fact 后创建 Plan。两条入口最终进入同一 admission、
通道所有权和采样管线。

## 9.4 Target 与动画表现通道

Target 由 subject identity 和类型化动画表现通道 identity 组成。常见通道包括：

- opacity、local transform、Color 或 Paint；
- layout result 的 child 表现边界；
- Connection route 表现值；
- Viewport pan、zoom 和 fit transform；
- dash phase 和 route visual progress；
- 动画临时绘制项的完整 `FigurePresentation`；
- 外部 Figure 声明的类型化 `PresentationBinding<V>`。

表现通道不是覆盖所有 Figure 私有状态的中心枚举。Core 提供通用通道；Connection、
Viewport 和外部 Figure 可以通过类型化 descriptor 增加领域通道。

同一 subject/表现通道同时只有一个 owner。

## 9.5 Motion 只负责采样

Motion 把时间映射为值：

- Tween：start/end 与 easing；
- Keyframes：有序 key time/value；
- Spring/Decay：具有终止预算的 simulation；
- Procedural：接受显式时间的纯采样器。

Motion 不能：

- 读取墙钟；
- 查询或修改 Runtime；
- 请求重绘；
- 修改 Figure 或模型；
- 决定表现通道冲突；
- 返回后端专有类型。

同一绝对时间必须产生确定结果。掉帧后直接采样新的单调时间，而不是循环补做每个中间
刷新步。

## 9.6 Composition 与 Timeline

Composition 组织 Track 时间：

```text
Parallel
Sequence
Stagger
Delay
Repeat / Reverse
```

Sequence 和 Stagger 使用稳定 child 顺序。有限动画必须能证明结束时间；Spring 和
Decay 需要 settle tolerance 与最大时长；无限 Procedural 只能位于允许 continuous 的
根组合中，不能阻塞 Sequence 的后续节点。

Pause 是 active timeline 的运行状态，不是与 Delay 重复的 composition node。

## 9.7 Runtime 拥有时间与服务

Runtime 独占 AnimationService：

```text
Runtime
├── FigureTree / UpdateManager
├── Layout / Connection / Input services
└── AnimationService
    ├── mode
    ├── active timelines
    ├── 表现覆盖值
    ├── 动画临时绘制项
    └── next wakeup
```

Host 只通过统一单调时间入口驱动 Runtime。动画不创建第二个 run loop、线程、全局 clock
或 Figure 私有 timer。

`AnimationMut::sample_version()` 只读取基础 Motion 缓存的身份：
尚未 advance 时，`time` 为 `None`（Unanchored）；成功推进后，记录最后接受的单调时间。
同时间重复 advance 不再次调用 Motion，也不增加采样计数。准入、retarget 或终止会改变
`active_revision`，因此相同时间可以对应不同活动集合。这个基础身份不代表完整几何
版本或已录制帧；ADR-046 的完整 E 还需要冻结合法源依赖。

active set 为空时：

- 不扫描 FigureTree；
- 不请求持续 wakeup；
- 不产生 animation damage；
- 不提交空帧。

因此，没有使用动画的应用不承担持续帧成本。

## 9.8 一帧中的因果顺序

```mermaid
sequenceDiagram
    participant Host
    participant Runtime
    participant Scene as 场景状态
    participant Animation as AnimationService
    participant Recorder
    participant Backend

    Host->>Runtime: advance monotonic time
    Runtime->>Scene: 提交 mutation 并稳定化
    Scene-->>Runtime: 稳定场景状态
    Runtime->>Animation: 处理稳定变化记录 / plans
    Animation->>Animation: 采样 active tracks
    Animation-->>Runtime: 稀疏表现覆盖快照或 None
    Runtime->>Runtime: 合并前后表现范围的 damage
    Runtime->>Recorder: 稳定场景状态 + 可选表现覆盖快照
    Recorder-->>Runtime: RenderSubmission
    Runtime-->>Host: FramePreparation::Ready
    Host->>Backend: submit(RenderSubmission)
    Backend-->>Host: RenderResult
    Host->>Runtime: complete_submission(result)
```

已提交状态的通知只依赖源事务已经稳定，不依赖动画是否完成，也不依赖当前帧是否
Presented。后端失败不能回滚源状态；动画失败也只能清除表现覆盖并显示最终值。

## 9.9 Stable Fact 与 AnimationBehavior

Core 默认不为普通 mutation 安装隐式动画。普通属性变化仍直接显示最终静态结果。

应用可以安装 AnimationBehavior：

```text
Behavior {
    scope,
    trigger,
    plan_factory,
    reduced_motion_fallback,
}
```

Behavior 在 source transaction 稳定后消费合并后的已提交变化。factory 读取同一
stable epoch 的变化记录与 `StableSceneQuery`，返回 Plan 或不采取动作。

Behavior 不能作为普通 listener 回调重入 Runtime，也不能在 Motion 采样阶段重新解释
Trigger。Trigger 只决定“何时建计划”，不决定“如何插值”。

## 9.10 Before/After Capture

属性 Tween 可以显式提供 start/end value。布局、路由、换父和生命周期过渡则需要真正
的旧几何，不能在 mutation 后从新状态反推。

```text
捕获过渡前稳定边界、路径或图形准备结果
-> 提交 source mutation
-> 收敛新的稳定场景状态
-> 比较 before / after
-> admission
-> 建立视觉过渡
```

过渡前快照是 Runtime-scoped、one-shot 的拥有值。它不锁住 FigureTree，也不承诺
source rollback。

若 transition admission 失败：

- source mutation 保持已提交；
- 不创建半个 timeline；
- 不留下通道 owner、表现覆盖值或 wakeup；
- 画面直接显示已提交最终状态。

## 9.11 Damage 与视觉包络

每次采样必须同时给出前后保守视觉包围范围：

```text
damage =
    union(old_visual_envelope,
          new_visual_envelope)
```

包络需要覆盖：

- stroke 与 shadow outset；
- transform 后的保守范围；
- Connection decoration；
- 动画临时绘制项的创建、移动和删除；
- cancel 后当前表现与已提交最终视觉的差异。

只有表现覆盖值而没有保守视觉包围范围的效果不能进入可见管线。后端可以优化实际绘制，
但不能缩小 Core 无法证明安全的 damage。

## 9.12 Channel 冲突与 Retarget

当前公开 `InterruptionPolicy` 支持两种处理：

- Replace：从当前表现值原子 retarget；
- Ignore：保留现有 owner；

已接受设计还定义了 Queue 和 Additive，但当前公开 API 尚未实现。Queue 需要显式等待
当前 owner 结束；Additive 还要求通道声明组合代数。书稿不能把这两项写成当前可用策略。

交互状态通常使用 Replace。Retarget 从最后成功时间对应的缓存值构造起点，先完成
replacement 预检与整组初始采样，再转移 owner。初始采样不能推迟到旧 owner 被取消后。
每次基础采样也先收集整组受检值，再一次发布；不会把已通过的前半组写入后才发现
后半组失败。若候选失败，旧计划原样继续。

## 9.13 Cancel 不回滚事实

取消动画的默认语义是：

1. 移除 track、动画临时绘制项和 wakeup；
2. 清除表现覆盖值；
3. 显示已经存在的已提交最终状态；
4. damage 覆盖当前表现与最终视觉范围。

“停在当前值”会让没有 owner 的表现状态永久残留，因此不能作为普通 cancel mode。
若要从当前值继续，必须 retarget；若要回到历史起点，必须创建以历史值为目标的新
source operation 或新 Plan，而不是回滚已提交状态。

## 9.14 Disabled、Enabled 与 ReducedMotion

| 模式 | 行为 |
|---|---|
| Disabled | 不创建或保留 timeline，直接显示已提交最终状态 |
| Enabled | 执行显式 Plan 与已安装 Behavior |
| ReducedMotion | 使用声明过的短时、离散或静态 fallback |

全局 Disabled 高于局部请求。切换到 Disabled 时，Runtime 原子终止 active plan、清理
表现覆盖值和动画临时绘制项，并重算 damage 和 wakeup。

切回 Enabled 不自动复活已终止计划。模式切换只有在像素实际变化时才请求帧。

## 9.15 Suspension 与不可见状态

Surface suspended 或 target effectively hidden 时，每个计划需要明确策略：

- Advance：逻辑时间继续，恢复时采样当前绝对时间；
- Pause：恢复后平移 local time；
- Finish：清除表现覆盖值，直接显示已提交最终状态。

持续 Connection 效果通常使用 Pause；短时业务 transition 通常使用 Advance。不能由
每个 Figure 私自实现另一套隐藏计时规则。

## 9.16 交互与 Accessibility

公共协议声明三种交互几何策略：

- Committed：命中使用最终几何；
- Presentation：命中使用当前采样几何；
- NonInteractive：对象不参与输入或 accessibility。

当前 Figure 动画的命中仍使用已提交几何。`bind_figure` 和
`bind_presentation` 只接受 `Committed`；传入 `Presentation` 或 `NonInteractive`
会返回 `UnsupportedInteractionGeometry`。`Presentation` 策略保留在公共协议中，
现有 Figure 绑定尚未实现基于采样表现几何的命中。

`TemporaryVisual` 是树外动画临时绘制项，默认不参与命中或 accessibility。Viewer 的
Overlay、Handle 和 Feedback 则是 Figure 树中的临时 Figure 子树所有权机制，继续使用
Figure 树的坐标、裁剪、重绘和销毁协议。

未来若启用采样表现几何命中，绘制和命中必须消费同一表现覆盖快照。Accessibility 的
role、state 和 action 仍来自稳定场景状态；是否报告采样表现边界必须由平台合同明确。

## 9.17 领域能力如何接入

### 属性过渡

已提交属性立即变成最终值；Track 只覆盖 opacity、transform、Color 等动画
表现通道。未安装 Behavior 时保持静态行为。

### Layout result transition

LayoutManager 只计算一次 final bounds。动画在过渡前后稳定快照之间插值采样表现边界，
不每帧重新布局。

### Connection route transition

Router 只计算 final route。兼容 topology 可以插值；不兼容 topology 使用旧动画临时
路径与新路径的 opacity crossfade，不伪造通用 path morph。

### Viewport transition

已提交 origin/scale 先到最终值，表现变换再过渡。直接用户输入默认抢占自动
pan/zoom；auto-expose 仍是输入控制器，不伪装成固定 Tween。

### 持续效果

Dash flow、moving pulse 等使用 Procedural track，共享 Runtime clock。它们不修改 route、
通知、history 或 hit-test，并受 active-track 与动画临时绘制项预算约束。

## 9.18 失败语义

Animation admission 在修改任何表现所有权前验证：

- 单调时间；
- duration、key time、value 和 simulation 参数；
- target namespace 与 generation；
- 表现通道 capability 与冲突策略；
- interaction geometry；
- 保守视觉包围范围；
- active track 和动画临时绘制项预算。

失败边界：

- admission 失败不创建 ID、owner、表现覆盖值、wakeup 或 damage；
- retarget 失败保留旧计划；
- sample/provider error 终止当前计划并显示已提交最终状态；
- provider panic 沿 Runtime fault boundary 处理；
- target dispose 同一事务清理 track、动画临时绘制项和 wakeup；
- backend 不支持时在提交前拒绝或使用已声明 fallback。

动画失败从不回滚已经提交的模型、布局或路由。

## 9.19 复杂度与确定性

动画热路径应按 active animation 数量增长，而不是按 FigureTree 大小增长：

```text
advance/sample = O(active tracks)
表现覆盖查找 = O(animated subjects/channels)
damage merge = O(sampled visual effects)
```

所有随机数、时钟和平台尺寸必须作为显式输入。Headless 用固定 MonotonicTime 重放时，
相同计划、相同输入和相同时间点必须产生语义等价的表现覆盖快照。

性能降级不能改变最终已提交状态。预算耗尽只拒绝新计划，不能驱逐无关 active
plan 或静默修改组合语义。

`AnimationMut::limits()` 与 `set_limits()` 管理当前 timeline 的活动 owner、活动轨道
和独立候选轨道容量。`reservations()` 返回存活 owner 的准入预留；容量不会随单次采样
少用而释放。新 plan 检查总预留加候选，替换检查扣除旧组后的净预留。配置下调低于
存活预留时原子拒绝；staging 不足只拒绝候选，原 owner、缓存值和 wakeup 保留。
这层轨道账目是 ADR-046 几何容量扩展的基础，尚不证明节点、派生输出或快照 lease
容量已经完成。

## 9.20 必须保持的不变量

1. 已提交状态是模型、布局、路由、通知和历史的最终值来源；
2. 表现状态由 Runtime 拥有且可随时丢弃；
3. 动画采样不产生 source mutation 或 derived-state work；
4. 所有时间来自同一 Runtime 单调时间入口；
5. 每个可见 sample 都提供保守 old/new envelope；
6. 同一 subject/表现通道的 owner 唯一；
7. Cancel 清除表现覆盖并显示已提交最终状态；
8. Retarget 失败时旧计划原样继续；
9. 交互几何策略必须显式；
10. 无 active animation 时不持续扫描、唤醒或提交。

## 9.21 常见错误设计

### 每帧修改 Figure bounds

中间值污染布局、连接、通知和撤销历史。

### 每个 Figure 创建 timer

多个时钟无法统一暂停、重放、取消和资源清理。

### Backend 自行推进动画

Core 无法统一 damage、Headless 结果、ReducedMotion 和取消语义。

### Cancel 保留当前表现覆盖值

Timeline 已结束但表现状态仍存在，形成没有 owner 的第二权威。

### Layout 动画每帧重新运行 LayoutManager

动画帧率改变布局计算次数，源状态和最终结果不再确定。

### 用字符串 property path 找目标

值类型、命名空间和外部扩展边界只能在运行时猜测。

### 把动画临时绘制项当作 Overlay

两者具有不同所有者和树归属。`TemporaryVisual` 不进入 Figure 树；Overlay、Handle 和
Feedback 由 Viewer 管理，并继续使用 Figure 树的交互协议。

## 9.22 与前后章节的关系

第 8 章定义稳定场景状态、damage、RenderSubmission 和 Runtime acknowledgement。
本章在不改变已提交场景状态的前提下，为帧录制增加可选的稀疏表现覆盖快照。

下一章从平台输入进入场景。当前输入命中读取已提交几何；`Presentation` 策略已经进入
公共协议，但现有 Figure 绑定仍会拒绝该策略。第 11 章的 Viewport 和 Connection 是
表现状态层的领域消费者。

## 9.23 思考题

1. 为什么布局动画不能每帧调用 LayoutManager？
2. 源状态已经是最终值时，取消动画为什么不需要回滚？
3. Retarget 为什么必须在替换 owner 前完成预检？
4. 未来启用 Presentation 命中时，输入系统必须共享什么快照？
5. 不兼容 Connection route 为什么应 crossfade，而不是强行逐点插值？
6. Disabled 模式为什么仍需要清理旧表现覆盖值和 damage？
7. active set 为空时，如何证明动画服务没有持续成本？
