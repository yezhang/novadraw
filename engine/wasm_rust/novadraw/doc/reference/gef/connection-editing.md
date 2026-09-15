# GEF Connection EditPart 与连接编辑语义

类型：`reference-analysis`

参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`

参考基线：Eclipse GEF Classic commit
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。

本文只记录 GEF Classic 源码事实，不定义 Novadraw 行为。

## 1. 连接不是模型 containment child

`ConnectionEditPart` 将连接描述为 source/target EditPart 的结构特征。应用模型可以用
任意方式保存连接，但端点 EditPart 需要分别通过
`AbstractGraphicalEditPart.getModelSourceConnections()` 和
`getModelTargetConnections()` 暴露连接模型。

连接可以独立选择和删除，也可以有标签等 child EditPart。GEF 还允许连接本身成为
其他连接的端点。

源码：

- `org.eclipse.gef/src/org/eclipse/gef/ConnectionEditPart.java`
- `org.eclipse.gef/src/org/eclipse/gef/editparts/AbstractGraphicalEditPart.java`

## 2. 两端发现、Registry 去重

`refreshSourceConnections()` 与 `refreshTargetConnections()` 分别对比模型列表和当前
关系列表，执行保留、重排、创建和删除。两端都可能先发现同一个连接，因此
`createOrFindConnection(model)` 先查询 Viewer 的 model-to-EditPart registry，只在
没有现存 ConnectionEditPart 时调用 factory。

由此得到的稳定语义是：

1. source/target 两侧都可以触发发现；
2. 一个连接模型在一个 Viewer 中只对应一个 ConnectionEditPart；
3. source 和 target 关系分别有稳定顺序；
4. 列表刷新不会把连接当作普通 child；
5. 模型身份是两侧发现结果的汇合键。

## 3. 生命周期与图层

`AbstractConnectionEditPart` 保存 source 和 target EditPart。任一端首次建立时，
ConnectionEditPart 获得 root owner 并执行 `addNotify()`；其 Figure 直接加入
`CONNECTION_LAYER`，而不是任一节点的 content pane。

只有 source 和 target 都存在时才刷新连接。两端都解除后，ConnectionEditPart
执行 `removeNotify()`，从 connection layer 移除 Figure，并清空两端 anchor。

source 节点激活/停用时会联动其 source connections。端点 EditPart 移除通知时会
解除仍指向自己的连接关系。

源码：

- `AbstractGraphicalEditPart.activate()` / `deactivate()` / `removeNotify()`
- `AbstractConnectionEditPart.addNotify()` / `removeNotify()`
- `AbstractConnectionEditPart.setSource()` / `setTarget()`

## 4. Anchor 责任

`NodeEditPart` 是可选能力，为已提交连接和交互 feedback 分别提供 source/target
anchor。`AbstractConnectionEditPart` 在 refresh 时向端点 NodeEditPart 查询 anchor；
若端点不实现该能力，则退回基于 Figure 的 ChopboxAnchor。

这意味着：

- anchor 由端点语义决定，不由 Connection Figure 猜测；
- anchor 可以依赖连接模型、节点模型或交互 Request；
- 已提交连接与临时 feedback 复用同一端点能力，但生命周期不同。

源码：

- `org.eclipse.gef/src/org/eclipse/gef/NodeEditPart.java`
- `AbstractConnectionEditPart.getSourceConnectionAnchor()`
- `AbstractConnectionEditPart.getTargetConnectionAnchor()`

## 5. 创建与重连

`CreateConnectionRequest` 是两阶段请求。第一端锁定 source 和 start command，第二端
补充 target 并形成最终 command。`AbstractConnectionCreationTool` 在 source
失活、焦点丢失或非法输入时取消反馈和手势。

`ReconnectRequest` 固定一个 ConnectionEditPart 和待移动端点。Endpoint tracker
在拖动期间持续更新 target、feedback 和 command，提交前先清除 feedback。

模型 Command 负责创建、断开和重连，EditPart/Figure 不直接修改业务连接。

源码：

- `org.eclipse.gef/src/org/eclipse/gef/requests/CreateConnectionRequest.java`
- `org.eclipse.gef/src/org/eclipse/gef/requests/ReconnectRequest.java`
- `org.eclipse.gef/src/org/eclipse/gef/tools/AbstractConnectionCreationTool.java`
- `org.eclipse.gef/src/org/eclipse/gef/tools/ConnectionEndpointTracker.java`
- `org.eclipse.gef/src/org/eclipse/gef/editpolicies/GraphicalNodeEditPolicy.java`
- `org.eclipse.gef/src/org/eclipse/gef/editpolicies/ConnectionEndpointEditPolicy.java`

## 6. Auto-expose

TargetingTool 在连接创建、重连和普通 drag 的 hover 阶段查找
`AutoexposeHelper`。`ViewportAutoexposeHelper` 只在指针位于 viewport 内部边缘带时
生效，并依据时间增量推进滚动。helper 返回值只是继续调度提示，Tool 可在 release、
cancel 或目标变化时随时停止。

源码：

- `org.eclipse.gef/src/org/eclipse/gef/AutoexposeHelper.java`
- `org.eclipse.gef/src/org/eclipse/gef/editparts/ViewportAutoexposeHelper.java`
- `org.eclipse.gef/src/org/eclipse/gef/tools/TargetingTool.java`

## 7. Self-loop 路由边界

GEF 允许 source 与 target 指向同一个 EditPart，但 Draw2D
`ConnectionRouter.NULL`/`NullConnectionRouter` 只请求两个 anchor location。相同 owner
的 ChopboxAnchor 互相使用同一中心 reference 时会退化为重合点。`FanRouter` 对
start/end 相等的 route 也直接返回，不生成回环。

因此 GEF 的“self-loop 模型合法”不等于默认 router 会产生可见回环；应用需要显式
router、bendpoint 或自定义 anchor 策略。

源码：

- `org.eclipse.draw2d/ConnectionRouter.NullConnectionRouter`；
- `org.eclipse.draw2d/BendpointConnectionRouter`；
- `org.eclipse.draw2d/FanRouter.handleCollision()`。
