# Editor 框架架构

类型：`normative-design`

状态：`target`

本文件定义 `novadraw-editor` 的目标架构。GEF Classic 源码事实见
[`../../reference/gef/core-principles.md`](../../reference/gef/core-principles.md)，
采用与差异见 [`../../parity/gef/api-coverage.md`](../../parity/gef/api-coverage.md)，
实施状态见 [`../../07-gef-roadmap/00-index.md`](../../07-gef-roadmap/00-index.md)。

## 1. 目标与边界

Editor 框架负责把应用模型、Novadraw Figure 和用户编辑行为连接成可撤销的事务：

```text
Model notification
       |
       v
EditPart tree -> FigureTree / Runtime -> Render
       ^
       |
Tool -> Request -> EditPolicy -> Command -> CommandStack -> Model
```

Editor 不负责：

- Figure 的绘制、布局、坐标、命中、damage 或资源生命周期；
- 具体业务文档 schema；
- winit、DOM、AppKit 或 Vello；
- 把任意业务模型强制装入一个框架对象层次；
- 复刻 Eclipse Workbench、JFace、Palette 或 TreeViewer。

## 2. 核心不变量

1. 模型是持久化事实源，Figure 是可重建视图。
2. EditPart 是模型与 Figure 的控制器，不是第二份业务模型。
3. Command 修改模型，不直接修改 EditPart 或 Figure。
4. 模型通知驱动视图刷新；Command 执行后不能手工伪造视图结果。
5. EditPart topology、Figure topology 和模型 containment 可以对应，但不共享身份。
6. selection 属于 Viewer；pressed/hover/capture 属于对应输入状态机。
7. feedback 必须在命令执行前清除或转为稳定模型结果。
8. 任何删除都必须同步清理 selection、focus、registry、policy、feedback 和订阅。

## 3. 身份与所有权

### 3.1 身份域

```text
EditorDomain
├── CommandStack
├── active Tool
└── Viewer*
    ├── EditorNamespace
    ├── PartTree<EditPartId>
    ├── model_registry: ModelId -> EditPartId
    ├── visual_registry: FigureId -> VisualOwner
    ├── SelectionModel<EditPartId>
    └── Runtime<FigureId>
```

`VisualOwner` 至少区分：

- `Part(EditPartId)`；
- `Handle(HandleId)`；
- `Feedback(FeedbackId)`；
- 不参与 Editor targeting 的普通 Figure。

命中 Figure 后，Viewer 沿 Figure ancestor 链查找首个已注册 visual owner。该规则保留
GEF compound Figure/content pane 的语义，不要求每个内部 Figure 都注册 EditPart。

### 3.2 ModelId

框架不定义全局业务 ID 类型。应用 adapter 提供稳定、可比较且可哈希的 ModelId。
ModelId 不得使用借用地址、`Rc` 地址或临时 arena index 作为持久身份。

### 3.3 EditPartId

EditPartId 使用 editor namespace + generational key。删除后旧 ID 必须失效；跨 Viewer
或跨 EditorDomain 误用必须结构化拒绝。

## 4. 模型适配

首版使用应用定义的 adapter，而不是 `Box<dyn Any>` 模型仓库：

```rust
pub trait ModelAdapter {
    type ModelId: Copy + Eq + Hash + Debug + 'static;
    type Event;
    type Error: Error + 'static;

    fn root(&self) -> Self::ModelId;
    fn revision(&self) -> ModelRevision;
    fn children(&self, model: Self::ModelId) -> Result<Vec<Self::ModelId>, Self::Error>;
    fn drain_events(&mut self) -> Vec<ModelEvent<Self::ModelId, Self::Event>>;
}
```

这是概念接口。对象安全、借用和通知形态由 G1/G2 的真实模型用例固定。框架需要支持：

- 初始模型投影；
- 有序 child 增删与重排；
- 属性变化刷新；
- source/target connection 关系；
- 批量通知和稳定 revision；
- 保存/加载后以新运行时身份重建。

G2 只投影 containment。Connection 的 source/target 双向发现必须与 connection layer、
Connection Runtime binding 和单 connection part 去重一起在 G5 引入，不能提前挂到
临时父 Figure。

## 5. EditPart

### 5.1 PartNode 与行为分离

采用与 FigureNode/Figure 相同的 Rust 迁移原则：

```text
PartNode
├── parent / children
├── model_id
├── figure_id
├── lifecycle / selection state
├── installed policy roles
└── Box<dyn EditPartBehavior>
```

PartTree 统一维护 topology、注册和生命周期；具体 EditPartBehavior 只表达差异行为：

- 创建 Figure；
- 指定 content pane；
- 枚举 model children 和 model connections；
- 刷新 visual；
- 安装 policy；
- 注册/注销模型观察。

不复制 Java 的宽 `EditPart` 接口和默认空方法。

### 5.2 生命周期

稳定顺序：

```text
factory creates behavior
-> bind model
-> create Figure
-> attach PartNode and Figure
-> register model/visual mappings
-> install policies
-> initial refresh
-> activate subscriptions
```

移除顺序：

```text
erase feedback
-> remove selection/focus
-> deactivate subscriptions and policies
-> recursively unregister descendants/connections
-> dispose Figure subtree
-> retire EditPartId
```

Undo 重建新的 EditPart/Figure；Command 不保存旧运行时句柄。

## 6. Viewer

Viewer 是一个编辑视图的组合根，负责：

- contents 和 RootEditPart；
- PartFactory；
- PartTree 生命周期；
- model/visual registry；
- selection 和 EditPart focus；
- targeting；
- Runtime 与 root layer 组合；
- 把归一化输入交给输入仲裁器。

RootEditPart 不对应业务模型，默认建立：

```text
root layered pane
├── handle layer                 # 不随内容缩放
├── feedback layer               # 不随内容缩放
└── scalable layers
    ├── grid layer
    ├── printable layers
    │   ├── primary layer
    │   └── connection layer
    └── scaled feedback layer
```

首版可以不实现 grid/guide，但 layer key 和缩放域必须从开始就明确。

## 7. Selection 与 Focus

SelectionModel 保存有序 EditPartId 列表：

- 空视觉选择对外可投影为 contents part；
- 最后一个元素是 primary selection；
- append、replace、toggle、remove 和 clear 都产生 typed delta；
- 删除或失活自动 reconcile；
- selection visual 由 policy/feedback layer 生成。

EditPart focus 与 Figure keyboard focus 是不同状态。Viewer 可以在明确场景同步二者，
但不能将一个状态别名成另一个。

## 8. Request 与 EditPolicy

Request 是不可直接修改模型的交互意图。首批 typed request：

- `SelectionRequest`；
- `ChangeBoundsRequest`；
- `CreateRequest`；
- `DeleteRequest`；
- `CreateConnectionRequest`；
- `ReconnectRequest`。

Request 包含入口域位置、增量、modifier、source parts、target candidate 和 interaction
revision。禁止使用无约束 `HashMap<String, Any>` 作为主协议。

EditPolicy 通过稳定 role 安装到 PartNode。Policy 可以：

- 判断是否理解 Request；
- 返回 target part；
- 贡献 Command；
- 创建、更新和清除 source/target feedback。

多个 Command contribution 的组合顺序必须确定；显式拒绝与“无贡献”必须区分。

## 9. Command 与 CommandStack

Command 只操作应用模型：

```text
prepare/can_execute
-> execute
-> push undo history
-> model notification
-> EditPart refresh
```

必须定义：

- execute、undo、redo 的结构化错误；
- CompoundCommand 的执行和逆序 undo；
- 新 execute 清空 redo；
- undo limit 与 dispose；
- save location 和 dirty；
- 失败是否进入 history；
- observer 通知时序。

首版不承诺对任意应用副作用做自动回滚。若 CompoundCommand 需要原子性，应由模型
事务或 prepared command 明确提供，不能假设逐条 `undo` 永远成功。

Command 返回普通 operation error 时，必须保证模型和 Command 保持调用前状态；
无法保证时必须返回显式 unknown-state error。CompoundCommand 会补偿已成功的前缀，
但补偿失败、unknown-state error 或扩展 panic 都会使 CommandStack faulted。Faulted
stack 保持 dirty 并拒绝继续编辑、flush 或标记保存点，由 Host 重建模型与 Editor。
Command 析构不得 panic；history trim/flush 不承诺恢复任意析构副作用。

## 10. Tool 与输入仲裁

Tool 是 EditorDomain 级状态机，同一 domain 同时只有一个 active Tool。Viewer 提供
当前输入来源和 targeting 服务。

输入仲裁必须明确：

1. 平台输入归一化；
2. Figure-native widget 优先处理；
3. Figure 已消费或建立 capture 时，不进入 Tool；
4. 否则 active Tool 接收事件；
5. Tool 在一次 gesture 内固定 tracker/source，并持续更新 target；
6. release/cancel 清理 feedback 和 capture；
7. 执行 Command 前先清除临时视觉。

`Runtime::dispatch_*` 通过最小、平台无关的 `DispatchOutcome` 返回 target、handled
与 dispatch 后 capture；Editor 不得在 app 层根据 hover 或 repaint 猜测消费结果。

## 11. 扩展点

- `ModelAdapter`：业务模型与通知；
- `EditPartFactory`：模型到 controller；
- `EditPartBehavior`：visual 与 refresh；
- `EditPolicy`：可组合编辑能力；
- `Command`：业务变更；
- `Tool` / tracker：输入解释；
- `SelectionPolicy`：选择规则；
- `RootLayerFactory`：root layer 组合；
- `ClipboardAdapter`、`Serializer`：后续产品集成。

## 12. 失败模式

公共操作至少区分：

- foreign editor/viewer/part/figure identity；
- unknown or retired model/part/visual；
- duplicate model or visual registration；
- inconsistent PartTree/FigureTree relation；
- unsupported request；
- command rejected/failed；
- model notification gap or stale revision；
- feedback cleanup failure；
- input capture conflict；
- editor faulted。

发生结构一致性破坏或扩展 panic 后，Editor 进入 faulted；不得只清理 updating 标志后
继续接受命令。

## 13. 验证入口

- Headless：模型投影、生命周期、registry、selection、command history；
- Contract：targeting、policy contribution、feedback cleanup、input arbitration；
- Native/Web：create/move/delete/reconnect、zoom/scroll、keyboard；
- Recovery：保存/加载、Runtime 重建、undo 后新 EditPart 身份；
- Accessibility：稳定模型语义与 Figure snapshot 的组合，后置到基础编辑闭环之后。
