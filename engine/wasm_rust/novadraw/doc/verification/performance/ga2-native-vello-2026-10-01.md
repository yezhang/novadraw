# GA-2 Native Vello GPU 基线

类型：`verification`

日期：2026-10-01

状态：`complete`（仅 Native Vello GPU queue completion 子阶段）

方法：[GA-2 性能测量方法](ga2-methodology.md)

## 环境与证据

| 项目 | 值 |
|---|---|
| 提交 | `657b37652407b78f1db94d9afd3d1a85e2dc7485` |
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
| SHA-256 | `e86032dbbca721504e3c0f18666d1257e4d3e316d7c6813a18f002285bf710bc` |

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
| Runtime `prepare_submission` | 2.959 ms | 3.598 ms |
| Vello backend submit CPU | 3.225 ms | 3.569 ms |
| submit 后 GPU queue completion wait | 5.094 ms | 7.646 ms |
| backend submit → GPU queue complete | 8.379 ms | 10.846 ms |
| frame start → GPU queue complete | 11.544 ms | 13.634 ms |

该固定场景的 frame-to-GPU-complete p95 低于 16.7ms 初始预算，但该结论只适用于
4,096 矩形、当前硬件、2× physical surface 和离屏 GPU 路径。没有 compositor
present 证据时，不能推导屏幕交互帧预算已满足。

进程峰值 RSS 从启动前 31.56 MiB 增至采样后 130.33 MiB。该数值包含 winit、wgpu、
Vello pipeline、Metal driver、surface 和 retained texture，不代表 Figure 树净内存。

## 剩余限制

- Vello 未启用 `wgpu-profiler`，没有硬件 timestamp；
- 当前 sandbox 无可见 drawable，surface present probe 为 `skipped`；
- 未测 compositor present、display timing 和 input-to-present；
- 尚未覆盖 TextFlow、连接与 viewport 的 Native GPU 场景；
- WebGPU 预算应在浏览器环境独立测量。

因此本报告关闭 Native GPU queue completion 证据缺口，但不关闭真实 present 与
input-to-present 验收。
