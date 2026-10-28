# Masonry Native 首帧与 resize 续验

类型：`implementation-verification`

2026-10-28，用户要求“继续下一步吧”。本记录继续
[首批实施](masonry-xilem-canvas-implementation-2026-10-28.md)，保留原失败证据。

## Native 结果

使用已有 show 后 redraw 与 Occluded(false) 回调重绘的最终代码：

1. 终端 release 启动：建立窗口，截图仍白屏。
2. 标准 macOS `.app` / LaunchServices 启动：建立窗口，截图仍白屏。

新增 [bundle 启动脚本](../../../tools/scripts/run_gef_macos.py)，只使用 Python 标准库、
Cargo 和 macOS `open -W`。不改变 signing、权限或系统设置。
脚本使用独立 demo lockfile，在忽略的 `target/masonry` 生成 bundle。
当前 bundle 与 release executable 的 SHA-256 都是
`0385d8f9d6e8225b3691433d16a5c619e321f6902dfaf3556878606ca1654856`。
因此不是通过换成其他应用或近似场景取得窗口。

终端进程的系统可执行路径位于工具的 `.sandbox_exec` 镜像；
LaunchServices 进程使用实际 `.app/Contents/MacOS/gef-editor-demo`。
两条路径都失败，尚不能将白屏确定归因于工具镜像路径。
两进程的 `isFinishedLaunching=true`、`isActive=false`，系统前台激活调用返回 false。
只读 `sample` 也无法检查对应进程；没有请求 sudo 或扩大权限。
没有拿到 surface acquisition / actual present 的 receipt，不能认定是 GPU 编码、
布局或 occlusion 中某一项的闭环根因。

上述两次初始白屏保留为历史证据。用户随后将现有窗口置前台并确认“内容出现了”。
同一进程、同一窗口的新截图 `native-visible-confirmed.png` 可见实际 Xilem 工具栏、
矩形与椭圆。由此确认该 Scene / Native 路径能够实际显示，未重启或替换模型来验收。
抓取截图时窗口又为 inactive，但已显示的内容仍保留。

已登记标准 `.app` 启动的 `gef-shapes-macos` Run action，保持原有默认 action，
无需再启动验证。仍不声称自动前台激活或冷启动时序已解决；当前现象与前台/遮挡
状态相关，缺少 acquisition/present 的完整时序 receipt，具体触发链继续保留限制。

截图与日志见 [原始证据目录](../evidence/masonry-xilem-2026-10-28/)：
`native-exposure-final.png`、`native-launchservices.png`、
`native-visible-confirmed.png`、`resize-dpi-followup-tests.log`。
失败截图与用户确认后的成功截图分别保存。

## Resize / DPI 消费者结果

新增公开事件消费者测试
`resize_and_dpi_changes_preserve_source_history_and_gpu_coordinates`：

- 420×320 @2、280×210 @1.5、640×480 @1 连续变化；
- 真实 WGPU 纹理尺寸与形状内部像素符合对应物理坐标；
- source bounds、Viewer namespace 与 history 保留；
- drag feedback 期间 resize 不提前提交，release 后仅一次 command，undo 恢复。

核验暴露了上游 harness 的另一个尺寸缺口：直接接收 Resize 后 RenderRoot 已更新，
而截图纹理仍使用旧 `window_size`。已在测试兼容层同步该物理尺寸，
不修改 Core、Editor 或生产 backend 的几何语义。变更记入 vendor provenance。

七项消费者测试通过，原 release CPU 成本测试在普通运行中按配置 ignored；
更新消费者的严格 Clippy、格式与文档门禁通过（419 个 Markdown 文件）。
Native 场景显示已获用户确认并有窗口截图；原生完整交互和启动时序仍待验收。
整体状态保持 [P2 backlog](../../roadmap/p2-delta-backlog.md) 的 in_progress。
