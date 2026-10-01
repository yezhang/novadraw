# GA-2 性能测量方法

类型：`verification`

## 测量边界

`core.performance` 固定运行无窗口、release 配置的 Novadraw CPU 场景。报告包含环境、
场景配置、预热次数、原始纳秒样本、p50/p95、确定性工作量计数和进程峰值 RSS。

当前 harness 测量：

- Runtime 构建或场景准备；
- validation、文本布局刷新、连接 routing；
- 全量或增量 CPU command recording；
- Figure、command、样式节点访问、routing order 等确定性工作量；
- 每个场景独立进程的启动基线、场景构建后和采样后峰值 RSS。

当前 harness 不测 GPU submission、GPU execution、present 或 input-to-present。这些字段
在 JSON 中显式为 `false`，不能以 command recording 时间、峰值 RSS 或工作量计数替代。

## 内存口径

不指定 `--scenario` 时，父进程按固定顺序为每个场景启动独立子进程，再合并子报告。
这样场景之间不会共享 allocator、字体缓存或此前场景的峰值。单场景运行本身也是独立
进程，报告中的 `process_isolated` 必须为 `true`。

内存字段使用进程生命周期峰值 RSS：

- macOS：`getrusage(RUSAGE_SELF).ru_maxrss`，原始单位为 byte；
- Linux 与其他 Unix：同一接口，原始单位按 KiB 转为 byte；
- Windows：`GetProcessMemoryInfo().PeakWorkingSetSize`，原始单位为 byte；
- 不支持的平台返回 `null`，并将 `measurement_scope.process_memory` 记为 `false`。

`setup_peak_growth_bytes` 是场景构建后峰值减启动基线；
`sample_peak_growth_bytes` 是采样后峰值减构建后峰值。它们反映进程峰值增长，不等同于
活跃堆大小、Rust allocator 独占内存或 GPU 显存。

## 固定配置

- logical viewport：1024 × 768；
- logical DPI：96；
- 默认预热：5 次；
- 默认采样：30 次；
- 字体场景：仓库内置 Inter；
- 输出：`target/verification/reports/ga2-novadraw.json`；
- profile：Cargo release。

场景覆盖宽树、1k/10k 深树、真实 Label/TextFlow、独立/分组连接、1%/100% 更新和
viewport scroll/zoom。每个场景在报告内记录 Figure 数、最大深度、可见比例、更新比例
和固定输入轨迹。

## 执行

```bash
cargo xtask verify core.performance
```

开发时可只运行单个场景：

```bash
cargo run --release -p r8-perf -- \
  --scenario=label_refresh_deep_1000 \
  --report=target/performance/label-deep.json
```

macOS 上的 Draw2D 同环境对照：

```bash
cargo xtask verify reference.draw2d-performance
```

Draw2D runner 位于 `third_party/draw2d-examples/`，不属于 Novadraw 示例或实现。
它固定 Draw2D JAR 源码提交、SWT artifact、JVM 参数、Inter 字体、viewport、逻辑 DPI、
预热和采样次数。每个场景使用独立 JVM，报告写入
`target/verification/reports/ga2-draw2d/`。

Native Vello GPU queue completion：

```bash
cargo xtask verify backend.native-performance
```

该 runner 先探测真实 surface submit。可见 drawable 不可用时，转为离屏 Vello GPU
submit，并在 JSON 中把 `surface_present_call`、`compositor_present` 和
`input_to_present` 明确记为 `false`。`wait_for_gpu_idle` 的墙钟结果证明此前 queue
work 已完成，但不是硬件 timestamp 或 compositor present 时间。结果见
[Native Vello GPU 基线](ga2-native-vello-2026-10-01.md)。

需要真实可见 surface 时使用严格入口：

```bash
cargo xtask verify backend.native-presentation-performance
```

该 suite 在 macOS 上构建 app bundle，并通过 LaunchServices 进入当前登录会话。它会
先检查控制台是否解锁，运行期间保持显示唤醒，并向 harness 传入
`--require-surface-present`。锁屏、5 秒内始终 occluded、资源持续 Retry 或没有生成报告
都会令 suite 失败，不允许回退离屏后返回 PASS。

2026-10-01 已在解锁、focused/unoccluded 的 macOS 登录会话执行通过。该严格入口只能
证明生产 Vello backend 成功调用 `SurfaceTexture::present()`；JSON 中的
`compositor_present` 和 `input_to_present` 仍保持 `false`，直到接入平台呈现回执和
真实输入时间戳。结果见
[Native Vello GPU 基线](ga2-native-vello-2026-10-01.md)。普通
`backend.native-performance` 继续允许无窗口环境降级，两者不能互相替代。

普通 CI 只校验场景可复跑且确定性工作量稳定，不使用跨机器墙钟或 RSS 阈值。
p50/p95 与峰值 RSS 只能在相同硬件、操作系统、工具链、profile、场景配置和采样口径
下比较。

## 对照约束

Draw2D 对照必须使用相同 OS、硬件、viewport、DPI、字体、Figure 数、可见比例、更新
比例和输入轨迹，并保存两端提交、JVM/Rust 工具链、预热与原始样本。完成同场景 runner
前，不得声称 Novadraw 在未测场景上快于 Draw2D。

Draw2D 的离屏 SWT GC paint 包含 traversal 与 raster，而 Novadraw headless harness
止于 CPU command recording。这两类结果只能分别观察，不能计算性能倍数。当前可直接
对照的是 validation 与 connection routing；具体边界和结果见
[Draw2D 同环境对照基线](ga2-draw2d-comparison-2026-10-01.md)。

Web 结果单独记录浏览器版本、WebGPU adapter、GPU 和 present；Draw2D 不存在 Web
运行结果，因此 Web 预算不能伪装成跨框架对照。
