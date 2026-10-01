# GA-2 Chrome WebGPU 浏览器基线

类型：`verification`

日期：2026-10-01

状态：`in_progress`（严格 runner 已实现，真实可见浏览器证据待解锁会话执行）

方法：[GA-2 性能测量方法](ga2-methodology.md)

## 固定环境

| 项目 | 要求 |
|---|---|
| OS / GPU | macOS / Apple GPU |
| 浏览器 | Google Chrome 154.x，非 headless |
| 自动化运行时 | Node.js 24 |
| WebGPU adapter | Apple vendor / Metal architecture / 非 fallback |
| viewport / DPR | 1280 × 900 / 1.0 |
| canvas / surface | 1024 × 768 / 1024 × 768 |
| Figure | 4,096 个矩形加根 Figure |
| 预热 / 采样 | 5 / 30 |
| 原始报告 | `target/verification/reports/ga2-webgpu-browser.json` |

执行入口：

```bash
cargo xtask verify backend.webgpu-browser-performance
```

suite 会构建 release Wasm，注入完整 Git revision 与 dirty 状态，启动独立 Chrome
profile，并通过 CDP 驱动 `mode=performance&backend=vello`。正式报告只接受 clean
构建。控制台锁屏、页面隐藏或失焦、canvas 未完整位于可见 viewport、WebGPU
不可用、Canvas2D、fallback/software adapter、版本漂移、样本缺失或超时都会直接
失败。

## 测量边界

每个样本依次记录：

1. Runtime `prepare_submission` CPU 墙钟；
2. Vello WebGPU backend `submit` CPU 墙钟；
3. submit 返回到 `GPUQueue.onSubmittedWorkDone` 对应 wgpu 回调的墙钟；
4. queue completion 回调到其后首个 `requestAnimationFrame` 的墙钟；
5. frame start 到该 RAF 的总墙钟。

queue completion 证明注册回调前的 WebGPU queue work 已完成。其后的首个 RAF
只代表保守的浏览器呈现机会，不是 compositor 精确 present 回执，也不证明物理显示器
scanout。报告因此固定
`browser_compositor_present=false` 与
`physical_display_scanout=false`，不得用该结果关闭 Native WindowServer 或物理显示
证据。

## 当前状态

静态 Wasm 编译、脚本语法和 suite 登记可在锁屏状态验证。正式采样必须在解锁且 Chrome
窗口可见的登录会话执行；在取得并固化原始 JSON、SHA-256 与 p50/p95 前，本报告不记录
性能结论，GA-2 的 WebGPU 浏览器预算保持 `not_verified`。
