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
