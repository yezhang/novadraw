# Flow 组装复用开关的原生滚动成本对照

类型：`verification-record`

能力归属：Core Flow、Preparation 与 Runtime 稳定化；Vello backend；
TextFlowLarge Native 研究宿主。Editor 不在范围。
日期：2026-11-05。状态：匹配原生滚动对照完成，确认本批开启复用后成本回退；
未实施性能修复，FR-02 净性能门禁保持未通过。

## 结论与成本归属

本批固定滚轮的活跃耗时中位数从 **19.83 ms 增至 30.93 ms**，增加约 56%。
开启 `--flow-assembly-reuse` 后，滚动虽命中组装缓存，
Core 稳定化与帧准备的额外成本仍超过节省的组装工作。
关闭复用时滚动也已超过 60 Hz 的 16.67 ms 工作预算，
因此当前卡顿同时包含原有整页文字提交成本和复用路径新增成本。

以下按 96 次固定滚轮取平均，区间互不重叠。
帧内非 backend 区间以 `host-frame - host-backend-submit` 计算，
包括 Core 帧准备、完成确认、宿主统计和对象释放，不能全部称为输入校验。

| 区间 | 关闭 ms | 开启 ms | 差值 ms |
|---|---:|---:|---:|
| host-operation | 6.128 | 13.090 | +6.963 |
| 滚轮 dispatch 在 operation 外的部分 | 0.231 | 0.240 | +0.009 |
| 帧内非 backend 区间 | 3.649 | 7.811 | +4.162 |
| host-backend-submit | 9.890 | 9.871 | −0.020 |
| 活跃耗时合计 | 19.898 | 31.012 | +11.114 |

operation 的新增时间几乎全部在 `host-stabilize-query`：
该区间均值从 6.097 增至 13.056 ms；
snapshot summary 均值仅从 0.0074 增至 0.0083 ms。
backend 的 `backend-vello-render` 均值为 8.548／8.543 ms，
surface acquire 为 0.023／0.025 ms，present 为 0.0095／0.0098 ms。
这些数据不支持把本批开关回退主要归因于 Vello、drawable 获取或 present。

Stable 控制操作全部 `no_damage`，没有 backend submit，
活跃耗时中位数仍从 4.00 增至 12.54 ms。
这说明新增成本在不绘制文字时也存在。
两组滚动帧仍各提交 7609 条命令、51232 个 glyph；
关闭开关不能消除原有的整页提交负担。

代码核对显示，[复用资格 reader](../../../crates/novadraw/src/figure/flow/reuse.rs)
每次重读 source prefix 中的 contributor 与 handler 声明并构造 participants；
[Flow 构建路径](../../../crates/novadraw/src/figure/flow/snapshot.rs)
在缓存比较、采用与布局返回时保持输入证明；
[布局与最终发布检查](../../../crates/novadraw/src/graph/presentation.rs)
还会重验缓存布局和准备结果的输入。
宿主 operation 主动 `stabilize_for_query`，之后 prepare_submission 再稳定化。
这些路径是下一步应定向测量的重点，不能通过删除正确性检查来缩短时间。
**本批粗阶段 trace 确认了回退所在的 Core／宿主边界，
没有测出 qualification、proof 比较、分配和释放各自的耗时占比。**

## 帧与控制操作

“活跃耗时”是输入根区间加对应 host-frame；直接操作使用 operation 加 host-frame。
输入根区间已包含 operation，不重复求和；不含两者之间的调度等待。
host-frame 包含准备和 backend submit，不是 GPU 执行时间或屏幕帧间隔。
P95 使用 nearest-rank。

| 操作 | 每组样本 | 活跃中位 ms：关／开 | 活跃 P95 ms：关／开 | host-frame 中位 ms：关／开 |
|---|---:|---:|---:|---:|
| 直接 viewport 滚动 | 96 | 19.64／31.05 | 21.60／35.95 | 13.52／17.91 |
| 固定滚轮输入 | 96 | 19.83／30.93 | 21.04／32.94 | 13.47／17.61 |
| 固定指针 | 48 | 4.32／12.71 | 4.67／13.86 | 2.01／6.31 |
| Stable | 48 | 4.00／12.54 | 4.44／13.72 | 2.01／6.26 |

固定滚轮每轮 24 次，四轮活跃耗时中位数：
关闭 19.664／19.561／20.097／20.085 ms；
开启 30.843／31.103／30.941／30.744 ms。
回退在四轮中持续存在，不能只解释为首帧冷启动。
直接滚动仍有后端长尾，开启组最大活跃耗时 52.46 ms，
其中 Vello 调用 21.28 ms；这部分与普遍存在的 Core 增量成本分别判断。

## 匹配与证据有效性

两次构建均命中现有 release 产物，Rust 1.99.1；相同 checkout、字体、
600 × 500 逻辑视口、1200 × 1000 物理 surface、自然 damage、
相同 preparation 预算。仅改变复用开关；纹理缓存、GPU wait、截图和请求采集关闭，
两组均 `--coarse-trace`，只开启宿主与 backend category。

每组 288 个比较样本，192 次滚动全部成功呈现且位移为 ±40，
96 次 Pointer／Stable 全部 `no_damage`。
完整会话均记录 193 次 Presented、98 次 no_damage、
初始化 1 次 Skipped 后恢复，最终均 `replay_complete`，没有 failure／frame_failure。
两份 trace 均无 health error／data_loss、未结束区间或负自身耗时。
Trace Processor v59.1 的 arm64 syscall parser 提示不影响本批 SDK track events；
本批没有采集 syscall 或调度数据。

逐样本比较正文、段数、行数、runs、fragments、有效宽度、首来源文字、
操作顺序、位移、damage、命令、glyph 和 outcome，均相同：
51232 bytes、40 段、561 行、1022 runs、4168 fragments、有效宽度 586。
Flow revision 在每组内保持不变；不同 Runtime 的 revision 数值不同，
不要求跨实例身份相同，也没有把两者数值相等作为输出等价条件。
这些分母核对不代替逐像素正确性比较。

最终累计 Flow build 两组均 397，styled layout 均 6；
assembly build 从 397 降至 3，开启组累计 hit 为 394。
除第一个滚动样本外，191 次滚动通过相邻操作计数求差：
两组均 382 次 Flow build、0 次新增 styled layout 和 source geometry；
关闭为 382 次组装构建，开启为 382 次命中、0 次组装构建。
组装命中没有消除重复测量入口和输入重验。

本批先关后开，每种配置仅一个进程、四轮串行回放，没有 ABBA 跨进程重复。
没有关闭 SDK 采集后的逐帧计时，不能把约 56% 当作无观测生产回退比例。
固定滚轮经 Winit adapter，不覆盖 OS 惯性滚动、事件积压、WindowServer 合成、
GPU timestamp 或屏幕 scanout。两组都超过工作预算支持继续整改，
不能据此宣称实际 FPS 或系统到屏幕延迟。

用户同日手验确认 F2 不再退出，resize 更流畅，滚动仍稍有卡顿。
该反馈不扩展为窗口切换后或 F1 重置后的按键验收。

## 复跑入口

证据根目录为 `target/verification/text-fr02-scroll-2026-11-05/`，不纳入 Git：

| 配置 | trace 相对证据根目录的路径 | SHA-256 |
|---|---|---|
| 关闭 | `off/textflow-1793847789958361000-89908-0.pftrace` | `3fe665ac744c2384167618f2de04180107b9432224957af1347d87aa59390c97` |
| 开启 | `on/textflow-1793847814634897000-90042-0.pftrace` | `4c1e591523b31909d3492196f84a6fa5666623cd5b37297243f8aa82d5cb305b` |

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --clt --target-dir /tmp/novadraw-text-stage-target -- --scroll-replay --cycles 4 --natural-damage --coarse-trace --exit-after-replay --output target/verification/text-fr02-scroll-2026-11-05/off
python3 tools/scripts/run_textflow_large_macos.py --release --clt --target-dir /tmp/novadraw-text-stage-target -- --scroll-replay --cycles 4 --natural-damage --flow-assembly-reuse --coarse-trace --exit-after-replay --output target/verification/text-fr02-scroll-2026-11-05/on
```

已执行的分析使用现有
[SQL](../../../tools/scripts/textflow_scroll_perfetto.sql) 和
[分析器](../../../tools/scripts/analyze_textflow_scroll.py)；
分别传入上述 trace、独立 `--session` 和 `--output`。
输出保存在 `off-analysis/` 与 `on-analysis/`，含 schema、健康检查、
原始 SQL CSV、`records.json` 和逐样本／分组统计 `analysis.json`。
每份 trace 只载入一个 warm session，分析完成后均已关闭。

下一步先定向量化输入 reader／proof 校验与稳定化重复调用，
再按完整输入契约减少重复工作；整页 glyph 提交另按既有安全裁剪方案推进。
本轮只形成诊断证据，没有修改引擎或应用代码。
