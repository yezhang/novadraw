# Logic 首个可运行切片实施记录

类型：`implementation-verification`

能力归属：应用消费者（`gef-logic-demo`）。
依据[行为契约](../../design/editor/gef-logic-first-slice.md)落实 Logic 子集；
不改变 Core/Editor 公共 API，不关闭完整 LG-01～15 对等验收。

## 实现范围

独立包 `examples/native/gef-logic-demo/` 使用六节点半加器，包含 LED、AND、OR、
XOR，typed 输入/输出端口、Manhattan 正交路由和高低电平显示。
节点创建、移动、删除、输入设置和导线创建/重连进入 EditorDomain；
删除与撤销恢复文档身份、顺序和相邻导线。信号属于模型派生值，
保存文件只记录结构及输入设置，重新打开后重算。

画布由 Vello texture 绘制，egui 负责工具栏、输入 bit 按钮和平台事件适配。
节点选区使用反馈视觉，不参与 Handle 命中；只有导线端点注册交互控制柄。
显式注册 Core 已有 Inter 字体，不扫描系统字体。

## 自动验证

当前 14 项功能测试通过，覆盖：

- 半加器四种输入、OR 真值表、LED 最低位映射及接线后覆盖设置值；
- 扇出、方向、端口占用、反馈环和非法文档拒绝；
- 输入与连接命令撤销重做、多节点加相邻线删除后精确恢复；
- 端口 EditPart 投影、移动后 Anchor 更新；
- CreationTool、ConnectionCreationTool、端点 Handle 重连、拖动及取消；
- 输入位变化但逻辑门输出不变时，目标输入端口仍收到刷新通知。
- 输入切换与撤销后，ConnectionFigure 组件中的实际描边颜色随信号更新。

`cargo clippy -p gef-logic-demo --lib --no-deps -- -D warnings` 通过。
包含依赖的严格 Clippy 在当前 Rust 1.99.1 下被现有 Core 的
`chunks_exact_to_as_chunks` 和两处 `drain_collect` 阻断；
本切片没有修改这些无关实现。

提交阶段另用 Rust 1.96.0 验证最新代码，包含依赖的严格定向 Clippy 通过；
14 项功能测试通过、0 失败，1 项成本探针按配置忽略。

最初的 `--include-ignored --nocapture` 执行了 13 项功能测试和 1 项成本采样，
全部通过。最终增加描边验证后，`cargo test -p gef-logic-demo --lib` 为
14 项通过、1 项成本采样按默认规则忽略；采样算法未改动，没有重复采样。
workspace `cargo fmt --all -- --check` 与 `cargo check --workspace` 通过。
切片开发阶段没有关闭框架里程碑，因此未启动 workspace 全量门禁；
提交阶段按仓库规则运行 workspace 格式、严格 Clippy、测试及文档检查，均通过。

## 成本采样

Rust 1.99.1，macOS arm64，Cargo debug 配置。使用 LED 链，
每档 201 次调用 `evaluate`，每次检查所有节点恰好各访问一次。
运行时另有 release 编译任务，结果只作为开发诊断，不能代表稳定的 release 延迟。

| 节点 | 导线 | p50（ms） | p95（ms） |
|---:|---:|---:|---:|
| 6 | 5 | 0.0108 | 0.0579 |
| 512 | 511 | 1.9423 | 2.4353 |
| 2048 | 2047 | 10.0458 | 40.1668 |

这只测模型信号求值，未包含命令通知、投影、命中、路由和 GPU 呈现，也未采集分配量。
默认展示为六节点/六线半加器，成本夹具使用单链，两者不能混为同一场景。
较大电路的 release 性能与输入响应仍待定向验证。

桌面 release 使用 `strip = "symbols"`，最终可执行文件为 36,939,360 字节。
这记录当前消费者总大小，不代表新增能力的体积增量；没有同配置的禁用特性基线。
Web 包体、传输与首次加载未验收。

## 原生启动与画面

复用 `tools/scripts/run_gef_macos.py --demo logic`，release 构建通过，
LaunchServices 打开当前 checkout 的 `target/egui/Novadraw Logic.app`。
进程真实路径与该 bundle 一致；CoreGraphics 查询到
`Novadraw - Logic · first slice` 窗口，逻辑窗口大小 1160×743。
[窗口截图](../evidence/logic-first-slice-2026-10-29.png)显示画布、
端口/导线、工具面板、bit 控件和状态栏。

截图时运行实例已有电路修改，保留该实例，未重启或覆盖当前状态。
工具侧只验证启动与画面，没有自动点击或拖动；
不据此声明全部平台交互已通过。
Run 面板已保存 `gef-logic-macos` action。
最后的描边修正已通过测试、Clippy 和 release 构建；当前窗口保留修正前的运行实例，
新二进制位于 `target/egui/release/gef-logic-demo`。先保存并关闭现有窗口，
再用 Run 入口启动即可装入修正版。启动脚本检查进程真实路径，
遇到当前 checkout 的已运行实例时只激活窗口，避免覆盖正在执行的 bundle。

## 已发现的边界

首版固定使用 Manhattan。将节点整体作为 shortest-path 障碍时，
端点 owner 是其子端口，首屏路由未形成可查询的结果；这条接入路径尚未验收，
后续路由切片需明确祖先障碍与端口出入口语义。

组合反馈电路被拒绝，尚不支持收敛或振荡求解。官方完整预设电路、
路由模式切换/折点、guide/嵌套/clone、直接编辑、鸟瞰、DnD 和打印后续推进。
真实平台拖动、窗口外 capture、跨显示器 DPI、IME 与系统 AT 仍须独立验收；
自动 Tool 测试不能代替原生输入验收。
