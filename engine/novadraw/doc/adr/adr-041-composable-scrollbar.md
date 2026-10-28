# ADR-041：通用 ScrollBar 与 Figure 活动调度

类型：`architecture-decision`

## 状态

已接受并实现（2026-10-21，用户要求补齐 Draw2D ScrollBar 通用能力并迁移示例）。
属于官方示例 K04，`api_semantics` 为 `viewport.scroll_zoom` 与 `input.dispatch`。

## 来源与目标

只读参考 org.eclipse.draw2d 的 ScrollBar、ScrollBarLayout、RangeModel、
DefaultRangeModel 与 Clickable。ZoomExample 使用 Draw2D ScrollBar，
范围 minimum=0、maximum=200、extent=25、value=40，zoom=(value+10)×0.02。
不编写、编译或运行 Java。

ScrollBar 必须独立于 ScrollPane，采用可替换 RangeModel，包含普通 Figure
组成的五个部件：两个箭头、两个翻页区域和 thumb。thumb 可为 Label 或外部 Figure；
部件替换沿现有 subtree admission 与 typed component update，不增加特权挂载。
默认纵向，横向采用同一轴向布局；步进默认 10，翻页默认 50。
保留 f64 模型的连续几何，不把 Java 整数运算推广到 Core 全局坐标。

## 所有权与运行

ScrollBarFigure 持有实例范围状态、部件身份和拖动/按住会话。
ScrollBarLayout 只查询受检子节点与约束，生成完整 LayoutOutput。
RangeModel 变化在 Runtime 稳定边界同步到布局、可见性和 typed value 通知。
模型和视口可以共享，ScrollPane 仅负责消费滚动值，Zoom 仅负责消费缩放值。
绘制不修改模型，不创建节点，也不更新时间。

新增开放的 FigureActivity capability，为模型同步和时钟活动提供同一回调入口。
外部 Figure 与内置 ScrollBar 使用相同 typed capability registry、
EventContext 的只读查询/受检 effects 和 Runtime 单调时钟。
Runtime 在输入返回、稳定更新与 advance_time 边界处理活动；
next_wake_deadline 合并活动与已有 tooltip/animation deadline。
不采用后台线程、全局 timer 或动画表现值模拟输入连发。

按住箭头或翻页区立即改变值，等待初始延迟后按间隔重复；
默认时序对齐 Draw2D ButtonModel：初始等待 250ms，重复间隔 40ms。
超时只推进一次并从本次时间安排下一次，避免补发风暴。
release、离开活动区域、失去输入状态、隐藏、禁用、移除及 Runtime fault 终止重复。
thumb drag 使用 press 时的轴向位置、值和可移动距离；零距离安全不移动。
空范围或 thumb 不可容纳时隐藏 thumb，翻页区分摊轨道。
输入只消费左键。

## 外部消费者证据

独立 Rust integration target 验证自定义 thumb、替换部件、自定义 RangeModel，
以及外部 FigureActivity 的时钟/失败边界；不修改 Core 即可使用全部公开协议。
Zoom 删除示例私有 ScrollBar，ScrollPane 复用同一组件。
测试覆盖横纵布局、非零 minimum、短轨道、模型外部变更、拖动、按住连发、
release/exit/隐藏/禁用/删除、替换失败保留原部件、模型替换后的旧通知隔离。

## 交付与主线

先完成通用契约和外部失败反例，再实现 Core 与两个消费者；
运行定向测试、Native Zoom 截图与最终 workspace 门禁。
完成后在 P2 backlog 记录 K04 切片结果，再按依赖返回 K05 文字流能力。

## 实施边界

`ScrollBarFigure::into_subtree` 返回五个部件的描述身份；构造期可用 `with_part`
提供任意 Figure，运行期沿 `replace_child_subtree` 原子替换，并保留 typed
`ScrollBarPart` 约束。部件的 minimum measurement 同时控制布局和拖动比例。
`SetScrollBarModel` 通过普通 component update 替换模型并取消旧输入会话；
同步读取当前模型，不向旧模型注册持久 listener。
`ScrollBarValueConsumer` 在事件 effects 阶段消费值；Zoom 只提交 scale，
ScrollPane 只提交 viewport 通知与 damage。

模型在输入、显式时钟推进或帧稳定边界同步；模型本身不唤醒宿主。
外部程序直接修改模型后，需要请求帧或推进 Runtime。
活动时钟由既有 animation facade 驱动，与动画表现状态分离。

已通过独立 `p2_scroll_bar` 的九项契约、`m8_viewport_contract` 的 26 项回归
和 Zoom 两项交互测试。Native Zoom 截图确认五部件布局与 Label thumb。
K04 的 Path/MultiLineLabel 消费者仍依赖后续 K05/K08，不据此关闭整批。
最终 workspace fmt、Clippy 与 test 通过：1,164 项通过、1 项忽略。
