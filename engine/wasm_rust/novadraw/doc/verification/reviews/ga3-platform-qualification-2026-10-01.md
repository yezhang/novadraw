# GA-3 四平台资格证据

类型：`verification`

日期：2026-10-01

实现基线：`24b9f43`

本页记录 GA-3 平台构建与真实运行资格，不用交叉工具链诊断替代目标环境证据。
支持等级的唯一声明入口仍是
[`platform-support-matrix.md`](../../roadmap/platform-support-matrix.md)。

## 自动前置

| 环境 | Suite | 结果 | 可得结论 |
|---|---|---|---|
| macOS Apple Silicon | `platform.native-macos-release` | PASS | 当前提交可编译 representative Native Editor |
| Web wasm32 | `platform.web-chrome-release` | PASS | 当前提交可生成 Web validation 与 wasm-bindgen 产物 |
| Windows x86_64 MSVC | `platform.native-windows-release` | `not_run` | 缺少原生 Windows runner |
| Linux x86_64 X11 | `platform.native-linux-x11-release` | `not_run` | 缺少原生 Linux X11 runner |
| Linux x86_64 Wayland | `platform.native-linux-wayland-release` | `not_run` | 缺少原生 Linux Wayland runner |

macOS 与 Web 的自动前置于 2026-10-01 在提交 `24b9f43` 上执行通过。已有真实运行
记录继续支撑两者的 `runtime_verified`，本轮没有把一次构建通过解释成新的运行证据。

## 非资格性诊断

维护者在 macOS 上安装了 `x86_64-pc-windows-msvc` 与
`x86_64-unknown-linux-gnu` Rust 标准库，并尝试执行目标检查：

- Windows 依赖图编译到 `psm` 后，因宿主没有 MSVC `lib.exe` 停止；
- Linux 依赖图编译到目标 `fontconfig`/`psm` 后，因没有 Linux sysroot、
  target `pkg-config` 和目标 C 工具链停止；
- `RUST_FONTCONFIG_DLOPEN` 不是本依赖图的可替代构建模式，不能据此绕过
  `fontique` 的静态 API 契约。

这些结果只证明 macOS 交叉环境不完整，不证明 Windows/Linux 源码通过或失败。
因此三个 Native 环境均保持 `not_verified`，Wayland 未因 X11 使用同一 Rust target
而继承任何运行结论。

## 真实运行缺口

Windows、Linux X11 与 Linux Wayland 后续必须在各自原生环境执行已登记 suite，并按
[`platform-release.md`](../manual/platform-release.md) 保存以下证据：

- OS、GPU、driver、窗口系统、DPI 与 Rust toolchain；
- window/surface 创建、resize、suspend/resume 与 surface 恢复；
- pointer capture/leave、双轴 wheel、zoom、键盘与 focus loss；
- CJK/组合字符、IME preedit/commit/candidate area；
- accessibility snapshot，以及已声明 provider 时的系统 AT 行为。

在这些证据闭合前，不提升 Windows、Linux X11 或 Linux Wayland 的支持等级。
