# 5. Editor 投影与编辑协议

> **本章解决的问题**：修改 Editor 时，如何保持模型事实、运行时投影、交互会话和命令
> 历史之间的边界。

## 5.1 Editor 不拥有业务事实

业务模型是唯一持久事实源。Editor 维护的是可重建投影：

```text
ModelId
-> EditPartId
-> FigureId
```

`ModelAdapter` 暴露稳定快照、修订和有序事件；`GraphicalViewer` 创建并刷新 Part 与
Figure；`EditorDomain` 协调活动工具和命令历史。Figure 不能反向成为业务状态来源。

## 5.2 投影先计划，再发布

一次刷新应遵循：

```text
排空模型事件
-> 校验 revision 连续
-> 捕获稳定模型快照
-> 计算退休、换父、创建和连接计划
-> 校验全部候选结构
-> 更新 PartTree、registry 与 Runtime
-> 发布已应用 revision
```

模型快照读取期间发生版本漂移时必须放弃计划。扩展失败后若无法证明内部结构完整，
Viewer 进入故障锁定，而不是继续接受编辑。

核心入口：

- [`Editor 架构`](../../../doc/design/editor/architecture.md)
- [`GraphicalViewer`](../../../novadraw-editor/src/viewer/mod.rs)
- [`投影实现`](../../../novadraw-editor/src/viewer/projection.rs)

## 5.3 编辑协议

标准编辑链是：

```text
Input
-> Tool
-> typed Request
-> EditPolicy
-> Command
-> CommandStack
-> Model
-> Model events
-> Viewer refresh
```

- Tool 保存跨事件手势状态；
- Request 只表达意图，不修改模型；
- Policy 解释请求并生成反馈或命令；
- Command 只保存模型身份和业务值；
- CommandStack 负责执行、撤销、重做和保存位置；
- Viewer 根据模型变化刷新投影。

修改其中任何一层，都要防止 Figure 与模型双写。

## 5.4 临时反馈与稳定投影

拖拽轮廓、连接预览、操作手柄、选择框和文本编辑草稿属于临时反馈。它们可以读取模型
和场景，但不能提前写入业务模型。

手势完成时先清理反馈，再执行模型命令并刷新稳定投影。取消、失焦、节点退休、指针
离开和 Viewer fault 都必须有显式清理路径。

## 5.5 命令失败语义

普通命令错误应保证模型和命令仍处于调用前状态。复合命令执行失败时逆序补偿已成功
前缀；补偿失败或 panic 会使 CommandStack 故障锁定。

命令不得保存：

- `EditPartId`；
- `FigureId`；
- `ConnectionId` 或 `AnchorId`；
- Viewer、Runtime 或 Figure 的长期引用。

## 5.6 修改 Editor 的评审问题

- 新状态属于模型、Viewer、EditorDomain、Tool 还是 Feedback？
- 输入是否先尊重 Core 的处理和捕获结果？
- 活动手势是否锁定来源、目标和版本？
- 结构创建失败是否可能留下半发布 registry？
- 撤销重做是否只依赖模型身份？
- 视口变化后反馈能否在同一逻辑表面指针位置重新投影？
- 新 Tool、Request 或 Handle 是否要求修改中心枚举？若是，应先评估扩展协议。
