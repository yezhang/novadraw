# 迭代渲染归档决策

类型：`archive`

## 决策

`render_iterative.rs` 是历史性能方向 POC，不再属于当前 Draw2D 核心主线。主线只保留递归渲染路径，直到 M1-M10 核心契约完备。

## 原因

- 递归渲染语义尚未全部完备前，迭代渲染会制造第二条需要同步维护的主循环。
- 当前阶段的优先级是 Draw2D paint、clip、coordinate、layout、event 契约收敛，而不是极端深度下的性能优化。
- 长期保留未成熟路径会干扰搜索、测试门禁和后续架构 delta 判断。

## 归档点

历史 POC 保留在 git tag：

```bash
archive/render-iterative-poc-20260617
```

未来如需查看或恢复：

```bash
git show archive/render-iterative-poc-20260617:novadraw-scene/src/graph/render_iterative.rs
git restore --source=archive/render-iterative-poc-20260617 -- novadraw-scene/src/graph/render_iterative.rs
```

## 恢复条件

只有满足以下条件后，才允许重新引入迭代渲染：

- M1-M10 Draw2D 核心契约已完成，并且递归渲染路径成为行为基准。
- 已定义独立性能专项 delta，明确恢复范围、契约、bench 和回归门禁。
- 重新引入时必须以递归渲染为 oracle，先建立等价测试，再恢复显式栈实现。

## 当前禁止项

- 不恢复 `render_iterative.rs` 到主线。
- 不恢复 `use_iterative_render` 状态字段。
- 不恢复 I 键渲染模式切换。
- 不把递归/迭代等价作为当前 M3 完成门禁。
