# 官方示例剩余契约方案

类型：`verification`

日期：2026-10-19

状态：待评审，未批准 Core API 变更

## 已确认的边界

目标仍是 25 个官方入口在 Figure 构成、元素类型和功能上的对等。所有具体行为从
`org.eclipse.draw2d.examples` 及其调用的 Draw2D 基础类取证；不引用已有业务场景，
不使用 Zest。当前审计见
[对等账本](../reviews/draw2d-official-examples-parity-2026-10-16.md)。

TreeExample 已移植十九个节点、方向/对齐/样式/压缩/间距、删除和非动画折叠。
More Children、Use Animation 和原生选择样式尚未对等。现有 EventEffects 可以添加
单个 Figure，不能取得新 ID 后继续构造其 AnimatingLayer、PageNode 和 Label。
FigureLifecycle 回调也不能写 Runtime。禁止通过画一个复合节点替代真实子树。

Core 已有 bounds/viewport transaction 和 timeline，不需要另建动画引擎。
这些同步 API 无法在持有 Figure 事件借用时调用。官方动画还要求让隐藏子树从
所选节点中心展开、反向收起，并跟踪视口；普通旧/新 bounds transition 不能直接
覆盖其全部语义。

ObstructionExample 使用 graph.ShortestPathRouter。已执行官方单障碍双路径探针：
障碍 `(40,20,20,60)`，端点分别为 `(0,50)→(100,50)`、
`(0,55)→(100,55)`，得到：

```text
(0,50) → (36,83) → (63,83) → (100,50)
(0,55) → (32,87) → (67,87) → (100,55)
```

斜线和共享拐角间距均是算法效果。现有正交 ShortestPathConnectionRouter 不具备
该效果；修改 clearance 或 bend penalty 无法消除算法差异。

## 建议的实施切片

### 动态构造真实子树

先设计 construction-only 的 owned subtree 描述。子节点采用暂存身份，挂载时由
Runtime 分配真实 FigureId，统一预检深度、能力、布局约束和资源 namespace。
绑定布局与场景模型需要显式 ID 映射结果，不能把任意 Runtime 闭包塞进生命周期。

同步挂载与事件延迟挂载应共用同一预检/提交路径。失败不发布半棵树，也不改
TreeModel 的 children；成功后才更新外部模型。不扩大事件回调的可变借用权限。

验收消费者是 More Children：新分支继承所选父分支 style，内部依次为
AnimatingLayer、PageNode，后者使用 StackLayout 和单个 `child` Label。
外部测试 crate 必须使用相同 API，不能为内置 Figure 增加专用构造分支。

### 事件期领域事务

先评审延迟 subtree mount 与延迟 animation transaction 的请求/结果契约。
请求必须 owned、typed、可预检，使用现有 pending mutation 和 Runtime 稳定边界；
禁止提供没有错误/资源/退出约束的通用 `FnOnce(&mut Runtime)` 事件逃逸口。

TreeExample 动画使用实例级状态，沿用官方 210ms、线性整数舍入、中心 reset、
反向收起和 viewport tracking。动画关闭时 committed layout、命中和通知必须与
非动画操作一致。若当前 presentation 无法表达中间布局连线，先定义该差额，
不能用渐隐或简单缩放替代官方连线运动。

### 官方绕障算法

将官方 Path/Obstacle/Vertex/Segment 和 ShortestPathRouter 分成独立算法实现，
数据只使用 Novadraw 几何值，实例持有状态。挂接现有开放 RouterRegistry，保留
稳定组快照、增量失效及原子路由提交。现有正交路由继续保持独立身份。

先跑 Java/Rust 相同 fixture 的路径点对照，覆盖多路径共享拐角、障碍移动/删除、
路径删除和确定性顺序，再移植 ObstructionExample 的按键与生命周期。
官方删除 paired ellipse 的 parent mismatch 单独记录并复现，不擅自修正。

### 原生元素与其余缺口

明确 SWT Group/Button/Scale/Combo/Text/Caret/双 Canvas/Printer 的目标宿主边界。
若只用 Figure 控件，必须证明行为与构成契约完整，并记录平台外观差额。
富文本片段、图算法、第二 live projection 和打印等已有 deferred/not_started
增量逐项评审，不因本次示例要求自动改写历史完成状态。

## 验证与退出条件

每个切片先定义契约、补外部消费者证据和失败测试，再实施。修改内环只运行对应
crate/target；切片完成运行对应 suite，最终交付执行 workspace fmt/Clippy/test。
升级 complete 前补官方源码映射、真实事件测试以及同尺寸/同字体双侧视觉证据。

本方案只提交可评审的边界与执行顺序，未定义的新 API 名称、内部数据结构和
animation presentation 策略应在对应 ADR 评审中确定。
