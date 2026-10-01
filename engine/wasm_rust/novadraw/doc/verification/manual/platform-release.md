# 四平台发布资格验收

类型：`verification`

本流程用于提升
[`平台支持矩阵`](../../roadmap/platform-support-matrix.md) 的支持等级。
它不替代 `verification/suites.toml` 中的自动命令，也不允许在没有对应环境时填写 PASS。

## 证据头

每次执行必须记录：

- Git commit；
- OS、版本、架构；
- Rust toolchain 与 target；
- GPU、driver、窗口系统；
- backend 与 feature；
- 浏览器名称、版本与 WebGPU 状态（Web）；
- 输入法、键盘布局、DPI/scale factor；
- 执行者与时间。

## 自动前置

| 环境 | Suite |
|---|---|
| macOS | `platform.native-macos-release` |
| Windows | `platform.native-windows-release` |
| Linux X11 | `platform.native-linux-x11-release` |
| Linux Wayland | `platform.native-linux-wayland-release` |
| Chrome + WebGPU | `platform.web-chrome-release` |

1. 运行本表对应的 suite；
2. 运行受影响 Core/Editor contract suite；
3. 保存原始日志，不只保存人工结论；
4. 若缺少平台 runner，保持 `not_verified`，不得用 host check 替代。

## Native 场景

在 macOS、Windows、Linux X11、Linux Wayland 分别验证：

1. 创建窗口并显示 representative Figure、Text、Connection；
2. 连续 resize、最小化/恢复、DPI 或 scale factor 变化；
3. surface suspend/resume 或等价生命周期后完整恢复；
4. pointer capture、离窗、返回、release；
5. 纵向与横向滚动、Ctrl/Command wheel zoom、触控板或等价设备；
6. Tab/focus、普通文本、快捷键、dead key；
7. CJK 输入法 preedit、候选窗、commit、cancel、focus loss；
8. direct edit accept/cancel、scroll/zoom 后 caret 与候选区对齐；
9. 字体 fallback、图片资源、连接路由和局部 damage 无明显错误；
10. accessibility provider 若已声明支持：读取树、focus、default action。

Linux 必须分别记录 X11 与 Wayland；一个通过不能代表另一个。

## Web 场景

在 Chrome、Firefox、Safari 分别记录：

1. WebGPU adapter/device 与 canvas configure；
2. resize、device pixel ratio 变化、tab hide/show；
3. pointer button 映射、capture/leave、wheel 方向和 zoom；
4. hidden textarea 或 EditContext 的 IME 全序列；
5. DOM accessibility tree、focus 与 default action 回流；
6. representative Vello scene 非空且与 Headless/Native 语义一致。

浏览器不支持所需 WebGPU 能力时记录 `unsupported_environment`，不得记为项目 PASS。

## 状态提升

- 自动构建通过：可提升到 `build_verified`；
- 自动前置和本平台全部必需场景通过：可提升到 `runtime_verified`；
- accessibility、恢复、发布制品与持续 runner 全部闭合：才可评估 `fully_supported`；
- 任一项未执行时记录 `not_run`，不等同于 PASS；
- 失败必须保留首个失败步骤和已通过前缀。
