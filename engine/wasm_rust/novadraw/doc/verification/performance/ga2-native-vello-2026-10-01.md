# GA-2 Native Vello GPU 基线

类型：`verification`

日期：2026-10-01

状态：`in_progress`（GPU queue completion 与可见 surface present 调用已验证）

方法：[GA-2 性能测量方法](ga2-methodology.md)

## 环境与证据

| 项目 | 值 |
|---|---|
| 提交 | `dcf1bc808b6db722929eacf9af049cdc8084c5da` |
| 工作区 | clean |
| OS / CPU | macOS / Apple M1 Pro |
| GPU / backend | Apple M1 Pro integrated GPU / Metal |
| Rust | 1.94.1 |
| Profile | release |
| logical viewport | 1024 × 768 |
| physical surface / scale | 2048 × 1536 / 2.0 |
| Figure / command | 4,097 / 53,258 |
| 预热 / 采样 | 5 / 30 |
| 原始报告 | `target/verification/reports/ga2-native-vello.json` |
| SHA-256 | `c3d644f6fd3d37908fe75dd057865f7ceccfbd76390a3477b6d055f5c600b1f0` |

执行入口：

```bash
cargo xtask verify backend.native-performance
```

## 测量边界

runner 启动真实 winit 窗口和 Vello Metal device，先尝试一次 surface submit：

- 可获得 drawable 时，采样完整 backend surface submit；
- surface 返回 `Skipped` 时，报告 probe 结果并改采
  `render_for_screenshot + wait_for_gpu_idle`；
- 两种模式都记录实际 adapter、surface、原始样本和测量 scope。

本次 TRAE sandbox 中 surface probe 为 `skipped`，正式结果的 `render_mode` 是
`Offscreen`。因此 `surface_present_call`、`compositor_present` 和
`input_to_present` 均为 `false`，不能把本报告称为真实呈现耗时。

`backend_submit_cpu` 包含 Vello lowering、command encoding 和内部 queue submission。
`gpu_completion_wait` 是 submit 返回后调用 `Device::poll(Wait)` 的墙钟等待；GPU 已在
submit 期间并行执行，因此它不是纯 GPU 时长。`submit_to_gpu_complete` 是从 backend
submit 开始到 queue 全部完成的端到端墙钟，仍不是硬件 timestamp。

## 结果

| 阶段 | p50 | p95 |
|---|---:|---:|
| Runtime `prepare_submission` | 2.940 ms | 3.600 ms |
| Vello backend submit CPU | 3.524 ms | 3.769 ms |
| submit 后 GPU queue completion wait | 5.105 ms | 6.393 ms |
| backend submit → GPU queue complete | 8.700 ms | 10.220 ms |
| frame start → GPU queue complete | 11.881 ms | 13.725 ms |

该固定场景的 frame-to-GPU-complete p95 低于 16.7ms 初始预算，但该结论只适用于
4,096 矩形、当前硬件、2× physical surface 和离屏 GPU 路径。没有 compositor
present 证据时，不能推导屏幕交互帧预算已满足。

进程峰值 RSS 从启动前 31.75 MiB 增至采样后 141.64 MiB。该数值包含 winit、wgpu、
Vello pipeline、Metal driver、surface 和 retained texture，不代表 Figure 树净内存。

## 可见 Surface Present 证据

`backend.native-presentation-performance` 已登记为独立 suite。它通过 macOS app bundle
启动同一 harness，并要求真实 surface present；控制台锁屏或 drawable 持续不可用时
直接失败，不再生成离屏 PASS。

2026-10-01 在提交 `3665b9a`、已解锁且 focused/unoccluded 的 macOS 登录会话执行通过：

| 阶段 | p50 | p95 |
|---|---:|---:|
| Runtime `prepare_submission` | 3.349 ms | 3.792 ms |
| Vello backend surface submit CPU | 3.884 ms | 4.147 ms |
| submit 后 GPU queue completion wait | 6.913 ms | 9.216 ms |
| backend submit → GPU queue complete | 10.745 ms | 13.188 ms |
| frame start → GPU queue complete | 14.069 ms | 16.487 ms |

环境为 Apple M1 Pro / Metal、1024 × 768 logical viewport、2048 × 1536 physical
surface、4,097 Figure、5 次预热与 30 次采样。surface probe 结果为 `presented`，
正式样本全部使用 Surface 模式并调用 `SurfaceTexture::present()`。

原始报告：
`verification/evidence/ga2-performance-2026-10-01/native-presentation.json`，
SHA-256 为
`bc6a5e797abe59d21ec0bbe59db843ac7e0712f228d6e19a196a71f5623eb36b`。

该 p95 接近 16.7ms 初始帧预算，但终点仍是 GPU queue completion，不是显示器扫描。
该入口只关闭可见 `SurfaceTexture::present()` 调用证据。compositor 回执与
input-to-present 仍需独立时间源，不能由 queue idle 或 present 调用返回替代。

## 剩余限制

- Vello 未启用 `wgpu-profiler`，没有硬件 timestamp；
- 可见 surface present 调用已验证，但未测 compositor present、display timing 和
  input-to-present；
- 尚未覆盖 TextFlow、连接与 viewport 的 Native GPU 场景；
- WebGPU 预算应在浏览器环境独立测量。

因此本报告关闭 Native GPU queue completion 与可见 surface present 调用证据缺口，
但不关闭 compositor present 与 input-to-present 验收。
