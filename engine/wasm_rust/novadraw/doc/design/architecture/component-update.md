# Figure 组件更新协议

类型：`normative-design`

规范效力：`accepted`

实现状态：`implemented`

范围：ADR-014 D4.4 与 ADR-033 的统一 typed component/capability update。

2026-10-12 修订：补齐 S03 已提供的 NoChange 和 effect 合同；新增权限验证要求见
[修订记录](../../verification/reviews/framework-architecture-contract-revision-2026-10-12.md)。

2026-10-13 修订：Effect 改为受限声明；Runtime 在写入前校验完整操作序列，
不再调用外部 Effect commit。组件与 capability 更新共用同一提交实现。

## 1. 问题

Runtime 必须继续作为 source mutation 的唯一提交边界，但不能为每一种第三方 Figure
新增 downcast 分支。直接暴露 `&mut dyn Figure` 或接收任意 mutation closure 会绕过
revision、damage、layout invalidation 和 panic/fault 边界。

组件 commit 只更新目标 Figure 的私有状态，不直接修改 topology、Runtime registry、
LayoutManager 或其他 Figure。prepared update 可以声明受限资源/结构 effect；
Runtime 统一预检和提交，不授予扩展可变服务引用。独立跨对象关系操作仍使用领域 facade。

## 2. 公共接口

以下表达协议角色；完整签名与构造器由领域公开 API snapshot/rustdoc 维护。

```text
FigureMut::update_component(owned typed update)
    → FigureComponentUpdate::prepare(current typed Figure, read-only context)
    → Result<NoChange | owned candidate + declared effects, domain error>
    → validate complete candidate/effects
    → FigureComponentUpdate::commit(candidate, mutable target)
    → ComponentUpdateReceipt
```

变更候选默认采用 `LayoutGeometryAndPaint`；调用方必须显式选择更窄的 invalidation，
不能把未声明影响解释为无影响。prepared outcome 区分 `NoChange` 与 changed candidate；
NoChange 没有待 commit 的值，也不附带属性通知、资源依赖或结构 effect。
`paint` 和 `geometry_and_paint` 是变更候选的显式窄化构造器。
`ComponentUpdateReceipt` 表达 Figure、提交前后 revision 和实际变化/失效结果。
`FigureTree::component_revision` 提供只读 revision 查询，不暴露可变 Figure。
`FigureMut` 只借用 Runtime 并转交目标身份，不拥有另一份状态或提交协议。

## 3. 执行顺序

```text
validate Runtime/attached/type
-> capture old visual envelope and immutable context
-> U::prepare(&typed Figure, context)
-> NoChange: return unchanged receipt without commit or effects
-> changed: validate revision capacity, owned candidate and all declared effects
-> U::commit(prepared, &mut typed Figure)
-> commit prevalidated engine effects without invoking extension callbacks
-> increment component revision
-> enqueue declared invalidation, typed facts and old/new damage
-> return committed receipt
```

prepare 读取本次 FIFO 提交前缀的当前 Figure 和上下文，不能使用入队时的陈旧快照替代。
连续两个更新中，第二次 prepare 必须看到第一次已提交的值。节点、资源及关系输入
只能从受控只读上下文取得。扩展不得通过 interior mutability 在 prepare 内改源状态。

NoChange 不调用 commit，不提升 revision，不改变 pending work、NotificationJournal、
DamageAccumulator 或资源/结构关系。错误身份、错误类型仍先拒绝，不能用 NoChange
绕过校验；revision capacity 只约束真实变更。相等语义由 typed 更新定义。

`prepare` 失败时，Runtime、Figure 与 revision 不变，owned update 及候选值正常释放。
`commit` 没有 Result；prepared value 必须已完整验证。`prepare` 或 `commit` panic 时
Runtime 标记 faulted，不承诺回滚 interior mutation 或外部副作用。

Runtime 能统一验证 target、namespace、revision 和 NodeState 几何；第三方私有候选值
的有限几何及引用合法性由 `prepare` 负责，因为引擎不能枚举其内部表示。

## 4. 私有派生快照

第三方 Figure 可把派生快照保存在自身具体类型中，由 prepared value 原子替换。
Runtime 不枚举快照类型，也不提供通用 `Any` registry。若派生计算依赖布局后的 bounds，
source update 只发布输入并声明 layout invalidation；稳定化阶段使用后续独立的 typed
derived component 协议，不能在 source commit 中读取未来布局结果。

### 4.1 声明 effect 与提交权限

| Effect | prepare 声明 | Runtime 必须校验 |
|---|---|---|
| 属性事实 | typed PropertyKey 与 old/new payload | owner、key/payload 一致性；不是第二份属性真值 |
| 资源依赖 | 私有内容所需资源身份/版本与依赖替换 | namespace、生命周期、请求版本；不能伪造完成或覆盖 registry |
| 结构关系 | 受限 add/remove/reparent/constraint 等意图 | 目标权限、拓扑环、深度、容器 admission、约束与关系清理 |

持有 FigureId 或 typed key 不等于具有跨对象写权限。EventContext 只能作用于 callback
target；Editor visual context 只能作用于当前 Part 已登记 visual。更大 scope 必须由
明确授权的领域操作提供，不能靠 payload 中出现任意 ID 扩权。

`FigureUpdateEffect::prepare` 消费 owned effect，通过 `FigureUpdateEffectContext`
声明属性事实、资源依赖、bounds、border 派生状态刷新与 reparent。Context 的 tree 查询
读取已提交前缀；操作校验按声明顺序考虑当前 target 的前序意图。
属性、资源依赖和几何出口隐式绑定 target；reparent 的 child 必须等于 target，
否则返回 `EffectTargetMismatch`。跨对象结构修改使用独立领域 facade。
资源操作检查 old/new 身份，连接 reparent 保留预先转换的约束，并在后续移动中继续转换
前序候选约束。合法 target 父级不位于 target 子树，其坐标链不会被本批操作修改。

全部 effect 在 commit 前从同一已提交前缀按顺序完成预检；任一非法 effect 都拒绝整个候选，
不修改组件、revision、资源/拓扑、work、journal 或 damage。提交后必要的 lifecycle
回调在结构写入借用释放后执行。用户 callback/Drop panic 沿 fault 边界处理，
不属于可恢复原子失败承诺。

Context 记住操作拒绝状态；扩展忽略某次操作的错误也不能提交前序意图。
NoChange 不调用 Effect prepare。提交阶段仅执行 Runtime 保存的已验证操作，
不重新计算连接约束或执行外部 Effect 回调。

外部消费者证据：`core.p2-f02-figure-capability` 覆盖 integration crate 中自定义
Effect 的连续移动、越权与后续非法操作拒绝、资源/几何拒绝、NoChange，以及 component
属性事实无部分提交；连接运行时契约覆盖连续移动时绝对/相对 bendpoint 的坐标保持。

## 5. 首个验收用例

在独立 integration test 中定义外部 `BadgeFigure`：

- 私有字段为 `text`、`measured_width` 和不可变 paint snapshot；
- `SetBadgeText` 拥有新 String，prepare 拒绝空文本并生成候选；
- commit 不需要修改 Runtime 分支；
- 成功后 revision 单调增加并产生 layout + paint；
- prepare 失败保持旧文本、revision 和 pending work 不变；
- foreign/disposed/wrong type 返回结构化错误；
- commit panic 后 Runtime faulted，后续 frame 返回 `FramePreparation::Error(Faulted)`。

该用例已落地于 `crates/novadraw/tests/d4_component_update.rs`。测试还验证保守更新实际
提升 layout generation、产生非空 damage，且完整 Runtime 移动之外不存在可变
Figure 逃逸。

## 6. 受控调用源

FigureMut 是同步更新入口。以下调用源复用同一协议，不获得 `&mut Figure`：

- `EventContext::update_component_later` 只排队更新当前 callback target；Runtime 在
  callback 借用结束后按 effect FIFO 应用，失败进入 deferred mutation errors；
- `VisualUpdateContext::update_visual_component` 只同步更新当前 EditPart 已登记的
  primary/content/internal visual，非 owned visual 在进入 Runtime 前拒绝。

Event callback 不返回 receipt，因为提交发生在 callback 之后。Editor refresh 可取得
receipt，但对 behavior 统一转换为 `EditPartError`，继续受 Viewer refresh fault 边界保护。
两条路径的所有权和失败规则见
[Runtime 所有权与模块协作边界](runtime-ownership-and-module-boundaries.md)。

## 7. 暂不包含

- 任意 closure mutation；
- 跨 Figure 原子事务；
- 扩展直接写 topology 或 registry；
- 动态 plugin registry；
- 跨 Runtime prepared value 重放；
- 通用派生 DAG。

受限 effect 声明不成为任意跨对象事务。Capability update 复用同一骨架，见
[Figure Capability 模型](figure-capability-model.md)。
