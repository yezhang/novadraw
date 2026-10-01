# GA-2 性能测量方法

类型：`verification`

## 测量边界

`core.performance` 固定运行无窗口、release 配置的 Novadraw CPU 场景。报告包含环境、
场景配置、预热次数、原始纳秒样本、p50/p95 和确定性工作量计数。

当前 harness 测量：

- Runtime 构建或场景准备；
- validation、文本布局刷新、连接 routing；
- 全量或增量 CPU command recording；
- Figure、command、样式节点访问、routing order 等确定性工作量。

当前 harness 不测 GPU submission、GPU execution、present、input-to-present 或进程内存。
这些字段在 JSON 中显式为 `false`，不能以 command recording 时间或工作量计数替代。

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

普通 CI 只校验场景可复跑且确定性工作量稳定，不使用跨机器墙钟阈值。p50/p95 只能在
相同硬件、操作系统、工具链、profile、场景配置和采样口径下比较。

## 对照约束

Draw2D 对照必须使用相同 OS、硬件、viewport、DPI、字体、Figure 数、可见比例、更新
比例和输入轨迹，并保存两端提交、JVM/Rust 工具链、预热与原始样本。完成同场景 runner
前，不得声称 Novadraw 在未测场景上快于 Draw2D。

Web 结果单独记录浏览器版本、WebGPU adapter、GPU 和 present；Draw2D 不存在 Web
运行结果，因此 Web 预算不能伪装成跨框架对照。
