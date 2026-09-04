# Figure 属性与样式契约

类型：`normative-design`

本文定义 Figure 通用视觉属性的所有权、继承和更新语义。具体 Figure 的点集、圆角、
文本内容和图像句柄不属于通用样式。

## 1. 所有权

`NodeState` 保存 `FigureStyle` 局部覆盖；具体 Figure 不复制通用 foreground、
background、alpha、font、cursor、tooltip 或 opaque 状态。

```text
FigureStyle       = 当前节点显式设置的可选值
ResolvedStyle     = 从当前节点向 root 解析后的确定值
NodeState.opaque  = 当前节点局部的绘制覆盖承诺，不继承
```

## 2. 继承

- foreground、background、font、cursor 和 tooltip 使用最近祖先值；
- alpha 使用最近祖先值，不沿父链相乘；
- 没有显式值时使用 root 默认值；
- opaque 不继承；
- visible 和 enabled 沿用各自既有的有效状态协议，不属于 FigureStyle。

默认值：

- foreground：黑色；
- background：透明；
- alpha：1.0；
- font：`12px sans-serif`；
- cursor：Default；
- tooltip：None。

## 3. 绘制

样式解析满足以下递推关系：

```text
Resolved(root)  = overlay(DefaultStyle, Local(root))
Resolved(child) = overlay(Resolved(parent), Local(child))
```

因此绘制是携带父级已解析状态的树折叠，不是彼此独立的节点查询。递归进入 child 时，
parent 的 resolved 状态已经存在于 graphics state 中；当前节点只需应用自己的 local
override。

递归绘制每个节点时：

1. push parent graphics state；
2. 只把当前节点的 FigureStyle local override 写入 graphics state；
3. 调用 Figure 的类型专属绘制 hook；
4. children 通过 graphics state 继承，得到与 ResolvedStyle 查询等价的绘制属性；
5. pop state。

Figure 的类型专属 paint 可以在自身 push/pop 范围内覆盖绘制参数，但不能修改
NodeState 或让状态泄漏到 sibling。

对每个节点重新遍历祖先链并不符合上述递推模型：它丢弃了遍历已经携带的父级上下文，
形成第二条重复的解析路径。其性能后果是深树退化为 O(n²)，但性能不是选择该模型的
首要依据。

`FigureTree::resolved_style` 用于 cursor、tooltip 和外部随机查询。若未来允许从任意
子树开始独立录制，应在子树入口解析一次 inherited style，再沿子树递推；不得让子树
中的每个节点各自回溯祖先链。

## 4. 更新

- foreground、background 或 alpha 改变：repaint 当前可见子树；
- font 改变：invalidate 当前子树并 repaint，因为内在尺寸可能变化；
- cursor 改变：若当前 cursor target 位于该子树，下一次 host effect 使用新值；
- tooltip 改变：不改变几何，更新 tooltip 查询结果；
- opaque 改变：repaint 当前节点，并保持 damage 正确性。

每个实际变化产生一个对应的 typed property event；相同值写入不产生通知或更新。

## 5. 边界

- CursorIcon 是平台无关枚举，平台 adapter 只负责映射为 native cursor；
- tooltip 保存内容描述，不在 FigureTree 内管理 popup 窗口；
- tooltip 局部值使用三态表达继承、显式关闭和本地文本；
- 字体在本阶段使用稳定描述值，资源解析和异步加载由 ResourceRegistry 承担；
- style 不包含 selection、hover、pressed 或业务状态。

## 6. 验证

- 每个属性按最近祖先独立解析；
- sibling 的局部覆盖互不污染；
- graphics command snapshot 反映 resolved foreground/background/alpha/font；
- inherited style 变化使后代进入正确的 repaint/invalidation 集合；
- cursor/tooltip 查询在 hidden、disabled、remove 和 reparent 后保持一致。
