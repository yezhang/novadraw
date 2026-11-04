# FR-02 贡献组装内部成本归因

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core Flow 的只读诊断；
官方例子驱动与采样器提供研究消费者，Editor／backend／platform 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`、`layout.manager`、
`viewport.scroll_zoom`、`notification.layout_update`。
状态：诊断、定向回归与匹配归因完成；生产复用合同待评审，未实施。

## 当前结论

原 Large 的 resize 每次仍组装 13 次，调用 contributor／handler 各 7566 次，
执行 6513 次文本追加。细分诊断中 contributor 仅占组装累计 exclusive 总量的
4.23%，String 追加占 2.88%；分发、解析、遍历、样式与来源 metadata 的重复更广。
这些数据支持评审完整组装资产，不支持把全部成本归为字符串 clone。

细阶段计时明显扰动成本，不能把本轮细分版本的约 14 ms 组装均值当作生产可回收时间。
诊断关闭路径的尾延迟也出现未解释变化；本轮关闭成本归因，不关闭性能回归或
生产默认启用门禁。复用的实际净收益必须在合同接受、实施后重新做匹配对照。

## 本轮实际改动

沿用默认关闭的 TimingRecorder，在同一开放路径拆出贡献解析、contributor、
handler、遍历、block、文本追加与来源 metadata、边框 renderer／metrics。
阶段名称只用于诊断显示，不参与行为身份、缓存资格或失效。
没有新增能力枚举、内置特权、全局状态、热路径日志或 production cache。

`flow-source-map` 有两个 scope：ancestor／source offset 捕获，以及 span／source 追加。
因此 calls 是 scope 数，不能当 source 数。文本 String 追加单列，
contributor 的字符串 clone／payload 装箱仍包含在 contributor 内，
没有 allocator 埋点，不能将其全部成本称为复制或分配成本。

真实 inline Figure 在 FlowPlan 完成后才做 prefix layout 与 measure；
本轮分别增加 `flow-inline-prefix` 和 `flow-inline-measure`，不计为贡献组装。
prefix 的 engine 子阶段有独立计时，不能把 inclusive 与子阶段相加。
边框在组装内发生，缺少真实调用的负载保持未测，不补造耗时。

| 计时边界 | 源码依据 |
|---|---|
| resolve／contributor／handler | [开放分发路径](../../../crates/novadraw/src/figure/flow/protocol.rs) |
| visit／children／block／text／source-map | [统一组装器](../../../crates/novadraw/src/figure/flow/assembly.rs) |
| border／inline 与 FlowPlan 前后关系 | [构建与布局](../../../crates/novadraw/src/figure/flow/snapshot.rs) |
| inclusive／exclusive、关闭和 scope 结束 | 历史实例计时器：`72077f53` 的 `engine/novadraw/crates/novadraw/src/runtime/timing.rs` |

2026-11-04 起该计时器已删除，原源码可用
`git show 72077f53:engine/novadraw/crates/novadraw/src/runtime/timing.rs` 查阅。
当前采集按[Perfetto 合同](../../design/architecture/perfetto-observation.md)执行，
本报告的旧版归因数据和结论保留。

## 构建、分母与证据

以 `75494e84` 加本轮诊断工作区为基线。先构建 coarse，再加入 Core 细阶段构建 fine；
二者是诊断模式对照，不是 FR-02 生产优化前后版本。
使用 Rust 1.96.0、相同锁文件、默认 release、字体、驱动与准备预算，
`CARGO_INCREMENTAL=0`、单构建 job，复用 `/tmp/novadraw-text-stage-target`。
两版仅因格式化存在驱动空白差异，输入、计时开关和输出逻辑相同。

release 构建沿用历史 CommandLineTools clang 和 SDKROOT 命令，
但 MacOSX15.0 SDK 路径已不存在；由于两版复用已有 C 依赖产物，构建均成功。
debug 测试重建 `psm` 时明确暴露该环境问题，改用已存在的 MacOSX15.2 SDK 与
CommandLineTools `ar` 后通过。实际命令、工具链与恢复说明记录在 build receipt，
不把历史路径存在当作本轮验证事实。

四种负载为原 Large、单长段、NoWrap 长行、真实 inline 混排。
原 Large 工厂 `populate_large` 的静态推导为每 build 582 次 visit／contributor／handler，
501 次 text、41 次 block、81 次 children、1002 次 source-map scope。
这些工厂工作分母已与采样核对；不是性能收益或新的最大容量验收。
初始 Large 保持 40 段／51,232 字节／561 行／1,022 runs／4,168 fragments，
追加和 resize 后保留 51,233 glyph／7,889 commands。

本地证据统一放在 `target/verification/flow-assembly-2026-11-03/`，不纳入 Git：

- `coarse/`、`fine/`：冻结二进制、锁文件、working patch、未跟踪源码和 manifest。
- `build-receipt.json`、`source-receipt.json`：工具链、配置、Core 源哈希与独立工作量推导。
- `preflight.json`：coarse／fine、on／off、Large／mixed 的八组预检，不参与分位数。
- `matched/`：正式采样的压缩原始 NDJSON、summary 和收据。

二进制 SHA-256：
coarse 为 `2c32981e2394c517b88247f606f950f9a10138ca7e4227a6444b2e1f5d4fedd2`，
fine 为 `c5e1a4e1a069f5e4db2adc9fb589dcff3d2bb71b7aa43c3312befd674e0528bb`。
外部 Draw2D 参考提交为 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，
只读 `FlowPage.getPreferredSize`／`invalidate`／`validate`；
宽度缓存与失效语义支持精确复用，但不证明任意 Rust 扩展具有纯度。未运行 Java。

## 匹配测量口径

每负载 coarse-on、fine-on、coarse-off、fine-off 四条 lane，
四会话依次轮转执行位置；每会话 50 个 fresh Runtime 正式样本和 5 个预热。
每 lane 200 个正式样本、20 个预热；全部四负载完成 3200 个正式样本和 320 个预热，
失败为 0。
每个 fresh Runtime 完整执行首次、追加、稳定查询、scroll、resize、CPU IR。
同进程新 Runtime 不表示 OS 缓存已清空，未额外采集所谓受控冷启动。

分位数使用 nearest-rank，预热与预检不进入统计。
逐 sample 检查输出分母、source receipt、零新增 scroll layout、
每个操作的 root inclusive 与全部 exclusive 之和；关闭时不得输出阶段。
独立子阶段 P95 不能相加。成本占比使用相同样本的累计 exclusive 总量，
不能用各阶段 P95 除以总体 P95 推导。

采样期间不运行本 Agent 发起的构建／测试；后台负载、电源与 OS 缓存未完全受控，
硬件与采样开始／结束负载在 receipt 中记录。
本轮不创建窗口或 GPU，CPU IR、backend queue 和屏幕呈现不能互相替代。

macOS 14.7.8、Apple M1 Pro，采样期间 AC 供电；1 分钟 load average 从
18.41 到 14.44。共有 297944 条原始记录，16 条 lane 各有 200 个成功正式样本；
raw、sampler 和二进制哈希复核一致。全部 220 个 fine-on Large 样本（含预热）
的十个 resize 细阶段 calls 与工厂推导一致，共核对 2200 条计数。

## 原 Large 的组装内部成本

下表为 fine-on 的 resize，单位 ms；均值是同一批 200 个正式样本的累计自身成本。
占比用各阶段累计 exclusive 总量除以同一批 contribution inclusive 总量，
本负载没有边框／真实 inline 或嵌套 contribution，所列自身成本之和等于组装总量。
每阶段 P95 独立列出，不相加。所有值包含诊断扰动。

| 阶段 | 每 resize calls | exclusive 均值 | exclusive P95 | 累计占比 |
|---|---:|---:|---:|---:|
| 解析 capability／Figure | 7566 | 3.022 | 3.595 | 21.05% |
| visit 自身：权限／parent、集合与顺序 | 7566 | 2.224 | 2.651 | 15.49% |
| registry dispatch 自身 | 7566 | 2.211 | 2.584 | 15.40% |
| handler 自身：含样式读取，扣除组装调用 | 7566 | 1.869 | 2.348 | 13.02% |
| text 自身：预算与段落初始化等 | 6513 | 1.721 | 2.028 | 11.98% |
| source offset／ancestors／span／来源记录 | 13026 scopes | 1.413 | 1.738 | 9.84% |
| contributor：含 payload 构造和文本 clone | 7566 | 0.607 | 0.782 | 4.23% |
| children 自身：查询与遍历编排 | 1053 | 0.569 | 0.681 | 3.96% |
| String push_str | 6513 | 0.414 | 0.494 | 2.88% |
| block 自身：间距与段落边界 | 533 | 0.247 | 0.303 | 1.72% |
| contribution 外层剩余 | 13 | 0.063 | 0.269 | 0.44% |

这不是原版无诊断的真实阶段占比。scope 的时钟、Mutex 和统计开销会落入各层自身成本；
尤其递归 scope 数多，不能逐项把这些均值作为未来可回收预算。
不过调用次数与源码边界共同证明：只缓存 TextContribution payload 仍需执行数千次
handler、visit、style 与 source 组装。完整 asset 还必须避免在请求构造重新复制。

| 操作 | Flow builds | contributor／handler calls | text calls | coarse-on 组装 P95 | fine-on 组装 P95 |
|---|---:|---:|---:|---:|---:|
| 首次 | 13 | 7566 | 6513 | 10.916 | 16.966 |
| 追加 | 5 | 2910 | 2505 | 4.381 | 6.723 |
| 滚动 | 2 | 1164 | 1002 | 1.684 | 2.582 |
| resize | 13 | 7566 | 6513 | 10.902 | 17.299 |

Large 初始布局请求的 514 个缓存命中与六次真正布局保持；
滚动新增 layout 为 0，resize 新增 layout 为 6。计时没有减少实际工作次数。

## 诊断扰动与其他负载

原 Large 的端到端操作如下，单位 ms；单元格为 P50／P95／P99。

| 操作 | coarse-on | fine-on | coarse-off | fine-off |
|---|---|---|---|---|
| 首次稳定化 | 34.789／40.630／45.758 | 38.623／48.953／61.015 | 33.349／39.717／48.883 | 35.073／46.902／82.211 |
| 追加端到端 | 14.747／17.647／19.378 | 16.184／20.910／30.522 | 14.283／16.873／28.167 | 15.097／18.992／28.712 |
| 滚动 | 6.100／7.358／8.038 | 6.715／8.421／10.583 | 5.893／7.479／8.106 | 6.278／8.730／10.905 |
| resize | 32.321／38.452／46.012 | 36.135／48.157／55.219 | 31.062／35.489／47.677 | 32.686／41.517／61.072 |
| CPU IR | 3.897／4.933／6.241 | 3.745／5.124／6.891 | 3.791／4.958／6.840 | 3.987／5.428／8.772 |

fine-on resize P95 比 coarse-on 高约 25.2%，contribution P95 高约 58.7%；
fine-off 的 resize P95 也比 coarse-off 高约 17.0%。不能全归因为时钟／锁开销，
也不能忽略关闭路径的变化。四会话的 fine-off resize P95 为
40.707／41.157／41.198／45.766 ms，coarse-off 为
33.566／39.218／36.880／32.376 ms。后台负载未受控，代码布局或调用开销未进一步区分，
现有证据不足以证明诊断关闭路径无性能回退。

其他负载的 resize P95 如下，单位 ms。完整各操作分布保存在 summary。

| 负载 | coarse-on | fine-on | coarse-off | fine-off |
|---|---:|---:|---:|---:|
| 单长段 | 39.821 | 39.457 | 38.943 | 41.830 |
| NoWrap 长行 | 19.210 | 17.365 | 17.523 | 17.607 |
| 真实 inline 混排 | 14.224 | 14.740 | 13.165 | 12.958 |

mixed 每次 resize 有 520 次 prefix 与 520 次 inline measure。
fine-on prefix inclusive P95 为 2.032 ms，exclusive P95 为 0.450 ms，
inline measure P95 为 0.649 ms。两阶段在组装外发生；prefix 含其 engine 子阶段，
不能相加这些 P95，也不能把 520 次 measure scene 入口都解释为真实布局器执行。
原 Large 没有边框，本轮未形成边框墙钟分布；边框正例／错误路径只由定向测试验证。

## 设计交付与未完成边界

[ADR-060 草案](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-060-flow-assembly-reuse.md.txt)选择可选精确声明与完整组装资产；
[复用合同](../../design/architecture/flow-assembly-reuse.md)定义实例／registry 绑定、
参与者版本、读取域、共享内容构造、预算、失败与观察回放；
[验收矩阵](../plans/flow-assembly-reuse.md)保留生产行为、成本与开放消费者要求。

当前 `StyledTextRequest` 的文本为 Arc，但构造接收 owned String，span 为 Vec；
直接缓存 FlowPlan 后继续 clone 构造请求会转移复制成本。
合同因此单列公开受检共享内容资产，owned／shared 与外部引擎使用同一路径。
生产资产安装、默认启用、边框声明和宽度独立命中都尚未实现，
不能从本轮计时 scopes 或既有段落缓存命中推断这些要求通过。

ADR-059 B1 继续暂停；本轮不关闭 16 ms 更新、4 ms 查询／IR、
LT、平台或发布门禁。唯一执行状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 验证

30 项定向公开回归通过：`long_text_layout_contract` 7 项、
`p2_t03_structured_flow_contract` 22 项、`p2_t03_external_flow_engine` 1 项。
包含第三方贡献、边框错误留存旧快照、真实 inline 超宽重测、
glyph／行指标等价、diagnostic drain 与失败 scope 结束。
Core `--lib` check 通过；Core lib 与两份受影响 integration target 的严格 Clippy 通过。
第三方 HarfRust 既有 unnecessary qualification warning 保留，不混入本轮修复。
原 SDK 失败和最终通过日志保存在本地 `checks/`，不删除或改写失败。

没有重跑 workspace full、GPU／窗口或 Wasm。本轮没有公开 API／feature 迁移，
生产接口仍是草案；其 compile-fail、独立消费者和生产成本矩阵保持未执行。
原 SDK 错误与恢复日志需要保留，不能将此前失败写成已通过。

## 交付复核

workspace 格式检查、`git diff --check` 和 508 份文档的类型／导航／命令门禁通过。
当前 Core 源哈希仍与被测快照一致；原样本、实际构建配置和失败恢复日志分别保留。
成稿按技术写作核对 owner、状态、计时范围、分位数和因果，再按好好说话检查中文。
四份新文档的机械扫描未发现章节编号、空元话语或中文半角标点；
报告有一处对比句，说明计时版本的适用范围，保留该技术限制。
自审不等于独立评审，ADR-060 与生产矩阵仍需评审。
