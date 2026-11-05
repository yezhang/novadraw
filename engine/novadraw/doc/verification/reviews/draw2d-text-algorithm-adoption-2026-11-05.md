# Draw2D 文本算法采用验证

类型：`verification`

能力归属：Core Text／Figure。无 Editor、backend 或 platform 行为改动。

日期：2026-11-05。状态：局部实现、定向回归、成本对照、Core Clippy、
workspace check 和文档门禁已通过。
用户授权和采用范围见[实施方案](../../design/architecture/draw2d-text-algorithm-adoption.md)。
本切片属于既有 FR-02，不关闭长文本整体性能或平台门禁。

## 实际改动

截断搜索共用一个候选字符串缓冲区，
保存最佳已测布局，结束后不再排版相同的获选前缀。
基础 TextFlow 行数截断共用这两项，只有未获选任何非空前缀时才测量 ellipsis。

默认 Parley 引擎缓存最近 descriptor 的成功 FontMetrics。注册、替换、移除字体
时清空；失败不安装新条目，非法替换保留旧资源；精确 run 的指标查询仍校验资源版本。

未复制 SWT 全局资源或第二套换行器。现有测量缓存和 clip 索引继续使用；
Flow 多宽度缓存保留声明及证明条件，已发布快照的片段不原地修改。

首轮平均 advance 预测未保留。真实 Arabic 混排 `"مرحبا world"` 的前缀
宽度并不单调；首轮独立枚举测试在宽度 `35.61909484863281` 下发现，
搜索获选布局的 visible end 为 6，全局枚举获选为 11。
这证明改变探测顺序时不能假定所有上下文前缀都按字符数增宽。
最终代码保持旧二分探测顺序，用旧搜索对照验证该类输入；
单调案例同时对照全局枚举，不将修复测试写成对所有文本的全局最长前缀承诺。

## 成本对照结果

修改前后各运行三次，顺序为 baseline／adopted、adopted／baseline、
baseline／adopted。使用冻结的 release 可执行文件，采样期间没有并行构建，
关闭 tracing、单线程执行。表内为三次总耗时的中位数，每组截断运行 20 次。

| 内容／可用宽度占比 | 修改前（μs） | 修改后（μs） | 耗时下降 |
|---|---:|---:|---:|
| 均匀文本／10% | 22,108 | 20,533 | 7.12% |
| 均匀文本／50% | 62,331 | 55,296 | 11.29% |
| 均匀文本／90% | 111,525 | 100,930 | 9.50% |
| Unicode 混排／10% | 32,324 | 29,444 | 8.91% |
| Unicode 混排／50% | 100,070 | 91,012 | 9.05% |
| Unicode 混排／90% | 166,508 | 156,717 | 5.88% |
| 不均匀文本／10% | 50,356 | 46,861 | 6.94% |
| 不均匀文本／50% | 108,020 | 98,686 | 8.64% |
| 不均匀文本／90% | 137,738 | 129,994 | 5.62% |

每次采样的九组合计耗时中位数从 790,843 μs 降至 721,667 μs，下降 8.75%。
此值先对每轮求和再取中位数，因此不等于表内各组中位数之和。
每组 layout 调用减少 20 次，即每次截断少一次最终重复排版：
原来的 260／240 次变为 240／220 次。缓冲区复用与获选布局复用一起测量，
没有分别测出两项的独立贡献。

同一 descriptor 的字体指标查询在预热后重复 1000 次，
耗时中位数从 4,363 μs 降至 7 μs；该结果只代表单条目持续命中的情况，
不代表交替字体、首次加载或完整文字排版的耗时。7 μs 样本很短，
不据此推导其他场景的加速倍数。

原始日志为 `0-baseline.log` 至 `5-adopted.log`，统计为 `comparison.json`，
可执行文件 SHA-256 和交替顺序为 `receipt.json`，均放在下述中间产物目录。

## 正确性与门禁

新增四项 release lib 测试通过：真实字体截断的旧二分对照、
grapheme 边界与获选结果复用、空输入与测量错误、字体指标缓存失效与失败恢复。
五个 integration target 的 45 项 release 测试全部通过：

- `m10_label_contract`：13 项。
- `p2_t01_text_flow_contract`：3 项。
- `p2_t02_text_interaction_contract`：15 项。
- `font_decoration_image_contract`：2 项。
- `p2_text_graphics_contract`：12 项。

这些回归包含外部文本引擎、字体资源替换／移除、UTF-8 截断、双向文本、
caret／selection 和统一 Graphics 消费。测试构建仍有既有的
`harfrust` qualification、support dead code 和未消费 `SubmissionAcknowledgement`
告警，本次没有修改这些无关路径。

执行命令（构建目录统一为 `CARGO_TARGET_DIR=/tmp/novadraw-text-stage-target`）：

```sh
cargo test -p novadraw --release --test m10_label_contract --test p2_t01_text_flow_contract --test font_decoration_image_contract --test p2_text_graphics_contract --test p2_t02_text_interaction_contract
cargo clippy -p novadraw --lib -- -D warnings
cargo check --workspace
cargo xtask docs
cargo fmt --all -- --check
git diff --check
```

Core lib Clippy（`-D warnings`）和 workspace check 通过。
workspace fmt 检查和 `git diff --check` 通过。
方案的类型标记修正为 `proposal` 后，docs 门禁通过：
497 个 Markdown 文件及当前 xtask 引用通过，design 检查为 0 errors。
工作区依赖仍报告 `block` 的 future incompatibility 提醒；本切片未修改该依赖。

## 证据边界

成本入口是两个明确忽略的 release lib 测试 `draw2d_truncation_cost` 和
`draw2d_font_metrics_cost`，需显式 `--ignored` 才运行。
前者用真实 Parley 和内置字体，并记录外部 TextLayoutEngine 包装器的 layout 次数；
后者重复查询相同 descriptor。计时关闭 tracing、单线程执行。

中间产物位于 `target/verification/text-draw2d-adoption-2026-11-05/`。
`baseline-tests` 是代码修改前冻结的测试可执行文件，`baseline.log` 保存首轮基线。
修改后的可执行文件、交替对照和统计在同目录保存。
计时单位、迭代次数和数据分母以日志为准，不从调用次数直接推算耗时。

这些测试只测 CPU 截断和字体指标查询，不测结构化 Flow 的整体组装、拖拽、
连续 resize、GPU 完成或 WindowServer 呈现。既有长文本对照不能被这些样本替代。
