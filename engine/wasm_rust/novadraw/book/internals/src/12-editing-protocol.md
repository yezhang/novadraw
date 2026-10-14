# 12. Tool、Request、Policy 与 Command

> **本章解决的问题**：连续、可取消的平台输入，如何转化为可验证、可撤销的业务模型
> 修改？

拖拽过程包含数十个 move 事件，但用户通常期望一次撤销恢复整个操作。连接预览可以
不断变化，却不能提前创建业务连接。要满足这些要求，输入、意图、反馈和提交必须分层。

核心结论是：

> 输入不是命令，反馈不是模型，Figure 变化也不是业务提交；四者必须由明确协议连接。

## 12.1 四个角色

### Tool

Tool 是手势状态机。它实现 object-safe `Tool<A>`，解释事件序列，持有来源目标、初始
位置、当前阶段和模型 revision。`EditorDomain` 只保存 `Box<dyn Tool<A>>`，内置与外部
Tool 经同一生命周期运行。Tool 只能通过窄 `ToolContext` 访问目标、分派请求和提交命令。

### Request

Request 是平台无关的编辑意图，例如“把这些对象移动到候选位置”或“尝试连接到该目标”。
它不包含鼠标按钮等设备细节。每个 request family 由 `RequestProtocol<A>` marker、
`RequestKey<R>`、typed payload 和 contribution 组成；公共 API 不暴露 `Any`。

### Policy

Policy 在目标上下文中解释 Request，判断是否允许，生成反馈和候选 Command。它通过
`PolicyKey<S>` 安装，并由 `PolicyHandlerBuilder` 注册 typed handler；存在 handler
本身就表示理解该 request，不再维护独立的 `understands` 分支。

### Command

Command 描述对业务模型的最终改变，支持执行、撤销和重做。它不操作 Figure。

```mermaid
flowchart LR
    Input[归一化输入] --> Tool
    Tool --> Request
    Request --> Policy
    Policy --> Feedback[临时反馈]
    Policy --> Command
    Command --> Domain[EditorDomain]
    Domain --> Session[ViewerEditSession]
    Session --> Stack[CommandStack]
    Stack --> Model[业务模型]
    Model --> Viewer[确定性投影]
    Viewer --> Figure[Figure 树]
```

## 12.2 Tool 是有版本的状态机

Tool 开始会话时应锁定：

- source Part/Model 身份；
- 起始 model revision；
- pointer 或 focus identity；
- 初始表面位置；
- 当前模式和修饰键语义。

后续事件先验证会话仍有效。模型删除目标、工具切换或 capture 丢失时，应取消，而不是
继续把新事件应用到旧假设。

## 12.3 Request 的设计

好的 Request 表达“想做什么”，而不是“如何修改”：

```text
MoveRequest {
    subjects: [ModelId],
    candidate_delta: Vector,
    phase: Preview | Commit,
}
```

Request 应：

- 使用稳定业务或 Part 身份；
- 明确坐标域；
- 包含解释意图所需上下文；
- 不携带内部可变引用；
- 可在测试中直接构造。

所有请求共享的 interaction revision 与 source parts 位于 `RequestMetadata`，其余字段
只属于具体 payload。内置 request marker 与外部 marker 使用同一个 registry 和 reducer，
不存在闭合 request enum。

若 Request 直接包含“把 Figure bounds 改成某值”，它已经越过模型和 Policy 边界。

## 12.4 Policy 的职责

Policy 回答：

- 目标是否支持该请求；
- 需要哪些模型对象共同参与；
- 当前约束是否允许；
- 预览应显示什么；
- 提交时贡献什么 Command。

多个 Policy 可以参与同一 Request，但组合规则必须稳定。例如父容器负责接纳，节点
负责移动约束，连接 Policy 负责相关业务关系。

Policy 不应执行 Command，也不应直接持久化模型。

## 12.5 Overlay、Handle 与 Feedback

覆盖视觉（Overlay）是 Viewer 在稳定 Part 视觉之上管理的临时 Figure。它把交互状态
画出来，但不把交互候选写入模型或稳定 Part 树。Overlay 主要分为 Handle 和 Feedback。

控制柄（Handle）是可命中的交互入口，例如缩放点、连接端点或折点控制点。它具有 Viewer
命名空间内的实例身份 `HandleId`。Handle 的实例身份与行为身份
`HandleActionKey<R>` 分离：每个 Handle 最多绑定一个 typed action，外部 action 可以
注册自己的 Tool factory，不需要修改 Editor。

反馈图形（Feedback）是候选结果的临时可视化，例如：

- 移动轮廓；
- 对齐线；
- 连接预览；
- 插入位置；
- 拒绝指示；
- 文本 preedit。

Feedback 具有 `FeedbackId`，可以关联 owner Part 以便定位和清理，但它不接收稳定业务
身份，也不成为 ConnectionPart。它由 Viewer 中具有匹配缩放语义的 feedback layer
持有，可在每次 move 后替换，并在完成或取消时清理。

```mermaid
flowchart TD
    Viewer --> PartVisual[稳定 Part 视觉]
    Viewer --> Overlay
    Overlay --> Handle[Handle<br/>可命中并启动 Tool]
    Overlay --> Feedback[Feedback<br/>只表达候选结果]
    Handle --> Action[typed HandleAction]
    Action --> Tool
    Tool --> Request
    Policy --> Feedback
```

Handle 与 Feedback 都是普通 Figure 子树，因此继续使用同一坐标转换、裁剪、damage 和
销毁协议。二者的差异在于交互语义：Handle 可以成为输入目标并启动动作；Feedback
通常只展示当前候选，不成为下一次业务命令的稳定目标。

Overlay 的典型生命周期按因果顺序发生：

1. selection 或 Tool 会话决定需要哪些临时视觉；
2. Viewer 创建 Handle 或 Feedback 身份；
3. Figure 挂入具有正确缩放语义的 overlay layer；
4. visual ownership registry 登记 Figure 与临时所有者；
5. 输入命中通过 registry 解析 Handle action 或 owner Part；
6. selection 变化、完成、取消或 Part 删除时，Viewer 注销并销毁整棵临时子树。

Feedback 不是模型，也不应被命令历史记录。若预览需要计算布局或坐标，应使用稳定场景
查询和候选数据，不能先改真实 Figure 再观察效果。

连接预览尤其不能提前创建稳定 ConnectionPart。它可以复用 Core 的连接 Figure 与路由
能力，但其所有者仍是 Feedback；只有最终 Command 创建业务连接后，Viewer 才在下一次
稳定投影中创建 ConnectionPart。

## 12.6 Command

Command 保存执行和撤销所需的业务身份和值：

```text
MoveNodes {
    nodes,
    before_positions,
    after_positions,
    expected_revision,
}
```

它应满足：

- 只修改模型权威；
- 可判断当前是否可执行；
- 执行后产生明确 revision；
- undo 恢复业务语义；
- redo 重放已确认意图；
- 不保存 Viewer 或 Runtime 私有身份。

Command 不负责刷新 Figure。模型变化发布后，Viewer 走第 11 章的投影流程。

## 12.7 移动案例

### 开始

1. pointer down 命中 Figure；
2. Viewer 解析到 Part 和 ModelId；
3. Tool 锁定选择集合与起始 revision；
4. 建立 capture。

### 预览

1. pointer move 转换为稳定坐标域中的 delta；
2. Tool 构造 `MoveRequest::Preview`；
3. Policy 应用网格、边界和父级规则；
4. 返回候选位置与反馈；
5. Viewer 替换反馈层内容。

### 提交

1. pointer up 形成 `MoveRequest::Commit`；
2. Policy 基于最终模型快照重新验证；
3. 生成一个 Move Command；
4. 清理反馈；
5. EditorDomain 在 ViewerEditSession 中执行 CommandStack 操作；
6. 会话校验模型通知并同步 Viewer 投影；
7. Viewer 显示新 revision 对应的 Figure。

整个拖拽只产生一个业务命令。

## 12.8 创建连接案例

连接工具开始于 source anchor 候选，移动时不断查询 target：

1. down 锁定 source ModelId 和 source role；
2. move 命中候选 target Part；
3. Policy 检查类型、方向、重复连接和业务约束；
4. Feedback 显示 resolved 或 rejected 预览；
5. up 时再次验证 source、target 和 revision；
6. Command 创建业务连接；
7. Viewer 投影新的 ConnectionPart；
8. Core 计算 anchor 和 route。

预览连接可以是 Figure，但它位于反馈层，不是最终业务连接的提前实例。

## 12.9 直接文本编辑案例

文本编辑包含 focus、IME 和业务提交：

1. Tool 进入 direct-edit 会话并锁定 ModelId；
2. 编辑控件维护临时字符串、选区和 preedit；
3. 模型原值保持不变；
4. commit text 更新临时编辑值；
5. 用户确认时 Policy 校验格式和业务规则；
6. 生成 ReplaceText Command；
7. ViewerEditSession 执行命令并校验新 revision；
8. Viewer 关闭编辑反馈并同步文本 Figure。

取消只丢弃临时编辑值，不需要执行反向模型命令。

## 12.10 编辑会话关闭命令模型事务

命令执行和 Viewer 刷新不能是两个依赖调用者记忆的公开步骤。EditorDomain 创建一个
短生命周期查看器编辑会话（`ViewerEditSession`），并在会话内完成 execute、undo 或
redo。

会话开始前，Viewer 先同步已经存在的外部通知。这样命令总是建立在已投影的稳定 revision
前缀上。会话只向一次 CommandStack 操作提供短期模型可变借用，并且必须以三种结论之一
关闭：

| 结论 | 适用情况 | 关闭动作 |
|---|---|---|
| commit | execute、undo 或 redo 成功 | 校验并消费通知，然后同步投影 |
| reject | 明确拒绝或可恢复错误 | 要求模型 revision 和待发布事件保持不变 |
| fault | panic、unknown-state 或不可恢复错误 | 不投影未知模型，使 Stack 和 Viewer 故障锁定 |

会话没有显式关闭就离开作用域，Viewer 也进入故障锁定状态。析构过程不猜测模型状态，
也不自动提交。

可恢复命令错误必须真正做到“不修改模型”。若 reject 路径观察到 revision 前进或新通知，
错误将升级为 Viewer 故障。这把原本依赖 Command 作者自律的合同变成了可执行门禁。

## 12.11 CommandStack

CommandStack 是命令历史的权威，负责：

- 执行；
- undo；
- redo；
- 保存点或 dirty 标记；
- 合并连续兼容命令；
- 阻止重入执行。

栈中保存的是成功提交的命令。prepare 失败或被拒绝的候选不能进入历史。

查询“是否存在历史”和调用扩展 Command 判断“当前是否允许撤销”是两种不同操作：

```text
has_undo / has_redo
    -> 只读查询 history 和 fault 状态

check_undo / check_redo
    -> 调用顶部 Command 的能力检查
    -> 返回允许、明确拒绝或结构化错误
```

`has_*` 不调用扩展代码，也不改变状态。`check_*` 若捕获扩展 panic，则返回
`CommandStackError` 并使 Stack 故障锁定；它不能把“无历史”“扩展拒绝”和“扩展故障”
压缩成同一个 `false`。实际 undo/redo 仍需在执行点重新检查。

## 12.12 CompoundCommand

一个业务意图可能包含多个原子子命令，例如移动节点并更新业务容器顺序。

CompoundCommand 需要：

1. 在执行前验证所有子命令；
2. 按稳定顺序执行；
3. 任一失败时按逆序补偿已执行部分，或由模型事务保证原子性；
4. undo 按执行的逆序进行；
5. redo 使用确定的原顺序。

如果无法证明补偿完整，就不应把多个独立副作用包装成“原子命令”。

## 12.13 版本冲突

Tool 开始后模型可能被其他来源修改。提交时应比较起始假设和当前 revision：

- 不相关变化可由 Policy 重新解释；
- 目标属性已改变时可以拒绝或重新计算；
- 目标被删除时取消；
- 不能静默覆盖新事实。

是否允许 rebase 由业务策略决定，但必须显式，不得由 Figure 当前位置猜测。

## 12.14 Selection 与操作对象

选择状态属于 Viewer，不等于模型属性。Tool 可以读取当前 selection 构造多对象请求，
但命令仍保存 ModelId 集合。

模型变化后，Viewer 按身份修复选择：

- 仍存在的对象保持选择；
- 删除对象移出集合；
- 新建对象是否选中由操作结果明确决定；
- 多 Viewer 的选择互不影响。

## 12.15 Policy 组合

当多个 Policy 贡献结果时，需要明确组合代数：

- 全部允许才执行；
- 任一拒绝终止；
- 多个 Command 按稳定顺序组成 CompoundCommand；
- Feedback 按语义层合并；
- 重复目标或冲突修改在 commit 前拒绝。

不能依赖注册顺序偶然决定谁覆盖谁。

## 12.16 取消路径

以下事件都应进入统一 cancel：

- Escape；
- pointer cancel；
- capture owner 丢失；
- Tool 切换；
- Viewer 销毁；
- model target 删除；
- revision 冲突不可重基；
- Runtime fault。

取消过程只清理会话、capture、feedback 和临时资源。若模型命令尚未执行，不能调用
undo 伪装成取消。

## 12.17 测试边界

五层可以分别测试：

- Tool：事件序列到 Request；
- Policy：Request 和快照到反馈/Command；
- Command：模型 before/after/undo/redo；
- ViewerEditSession：commit/reject/fault 与通知同步；
- Viewer：模型 revision 到 Figure 投影。

端到端测试再验证完整闭环。分层后，核心业务规则不需要窗口或 GPU 才能测试。

## 12.18 必须保持的不变量

1. Tool 锁定稳定目标和会话版本；
2. Request 不携带平台或 Runtime 私有可变状态；
3. Policy 计算候选，不直接提交模型；
4. Feedback 不成为业务事实；
5. Command 只引用业务身份；
6. 历史只记录成功提交；
7. cancel 不产生最终模型变化；
8. Viewer 只通过模型 revision 刷新稳定投影；
9. CommandStack 操作和 Viewer 同步在同一 ViewerEditSession 中关闭；
10. reject 必须证明模型、revision 和待发布事件均未改变；
11. `has_*` 是纯查询，`check_*` 的扩展故障必须结构化暴露；
12. Handle 与 Feedback 都由 Viewer 管理，不进入稳定 Part 树；
13. 连接预览不能提前创建业务连接或稳定 ConnectionPart。

## 12.19 常见错误设计

### Tool 直接改 Figure

拖拽完成后模型、撤销和多 Viewer 都无法同步。

### 每个 move 执行一个 Command

命令历史粒度错误，取消和撤销产生大量中间状态。

### Policy 内直接写模型

多个贡献无法整体验证，CommandStack 也无法管理历史。

### Feedback 复用业务 Figure

预览污染稳定场景，取消需要猜测如何恢复。

### 把连接预览注册为 ConnectionPart

候选关系获得了稳定投影身份，取消手势时只能模拟一次结构删除，也可能被选择和 Policy
误认为已提交业务连接。

### Handle 直接执行模型修改

Handle 绕过 Tool、Request 和 Policy 后，外部 action 无法复用统一校验、取消和命令历史。

### Command 保存 EditPart

Viewer 重建后历史失效，也可能意外延长视图生命周期。

### 命令成功后由调用者手工刷新 Viewer

成功、拒绝和模型状态未知没有统一关闭动作；遗漏刷新会让 history、模型和投影停在
不同前缀。

### 用 `can_undo() -> bool` 隐藏扩展检查

UI 查询可能调用扩展代码并使 Stack 故障，却只返回 `false`，调用者无法区分无历史、
明确拒绝和扩展 panic。

## 12.20 与后续章节的关系

Editor 闭环至此完成。下一章讨论这些层中的扩展对象如何分类：哪些属于基础协议，哪些
是可发现 Capability，哪些只是具体 Figure 的共享数据或 typed update。第 14 章再统一
处理命令和 Runtime 事务的失败证据。

## 12.21 思考题

1. 为什么移动预览和最终移动命令可能使用不同的校验时机？
2. Tool 应保存 ModelId、EditPartId 还是 FigureId？答案取决于什么？
3. CompoundCommand 在什么条件下才能声称原子？
4. 直接文本编辑为什么不能每次 preedit 都写模型？
5. 多个 Policy 同时贡献 Command 时，需要定义哪些冲突规则？
6. reject 路径观察到模型 revision 前进时，为什么必须升级为故障？
7. `has_undo` 与 `check_undo` 分离后，UI 和实际 undo 各自应使用哪个入口？
8. 为什么 Handle 可以命中并启动 Tool，而 Feedback 通常不应成为稳定目标？
9. 连接预览复用 Core 路由能力时，为什么其所有者仍应是 Feedback？
