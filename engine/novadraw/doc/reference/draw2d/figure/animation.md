# Draw2D Animation / Animator 源码事实

类型：`reference-analysis`

源码仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`

源码基线：`4463d9d0c`（2026-01-01）

范围：`org.eclipse.draw2d.Animation`、`Animator`、`LayoutAnimator`、
`RoutingAnimator`。本文只记录 Draw2D 事实，不直接定义 Novadraw 契约。

## 1. 总体模型

Draw2D 使用一次全局 animation session 协调多个 Animator：

```text
Animation.markBegin()
→ source mutation / invalidate
→ Animator 捕获 initial state
→ Animation.run()
→ UpdateManager.performValidation()
→ Animator 捕获 final state
→ 同步循环推进 progress
→ Animator 插值并覆盖布局或路由结果
→ UpdateManager.performUpdate()
→ cleanup / tearDown
```

`Animation` 保存三阶段全局状态：

- `RECORD_INITIAL`：监听 invalidation，登记参与动画的 Figure；
- `RECORD_FINAL`：强制 validation，捕获最终状态；
- `PLAYBACK`：按墙钟计算 progress，反复 revalidate 和 update。

`run(duration)` 同步阻塞，直到 progress 到达 1。默认 duration 为 250ms。

源码入口：

- `org.eclipse.draw2d/Animation.java`
- `org.eclipse.draw2d/Animator.java`

## 2. Animator 生命周期

`Animator` 的职责是捕获和播放某类 Figure 状态：

1. `init(figure)` 捕获 initial state；
2. `capture(figure)` 捕获 final state；
3. `playbackStarting(figure)` 可对两端状态做归一；
4. `playback(figure)` 根据全局 progress 写入中间状态；
5. `tearDown(figure)` 恢复临时设置。

Animator 通常无状态并可被多个 Figure 共享；每个 Figure 的 initial/final state
由 `Animation` 的全局 map 保存。Draw2D 通过 Figure/Layout/Connection listener
把 Animator 接入 validation。

## 3. LayoutAnimator

`LayoutAnimator` 同时实现 `LayoutListener`：

- `invalidate(container)` 在 initial recording 阶段登记 animator；
- `postLayout(container)` 在 final recording 阶段标记需要捕获；
- initial/final state 是 container 全部 child 的 bounds 副本；
- playback 对 child 的 x、y、width、height 做线性插值；
- playback 直接调用 child `setBounds`，以中间几何替代正常布局结果。

它只跟踪 LayoutManager 产生的 child placement。手工 `setBounds` 变化不保证被正确捕获。

源码入口：`org.eclipse.draw2d/LayoutAnimator.java`。

## 4. RoutingAnimator

`RoutingAnimator` 同时实现 `RoutingListener`：

- `invalidate(connection)` 在 initial recording 阶段登记 animator；
- `postRoute(connection)` 在 final recording 阶段标记需要捕获；
- initial/final state 是 Connection PointList 副本；
- playback 对对应 point 做线性插值，并直接替换 Connection points；
- 两端 point 数不同会在 playback 开始前插入中间点，使列表长度一致；
- 没有 initial route 时，playback 暂时隐藏 Connection。

源码入口：`org.eclipse.draw2d/RoutingAnimator.java`。

## 5. 可观察语义

Draw2D 动画提供以下长期可复用语义：

1. 动画是可选能力；未安装 Animator 时，布局和路由保持正常行为。
2. initial/final state 来自 source mutation 前后的稳定状态。
3. Layout 与 Routing 是动画系统的领域消费者，不各自拥有时钟。
4. 多个 Figure 可以加入同一 animation session。
5. 动画结束必须清理临时状态。
6. 播放期间仍通过 UpdateManager 执行 update。

## 6. 不应照搬的实现约束

以下是 Draw2D/SWT 时代实现选择，不应成为 Novadraw 目标：

- `Animation` 使用静态全局可变状态；
- 默认 Animator 使用 Singleton；
- `run()` 同步阻塞调用线程；
- 直接读取系统墙钟；
- 每帧修改 Figure source bounds 或 Connection route points；
- Layout/Route callback 通过全局阶段值改变正常协议；
- 无 Runtime namespace、结构化取消、reduced-motion 或 suspend/resume 合同；
- point 数量归一只按索引插点，不声明弧长或路径拓扑语义。

Novadraw 应保留可观察目标，使用 Runtime-owned、显式时钟、source/presentation 分离的
协议表达。

## 7. 对 Novadraw 设计的输入

Draw2D 事实支持以下设计输入，但不直接决定具体 Rust API：

- Runtime 应拥有 animation session 与当前 presentation state；
- Host 应只注入单调时间和 wakeup/redraw；
- source mutation 应立即提交最终业务状态；
- animation track 只生成可丢弃的表现状态；
- layout、route、viewport、property 和 lifecycle 使用同一 Timeline/Track 基础；
- 动画禁用时必须退化为普通、立即完成的既有事务；
- 临时鼓包、exit snapshot 等表现对象不应伪装成 live Figure。
