# 官方 Grid 事件基线

类型：`verification`

本工具编译固定上游基线的 `AbstractExample.java` 和 `GridLayoutExample.java`，
不修改官方源码。`GridEventOracle` 调用原生 SWT 控件注册的事件回调，并通过
官方 `handleMousePressed` 选择 Shape。它不注入操作系统键鼠事件，因此不能证明
Tab 的真实焦点顺序、鼠标命中、截图或其他 24 个入口已经验收。

运行时需提供 Draw2D jar、当前平台 SWT jar 和已解压的 SWT 原生库。以下为本次
macOS Cocoa 的实际命令：

```sh
python3 verification/oracles/draw2d/run_grid_oracle.py \
  --reference-root /Users/bytedance/Documents/code/GitHub/gef-classic \
  --draw2d-jar /Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/target/org.eclipse.draw2d-3.22.0-SNAPSHOT.jar \
  --swt-jar third_party/draw2d-examples/target/performance-deps/org.eclipse.swt.cocoa.macosx.aarch64-3.135.0.jar \
  --native-library-dir third_party/draw2d-examples/target/performance-deps/native-org.eclipse.swt.cocoa.macosx.aarch64-3.135.0 \
  --output verification/evidence/draw2d-official-examples/grid-events.json
```

输出记录源码与 jar 的 SHA-256、Git 基线、Java/SWT 版本、字体、DPI 及回调结果。
jar 作为显式输入记录，不将其文件名视为源码一致性的证明。工具拒绝不同 Git 基线，
编译和运行分别限制为 60 秒；任一步失败均不发布成功证据。

本次结果见 [原始记录](../../evidence/draw2d-official-examples/grid-events.json)：

- 两个初始 Shape 后，Add 依次生成剩余七项，再循环到蓝色 Ellipse、红色 Rectangle。
- widthHint 的 FocusOut 和 DefaultSelection 都更新 GridData，并各调用一次 updateView。
  非法数字回退为 1，官方诊断输出保留在原始记录。
- Clear All 清空 children、禁用 GridData 控件，保留 selectedShape 的 Java 引用；
  再次 Add 从当前序列继续，生成 RoundedRectangle。

Rust 对失效 FigureId 不保留可写引用：Clear 时取消选中，Shape 实际移除后通过
公开生命周期能力清理控制器的约束表。动态 Add 成功回执发布新 ID 的默认约束，
拒绝不发布约束。图形序列仍在构造时推进，与官方 `createShape()` 一致。
这一身份管理差异不修改官方 Java 行为，也不据此宣称两端状态模型完全相同。

## Grid 测量对照

同一运行命令增加 `--measurement`，输出目标改为
`verification/evidence/draw2d-official-examples/grid-measurements.json`，即可运行
`GridMeasurementOracle`。该 fixture 使用三个 70×70 Rectangle，覆盖普通、等宽、
span/indent/hint、双轴 grab、等宽 grab 和混合 grab 六组配置，每组核对
0/80/155/300 四个容器宽度。

[原始测量](../../evidence/draw2d-official-examples/grid-measurements.json) 保留输入与
36 组 preferred/minimum/child bounds；同内容的 TSV 由 Rust 外部测试
`p2_grid_java_oracle` 消费。固定尺寸避免字体指标干扰，不能代替官方 Shape 的
可变 size/reset、文字换行、边框、全部 span/hint 或宿主 resize 验收。

本次差额集中在 grab：Rust 原来只分配正向余量，小容器不收缩；余量采用浮点均分，
例如 300 宽输出 142.5/142.5，Java 为 142/143。整改后按整数增量和末轨余数分配，
无 hint 的 grab 轨道可收缩，非 grab 或显式 hint 保留相应尺寸下限。
等宽列单独保持官方整除规则；Core 的小数输入保留末轨残量及非负下限。
复杂 span 的下限再分配和换行仍属于 K02 后续核对范围。

续批增加三组跨列/跨行 fixture：200px 跨列 hint、跨列 hint 与末列 grab 混合、
200px 跨行 hint。36 组输出均已对照。span 无可扩展轨道时，deficit 落在末轨；
已有可扩展轨道时只分配到这些轨道，按官方反向遍历分配整数余数。
原来平均分摊会将固定列从 x=80 移到 x=107.5，并在混合 grab 下缩短 200px
hint；现已修正。跨行整数居中采用整除，Grid minimum 按 AbstractLayout
返回 preferred。约束替换回归同时验证测量缓存失效与固定列不漂移。
这些结果只覆盖记录输入，多重重叠 span、measurement hints 和换行仍待核对。

## 图布局全 fixture 基线

运行命令增加 `--graph-fixtures`，输出目标使用
`verification/evidence/draw2d-official-examples/graph-fixtures.json`。
工具直接编译两个官方 Demo、AbstractGraphDemo、GraphTests 和 CompoundGraphTests，
调用 Demo 的 `getGraphMethods()` 获取真实反射菜单顺序；不按源码名称排序。
随后调用 16 个 Directed 和 10 个 Compound factory，每个执行 SOUTH=4/EAST=16，
共 52 个结果。JSON 记录源码/jar 指纹及异常栈，TSV 保存菜单、节点、包含关系、
边端点、折点和 vNodes 数量。

本次 [原始输出](../../evidence/draw2d-official-examples/graph-fixtures.json)
得到 45 组正常结果、7 组原版 ArrayIndexOutOfBoundsException；
576 条 node 和 461 条 edge 输出。异常来自 GraphTests 的
`shuffleConnections` 负索引，保留原版源码和栈，不把异常组合改成空图或删除菜单。
GraphTests 持有静态 Random，输出与调用历史有关；本基线是记录的菜单/方向调用
序列，不声明跨进程、跨版本或单独重跑均得到同一坐标与异常分布。

`source_contracts` 校验 26 菜单项、52 组合均有正常或异常结果，成功结果的 node/edge
行数完整。它只验证基线记录，不证明 Rust 图布局实现、所有算法中间态、宿主菜单
事件或视觉已经对等。K06 移植时仍需保存完整输入权重/padding/offset，分别验证
算法不变量和官方异常适配；从源代码确认需求后再设计受检外部算法协议。
