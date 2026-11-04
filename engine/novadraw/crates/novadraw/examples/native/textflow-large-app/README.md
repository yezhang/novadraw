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

默认不采集，`--trace` 开启 `novadraw.phase`、`novadraw.host` 和
`novadraw.backend`；`--requests` 额外开启 `novadraw.request` 与逐 run 的
`novadraw.glyph`，按需格式化完整 Flow 请求与来源版本。
默认构建的 Core 不链接 SDK，本应用显式启用 Native `perfetto` feature。

滚动专项采集：

```sh
python3 tools/scripts/run_textflow_large_macos.py --release -- --scroll-replay --cycles 2 --trace --exit-after-replay --output target/verification/textflow-scroll
```

`--scroll-replay` 在同一个 Runtime 和窗口中先预热，再依次采集 full redraw
和自然 damage。每组每轮包含 24 次直接 viewport 滚动、24 次经 Winit adapter
的固定像素滚轮输入，以及 12 次固定指针移动和 12 次 Stable。
滚动 delta 为正负 40 逻辑像素配对，不包含追加文本或 resize。
`--natural-damage` 可仅采自然 damage；普通模式仍保留原 full redraw 行为。
自然 damage 的 Idle 单独记为 `no_damage`，不伪装为呈现成功。
固定输入不是 OS 输入延迟；GPU 调用区间也不等于 GPU 执行时间。
手动滚轮使用相同打点，记录来源、phase、delta、实际位移及关联 operation。
本轮数字、有效 trace、SQL 和整改建议见
[滚动诊断](../../../../../doc/verification/reviews/textflow-scroll-perfetto-2026-11-04.md)。

纹理复用实验使用 `--text-raster-cache`，默认关闭。仅缓存符合条件的文字绘制范围，
按内容坐标保存 512 设备像素高的块，原始纹理驻留预算为 16 MiB；
内容、样式、字体或宽度变化使缓存失效，不合格输入走原路径。
设计和限制见[实验方案](../../../../../doc/design/rendering/text-scroll-raster-reuse.md)，
采样与像素差异见[实验结果](../../../../../doc/verification/reviews/textflow-scroll-raster-experiment-2026-11-04.md)。
`--coarse-trace` 保留宿主与后端阶段，关闭细粒度 Core 阶段，用于匹配成本对照。

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --clt --target-dir /tmp/novadraw-text-stage-target -- --scroll-replay --natural-damage --text-raster-cache --coarse-trace --exit-after-replay --output target/verification/textflow-scroll/raster
```

`--offscreen-gpu` 使用已有 GPU target 提交路径，跳过窗口 drawable 获取和呈现；
即使锁屏仍可采样，但结果不能代表窗口呈现或实际 FPS。
`--raster-validation` 对连续下滚／回滚、追加、变宽和重置保存逐帧 GPU 图片，
与关闭缓存的同序列比较。该选项包含同步读回，只用于正确性检查。

启动脚本默认使用当前环境的 Rust 工具链；`--toolchain` 仅用于主动指定覆盖，
常规启动不固定为历史采样使用的 Rust 1.96.0。

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
SDK 使用 256 MiB 环形缓冲和 32 MiB producer 共享内存，
每秒刷新增量字符串状态。两者分别应对整次采集容量和突发写入。
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
