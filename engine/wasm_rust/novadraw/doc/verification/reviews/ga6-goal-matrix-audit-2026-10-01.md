# GA-6 目标矩阵审计

类型：`verification`

日期：2026-10-01

审计状态：`complete`

发布结论：`not_ready`

代码基线：`32b9568`

后续证据说明：本报告的矩阵冻结于上述审计基线。后续 Linux X11/Wayland 与 Windows
x86_64 构建证据已将三者提升为 `build_verified`，当前声明以
[`platform-support-matrix.md`](../../roadmap/platform-support-matrix.md) 为准；
三者真实窗口运行仍未验证，发布结论保持 `not_ready`。

## 1. 结论

GA-0 至 GA-5 已完成可在当前仓库和 macOS 环境闭合的目标登记、API 修复、平台任务
注册、模块/扩展整改和文档门禁。最终本地 full gate 通过。

这不等于“Novadraw 已全面超过 Draw2D”或“四平台已完整支持”。GA-2 的真实 present /
input-to-present 和 GA-3 的 Windows/Linux 原生运行证据仍缺失，因此当前不能给出总体
发布资格。

## 2. 目标矩阵

| 目标 | 当前结论 | 已有证据 | 未闭合项 |
|---|---|---|---|
| GOAL-CAP | `partial` | M1-M10、G0-G5、已完成 P2 delta；能力分母与剩余项已登记 | P2-G01/S01/C03/T03/W01/L01/O01/A01/LC01 |
| GOAL-PERF | `partial` | Novadraw CPU/内存、同机 Draw2D、Native GPU queue；GA-4 A/B 无可归因回退 | compositor present、input-to-present、WebGPU 浏览器预算；validation/routing 仍落后 Draw2D |
| GOAL-EXT | `verified` | 外部 Figure/Layout/component/text/backend 与 Editor compound visual 消费者；Runtime/Viewer 单一 owner | 后续新扩展仍需按真实消费者维持验证 |
| GOAL-PORT | `partial` | macOS、Chrome Web、Headless 为 `runtime_verified` | Windows、Linux X11/Wayland、Firefox、Safari 为 `not_verified`；原生 AT provider 待 P2-A01 |
| API/模块 | `verified` | facade、依赖方向、第三方类型、capability editor、GA-4 模块边界 | 不代表未实施 P2 能力已存在 |
| 文档/验证 | `verified` | 208 份 Markdown、26 个行为 suite 的 `api_semantics`、独立 facade probes | 外部 URL 在线性与行为语义仍需人工评审 |

## 3. 性能边界

同机可直接对照的证据显示：

- Novadraw 深链 validation 仍慢于 Draw2D；
- 1,000 条独立 connection routing 与共享 FanRouter routing 仍明显慢于 Draw2D；
- Native Vello 离屏 frame-to-GPU-complete 的固定矩形场景 p95 低于 16.7ms，但没有
  compositor present，不能推导屏幕交互预算；
- GA-4 前后各三轮 A/B 的确定性工作量一致，p50 最大回退 4.6%。

因此当前证据不支持总体“性能超过 Draw2D”结论。

## 4. 平台边界

| 平台 | 状态 |
|---|---|
| macOS Apple Silicon / Metal / winit | `runtime_verified` |
| Chrome / WebGPU / wasm32 | `runtime_verified` |
| Headless | `runtime_verified` |
| Windows x86_64 | `not_verified` |
| Linux X11 | `not_verified` |
| Linux Wayland | `not_verified` |
| Firefox / Safari | `not_verified` |

Windows/Linux suite 已登记，但 macOS 交叉工具链诊断不能替代目标环境构建和窗口运行。

## 5. 扩展与失败契约

- `Runtime` 是 FigureTree、UpdateManager、connection/resource/text service 的唯一 owner；
- `GraphicalViewer` 拥有单一 Runtime 与 Part/Policy/Tool 状态；
- 第三方私有 Figure 状态通过 `FigureComponentUpdate` 提交；
- callback deferred update 与 Editor visual refresh 复用 Runtime revision、damage、
  invalidation 和 fault 边界；
- foreign/disposed/wrong-capability、prepare rejection 和 panic 均有结构化契约或 fault
  隔离；
- 外部消费者不依赖 `pub(crate)`、测试 helper 或 Core 枚举扩张。

## 6. 最终门禁

第一次 full gate 在 `node-editor-demo` 的 bin-test 配置发现 `MouseButton` 缺少显式导入。
提交 `32b9568` 修复后重新执行：

```text
cargo xtask check --full: PASS
workspace fmt: PASS
facade / dependency / third-party API checks: PASS
Native/Web Vello build: PASS
workspace Clippy: PASS
workspace tests and doctests: PASS
```

补充阶段门禁：

```text
cargo xtask verify ga4.extension-consumers: PASS
cargo xtask verify core.performance: PASS
cargo xtask docs: PASS
```

## 7. 后续顺序

1. 在可见 Native compositor 环境补 present 与 input-to-present；
2. 在 Windows、Linux X11、Linux Wayland 原生 runner 执行已登记 suite 和人工矩阵；
3. 在 Chrome 记录 WebGPU 性能预算，并分别验证 Firefox/Safari；
4. 对 validation 与 routing 建立剖析后再实施性能 delta；
5. 按 P2 backlog 逐项推进长期能力，不重开 M1-M10。

GA-6 审计动作完成，但发布结论保持 `not_ready`，直到上述证据闭合。
