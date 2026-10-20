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
24 组 preferred/minimum/child bounds；同内容的 TSV 由 Rust 外部测试
`p2_grid_java_oracle` 消费。固定尺寸避免字体指标干扰，不能代替官方 Shape 的
可变 size/reset、文字换行、边框、全部 span/hint 或宿主 resize 验收。

本次差额集中在 grab：Rust 原来只分配正向余量，小容器不收缩；余量采用浮点均分，
例如 300 宽输出 142.5/142.5，Java 为 142/143。整改后按整数增量和末轨余数分配，
无 hint 的 grab 轨道可收缩，非 grab 或显式 hint 保留相应尺寸下限。
等宽列单独保持官方整除规则；Core 的小数输入保留末轨残量及非负下限。
复杂 span 的下限再分配和换行仍属于 K02 后续核对范围。
