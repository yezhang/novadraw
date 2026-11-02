# 文字优化验证指南

类型：`verification`

日期：2026-11-02。能力归属：Core 文字布局、Flow 缓存与 Render IR；
官方样例提供消费者和性能驱动，窗口呈现由平台／后端另行验证。

当前可以验证完整布局缓存、段落复用、共享 glyph 与来源几何。
ADR-059 A 的阶段协议已完成，B 的只读存储补丁已批准接入，
Parley 阶段适配及 Flow 宽度复用仍未实现。
因此本轮存储补丁不会让拖动窗口时自动省去 shaping。
当前状态见 [ADR-059](../../adr/adr-059-staged-text-width-reuse.md)及
[B 接入记录](../reviews/text-stages-b-storage-patch-2026-11-02.md)。

## 先检查行为

在仓库根目录运行：

```bash
cargo +1.96.0 test -p novadraw \
  --test long_text_layout_contract \
  --test long_text_paint_contract \
  --test shared_glyph_storage_contract
```

这些测试检查精确请求命中与失效、段落和来源映射、glyph 范围及旧引用存活，
也检查有墨迹证书时的轮廓裁剪和未知原生文字的保守绘制。
通过这些测试不等于默认原生 glyph 已实现可见区域裁剪。

阶段协议和存储补丁另有独立入口：

```bash
cargo +1.96.0 test -p novadraw --test p2_text_stages
cargo +1.96.0 test \
  --manifest-path tests/consumers/parley-storage/Cargo.toml --lib
```

前者验证外部阶段引擎、预算拒绝、缓存淘汰及原始错误；
后者验证生产依赖的存储观察与字体 backing 引用。
它们属于契约验证，不测实际 Flow 宽度复用的收益。

有本机 GPU、需要检查整套长文本像素和消费者时，可改用
`cargo +1.96.0 xtask verify text.long-layout`。
该 suite 已包含文字契约、Full／Partial GPU 对照、release 构建和 CPU smoke，
不必紧接着重复其中相同的测试。子命令使用默认 Cargo 工具链；
如需统一 1.96.0，可为该命令设置 `RUSTUP_TOOLCHAIN=1.96.0`。

## 再测当前成本

下面按未设置 `CARGO_TARGET_DIR` 的默认路径运行。
如果指定了独立构建目录，二进制位于该目录的 `release/` 下。

```bash
cargo +1.96.0 build --locked --release \
  -p draw2d-official-examples --bin textflow-perf
target/release/textflow-perf 5 extended
```

驱动输出逐行 JSON，每轮创建独立 Runtime，运行原 TextFlowLargeExample，
向第一文字来源追加固定的 Z，再执行稳定查询、滚动、resize 和 CPU 帧录制。
5 次用于确认入口与分母，不足以判断 P95／P99 或宣布达到目标。
不要使用 debug 构建评估性能。

| 输出字段 | 看什么 |
|---|---|
| `initial_ns` | 首次布局到稳定查询的耗时；构造成本另见 `construct_ns` |
| `append_e2e_ns` | 固定追加的分发与稳定化总耗时 |
| `stable_ns` | 内容不变时重复查询的成本 |
| `paragraph_hits`、`layout_calls` | 缓存是否命中，是否仍发生布局调用 |
| `scroll_ns`、`scroll_layout_calls` | 纯滚动应没有新增布局调用；当前原始 fixture 预期为 0 |
| `resize_ns`、`resize_layout_calls` | 宽度改变后的稳定化成本；当前仍允许完整布局调用 |
| `record_ns`、`commands`、`native_glyphs` | CPU IR 录制成本及输出规模；不是 GPU 帧耗时 |
| `status` 与 source-commit revision | 失败和 source 更新是否符合驱动合同 |

时间单位为 ns，除以 `1_000_000` 得到 ms。
原始 fixture 固定有 40 个段落、51,232 个初始文本字节；
追加和 resize 后的原生输出仍为 51,233 个 glyph。
若通过改变输入、输出规模或预算取得更低耗时，应另记负载，不能沿用原始基线。

## 用重复采样判断波动

构建完成后，可使用现有采样器冻结本轮二进制和源码记录。
以下目录必须是新目录；再次采样时换一个名字，避免混入上次结果。

```bash
RUSTUP_TOOLCHAIN=1.96.0 python3 tools/scripts/textflow_performance.py \
  freeze /tmp/novadraw-text-check-original
RUSTUP_TOOLCHAIN=1.96.0 python3 tools/scripts/textflow_performance.py \
  run /tmp/novadraw-text-check-original --extended \
  --sessions 5 --samples 200 --warmup 20 --cold-per-session 40
```

查看该目录的 `summary.json`：

- `failures` 应为空；超时、预算拒绝、分母和 source receipt 不一致都须保留。
- `groups` 包含首次、追加和稳定查询的 P50／P95／P99。
- `extended` 包含滚动、resize 和 IR 录制的分位数。
- `additional_groups` 保留布局调用、缓存命中和各阶段成本。

采样期间停止自己启动的编译和测试，并尽量保持电源、后台负载与窗口状态一致。
新进程样本包含进程启动、追加和销毁，不能当作首次画面出现时间。
采样器的退出成功也不代替检查 `failures`。

`--workload paragraph`、`line`、`mixed` 分别提供单个长段落、
NoWrap 长行和带真实 inline Figure 的混排。
为每种负载使用独立冻结目录，并在 `freeze` 和 `run` 两处指定同一 workload；
需要滚动和 resize 时保留 `--extended`。

## 前后比较必须匹配

单版本采样只能说明当前成本。要判断优化收益，分别构建优化前和当前版本，
使用同一工具链、release 配置、字体、驱动、负载、诊断和准备预算，
在两个独立 Cargo target 中冻结二进制，再交替运行：

```bash
RUSTUP_TOOLCHAIN=1.96.0 python3 tools/scripts/textflow_performance.py \
  compare /tmp/novadraw-text-comparison \
  --baseline /path/to/frozen-baseline \
  --optimized /path/to/frozen-current --extended
```

这里两个输入路径是已经冻结并匹配的目录，输出路径必须尚不存在。
`--optimized` 对照会核对两份配置、锁文件、字体和驱动记录；
不同记录会被拒绝。若要只比较 Core 算法改动，两版应采用相同的依赖补丁配置。
不能直接把旧报告中的数值或配置不同的旧二进制与当前运行计算提速比例。
初始化、命中、resize 和失败路径分开判断，不能用缓存查找更快代替整体更快。

## 本轮实际采样

2026-11-02 在 Apple M1 Pro、macOS 14.7.8、Rust 1.96.0 上完成
5 个会话的原始负载采样。共 1,000 个同进程新建 Runtime 样本、
200 个冷进程样本，另有 100 个预热样本不进入分位数；失败记录为空。
这里的“冷进程”不表示已经清空系统或文件缓存。

下表只使用 1,000 个同进程新建 Runtime 样本，单位为 ms。
每个样本仍从首次构造开始，不能把这组数据理解为同一个 Runtime 的持续热缓存命中。

| 操作 | P50 | P95 | P99 |
|---|---:|---:|---:|
| 首次布局／稳定化 | 35.14 | 40.70 | 49.90 |
| 固定追加端到端 | 15.05 | 17.37 | 22.49 |
| 内容不变的稳定查询 | 2.03 | 2.53 | 3.03 |
| 纯滚动／稳定化 | 5.82 | 6.94 | 8.56 |
| resize／稳定化 | 32.42 | 36.72 | 52.16 |
| CPU IR 录制 | 3.72 | 4.45 | 5.28 |

全部 1,200 个有效样本的初始分母一致：
40 段、51,232 字节、561 行、1,022 runs、4,168 fragments、有效宽度 586。
全部扩展样本的纯滚动新增布局调用为 0，resize 调用为 6，
录制输出为 7,889 个命令、51,233 个原生 glyph。
冷进程和各会话分位数另保存在原始汇总中，没有混入上表。

本轮使用电池供电；会话开始时的 1 分钟 load average 从约 26.29 降到 9.01，
外部后台负载未受控。数据用于复现当前本机成本，不作为稳定发布基线。
该环境下，追加和 resize 的 P95 高于 16 ms，IR 录制的 P95 高于 4 ms；
稳定查询的 P95 低于 4 ms。仍须在匹配环境下复测，不能据此关闭整体验收。
本轮只有一个版本，也未启用 ADR-059 宽度复用，不报告提速比例。

证据包括[分位数汇总](../evidence/text-stages-b-2026-11-02/production-perf-summary.json)、
[全部原始样本](../evidence/text-stages-b-2026-11-02/production-perf-samples.ndjson.gz)、
[分母和文件散列核对](../evidence/text-stages-b-2026-11-02/production-perf-receipt.json)及
[构建／负载记录](../evidence/text-stages-b-2026-11-02/production-perf-manifest.json)。
冻结二进制、源码 patch 和未跟踪源码归档保留在本地
`/tmp/novadraw-text-validation-20261102-original/`，未作为长期源码归档提交。

## 最后看真实窗口

macOS 可运行 `python3 tools/scripts/run_draw2d_macos.py`，
选择 TextFlowLargeExample、TextFlowExample、TextSurroundingFigureExample 和 BidiBlockExample。
检查连续缩窄／放宽窗口、滚动、光标及选择、混排换行和 inline 位置，
注意是否出现旧字形残留、位置漂移或预算拒绝。
该启动脚本使用 debug 构建，适合功能复核；窗口性能仍需 release 宿主单独采证。

性能驱动不创建窗口或 GPU，输出 `present: unavailable` 是预期行为。
它不能证明输入到屏幕的延迟、compositor 性能或 WebGPU 性能。
16 ms 更新与 4 ms 查询／IR 的整体验收仍未关闭；
ADR-059 C／D 完成前，也不能用协议测试或约 2 ms 的隔离 reflow
宣称真实 Flow resize 达标。
