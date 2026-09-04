# 树查询与焦点遍历契约

类型：`normative-design`

状态：`approved`

本文定义 D1.5 的规范契约。各分批只有在实现与验证完成后才视为已交付。

## 1. 目标

- 为几何命中提供可接受、可剪枝的搜索策略；
- 消除普通命中和事件目标查找之间重复的树遍历规则；
- 提供稳定的 ancestor/descendant 查询；
- 区分直接请求焦点与键盘遍历资格；
- 让 Native、Web 和 Headless 共享同一焦点状态机。

本阶段不引入 DOM 式事件冒泡、GEF selection、focus ring 绘制或平台控件树。

## 2. Draw2D 基线与 Novadraw 变体

Draw2D `TreeSearch` 在访问节点时先执行 `prune`，子节点均未命中后才执行
`accept`。子节点按逆 Z-order 搜索，因此返回最上层的最深节点。

Draw2D `FocusTraverseManager` 使用树顺序：

- forward：前序、child 从前到后；
- backward：前序的逆序；
- 仅 `isFocusTraversable && isShowing` 的 Figure 可由 Tab 获得焦点；
- 遍历到边界返回空，不在引擎内循环。

Novadraw 保留这些语义，并作以下明确变体：

- `FigureId` 替代对象引用；
- 搜索策略只能读取受控的 `TreeSearchContext`，不能修改 FigureTree；
- effective enabled 与 effective visible 同为搜索和焦点资格的硬门禁；
- Runtime 在焦点切换时先原子更新 owner，再按 lost → gained 顺序投递事件；
- 平台 traversal 与普通 `Key::Tab` 分开建模，边界结果交回平台。

## 3. TreeSearch

接口：

```rust
pub trait TreeSearch {
    fn prune(&mut self, candidate: TreeSearchContext<'_>) -> bool {
        false
    }

    fn accept(&mut self, candidate: TreeSearchContext<'_>) -> bool {
        true
    }
}
```

`TreeSearchContext` 是只读临时视图，至少提供：

- `id() -> FigureId`
- `parent_id() -> Option<FigureId>`
- `state() -> &NodeState`
- `figure() -> &dyn Figure`

它不能逃逸当前查询，也不暴露 mutation、UpdateManager 或 Runtime。

内置策略：

- `IdentitySearch`：不剪枝，接受所有候选；
- `ExclusionSearch`：命中排除集合中的节点时剪掉整个子树。

### 3.1 命中 API

```rust
FigureTree::hit_test(point)
FigureTree::hit_test_with(point, &mut dyn TreeSearch)
FigureTree::hit_test_excluding(point, excluded)
```

现有 `hit_test` 是 `IdentitySearch` 的便利入口，返回值和路径语义保持不变。

遍历顺序固定为：

```text
effective visible/enabled
→ inverse edge transform into node local
→ policy-aware branch containment
→ strategy.prune
→ effective child clip 与 child transform
→ children reverse z-order
→ self participation / precise geometry / strategy.accept(current)
```

`accept` 只决定当前节点能否作为结果；`prune` 同时排除当前节点和整个子树。
策略不能绕过 visibility、enabled、几何、裁剪、坐标转换或深度上限。

默认 branch containment 要求 point 命中当前节点 precise geometry，并位于允许下降的
client clip。`HitParticipation::DescendantsOnly` 只禁止当前节点成为结果，不等价于
`precise_hit = false`，也不自动剪掉 children。

`ChildClippingStrategy::OverflowVisible` 是 Freeform 容器的显式变体：branch
containment 使用进入容器的有效 ancestor clip，不再与当前 client box 相交，因此可以
搜索 border-box 外的负坐标 descendants。paint、hit-test 和 damage 必须使用同一个
effective clip；普通容器继续使用默认 client clip。

事件 target、cursor、tooltip 和 gesture 必须复用同一遍历内核，仅通过内部
`TreeSearch` 策略表达各自的接受条件，不再维护第二套递归算法。

### 3.2 结构查询

FigureTree 提供以下只读查询：

```rust
ancestor_ids(id) -> Option<Vec<FigureId>>
descendant_ids(root) -> Option<Vec<FigureId>>
is_ancestor_of(ancestor, candidate) -> bool
find_in_subtree(root, &mut dyn TreeSearch) -> Result<Option<FigureId>, TreeQueryError>
```

- ancestor 顺序为 parent → root，不包含自身；
- descendant 顺序为稳定前序、child 从前到后，不包含 root；
- unknown root 返回结构化错误，不与“无匹配项”混淆；
- stale exclusion ID 无副作用；
- 查询期间不允许结构修改。

## 4. 焦点资格

`NodeState` 增加两个互相独立的局部属性：

```text
focusable          = 允许显式 request_focus 或鼠标行为请求焦点
focus_traversable  = 允许键盘 traversal 选择该节点
```

两者默认均为 `false`，都不继承。内置控件通常同时开启；只需要程序化焦点或只参与
遍历的 Figure 可以分别配置。

资格规则：

```text
direct focus =
    attached
    && effectively visible
    && effectively enabled
    && focusable

traversal focus =
    attached
    && effectively visible
    && effectively enabled
    && focus_traversable
```

焦点资格不再由 `FigureEventHandler::wants_key_events()` 推导。没有 key handler 的
Figure 仍可接收 focus gained/lost；键盘事件投递给没有 handler 的 owner 时自然
成为 no-op。

## 5. FocusTraversalPolicy

候选接口：

```rust
pub enum FocusTraversalDirection {
    Forward,
    Backward,
}

pub trait FocusTraversalPolicy {
    fn traverse(
        &mut self,
        tree: &FigureTree,
        scope: FigureId,
        current: Option<FigureId>,
        direction: FocusTraversalDirection,
    ) -> Option<FigureId>;
}
```

Runtime 持有 policy，默认使用 `TreeOrderFocusTraversal`。应用可替换 policy，但
Runtime 必须再次验证返回节点的 attachment 和 traversal 资格，错误策略不能污染
interaction state。

默认 policy：

- scope 为当前 contents，不包含 synthetic root；
- forward 使用稳定前序；
- backward 使用前序的逆序；
- current 为空时 forward 返回首个候选，backward 返回最后一个候选；
- 到达首尾返回 boundary，不自动 wrap；
- current 不在 scope 或已失效时，按无 current 处理。

这里对 backward + no current 采用对称的“最后一个候选”，是 Novadraw 为 Web/native
反向进入画布提供的合理变体。

## 6. Runtime 焦点事务

候选公开入口：

```rust
Runtime::request_focus(id) -> Result<FocusChange, FocusError>
Runtime::clear_focus() -> FocusChange
Runtime::traverse_focus(direction) -> FocusTraversalOutcome
Runtime::set_focus_traversal_policy(policy)
```

`FocusChange` 区分 changed/unchanged；`FocusTraversalOutcome` 区分 moved/boundary。
unknown、detached、hidden、disabled 和不可 focus 的请求返回结构化错误，且不改变
原 owner。

焦点切换是单个 Runtime 事务：

```text
validate requested owner
→ commit interaction.focus_owner
→ dispatch FocusLost(old, related = new)
→ dispatch FocusGained(new, related = old)
→ flush callback effects
→ apply deferred mutations
→ reconcile focus eligibility
```

同一 owner 的重复请求不产生事件。owner 因 remove、hide 或 disable 失效时必须经过
同一切换路径产生 FocusLost，禁止由 `InteractionState::reconcile` 静默删除。

鼠标按下后的自动焦点规则：

- 事件已处理且 target 具有 direct focus 资格时请求焦点；
- target 不具备资格时保持原 focus owner，不得清空旧焦点。

## 7. 平台 traversal

平台 adapter 将 Tab/Shift+Tab 转为 `FocusTraversalDirection`：

- `Moved`：消费平台 traversal，防止同一次按键再次离开画布；
- `Boundary`：不消费，允许浏览器或窗口系统把焦点移出画布；
- traversal 成功时不再把同一 Tab 作为普通 key event 投递。

Headless 直接调用 `Runtime::traverse_focus`，用于确定性契约测试。普通字符键仍只投递
给当前 focus owner，不引入冒泡。

## 8. 失败模式

| 场景 | 行为 |
|------|------|
| 搜索起点不存在 | `TreeQueryError::UnknownFigure` |
| strategy 排除当前节点 | 当前节点和整个子树均不访问 |
| direct focus 目标不合格 | 返回 `FocusError`，原 owner 不变 |
| policy 返回 scope 外节点 | 视为 boundary，原 owner 不变 |
| owner 在事务后失效 | 发出一次 FocusLost 并清空 |
| callback 请求结构修改 | 延迟到事件投递完成后执行 |
| traversal 到边界 | 返回 boundary，不循环 |

## 9. 分批实施

### D1.5a TreeSearch

状态：`complete`

- 引入只读搜索上下文和内置策略；
- 统一几何命中与事件目标遍历内核；
- 增加 exclusion、ancestor 和 descendant 契约测试。

### D1.5b Focus model

状态：`complete`

- 增加显式 focusable/focus_traversable 节点属性；
- 引入默认 tree-order policy；
- 修正 focus 切换和失效清理的因果顺序；
- 迁移现有 event demo 的显式资格配置。

### D1.5c Platform traversal

状态：`complete`

- Native/Web adapter 接入 Tab/Shift+Tab；
- Headless 覆盖 forward/backward/boundary；
- macOS 和 Web 人工验收于 2026-09-04 通过，无失败项。

## 10. 完成门禁

- identity search 与现有 hit-test 结果一致；
- exclusion prune 不访问被排除子树；
- paint Z-order 与 hit-test reverse Z-order 保持一致；
- ancestor/descendant 顺序稳定，unknown 与 no-match 可区分；
- direct focus 与 traversal focus 资格独立；
- forward/backward、hidden/disabled、remove/reparent 和 boundary 均有测试；
- lost → gained 顺序唯一，失效 owner 不被静默删除；
- macOS 与 Web 的 Tab/Shift+Tab 行为通过人工验收；
- workspace test、核心 Clippy 和 Web target check 通过。
