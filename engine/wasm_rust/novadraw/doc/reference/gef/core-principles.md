# GEF 框架核心原理

类型：`reference-analysis`

> 基于 Eclipse GEF (Graphical Editing Framework) 架构设计分析

## 1. 概述

GEF (Graphical Editing Framework) 是一个用于构建图形编辑器的框架，参考其架构设计可以为 Novadraw 提供清晰的架构指导。

### 1.1 GEF 解决的问题

GEF 旨在解决图形编辑器的三个核心问题：

1. **视图构建**：当编辑器打开时，根据模型状态自动构建视图
2. **视图更新**：当模型变化时，自动更新视图以反映新状态
3. **用户交互**：捕获用户在 GUI 上的操作并将其转换为模型修改

## 2. MVC 架构

GEF 基于经典的 Model-View-Controller (MVC) 架构，但提供了各个组件的实现框架，需要开发者自行组装。

### 2.1 Model（模型）

模型是存储所有用户编辑数据的对象集合。

**核心要求**：

- 模型必须持有所有需要被编辑的数据
- 模型不应知道视图或编辑器的任何信息（无反向依赖）
- 模型必须实现通知机制：状态变化时触发事件，允许注册监听器

```text
Model --firePropertyChange--> Listeners
```

### 2.2 View（视图）

视图是构成图形界面的构建块集合。

**核心要求**：

- 视图不应持有模型中未存储的重要数据
- 视图不应知道模型的存在，完全"哑"（dumb）
- 在 GEF 中，视图通常由 Draw2d 的 Figure 树构建

```text
Figure Tree（由 viewer/root edit part 管理的根 Figure 层次承载）
```

### 2.3 Controller（控制器）

控制器连接模型和视图，在 GEF 中对应 **EditPart**。

**核心职责**：

- 每个需要图形表示的模型对象都有一个对应的 EditPart
- EditPart 同时持有模型对象引用和视图 Figure 引用
- EditPart 注册为模型监听器，接收变化通知
- 根据模型新状态更新视图

```text
Model <---引用--- EditPart ---> Figure
  ^                  |
  └---模型监听注册----┘
```

## 3. Draw2d 基础

Draw2d 是一个轻量级图形组件系统，类似于 Swing，但完全专注于图形显示。

### 3.1 Figure 树

Figures 是 Draw2d 的基本构建块，GUI 由一棵 Figure 树定义。

**父子关系含义**：

- 子 Figure 总是绘制在父 Figure 之上
- 默认 `Figure.paintChildren()` 把每个 child 裁剪到 child bounds；父级调用链的
  graphics clip 继续限制祖先可见区域。父 Figure 也可通过
  `IClippingStrategy` 为 child 返回一个或多个自定义裁剪矩形
- 移动父 Figure 会带动整个子树移动
- 子 Figure 的位置由父 Figure 的布局管理器决定

```text
paint(Graphics) 执行顺序：
1. 绘制 Figure 自身 (paintFigure)
2. 递归绘制所有子 Figure
3. 绘制 Figure 边框
```

### 3.2 关键特性

- **Layout Managers**：自动调整子 Figure 的边界
- **Hit Testing**：递归算法确定指定点处的顶层 Figure
- **Layers**：透明 Figure，用于分层管理（如连接线层）
- **ConnectionAnchors**：锚点，跟踪关联 Figure 的位置变化

## 4. 视图构建机制

### 4.1 核心组件

| 组件                | 职责                                |
|---------------------|-------------------------------------|
| EditPartFactory     | 根据模型对象创建对应的 EditPart      |
| EditPart            | 连接模型与视图的控制器               |
| GraphicalViewer     | 在 SWT Control 上安装视图            |
| RootEditPart        | EditPart 树的根节点，管理 Figure 图层 |
| Figure              | Draw2d 图形对象                     |

### 4.2 构建流程

```text
1. contents 模型对象传入 EditPartViewer
2. EditPartFactory 为其创建 EditPart（content EditPart）
3. 调用 content EditPart 的 createFigure() 和 refreshVisuals()
4. 调用 getModelChildren() 获取子模型对象列表
5. 为每个子模型创建对应 EditPart 和 Figure
6. 递归直到所有叶子节点处理完毕
```

### 4.3 EditPart 核心方法

```java
// 创建视图 Figure（由 GEF 调用）
protected IFigure createFigure();

// 刷新视图以匹配模型状态
public void refreshVisuals();

// 返回内容面板（默认返回根 Figure）
protected IFigure getContentPane();

// 返回应在内容面板中表示的子模型对象列表
protected List getModelChildren();
```

### 4.4 Content Pane

Content Pane 是接收 child EditPart Figure 的容器 Figure。默认实现直接返回
`getFigure()`；复合 Figure 可以返回其中一个内部容器。它通常有 children，因此
不是“叶子节点”。

```text
PersonFigure
├── Label (name)
├── Label (surname)
└── Figure (contentPane)
    ├── FruitFigure 1
    ├── FruitFigure 2
    └── ...
```

## 5. 视图更新机制

### 5.1 通知机制

模型通过 PropertyChangeSupport 或类似机制发出通知：

```java
public class Person {
    private PropertyChangeSupport listeners;

    public void setName(String newName) {
        String oldName = name;
        name = newName;
        listeners.firePropertyChange("name", oldName, newName);
    }
}
```

### 5.2 EditPart 监听与响应

```java
public class PersonEditPart extends AbstractGraphicalEditPart
        implements PropertyChangeListener {

    public void activate() {
        ((Person) getModel()).addPropertyChangeListener(this);
    }

    public void deactivate() {
        ((Person) getModel()).removePropertyChangeListener(this);
    }

    public void propertyChange(PropertyChangeEvent ev) {
        if (ev.getPropertyName().equals(Person.PROPERTY_NAME)) {
            refreshVisuals();
        } else if (ev.getPropertyName().equals(Person.PROPERTY_FRUITS)) {
            refreshChildren();
        }
    }
}
```

### 5.3 刷新方法

| 方法                        | 用途                                          |
|-----------------------------|-----------------------------------------------|
| `refreshVisuals()`          | 刷新单个 EditPart 的视图属性                   |
| `refreshChildren()`         | 根据 getModelChildren() 重建子 EditPart        |
| `refreshSourceConnections()` | 刷新源连接                                      |
| `refreshTargetConnections()` | 刷新目标连接                                   |

## 6. 用户交互机制

### 6.1 EditDomain

EditDomain 是所有参与编辑会话的 GEF 对象的共同容器：

- 提供 CommandStack（命令栈）
- 管理 PaletteViewer 和 Tools
- 绑定所有编辑组件

### 6.2 交互流程

```text
用户操作
    ↓
EditPartViewer 捕获事件
    ↓
转发给 EditDomain 的 active Tool
    ↓
Tool 解释事件序列，构建 Request
    ↓
Tool 发送 Request 给 EditPart
    ↓
EditPart 将 Request 转发给 EditPolicy
    ↓
EditPolicy 返回 Command
    ↓
CommandStack.execute(Command)
    ↓
模型修改
    ↓
触发通知 → 视图更新
```

### 6.3 Request（请求）

Request 是对编辑操作的高层抽象，封装了操作意图但不含具体实现：

| Request 类型           | 含义             |
|----------------------|------------------|
| REQ_MOVE             | 移动操作         |
| REQ_CREATE           | 创建操作         |
| REQ_ADD              | 添加子元素       |
| REQ_DELETE           | 删除操作         |
| REQ_CONNECTION_START  | 开始连接         |
| REQ_RECONNECT_SOURCE / REQ_RECONNECT_TARGET | 重连源端或目标端 |

### 6.4 Command（命令）

Command 封装可执行、可撤销的模型修改，是 GEF 编辑交互的核心扩展点：

```java
public abstract class Command {
    public void execute();        // 执行修改
    public void undo();           // 撤销
    public void redo();           // 重做
    public boolean canExecute();  // 检查是否可执行
}
```

**CommandStack**：

- 实现撤销/重做栈
- 通过 `execute(Command)` 执行命令
- 通过 `isDirty()` 判断编辑器是否需要保存
- 调用 `markSaveLocation()` 标记保存点

### 6.5 EditPolicy（编辑策略）

EditPolicy 是安装到 EditPart 上的可插拔组件，负责：

- 根据 Request 返回对应的 Command
- 显示操作反馈（Feedback）

**Role 机制**：

```java
installEditPolicy(EditPolicy.CONTAINER_ROLE, new ContainerEditPolicy());
installEditPolicy(EditPolicy.LAYOUT_ROLE, new XYLayoutPolicy());
```

常用 Role：

| Role                 | 用途                                    |
|----------------------|-----------------------------------------|
| `COMPONENT_ROLE`     | 不属于其他 role 的基础模型操作，例如删除 |
| `CONNECTION_ROLE`    | 连接操作                                |
| `CONTAINER_ROLE`     | 容器操作（添加/移除子元素）              |
| `LAYOUT_ROLE`        | 布局策略                                |

### 6.6 Feedback（反馈）

操作过程中显示的预览效果：

- **Source Feedback**：操作源上的预览
- **Target Feedback**：操作目标上的预览

## 7. 连接（Connections）

### 7.1 特殊挑战

连接是特殊的模型对象，因为：

- 不能属于特定内容面板（需能连接任意节点）
- 必须始终位于其他元素之上
- 必须连接源和目标的表示

### 7.2 Draw2d 连接组件

| 组件                  | 职责                                       |
|-----------------------|--------------------------------------------|
| `Polyline`            | 由点列表定义的线段                          |
| `ConnectionAnchor`    | 跟踪源/目标 Figure 位置的锚点               |
| `ConnectionRouter`    | 根据约束计算连接路径                        |
| `PolylineConnection`  | 实现 Connection 接口的 Polyline             |

### 7.3 连接 EditPart

连接使用 `AbstractConnectionEditPart`：

```java
public class ConnectionEditPart extends AbstractConnectionEditPart {
    protected IFigure createFigure() {
        return new MyConnectionFigure();
    }

    protected void refreshVisuals() {
        // 更新约束、标签等
    }
}
```

### 7.4 连接发现

连接被两次"发现"：

1. 在源节点的 `getModelSourceConnections()` 中
2. 在目标节点的 `getModelTargetConnections()` 中

## 8. Novadraw 设计映射

### 8.1 架构对应

| GEF                | Novadraw         | 说明                     |
|--------------------|------------------|--------------------------|
| Model              | Model / Scene    | 数据模型                  |
| Figure             | Figure           | 渲染对象                  |
| EditPart           | SceneNode        | 控制器                   |
| EditPolicy         | OperationHandler | 操作处理                 |
| Command            | Operation        | 命令模式                 |
| EditDomain         | EditorContext    | 编辑上下文               |
| Tool               | InteractionTool  | 交互工具                 |

### 8.2 关键设计决策

1. **模型纯化**：模型不依赖任何视图组件
2. **单向绑定**：模型变化驱动视图更新，而非反向
3. **命令模式**：由 Tool/EditPolicy 产生的编辑操作通常通过 CommandStack
   执行；框架并不能阻止应用代码直接修改模型
4. **策略模式**：通过 EditPolicy 机制支持操作的可插拔
5. **增量子树同步**：`AbstractEditPart.refreshChildren()` 线性比较 model children
   与现有 EditPart children，复用、重排、新建或移除直接子节点；整个 EditPart
   层次仍通过父子调用逐层构建和刷新

### 8.3 架构优势

- **关注点分离**：模型、视图、控制器各司其职
- **可测试性**：各组件可独立测试
- **可扩展性**：通过 Command、EditPolicy 扩展
- **可复用性**：模型可在不同视图/应用中复用
- **一致性**：统一的交互模式

## 参考

- [GEF Documentation](https://github.com/eclipse-gef/gef-classic)
- [draw2d 源码](https://github.com/eclipse-gef/gef-classic/tree/master/org.eclipse.draw2d/src/org/eclipse/draw2d)
- 本地审计基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）
