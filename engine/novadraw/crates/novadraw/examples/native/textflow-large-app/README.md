# TextFlowLarge 性能研究应用

能力归属：应用／宿主。直接复用 `draw2d-official-examples` 的 Large 场景，
通过既有 Core、Vello backend 和 Winit adapter 研究长文本排版与绘制。
该应用提供固定输入、逐帧回放和诊断导出。

从仓库根目录运行：

```sh
cargo run -p textflow-large-app
```

macOS 推荐通过 LaunchServices 打开窗口：

```sh
python3 tools/scripts/run_textflow_large_macos.py
```

窗口保持原 Large 的输入和滚动行为。研究操作使用功能键，避免占用文本字符：

| 按键 | 操作 |
|------|------|
| F1 | 新建 Runtime，恢复原文、滚动位置和 600 × 500 视口 |
| F2 | 经 Core 输入追加 `Z` |
| F3 | 固定向下滚动 100 |
| F4 | 在 600 和 400 逻辑像素宽之间切换 |
| F5 | 执行稳定查询 |
| F6 | 在新 Runtime 上执行固定回放 |
| F7 | 导出当前记录 |
| F8 | 切换 Core timing，切换事件写入导出 |
| Escape | 导出并关闭 |

macOS 若将功能键映射到系统操作，请同时按 Fn。
默认关闭 Core timing；`--timing` 开启阶段耗时与布局调用诊断。
`--requests` 同时开启 timing，并导出完整请求的 Debug 表示，文件会明显增大。

固定回放每轮依次执行：重建、追加、向下滚动、回滚、稳定查询、
宽度 550 → 500 → 450 → 400 → 450 → 500 → 550 → 600。
每个操作稳定化成功且对应帧获后端确认后才进入下一项。
自动回放期间用户输入暂停，窗口实际尺寸确认后才执行下一次 resize。

```sh
cargo run -p textflow-large-app -- --replay --cycles 3 --timing --exit-after-replay
python3 tools/scripts/run_textflow_large_macos.py -- --replay --timing --gpu-wait --screenshot --exit-after-replay
cargo run -p textflow-large-app -- --headless --replay --cycles 3 --timing
```

无窗口模式使用相同工厂、Runtime 和 Vello Scene 编码器，不初始化窗口或 GPU。
`--headless` 单独使用时只执行首次场景；`--cycles` 会启用回放。
`--gpu-wait` 与 `--screenshot` 仅供 Native 使用。
Native 默认在回放结束后保留窗口，`--exit-after-replay` 才自动关闭。

JSONL 默认写入忽略的 `target/verification/textflow-large-app/`；
`--output DIRECTORY` 可指定目录。F7、关闭窗口及回放结束均会导出；
进入下一轮或手动重建前也会导出上一轮，避免多轮诊断持续堆积。
文件使用独立名称且不覆盖既有导出，写入成功后清空已导出的缓冲。
缓冲上限为 8192 条、32 MiB，尾记录明确给出丢弃数量。
同一次运行的多个文件通过配置中的 `run_id` 和连续 `operation_id` 关联。
`--screenshot` 保存首个成功帧的 GPU retained target，不是桌面截图。

每项操作记录 mutation、stabilization 两个墙钟区间和 Flow 快照分母；
每帧记录 CPU IR 准备、backend submit、命令数、录制 glyph 数、尺寸和帧标识。
研究帧统一请求 full redraw，保证稳定查询也能录制同口径的完整帧；
因此这些数据不能当作默认增量 damage 路径的成本。CPU IR 准备区间覆盖整个
`prepare_submission`，其中可能包含再次稳定化；开启 timing 后可查看内部阶段。
同一 `operation_id` 关联操作、阶段耗时、布局尝试、帧与可选 GPU queue wait。
Flow work 是当前 Runtime 内累计计数；比较相邻成功操作时取差值，reset 后重新计算。
失败保留错误和重试，失败操作的快照为 null；终止失败后需用新 Runtime 重试。

这些区间不含导出序列化和文件写入。阶段 inclusive／exclusive 耗时有嵌套关系，
不能将不同阶段的分位数相加。Native `Presented` 表示后端提交确认，
GPU queue wait 表示队列同步，两者都不能证明 WindowServer 合成或屏幕 scanout。
无窗口帧标为 `cpu_recorded`；录制 glyph 数也不等于实际可见 glyph 数。
调试构建、timing、完整请求导出及 GPU wait 均可能影响观测成本，比较时需保持配置一致。

构建元数据记录 package 版本、rustc、profile、build script 执行时的 Git HEAD
与工作区 dirty 状态；它不是源码内容哈希，也不代表应用运行时的工作区状态。
当前缓存与 shaping 策略由引擎决定；本应用没有新的优化策略开关。

实施与验收入口见
[研究计划](../../../../../doc/verification/plans/textflow-large-research-app.md)
及[实施记录](../../../../../doc/verification/reviews/textflow-large-app-implementation-2026-11-03.md)。
