# Tree 动画需要表现几何

类型：`proposal`

状态：待评审，未实施。承接 K06 与 ADR-026 的领域消费者目标。

`api_semantics`：`animation.presentation`、`animation.timeline`、
`layout.manager`、`coordinate.conversion`、`viewport.scroll_zoom`、`damage.repaint`。

## 上游按 bounds 重放布局

官方 Tree 动画只在开启 Use Animation 后用于展开与折叠，默认关闭。上游
`examples/tree/Animation.java` 捕获每个容器的子节点 bounds，按 210 ms 进度分别
插值位置和尺寸；`TreeBranch.animationReset` 把后代初始位置收拢到被操作节点。
连接线根据当前节点和内容区几何绘制；每步按被标记节点的位置差更新 viewport，
使操作节点在屏幕上保持稳定。

上游通过改写 Figure bounds、重复 validation 和同步 while 循环实现。这些做法
不能直接搬进 Novadraw：源状态必须一次提交，布局只计算最终结果，Host 驱动
Timeline，表现状态不写 source bounds、通知、撤销历史或选择状态。

现有 `BoundsTransition` 用仿射变换把最终 Figure 内容映射到旧 bounds。它能完成
普通位置/尺寸过渡，却会同时缩放文字和描边，不能表达 Tree 的子节点 bounds
重放；零尺寸折叠还会失去可逆映射。仅对现有 branch 调用该 API 不足以完成 K06。

## 增加 Runtime 拥有的表现几何快照

建议将真实 bounds 与显示可见性作为独立的 typed presentation channel。
局部 transform channel 保留变换含义，不再承担 Tree 的 bounds 重放。
每帧采样后生成一次不可变的表现几何快照，显式关联目标 FigureId 和捕获版本。

同一快照供以下路径读取：

- 递归绘制的节点 bounds、父子坐标映射与 child clip；
- Figure 自内容准备和几何相关的表现 provider；
- Tree 线条的端点计算；
- viewport 对被操作节点的跟踪；
- 新旧表现 envelope 的 damage。

普通 `FigureTree` 查询继续返回已提交几何。表现快照不替换树，也不参与布局测量。
只有绘制准备阶段的只读 geometry view 能访问采样值；它不提供 mutation、model
或 layout 调用入口。所有消费者共享这一视图，避免各自叠加坐标偏移。

快照仅覆盖捕获目标及其必要祖先，按显式稳定顺序持有身份。Timeline 逐帧采样既定
轨道，不执行全树 discovery。没有轨道时直接使用已提交几何。

## 线条使用同一个几何视图

Tree 的 Normal/Hanging 线条属于示例算法，通过普通 typed capability/provider
注册。provider 从采样后的 title、contents 和子 branch bounds 派生端点，并输出
不可变 self-content presentation。

Core 只提供读几何和发布 drawing 的扩展协议，不匹配 Tree 名称或 BranchStyle。
provider 不调用 LayoutManager，不改写 Lines 的已提交数据，不保存私有 timer。
样式切换仍按静态最终结果处理；第一切片不扩张为所有样式的自动动画。

每个采样时刻的文字大小、描边宽度保持原值，只有 bounds 和端点位置变化。
嵌套容器只使用一次父子坐标映射，禁止同时把 surface 位移与局部位移叠加到同一节点。

## 折叠保留显示，交互立即使用终值

折叠先提交 expanded=false 和最终布局。表现可见性轨道允许仍存活的后代在过渡中
继续绘制，终止时丢弃轨道，回到已提交隐藏状态。展开先提交 expanded=true，
从收拢到操作节点的几何过渡至最终布局。零尺寸 bounds 是合法样本，不通过除以
宽高构造 transform。

默认命中、焦点、选择和 accessibility 使用已提交状态。折叠中的后代即使还在显示，
也不能继续命中。表现显示不恢复源树的交互资格。

本切片只动画展开/折叠。删除 Figure 时取消整个关联计划，不通过复用身份保留
已释放节点。后续 enter/exit 临时树视觉如需要，另用已有 lifecycle 目标定义捕获与
资源 lease；不能把动态 source 节点伪装成 temporary visual。

## Viewport 跟踪与布局共用一个进度

捕获操作节点在 viewport 内容域中的起点，计算最终布局中同一节点的位置。
最终 viewport origin 先按该位移提交，再用相同 Timeline 进度采样：

```text
sampled_origin = initial_origin
               + sampled_marked_node_position
               - initial_marked_node_position
```

两轴同时计算，随后应用既有 extent/clamp 规则。目标是保持操作节点的屏幕位置，
不要求 viewport 在 extent 缩小时越界。动画终止后保留最终已提交 origin。
直接拖拽/滚动打断跟踪计划，使用已有 viewport 输入仲裁；不重新写回旧 origin。

## 取消与失败使用既有组语义

bounds、表现可见性、线条 provider 与 viewport 在同一计划中提交。预检身份、
轨道数、快照预算、provider 与坐标数值后才激活，任何失败都不留下半组覆盖。
Source mutation 已提交后的动画失败保留静态最终场景，不声称回滚外部副作用。

Replace 从当前表现值 retarget；Cancel、Disabled、ReducedMotion、dispose、
suspend/resume 与 budget fallback 均丢弃或暂停同一个计划，不留下线条或 viewport
的独立 owner。静态零尺寸 fallback 已修复并有回归，此设计不缩减该契约。

## 实施与退出证据

| 顺序 | 切片 | 必须证明 |
|---|---|---|
| T-1 | bounds/表现可见性与只读几何快照 | 零尺寸、嵌套坐标/clip、源终值不变、damage、取消与外部 provider |
| T-2 | Tree Normal/Hanging 线条消费者 | 任意采样时刻端点对齐，文字与描边不缩放，不逐帧布局 |
| T-3 | 展开/折叠与 viewport 同计划 | 操作节点屏幕跟踪、双轴 clamp、重复反向、直接输入、释放和静态等价 |
| T-4 | 官方 19 节点入口 | Use Animation 默认关闭；真实面板、Native GPU 中间帧与恢复，完整资源/帧门禁 |

独立测试 crate 的自定义容器和线条 Figure 必须能注册同样的 channel/provider，
不改 Core 的能力分派。对等测试比较 committed bounds、采样几何、glyph 字号、
线端点与 viewport，而不以 active track 数量代替绘制证据。

该设计涉及渲染使用的几何协议。实施前需要评审，不直接扩张现有 affine bounds
实现。渲染主循环如需调整，必须限定为从统一只读几何视图取值；不得增加第二套
遍历或为 Tree 增设快路径。

依据：[ADR-026](../../adr/adr-026-animation-and-presentation-plane.md)、
[领域能力矩阵](capability-integration.md)、
[坐标规范](../coordinates/coordinate-system.md)。
