# GA-2 Draw2D 同环境对照基线

类型：`verification`

日期：2026-10-01

状态：`complete`（仅 Draw2D macOS CPU / 离屏 SWT 子阶段）

方法：[GA-2 性能测量方法](ga2-methodology.md)

## 环境与证据

| 项目 | 值 |
|---|---|
| Novadraw runner 提交 | `3a05b4d80ffee2fd064767eea27cf332fbda23d1` |
| Draw2D runner 提交 | `0c6a6c7c67334315dc89e9821ae557aa55f4eb72` |
| Draw2D 源码提交 | `4463d9d0ce13c19d10fbe769d29f28b7345a8cba` |
| Draw2D JAR SHA-256 | `f29b0247ae1a58cdc685bbcfbfcfc1174bbecd531f3da5d765c982ef3398f430` |
| SWT | 3.135.0 / Cocoa aarch64，内部版本 4974 |
| SWT JAR SHA-256 | `11555a0133901e0acad659d923c2ba968857032829e0a3b4a2908bf0b2986e23` |
| OS / CPU | macOS / Apple M1 Pro |
| JVM | OpenJDK 21.0.5，`-Xss64m -Xms128m -Xmx1g` |
| Rust | 1.94.1，release |
| viewport / logical DPI | 1024 × 768 / 96 |
| 预热 / 采样 | 5 / 30 |
| Draw2D 原始报告 | `target/verification/reports/ga2-draw2d/*.json` |
| Draw2D `SHA256SUMS` 哈希 | `4e9f69876504e78ff3173c8d3414935cc6f2a57dfa5518c5bc93c8a31c7fd4ad` |
| Novadraw 原始报告 | `target/verification/reports/ga2-novadraw.json` |

执行入口：

```bash
cargo xtask verify reference.draw2d-performance
```

runner 固定下载与 Draw2D JAR 要求相容的 SWT，显式把原生库解压到仓库 `target/`，
每个场景启动独立 JVM。报告保存 JVM 参数、两份 JAR 指纹、原始纳秒样本、p50/p95、
JVM heap 和操作系统进程峰值 RSS。

## 可直接对照的 CPU 阶段

下列操作两端都测量状态变更、相应算法和结果提交，不包含 GPU 或窗口 present。

| 场景 | Novadraw p50 / p95 | Draw2D p50 / p95 | 结论 |
|---|---:|---:|---|
| 深链 validation 1,000 | 0.099 / 0.108 ms | 0.048 / 0.048 ms | Novadraw 约 2.1 倍耗时 |
| 深链 validation 10,000 | 2.101 / 2.496 ms | 0.217 / 0.222 ms | Novadraw 约 9.7 倍耗时 |
| 独立 direct routing 1,000 | 79.784 / 83.241 ms | 0.226 / 0.297 ms | Novadraw 明显落后，需继续剖析 route commit |
| 共享 FanRouter routing 256 | 6.510 / 6.553 ms | 0.723 / 1.189 ms | Novadraw 明显落后 |

独立 routing 的两端都为 1,000 条连接计算两个端点并提交折线结果；共享 routing 都使用
同一 source/target anchor pair 和 separation 16。两端内部所有权、通知与缓存机制不同，
因此表格证明的是当前完整操作成本，不把差值直接归因到某一个函数。

## 不直接比较的阶段

| 场景组 | Novadraw 当前阶段 | Draw2D 当前阶段 | 处理 |
|---|---|---|---|
| 宽树 / 深树 | CPU command recording | CPU traversal + 离屏 SWT GC raster | 分别保留基线，不计算倍数 |
| 1% / 100% 更新 | mutation + validation + recording | mutation + validation + 离屏 SWT GC raster | 分别保留基线 |
| TextFlow | layout + command recording | `FlowPage` / `TextFlow` layout + SWT text raster | 逻辑流相同，结构和末端阶段不同 |
| Viewport | clip/transform command recording | clip/transform + 离屏 SWT GC raster | 分别保留基线 |
| Label | 引擎级缓存刷新与样式遍历 | 逐 Label 缓存 preferred-size 查询 | 只验证宽/深渐进性 |

Draw2D 的 512 个逻辑 TextFlow 各需要一个 `FlowPage` wrapper，所以报告中的逻辑规模与
Novadraw 相同，但 `painted_figures` 为 1,025。该结构差异已显式记录，不能用对象数直接
解释耗时。

## 进程内存

Novadraw 使用每场景 `getrusage` 峰值 RSS；Draw2D 使用 `/usr/bin/time -l` 的进程峰值
RSS，并另记 JVM heap。Draw2D 进程还包含 JVM、128 MiB 初始堆策略和 SWT，因此总 RSS
可用于部署足迹观察，但不能归因成 Draw2D Figure 的净内存。

代表性采样后峰值：

| 场景 | Novadraw | Draw2D |
|---|---:|---:|
| 宽树 4,096 | 48.58 MiB | 289.41 MiB |
| 深树 10,000 | 142.69 MiB | 488.73 MiB |
| 深链 validation 10,000 | 84.36 MiB | 130.63 MiB |
| 独立 routing 1,000 | 30.34 MiB | 141.98 MiB |
| TextFlow 深树 512 | 19.44 MiB | 188.03 MiB |

## 结论与限制

- 同环境、同逻辑规模的 Draw2D runner 已可复跑，GA-2 的“无 Draw2D 原始对照”缺口关闭。
- 当前证据不支持“Novadraw 性能总体超过 Draw2D”。
- 直接同阶段证据表明 Novadraw 深链 validation 和 connection routing 仍有明显差距。
- 绘制类场景必须等 Novadraw Native GPU submission / execution / present runner 完成后，
  才能形成更接近端到端的对照。
- 本报告不包含 Native 窗口 present、GPU execution、input-to-present 或 WebGPU。
