# R9 消融式架构简化计划

类型：`migration-guide`

状态：`complete`

本文在 R8 完成后，以消融实验方式收窄 Novadraw 的实现和公共 API。目标不是减少类型
数量本身，而是删除没有独立语义或替换需求的暴露面，同时保持 Draw2D 行为语义、
Runtime 事务、跨平台边界和后续扩展能力。

## 1. 不变量

任何简化都必须保持：

- FigureTree 的拓扑、稳定 Z-order 和代际 FigureId；
- parent-local bounds 与统一 Affine2D 坐标链；
- self、children、border 的受控绘制顺序；
- EventContext effect recording 和 mutation FIFO；
- InteractionState 与 FigureTree 分离；
- Validation 先于 Damage Repair；
- LayoutSnapshot 到 LayoutOutput 的原子提交；
- PlatformHost 与 RenderBackend 分离；
- RenderSubmission 的 damage、resource、surface 和 frame identity；
- 10,000 层递归边界及 R8 性能基线。

## 2. 消融判据

对每个候选边界执行：

1. 删除或收窄该边界；
2. 保持调用方行为不变，不增加布尔开关、类型判断或旁路状态；
3. 运行相关契约测试和 workspace 门禁；
4. 对热路径批次运行 R8 性能复测；
5. 若需要恢复同等抽象才能表达第二实现，立即停止该项。

通过编译不等于消融成立。事件因果顺序、坐标往返、布局原子性和 damage 像素等价
都是必要观测量。

## 3. 批次

### R9.1 渲染遍历辅助类型收窄

状态：`completed`

- 将 `FigureRenderer`、`FigureTreeRenderRef` 限制为 crate 内部实现；
- 不改变 `render_recursive.rs` 的遍历逻辑；
- 外部只通过 FigureTree/Runtime 触发绘制。

消融结果：

- `render_recursive` 模块、`FigureRenderer` 和 `FigureTreeRenderRef` 已收窄为 crate
  内部实现；
- workspace fmt、check、Clippy `-D warnings` 与全量测试通过；
- R8 benchmark command 数完全一致，各场景 median 均未超过 15% 回归阈值。

### R9.2 事件分发公共面收窄

状态：`completed`

- 评估应用和验证入口能否全部迁移到 Runtime 命名操作；
- 删除没有第二实现的完整 dispatcher trait；
- 评估 `SceneDispatchContext`、`DispatchContext` 是否仍有独立用途；
- 保留单一 target、capture、focus、hover、gesture session 和 typed fallback；
- 若测试替身仍需要替换完整 dispatcher，则停止 trait 删除，只收窄 re-export。

已完成子批：

- 完整 dispatcher trait 只有一个实现，测试替身只替换 `DispatchContext`；
- 删除 dispatcher trait，将默认实现收敛为具体 `EventDispatcher`；
- M4、M6、M8 契约测试及 workspace 全量门禁通过。

停止结论：

- M4、M6、M8 契约测试使用 `DispatchContext` 注入可观测状态；
- 隐藏该 seam 会迫使 Runtime 增加测试专用公开访问器，公共面反而扩大；
- 因此保留 `SceneDispatchContext` / `DispatchContext`，不继续过度简化。

### R9.3 Mutation 包装层简化

状态：`completed`

- 区分外部 context 仍需要的 queue 与仅在 crate 内流转的 batch；
- 将纯包装 batch 合并为冻结后的内部 mutation 向量；
- 必须保留“冻结当前批次、新 mutation 延后”和 FIFO 语义。

消融结果：

- 删除 `PendingMutationBatch`；
- `PendingMutations::drain` 直接冻结为 crate 内部 mutation 向量；
- `FigureTree::apply_pending_mutations` 收窄为 crate 内部提交入口；
- `PendingMutations` 继续服务保留的 context seam，不强行收入 Runtime；
- FIFO、批次冻结、原子 mutation 和 Runtime 提交边界保持不变；
- workspace fmt、check、Clippy `-D warnings` 与全量测试通过。

### R9.4 条件项观察

状态：`completed`

以下内容至少观察到 M10 或出现重复实现证据后再决定：

- `Bounded` capability；
- typed listener 外观；
- `NodeState` 与 `FigureNode` 的物理布局；
- `ResourceDelta`；
- Figure 的输入、生命周期和 accessibility capability。

不得把这些能力合并回宽 Figure trait，也不得删除独立的状态域或事务阶段。

已完成结论：

- `Updatable` 只有 Triangle 存在非空实现，且 Runtime validation 已统一通过
  `FigureLifecycle` 调用；
- 删除 `Updatable` 及全部空实现，Triangle 派生缓存改用 inherent 方法并桥接
  `FigureLifecycle`；
- M10 完成后的消融复查确认 `Bounded` 仍承载构造期/独立图元可变几何，与树内
  `NodeState` 运行时真源分离；保留，不允许用于绕过 Runtime mutation；
- typed listener 分别固定 Figure、Coordinate、Ancestor、Property、Action、Layout
  payload 与回调签名；合并会退化为调用方 enum 分派，保留；
- `NodeState` 提供不依赖具体 Figure 的共享只读状态面，`FigureNode` 独立拥有拓扑、
  LayoutState 与具体行为；物理合并不减少状态域或事务，保留；
- `ResourceDelta` 的有序 op 序列是 Ready -> Failed -> Ready、Retry 前缀恢复和
  backend session 因果的必要边界，不能归约为无序最终 map，保留；
- Figure 输入、生命周期、accessibility capability 均有不实现该能力的 Figure，
  `Option<capability>` 明确表达能力缺席；合并回宽 Figure trait 会恢复空方法，保留；
- R9.4 未发现可继续删除且不损害独立语义的边界，条件观察关闭。

最终证据：
[`../verification/reviews/draw2d-core-1.0-final-audit-2026-09-13.md`](../verification/reviews/draw2d-core-1.0-final-audit-2026-09-13.md)。

## 4. 统一门禁

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

事件批次额外运行 M4、M6、M8 契约测试；热路径批次复用 R8 release benchmark。涉及
可见行为时继续使用 `doc/verification/manual/` 中对应流程。

## 5. 停止条件

- 简化要求修改递归渲染主流程；
- 需要合并 Runtime、FigureTree、InteractionState 或 UpdateManager；
- 需要让回调直接取得可变 FigureTree；
- 需要合并 local transform 与 child transform；
- 需要把平台或后端类型引入 Figure/runtime 协议；
- 简化后必须立即恢复同等抽象才能支持既有用例。
