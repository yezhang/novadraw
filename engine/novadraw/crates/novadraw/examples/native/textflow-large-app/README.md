# TextFlowLarge 性能研究应用

能力归属：应用／Native 宿主。复用官方 Large 场景、Core、Vello 和 Winit，
由官方 Perfetto Rust SDK 采集 CPU 阶段及请求，导出 `.pftrace`。

从仓库根目录运行：

```sh
cargo run -p textflow-large-app -- --trace
python3 tools/scripts/run_textflow_large_macos.py -- --trace
cargo run -p textflow-large-app -- --headless --replay --cycles 1 --trace
cargo run -p textflow-large-app -- --headless --replay --trace --requests
```

默认不采集，`--trace` 开启 `novadraw.phase` 和 `novadraw.host`；
`--requests` 额外开启 `novadraw.request`，按需格式化完整 Flow 请求与来源版本。
默认构建的 Core 不链接 SDK，本应用显式启用 Native `perfetto` feature。

| 按键 | 操作 |
|------|------|
| F1 | 新建 Runtime，恢复原文、滚动位置和 600 × 500 视口 |
| F2 | 经 Core 输入追加 `Z` |
| F3 | 向下滚动 100 |
| F4 | 在 600 和 400 逻辑像素宽之间切换 |
| F5 | 稳定查询 |
| F6 | 固定回放 |
| F7 | 停止、导出当前会话；采集开启时继续新会话 |
| F8 | 启停采集；重新开启前导出上一份已停止的会话 |
| Escape | 导出并关闭 |

macOS 功能键可能需要同时按 Fn。
每轮回放执行重建、追加、下滚、回滚、稳定查询，再执行
550 → 500 → 450 → 400 → 450 → 500 → 550 → 600 的宽度变化。
前一帧获后端确认后才推进下一项。`--cycles N` 启用回放；
`--exit-after-replay` 在 Native 回放完成后关闭窗口。
`--gpu-wait` 与 `--screenshot` 仅支持 Native，后者保存首个成功帧的 GPU target。

trace 写入忽略的 `target/verification/textflow-large-app/`，
`--output DIRECTORY` 可指定目录。重建、回放结束和关闭时导出；
文件不覆盖，写入失败保留 SDK 编码字节以便重试。
SDK 使用 128 MiB 环形缓冲，每秒刷新增量字符串状态。
长时间采集或完整请求仍可能超出容量：必须检查 Perfetto 的 trace health，
存在数据丢失时不能据此计算完整调用次数。

在 [Perfetto UI](https://ui.perfetto.dev) 打开 `.pftrace`，或使用官方 Trace Processor：

```sh
trace_processor query TRACE.pftrace \
  "SELECT name, count(*) AS calls, sum(dur)/1e6 AS cpu_ms FROM slice WHERE dur >= 0 GROUP BY name"
trace_processor query TRACE.pftrace \
  "SELECT name,value FROM stats WHERE value > 0 AND severity IN ('error','data_loss')"
```

阶段事件包括 Runtime 稳定化、Flow 组装、外部文字引擎和 IR 准备，
携带 Runtime namespace；请求事件记录 owner、来源版本、缓存命中和失败事实。
宿主事件记录操作、布局计数、帧确认及构建元数据，查询 `args` 可查看参数。
Flow 的确定性计数仍可用于复用验证，耗时从 SDK slice 查询。

研究帧统一请求 full redraw，不代表默认增量 damage 成本。
CPU slice、backend `Presented` 和 GPU queue wait 都不证明屏幕 scanout；
无窗口帧标为 `cpu_recorded`，glyph 数也不等于实际可见 glyph 数。
SDK 接入没有证明观测开销降低，也没有关闭生产性能门禁。

当前合同与验证见
[Perfetto 接入](../../../../../doc/design/architecture/perfetto-observation.md)。
原 JSONL 和计时聚合器已删除；旧版验收记录仅供历史追溯。
