# ADR-014: 扩展协议、生命周期与稳定发布边界修订

类型：`architecture-decision`

状态：`accepted`（设计修订；不表示已实现）

日期：2026-09-10

## 背景与替代范围

项目目标是可扩展的 Rust 图形框架，而不是只支持内置 Figure 的封闭渲染器，也不是
通用对象图迁移系统。Draw2D 的对象引用、生命周期行为和同步 listener 不能整体等同于
Rust arena、事务与延迟观察通知。

本次按用户要求重审 ADR-001 至 ADR-013。逐项证据、反例和保留结论见
[审计记录](../verification/reviews/adr-audit-2026-09-10.md)。
旧版全文及校验清单见[历史入口](../archive/adr-audit-2026-09-10/README.md)。

- 完全替代 ADR-013 的 owned detach/自动迁移方案。
- 修订 ADR-002/010 的内部行为与外部观察边界。
- 补充 ADR-003/006/009 的自定义组件写入协议。
- 修订 ADR-007/011 的文本测量、服务归属和稳定发布语义。
- 澄清 ADR-005/008 的失败批次、ADR-012 的 session 接管范围。
- 保留 Rust/Vello/Parley、parent-local 坐标、Runtime 组合根、只读策略、typed
  worklist、单 backend consumer 与有序资源日志。无证据的性能判断不升级为结论。

## 决策 1：Runtime 协调写入，但不枚举所有具体 Figure 类型

运行期 topology、NodeState、资源和关系状态仍只能经 Runtime 提交。第三方 Figure
必须能在不修改 Runtime 类型分支的情况下更新自己的内容：

1. 调用方提交拥有参数的 typed component update，而不是任意 `FnOnce(&mut Figure)`。
2. 组件从只读旧状态计算候选值，返回结构化错误或拥有候选值的 prepared update。
3. 预验证目标 namespace、类型、版本、引用与有限几何。失败不改变可见源状态。
4. 引擎以统一提交协议替换候选内容、提升 revision、生成 committed facts 和 damage。
   必需的失效类别由组件契约确定；未声明精细类别时保守失效 layout/geometry/paint，
   不允许默认“无影响”。
5. callback 只排队 owned update，实际执行仍遵循 FIFO 和逐操作原子性。

prepared update 是协议角色，不要求立即发明通用 plugin registry 或任意 effect DAG。
具体 Rust 对象安全 API 需以一个外部自定义 Figure 的非空更新用例验证后固定。
Figure 可保存组件关系引用，但必须声明归属和失效策略；不能维护第二份树拓扑。

LayoutManager 必须显式声明 constraint 接受策略。宽松兼容适配不等于语义校验已完成；
不能默认 no-op 校验后仍承诺所有第三方 constraint 类型错误均在提交前被拒绝。

自定义布局的测量输出和组件私有派生快照使用相同的“计算候选、校验、发布”原则。
sealed effects 只限制引擎结构与内置容器状态写入，不禁止第三方发布自己的派生快照。
不支持的能力必须在 prepare 阶段拒绝。

## 决策 2：原子性分层，不承诺任意用户代码回滚

区分：

- source operation：预验证失败没有可见修改；FIFO 后续操作失败不回滚已提交前缀。
- derived calculation：失败不发布半个 LayoutOutput/RouteBatch；已接受源输入不回滚。
- stable scene publication：所有必需派生工作闭合后才能发布新场景、录制和通知。
- presentation acknowledgement：由 `(session_id, frame_id)` 确认，与场景稳定不是一回事。

Rust 的 `&self` 不禁止 interior mutation，`Drop` 也可能 panic。纯计算、无外部副作用
和析构不 panic 是扩展实现必须遵守的契约，不是类型系统已经证明的事实。
一般扩展 panic 后恢复 phase guard，但将 Runtime 标记为 faulted 并拒绝新提交，
由 Host 重建或执行明确恢复流程；不能只清除 updating 标志就宣称状态可继续使用。
不承诺 `panic=abort` 或析构二次 panic 的恢复。

## 决策 3：生命周期行为、内部失效、外部观察分别建模

顺序为：

```text
prepare/validate and capture old geometry
-> commit engine topology and binding cleanup
-> complete required component activation/deactivation
-> drain derived work and publish stable scene
-> dispatch external observation journal
```

结构提交期间不调用用户代码或析构用户对象。先提取待释放对象，在结构一致后执行
组件生命周期和释放。必要组件行为得到旧/新关系的只读上下文，不获得可变 Runtime；
其结构写请求进入后续事务。行为完成前不能发布新场景；失败遵循 faulted 边界。

`Validating/Painting` 等延迟事件表示“阶段曾发生”，不是事前拦截 hook。
每条历史事件带 source revision/epoch、sequence、必要的 old/new 数据；查询上下文
只指向 flush 时最新 stable snapshot，不能宣称它是每个历史事件发生时的场景。
未稳定时的失败诊断走单独诊断出口，不依赖成功通知 flush。

订阅声明 `Runtime` 或 `Figure(owner)` scope。删除 Figure 自动解除 owner-scoped
订阅；不能根据 closure 捕获或 payload 过滤推断归属。共享资源、Router 和 Runtime
观察者不因一个引用者删除而销毁。ListenerId 具有 Runtime 归属，禁用进程全局分配器。

## 决策 4：销毁、脱离显示树、跨 Runtime 重建不再捆绑

D4.3 的必需公共能力是 namespaced identity、同 Runtime reparent、可靠 dispose
和显式 contents replacement。dispose 使旧 handle 失效；reparent 保持身份。
运行时 `remove_figure` 迁移为明确的销毁语义，不再暗中积累孤立 arena 节点。

本轮撤销“所有 Figure 均可自动提取为 DetachedSubtree 并跨 Runtime 重映射”的承诺。
Core 1.0 不暴露该通用迁移 API；在专门设计完成前明确 unsupported，而不是假 detach。
这不否认未来同域 unmount/mount 或 owned extraction 的价值，但必须分别定义：
对象由谁保活、ID 是否存续、共享引用如何处理及退出路径。

跨 Runtime 的默认方案是模型/描述 + 显式工厂重建，产生新运行时身份。
GEF 层 history 保存模型操作，不要求保存活 Figure。需要保留活对象身份时另行评估
对象存储与显示树分离，不能声称 SlotMap 是唯一可行模型。

Runtime 是 FigureTree、资源、连接、订阅、更新状态和 backend session 的最小完整
所有权单元。`Runtime::new(FigureTree)` 只接收从未附着 Runtime-owned registry 的
构建期新树；运行期不提供 `Runtime -> FigureTree -> Runtime` 的有损重包装。
需要转交所有权时整体 move Runtime；需要独立副本时由模型/描述重建新的 Runtime。
场景工厂同样交付 Runtime，不能先注册资源或连接再只返回 FigureTree。

具体销毁、共享绑定、生命周期和验证契约见
[Figure 生命周期](../design/architecture/figure-lifecycle.md)。

## 决策 5：约束测量属于 layout，不属于 paint-only presentation

单行 Label/TitleBar 可以用自然尺寸参与布局、最终 ellipsis/alignment 仅 repaint。
换行文本必须支持 `measure(constraints)`：父级提供宽度，子级返回该宽度下高度、
baseline 与不可变布局快照，再完成 arrange。不能先锁死高度，再在 presentation 中换行。

测量缓存包含内容、字体 revision 和规范化约束。宽度变化只重算受影响测量；
后续 placement 复用同约束快照。合法的测量依赖不等于允许任意环；动态反馈仍有预算，
非收敛不发布。完整 TextFlow/编辑仍延后，但其契约不能被 Label 特例封死。

文本服务在运行期由 Runtime 独占；构建器只能拥有待移交配置/服务，FigureTree
不成为第二个文本服务所有者。字体 fallback 只来自显式注册集合，不隐式依赖系统字体。
Glyph IR 和外部受校验构造器仍是可替换后端的边界。

## 决策 6：失败状态和 session 顺序显式化

- 路由整组失败时不发布部分成功 route/reservation；运行期以一个失败批次清空该组
  几何/reservation 并标记 unresolved，保留恢复依赖。这是稳定错误结果，不是提交失败
  后继续显示旧有效路线。
- session generation 仅在同 namespace 比较，namespace 无时间顺序。
- Core 1.0 Host 串行化 backend 所有权切换，先停止旧生产者并排空或撤销旧提交，
  再交给新 Runtime，以 Snapshot + Full 建立基线。返回此前 Runtime 也重建基线。
- 仅靠 incoming namespace 不同就清 cache，不能证明跨 Runtime 的迟到 submission
  一定会被拒绝。未来允许并发交接时须增加 Host/backend 颁发的 activation token，
  不用 UUID 大小冒充时间顺序。
- 新 session/首次接管不能以 Delta 建立基线；缺基线应结构化拒绝并请求 snapshot。
  永久 unsupported 不得无限 Retry。

## 取舍与验证

收窄通用活对象迁移承诺，换取可明确验证的 Core 生命周期；增加组件 prepared update、
scoped subscription、测量快照等协议验证成本。严格 viewport topology 仍是 Core 1.0
的有意限制，不宣称完全兼容 Draw2D 跨视口连线。

新契约不修改既有验证记录，也不把 D4.1/D4.2 历史通过结果当作本修订已实现证据。
历史门禁见 [D4 收口计划](../archive/core-completion-and-draw2d-migration-plan.md)：
外部组件更新、scope 清理、panic/fault、宽度约束测量、group 失败和 session handoff。
未证明对象安全接口、引用所有权或 panic 边界时，继续停止编码并回到设计。
