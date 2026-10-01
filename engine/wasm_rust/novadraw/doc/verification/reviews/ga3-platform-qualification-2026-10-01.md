# GA-3 四平台资格证据

类型：`verification`

日期：2026-10-01

实现基线：`24b9f43`

后续构建证据基线：`5ec80a6`

Windows 构建证据基线：`379f1ac`

本页记录 GA-3 平台构建与真实运行资格，不用交叉工具链诊断替代目标环境证据。
支持等级的唯一声明入口仍是
[`platform-support-matrix.md`](../../roadmap/platform-support-matrix.md)。

## 自动前置

| 环境 | Suite | 结果 | 可得结论 |
|---|---|---|---|
| macOS Apple Silicon | `platform.native-macos-release` | PASS | 当前提交可编译 representative Native Editor |
| Web wasm32 | `platform.web-chrome-release` | PASS | 当前提交可生成 Web validation 与 wasm-bindgen 产物 |
| Windows x86_64 MSVC | `platform.native-windows-release` | PASS | 固定 cargo-xwin、SDK 与 CRT 的 x86_64 MSVC target 检查通过 |
| Linux x86_64 X11 | `platform.native-linux-x11-release` | PASS | Linux VM 内 x86_64 target 与 X11/Wayland feature 图编译通过 |
| Linux x86_64 Wayland | `platform.native-linux-wayland-release` | PASS | Linux VM 内 x86_64 target 与 X11/Wayland feature 图编译通过 |

macOS 与 Web 的自动前置于 2026-10-01 在提交 `24b9f43` 上执行通过。Linux 两项
构建前置随后在提交 `5ec80a6` 上执行通过，Windows 构建前置在提交 `379f1ac` 上
执行通过。已有真实运行记录继续支撑 macOS 与 Web 的 `runtime_verified`；
Windows 与 Linux 只提升到 `build_verified`。

## 早期非资格性诊断

维护者在 macOS 上安装了 `x86_64-pc-windows-msvc` 与
`x86_64-unknown-linux-gnu` Rust 标准库，并尝试执行目标检查：

- Windows 依赖图编译到 `psm` 后，因宿主没有 MSVC `lib.exe` 停止；
- Linux 依赖图编译到目标 `fontconfig`/`psm` 后，因没有 Linux sysroot、
  target `pkg-config` 和目标 C 工具链停止；
- `RUST_FONTCONFIG_DLOPEN` 不是本依赖图的可替代构建模式，不能据此绕过
  `fontique` 的静态 API 契约。

这些结果只证明 macOS 交叉环境不完整，不证明 Windows/Linux 源码通过或失败。
后续 Linux VM 与 cargo-xwin 证据已分别闭合 Linux 和 Windows 构建，不反向改变这次
早期诊断的性质，也不能替代真实窗口运行。

## Windows 构建证据

macOS arm64 宿主上的固定交叉构建环境：

- Rust：1.94.1，安装 `x86_64-pc-windows-msvc` target 与 `llvm-tools-preview`；
- cargo-xwin：0.23.1；
- Windows SDK：10.0.26100；
- MSVC CRT：14.44.17.14；
- target/variant：`x86_64` / `desktop`；
- representative application：`node-editor-demo`。

`scripts/build_windows_target.sh` 在原生 Windows MSVC host 使用 `cargo check`，在其他
宿主使用上述固定 cargo-xwin 配置。已登记 suite 执行通过。原始日志：
`verification/evidence/ga3-platform-qualification-2026-10-01/windows-x86_64-build.log`，
SHA-256 为
`5c927d38379b0de1f7b90c9e4e985612382f89f2aa65630c20b6e95a96582b0b`。

该证据证明 x86_64 Windows MSVC target 的代表应用可以重复检查，不包含 Windows
window、DirectX/Vulkan surface、输入、DPI、IME、恢复或系统 AT 运行，因此不能提升到
`runtime_verified`。

## Linux 构建证据

Docker Desktop Linux VM 内执行环境：

- kernel：Linux 6.12.76-linuxkit，容器 host 为 aarch64；
- Rust：1.94.1，安装 `x86_64-unknown-linux-gnu` target；
- C toolchain：`x86_64-linux-gnu-gcc/g++`；
- target sysroot：amd64 fontconfig、X11、XKB 与 Wayland development packages；
- `pkg-config` 明确解析到 `/usr/lib/x86_64-linux-gnu`；
- winit feature 图同时包含 `x11`、`x11rb`、`wayland` 与 `wayland-dlopen`。

两个已登记 suite 均执行通过。原始日志：
`verification/evidence/ga3-platform-qualification-2026-10-01/linux-x86_64-build.log`，
SHA-256 为
`e7aa443918ee3b9bafffbd7c940e6097497dfd7fb19e9d90091ef4e63a72b87a`。

该证据证明 x86_64 Linux target 的代表应用可重复检查，不包含 X11 server、Wayland
compositor、GPU surface 或输入法运行，因此不能提升到 `runtime_verified`。

## 真实运行缺口

Windows、Linux X11 与 Linux Wayland 后续必须在各自原生环境执行已登记 suite，并按
[`platform-release.md`](../manual/platform-release.md) 保存以下证据：

- OS、GPU、driver、窗口系统、DPI 与 Rust toolchain；
- window/surface 创建、resize、suspend/resume 与 surface 恢复；
- pointer capture/leave、双轴 wheel、zoom、键盘与 focus loss；
- CJK/组合字符、IME preedit/commit/candidate area；
- accessibility snapshot，以及已声明 provider 时的系统 AT 行为。

在这些证据闭合前，不把 Windows、Linux X11 或 Linux Wayland 提升到
`runtime_verified`。
