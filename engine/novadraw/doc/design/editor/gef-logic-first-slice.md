# Logic 首个可运行切片

类型：`proposal`

能力归属：应用消费者（`gef-logic-demo`）。
电路模型、信号求值与文件存储属于应用；端口、连接与命令历史复用 Editor，
Figure、命中、坐标和绘制复用 Core。该切片验证消费者接入，
不将电路业务逻辑计为新增 Core 或 Editor 能力。

本切片落实 [GEF 样例方案](gef-official-examples-parity-proposal.md) 的 Logic 模型、
端口和编辑链路子集。入口为 `examples/native/gef-logic-demo/`，不改 Shapes UI，
也不引入 Core 电路求解器。

## 行为契约

- AND、OR、XOR 有 A/B 输入和 OUT 输出；LED 有 A/B/C/D 输入及 1/2/3/4 输出，
  对应从最低位到最高位。LED 无输入线时使用设置值；有任意输入线时全部位由
  端口决定，未连接输入为 false。对照 GEF `Gate`、`SimpleOutput`、`LED`、`Wire`。
- 首屏是六节点半加器，两个 LED 输入经 XOR/AND 到 Sum/Carry LED。
  预设菜单提供半加器、全加器和四位加法器的展开拓扑。按 GEF
  `LogicDiagramFactory` 的连接关系展开 Circuit 边界透传；
  全加器由两个半加器与 OR 门构成，四位预设由一个半加器和三个全加器串联。
  四位预设初值为 3 和 7，显示四个和位，不输出最高进位，与官方 large model 一致。
  可编辑 Circuit 层级及边界端口不在本轮实现范围，不能据此关闭嵌套容器验收。
- Live 和 Ground 各提供一个固定高／低电平输出，不允许输入接线。
- 端口身份含节点、方向、位号；ModelConnection 绑定端口 EditPart。
  输出允许扇出，每个输入最多一条线。创建和重连拒绝方向错误、自连接及组合反馈环。
  当前明确限定无环组合电路，反馈电路求解不在本切片内。
- 创建、移动、删除、输入值和重连均走 EditorDomain 命令历史。删除节点同时删除
  相邻线，撤销恢复原身份、端口和顺序。重连保持导线身份。
- 信号是派生值，不序列化、不进入撤销历史。实例内确定性拓扑队列按每个节点一次
  的预算求值；命令、撤销和重做之后重算，并发布受影响节点及导线的模型通知。
- egui 仅承担宿主输入适配和控制栏；Figure 树、命中、坐标、连接反馈及拖动归引擎。
  Vello 绘制真实画布；选区反馈在 overlay 层。
- 路由作为文档状态保存，切换使用可撤销命令。Manual 复用 FanRouter 与
  BendpointConnectionRouter；Manhattan 复用正交路由；
  Shortest path 复用 VisibilityGraphConnectionRouter，以节点 Figure 为障碍。
  路由器持有稳定 Figure 身份，节点结构变化时更新障碍列表，位置变化由 Runtime
  依赖失效驱动，不在每帧重建路由器。
- 手动折点保存相对 source 与 target 端口中心的两个偏移，权重固定为 0.5。
  解析位置为 `(source + source_offset + target + target_offset) / 2`；
  移动单个端点时折点跟随其一半位移。Policy 接收 Editor 转换后的 routing 坐标，
  通过同一 EditBendpoint Request 实现增、移、删，释放指针时一次提交。
  仅 Manual 创建折点手柄且允许命令执行；切换前取消未提交手势，自动路由保留
  折点数据。删除导线同时删除折点，撤销恢复。打开文件校验偏移、长度和导线身份。

## 成本与验证

每次信号求值建立邻接索引，拓扑访问量为 O(V+E)，有序索引查找为 O(log V)，
暂存空间为 O(V+E)，不是递归传播。Manual 成本随折点数增长；
可见性路由在障碍角点间构建候选关系，较大场景的成本不能由首屏表现推断。
本轮采样输入变更、节点移动与稳定查询的合并延迟；不将其写成 GPU 帧耗时。
结构命令保存局部变更；不复制整个文档来修改输入或移动节点。
只复用 Shapes 已使用的 egui/eframe/Vello 依赖，并显式注册 Core 已有的 Inter
字体；不新增图标、字体文件或求解器资源。
新包不进入默认成员，不影响默认 Core 加载。桌面 release 体积与六节点交互证据
在实施记录中填写；Web 接入和传输成本尚未验收。

测试覆盖真值表、LED 位序、扇出、占用、环、文件校验、撤销、端口投影、移动后的
Anchor、创建/重连及取消。原生窗口显示与实际平台输入分别记录。

本次不关闭 LG-01～15 的整组对等验收。门级预设、路由模式与相对折点已实现，
证据见[后续记录](../../verification/reviews/gef-logic-routing-2026-10-30.md)；
后续仍需 Circuit 层级与边界端口、guide/嵌套/clone、直接编辑、鸟瞰、DnD、
打印及平台输入验收。
