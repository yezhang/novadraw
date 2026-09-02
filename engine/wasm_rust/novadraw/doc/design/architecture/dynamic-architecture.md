# 理想架构：动态协议

类型：`normative-design`

本文定义 Novadraw 输入、回调、变更、更新和渲染的运行时时序。所有流程按实际发生
顺序自上而下描述。

## 1. 顶层事务

Runtime 只允许一个顶层事务处于执行状态：

```text
接收命名操作或平台无关输入
→ 校验输入
→ 执行领域协议
→ 按因果顺序提交 effects
→ 提交结构 mutations
→ 收集 invalid / dirty
→ 必要时请求下一帧
```

事务执行期间：

- FigureTree 拓扑对当前遍历保持稳定；
- 回调不能递归启动另一个顶层事务；
- 新输入可以由 PlatformHost 排队，但不能重入；
- panic 或错误不得让 Runtime 永久停留在 updating/dispatching 状态。

## 2. 平台输入

```text
Native Event
→ PlatformInputAdapter
→ InputEvent in logical units
→ Runtime.dispatch_input
```

PlatformInputAdapter 只负责：

- native key/button/pointer 标识映射；
- physical pixels 到 logical units；
- IME、wheel、pinch 和 pointer phase 规范化；
- 平台时间戳转换；
- 拒绝 NaN、Infinity 和非法 scale factor。

它不执行 Figure hit-test，不持有 focus/capture，也不调用 Figure。

## 3. Pointer 分发

```text
InputEvent
→ EventDispatcher
→ read InteractionState
→ captured target 或 FigureTree hit-test
→ 更新 hover/target 状态
→ 将事件点转换到 target local domain
→ 调用 target 一次
→ 提交 callback effects
→ 更新 capture/focus 状态
```

### 3.1 Target 规则

1. 若 pointer 已 capture，主事件发给 captured Figure；
2. 否则从 root 按逆 Z-order 命中；
3. invisible 节点不绘制且不命中；
4. disabled 节点可以显示和参与布局，但不能成为默认输入 target；
5. children descent 使用 parent 的 client clip 和 child transform；
6. Figure 的精确 `hit_test` 在自身 local domain 执行；
7. 普通事件只回调一个 target，不执行 DOM 式冒泡。

### 3.2 Hover

target 变化时按以下顺序：

```text
old target MouseExited
→ 提交 old callback effects
→ InteractionState.target = new target
→ new target MouseEntered
→ 提交 new callback effects
→ 分发主事件
```

若协议需要完整 ancestor enter/exit 路径，必须另行定义，不得通过通用冒泡隐式加入。

### 3.3 Capture

capture 是 pointer 状态，不是事件传播阶段：

```text
PointerDown handled
→ request_capture(pointer, target)
→ subsequent move/up routed to target
→ PointerUp or Cancel
→ release_capture
→ recompute hover target
```

多 pointer 分别维护 capture。删除 captured Figure 时，Runtime 发送可选 cancel，
随后清除引用。

## 4. Figure 回调与 Effect

调用 Figure 时，Runtime 已经可变借用了目标 Figure，因此回调上下文不能再次借用
整个 Runtime。统一采用 effect recording：

```text
borrow target Figure
→ handler(event, EventContext)
→ append Effect in occurrence order
→ release Figure borrow
→ Runtime applies effects
```

Effect 分类：

| 类别 | 示例 | 提交时机 |
|---|---|---|
| 节点状态 | set bounds、visible、enabled、style | 当前 callback 返回后 |
| 更新请求 | repaint、invalidate | 对应状态 effect 之后 |
| 交互请求 | focus、capture、cursor、IME | 当前 callback 返回后 |
| 结构 mutation | add、remove、reparent、reorder | 顶层分发结束后 |
| 应用消息 | command、selection request | Runtime 事务结束时输出 |

Effect 严格保持产生顺序。Runtime 可以合并 repaint 区域，但不能改变有可观察差异的
状态顺序。

## 5. 结构 Mutation

```text
完成全部 callback
→ freeze MutationQueue
→ validate all referenced FigureId
→ apply mutations FIFO
→ invoke detach/attach lifecycle effects
→ repair InteractionState references
→ invalidate affected layout roots
→ damage old and new visual bounds
```

每个 mutation 必须是原子的。失败时：

- 不留下单边 parent/children 关系；
- 不保留属于旧 parent 的 layout constraint；
- 不产生只覆盖一半操作的通知；
- 返回结构化错误并继续维持 Runtime 可用状态。

不按 mutation 类型重新排序。调用者产生的 FIFO 顺序就是语义顺序；需要复合原子操作
时应使用单个 `Reparent`、`ReplaceContents` 等高层 mutation。

## 6. 几何变更

`set_bounds` 是一个原子 effect：

```text
capture old projected visual bounds
→ set new parent-local bounds
→ invalidate own/parent layout as required
→ invalidate transform cache for subtree
→ emit one geometry notification
→ damage union(old visual bounds, new visual bounds)
```

移动和 resize 不能拆成两次外部可观察通知。坐标根或 child transform 改变时，不改写
后代 bounds，而是失效子树的 world-transform 和 projected-bounds cache。

## 7. Validation

```text
freeze current invalid generation
→ reduce nodes to minimal validation roots
→ validate parent before affected descendants
→ measure/layout container
→ commit LayoutOutput atomically
→ process invalidations generated during validation
→ stop when stable
```

规则：

- invisible 节点是否跳过昂贵测量可以是策略，但不能丢失 invalid 状态；
- disabled 不影响 validation；
- valid 标志只表示相关几何和布局缓存对应当前 generation；
- Runtime 先构造 `LayoutSnapshot`，布局器据此计算并通过 `LayoutOutput` 提交，
  不同时借用布局器和整棵树，也不在遍历中任意改树；
- 同一节点的重复 invalidation 必须去重。

为防止错误 Layout/Figure 无限失效，UpdateManager 必须有诊断性收敛保护。超过可配置
事务预算时返回 `NonConvergingValidation`，保留待处理工作并请求后续诊断；不能静默
丢弃，也不能永久挂住 UI 线程。

## 8. Damage Repair

Validation 稳定后才计算最终 damage：

```text
freeze dirty sources
→ map each local dirty region through world transform
→ intersect ancestor clips
→ include filter/effect expansion
→ merge into logical Damage
```

Damage 至少表达：

```rust
pub enum Damage {
    None,
    Full,
    Partial(DamageRegions),
}
```

规范要求：

- 旧位置与新位置都必须覆盖；
- 坐标根、scroll、scale 和投影视觉效果必须使用统一变换；
- blur、shadow、stroke 等效果需要扩张视觉 bounds；
- `Partial` 的 regions 是优化信息；
- 后端可以选择全帧重绘，只要提交后的可见结果正确。

## 9. Paint Recording

绘制协议固定为：

```text
resolve inherited style
→ push node state
→ apply local-to-parent transform
→ paint Figure self
→ restore style while retaining required geometry state
→ clip to client area according to child clip policy
→ paint visible children in forward Z-order
→ paint border/foreground
→ pop node state
```

具体状态栈细节由 RecordingCanvas 定义，但必须保证：

- child 的状态变更不会泄漏给 sibling；
- paint 顺序和 hit-test 顺序互为正序/逆序；
- client area、clip 和坐标链与命中测试一致；
- Figure 只能录制平台无关命令。

## 10. Frame 提交

```text
UpdateManager produces stable scene state
→ RecordingCanvas records CommandStream
→ Runtime builds RenderSubmission
→ RenderBackend.submit
→ backend presents or stores frame
→ Runtime notifies frame observers
```

`RenderSubmission` 可以包含：

- command stream；
- logical damage；
- resource additions/removals；
- logical and physical surface information；
- frame sequence number。

是否序列化为二进制、是否增量 patch command stream，不属于核心事务。

## 11. Redraw 调度

```text
Runtime pending work changes empty → non-empty
→ PlatformHost.request_redraw()
→ platform coalesces requests
→ redraw callback
→ Runtime.prepare_frame()
→ RenderBackend.submit()
```

UpdateManager 不调用平台 API。PlatformHost 不执行 Figure layout 或事件分发。

当 redraw 到来但没有工作时，Runtime 可以返回 `NoFrame`。surface resize、恢复或
后端内容丢失必须提升为 `Damage::Full`。

## 12. Scroll 与 Zoom

```text
gesture Begin
→ hit-test and pin target
→ target callback once
→ if unhandled, find nearest typed controller
→ apply RangeModel or scale transaction
→ invalidate transforms/layout/damage
```

Update、End 和 Cancel 使用固定 session target。typed fallback 不是普通 ancestor
冒泡。详细契约见
[scroll-zoom-gesture-contract.md](../input/scroll-zoom-gesture-contract.md)。

## 13. Resize 与 DPI

```text
platform surface changed
→ PlatformAdapter produces SurfaceChanged
→ Runtime updates logical viewport
→ root bounds/layout invalidated
→ full logical damage
→ RenderBackend.resize physical surface
→ request redraw
```

Surface resize 只改变 logical viewport 的可见范围，不改变 Figure 的世界坐标或
ScalablePane 的 scale。fit width、fit height 和 fit all 只能由显式
`ZoomManager` 操作触发；平台 resize 不得隐式调用这些操作。

连续 resize 事件只保留最新 physical size。RenderBackend 在下一次 frame submission
开始前原位配置既有 surface，并在同一帧重建尺寸相关纹理；禁止为每个 resize 事件
销毁和重建平台 surface。macOS 的 Metal presentation layer 必须保持上一帧的左上
逻辑原点（对应 Core Animation 的 bottom-left native gravity），不得在新尺寸帧
present 前缩放旧 drawable。

DPI 改变可能同时改变 physical size 和 logical scale。Runtime 使用 logical units，
RenderBackend 在提交边界接收 scale factor。

## 14. 异步资源

```text
Figure references ResourceHandle
→ ResourceRegistry reports Pending
→ paint uses deterministic fallback
→ worker resolves resource
→ completion message enters Runtime
→ resource generation changes
→ dependent Figures repaint/revalidate
```

worker 不直接修改 FigureTree。资源完成消息与输入一样，经顶层事务顺序处理。

## 15. 2.5D 合成

```text
2D layout and paint
→ optional Projective3D composition
→ project visual bounds
→ backend composition
```

项目必须为每种 projective effect 明确：

- 是否只影响视觉，还是也影响 hit-test；
- projected clip 和 damage；
- 不可逆矩阵的处理；
- backend 不支持时是降级、软件回退还是显式错误。

真正 3D 场景使用独立 camera/ray/depth 流程，不进入本文的二维 Figure 事务。

## 16. 错误与恢复

| 失败 | 必须行为 |
|---|---|
| 无效 FigureId | mutation 失败，不部分提交 |
| 树环 | 拒绝操作 |
| constraint 类型错误 | 返回布局错误，保留旧 constraint |
| 不可逆 transform | 跳过相关命中并记录诊断 |
| validation 不收敛 | 返回诊断错误并保留工作 |
| backend submit 失败 | 保留 full repaint 需求以便恢复 |
| surface lost | 暂停提交，恢复后 full repaint |
| callback panic | 恢复事务标志；是否隔离 panic 由嵌入策略决定 |

## 17. 因果可观测性

调试和测试输出应按发生顺序记录：

```text
input
→ target selection
→ callback
→ effects
→ mutations
→ validation
→ damage
→ submission
```

通知队列不能逆序展示，也不能为了批处理打乱不同语义事件的因果关系。
