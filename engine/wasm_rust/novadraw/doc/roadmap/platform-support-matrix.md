# 平台支持矩阵

类型：`roadmap`

状态：`current`

本页是 Novadraw 平台支持声明的唯一入口。平台 package 存在、依赖可编译或其他平台
通过，均不能自动提升某个平台的支持等级。

## 支持等级

| 等级 | 含义 | 最低证据 |
|---|---|---|
| `not_verified` | 设计目标存在，但当前没有该环境的有效构建或运行证据 | 无 |
| `build_verified` | 指定 target、feature 和工具链可以重复构建 | manifest command 与原始日志 |
| `runtime_verified` | 真实环境完成窗口、输入、surface、文本和核心交互验收 | 自动 suite + 人工记录 |
| `fully_supported` | 发布矩阵、accessibility、恢复路径和已声明设备/窗口系统均闭合 | 发布审计与持续回归 |

支持等级针对“平台 + 运行环境”组合，不按 Winit/WGPU 的理论覆盖范围推断。

## 当前矩阵

| 平台 | 环境边界 | 等级 | 已有证据 | 主要缺口 | 下一验证 |
|---|---|---|---|---|---|
| macOS | Apple Silicon、Metal/Vello、winit | `runtime_verified` | `platform.native-macos-release`、M1-M10、G3-G5、P2-E02 人工验收 | 完整原生 AT provider、固定性能 runner | GA-2、P2-A01 |
| Web | Chrome + WebGPU/Vello + wasm32 | `runtime_verified` | `platform.web-chrome-release`、M10 Web、P2-E02 人工验收 | Firefox/Safari、DOM accessibility action、浏览器性能矩阵 | GA-3、P2-A01 |
| Windows | x86_64、winit + Vello/WGPU，具体 GPU 未固定 | `not_verified` | `platform.native-windows-release` 已登记，尚无合格 runner 结果 | 原生构建、DirectX/Vulkan、DPI、IME、surface 恢复、AT | `platform.native-windows-release` |
| Linux X11 | x86_64、winit + Vello/WGPU | `build_verified` | Linux 6.12 VM 中以 Rust 1.94.1、x86_64 sysroot 执行 `platform.native-linux-x11-release` 通过 | 真实 X11 窗口、GPU、DPI、XIM/IME、clipboard、AT | `run.native-linux-x11-release` |
| Linux Wayland | x86_64、winit + Vello/WGPU | `build_verified` | Linux 6.12 VM 中以 Rust 1.94.1、x86_64 sysroot 执行 `platform.native-linux-wayland-release` 通过 | 真实 Wayland compositor、GPU、fractional scale、IME、surface 恢复、AT | `run.native-linux-wayland-release` |
| Headless | 无窗口、测试 backend/host | `runtime_verified` | contract tests 与 Editor replay | 不是桌面发布平台；不替代 GPU/输入验收 | 保持回归 |

## 浏览器范围

| 浏览器 | 等级 | 说明 |
|---|---|---|
| Chrome | `runtime_verified` | 只限已有验收记录中的桌面 Chrome/WebGPU 环境 |
| Firefox | `not_verified` | 不从 wasm32 构建成功推断运行支持 |
| Safari | `not_verified` | 不从 macOS Native 通过推断 Web 支持 |

## 运行验收维度

每个平台提升到 `runtime_verified` 前必须覆盖：

1. window/canvas create、resize、DPI/scale factor；
2. suspend/resume、surface loss/rebuild、连续 redraw；
3. pointer press/move/release/capture/leave，双轴 wheel 与 zoom；
4. keyboard、shortcut、CJK/combining text、IME preedit/commit/candidate area；
5. focus loss、迟到事件与 direct-edit lease；
6. 字体 fallback、文本测量、图片资源和 representative scene；
7. accessibility snapshot；若声明可操作 accessibility，还需 focus/default action 回流；
8. 对应平台的原始日志、工具链、GPU、窗口系统和人工验收时间。

`build_verified` 只要求第 1 步之前的构建产物，不得在文档中写成“平台已支持”。

## 权威关系

- 平台无关行为由 `doc/design/` 和 ADR 定义；
- 自动命令由 `verification/suites.toml` 定义；
- 人工步骤由 `doc/verification/manual/platform-release.md` 定义；
- 当前 GA-3 执行证据见
  [`ga3-platform-qualification-2026-10-01.md`](../verification/reviews/ga3-platform-qualification-2026-10-01.md)；
- 本页只维护支持声明和证据链接；
- 历史审计保留当时结论，不因本页等级变化而改写。
