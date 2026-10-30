# Native 应用启动说明

## gef-shapes-macos

入口由 [run.json](run.json) 的 `gef-shapes-macos` action 管理，
使用 [标准 macOS bundle 启动脚本](../../tools/scripts/run_gef_macos.py)。
需要 macOS、Python 3 和满足依赖最低版本要求的系统默认 Rust 工具链。
UI 包属于根 workspace，使用根目录锁文件；普通引擎构建通过 `default-members`
排除 UI 包。也可在根目录直接执行 `cargo run -p gef-editor-demo`。
UI 直接依赖固定的未修改上游 Git 提交，Cargo 管理源码缓存，无需本地 vendor。
构建和 bundle 位于忽略的 `target/masonry`；脚本通过 LaunchServices 打开确切
bundle，并保持启动命令运行直至应用关闭。

2026-10-28 已建立真实窗口；用户将窗口置前台后确认“内容出现了”，随后窗口截图
可见 Xilem 工具栏、矩形与椭圆。这证明该 Native 路径能够显示实际场景。
工具的前台激活返回 false，启动时可能暂为白屏；需要实际置前台观察，不能以
白屏截图判定 Scene 编码失败。自动激活与冷启动时序尚未独立验收。

运行后不需重启已存在的窗口。原生拖动、undo/redo、resize、跨显示器 DPI、
IME 与系统 AT 均按各自验收范围记录；可见画面不等于全部交互已通过。
详见 [Native 续验](../../doc/verification/reviews/masonry-native-first-frame-followup-2026-10-28.md)。

同日迁移后已重新通过标准脚本构建并由 LaunchServices 启动原生进程，
构建产物和进程真实路径一致。当前桌面查询未取得窗口，原生画面与交互未核验。
自动输入、DPI 与 GPU 验证见
[上游迁移记录](../../doc/verification/reviews/masonry-upstream-migration-2026-10-28.md)；
迁移前的可见画面结论不能自动继承到新依赖。

## gef-logic-macos

入口由 [run.json](run.json) 的 `gef-logic-macos` action 管理，
复用[GEF macOS bundle 脚本](../../tools/scripts/run_gef_macos.py)的 Logic 参数。
需要 macOS、Python 3 和 Rust ≥1.96；本次使用系统 Rust 1.99.1。
该包属于 workspace，未加入默认成员；构建与 bundle 位于忽略的 `target/egui`。
脚本通过 LaunchServices 打开当前 checkout 的确切 bundle，跟随应用生命周期。

2026-10-29 release 构建、真实路径和窗口画面已核验；
窗口显示电路、端口/导线和工具面板，运行实例保留。
最终 14 项功能测试通过，另有 1 项手动成本采样完成，完整平台输入仍待验收。
启动脚本识别当前 checkout 的已运行实例时只激活窗口；
要装入新的构建，先保存并关闭现有窗口，再使用 Run 入口。
详见[Logic 实施记录](../../doc/verification/reviews/gef-logic-first-slice-2026-10-29.md)。

## draw2d-official-macos

入口由 [run.json](run.json) 的 `draw2d-official-macos` action 管理，
使用[官方样例启动脚本](../../tools/scripts/run_draw2d_macos.py)。
需要 macOS、Python 3 与满足锁定依赖要求的 Rust 工具链；本次使用 Rust 1.99.1。
脚本构建根 workspace 的官方样例，将二进制放入独立 bundle，并通过 LaunchServices
打开确切路径。构建与 bundle 位于忽略的 `target/official-native`。

PageUp／PageDown 切换场景，Escape 关闭窗口。真实窗口验收应使用交互模式；
`--screenshot` 使用独立截图路径，不能代替实际屏幕呈现。
启动后的前台状态可能被编辑器抢占，操作前需激活应用并核对窗口标题。
本机 Flow／Grid／Toolbar 的系统窗口 resize 与恢复屏幕像素已验证，
详见[窗口验收记录](../../doc/verification/reviews/official-layout-native-window-2026-10-29.md)。
后续[鼠标验收](../../doc/verification/reviews/official-layout-native-mouse-2026-10-29.md)
使用正常系统鼠标事件补齐代表性控件和窗口角拖拽，工具只用于采证，
不是应用启动依赖。跨平台、系统 DPI 和完整参数组合未验收。
