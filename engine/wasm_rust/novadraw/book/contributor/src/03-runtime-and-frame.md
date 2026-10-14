# 3. Runtime 事务与稳定帧

> **本章解决的问题**：一次输入或状态修改如何安全地收敛为可提交帧，以及修改执行链时
> 必须保护哪些阶段边界。

## 3.1 Runtime 是唯一提交权威

`Runtime` 同时协调：

- Figure 树与通用节点状态；
- 布局和派生状态失效；
- 重绘区域；
- 输入分发产生的延迟效果；
- RenderSubmission 的准备和完成反馈；
- 对外通知的稳定发布。

外部调用者只能通过短生命周期 facade 提交意图。具体 Figure、事件回调、布局器和
路由器都不能直接同时修改这些子系统。

## 3.2 一次变化的因果顺序

```text
输入或应用修改
-> prepare 并校验
-> 提交场景源状态
-> 形成已提交场景状态
-> 标记布局与派生状态失效
-> 收敛到稳定场景状态
-> 计算重绘区域
-> 录制绘制命令
-> 生成 RenderSubmission
-> Backend 提交
-> Runtime 接收完成反馈
-> 发布稳定通知
```

修改任何阶段时，都要检查前后阶段是否仍然只读取它们承诺的状态版本。

## 3.3 Prepare、Validate、Commit

可能失败的复杂修改应先准备完整候选结果：

1. **Prepare**：读取稳定快照，运行扩展代码并产生候选变化；
2. **Validate**：检查身份、拓扑、数值、坐标域和跨对象约束；
3. **Commit**：以不可失败或最小失败面的步骤更新权威状态；
4. **Publish**：稳定后再发布通知或对外可见结果。

扩展回调不得在 prepare 阶段持有 Runtime 可变引用。无法安全补偿的提交失败会使
Runtime 进入故障锁定状态，而不是继续暴露可能不一致的场景。

## 3.4 Query 只暴露当前阶段的数据项

Context 和 Query 不拥有 Runtime 状态。`EventSceneQuery` 读取 callback 开始时的已提交
场景状态，`StableSceneQuery` 读取最新 stable epoch 的稳定场景状态。它们只返回当前
阶段允许的场景数据项，不返回 Runtime、FigureTree 或可变 Figure 引用。

Effects、Output 和 Commands 承载候选变化或延迟意图。Query 不能隐式提交、flush 或
stabilize，也不能观察当前 callback 尚未提交的 effect。

## 3.5 稳定场景状态与提交完成不同

稳定场景状态表示布局和派生状态已经收敛，不代表 GPU 已经消费本帧。Runtime 必须保留
session 与 frame 身份，宿主将后端结果交回完成入口后，才能推进相关资源和提交状态。

`Suspended` 只表示 surface 当前不可呈现。Runtime 仍管理已提交场景状态和待处理工作，
但该结果本身不证明最新派生工作已经稳定。

需要核验的公开合同：

- [`ADR-018：Runtime 驱动与测量 API`](../../../doc/adr/adr-018-runtime-driving-and-measurement-api.md)
- [`ADR-025：统一 Graphics 与 glyph 预处理`](../../../doc/adr/adr-025-unified-graphics-and-glyph-preparation.md)
- [`Runtime 实现`](../../../novadraw/src/runtime/runtime.rs)

## 3.6 重绘区域必须覆盖新旧状态

几何或视觉变化可能同时影响旧位置和新位置。提交修改前应保存旧视觉边界，提交后计算
新视觉边界，再由 UpdateManager 变换到逻辑表面域并合并 damage。

只重绘新位置会留下旧像素；只依赖 Figure 的逻辑 bounds 会漏掉边框、装饰、连接或
变换后的视觉范围。

## 3.7 修改执行链时的门禁

- 不在渲染主循环打印逐节点或逐命令日志；
- 不绕过稳定化直接录制命令；
- 不在绘制、测量或路由的纯计算阶段提交结构变化；
- 不把后端对象写入 Core Figure；
- 不把提交成功、可重试和致命失败压缩成无法恢复的布尔值；
- 保持递归树遍历深度上限 10,000。

`novadraw/src/graph/render_recursive.rs` 是受保护的渲染主流程。除非语义证据证明主流程
错误，否则应先检查 Figure、坐标、命令录制、后端 lowering 和调用路径。
