# GEF Direct Edit 与 Draw2D 文本交互源码语义

类型：`reference-analysis`

源码基线：Eclipse GEF Classic commit
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

范围严格限定为 `org.eclipse.gef` 与 `org.eclipse.draw2d`。

## 1. 证据入口

GEF：

- `org.eclipse.gef.requests.DirectEditRequest`
- `org.eclipse.gef.editpolicies.DirectEditPolicy`
- `org.eclipse.gef.tools.DirectEditManager`
- `org.eclipse.gef.tools.CellEditorLocator`
- `org.eclipse.gef.tools.SelectEditPartTracker`
- `org.eclipse.gef.tools.DelayedDirectEditHelper`
- `org.eclipse.gef.ui.actions.DirectEditAction`

Draw2D：

- `org.eclipse.draw2d.text.FlowFigure`
- `org.eclipse.draw2d.text.TextFlow`
- `org.eclipse.draw2d.text.ParagraphTextLayout`

行为样例：

- `org.eclipse.gef.examples.flow.parts.ActivityDirectEditManager`
- `org.eclipse.gef.examples.flow.policies.ActivityDirectEditPolicy`
- `org.eclipse.gef.examples.logicdesigner.edit.LabelDirectEditPolicy`

## 2. 启动与目标

Direct edit 是作用于单个 EditPart、单个 feature 的编辑会话，不是 Viewer selection
本身。GEF 提供三种启动路径：

1. 已选中的 EditPart 再次单击，经过双击时间延迟后发送 `DirectEditRequest`；
2. action 在恰好选择一个且理解 direct-edit request 的 EditPart 上执行；
3. 应用直接向 EditPart 发送带 feature/location 的 request。

延迟启动期间，新的鼠标、键盘或焦点事件会取消待启动请求。Request 的 location 用于
确定命中的可编辑 feature；feature 用于区分同一 EditPart 的多个可编辑属性。

## 3. 会话生命周期

`DirectEditManager` 拥有一次会话的临时 UI 和监听器：

```text
show
-> create CellEditor
-> initialize value and validators
-> activate and focus
-> locate over target Figure
-> show source feedback
-> value changes update feedback and placement
-> apply commits / cancel tears down
```

Figure ancestor 移动时重新定位 editor；control resize/move 时重新定位 feedback frame；
EditPart 停用时强制结束。`bringDown` 必须可重入，统一清除 feedback、监听器和平台
control。

GEF 的 `CellEditorLocator` 只负责平台 editor 的几何放置，不决定文本内容、模型提交或
Figure 布局。

## 4. Preview 与提交

`DirectEditPolicy` 分离两类行为：

- `showCurrentEditValue` 把草稿显示为临时 feedback，使目标 preferred size 能跟随；
- `getDirectEditCommand` 从最终值生成修改模型的 Command。

提交顺序是：

```text
erase/revert feedback
-> obtain Command from source EditPart
-> execute through CommandStack
-> model notification refreshes stable Figure
-> dispose session
```

取消只撤销 feedback，不生成 Command。草稿变化不能直接写入业务模型；历史记录中也不
保存 CellEditor、EditPart 或 Figure。

## 5. TextFlow 交互几何

Draw2D `TextFlow` 在布局 fragment 上提供：

- point 到文本 offset 与 trailing position 的命中；
- offset 到绝对 caret placement；
- 上/下行最近 offset；
- 行首、行尾及前后可见 offset；
- selection range 的逐 fragment 绘制；
- bidi run 中 logical offset 与 visual position 的转换。

offset 与 trailing/affinity 必须共同表达换行边界和 bidi 边界上的 caret 位置。选择
几何来自已经完成 shaping 和 wrapping 的 fragment，不得重新用平均字符宽度估算。

Draw2D 把 selection 字段放在 Figure 中是其轻量 widget 方案，不代表业务文档 selection
应进入持久模型。Novadraw 只采用其布局查询语义，不复制可变 Figure selection 字段。

## 6. 平台绑定的局限

GEF Classic 直接创建 SWT/JFace `CellEditor`，因此输入法、焦点、候选窗和原生控件
生命周期由 SWT 承担。该实现不能直接作为跨 Native/Web 的公共契约：

- 平台 control 类型穿透 manager；
- 草稿值通过 `Object` 和 CellEditor 读取；
- Web 不存在对应 SWT control；
- 输入法 composition 没有进入 GEF 自身的 typed request。

Novadraw 应采用 manager/policy/command 的职责划分，但以平台无关的文本会话和 host
input bridge 替代 SWT CellEditor。
