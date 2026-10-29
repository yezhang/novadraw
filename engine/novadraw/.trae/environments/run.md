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
