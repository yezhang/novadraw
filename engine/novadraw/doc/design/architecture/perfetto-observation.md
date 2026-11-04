# 使用 Perfetto 采集性能观测

类型：`normative-design`

能力归属：Core 提供 CPU 阶段埋点；Native 示例宿主持有 Perfetto 会话并导出 trace。
Editor、Inspector、渲染后端和平台适配器没有新增业务能力。

日期：2026-11-04。状态：用户已授权，替换实现已完成。
本文记录当前采集合同及本轮验证；Native 窗口人工操作未重新验收。

## 替换范围

删除 Runtime 的 `TimingRecorder`、`RuntimePhaseTiming`、inclusive／exclusive 聚合表和
Flow 请求观测缓冲，移除对应的开关与 drain API。删除依赖这些输出的 Python 归因采样器
和专用 `textflow-perf` 驱动。历史报告与冻结证据保留，旧命令不再作为当前入口。

研究应用保留 Large 场景、固定操作和原生／无窗口宿主，使用 SDK 会话输出 `.pftrace`，
替换自研 JSONL journal。统计查询交给 Perfetto UI 和 Trace Processor。
Inspector 的树查询、通知订阅、故障诊断，以及保证布局正确性的准备预算仍属于引擎合同，
不随性能采集工具一起删除。Flow 的确定性工作计数可继续用于缓存与布局契约验证；
累计纳秒字段移除，耗时由 trace 中的真实事件区间查询。

## SDK 与所有权

直接使用 `perfetto-sdk 1.2.1`，由其管理时间戳、事件缓冲、字符串驻留和 trace 编码。
SDK 的进程级 producer 与 category 注册在宿主初始化时执行；Core 不创建会话、
初始化 producer、写文件或新增 Singleton。一次运行可以采集多个 Runtime，
事件携带既有 Runtime namespace，不能用局部 Figure 编号区分不同实例。

Core 的 Native `perfetto` feature 是显式可选依赖。默认构建不链接原生 SDK；
Web/Wasm 的默认 Core 继续编译，本次不自研 Web 采集器或声称原生 SDK 已通过 Web 验收。
公开领域签名不暴露 Perfetto 类型；既有文字阶段诊断入口只保留轻量 SDK 埋点适配，
不再持有聚合器、请求缓冲或自研计时算法。

## 阶段与请求事件

保留 Runtime 稳定化、候选准备、Flow 贡献组装、文字引擎和 IR 准备的阶段边界。
作用域结束使用 SDK guard；正常返回和错误返回都结束对应区间。
自定义文字引擎仍可通过同一入口提供诊断阶段名，阶段名不参与行为身份或缓存资格。

完整请求信息使用独立 category，只有宿主显式启用后才生成诊断参数。
记录既有请求、来源版本、缓存结果和成功／失败事实，不复制到另一份待 drain 队列。
关闭 category 时不为埋点读取时钟、格式化请求或分配诊断记录。

宿主在帧操作之外开始、停止和导出会话，避免在一个活动阶段中切换会话。
导出失败保留可重试的会话数据并返回错误；新文件不得覆盖已有 trace。
缓冲容量由 SDK 配置管理，采集丢失必须通过 Perfetto 的 trace 状态检查，
不能把有丢失的 trace 当作完整调用分母。

研究宿主配置 128 MiB SDK 环形缓冲，并每秒刷新一次字符串增量状态。
F8 停止后保留会话；重新开启前先导出，F7 在采集开启时导出并继续新会话。
每个会话包含构建与输入配置，不保留另一份待导出的事件列表。

## 验证与性能边界

先验证官方 SDK 原生录制和官方 protobuf decoder，再迁移消费者。
定向验证覆盖开启／关闭观测的布局、glyph 与行指标等价，外部文字引擎、
错误返回后的旧快照留存、Runtime 身份区分，以及实际 `.pftrace` 录制与解码。
API 清单与当前文档入口同步移除已删除接口。构建检查覆盖默认 Core、Native feature、
两个研究宿主与 Web 默认 Core。

用 Perfetto 的官方工具检查示例 trace 的阶段、请求参数、时间线和采集状态。
SDK 接入不证明开销低于旧计时器，也不会关闭生产组装复用或性能预算门禁。
后续比较需要分别测量未编译观测、编译但未采集、阶段采集和完整请求采集。
CPU 阶段事件不代表 GPU 完成或屏幕呈现；本次不补造 GPU 时间戳。

SDK 接口依据：
[Rust SDK](https://perfetto.dev/docs/getting-started/rust-sdk)、
[Track Events](https://perfetto.dev/docs/instrumentation/track-events)、
[Trace Processor](https://perfetto.dev/docs/analysis/trace-processor)。

## 本轮验证

Rust 1.96.0、macOS ARM64 下，默认 Core 和 Native `perfetto` feature 的三个
定向消费者共 30 项测试均通过。SDK decoder 验证当前测试线程的阶段成对结束、
外部引擎事件、失败请求、旧快照留存和两个 Runtime 的 namespace。
按 SDK packet sequence 区分测试线程，避免把其他并行测试跨采集边界的片段
误判为本次同步调用没有结束。

研究宿主测试录制真实 trace，覆盖原文、追加、缩窄、预算失败后的重建、
后端重试与失败，以及停止会话重复调用、导出失败保留数据和文件不覆盖。
默认 Web Core check、workspace Clippy 及 Native／Web 公开 API 边界检查通过。
`cargo test --workspace` 最终通过：1909 项通过、26 项按既有配置忽略、0 项失败，
统计包含文档测试。第一次全仓运行发现历史报告仍链接到已删除的计时器源码；
改为提交历史定位并修正文档类型后，文档门禁和完整回归重新通过。
`cargo xtask verify text.long-layout` 也已通过，最终 smoke 直接运行 SDK 研究宿主，
导出 `.pftrace`；日志在 `target/verification/perfetto-text-suite.log`。

官方 Trace Processor v59.1 的 mac-arm64 二进制按官方 manifest 的 SHA-256
`8488edbefadfc4be152cda1d42d9bfcede8b47638295eb85eb3bad8cd148fdf4` 校验。
完整请求回放的结果如下，均为 debug 构建的采集验证，不用于性能结论：

| trace 配置 | host 事件 | phase 事件 | request 事件 | 未结束 slice | 丢失／错误 stats |
|---|---:|---:|---:|---:|---:|
| 完整请求，一轮 13 个操作及 13 帧 | 27 | 641812 | 5040 | 0 | 0 |
| 仅阶段，初始场景及一帧 | 3 | 66260 | 0 | 0 | 0 |

关闭采集时同一初始场景运行成功，没有创建导出目录。
首次使用 32 MiB 缓冲的完整请求回放发生覆盖和增量状态丢失，该 trace 无效；
扩大 SDK 缓冲并启用增量刷新后，上表完整回放通过。不把一次通过解释为
任意时长或任意场景都不会丢失。

本地产物位于
`target/verification/textflow-large-app/perfetto-v2/`、
`target/verification/textflow-large-app/perfetto-phase-only/` 和
`target/verification/textflow-large-app/sdk-contract/`；
架构、workspace Clippy 与回归日志分别在
`target/verification/perfetto-architecture.log`、
`target/verification/perfetto-workspace-clippy.log`、
`target/verification/perfetto-workspace-test.log`。
使用方法见[研究应用](../../../crates/novadraw/examples/native/textflow-large-app/README.md)。
