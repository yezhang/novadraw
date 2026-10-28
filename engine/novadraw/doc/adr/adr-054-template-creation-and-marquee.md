# ADR-054：类型化模板创建与框选

类型：`adr`

状态：Accepted。2026-10-28，依据已批准的 GEF 官方样例对等方案和用户继续完成
Shapes 的授权。实施状态只在 P2 backlog 维护。

## 契约

`CreationTemplate<A>` 提供默认尺寸和关联的开放 RequestProtocol。CreationTool
负责点击/拖拽、坐标转换、预览清理和提交时机；模板负责构造 typed payload 与
目标元数据。平台模板拖放调用 `EditorDomain::drop_template`，使用同一模板、
策略注册表和命令事务。业务类型不进入 Editor 的闭合枚举。

创建预览不改变模型。释放主键提交一次命令，取消、失焦和换工具清理预览。
成功后回到 SelectionTool。点击使用默认尺寸，拖动使用规范化矩形；应用策略负责
最小尺寸和合法性。当前创建工具的坐标域为 model contents，嵌套容器的父域转换
不属于 Shapes 的需求，扩展模板不得把 contents 坐标假定为任意父节点坐标。

MarqueeTool 在 surface 域判断完全包含的节点，按 containment 顺序提交选择。
Shift 追加，Control/Meta 切换，普通框选替换。取消保留原选择。连接和 contents
不参与此默认节点框选模式。Viewer 提供批量预校验选择与只读包围查询；第三方工具
可通过同一个 ToolContext 实现其他框选方式。

应用命令与保存点分别通过 `execute_command` 和 `mark_save_location` 接入
现有 CommandStack。文件写成功之后才标记保存点；打开文档建立新的模型和历史。

连接装饰在 VisualBuildContext 中注册 Figure 与 locator，沿用 Runtime 连接子节点
协议。Router 的注册、绑定、解绑与移除保持显式所有权，避免增删障碍物时泄漏旧 Router。

Tool 通过 `accepts_handle_activation` 决定是否接受 Viewer 的 handle 工具切换；
默认不接受，SelectionTool 接受。创建、框选和其他第三方模态工具因此不会被已有
selection handle 截断，不按具体内置 Tool 类型建立分支。
重连重新绑定连接状态时，同时重新挂载该 visual 拥有的 locator；
装饰 Figure 和连接身份保持不变。

Masonry 模板源保留 typed payload，点击激活与窗口逻辑点 drop 共用一个 action。
CanvasView 可安装应用键盘回调，未处理的可表示键继续交给 Tool；
快捷键不进入 Core 的闭合业务枚举。滚轮、中键平移和 auto-expose 使用现有
EditorDomain 视口契约。错误、会话身份、选择和稳定快照参与 action 去重，
pointer feedback 不发布外层 action。绘制成功只清除绘制错误，不吞掉命令错误。

## 对标与成本

GEF `CreationTool`、`MarqueeSelectionTool` 是静态源码参考；不执行 Java。
框选扫描节点为 O(N)，批量选择预校验后提交；创建每次预览走现有 policy/feedback
路径，不引入新依赖或内置资源。Shapes 的障碍路由复用 Core shortest-path，
节点集合变化时更新障碍身份，几何变化由 Runtime 失效传播处理。
应用序列化依赖和最终 Native 产物成本在样例验收中记录。

## 外部消费者证据

`gef-editor-demo` 在独立 crate 实现 ShapeTemplate，关联 CreateChild；
Editor integration consumer 另定义独立 Request marker、payload、policy 和 template，
验证工具与 drop 共享全部派发、错误与事务通道。Marquee 的测试从消费者侧使用
公开 ToolContext/Viewer 查询与 selection 接口。
具体通过情况与命令记录见
[Shapes 验证报告](../verification/reviews/shapes-editor-implementation-2026-10-28.md)。
