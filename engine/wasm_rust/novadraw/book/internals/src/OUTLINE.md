# 《深入理解 Novadraw》书籍设计

## 1. 定位

本书是图形编辑框架原理书，不是 API 手册，也不是仓库贡献手册。

核心问题：

> 一个支持组合、布局、交互、连接和模型编辑的图形框架，如何让所有子系统对同一次
> 状态变化得出一致结果？

书中使用 Novadraw 的概念和类型名建立精确模型，但优先解释稳定机制，不绑定当前文件
布局、实现进度或提交工作流。

## 2. 读者画像

主要读者：

- 已经能够阅读 Rust，希望理解图形框架架构的开发者；
- 使用 Novadraw 后需要理解其行为边界的高级用户；
- 准备参与 Novadraw 开发、但尚未进入具体代码的贡献者；
- 设计画布、节点编辑器、流程编辑器或 GUI 基础设施的工程师。

前置知识：

- Rust 所有权、trait、泛型和错误处理；
- 基本二维几何和矩阵概念；
- 事件循环与 retained-mode UI 的基本认识。

本书不要求读者了解 Vello、Winit 或 Draw2D 的具体 API。

## 3. 叙事主线

全书围绕两条闭环组织。

图形闭环：

```text
源状态变化
-> Runtime 事务
-> 布局与派生状态收敛
-> 稳定场景状态
-> 可选表现覆盖快照
-> damage 与绘制命令
-> 后端提交和完成反馈
```

编辑闭环：

```text
平台输入
-> 命中与交互状态
-> Tool / Request / Policy
-> Command 修改模型
-> Viewer 同步视图
-> 图形闭环
```

每个专题都回到这两条闭环，避免按模块孤立介绍概念。

## 4. 章节设计

### 第 1 章：图形编辑框架解决什么问题

主问题：为什么绘制库、场景图和编辑框架是三个不同层次？

覆盖：

- immediate mode 与 retained mode；
- Figure 树作为共享场景状态的结构骨架；
- 图形应用与图形编辑器的差异；
- 业务源状态、场景源状态、派生状态、交互状态、表现状态和后端状态；
- 两条端到端闭环。

关键图：从业务模型到像素、再从输入回到模型的双向因果图。

### 第 2 章：状态、身份与所有权

主问题：复杂交互中如何避免多个子系统同时拥有同一事实？

覆盖：

- 单一事实来源；
- `ModelId -> EditPartId -> FigureId`；
- 代际身份与 stale reference；
- detached、build、attached、drive 生命周期；
- 长期所有权与短生命周期借用；
- 缓存和派生状态的重建条件。

关键图：状态所有权表与三个身份域。

### 第 3 章：固定管线与开放扩展

主问题：为什么框架应开放差异能力，但不能开放主流程控制权？

覆盖：

- 模板方法与固定执行阶段；
- pure prepare 与统一 commit；
- capability discovery；
- dependency inversion；
- 扩展点和框架不变量的边界；
- 组合优于继承在 Rust 中的表达。

关键图：Runtime 固定管线和可插拔策略的关系。

### 第 4 章：Figure 树与组合语义

主问题：一棵树为什么能够同时承载组合、Z-order、绘制、命中和坐标传播？

覆盖：

- Figure、FigureNode、NodeState 的职责；
- 父子关系、接纳规则与循环检测；
- 深度和遍历顺序；
- 生命周期与拓扑的区别；
- 绘制顺序和命中顺序的镜像关系；
- 容器、图层和复合 Figure。

关键图：节点状态、行为对象和树所有权。

### 第 5 章：几何、坐标域与变换

主问题：如何保证画到哪里、点到哪里、重绘哪里是一致的？

覆盖：

- bounds、client area、visual bounds 和表面包围盒；
- 父内容域、节点本地域、子内容域和表面域；
- 仿射变换组合顺序；
- 坐标根、视口与缩放；
- 不可逆变换和保守轴对齐包围盒；
- 坐标查询为何必须集中。

关键图：一个嵌套节点从本地域变换到逻辑表面域的完整链路。

### 第 6 章：测量、布局与稳定化

主问题：为什么布局不是一次函数调用，而是受约束的派生状态收敛？

覆盖：

- intrinsic size、尺寸提示和约束测量；
- 布局约束属于父子关系；
- snapshot、纯计算输出和原子应用；
- invalidate、validate 和稳定 epoch；
- 派生状态依赖图；
- 收敛失败与循环更新。

关键图：测量、排列、派生状态和稳定发布的时序。

### 第 7 章：文本排版与字形管线

主问题：文本如何从 Unicode 内容和字体资源变成稳定的尺寸、字形位置与交互几何？

覆盖：

- 字体描述、字体资源 revision 与精确字体实例；
- 分段、字体 fallback、字形整形、换行和定位；
- `MeasureContext` 与 `PaintContext` 的分工；
- 布局盒、ink bounds、caret 与 selection 几何；
- `TextFlow` 的段落结构、缓存键和失效；
- 多语言文本、glyph outline 与直接编辑边界。

关键图：文本从 UTF-8 内容经过 shaping 和换行形成不可变 `TextLayout` 的完整管线。

### 第 8 章：Damage、绘制与帧提交

主问题：状态变化如何转化为正确且可增量提交的像素结果？

覆盖：

- dirty 与 damage 的区别；
- 旧视觉边界和新视觉边界的 union；
- 裁剪、绘图状态和递归绘制；
- Graphics、命令录制与 RenderSubmission；
- PaintContext、CommandRecorder 与帧提交准备的分离；
- RenderResult 与 Runtime acknowledgement；
- 资源增量、ResourceRequestToken 和 session/frame identity。
- 图像的物理像素、逻辑尺寸、源矩形和资源 revision；
- 图像异步加载、迟到结果、解码失败和渲染失败。

关键图：从 mutation 到完成反馈的完整帧生命周期。

### 第 9 章：动画与表现状态层

主问题：如何产生连续视觉过渡，同时保证模型、布局、命中、通知和历史只认最终事实？

覆盖：

- 已提交状态、表现状态与后端状态的所有权边界；
- Target、Motion、Trigger 与 Composition；
- Runtime-owned clock、timeline 与稀疏表现覆盖快照；
- before/after capture、channel ownership 与 retarget；
- old/new visual envelope 与 animation damage；
- Disabled、ReducedMotion、suspension 和当前已实现的 interaction geometry；
- layout、route、viewport 与动画临时绘制项消费者。

关键图：稳定场景状态与可选表现覆盖快照合并形成帧，表现状态不反向污染源状态。

### 第 10 章：命中、输入与交互状态

主问题：离散平台事件如何组成稳定、可取消的交互会话？

覆盖：

- 平台输入归一化；
- 命中顺序与坐标降域；
- hover、capture、focus 和 gesture；
- 回调效果队列与禁止重入；
- Core 与 Editor 的输入仲裁；
- 键盘事件、文本输入和 IME 的区别。

关键图：pointer down、move、up/cancel 的交互状态机。

### 第 11 章：图层、视口与连接

主问题：跨节点派生几何如何在滚动、缩放和结构变化后保持一致？

覆盖：

- 语义图层与叠放；
- freeform 内容范围；
- viewport、range model、缩放约束和 zoom-to-fit；
- 业务连接、ConnectionPart 与 Core Connection 的三层边界；
- 锚点、SceneQuery 和依赖追踪；
- 路由器的纯计算边界与内置算法分类；
- 分组路由、连接失效和自环。

关键图：节点变化经依赖追踪触发连接重算和表面几何重算。

### 第 12 章：模型、EditPart 与确定性视图派生

主问题：为什么业务模型不能直接等同于 Figure 树？

覆盖：

- 模型是持久事实，Figure 是可重建的派生视图；
- ModelAdapter 和稳定快照；
- PartNode、EditPartBehavior 与 PartTree 的职责；
- Root、Containment 与 Connection 三种 Part 角色；
- containment 视图构建、连接视图构建，以及 ConnectionPart 的身份边界；
- primary Figure、content pane、registry 和 visual ownership；
- Overlay、Handle 与 Feedback 的所有权和生命周期；
- plan、validate、commit 与 revision 发布；
- synchronize/synchronize_model 的受控模型入口；
- ViewerDrive 的宿主驱动边界。

关键图：模型、Part、Figure 三个身份域及视图同步事务。

### 第 13 章：Tool、Request、Policy 与 Command

主问题：如何把连续输入转化为可撤销、可测试的业务修改？

覆盖：

- Tool 作为手势状态机；
- Request 作为类型化编辑意图；
- Policy 负责目标解释和命令贡献；
- Command 的执行、撤销、重做和补偿；
- ViewerEditSession 的 commit/reject/fault 关闭动作；
- CommandStack 的 has/check 查询分层；
- Overlay 中 Handle 与 Feedback 的交互边界；
- Feedback 和连接预览不提前写入模型或创建稳定 ConnectionPart；
- 创建、移动、连接和直接编辑的共同结构；
- Selection、focus、多 Viewer 隔离与删除修复；
- 跨容器拖放和复制粘贴的协议边界。

关键图：输入到模型再到 Viewer 刷新的完整编辑闭环。

### 第 14 章：类型化 Capability 与组件更新

主问题：如何避免宽接口、中心枚举和伪行为 trait？

覆盖：

- 基础协议、横切 Capability、共享模型和具体组件；
- capability 成立的消费者判据；
- descriptor-typed key、owned descriptor 和冻结 registry；
- 私有组件的 typed update；
- namespace/name/type 组成的 PropertyKey；
- Runtime 统一提交副作用；
- 外部能力发现与类型安全。

关键图：四类扩展对象的决策树。

### 第 15 章：原子性、故障与可验证演进

主问题：如何判断一个框架修改在失败、扩展和优化后仍然正确？

覆盖：

- prepare、validate、commit、发布；
- recoverable error 与 faulted state；
- 取消、补偿和结构半发布；
- 确定性 replay 与分层测试；
- 性能预算和证据驱动优化；
- API 稳定性与架构增量；
- Runtime 单一 owner、平台线程边界和异步结果回传。

关键图：正常提交、可恢复拒绝和故障锁定三条路径。

## 5. 每章写作模板

每章固定包含：

1. 一个真实问题；
2. 一条核心结论；
3. 最小概念模型；
4. 数据、身份和所有权；
5. 按时间顺序描述的算法；
6. 必须保持的不变量；
7. 失败与恢复；
8. 常见错误设计及其后果；
9. 与前后章节的连接；
10. 一组用于自检的思考题。

代码只用于澄清数据结构或算法，不提供逐文件实现教程。图必须展示因果顺序或所有权，
不能只做静态装饰。

## 6. 与另外两本书的同步

- 用户书可以引用本书解释“为什么”，但仍需独立给出可运行用法；
- 贡献者指南可以引用本书建立心智模型，但必须独立给出当前代码位置和验证命令；
- 本书只在核心模型或长期架构发生变化时更新；
- API 改名、文件移动、suite 调整通常不应触发本书修改；
- 三本书都不能替代 `doc/` 中的规范事实。
