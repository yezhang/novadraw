# Logic 预设、相对折点与路由切换

类型：`implementation-verification`

能力归属：应用消费者（`gef-logic-demo`）。
本轮复用 Editor 的 Command、EditBendpoint、Tool、Handle 与 Viewer 投影，
以及 Core 的锚点、坐标和路由；没有修改 Core／Editor 公共 API。
契约见 [Logic 切片](../../design/editor/gef-logic-first-slice.md)。

## 预设与官方对应

对照 GEF Classic 的 `LogicDiagramFactory.createHalfAdder/createFullAdder/createLargeModel`
读取门与导线关系。以展开拓扑展示边界端口透传后的连接：

| 预设 | 节点／导线 | 行为 |
|---|---|---|
| 半加器 | 6／6 | 两个 LED 输入，经 XOR／AND 到 Sum／Carry |
| 全加器 | 9／12 | 两个半加器与一个 OR；LED 提供 A、B、Carry in，结果 LED 位 0 为和、位 1 为进位 |
| 四位加法器 | 20／38 | 最低位半加器，后三位全加器串联；初始 3 + 7 = 10，显示低四位 |

布局采用从左到右的传播顺序，切换预设时自动缩放到画布。
未保存状态需要确认丢弃。Live／Ground 提供固定高／低电平源。
该展开视图保留门组合与进位关系；没有实现官方可编辑 Circuit 子树及其八对
边界端口，LG-01～04 的完整嵌套预设验收仍未关闭。

## 折点与路由

路由模式进入文档与撤销历史。Manual 使用 FanRouter 包装 BendpointConnectionRouter；
Manhattan 使用正交路由；Shortest path 使用已有可见性图路由及节点障碍身份。
此前正交 shortest router 的端口 owner 与祖先障碍不一致问题，不靠忽略全部障碍
规避；本轮使用可见性路由已有的端点位于障碍内部语义，并验证中间节点避让。

相对折点保存两端参考点偏移，权重为 0.5。Policy 读取 routing 域坐标，
通过 EditBendpoint 处理新增、移动、删除；工具释放时一次提交。
Manual 提供线段中点创建手柄与折点移动手柄，侧栏提供逐点删除。
自动路由取消未提交手势、移除折点手柄、拒绝折点命令，同时保留已提交数据；
回到 Manual 恢复手柄。删除线及撤销恢复折点数据和线身份。

## 自动验证

`cargo test -p gef-logic-demo --lib`：21 项通过、0 失败、2 项成本采样默认忽略。
新增覆盖全加器 8 种输入、四位加法器全部 256 种输入，所有预设在三种模式下
均产生已提交路径；并验证中间障碍避让及障碍移动后的重路由。
0.5 与 1.5 倍缩放下真实 Tool 创建、移动、删除折点和撤销通过；
另验证单端移动引起折点半位移、切换时取消、无效模式拒绝、文件往返、
折点校验和删除恢复。沿用的连接、端口、节点及历史测试保持通过。

`cargo clippy -p gef-logic-demo --no-deps --lib -- -D warnings`、
workspace 格式检查、`cargo xtask docs`（461 份 Markdown）与 `git diff --check`
通过。本轮没有修改 workspace 配置或共享框架 API，没有启动 workspace 全量测试。

## 成本与窗口

Rust 1.99.1、macOS arm64、release；每种预设和路由各采样 51 次。
每次包含一次输入设置、一次节点移动及 `sync` 稳定查询；不包含 GPU 提交，
不测分配量。输入是交替值，第一轮输入与初值相同时不产生输入命令。
以下为合并路径耗时：

| 预设 | Manual p50／p95（ms） | Manhattan p50／p95（ms） | Shortest p50／p95（ms） |
|---|---:|---:|---:|
| 半加器 | 0.5362／1.3354 | 0.5365／1.3435 | 1.3375／1.9080 |
| 全加器 | 1.0809／1.6818 | 1.0990／1.7186 | 4.2590／8.1726 |
| 四位加法器 | 4.2582／7.5168 | 5.4100／9.5246 | 43.7287／74.4066 |

可见性路由在四位电路上已有明显成本，不能宣称连续交互性能验收通过。
默认保持 Manhattan；较大场景的可见性候选、失效范围和连续输入成本需专项优化。
debug 同夹具的四位最短路径 p95 为 733.7285 ms，采样期间存在并行编译，
只作开发诊断。release 采样时还开着默认半加器窗口，以上不是隔离性能基准。

本轮复用依赖、字体和公开路由，不新增资源或 package。
同配置 release 可执行文件为 37,157,152 字节，上轮记录为 36,939,360 字节；
总大小增加 217,792 字节（约 0.59%）。这比较两个消费者版本，不能当作单一
功能开关的消融结果；Web 传输和首次加载未验收。

保存的 `gef-logic-macos` action 构建并通过 LaunchServices 打开当前 checkout
的 `target/egui/Novadraw Logic.app`。进程实际路径一致，真实窗口为 1160×688。
[窗口截图](../evidence/logic-routing-2026-10-30.png)已复核预设菜单、三种路由、
Live／Ground、输入控件与真实电路。当前保留该窗口；未使用系统鼠标自动化，
不把截图或测试当作完整原生交互验收。

## 保留边界

本轮只覆盖无环组合电路。Circuit 层级与边界端口、guide、clone、直接编辑、
鸟瞰、DnD、打印仍待后续切片。自动 Tool 测试不代替窗口外 capture、跨显示器 DPI、
系统鼠标完整手势、IME 与系统 AT；不关闭 LG-01～15 整组对等验收。
