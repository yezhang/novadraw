# GA-2 Novadraw CPU 基线与复杂度整改

类型：`verification`

日期：2026-10-01

状态：`complete`（仅 Novadraw headless CPU 子阶段）

方法：[GA-2 性能测量方法](ga2-methodology.md)

## 环境与证据

| 项目 | 优化前 | 优化后 |
|---|---|---|
| 提交 | `b587acfa0e40edfc2bb53d1ddc388b46c69ebe11` | `01c3c557926ed30e1abba29869b24d55329b7605` |
| 工作区 | clean | clean |
| OS / CPU | macOS / Apple M1 Pro | macOS / Apple M1 Pro |
| 架构 | aarch64 | aarch64 |
| Rust | 1.94.1 | 1.94.1 |
| Profile | release | release |
| 预热 / 采样 | 5 / 30 | 5 / 30 |
| 原始报告 | `target/performance/ga2-before-b587acf.json` | `target/verification/reports/ga2-novadraw.json` |
| SHA-256 | `c7ce99b0d1d7b81d8d2063596def801cae3bf6705fc21669c56f5c6e6f96c39d` | `913eaae494ddc24b8560643ceec3396073fcdaa815da1d0649503351d81f37eb` |

报告由 harness 写出全部原始样本、p50/p95、场景配置和工作量计数。`core.performance`
会重新生成优化后报告；`target/` 证据不提交到 Git。

## 结果

| 场景 | 优化前 p50 / p95 | 优化后 p50 / p95 | 确定性工作量 |
|---|---:|---:|---|
| 宽树全量录制 4,096 | 2.464 / 2.838 ms | 2.192 / 2.526 ms | 53,258 commands |
| 深树全量录制 1,000 | 0.490 / 0.611 ms | 0.294 / 0.608 ms | 12,997 commands |
| 深树全量录制 10,000 | 10.598 / 12.494 ms | 9.411 / 11.859 ms | 129,997 commands |
| 深树 validation 1,000 | 0.093 / 0.113 ms | 0.093 / 0.109 ms | deepest leaf valid |
| 深树 validation 10,000 | 1.663 / 1.939 ms | 1.709 / 1.926 ms | deepest leaf valid |
| 宽 Label 刷新 1,000 | 0.258 / 0.261 ms | 0.270 / 0.369 ms | style visits 3,000 → 1,002 |
| 深 Label 刷新 1,000 | 6.095 / 6.116 ms | 0.243 / 0.249 ms | style visits 502,500 → 1,002 |
| 宽 TextFlow 录制 512 | 0.385 / 0.419 ms | 0.381 / 0.454 ms | style visits 3,072 → 1,028 |
| 深 TextFlow 录制 512 | 3.244 / 3.304 ms | 0.536 / 0.592 ms | style visits 264,704 → 1,028 |
| 独立连接 routing 1,000 | 75.388 / 78.607 ms | 75.444 / 78.369 ms | order entries 1,002,000 → 1,000 |
| 分组连接 routing 256 | 6.544 / 6.976 ms | 6.562 / 6.965 ms | 256 routes / 258 order entries |
| 1% 局部更新 4,096 | 1.711 / 1.957 ms | 1.657 / 2.220 ms | 40 mutations / 53,258 commands |
| 100% 更新 4,096 | 4.162 / 4.735 ms | 4.691 / 5.098 ms | 4,096 mutations / 53,258 commands |
| viewport 录制 1,024 | 0.283 / 0.296 ms | 0.306 / 0.335 ms | 13,347 commands |

深 Label 的 p50 降低约 96%，深 TextFlow 降低约 83%。独立 routing 的顺序构造已从
O(E²) 降为 O(E)，但 1,000 条完整 route + geometry commit 的墙钟没有显著变化，
因此不能把工作量改善表述为端到端 routing 加速。

非文本场景曾因无条件样式传播产生回退。最终实现先按原路径收集能力目标，零目标时直接
返回；`non_text_tree_skips_style_propagation_work` 固定该边界。

## 验证

```text
cargo xtask verify core.performance
PASS

m10_label_contract: 13 passed
m9_connection_runtime: 29 passed
```

连接失败路径同时复核了缺失端点、unresolved 状态和旧几何清理，未以性能修改绕过失败
语义。

## 限制

本报告只覆盖 headless CPU setup、validation、routing 和 command recording，不覆盖：

- GPU submission、GPU execution、present 或 input-to-present；
- 进程峰值或增量内存；
- Draw2D 同场景、同环境对照；
- 浏览器/WebGPU 运行预算。

因此本报告不支持“Novadraw 性能超过 Draw2D”的总体结论，也不单独满足 GA-2 毕业条件。
