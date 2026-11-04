# TextFlowLarge 滚动卡顿诊断

类型：`verification`

能力归属：Core 的 viewport、Layout／Flow、准备与 IR；Vello backend 的编码和提交；
Native 样例宿主的输入与帧调度。Editor 不在范围。

日期：2026-11-04。用户授权增补 SDK 打点、采集 Native trace、定位滚动瓶颈并提出方案。
本轮完成观测与诊断，未实施生产性能整改。

## 结论

滚动慢路径集中在 **重复 Flow 测量组装，以及仍处理整页文字的 Vello 调用**。
本次 release 回放中，滚轮输入到提交完成的活跃工作中位耗时约 31 ms；
自然 damage 下仍约 31 ms。文字引擎缓存已命中，继续优先优化 shaping 或换行，
无法覆盖这条滚动慢路径。

宿主强制 full redraw 确实放大了无变化输入的成本，但它不是滚动卡顿的唯一原因：
自然 damage 将固定指针事件的中位活跃耗时从 17.02 ms 降到 4.43 ms，
滚动帧却仍记录相同的 7609 条命令和 51232 个 glyph。

所有数字都是**开启 SDK 的墙钟区间**。约 31 ms 已超过一个 60 Hz 的 16.67 ms
工作预算，支持继续整改；本轮没有测量关闭观测的成本，不能把它直接换算成实际 FPS、
CPU 利用率或 OS 输入到屏幕的延迟。细粒度 Flow 埋点本身的开销也待独立量化。

## 采集与有效性

采用[采集计划](../plans/textflow-scroll-perfetto-2026-11-04.md)、
官方 Perfetto SDK 和官方 skill 随附的 Trace Processor v59.1。
真实 macOS 窗口由 LaunchServices 打开，后端为 Apple M1 Pro／Metal。
Rust 1.96.0，release；窗口为 600 × 500 逻辑像素，1200 × 1000 物理像素。
没有启用 GPU queue wait、截图、完整请求或逐 run glyph category。

有效 trace：

`target/verification/textflow-scroll-20261104/validated/textflow-1793775239637933000-46083-0.pftrace`

SHA-256：
`3217c6d7556d20ea4497e0d0cee430046e7384179c3beab277a8e7ef56b05dbc`

SDK 配置为 256 MiB session buffer、32 MiB producer 共享内存、每秒刷新增量状态。
同一 Runtime、同一窗口先 full、后自然 damage，每组一轮；
没有更改文字、窗口尺寸、默认预算或 Vello MSAA16。
先重建和预热，再按正负 40 逻辑像素配对执行直接 viewport 滚动和固定滚轮输入，
穿插固定位置的指针事件与 Stable 控制。每个操作等待帧完成后才推进。
固定滚轮通过原 Winit adapter 和 Core dispatch，不是系统硬件输入。

有效 trace 无 error／data_loss 统计，无未结束区间，无负自身耗时。
148 个 operation、150 次 frame attempt；其中 123 次 Presented、25 次 no_damage、
两次初始 Reset 的 Skipped。全部 96 个滚动样本实际位移为 ±40，没有边界钳制或零位移。
报告排除 Reset、显式预热 Stable 和模式切换，剩余 144 个比较样本。
正文 51232 bytes、40 段、561 行、宽度 586 和已发布 Flow revision 始终不变。

第一份两轮采集曾发生共享内存满，不能用于完整归因，保留供检查：

`target/verification/textflow-scroll-20261104/paired/textflow-1793775111610849000-44245-0.pftrace`

SHA-256：
`6f3a6e928d08362bbac317086a646d9fb3a02a18b771b30a9e061f4b6b8245e9`

其 `traced_buf_data_loss_smb_full=1`、sequence loss=6、writer packet loss=5，
并有 436359 个 packet 因增量状态失效被跳过。增大 session buffer 无法单独解决
producer 突发写入；本轮同时调整共享内存，并缩短回放，补采后通过健康检查。

## 滚动与控制组

“活跃耗时”为输入根区间（含其 operation）加对应 host-frame；
直接操作没有输入根区间时使用 operation 加 host-frame。
不重复累加嵌套的输入／operation，也不包含两者之间的等待。
P95 使用 nearest-rank，只描述这批有限样本；不作显著性推断。

| 操作 | damage | 样本 | 活跃中位 ms | P95 ms | host-frame 中位 ms |
|---|---|---:|---:|---:|---:|
| 直接 viewport 滚动 | full | 24 | 37.10 | 66.82 | 16.88 |
| 固定滚轮输入 | full | 24 | 30.75 | 33.17 | 13.64 |
| 直接 viewport 滚动 | natural | 24 | 31.17 | 34.02 | 14.04 |
| 固定滚轮输入 | natural | 24 | 31.07 | 45.39 | 13.95 |
| 固定指针事件 | full | 12 | 17.02 | 20.68 | 14.26 |
| 固定指针事件 | natural | 12 | 4.43 | 5.19 | 2.14 |
| Stable | full | 12 | 16.39 | 20.78 | 13.99 |
| Stable | natural | 12 | 4.19 | 4.52 | 2.12 |

natural 的指针／Stable 全为 no_damage，不是成功呈现的新帧。
滚动 natural 全为 Partial，full 全为 Full，但命令和 glyph 数相同。
full 固定滚轮的重绘等候间隙均值 0.18 ms，输入 adapter／dispatch 的额外区间均值
0.25 ms；这两项不是本批约 31 ms 活跃耗时的主要来源。

full 直接滚动的第一帧活跃耗时 106.49 ms，其中 Vello 调用 64.36 ms，
后续也存在较慢 Vello 区间。full 先采、natural 后采，后端缓存、驱动或系统状态
可能影响这些长尾；不能把直接滚动两组均值差全部解释为 damage 优化。
固定滚轮在两组均约 31 ms，是更稳定的对照。

## 成本落在哪里

以下以 full 固定滚轮的 24 个有效样本为分母，平均活跃耗时 **30.98 ms**。
自身耗时扣除直接子区间；每个阶段只归入一组，递归的 Flow 阶段不重复求和。

| 互不重叠的成本组 | 每次平均 ms | 占活跃耗时 |
|---|---:|---:|
| Flow 全部阶段自身耗时 | 12.37 | 39.9% |
| Vello render_to_texture 调用 | 8.73 | 28.2% |
| preparation 签名和批量校验 | 2.93 | 9.5% |
| Layout 除已标出的子阶段外 | 2.00 | 6.5% |
| backend 其余已标阶段 | 1.37 | 4.4% |
| IR／submission 构造 | 1.30 | 4.2% |
| 其他已标工作及输入额外区间 | 2.28 | 7.4% |

宿主的 `stabilize_for_query` 平均 16.75 ms，帧准备约 3.55 ms。
这两个包络包含上表的一部分，不能再与上表相加。

每个滚动操作有两次 `flow-build`，直接父区间均为 `layout`；
没有 `flow-engine-layout` 或 shaping 调用。每组 24 次滚动共 1920 次段落查找，
即每次 80 次，平均仅约 0.057 ms。已发布快照的累计 build 指标保持不变，
因此本轮使用实际 SDK slice 数，不用该指标代表临时测量构造次数。

[稳定查询](../../../crates/novadraw/src/runtime/runtime/frame_submission.rs)会收敛派生状态；
[样例 apply](../../../crates/novadraw/examples/native/textflow-large-app/src/research.rs)
对所有操作调用它，[输入入口](../../../crates/novadraw/examples/native/textflow-large-app/src/native.rs)
在 dispatch 后又执行该操作路径。随后 prepare_submission 再经过稳定化。
已发布 Flow 未变，并不阻止 Layout 重新走测量贡献、遍历来源和组装临时结果。
**减少重复测量与组装应优先于进一步提高段落缓存命中率。**

backend 中，Scene encode 平均 0.98 ms，校验 0.20 ms，AA prepare 0.009 ms，
surface acquire 0.024 ms，present 调用 0.014 ms。
这些证据不支持把本批慢帧主要归因于 drawable 获取或 AA-OFF CPU mask。
Vello render_to_texture 的 8.73 ms 包含 CPU resolve／packing、命令构建、
资源工作及 queue 操作；没有 GPU timestamp，不能称为 GPU 执行时间。

## 为什么 Partial 仍处理整页文字

[Vello capabilities 与 encoder](../../../crates/novadraw-backend-vello/src/lib.rs)
支持 Partial，但每次 submit 仍重新建立 Scene、遍历提交命令，再调用 Vello。
Partial 会限制 damage 修复范围，并不自动省去这些 CPU 工作。

Core 已有[Flow 段落索引](../../../crates/novadraw/src/figure/flow/snapshot.rs)
和[文字裁剪查询](../../../crates/novadraw/src/render/context.rs)；
`covered_text_clip` 要求有效的 `coverage_outset`，它由 backend 的
`filled_path_coverage_outset_pixels` 契约提供。当前 Vello 未声明这个保证，
因此不能直接打开该安全裁剪路径。没有预处理 outlines 时，原生 glyph 路径
还会遍历所有 glyph runs 生成命令。

这与本轮两种 damage 都记录 51232 glyph 的事实一致：
屏幕 clip 可以限制最终可见像素，仍不能避免整页命令、glyph 数据和 resolve 的处理。
已有文本索引的安全裁剪资格与 native glyph 分支仍需打通。

## 可能的整改方案

| 优先级／owner | 方案 | 为什么做／验收重点 |
|---|---|---|
| 高：Core Layout／Flow | 按完整测量输入、source prefix、provider／字体版本与观测依赖复用贡献和测量结果；先查滚动为何使 Layout 再测两次 | 本批没有 shaping，却花 12.37 ms 组装。核验滚动前后输入等价性，不能仅凭宽度和正文相同跳过 |
| 高：Core Graphics + backend | 明确 Vello coverage 保证与 native glyph 的安全保守边界，让现有段落／行／glyph 索引按 viewport clip 缩小提交 | 当前每帧 51232 glyph。必须验证 AA、变换、skew、文字描边和 fallback，不能猜一个 outset 数字启用裁剪 |
| 高：Native 宿主 | dispatch 后按真实派生工作调度帧；只在需要查询稳定几何时提前 stabilize；默认遵循引擎 damage | 控制组减少约 12.5 ms 重绘；需保留焦点、滚动、动画、tooltip／AT 的更新与重试语义 |
| 次：Core Preparation | 按受检版本复用 source／observation 校验，避免无变化边界反复遍历 | natural Stable 无渲染仍约 4.19 ms。保持一致性、故障隔离和预算契约 |
| 次：Vello backend | 在提交缩小后评估 Scene／resolve 复用，另采 GPU timestamp 或驱动证据 | 主 Vello 区间约 8.73 ms；不能把它全归因于 glyph outline，也不先删除 MSAA 或校验降低数字 |

建议先独立设计 Core 的测量复用与安全可见文字提交，宿主调度作为配套；
不要仅关闭 full redraw 后宣称滚动已修复。
整改前后应补充不编译观测、编译但不采集、粗阶段和细阶段的对照，
用相同字体／release／输入验证结果等价及成本变化。
还需人工连续滚轮与真实呈现证据；当前串行固定回放不覆盖事件积压、惯性滚动、
WindowServer 合成或系统到屏幕的完整延迟。

## 复跑与交付

采集命令：

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --toolchain 1.96.0 --clt --target-dir /tmp/novadraw-text-stage-target -- --scroll-replay --cycles 1 --trace --exit-after-replay --output target/verification/textflow-scroll-20261104/validated
```

原始 SQL：
[textflow_scroll_perfetto.sql](../../../tools/scripts/textflow_scroll_perfetto.sql)。
分析入口：
[analyze_textflow_scroll.py](../../../tools/scripts/analyze_textflow_scroll.py)。
分析器先发现 schema、检查健康，再导出记录／根区间／阶段；JSON 参数使用 hex 导出，
避免 Trace Processor CSV 内部引号未转义。每次分析在一个 warm session 中完成，
结束后关闭会话。

```sh
python3 tools/scripts/analyze_textflow_scroll.py \
  --processor /Users/bytedance/.trae-cn/skills/perfetto/bin/trace_processor \
  --session textflow_scroll_validated \
  --trace target/verification/textflow-scroll-20261104/validated/textflow-1793775239637933000-46083-0.pftrace \
  --output target/verification/textflow-scroll-20261104/analysis
```

CSV、解析后的记录、逐样本及分组统计保存于
`target/verification/textflow-scroll-20261104/analysis/`。
这是本地产物目录，清理 target 前应另存 trace 和导出。
所有 SDK 阶段均同步嵌套，归因 SQL 的核心为：

```sql
WITH children AS (
  SELECT parent_id,sum(dur) AS ns FROM slice WHERE dur>=0 GROUP BY parent_id
)
SELECT s.id,s.name,s.dur-coalesce(c.ns,0) AS self_ns
FROM slice s LEFT JOIN children c ON c.parent_id=s.id;
```

已完成 Native release 构建与采集、应用定向 check／Clippy、脚本语法和文档检查。
SDK optional feature 默认关闭；backend 无 tracing 的定向 Clippy 已检查，
首次发现空 scope 的 Drop lint，修正后通过 `--lib --no-default-features -- -D warnings`。
第三方 HarfRust 原有 unused-qualifications 警告保留，本轮未修改第三方实现。
没有执行 workspace 全量测试，也没有 Git 提交。
