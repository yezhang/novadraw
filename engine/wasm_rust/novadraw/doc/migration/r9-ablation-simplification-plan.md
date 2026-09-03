# R9 消融式架构简化计划

类型：`migration-guide`

状态：`in_progress`

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

状态：`pending`

- 先把应用和验证入口迁移到 Runtime 命名操作；
- 再将 `SceneDispatchContext`、`DispatchContext` 和默认 dispatcher 实现收窄；
- 保留单一 target、capture、focus、hover、gesture session 和 typed fallback；
- 若测试替身仍需要替换完整 dispatcher，则停止 trait 删除，只收窄 re-export。

已完成子批：

- 完整 dispatcher trait 只有一个实现，测试替身只替换 `DispatchContext`；
- 删除 dispatcher trait，将默认实现收敛为具体 `EventDispatcher`；
- M4、M6、M8 契约测试及 workspace 全量门禁通过。

剩余工作：

- 将应用和外部契约测试的直接 context 构造迁移到 Runtime；
- 评估 `SceneDispatchContext` / `DispatchContext` 是否可完全收窄。

### R9.3 Mutation 包装层简化

状态：`pending`

- 在调用方不再直接构造 mutation queue 后，将其收进 Runtime；
- 允许合并纯包装类型；
- 必须保留“冻结当前批次、新 mutation 延后”和 FIFO 语义。

### R9.4 条件项观察

状态：`deferred`

以下内容至少观察到 M10 或出现重复实现证据后再决定：

- `Bounded`、`Updatable` capability；
- typed listener 外观；
- `NodeState` 与 `FigureNode` 的物理布局；
- `ResourceDelta`；
- Figure 的输入、生命周期和 accessibility capability。

不得把这些能力合并回宽 Figure trait，也不得删除独立的状态域或事务阶段。

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
