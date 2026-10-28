# Tree 表现几何实施复核

类型：`architecture-review`

日期：2026-10-26

状态：T-1 至 T-4 退出验证通过；窗口 surface present 独立保留未通过。

对象：[修订 3](../../design/animation/tree-layout-presentation-proposal.md)与
[ADR-046](../../adr/adr-046-sampled-geometry-and-animation-reservations.md)。
本文记录实现证据，不改写前三轮设计评审的原始判断。

## 实现边界

Core 新增 typed 真实 bounds、可见性、origin、冻结内容与 bounds-dependent 自绘，
通过统一几何视图提供 placement、client/child mapping、clip 和 envelope。
`render_recursive.rs` 保持 paint 顺序，只调整几何/冻结绘图读取；递归深度仍为
10,000。无活动动画时使用原 committed 路径，不构造表现视图。

Tree 的 19 节点模型置于普通 Viewport，Use Animation 默认关闭。展开/折叠先稳定
旧状态并 capture，再提交最终布局；非零收拢 endpoint、可见性保留、线条与 origin
属于同一计划。Normal/Hanging 共用静态端点函数，sample 不写 Lines 或运行布局。
PageNode 与折角边框通过公开 bounds snapshot 扩展绘制。

## 契约证据

| 批次 | 公开入口与断言 | 证据 |
|---|---|---|
| T-1 | 同 Rectangle 多语义、四侧 inset、scale/rotation、零 client clip、OverflowVisible、冻结字形/资源、循环/查询/panic/越限和静态修复 | `p2_animation_geometry_contract` |
| T-1 / F01 | Unanchored、advance 后未录制的 capture、当前 E 输入接管、在途包不变、Retry/reset 不回退 S | 同上与 `p2_animation_sample_reservation_contract` |
| T-1 / F02 | 两组交换顺序、容量恰满、净替换、staging 拒绝、配置下调、capture/submission/recovery 独立留存 | 同上 |
| T-2 | Normal/Hanging、多层静态线条与动画首帧逐项相同、字形不重排、provider 有界查询及输出 | Tree 示例与 Core integration targets |
| T-3 | 双方向所有后代中心收拢、反向捕获当前采样、首次展开与嵌套隐藏、增量输入取消整组、旧 transform 冲突拒绝 | Tree 示例、geometry 与 `m8_viewport_contract` |
| T-4 | dependency dispose、外部 hide 三种 policy、surface pause/resume、静态模式提前抑制、资源 release 与工作量 | 同上 |

新增回归发现并修复两项边界问题：只有 origin 轨道、无 provider 的计划原先未拒绝
旧 contents transform owner；现在 child-transform 候选将冻结闭包纳入冲突检查。
关闭动画的首次操作原先可能在初始布局稳定前隐藏节点；现在 toggle 在任何模式下
先稳定源状态，保证静态与动画模式得到相同的最终几何。

## Native 像素与实际工作量

稳定跨工具入口为 `cargo xtask verify animation.tree`。其顺序覆盖 Core geometry、
采样预留、Viewport、Tree 示例，最后运行 `tree-animation-proof`。
Native 在窗口 RedrawRequested 后使用 Vello 的真实 GPU retained texture 绘制和截图。
本机尝试真实 `submit` 时，基线帧仍返回 Skipped；因此这里不声明 surface present
通过。surface 呈现属于发布准备的独立平台证据，本次 T-4 核验 GPU 像素与擦除。
复核还发现旧截图入口始终重建整张纹理，不能证明 retained repair。截图入口现已
按有效 damage 编码 scratch texture，并只复制 damage regions 到 retained texture；
修正后已重新验证通过，旧整图截图结果不作为 partial 退出证据。中间帧和最终清理帧
同时断言提交的 `DamageMode::Partial`，排除整图重绘掩盖修复遗漏。

固定时刻为 0、105,000、209,999、210,000 微秒，覆盖横向/纵向和 DPI 1/2。
中间帧及最终 retained-partial 图像必须与独立 Full 重录逐像素相同；
首帧与旧静态帧相同，完成后与默认关闭、Disabled、ReducedMotion 的静态帧相同。
产物目录为 `target/verification/tree-animation/`。

`geometry-work.csv` 记录各活动 E 最终几何遍历的实际 nodes、edges、matrices、clips、envelopes。
横纵方向和 DPI 1/2 均为 79 节点、78 边、1,181 次矩阵/角点操作、213 次 clip、
157 次 envelope；整个 E 还包括固定次数的基础、provider 绘图阶段重建与最终遍历。
重叠 nested targets 的 8/32/128 节点回归断言每个节点只访问一次，
祖先与后代 target 不重复扫描。provider 用 HashSet 校验声明依赖，端点链与输出均
按实际边/线段计数；通用扩展内部的任意 CPU 工作不属于 Core 可约束的执行时间。

## 最终门禁

retained repair 修正后的 Native GPU 40 幅截图比较通过；窗口 surface present 保留未通过。
公开 API 的 host/wasm rustdoc 边界通过，新增符号只归 animation/figure 领域，
Runtime 方法预算保持 28/28；workspace fmt/check 与严格 Clippy 通过。
首次退出验证时，`animation.tree` 的 94 项定向回归及 Native GPU 图像验证通过。
当时 workspace test 为 1,472 项通过、0 失败、6 项既有忽略；374 份 Markdown、
三本书的 HTML/PDF 构建和 `git diff --check` 通过。T-1 至 T-4 已关闭。
中文复读与机械标点扫描完成：四处字面命中均为 Mermaid 的 `-.文字.->` 连线
语法，书稿三处、本文示例一处；正文半角标点命中为零。技术术语和原有查阅编号保留。
AA、混合字体装饰及其他 M01-C consumer 不在本次关闭范围。

## Native 双击输入补充修复

人工复查发现默认关闭动画时，真实窗口双击没有展开或折叠。根因是共享 winit 宿主
只转发 Pressed/Released，没有识别和投递 DoubleClicked；原有 Tree 回归直接调用
Core 入口，没有覆盖这一桥接缺口。上文的几何与 GPU 证据保持原范围，不能作为
真实鼠标双击链路已经验证的证明。

平台新增 `WinitPointerAdapter`，共享宿主统一调用它处理按钮输入。识别使用调用方
提供的单调时间和逻辑坐标，按 Pressed → DoubleClicked → Released 投递；离窗、
失焦、场景切换、面板接管与滚动会中断点击序列，拖拽不能被识别为双击。
默认 500 ms / 4 逻辑像素为可替换 fallback，没有查询 OS 的双击设置。
该修复没有新增依赖、资源或 Core/Wasm 入口；识别使用固定大小的局部状态和 O(1)
比较，不扫描 FigureTree，也不推进 Runtime 动画时钟。

新增平台按钮序列、共享宿主桥接和 Tree 动画开/关回归。Tree 消费者回归通过平台
适配器输入 winit 按下/释放，断言折叠、再展开及后代源可见性，不直接调用 Core
双击入口。稳定 `animation.tree` 已纳入这三层检查，99 项定向回归与 40 幅 GPU
图像比较通过。修复后 workspace fmt/check、严格 Clippy 与 test 通过：
1,477 项通过、0 失败、6 项既有忽略；374 份 Markdown 与差异空白检查通过。
本次没有 Desktop 自动操作工具，未把程序化按钮序列回归表述为物理鼠标人工验收。

## 浮动面板输入归属补充修复

用户再次复查时，TreeExample 单击无法选中，双击也无法折叠。前一次修复补齐了
双击识别，却遗漏了此前的面板输入分流：egui 0.35 在 Background 层缺少 root Ui
可用区域时，将画布悬停也判断为 egui 区域。原 PanelHost 只用 begin/end pass
绘制浮动 Window，没有创建 root Ui；egui-winit 因此消费按钮事件，共享宿主在
Core 投递前返回。此前绕过面板的按钮序列测试无法发现这一问题。

共享 Panel 改用 `Context::run_ui` 注册 root Ui 可用区域，PanelHost 与回归共用
`Panel::prepare_ui`。每帧最多一轮 pass，保持 Runtime 控制动作只执行一次。
修复不新增依赖、资源、树遍历或 Core/Wasm 行为。

旧实现下的画布输入归属回归已复现失败，修复后通过；同时确认浮动面板自身仍接收
鼠标输入，控件请求布局重试不会重复执行回调。Tree 回归创建实际生产面板，按
egui-winit 使用的输入归属查询分流，再将 winit 按下/释放序列交给平台适配器。
断言单击选择及文字样式、双击折叠与再展开、后代源可见性，覆盖动画关闭和开启。
`animation.tree` 已加入面板回归；这些程序化检查仍不代表物理鼠标人工验收。

修复后的 `animation.tree` 为 104 项通过、0 失败；Native GPU 40 幅图像比较通过。
workspace fmt/check、严格 Clippy 和 test 通过：1,480 项通过、0 失败、
6 项既有忽略；374 份 Markdown 与差异空白检查通过。
README 与本文已按好好说话规范复读，输入拦截的因果顺序和原有验收范围均保留。
机械扫描中，半角标点仅命中本文原有的内联 Mermaid 连线语法，按代码豁免；
中文正文半角标点、破折号、元话语和对比句式命中均为零。
