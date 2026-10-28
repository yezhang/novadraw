# Figure 生命周期与 Runtime 身份

类型：`normative-design`

状态：`accepted / implemented`

范围：D4.3；决策：[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)。
本文替代原 ADR-013 配套规范，不表示旧实现已经符合新契约。

## 1. 身份、拓扑与所有权

```text
FigureId = RuntimeNamespace + LocalFigureId(index, generation)
```

RuntimeNamespace 在构建身份域时创建，Runtime 接管后沿用；ResourceRegistry 与
BackendSessionId 使用该身份域。公共入口验证 namespace、generation 和操作允许状态。
内部可信遍历可用紧凑 local key，不要求逐节点 UUID 查找，也不引入全局 ID 分配器。

FigureTree 只能从构建期单向移交给 Runtime。Runtime 创建 registry 后成为最小完整
所有权单元，不再提供只取出 FigureTree 的 API；转交使用 Rust move 整体移动 Runtime。
因此旧 resource/listener/router/session 不会因同树重包装而复活。

业务 UUID、运行时 ID、树成员关系是不同概念。当前 arena 所有权模型不是 Rust 唯一
方案。本文不承诺存活对象跨 Runtime 保持 ID。

- `reparent`：同 Runtime 的 attached subtree 移动，ID 不变。
- `dispose_subtree`：移除并释放节点，旧 ID 立即无效。
- `remove_figure`：公共 convenience 明确委托 dispose，不能只断链后永久留存节点。
- `set_contents`：原子替换；新候选预验证失败保留旧 contents，成功后销毁旧 contents。
  新旧相同为 no-op；不接受新 contents 位于待销毁旧 subtree 内，需先显式 reparent。

错误区分 ForeignRuntime、UnknownOrDisposedFigure、InvalidTopology、
SyntheticRootOperation、UnsupportedLifecycleOperation。无需永久 tombstone。

## 2. 本期不提供通用脱离/迁移

Core 1.0 不提供任意 `Box<dyn Figure>` 的 owned detach/reattach 自动迁移。
不能以一个 token 冒充调用方已取得对象所有权，也不能把隐藏在 arena 的孤立节点
当作已完成的内存清理。

未来能力分别设计，不能混用同一名称：

| 能力 | 必须另行回答 |
|---|---|
| 同域 unmount/mount | 谁显式保活、如何释放、ID 是否保持、是否参与扫描和更新 |
| owned extraction | Figure/布局/连接内部引用、共享状态和失败返还所有权 |
| 跨 Runtime 重建 | 模型/描述到新对象的工厂、资源解析、业务 ID 到新 FigureId 映射 |

当前跨 Runtime 方案采用显式模型/描述重建，不尝试反射或遍历任意 trait object。
GEF history 属于编辑器模型层；不要求保存活 Figure。保留活对象确有必要时再评估
对象存储与显示树分离。

## 3. Dispose 的准备与提交

### Prepare

只读计算并完整验证：

1. namespace、generation、synthetic root、attached 与深度限制；
2. 受影响子树、parent constraint、layer membership 和外部引用；
3. 旧 visual envelope，冻结为 root/surface 逻辑域 damage，不再依赖被删除节点；
4. interaction、update work、UUID lookup、资源依赖、连接与订阅的清理计划；
5. 组件生命周期上下文所需旧关系快照与待释放对象列表容量。

准备失败不改变可见源状态。内存分配失败等不可恢复进程错误不在 Result 回滚承诺内。

### Engine commit

不运行用户 callback、策略校验或析构。结构提交执行：

```text
remove parent relation/constraint and layer membership
-> clear focus/hover/capture/gesture ownership
-> detach owned registrations and dependency edges
-> mark affected external connection groups dirty/unresolved
-> remove UUID/cache/update references
-> extract arena nodes into retired objects
-> append frozen old damage and parent invalidation
```

对象先 move 到 retired storage，不能在 `remove`/`retain` 隐式 Drop 用户对象。
引擎结构提交不能出现可恢复失败；这一要求不等于整个 lifecycle 是任意副作用事务。

### Component completion and release

必要的 deactivation 按 descendant-before-parent 顺序执行，接收旧关系只读快照；
不得通过已失效 ID 查询旧树或取得可变 Runtime。结构请求只能排入后续事务。
attach/add 的 activation 按 parent-before-children，完成后再稳定化和发布。

默认要求 lifecycle 与 Drop 不 panic。遇到扩展 panic，恢复 guard 并标记 Runtime
faulted，拒绝继续发布；不宣称回滚用户副作用，也不保证 abort/二次 panic 可恢复。
在结构一致后才释放 retired 对象。正常路径各对象仅释放一次。

## 4. 清理以归属为准

| 对象 | 删除子树时 |
|---|---|
| 子树节点、私有缓存、parent-owned child constraint | 移出引擎，按生命周期顺序释放 |
| focus/capture/gesture | 清除引用；可选通知不能替代实际清理 |
| resource dependency | 移除依赖边，不删除其他 Figure 使用的资源 |
| connection | 删除自身状态和组成员资格；外部引用失效 owner 的组重新求解 |
| Router/Anchor registry entry | 移除 binding；仅释放明确 node-owned 且不再被引用的条目 |
| layer key/UUID lookup | 删除失效映射，保留其他节点映射 |
| Figure-scoped listener | 解除注册，终止后续调用 |
| Runtime-scoped listener | 保留；可收到带旧 ID/快照的删除事实，不可解引用旧节点 |
| invalid/dirty work | 删除 stale source；已经冻结的旧 damage 不丢弃 |

共享 registry 对象不能因最后一个当前连接删除就被默认销毁，除非其注册策略明确
采用该所有权规则。Runtime-owned 对象由显式 unregister 或 Runtime drop 释放。
被删除 owner 的连接恢复依赖只用于显式重绑定/输入变化，不复活 stale FigureId。

## 5. 生命周期不是 observer

内部绑定清理不依赖 `ListenerId` 或 observer 执行。组件行为完成且派生状态闭合后，
Runtime 才发布新的 stable scene；外部 listener 读取该稳定上下文。
删除事件携带必要的旧数据，不要求旧 Figure 仍存活。提交后通知不是 Draw2D
删除前 `removeNotify` 的逐调用栈复制，差异在 ADR-014 中明确接受。

## 6. 验证入口

实施时必须新增公共 API 契约验证，不能引用旧报告代替：

- 两个 Runtime 相同创建顺序，foreign Figure/Router/Anchor/Listener handle 均不能误命中。
- reparent 保持 ID；dispose 后 arena 数下降、Drop 一次、旧 ID 永远不命中新节点。
- old visual envelope 在下一成功帧擦除；节点已删除不影响 damage。
- 共享 Router、共享资源、Runtime listener 不因单个 Figure 删除而失效。
- owner-scoped listener、interaction、layer lookup 与 update source 无 stale 引用。
- 必要 lifecycle 在 stable publication 前完成，observer 在后；panic 后拒绝新提交。
- contents 预验证失败保留旧树；重复替换不累积孤立节点。
- 未支持的活对象迁移明确失败，不提供占位成功或隐藏保活。
- 10,000 层边界继续遵守现有深度约束；不修改受保护渲染主循环。
