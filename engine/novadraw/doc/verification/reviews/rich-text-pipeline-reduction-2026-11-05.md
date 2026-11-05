# 富文本管线删减逐项验证

类型：`verification`

能力归属：Core 负责输入证明、Flow 内容和布局；Vello backend 负责 GPU 编码与绘制；
platform／研究宿主负责 Winit 输入、窗口 resize、计时和采样。本批不修改 Editor 行为。

日期：2026-11-05。状态：实施中，尚未关闭总体方案和 FR-02。
用户已授权按[总体方案](../../design/architecture/rich-text-pipeline-reduction-proposal.md)
执行，每项改进都要比较前后渲染时间，并覆盖鼠标拖拽与 resize 的整体成本。

`api_semantics`：`text.flow`、`text.interaction`、`layout.manager`、
`viewport.scroll_zoom`、`frame.preparation`、`render.backend_session`。

## 采样边界

研究宿主新增 `--interaction-replay` 和 `--timings`。每个 cycle 包含滚动条 thumb
按下、40 次连续移动、释放、40 次 Winit 滚轮、40 次窗口 resize、追加、Stable 和 Pointer。
thumb 位置来自公开布局约束和当前 surface transform；resize 从 600 连续到 400，
再回到 600，每次等待实际窗口确认。拖拽与滚轮必须实际移动 viewport，
失败、缺失帧和零位移假回放不用于宣称性能改善。

独立计时通过实例持有的主机记录导出 JSON，关闭 tracing 后仍工作。
`active_ms` 是输入／操作与帧准备、提交及显式 GPU 队列完成等待的非重叠总时间；
`frame_ms` 只包括 prepare、submit、ack 和 frame 释放，不包含 GPU 等待；
`latency_ms` 包含输入到完成、窗口尺寸确认、重绘调度和 GPU 等待。
GPU 等待不是硬件 timestamp，也不能证明 WindowServer 已在屏幕呈现。
title 与最终 host effects 不计入 active；当前 latency 的终点为后端完成等待后的主机记录。

主对照使用相同 release、字体、51232 字节初始内容、显式 preparation 预算、
assembly reuse 开启、natural damage、raster cache 关闭、offscreen GPU 和 GPU wait 开启。
每版独立进程，三组交替顺序，每进程两个 cycle；拖拽、滚轮和 resize 各有 240 个样本。
收据保存命令、binary SHA-256、配置、逐操作快照、工作量与时间。测试和编译不与采样并行。

当前原生 surface 路径在本次会话没有产生首帧导出，通过 LaunchServices 和直接执行
均未完成，相关进程已停止。既有 `--offscreen-gpu` 路径完成同一 Winit 输入及窗口确认，
用于本批 Core／backend 对照。屏幕呈现和真实 OS 拖动节奏仍未验收；
该限制不允许将本批结果称为完整原生交互验收。

中间产物：`target/verification/text-pipeline-reduction-2026-11-05/`，不纳入 Git。
`baseline/` 保存删减前 binary、相关源码及 working-tree patch；
各 `stepNN/` 保存该步 binary、交替对照收据、逐操作数据和 Perfetto trace。
原始时序不拼接历史样本，不进行产物体积比较。

## 步骤 0：基线与归因

不等待 GPU 完成的补充基线，拖拽／滚轮／resize active P50 分别为
34.46／37.14／76.76 ms，frame P50 为 20.38／22.26／23.11 ms。
这份补充数据不与 GPU wait 开启的主对照混用。

Perfetto SDK trace 已通过完整性检查，无未结束 scope、负 exclusive duration 或 parser
error／data loss。Trace Processor 报告 arm64 syscall tracing 不支持，本批没有 syscall
或调度采证，阶段记录仍可解析。命名分析 session 已关闭。

第一项之后的诊断 trace 显示，每次拖拽和滚轮平均仍有 27 次 input proof 重验、
29 次资格读取及 2 次 measure Flow build；资格阶段平均约 9.1 ms。
每次 resize 平均有 71.75 次 proof 重验、84.7 次资格读取和 12.95 次 Flow build；
资格阶段约 26.15 ms。各阶段是诊断配置下的实测值；
inclusive 阶段嵌套，不能相加或当作可直接回收的时间。

## 步骤 1：删除检查点的证明列表复制

内容、spans、来源、ancestors 和 glyph backing 的交接已由既有共享资产承担。
`paragraph_boxes` 和 inline 前缀的 owned 构造服务于实际排版算法，继续保留。
本步删除 `validate_input_proofs` 调用 `input_proofs()` 所生成的检查点 receipt 副本，
改为直接借用有序首次证明。reader 始终使用独立 `input_scope`，所有注册、约束、
完整值比较和最终检查仍执行；失败在释放借用后锁存原始错误。
测量缓存和候选需要独立 ownership 时，继续生成并计费 receipt。

关闭 tracing、包含 GPU 完成等待的匹配结果（ms）：

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 40.427 → 41.314 | 16.417 → 16.637 | 40.696 → 41.731 | +2.2% |
| 滚轮 | 40.200 → 41.504 | 16.277 → 16.707 | 40.467 → 41.730 | +3.2% |
| resize | 77.615 → 78.741 | 17.227 → 17.654 | 79.590 → 80.522 | +1.5% |
| 追加 | 52.342 → 52.835 | 16.837 → 17.022 | 53.260 → 52.915 | +0.9% |
| Stable | 12.637 → 12.723 | 6.189 → 6.153 | 13.831 → 14.425 | +0.7% |

本项没有取得可宣称的净提速。局部复制已删除，主指标略有回退；
后续需要确认其是否属于进程与 GPU 采样波动，不用代码行数或少量分配推算收益。
753 对操作输出的 bytes、lines、runs、fragments、effective width、首个 source text、
glyph／command 数量及 viewport 位移一致。P95／P99 和完整逐样本保存在
`step01/comparison/comparison.json` 与 `receipts.json`。

输入证明的 11 项既有 lib 回归通过，覆盖忽略冲突、reader 原始错误、注册退役、
测量缓存重验、后续回调改写、布局输出消费、最终批次拒绝与失败恢复。
新增嵌套 reader 反例已随下一批定向 lib 门禁通过；该批 58 项通过、1 项成本测试忽略。

## 步骤 2A：共享资格 metadata

同一不可变 source prefix 的资格 metadata 现已共享。
每次检查仍读取全部 contributor／handler 声明，比较完整身份、generation、
贡献类型序列与输入快照；逐项相同才共享 participants 和 handlers backing。
新声明的临时分配继续计费，首次不同才创建新 metadata，旧快照不变。
source 只持有 participants 的弱引用，不保活过期 metadata 或注册。
该项没有缓存动态 generation，也没有省略 reader 或最终发布检查。

与步骤 1 的匹配对照（同样为三组交替进程，GPU wait 开启，ms）：

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 41.379 → 41.169 | 16.861 → 16.856 | 41.675 → 41.394 | −0.5% |
| 滚轮 | 41.451 → 40.779 | 16.730 → 16.778 | 41.728 → 40.973 | −1.6% |
| resize | 80.197 → 76.471 | 17.728 → 17.733 | 81.954 → 78.302 | −4.6% |
| 追加 | 52.874 → 51.454 | 17.401 → 18.344 | 53.419 → 52.001 | −2.7% |
| Stable | 12.824 → 13.352 | 6.339 → 6.392 | 13.910 → 15.348 | +4.1% |

resize 的整体成本有下降，frame 编码成本没有下降；拖拽改善较小。
Stable 只有 9 个样本，当前中位数上升，不能据此宣布所有操作都更快。
753 对操作的完整摘要、viewport 位移和 glyph／command 数量仍一致。
完整分布与收据保存在 `step02/comparison/`。
定向 lib 门禁 58 项通过、1 项成本测试忽略；Core 和研究宿主 Clippy 通过，
宿主计时类型复杂度告警已提取 alias，并通过后续 Clippy 复验。
独立消费者 integration target 的 8 项测试通过。

步骤 2A 的 Perfetto trace 也已通过完整性检查。每次拖拽仍有 29 次资格读取，
平均 inclusive 时间约 9.18 ms；resize 平均 84.7 次、约 24.80 ms。
与前一步的诊断样本相比，资格重读次数没有下降，不能将 metadata 共享解释为
reader 已被删除。诊断 trace 的活跃时间未包含独立 GPU wait，
不能和上表的主计时混用。

## 步骤 3A：复用完整输入下的纯文字测量尺寸

本项按 FR-02 的[增量合同](../../design/architecture/flow-assembly-reuse.md#管线删减增量纯文字测量复用)
保存四个近期尺寸记录。只有完整声明合格、没有真实 inline boxes、
没有 fragment border、文字引擎提供稳定实例身份时才允许命中。
键完整比较输入快照、引擎身份／revision、bounds、有效宽度和约束；
依赖 observation 继续重验并保留。adoption、return 和最终发布的输入检查继续执行。
留存 metadata 计费，缩小 cache limit 会淘汰记录；普通错误与预算拒绝继续传播。

初版配对结果（ms，三组交替进程，各两 cycle，GPU wait 开启）：

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 40.603 → 40.057 | 16.750 → 16.390 | 40.991 → 40.154 | −1.3% |
| 滚轮 | 40.772 → 39.664 | 16.711 → 16.259 | 40.916 → 39.871 | −2.7% |
| resize | 74.250 → 72.359 | 17.610 → 17.196 | 76.076 → 74.141 | −2.5% |
| 追加 | 52.992 → 50.375 | 18.779 → 17.129 | 53.225 → 50.758 | −4.9% |
| Stable | 13.028 → 12.504 | 6.213 → 6.037 | 14.176 → 13.947 | −4.0% |

初版 753 对文字摘要及 viewport 位移一致；完整分布在 `step03a/comparison/`。
Perfetto 中 `flow-build` scope 仍包含入口验证，不等同于生产次数。
每次拖拽的两次入口都命中 `flow-measurement-reuse`，段落布局查询从 80 次降为零；
resize 的 1036 次入口中 556 次复用尺寸，段落查询从 41440 降为 19200。
资格读取次数没有下降；省略的是尺寸生产，不是私有输入检查。
trace 完整性通过，分析 session 已关闭。

随后补齐直接 service 调用的完整 requested constraints 字段，并增加相同引擎下
私有 generation 变化／首次证据不被覆盖、不同引擎身份及 height constraint 反例。
修订版 60 项定向 lib 测试、8 项消费者 integration 测试与 Core Clippy 通过。
修订版重新冻结 binary 并完成独立配对复验，数据在 `step03a-rev2/`；
未将初版与修订版样本混合。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 39.387 → 39.016 | 16.106 → 15.926 | 39.586 → 39.176 | −0.9% |
| 滚轮 | 39.249 → 38.524 | 16.055 → 15.933 | 39.495 → 38.726 | −1.8% |
| resize | 71.942 → 71.134 | 16.891 → 16.865 | 73.595 → 72.812 | −1.1% |
| 追加 | 49.895 → 49.864 | 16.378 → 16.176 | 50.274 → 50.520 | −0.1% |
| Stable | 12.262 → 12.193 | 5.891 → 5.969 | 12.557 → 13.166 | −0.6% |

拖拽／滚轮／resize 的 active P95 分别为
42.485 → 41.797、42.455 → 41.896、77.243 → 74.999 ms；
P99 分别为 44.511 → 42.816、43.793 → 42.777、79.773 → 77.011 ms。
753 对文字摘要及位移仍一致，完整原始分布及 binary hash 保存在收据。
修订版 trace 再次通过完整性检查，测量复用和段落查询计数与初版一致。
其中帧耗时有波动，仅用于阶段归因，不用于替代无 tracing 的主对照。
本项收益有限，Stable 只有 9 个样本且 latency 中位数上升；
尚不能宣布整条管线或全部交互完成净性能验收。

## 步骤 4A：取消字体资源重复留存

Graphics 录制在不可变资源表中借用 ready payload，逐 run 仍执行字体身份、
revision 和已有 lease 冲突检查；只有成功录制新增的唯一资源需要克隆。
旧 lease 不替代当前资源表查询，跨作用域字体更新仍会拒绝冲突。
这是留存副本删减，不是原生裁剪或资源查询次数删减。
Flow 的帧录制没有资源表，后续统一验证负责检查；本项只作用于带资源表的录制。

与步骤 3A 修订版的独立配对结果（ms）：

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 40.234 → 39.796 | 16.740 → 16.476 | 40.499 → 40.060 | −1.1% |
| 滚轮 | 40.197 → 39.924 | 16.774 → 16.622 | 40.466 → 40.191 | −0.7% |
| resize | 73.195 → 73.477 | 17.490 → 17.324 | 74.903 → 75.371 | +0.4% |
| 追加 | 49.317 → 50.902 | 16.412 → 17.620 | 49.864 → 51.281 | +3.2% |
| Stable | 12.166 → 12.649 | 5.895 → 6.156 | 12.679 → 14.901 | +4.0% |

753 对文字摘要及 viewport 位移一致，逐操作 glyph／command 数量相同，
完整分布和收据在 `step04a/comparison/`。本项未取得各操作一致的净收益，
不能将拖拽的小幅下降外推为整条管线提速。
12 项 Graphics 契约测试与 Core Clippy 通过，覆盖版本冲突、过期／外部布局，
以及重复 fill／stroke 后销毁字体服务仍保持单一有效字体 lease。

## 步骤 4B：取消帧末尾校验的临时资源副本

`NdCanvas::validate_recording` 现在借用字体 payload，不再为每条原生字形命令
创建随后即释放的 ResourceUpdate。仍按原序检查全部字体依赖、命令、
资源状态、字体身份／revision 与栈平衡。普通失败不变，最终提交资源同步独立执行。
本项不缓存查询结果，不跳过后续 run 校验。

与步骤 4A 的首轮配对结果（ms）：

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 39.447 → 39.525 | 16.140 → 16.300 | 39.614 → 39.812 | +0.2% |
| 滚轮 | 38.924 → 39.559 | 16.053 → 16.234 | 39.068 → 39.821 | +1.6% |
| resize | 71.683 → 72.708 | 16.917 → 17.172 | 73.530 → 74.506 | +1.4% |
| 追加 | 49.405 → 51.993 | 16.434 → 17.429 | 49.696 → 52.081 | +5.2% |
| Stable | 12.270 → 12.531 | 5.893 → 6.169 | 13.006 → 14.014 | +2.1% |

首轮 753 对摘要与位移一致，数据在 `step04b/comparison/`。
4 项长文本绘制、12 项 Graphics 契约测试、Core Clippy 及新增 lib 反例通过。
新增反例证明前两个合法同字体 run 不能使后续错误 revision 被跳过，
资源转为非 ready 后仍返回原始资源错误。

单独的 before／after Perfetto trace 各执行一个 cycle，未开启 GPU wait；
均通过完整性检查并关闭分析 session。`ir-validate` 拖拽平均
0.211397 → 0.206635 ms，滚轮 0.209859 → 0.205198 ms，
resize 0.215221 → 0.211704 ms。局部阶段只减少约 0.004～0.005 ms，
无法支持整体提速结论。此 trace 的 resize `backend-vello-render` 平均
7.918884 → 8.498090 ms；该阶段包含主机提交调用，不能解释为 GPU 硬件时间。
这只是另一次诊断样本，不用于替换或扣除主对照的回退。
完整 SQL、阶段导出和解析收据在 `step04b/trace/{before,after}/analysis/`，
查询入口为 `tools/scripts/textflow_scroll_perfetto.sql`。

同配置独立复验保存在 `step04b/comparison-repeat/`，没有与首轮合并。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 41.163 → 41.679 | 16.918 → 17.077 | 41.382 → 41.961 | +1.3% |
| 滚轮 | 41.450 → 41.801 | 17.039 → 16.942 | 41.671 → 41.974 | +0.8% |
| resize | 73.258 → 75.023 | 17.422 → 17.651 | 75.150 → 76.918 | +2.4% |
| 追加 | 52.365 → 51.273 | 17.563 → 17.198 | 52.455 → 51.347 | −2.1% |
| Stable | 13.036 → 12.939 | 6.296 → 6.200 | 13.404 → 13.446 | −0.7% |

复验 753 对输出摘要与位移一致。追加回退没有复现，拖拽与 resize 的整体上升仍在；
本项局部副本删减通过正确性验证，净性能验收未通过。
没有用局部阶段下降扣除整体回退，也没有将这项改动宣布为已完成的性能改善。

## 累计对照：初始基线至步骤 4B

新一轮独立对照使用冻结的初始 baseline 和步骤 4B binary，未将逐步结果相加。
三组交替进程各两 cycle，GPU wait 开启、tracing 关闭，753 对文字摘要与位移一致。
原始数据在 `step04b/cumulative/`。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 39.842 → 39.208 | 15.781 → 16.258 | 40.024 → 39.452 | −1.6% |
| 滚轮 | 39.694 → 39.016 | 15.789 → 16.167 | 39.849 → 39.286 | −1.7% |
| resize | 77.111 → 74.308 | 16.925 → 17.823 | 79.015 → 76.159 | −3.6% |
| 追加 | 52.947 → 49.547 | 17.357 → 16.905 | 53.550 → 49.954 | −6.4% |
| Stable | 12.022 → 12.409 | 6.015 → 6.109 | 13.391 → 14.112 | +3.2% |

尾部未取得一致收益。拖拽 active P95／P99 为 43.354／44.407 → 46.344／49.261 ms，
滚轮为 43.664／46.056 → 52.573／59.359 ms；
resize 为 89.499／102.523 → 85.972／95.501 ms。
`after-0` 的拖拽和滚轮帧耗时明显高于另外两个 after 进程，
滚轮最大 active 为 135.277 ms，frame 为 108.402 ms；原因尚未由运行时证据确定。
保留该进程全部样本，不以异常值名义删除。总体中位数下降不能关闭尾延迟验收，
也不能据此裁定步骤 4A／4B 各自已经获得净性能收益。

## 步骤 2B：取消首个贡献类型的独立数组分配

声明的首个类型标识改存于固定记录，后续标识继续使用有序数组。
全部 contributor／handler 回调、完整类型序列比较和输入重验继续执行。
重复类型不会去重，外部扩展与内置贡献者使用同一存储。
固定记录增长由既有 metadata 账本计费；额外数组按实际 capacity 收费，
声明回调中的保守预算预留保持。该项不减少资格检查次数。
11 项定向 Flow reuse lib 测试通过，新增反例覆盖多类型重复顺序、
追加后旧声明不变及首个标识／后续数组的留存计费。
8 项独立消费者测试、Core Clippy 与 release 构建通过。冻结的源码、binary 和构建收据
保存在 `step02b/`；与步骤 4B 的独立配对数据在 `step02b/comparison/`。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 38.864 → 39.645 | 15.985 → 16.447 | 39.144 → 39.725 | +2.0% |
| 滚轮 | 38.785 → 39.745 | 15.940 → 16.466 | 38.919 → 39.926 | +2.5% |
| resize | 72.596 → 72.493 | 16.892 → 17.208 | 74.545 → 74.346 | −0.1% |
| 追加 | 50.281 → 49.782 | 16.498 → 16.726 | 50.655 → 50.207 | −1.0% |
| Stable | 12.346 → 12.170 | 6.013 → 5.919 | 14.973 → 13.079 | −1.4% |

该项仍未通过净性能验收；声明的临时数组减少不能代替拖拽和滚轮的整体收益。
753 对文字摘要、位移和 glyph／command 数量一致。拖拽 active P95／P99 为
51.487／62.155 → 45.889／52.247 ms，滚轮为
52.332／69.037 → 51.620／55.294 ms，resize 为
88.831／100.135 → 79.932／87.350 ms。尾部分布改善与中位数回退同时保留，
不择取其中一项宣称通过。

单独保存了粗阶段与细阶段 Perfetto trace，两种配置各自配对且均未开启 GPU wait。
粗阶段不记录 Core 资格 scope，只能用于宿主诊断；细阶段在 `step02b/trace-fine/`。
细阶段每次拖拽／滚轮仍各有 29 次资格读取，resize 平均 84.7 次。
`flow-qualification` 每次操作平均 inclusive 时间，拖拽
9.014490 → 8.906447 ms，滚轮 9.036738 → 8.795343 ms，
resize 24.770296 → 24.285264 ms。
这份单 cycle 诊断样本仅支持局部阶段下降，不能扣除主对照的整体回退。
粗、细 trace 都通过完整性检查，命名分析 session 全部关闭；
SQL 及导出保存在各 `analysis/` 目录。
workspace fmt 和文档门禁通过。该项已因拖拽／滚轮净成本回退而退役，
当前声明恢复完整有序数组；顺序、旧声明不变与数组收费反例继续保留。
原始源码、binary、收据和 trace 不改写。

## 步骤 2C：取消不可变 metadata 的重复计费遍历

本项以步骤 4B 为前版，步骤 2B 的失败试验不混入后版。
不可变 participants 构造时汇总留存费用，后续接收同一 backing 时直接读取摘要；
省去为每次 snapshot 接收再次遍历 participants、handlers 和输入快照。
首次构造和 metadata 改变时仍完整计算。摘要字段、Vec 固定记录和 Arc counters
进入收费；逻辑输入相等只比较内容，不比较 allocation capacity 或收费摘要。
全部 contributor／handler 声明、generation 与输入快照的检查保持。

定向回归覆盖旧证明不被后读值覆盖、metadata 更新、弱引用释放及预算拒绝。
新增检查核对嵌套留存总数，并证明不同 capacity 只改变收费，不改变输入身份。
12 项定向 Flow reuse lib、8 项消费者测试、Core Clippy 和最终 release 构建通过。
三组交替进程、各两 cycle 的独立对照保存于 `step02c/comparison/`。
753 对文字摘要、viewport 位移与 glyph／command 数量一致。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 42.307 → 42.052 | 17.562 → 17.572 | 42.630 → 42.292 | −0.6% |
| 滚轮 | 42.160 → 41.863 | 17.498 → 17.514 | 42.499 → 42.051 | −0.7% |
| resize | 77.019 → 77.984 | 18.142 → 18.341 | 79.071 → 80.112 | +1.3% |
| 追加 | 54.148 → 52.788 | 17.920 → 17.841 | 54.608 → 52.921 | −2.5% |
| Stable | 13.349 → 13.486 | 6.493 → 6.822 | 13.489 → 13.579 | +1.0% |

拖拽 active P95／P99 为 46.942／48.749 → 46.048／47.255 ms，
滚轮为 46.890／48.165 → 45.991／48.018 ms，
resize 为 83.945／90.007 → 83.159／85.980 ms。
尾部下降和 resize 中位数回退同时保留，不能据此宣称净成本验收通过。
追加只有 6 个样本，Stable 只有 9 个样本。

同配置细阶段 trace 的有效对照为 `trace-fine/before/analysis-complete/`
和 `trace-fine/after-matched/analysis/`。两份均通过完整性检查、各有 125 个操作，
glyph／command 数量一致，分析 session 已关闭。未开启 GPU wait。
每次拖拽／滚轮仍有 29 次资格读取，resize 平均 84.7 次；
资格 inclusive 平均为拖拽 9.485415 → 9.661686 ms、
滚轮 9.315947 → 9.497428 ms、resize 25.565793 → 25.775817 ms。
本份诊断没有支持资格阶段下降，更不能代替无 tracing 的主对照。

首次分析选中了退出时额外导出的短 trace，因缺少完整 input record 被拒绝，
失败结果保留在 `before/analysis/`；随后解析同一进程导出的完整 trace。
`after/` 未启用独立 timings，仅作为补充诊断保留，不与前版作匹配比较。
有效 after 已按前版完整参数重新采样到 `after-matched/`，不合并两次数据。

本项已退役，当前恢复步骤 4B 的资格 metadata 表示和计费路径。
原始源码、binary 与全部样本保留，完整数组顺序和收费回归继续保留。
退役后 11 项 Flow reuse lib、Core Clippy、workspace fmt 与文档门禁通过。

## 步骤 2D：取消 Figure 借用时的重复 route 祖先查询

本项以退役资源副本试验后的 binary 为前版，前述失败试验不混入后版。
Preparation 在同一不可变 tree/style 借用内，随已有 subtree membership 一起汇总
Connection capability 的结构归属。root 外祖先的连接归属也进入摘要。
后续资格 reader 借用 Figure／capability 时查询成员摘要，不再逐次爬祖先。
仍逐次读取 contributor／handler 声明，仍在连接子树记录原节点的
FigureGeometry observation；不缓存私有输入、几何值或 provider revision。
摘要随借用结束失效，新借用重新构造；新增临时记录与 route 成员计费。

23 项 Preparation lib 回归、8 项独立消费者 integration、Core Clippy 与 release
构建通过。新增外部 Connection Figure 反例覆盖普通 sibling、子树外的连接祖先、
独立 input scope、原节点 geometry observation、预算失败与重挂载后重新判断。
源码、binary 及构建收据冻结在 `step02d/`。

三组交替进程、各两 cycle，GPU wait 开启、tracing 关闭；
753 对文字摘要与位移一致，逐操作 glyph／command 数量不变。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 41.639 → 36.904 | 17.346 → 15.966 | 41.823 → 37.188 | −11.4% |
| 滚轮 | 41.778 → 37.259 | 17.307 → 15.881 | 41.973 → 37.397 | −10.8% |
| resize | 76.503 → 63.994 | 18.227 → 16.916 | 78.502 → 65.995 | −16.4% |
| 追加 | 52.394 → 44.882 | 17.978 → 16.340 | 52.545 → 45.254 | −14.3% |
| Stable | 13.574 → 10.497 | 6.442 → 5.130 | 14.111 → 11.041 | −22.7% |

拖拽 active P95／P99 为 45.061／48.557 → 40.993／44.741 ms，
滚轮为 44.824／46.519 → 41.588／43.507 ms，
resize 为 84.625／89.038 → 71.675／80.273 ms。
整体 active 分布有明确下降，frame 也下降；仍不能称为 GPU 硬件时间减少。
滚轮 latency P99 为 47.329 → 54.919 ms，尾延迟未全部改善，
完整原始分布继续保留。追加只有 6 个样本，Stable 只有 9 个样本。
本项待补累计对照，不能关闭整个方案或实际屏幕呈现验收。

匹配细阶段 trace 在 `step02d/trace-fine/`，两份各有 125 个操作，
完整性检查通过，命名分析 session 均已关闭。tracing 开启，未开启 GPU wait。
每次拖拽／滚轮仍有 29 次资格读取，resize 平均 84.7 次；
每次操作的 `flow-qualification` inclusive 平均时间，拖拽
9.697101 → 5.437679 ms，滚轮 9.575720 → 5.467887 ms，
resize 25.328452 → 14.665385 ms。校验调用次数也不变，拖拽／滚轮
各 27 次，resize 平均 71.75 次。该证据支持相同检查次数下重复结构查询变少，
没有省略 reader 或最终检查。嵌套 scope 的 inclusive 时间不能相加；
本份 trace 只用于归因，不能从前述主计时中扣除诊断时间。
查询使用 `tools/scripts/textflow_scroll_perfetto.sql`，逐条 SQL 导出在对应
`analysis/` 中，阶段汇总在 `stage-comparison.json`。

## 后续步骤

资源副本步骤 4A／4B 已退役，恢复原资源快照与字体检查构造，
取消仅服务于试验的 `FontInstanceRef::from_resource_parts` 内部入口。
后续错误 revision／非 ready 资源反例及重复绘制后唯一 lease 反例继续保留。
退役后 1 项定向 lib 与 12 项 Graphics integration 测试通过。
release binary 和本次源码冻结在 `resource-retirement/`。
退役前后重新执行三组交替进程、各两 cycle，GPU wait 开启、tracing 关闭；
753 对文字摘要和位移一致，没有使用历史步骤 3A 的样本代替当前复验。

| 操作 | active P50 前 → 后 | frame P50 前 → 后 | latency P50 前 → 后 | active 变化 |
|---|---|---|---|---|
| 拖拽 | 41.201 → 41.080 | 17.066 → 16.940 | 41.500 → 41.408 | −0.3% |
| 滚轮 | 40.910 → 40.804 | 16.958 → 16.860 | 41.133 → 41.055 | −0.3% |
| resize | 74.957 → 75.480 | 17.922 → 17.840 | 76.927 → 77.282 | +0.7% |
| 追加 | 50.709 → 51.714 | 17.204 → 17.277 | 50.857 → 51.816 | +2.0% |
| Stable | 12.811 → 12.862 | 6.263 → 6.373 | 13.379 → 13.449 | +0.4% |

拖拽 active P95／P99 为 44.155／48.215 → 44.511／46.817 ms；
resize 为 80.305／87.190 → 83.126／86.821 ms。退役没有带来明确整体提升，
也没有把先前副本删减变成有效性能优化。两项继续退役，保留完整分布和原试验，
后续以有效删减路径继续推进；本表不关闭整个方案的净性能或尾延迟验收。

验证 reader 内部的 observation 还服务于嵌套测量缓存与依赖检查；
本批没有省略这些记录，需先证明消费边界才能继续删减。
纯文字测量复用已完成修订版逐项对照。原生字形裁剪和重复路径退役尚未完成，
不因尺寸复用或局部提速关闭总体方案。
资源副本两项未取得一致净收益，退役后的累计对照仍需完成。
原生覆盖已核对实际锁定的 Vello 0.10.0 Euler lowerer，
现有 0.25 像素容差没有覆盖强制细分终止与顶点近似，
仍缺少可复核的有限域证明；细节见
[原生覆盖专题](../../design/architecture/long-text-native-coverage-boundary.md#锁定-lowerer-核验)。
本批未添加未经证明的原生能力声明。

空 glyf 记录的受限分支已有首个测试 oracle：
`cargo test -p novadraw-backend-vello --lib empty_native_glyf_domain_preserves_gpu_pixels_without_fragmenting_runs`
通过。测试 helper 与生产分离，先排除颜色／bitmap 和非 Glyf 的实际分流，
仅接受 `loca.get_glyf` 成功返回空记录；缺失 glyph 不接受。
Inter 的混合 run 删除空记录后保持原位置和单 run，36 对 GPU readback 逐字节一致。
同一空记录在四种实际 size 和三个变量坐标下也没有路径操作。
本项尚未接入生产，没有渲染时间优化结论；资源／session 绑定、全空 run、
其他字体格式、公开外部消费者及计费仍未验完。具体边界见原生覆盖专题。

后续迁移到 Parley 0.12.0 时，这个 GPU 对照出现逐字节差异，
随后原顺序连续三轮复验通过；波动尚未解释，旧基线通过不代表新版本稳定通过。
最新范围与失败记录见[上游迁移验证](parley-upstream-migration-2026-11-05.md#gpu-对照波动)。
