# ADR-026: 动画正交模型与 Presentation Plane

类型：`architecture-decision`

## 状态

已接受。2026-10-06 用户确认继续按本方案推进动画与能力丰富度建设。

目标 delta：P2-M01。

## 背景

Draw2D 提供 Animation、Animator、LayoutAnimator 与 RoutingAnimator，通过捕获
validation 前后的状态并同步插值实现动画。它证明布局与路由可以共享动画会话，但其
静态全局状态、Singleton animator、阻塞播放和每帧 source mutation 不符合 Novadraw
的 Runtime 所有权、跨平台驱动与无全局状态约束。

Novadraw 后续内置控件、图布局、Connection、Viewport、Thumbnail 和反馈能力需要共享
动画基础；应用同时必须能够完全禁用动画，且不承担持续 tick 或行为差异。

## 决策提案

接受
[Animation / Presentation Plane 设计提案](../design/animation/animation-system.md)：

1. 动画由 Target、Motion、Trigger、Composition 四个正交维度组成，不为每种组合建立
   独立类层次。
2. Runtime 独占 AnimationService；Host 只通过现有单调时间入口驱动，不创建第二个
   run loop、全局 clock 或 Figure 私有 timer。
3. source/committed state 立即到达最终业务状态；animation 只生成可丢弃的
   presentation state，不成为布局、路由、模型、通知或 history 真值。
4. Timeline/Track 统一服务属性、layout result、Connection route、lifecycle、
   viewport 和持续程序动画。
5. Core 默认不安装隐式 Trigger。只有应用显式启动 plan 或安装 Behavior/Transition
   policy 才执行动画。
6. 提供 Disabled、Enabled、ReducedMotion；Disabled 直接显示最终 committed state，
   无 active timeline 时不持续 tick、request redraw、扫描 FigureTree 或提交空帧。
7. 第三方扩展使用 typed value 或 prepared FigurePresentation；不使用字符串 property
   path、核心枚举穷举或无类型 payload。
8. 每个 sampled effect 提供 old/new visual envelope，并进入既有 damage、recording
   与 RenderSubmission 合同。
9. temporary visual 默认不参与 hit-test 或 accessibility；可交互动画必须显式声明
   presentation geometry 策略。
10. 首批领域消费者包括属性、layout result、Connection route、Viewport、dash flow
    与 moving pulse → endpoint decoration。
11. layout、route 与 lifecycle transition 使用 Runtime-scoped、one-shot 的显式
    before capture；不从 mutation 后状态猜测旧 geometry，也不承诺 source rollback。
12. 独立 cancel 清除 presentation override 并显示 committed final state；从当前表现值
    继续只能通过预检后原子 retarget，失败时旧 plan 原样继续。
13. 属性、布局、路由、视口、生命周期、持续效果和后续组件只通过同一组正交维度与
    presentation channel 接入，不建立领域私有 clock、timeline 或 cancel 语义。

## 取舍

### 正面

- 后续能力复用一套时钟、生命周期、取消和 damage 语义；
- source state 与动画表现隔离，禁用动画不改变业务结果；
- Headless 可以用固定时间确定性重放；
- 外部 Figure 可扩展表现而不修改 Core 枚举；
- 为未来 backend compositor 优化保留空间，但不泄露后端类型。

### 代价

- Runtime 增加 active timeline、presentation override 和 temporary visual 状态；
- hit-test/accessibility 与 presentation geometry 需要显式策略；
- route topology、path morph、reparent/shared element 不能自动获得通用插值；
- 持续动画会持续请求帧，必须有可见性、预算和 reduced-motion 约束。

## 被拒绝的替代方案

1. **复制 Draw2D Animation 静态工具**
   - 违反无全局状态和多 Runtime 隔离；
   - 阻塞 run loop 不适配 Native/Web。
2. **每个 Figure 自己持有 timer**
   - 产生多个时钟、不可测试 wakeup 和生命周期泄漏。
3. **每帧修改 source bounds/route/style**
   - 污染通知、history、layout/routing 和 damage 因果。
4. **只有 backend shader animation**
   - 无法统一 layout、route、viewport、取消和 Headless 语义。
5. **字符串 property path**
   - 缺少 Rust 类型校验和外部扩展边界。

## 失败边界

- 非单调时间、非法参数、foreign/disposed target、channel 冲突在 admission 或
  reconcile 边界结构化拒绝；
- animation 失败不回滚已提交 source transaction，清除 override 后显示最终状态；
- extension panic 沿 ADR-014 Runtime fault 边界；
- backend 不支持的 presentation capability 在提交前拒绝或使用显式 fallback；
- budget 耗尽只拒绝新 plan，不破坏无关 active plan；
- start/transition admission 失败不创建 ID、owner、override、wakeup 或 damage；
- retarget 失败保留旧 plan，cancel 不留下无 owner 的 presentation override。

## 关系

- 扩展 ADR-018 的 Runtime 帧驱动边界；
- 复用 Tooltip/Accessibility 已建立的 Host monotonic time；
- 保持 ADR-011 derived-state convergence：动画采样不反向生成 source work；
- 保持 ADR-014 extension panic 与生命周期边界；
- 复用 ADR-024 的 Paint/Stroke/dash 与 damage；
- 复用 ADR-025 的 FigurePresentation、Graphics 与 RecordedDrawing；
- 公开调用、错误与状态机形状见
  [Animation 公开 API 合同提案](../design/animation/public-api-contract.md)；
- 领域消费者与能力丰富度接入见
  [Animation 领域能力集成矩阵](../design/animation/capability-integration.md)；
- 不改变 `render_recursive.rs` 的递归 paint 协议。

`api_semantics`：`animation.timeline`、`animation.presentation`、
`frame.preparation`、`damage.repaint`。

## 验证

目标 suite：`core.p2-m01-animation`。
架构检查见
[P2-M01 Animation 架构评审记录](../verification/reviews/p2-m01-animation-architecture-review.md)。

实现前必须完成：

- design/ADR、public API 与错误枚举人工评审：已完成；
- suite manifest 登记；
- 外部 typed animation consumer 设计；
- no-animation zero-work 基线。

## 日期

2026-10-06
