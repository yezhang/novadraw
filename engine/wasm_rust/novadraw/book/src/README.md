# Novadraw 原理与图形应用开发

本书回答两个问题：

1. Novadraw 如何让布局、绘制、命中、输入和局部重绘始终使用同一份场景事实？
2. 应用开发者如何把业务数据变成可显示、可交互、可撤销编辑的图形应用？

全书围绕一条应用开发主线展开：

```text
业务数据或应用状态
-> 构造 Figure 与 FigureTree
-> 交给 Runtime 管理
-> 接入平台输入
-> 收敛布局和派生状态
-> 生成并提交一帧
-> 运行期通过 scoped mutable facade 更新
```

需要节点选择、拖拽、连线和撤销重做时，再在这条主线之上加入 Editor：

```text
业务模型
-> ModelAdapter
-> EditPart / GraphicalViewer
-> Figure Runtime
-> Tool -> Request -> EditPolicy -> Command
-> 修改模型
-> 刷新图形投影
```

## 适合谁

本书面向三类读者：

- **图形应用开发者**：使用内置 Figure、布局、输入、视口和连接构建应用；
- **编辑器开发者**：把业务模型接入 `GraphicalViewer`，实现可撤销编辑；
- **引擎扩展者**：实现自定义 Figure、LayoutManager、Anchor、Router 或平台宿主。

如果只是使用引擎，不需要先读完全部实现原理。先完成
[快速开始](00-first-application.md)，再按功能查阅对应章节。

## 先选择开发路径

| 你的应用 | 推荐入口 | 主要阅读 |
|---|---|---|
| 仪表盘、流程展示、静态或轻交互画布 | `FigureTree + Runtime` | 快速开始、1-6 章 |
| 节点图、流程编辑器、拓扑编辑器 | `ModelAdapter + GraphicalViewer + EditorDomain` | 快速开始、1-8 章 |
| 自定义渲染宿主或后端 | `PlatformHost + RenderBackend` | 1、3、4、9 章 |
| 自定义图形、布局或路由算法 | 对应领域 trait | 2-4、7、9 章 |

选择标准很简单：

- 业务状态可以直接由应用控制，只需要图形显示和基础交互，使用 Core；
- 业务对象需要选择、创建、移动、连接、撤销和重做，使用 Editor；
- 不要为了得到一棵图形树而引入 Editor，也不要在复杂编辑器中绕过模型直接改图形。

## 公开 API 从哪里进入

普通绘图应用依赖 Core crate `novadraw`。

```rust
use novadraw::prelude::*;
```

公开表面分为三层：

1. crate root：`Runtime`、`FigureTree`、常用 Figure、布局和基础值；
2. `novadraw::prelude::*`：常规 Figure/Runtime 开发所需的常用导入；
3. 领域模块：`container`、`connection`、`event`、`render` 等专业能力。

`novadraw::advanced` 面向诊断和深度集成，不是普通应用的默认入口。Vello 后端也不是
默认依赖；Editor、Inspector、Vello backend 和 Winit/Web platform adapter 均由独立
crate 按需组合。

## 四个生命周期阶段

使用 Novadraw 时，最重要的 API 边界不是 crate 边界，而是对象所处的生命周期：

| 阶段 | 使用方式 | 可以做什么 |
|---|---|---|
| Detached | 具体类型构造器、`with_*` | 配置尚未入树的 Figure、布局器、边框、锚点和路由器 |
| Build | `FigureTreeBuilder` | 分配 `FigureId`，组装拓扑、初始边界和布局约束 |
| Attached | `Runtime` scoped mutable facade | 更新已挂载 Figure，并自动维护失效、重绘和通知 |
| Drive | `Runtime` + Host/Backend | 分发输入、准备提交、提交后端并确认结果 |

典型错误是在挂载后继续寻找对原 Figure 值的可变引用。Figure 一旦进入树，应用应保存
`FigureId`，并通过 `runtime.figure(id)?`、`runtime.container(id)?` 等短生命周期
编辑器修改它。

## 阅读路线

### 路线 A：先做出一个应用

1. [快速开始：构建第一个图形应用](00-first-application.md)
2. [图形树与绘制](03-figure-tree-and-rendering.md)
3. [布局与更新](04-layout-update-and-frame.md)
4. 按需阅读输入、视口和连接章节

### 路线 B：理解引擎为什么正确

按第 1 章到第 7 章顺序阅读。章节沿一次状态变化形成稳定帧的因果顺序组织，不按
源码目录罗列。

### 路线 C：开发节点编辑器

先完成快速开始并阅读第 1、2、5、6 章，再阅读
[第 8 章](08-editor-framework.md)。Editor 依赖图形核心的坐标、输入和视口契约，
不能脱离这些基础单独理解。

## 每章怎么读

每章尽量回答四类问题：

- **它解决什么应用问题**；
- **内部如何维持核心不变量**；
- **应用应该从哪个公开 API 进入**；
- **出错时如何定位和验证**。

代码片段分为两类：

- “实际接口摘录”来自当前公开 API，可能省略无关错误处理；
- “概念伪代码”只解释协议和调用顺序，不承诺可以直接编译。

正文第一次使用关键概念时会给出中文定义和代码名。完整术语与代码入口见
[附录](appendix-glossary-and-map.md)。
