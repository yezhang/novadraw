# Runtime 所有权与模块协作边界

类型：`normative-design`

状态：`accepted`

范围：GA-4 Core/Editor 内部依赖与扩展更新入口。

实现状态：`complete`（2026-10-01）

上述是 GA-4 实现基线；2026-10-12 公开 mutation 按 ADR-033/S03 同步。
补充权限合同的验证范围见 [公共 API 契约](public-api-contract.md)。

## 1. 所有权图

```mermaid
flowchart TD
    Host["PlatformHost / RenderBackend"] --> Runtime
    Runtime --> Tree["FigureTree\n节点、拓扑、节点状态"]
    Runtime --> Updates["UpdateManager\n失效、damage、通知阶段"]
    Runtime --> Input["InteractionState / EventDispatcher"]
    Runtime --> Services["Connection / Resource / Text services"]
    Viewer["GraphicalViewer"] --> Runtime
    Viewer --> Parts["PartTree / policies / tools"]
```

`Runtime` 是唯一同时拥有这些可变组件的对象。`GraphicalViewer` 拥有一个 Runtime，
但不能取得其内部服务的并列可变引用。Platform adapter 与 RenderBackend 不拥有
FigureTree、UpdateManager 或 Viewer 状态。

## 2. FigureTree 与 UpdateManager

禁止依赖的含义是“禁止所有权和可变服务逃逸”，不是禁止同 crate 内任何函数调用。

允许：

- Runtime 在一个事务内同时短借 `&mut FigureTree` 与 `&mut UpdateManager`；
- FigureTree 的 crate-private 原语接收短生命周期 `&mut UpdateManager`，原子记录
  topology/state 变化产生的 invalidation、damage 和 notification；
- UpdateManager 的 crate-private 算法短借 `&FigureTree` 计算 damage 或 drain work；
- 协调逻辑位于 Runtime，或位于只被 Runtime 调用的 Core 内部函数。

禁止：

- FigureTree 字段持有 UpdateManager、InteractionState、PlatformHost 或 RenderBackend；
- UpdateManager 字段持有 FigureTree、Runtime、Figure 或平台对象；
- Figure、LayoutManager、Anchor、Router 或外部扩展保存上述可变服务引用；
- 从公共 API 同时返回 `&mut FigureTree` 与 `&mut UpdateManager`；
- 用 identity handle 建立第二套 `FigureTree + UpdateManager` 提交入口。

因此 `graph` 与 `runtime::update` 是同一 Core 事务内的协作模块，不是可独立发布的
package。实现模块划分及当时验证结果见
[GA-4 完成记录](../../verification/reviews/ga4-module-extension-completion-2026-10-01.md)。
内部职责划分不产生新的提交者。`Runtime` 仍是 Core 可变服务的唯一 owner，
`GraphicalViewer` 仍拥有单一 Runtime 与 Part/Policy 状态，没有新增共享可变服务。

## 3. 扩展组件更新

所有第三方 Figure 私有状态继续使用 `FigureComponentUpdate`。不同调用源只决定
更新何时提交和可选择的目标，不产生新的 mutation 方言。

### 3.1 Figure 回调

`EventContext::update_component_later` 只允许排队更新当前 callback target：

1. callback 移交 owned update；
2. Figure 不再被借用后，Runtime 按 effect FIFO 应用；
3. prepare/commit、revision、invalidation、damage 与 fault 语义复用 FigureMut；
4. deferred rejection 进入 Runtime deferred mutation errors；
5. callback 不能取得 receipt，也不能更新未被捕获的任意 Figure。

这条路径用于 Figure 的自身输入状态，不替代跨 Figure 命令或模型事务。

### 3.2 Editor 模型刷新

`VisualUpdateContext::update_visual_component` 只接受当前 EditPart 已登记的 visual：

1. Viewer 从 PartNode 的 visual ownership 建立短生命周期 context；
2. behavior 传入 owned typed update；
3. context 先验证 visual ownership，再通过 Runtime FigureMut 提交；
4. 非本 Part visual 在进入 Runtime 前拒绝；
5. update 错误转换为 `EditPartError`，Viewer 保持既有 refresh fault 边界。

该入口覆盖 primary Figure、content pane 和 `configure_visual` 登记的内部 visual，
不允许 behavior 修改其他 Part 或 feedback layer。

## 4. FigureMut 边界

`FigureMut` 保留以下通用职责：

- 通用节点状态与 bounds/style；
- 容器 add/remove/order/clip、layout manager/constraint、reparent、revalidate 和 repaint；
- `FigureComponentUpdate` 与 typed capability update 的统一提交。

Label、Clickable、Image、PointList、ScalablePolygon、TextFlow、Border 等私有内容
统一使用 typed component/capability update；不保留仅支持内置 concrete 类型的 facade。
类型适配封装在 update/descriptor 内，Runtime 不增加具体类型字段 setter 或中心 match。
错误、NoChange、revision、invalidation、damage 和 notification 使用同一提交原语。

Frames/Resources/Connections/Viewports/Layers 等子 facade 表达领域服务与关系权限，
只短借 Runtime，不拥有另一份缓存或待提交状态。Viewports/Layers 不能变成内置
Figure 专用 mutation 入口。取得 facade/identity/key 不等于取得任意跨对象写权限；
资源和结构 effect 必须先验证 scope，再完整预检后提交，见 [组件更新](component-update.md)。

## 5. 失败语义

- target foreign/disposed/faulted：返回或记录 `RuntimeMutationError`；
- deferred component 类型错误、revision 耗尽或 prepare rejection：记录结构化 deferred
  component error，不提交 revision、invalidation 或 damage；
- Event callback 移交 update 后 panic：沿 Runtime fault 边界处理，未开始的 effect
  不承诺执行；
- Editor visual ownership 不匹配：同步返回 `EditPartError`，不调用 update；
- component prepare/commit panic：Runtime/Viewer 进入既有 fault 隔离，不承诺回滚
  扩展内部副作用。

## 6. 验证

GA-4 至少保留两个独立外部消费者：

1. 自定义 Figure + Layout，覆盖 FigureMut 与 EventContext deferred component update；
2. 自定义 Router + Editor compound visual，覆盖模型 refresh 的 owned visual update。

消费者只能依赖公开 package API，不得使用 `pub(crate)`、测试 helper 或修改 Core 枚举。

当前可执行 suite 见
[验证入口](../../../tests/README.md)；定向消费者和契约使用精确 Cargo target。

完成证据见
[GA-4 模块与扩展表面完成记录](../../verification/reviews/ga4-module-extension-completion-2026-10-01.md)。
