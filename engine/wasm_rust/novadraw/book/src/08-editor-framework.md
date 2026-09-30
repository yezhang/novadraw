# 8. 编辑框架：从业务模型到可撤销编辑

> **本章解决的问题**：如何把应用自己的模型投影成节点和连接，并把选择、拖拽、
> 创建、删除、连线和直接文本编辑转换成可撤销的业务命令。

Editor 的核心纪律是：**模型保存业务事实，Figure 只是投影，Command 只改模型，
Viewer 负责刷新。** 一旦让命令和 Figure 同时写同一项事实，撤销、重建和多视图都会
失去一致性。

## 8.1 图形核心与编辑框架的边界

**图形核心**（Core）负责呈现与输入，回答：

- 图形对象如何布局、绘制、命中和接收输入？
- 坐标、重绘区域、视口和连接如何保持一致？

**编辑框架**（Editor）负责把业务模型映射成可交互图形，并把输入转换成可撤销的模型
修改，回答：

- 哪个业务模型对应哪个图形对象？
- 当前选择了什么？
- 一段输入手势表示什么编辑意图？
- 如何生成可撤销的模型修改？

这里的**投影**是指根据业务模型创建或刷新对应的编辑部件与图形对象；模型是事实
来源，投影可以删除并重建。

因此 `novadraw-editor` 是独立的 Rust 包（crate），不把业务模型、选择状态或命令
历史栈写入 `novadraw` Core。

## 8.2 三个身份域

**身份域**是某类 ID 有效且可比较的范围。模型、查看器和场景运行时各自管理不同
身份，不能因为它们指向同一业务对象就混用：

```mermaid
flowchart LR
    M[应用模型身份 ModelId] -->|模型注册表| E[编辑部件身份 EditPartId]
    E -->|视觉对象注册表| F[图形身份 FigureId]
```

- `ModelId`：应用定义，可持久化；
- `EditPartId`：图形查看器（Viewer）命名空间内的代际运行时身份；
- `FigureId`：场景运行时（Runtime）命名空间内的代际图形身份。

可撤销命令只能保存 `ModelId` 与业务数据，不能保存仍绑定当前运行时的
`EditPartId`、`FigureId`、`ConnectionId` 或 `AnchorId`。撤销后允许重建新的视图身份。

## 8.3 业务模型是持久事实源

**模型适配器**（`ModelAdapter`）是编辑框架读取应用模型结构、连接和有序变更通知的
边界。应用通过它暴露：

```rust
pub trait ModelAdapter {
    type ModelId: Copy + Eq + Hash + Debug + 'static;
    type Event;
    type Error: Error + 'static;

    fn root(&self) -> Self::ModelId;
    fn revision(&self) -> ModelRevision;
    fn children(&self, model: Self::ModelId)
        -> Result<Vec<Self::ModelId>, Self::Error>;
    fn connections(&self)
        -> Result<Vec<ModelConnection<Self::ModelId>>, Self::Error>;
    fn drain_events(&mut self)
        -> Vec<ModelEvent<Self::ModelId, Self::Event>>;
}
```

代码锚点：
[`ModelAdapter`](../../novadraw-editor/src/model/mod.rs)。

模型不知道图形对象或查看器，图形对象也不知道模型。编辑部件和查看器负责投影。

## 8.4 编辑部件是控制器，不是第二份模型

**编辑部件**（EditPart）是连接业务模型与图形对象的控制器：它读取模型并刷新视觉，
但不复制一份可独立修改的业务状态。

`PartNode` 是编辑部件在 `PartTree` 中的运行时记录，保存：

- `model_id`
- 父节点与子节点
- 主图形与内容面板
- 视觉对象列表
- 活动状态

具体 `EditPartBehavior` 负责创建图形、刷新视觉、提供编辑策略和锚点描述符。
**锚点描述符**（Anchor Descriptor）是模型或编辑部件提供的稳定数据，用于在当前
运行时解析出实际锚点；它不会把临时的 `AnchorId` 写入持久模型。

```mermaid
flowchart LR
    Model --> Part[编辑部件 EditPart / PartNode]
    Part --> Figure[主图形]
    Part --> Pane[内容面板]
    Part --> Policies[编辑策略]
    Model -.变更通知.-> Part
    Part -.刷新视觉.-> Figure
```

编辑部件拓扑、模型包含关系和图形拓扑可以对应，但不共享身份。

代码锚点：

- [`PartNode`](../../novadraw-editor/src/part/mod.rs)
- [`PartTree`](../../novadraw-editor/src/part/mod.rs)
- [`EditPartBehavior`](../../novadraw-editor/src/part/mod.rs)

## 8.5 连接编辑部件不是包含关系子节点

**连接编辑部件**（ConnectionPart）控制一条模型连接及其连接图形。连接不是单一父级
包含的对象，因此使用独立索引：

```text
连接 -> { 起点 EditPartId, 终点 EditPartId }
出边索引(端点) -> 有序 ConnectionPartId[]
入边索引(端点) -> 有序 ConnectionPartId[]
```

`ConnectionPartId` 是受检的 `EditPartId` 角色包装，不分配第二套键。自环连接同时
出现在同一部件的出边和入边索引中，但仍只有一个连接编辑部件。

连接图形只挂到连接层，不塞入包含关系子节点。

## 8.6 查看器的组合职责

**图形查看器**（`GraphicalViewer`）把一份业务模型投影成编辑部件树和图形树，并
协调交互状态。它拥有一个模型投影及其运行时：

```rust
pub struct GraphicalViewer<A, F> {
    model: A,
    factory: F,
    runtime: Runtime,
    root_layers: RootLayers,
    parts: PartTree<A::ModelId>,
    model_registry: HashMap<A::ModelId, EditPartId>,
    visual_registry: HashMap<FigureId, VisualOwner>,
    selection: SelectionModel<EditPartId>,
    // behavior, policy, connection projections...
}
```

实际定义见
[`GraphicalViewer`](../../novadraw-editor/src/viewer/mod.rs#L646-L667)。

查看器负责：

- 捕获稳定模型快照；
- 创建和退休编辑部件与图形对象；
- 维护模型注册表与视觉对象注册表；
- 维护选择状态与编辑部件焦点；
- 维护根图层拓扑；
- 仲裁图形核心分发与编辑工具输入；
- 投影连接。

## 8.7 投影先计划，再提交

**投影刷新**是把稳定模型快照与当前视图比较，先制定变更计划，再统一同步到编辑部件
树和图形树。`GraphicalViewer::refresh()` 的顺序如下：

```text
取出模型事件
-> 校验版本连续
-> 捕获稳定模型快照
-> 确认读取期间版本未漂移
-> 计算退休、换父节点与创建计划
-> 准备受影响的连接
-> 同步包含关系
-> 同步连接
-> 发布已应用版本
```

重复的 `ModelId`、缺失的连接端点或快照版本漂移会在计划阶段被拒绝。提交期间遇到
无法补偿的扩展失败时，查看器进入故障锁定状态。

代码锚点：
[`GraphicalViewer::refresh`](../../novadraw-editor/src/viewer/mod.rs#L1752-L1791)。

## 8.8 选择状态与视觉对象所有者

**选择状态**（Selection）表示当前被用户选中的编辑部件集合。查看器按顺序保存
`EditPartId`：

- 最后一个元素是主选择项；
- 追加、替换、切换、移除、清空会产生类型明确的变化量；
- 焦点与选择状态相互独立；
- 编辑部件退休时清理失效选择。

**视觉对象所有者**（`VisualOwner`）记录命中的图形属于编辑部件、操作手柄还是临时
反馈：

```text
Part(EditPartId)
Handle(HandleId, owner, role)
Feedback(FeedbackId, optional owner)
```

查看器从命中的图形沿祖先链查找首个已注册所有者，因此一个编辑部件可以控制复合
图形，无需给每个内部图形分别注册控制器。

代码锚点：

- [`SelectionModel`](../../novadraw-editor/src/selection/mod.rs)
- [`VisualOwner`](../../novadraw-editor/src/feedback/mod.rs)
- [`GraphicalViewer::target_at`](../../novadraw-editor/src/viewer/mod.rs#L1127-L1155)

## 8.9 编辑工具、请求、策略与命令

完整编辑链：

```mermaid
sequenceDiagram
    participant Input as 输入
    participant Core as 图形场景运行时
    participant Tool as 编辑工具
    participant Policy as 编辑策略
    participant Stack as 命令历史栈
    participant Model as 业务模型
    participant Viewer as 查看器

    Input->>Core: 归一化事件
    Core-->>Tool: 分发结果
    alt 图形已处理或捕获
        Tool-->>Input: 停止
    else 交给编辑框架
        Tool->>Tool: 构造类型明确的请求
        Tool->>Policy: 查询目标、反馈与命令
        Policy-->>Tool: 仅修改模型的命令
        Tool->>Stack: 执行
        Stack->>Model: 修改模型
        Model-->>Viewer: 有序变更通知
        Viewer->>Viewer: 刷新投影
    end
```

### 编辑工具（Tool）

编辑工具是把连续输入解释成一次编辑手势的状态机。它锁定手势来源、操作类型和手势
版本；移动阶段只更新请求与反馈。

### 编辑请求（Request）

编辑请求是平台无关、模型无关、类型明确的编辑意图，本身不修改模型。当前类型包括：

```rust
pub enum EditorRequest {
    ChangeBounds(ChangeBoundsRequest),
    Create(CreateRequest),
    Delete(DeleteRequest),
    CreateConnection(CreateConnectionRequest),
    ReconnectConnection(ReconnectConnectionRequest),
    Bendpoint(BendpointRequest),
}
```

实际定义：
[`EditorRequest`](../../novadraw-editor/src/request/mod.rs#L546-L603)。

### 编辑策略（EditPolicy）

编辑策略解释某类请求对某个编辑部件意味着什么，并按稳定的 `PolicyRole` 安装到
部件。策略可以：

- 判断是否理解请求；
- 决定目标；
- 贡献命令；
- 创建来源反馈或目标反馈；
- 创建仅在本次手势内有效的连接计划。

多个策略贡献的命令按稳定角色顺序组成复合命令（`CompoundCommand`）。

### 可撤销命令（Command）

可撤销命令只修改应用模型。成功执行后才进入历史；执行新命令会清空重做历史。

## 8.10 反馈图形不是模型预写

**反馈图形**（Feedback）是命令提交前用于预览操作结果的临时图形，不是业务模型的
提前写入。

拖拽中：

```text
编辑工具更新请求
-> 编辑策略计算预览
-> 查看器安装临时反馈图形
-> 业务模型保持不变
```

释放指针时：

```text
清除临时反馈
-> 构造最终命令
-> 命令历史栈执行命令
-> 模型发出通知
-> 查看器刷新
```

反馈图形必须在命令执行前清理，避免稳定模型图形与临时图形同时表达结果。

## 8.11 命令历史栈的错误语义

**命令历史栈**（`CommandStack`）负责执行、撤销、重做并记录保存位置。命令的普通
错误必须保证模型与命令都保持调用前状态；无法保证时返回 `state_unknown`，命令
历史栈进入故障锁定状态。

复合命令（`CompoundCommand`）：

- 正序执行；
- 失败时逆序补偿已执行前缀；
- 逆序撤销；
- 补偿失败或代码恐慌导致故障锁定。

实际历史提交逻辑见
[`CommandStack::execute/undo/redo`](../../novadraw-editor/src/command/mod.rs#L398-L574)。

`is_dirty` 比较当前历史身份与保存位置，而不是只判断撤销栈是否为空。

## 8.12 编辑域（EditorDomain）

`EditorDomain` 是一组协同工作的编辑工具和命令历史的组合根，包含：

- 一个 `CommandStack`；
- 选择、移动与调整尺寸工具；
- 连接创建工具；
- 端点重连工具；
- 折点工具；
- 交互版本；
- 指针与边缘自动滚动调度状态。

执行编辑请求时：

```rust
cancel_active_tools();
let command = viewer.command_for_request(request)?;
command_stack.execute(viewer.model_mut(), command)?;
viewer.refresh()?;
```

实际实现见
[`EditorDomain::execute_request`](../../novadraw-editor/src/domain.rs#L193-L209)。

## 8.13 连接编辑

### 创建连接

两阶段手势先锁定起点，移动时更新候选终点和路径反馈，第二次确认后生成唯一模型
命令。

### 重连

固定连接编辑部件与未移动端，只替换起点或终点的模型关系。成功刷新时尽量保留连接
编辑部件和图形身份。

### 折点

请求明确区分创建、移动和删除，并携带路由坐标域中的位置与索引。模型保存折点，
运行时路径是模型投影，不是纯视觉模拟。

### 边缘自动滚动

拖拽到视口内边缘时只改变视口原点，然后用固定的表面指针位置重新运行活动工具。
它不执行模型命令。

## 8.14 直接文本编辑与插入光标

**直接文本编辑**（direct text edit）是在图形原位置编辑某个业务文本属性。它不是
把 `LabelFigure` 变成可变文本框，而是建立一个 Viewer 级临时会话：

```text
业务模型中的稳定文本
-> DirectTextEditDescriptor 捕获初值与模型修订
-> DirectTextEditState 保存草稿、选择和预编辑
-> TextFlow feedback 完成排版
-> Core 查询同一排版的光标与选择几何
-> 接受时生成一个只修改模型的 Command
```

这种分层保证取消编辑时无需回滚模型，也避免把输入法状态写入可持久化业务对象。

### 状态分别由谁拥有

| 状态 | 所有者 | 原因 |
|---|---|---|
| 已提交文本 | 业务模型 | 可持久化、可撤销的事实 |
| 初始文本与源修订 | `DirectTextEditDescriptor` | 检测编辑期间的模型漂移 |
| 草稿、定向选择、预编辑 | `DirectTextEditState` | 仅在一个编辑会话内有效 |
| 字形、行、光标和选择几何 | Core `TextFlow` 布局快照 | 必须与实际绘制使用同一次排版 |
| 文本、选择、预编辑线和光标图形 | Viewer feedback | 临时显示，不写模型 |
| 原生焦点与候选窗 | 平台文本宿主 | 只处理操作系统或浏览器差异 |

一个 Viewer 同时只允许一个直接编辑会话。`DirectTextFeature` 标识要编辑的业务属性，
`DirectTextEditRequest` 固定来源 EditPart 和属性，安装在
`PolicyRole::DirectTextEdit` 的策略返回会话计划。描述符还明确单行/多行模式及失焦
时接受或取消的策略。

```rust
pub trait DirectTextEdit<A: ModelAdapter> {
    fn descriptor(&self) -> &DirectTextEditDescriptor;
    fn feedback(
        &mut self,
        state: &DirectTextEditState,
        model: &A,
    ) -> Result<DirectTextFeedback, PolicyError>;
    fn validate(
        &mut self,
        state: &DirectTextEditState,
        model: &A,
    ) -> Result<(), PolicyError>;
    fn command(
        &mut self,
        state: &DirectTextEditState,
        model: &A,
    ) -> Result<Box<dyn Command<A>>, PolicyError>;
}
```

代码锚点：

- [`DirectTextEditState` 与 `DirectTextEdit`](../../novadraw-editor/src/direct_edit.rs)
- [`EditPolicy::start_direct_text_edit`](../../novadraw-editor/src/policy/mod.rs#L220-L228)
- [`GraphicalViewer::start_direct_text_edit`](../../novadraw-editor/src/viewer/mod.rs#L1504-L1570)

### 草稿和输入法预编辑

普通插入先替换当前选择，再把选择折叠到插入内容末尾。文本位置由段落索引、段落内
UTF-8 字节偏移和 affinity 构成；位置必须落在合法字符边界上。删除和移动不按
Unicode 标量或字节自行计算，而是查询排版产生的视觉字素、单词和行边界，因此 emoji、
组合字符和双向文本不会被拆成无效片段。

输入法预编辑需要额外的可恢复基线：

```text
第一次 Preedit
-> 保存 composition_base = { draft, selection }
-> 用 preedit 替换原选择
-> 后续 Preedit 替换上一段 preedit

Commit
-> 用提交文本替换 preedit
-> 清除 composition_base
-> 仍停留在直接编辑会话

CancelComposition
-> 恢复 composition_base
-> 清除 preedit
```

输入法提交只改变草稿，不等于接受整个会话。活动 preedit 尚未结束时也不能接受会话，
否则业务模型可能得到候选选择过程中的中间文本。

Web `EditContext` 维护完整的浏览器文本缓冲区，因此不把一次 `textupdate` 拆成“修改
selection”和“插入文本”两个可观察步骤。`WebEditContextBridge` 先把 UTF-16 offset
转换为 UTF-8 边界，再生成包含完整 draft、selection 和 composition range 的
`TextInputSnapshot`；`EditorDomain` 在一次状态替换中应用它并重建 feedback。Viewer
随后把校验后的快照同步回 EditContext，使浏览器缓冲区始终只是 Editor 状态的镜像。

### 光标几何来自实际文本布局

插入光标不能通过“字符数乘平均字宽”估算。`TextFlowFigure` 在排版时保存不可变的
交互映射，命中、位置移动、选择矩形和光标矩形都查询这份映射：

```mermaid
flowchart LR
    D[当前草稿] --> L[TextFlow 排版快照]
    L --> P[Glyph 绘制]
    L --> H[点到文本位置]
    L --> M[视觉字素、单词与行移动]
    L --> S[选择矩形]
    L --> C[插入光标矩形]
```

`TextFlowFigure` 先在节点本地域返回几何；`Runtime` 再应用完整祖先变换，返回逻辑
表面坐标。Viewer 使用同一个表面坐标结果绘制 feedback，并把可见光标矩形交给平台
定位输入法候选窗。滚动、缩放、节点移动或窗口 resize 后必须重新同步，不能缓存旧的
表面坐标。

代码锚点：

- [`TextFlowFigure` 的文本交互查询](../../novadraw/src/figure/text_flow.rs#L293-L390)
- [`Runtime` 的表面坐标查询](../../novadraw/src/runtime/runtime.rs#L2834-L2903)
- [`GraphicalViewer::direct_text_caret_geometry`](../../novadraw-editor/src/viewer/mod.rs#L1669-L1699)

### 编辑反馈、裁剪与长文本

策略生成的 `DirectTextFeedback` 必须指出其中哪个图形是可查询的
`TextFlowFigure`。单行编辑通常使用目标 client area 扣除 padding 后的边界，并配置
`NoWrap`；编辑态不能使用 `Truncate` 或省略号，因为被隐藏文本仍需参与选择和光标
移动。

Viewer 把完整草稿排版在这个固定视口内，并通过 `TextFlowViewport::clipped` 同时
施加内容偏移和边界裁剪：

```text
完整文本宽度
-> 查询未滚动的自然光标位置
-> 计算最小水平 scroll offset，使光标留在可见边距内
-> 以 -scroll offset 绘制文本
-> 把文本、选择、preedit 下划线和光标裁剪到编辑视口
```

因此输入长度不受节点宽度限制；超长草稿在节点内水平滚动，而不是越界或被省略。
选择区使用半透明矩形，preedit 使用下划线，插入光标使用带最小可见宽度的矩形。
输入法未提供组合区内选择时，Viewer 隐藏组合光标，而不是伪造一个位置。

每次草稿、选择或 preedit 改变时，Viewer 先构造并校验新 feedback，再移除旧
feedback。添加与删除分别登记新旧可视区域的 damage，使目标移动或文本滚动后旧像素
也会被清除；新投影失败时则保留旧状态。

代码锚点：

- [`TextFlowViewport`](../../novadraw/src/figure/text_flow.rs#L184-L215)
- [`GraphicalViewer::attach_direct_text_feedback`](../../novadraw-editor/src/viewer/mod.rs#L1995-L2215)
- [`GraphicalViewer::replace_direct_text_state`](../../novadraw-editor/src/viewer/mod.rs#L1948-L1993)

### 光标闪烁由宿主时间驱动

光标是 feedback 层中的普通矩形 Figure，不是 Native 控件或隐藏 `textarea` 自带的
光标。它的绘制分成几何生成、图形挂载、定时切换和局部修复四步。

#### 几何生成与图形挂载

每次建立或更新编辑反馈时，Viewer 先稳定 `TextFlow` 布局，再从当前 selection focus
或 preedit 内部选择得到 `FlowTextPosition`。这个位置经过以下变换：

```mermaid
flowchart LR
    P[FlowTextPosition] --> L[TextFlow 本地 caret geometry]
    L --> R[应用 TextFlow 内部滚动]
    R --> S[变换到逻辑表面坐标]
    S --> C[与编辑视口求交]
    C --> F[创建 RectangleFigure]
    F --> O[挂到 unscaled feedback 层]
```

光标宽度会被提升到最小可见宽度，高度直接采用当前文本行的 caret 高度。它挂在
unscaled feedback 层，因此可见宽度不会随模型缩放放大；当 viewport、zoom 或祖先
transform 改变时，Viewer 重新查询表面坐标并重建位置。这个表面矩形同时作为
`TextInputEffect::Acquire/SetArea` 的候选窗锚点，但平台宿主不负责绘制它。

普通 selection focus 总是产生光标。IME preedit 携带内部 selection 时，光标位于该
selection 的末端；平台明确传入 `None` 时不创建光标 Figure，也不安排闪烁 deadline。

#### 定时与局部重绘

光标刚创建或文本交互发生后处于可见状态，Viewer 从当前单调时间计算下一个闪烁
deadline。宿主只在 deadline 到达时唤醒：

```mermaid
sequenceDiagram
    participant Viewer
    participant Host as 平台宿主
    participant Runtime
    participant Backend as 渲染后端

    Viewer-->>Host: next_wake_deadline()
    Host->>Host: WaitUntil / timeout
    Host->>Viewer: advance_time(now)
    Viewer->>Runtime: caret.set_visible(false)
    Runtime->>Runtime: 擦除旧 visual bounds
    Runtime-->>Host: changed = true
    Host->>Runtime: prepare_submission()
    Runtime-->>Backend: 只提交 caret damage
    Viewer-->>Host: 下一个 deadline
    Host->>Viewer: advance_time(now)
    Viewer->>Runtime: caret.set_visible(true)
    Runtime->>Runtime: 登记 caret repaint
```

隐藏时，`set_visible(false)` 记录光标原有 `visual_bounds` 作为擦除区域；显示时，
`set_visible(true)` 请求光标自身重绘。两者都使父级派生状态失效，之后由
`UpdateManager` 把局部 damage 传播到表面并交给正常帧提交。因此闪烁复用已有文本与
光标 feedback，不需要重建整组编辑反馈或强制全屏重绘。

`advance_time` 会计算自上个 deadline 起已经跨过多少个闪烁间隔：跨过奇数个间隔才
反转可见性，跨过偶数个间隔保持原状态，然后直接安排未来的下一个 deadline。窗口
休眠或事件循环延迟时不会补画已经错过的中间帧，也不会让 deadline 永久落在当前时间
之前。

`next_wake_deadline()` 返回 Runtime 内部动画 deadline 与 caret deadline 中更早的
一个。Native 宿主据此设置 `WaitUntil`，Web 宿主据此设置带 generation 的 timeout，
旧 timeout 回调会被丢弃。Native 可依据 `advance_time` 的变化结果请求绘制；Web
唤醒后进入统一 render 调度，没有待提交 damage 时不会产生新提交。

使用宿主注入的单调时间而不是 wall clock，可以让 Native、Web 和 headless replay
共用确定性逻辑。草稿、选择或预编辑更新会创建新的可见光标并重置闪烁周期；仅同步
坐标变换时则保留原来的显示/隐藏相位。会话结束或不存在可见光标时清除 deadline。
Web 的隐藏输入控件关闭自身 caret 显示，避免 canvas 光标与 DOM 光标重叠。

代码锚点：

- [`GraphicalViewer::attach_direct_text_feedback`](../../novadraw-editor/src/viewer/mod.rs#L2032-L2215)
- [`GraphicalViewer::advance_time`](../../novadraw-editor/src/viewer/mod.rs#L880-L927)
- [`GraphicalViewer::reset_direct_text_blink`](../../novadraw-editor/src/viewer/mod.rs#L2240-L2250)
- [`FigureTree::set_visible_with_update`](../../novadraw/src/graph/mod.rs#L4177-L4205)
- [`Native deadline 调度`](../../examples/native/node-editor-demo/src/main.rs#L2017-L2070)
- [`Web deadline 调度`](../../examples/web/web-validation/src/direct_edit_mode.rs#L602-L644)

### 接受、取消与失败恢复

接受前依次检查：

1. 来源 EditPart 仍然活动；
2. 模型修订仍等于会话开始时捕获的修订；
3. 没有活动的输入法预编辑；
4. 策略验证草稿通过。

草稿未变化时直接结束，不产生空命令；草稿变化时只生成一个模型 Command，交给
`CommandStack` 执行，再由 Viewer 刷新投影。可恢复的命令拒绝会重新安装原草稿
feedback 并重新获取输入租约。取消则只删除 feedback、停止闪烁并释放租约，不创建
历史记录。来源节点退休或 Viewer 故障时也必须强制清理会话。

代码锚点：

- [`GraphicalViewer::prepare_direct_text_accept`](../../novadraw-editor/src/viewer/mod.rs#L1863-L1914)
- [`EditorDomain::accept_direct_text_edit`](../../novadraw-editor/src/domain.rs#L283-L312)
- [`GraphicalViewer::force_drop_direct_text_edit`](../../novadraw-editor/src/viewer/mod.rs#L2845-L2859)

## 8.15 实现一个编辑器的顺序

不要从拖拽工具开始。先按以下顺序建立闭环：

1. **定义业务模型身份与修订**
   使用可持久化 `ModelId`，每次已提交变化产生连续 `ModelRevision`。
2. **实现 `ModelAdapter`**
   先支持根、子节点、连接快照和有序事件排空。
3. **实现 `EditPartFactory` 与最小 `EditPartBehavior`**
   只创建静态主图形，证明模型可以确定性投影。
4. **创建 `GraphicalViewer`**
   检查模型注册表、图形注册表、连接层和刷新错误。
5. **加入选择与一个编辑请求**
   从 `ChangeBounds` 或 `Delete` 开始，完成
   `Tool -> Request -> EditPolicy -> Command -> Model -> refresh`。
6. **接入 `EditorDomain` 与 CommandStack**
   验证执行、撤销、重做、取消和保存位置。
7. **按业务需要加入直接文本编辑**
   为稳定的文本 feature 实现 policy、草稿 feedback 和模型 Command，再接入平台
   文本输入宿主。
8. **最后加入连接、折点和边缘自动滚动**
   这些能力依赖前面的身份、坐标、反馈和命令边界。

最小组合形态：

```rust
use novadraw_editor::{EditorDomain, GraphicalViewer};
use novadraw::Rectangle;

let viewer = GraphicalViewer::new(
    model_adapter,
    edit_part_factory,
    Rectangle::new(0.0, 0.0, width, height),
)?;
let domain = EditorDomain::new();
```

`GraphicalViewer` 拥有 Figure Runtime，平台输入应先交给 Viewer/Core 仲裁，再由
`EditorDomain` 推进活动工具。窗口尺寸、滚动或缩放变化后，需要让活动工具在同一
逻辑表面指针位置重新投影。

产品代码建议把职责分开：

| 模块 | 保存内容 |
|---|---|
| `model` | 可持久化节点、连接和业务属性 |
| `adapter` | 模型快照、修订与事件 |
| `parts` | Figure 创建、视觉刷新和锚点描述 |
| `policies` | 请求解释、反馈、直接编辑计划与命令生成 |
| `commands` | 只依赖模型 ID 的可撤销修改 |
| `app` | Viewer、EditorDomain、文本输入宿主和后端组合 |

可运行示例：
[`examples/native/node-editor-demo`](../../examples/native/node-editor-demo)。该示例包含完整能力，
实现自己的应用时应按上面的顺序逐层引入，而不是一次复制全部代码。

## 8.16 失败模式

| 错误 | 后果 |
|---|---|
| 命令保存 `FigureId` | 撤销或重建后引用失效 |
| 图形对象直接修改模型 | 绕过历史和通知 |
| 命令执行后手工修改图形 | 模型与视图出现双写 |
| 连接编辑部件塞入包含关系 | 树关系无法表达双端点 |
| 选择状态写入 `NodeState` | 多个查看器的选择状态互相污染 |
| 反馈图形直接写模型 | 取消时无法恢复，撤销粒度错误 |
| 图形已处理后编辑工具仍执行 | 控件点击同时触发编辑 |
| 刷新时忽略版本缺口 | 丢失事件后仍宣称投影稳定 |
| 把草稿写入稳定 Figure 或模型 | 取消、撤销和多视图同步失去边界 |
| 光标几何自行估算 | 字体替换、emoji、换行或双向文本下错位 |
| 编辑态使用省略号 | 光标和选择无法到达被截断文本 |
| 只重绘新 feedback 区域 | 节点移动或内部滚动后留下旧像素 |
| 接受活动 preedit | 候选过程中的中间文本进入模型 |
| 不释放文本输入租约 | 候选窗和迟到输入污染后续会话 |

## 8.17 验证入口

- [`g1_model_contract.rs`](../../novadraw-editor/tests/g1_model_contract.rs)
- [`g1_command_stack_contract.rs`](../../novadraw-editor/tests/g1_command_stack_contract.rs)
- [`g2_viewer_projection_contract.rs`](../../novadraw-editor/tests/g2_viewer_projection_contract.rs)
- [`g3_selection_contract.rs`](../../novadraw-editor/tests/g3_selection_contract.rs)
- [`g3_viewer_interaction_contract.rs`](../../novadraw-editor/tests/g3_viewer_interaction_contract.rs)
- [`g4_editing_loop_contract.rs`](../../novadraw-editor/tests/g4_editing_loop_contract.rs)
- [`g5_connection_projection_contract.rs`](../../novadraw-editor/tests/g5_connection_projection_contract.rs)
- [`g5_connection_creation_contract.rs`](../../novadraw-editor/tests/g5_connection_creation_contract.rs)
- [`p2_e02_direct_text_edit_contract.rs`](../../novadraw-editor/tests/p2_e02_direct_text_edit_contract.rs)
- `cargo xtask run replay.editor-g3`
- `cargo xtask run replay.editor-g4`
- `cargo xtask run replay.editor-g5.2`
- `cargo xtask run replay.editor-g5.3`
- `cargo xtask run replay.editor-g5.4`
- `cargo xtask run replay.editor-g5.5`
- `cargo xtask verify editor.p2-e02-direct-text-edit`
- `cargo xtask verify platform.p2-e02-text-input`
