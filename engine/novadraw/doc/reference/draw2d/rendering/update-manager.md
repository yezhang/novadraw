# Draw2D 更新与渲染管线

类型：`reference-analysis`

源码仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`。
基线：`4463d9d0c`（2026-01-01）。范围限 `org.eclipse.draw2d`。
本文定义参考事实；Novadraw 契约见
[UpdateManager 规范](../../../design/rendering/update-manager.md)。

## 宿主入口与所有者

| 对象 | 职责与稳定源码入口 |
|---|---|
| `LightweightSystem` | root/contents、manager、dispatcher 与 SWT Canvas 桥接；`setControl`、`addListeners`、`controlResized`、`paint` |
| `UpdateManager` | 抽象扩展点；`addDirtyRegion`、`addInvalidFigure`、`performUpdate`、`performValidation` |
| `DeferredUpdateManager` | 默认批处理实现；invalidFigures、dirtyRegions、damage、afterUpdate、updateQueued/updating/validating |
| `GraphicsSource` | `getGraphics`、flush 与释放；BufferedGraphicsSource 用离屏图像，NativeGraphicsSource 直接绘制 |
| `Figure` | 可覆盖的 paint 模板、子树遍历、布局与 bounds 协议 |
| `SWTGraphics` | 逻辑状态、状态栈和底层 GC 延迟同步 |
| `SWTEventDispatcher` | 输入目标、capture、focus 与实际事件分发 |

`LightweightSystem.setRootPaneFigure` 同时把 root 交给 UpdateManager。
`setControl` 根据 SWT.DOUBLE_BUFFERED 选择 GraphicsSource，并注册 paint、resize、
鼠标、键盘、焦点、遍历、dispose 与 accessibility 监听。
`controlResized` 同步 root bounds、revalidate，再 performUpdate。
`LightweightSystem.paint(GC)` 委托 manager，不直接遍历 Figure。

`UpdateManager` 是公开抽象扩展点；SubordinateUpdateManager 是已 deprecated 的抽象类。
内置类列表不能证明 DeferredUpdateManager 是唯一可能实现。

## 正常更新事务

`DeferredUpdateManager.performUpdate()` 按顺序执行：

```text
disposed/updating → 拒绝重入
设置 updating
→ performValidation
→ 清除 updateQueued
→ repairDamage
→ swap afterUpdate 并执行回调
→ 回调追加新任务时 queueWork
→ finally 清除 updating
```

`performUpdate(Rectangle exposed)` 先把 exposed 加入 root dirty region，再复用正常事务。
`queueWork` 用 updateQueued 去重；refreshRate ≤ 0 时 Display.asyncExec，
否则 Display.timerExec。它合并同一 UI 事件循环内的多次变化。

## Validation

`performValidation` 在 invalidFigures 为空或 validating 时返回，否则：

1. 设置 validating，发 `fireValidating()`；
2. 按索引读取 invalidFigures，当前槽先置 null，再调用 `fig.validate()`；
3. 循环读取当前 size，因此验证中追加的 invalid Figure 可继续被处理；
4. finally 清空列表并解除 validating。

这里是列表原地置空；dirty region 和 afterUpdate 才采用容器 swap。
三者不能统一描述成“所有队列双缓冲”。
Figure 的 validate 执行布局并递归验证 children；invalidate/revalidate 的差异见
[Figure 实现](../figure/figure-implementation.md)。

## Damage Repair

`repairDamage` 先把 dirtyRegions 移交为 oldRegions，再创建新 map：

```text
每个旧 contribution
→ 与 figure bounds 求交
→ 沿父链 translateToParent
→ 与每层 ancestor bounds 求交
→ union 到 damage
→ oldRegions 非空时 firePainting
→ damage 非空时取得 Graphics，root.paint，释放 Graphics
→ 清除本轮 damage
```

遍历期间新增 dirty region 写入新 map，不干扰当前前缀。
祖先传播使用该祖先公开坐标协议，不等价于无条件累加所有 bounds。
GraphicsSource 负责实际目标与缓冲刷新；它不是 Figure 树或布局的所有者。

## SWT paint 与延迟更新不同

`DeferredUpdateManager.paint(GC)` 有独立分支：

- validating 时把 GC clipping 加入 root dirty region，延后处理；
- 否则构造 SWTGraphics；
- 当前不在 updating 中时，用 GC clip 发 Painting；
- performValidation，然后 root.paint；
- 释放这次 SWTGraphics。

因此不能声称“所有 Painting 回调必定在验证之后”。正常 performUpdate 中 validation
先于 repair，直接 SWT paint 入口的通知顺序由上述分支决定。

## Figure 默认绘制模板

`Figure.paint` 先安装本地 background/foreground/font，再：

```text
pushState
→ paintFigure
→ restoreState
→ paintClientArea
→ paintBorder
→ finally popState
```

`restoreState()` 恢复保存的完整状态，包括变换、clip、画刷、字体和绘图属性，
但不弹栈。`popState()` 恢复并减少栈深度。两者的差别是栈操作，不能解释为
“restore 只恢复颜色，pop 才恢复变换”。
本地样式在 push 前安装；兄弟间隔离还依赖父级 paintChildren 的 restore。

`paintClientArea` 没有 children 时返回：

- local-coordinate 分支 translate 到 bounds + insets；必要时 clip client area；
  push、paintChildren、pop，最后恢复外层状态；
- 非 local-coordinate 且 optimizeClip 为真时直接 paintChildren；
- 否则 clip client area，push、paintChildren、pop，再 restore。

`optimizeClip` 在没有 Border 或 Border opaque 时为真。
不能把默认实现简化为“每个分支无条件 clip client area”。

`paintChildren` 按 children 正序处理 visible child。每个 child 使用
IClippingStrategy 的 Rectangle 数组，未设置策略时使用 child bounds。
每个与当前 clip 相交的区域执行 clip、child.paint、restoreState。
恢复发生在每次区域绘制后，避免把 child1 的 clip 带给 child2。
多区域实际重放与 alpha 行为见 [Clip 分析](clip.md)。

Figure.paint、paintClientArea 和 paintChildren 可覆盖；默认模板不等于禁止其他合成顺序。
Graphics 属性与 GC 同步详见 [Graphics API](graphics-api.md)，不在本页再展开状态字段全表。

## 输入事务

`SWTEventDispatcher.receive` 先更新物理指针下 Figure，再决定 mouseTarget：

```text
captured → 保留 mouseTarget，构造本次事件
未 capture → findMouseEventTargetAt
→ target 相同时保留
→ 旧 target exited
→ 设置新 target
→ 新 target entered
```

主事件随后分发：

| 事件 | 顺序 |
|---|---|
| press | receive → handler；consumed 后建立 capture |
| release | receive → handler → releaseCapture → 再 receive，恢复 hover/target |
| move | receive → 按 BUTTON_MASK 选择 dragged 或 moved |
| key | 交给 focusOwner |
| traversal | FocusTraverseManager 选择下一焦点；详见树遍历事实 |
| wheel | receive → mouseTarget wheel callback |
| focus | 原 owner lost，再向新 owner gained |

capture 固定输入目标，物理指针下 Figure 仍可独立更新。
dragged 根据按键状态选择，不以 consumed/capture 作为唯一判据。
目标资格及逆序搜索见 [树操作](../figure/tree-operations.md)，
输入 listener 与对象通知的区别见 [通知对照](../../../parity/draw2d/notification-mapping.md)。

## 核验入口

在基线源码中核对以下 class/method：

- `LightweightSystem::{setControl,controlResized,paint}`；
- `DeferredUpdateManager::{performUpdate,performValidation,repairDamage,paint,queueWork}`；
- `Figure::{paint,paintClientArea,paintChildren,optimizeClip}`；
- `SWTGraphics::{pushState,restoreState,popState}`；
- `SWTEventDispatcher::{receive,dispatchMousePressed,dispatchMouseReleased,dispatchMouseMoved}`。

Novadraw 对这些事实的采用和主动差异分别见
[语义账本](../../../parity/draw2d/api-coverage.md)与
[动态架构](../../../design/architecture/dynamic-architecture.md)。
