# Parley 0.12 输入检查与滚动分离验证

类型：`implementation-verification`

能力归属：Core Preparation、Flow、布局与 Runtime；研究宿主负责输入回放、
窗口尺寸确认和独立计时；Vello backend 只提供现有绘制与队列完成证据。
本轮不修改 Editor、文字语义或第三方依赖。

日期：2026-11-05。状态：执行中。基线与归因已完成；局部扫描缓存已撤回；
静态读取削减在三轮离屏对照中有净收益，试验版暂保留，原生补验尚未完成。
纯投影分离已形成具体传播契约草案，尚未接受或实现。
用户已授权按“新基线 → 输入检查有效区间 → 纯滚动与排版失效分离”的顺序推进。
执行归入唯一 FR-02 切片，队列由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
本轮不自动提交后续修改。

`api_semantics`：`frame.preparation`、`text.flow`、`text.interaction`、
`layout.manager`、`viewport.scroll_zoom`、`notification.layout_update`。
语义依据为[现行账本](../../parity/draw2d/api-coverage.md)。

## 本轮要确认什么

Parley 0.12 迁移已通过编译和回归，但尚无匹配交互性能采样。
[上一轮删减验证](rich-text-pipeline-reduction-2026-11-05.md)的步骤 2D
减少了重复 route 祖先查询，资格读取与校验次数仍未减少。
旧版耗时只用于确定测量方向，不与新版样本拼接为前后对照。

本轮先记录当前默认完整组装与显式复用路径的成本，再用相同基线判断下一项改动。
净收益由关闭 tracing 的独立计时判断，Perfetto 只解释成本归属。
在完整输入仍有效时，目标是减少重复读取和物化；外部回调改变输入后仍须拒绝旧结果。

## 基线与证据

代码起点为 `7ab036ce`，默认文字依赖固定为未打补丁的 Parley 0.12.0。
使用既有 ARM64 CLT 构建配置和 `/tmp/novadraw-text-stage-target` 缓存；
冻结 binary、构建日志、Rust／SDK、锁文件与 SHA-256 收据。
证据根目录为 `target/verification/text-fr02-parley012-input-scope-2026-11-05/`，
不纳入 Git。`baseline/receipt.json` 记录构建收据；release 构建用时约 390 秒，
已冻结 binary，SHA-256 为
`1915a4cae39402a8fbfbb751406eb1aecccf980bc3cba409e432e3b0f5560ee3`。
锁文件 SHA-256 为
`b13613e9294ed5ff2db19c13eeb5a13f5288a193bcd7bafc6c3d29735619e342`。
构建期间只修改了本轮文档和采样编排脚本，尚无 Rust 实现差异；
工作区状态完整保存在收据中，不将 build-time Git metadata 当作逐文件源码散列。

采样沿用现有 `--interaction-replay`，覆盖 thumb 按下／拖拽／释放、
Winit 滚轮、连续窗口 resize、追加和 Stable。
固定字体、正文、窗口、预算、damage 与输入序列，纹理缓存关闭。
独立计时与细阶段 trace 分开采集；GPU queue wait 口径一致。
配置交替并重复进程，保留 P50／P95／P99、操作计数、位移、文字摘要和 glyph／command 数。

Native drawable 不能呈现时，失败证据保留，离屏 GPU 结果单列。
离屏发布、窗口尺寸确认和 GPU queue wait 均不能证明 WindowServer 呈现或 scanout。
失败、未完成、零位移假回放及 trace 丢失不用于成本结论。

分析复用现有
[交互计时分析器](../../../tools/scripts/analyze_textflow_interactions.py)、
[Perfetto SQL](../../../tools/scripts/textflow_scroll_perfetto.sql)和
[warm-session 分析器](../../../tools/scripts/analyze_textflow_scroll.py)。
原始查询、schema、health 与逐操作分母随样本保存。

## 输入检查的边界

`PreparationInputs` 已以不可变 tree/style 借用为静态缓存权威。
该借用不能证明扩展私有状态不变：现行合同允许 Runtime 串行回调改变输入。
`PreparationScene::input` 必须保留首次值，后读冲突和原始错误不能被新值覆盖。
注册退役、约束绑定、嵌套 reader 的独立日志和最终发布检查继续保持。

首片只考虑明确无相关写入、无可能改变依赖的回调的消费区间。
读取复用必须绑定 root、source、注册实例、完整约束和该区间；
区间结束后不能把旧成功结果带入下一轮稳定化或新的回调。
动态声明和 provider revision 不因静态树借用而自动缓存。
若基线支持局部批次内重复读取合并，先在
[FR-02 合同](../../design/architecture/flow-assembly-reuse.md)中冻结实际机制与失败边界，
再实现并补外部消费者反例。当前尚未扩大任何现有检查的有效区间。

## 纯滚动的边界

第二项正确性及净成本成立后，再区分 viewport 投影与真实排版依赖。
仅投影变化的合格路径复用受检布局，更新坐标消费者和 damage；
实际宽度、字体、来源、inline 反馈或 contributor 依赖变化时，仍执行必要排版。
不按事件名或一个全局 revision 强制跳过测量，也不返回不匹配当前源的旧布局。

验收包含跨 viewport 命中与选区、连接坐标、滚动条宽度反馈、真实 inline、
回调改变旧候选输入、注册退役、预算失败和恢复。
纯投影样本需记录新增 assembly、文字测量、shaping、换行和来源几何工作，
性能结论同时覆盖首次、不合格路径、拖拽、滚轮及 resize。

## Parley 0.12 基线结果

同一冻结 binary 交替运行六个进程，每个配置三轮、每轮两个 cycle，
关闭 tracing、开启 GPU queue wait、自然 damage、纹理缓存关闭。
每配置 753 个操作，文本摘要与位移逐操作一致；
`baseline-off-on/geometry-check.json` 保存匹配字段及分母。
这是离屏 GPU 数据，独立计时的中位数如下。

| 操作 | 默认完整组装 active ms | assembly reuse active ms | 差值 |
|---|---:|---:|---:|
| 拖拽 | 30.10 | 37.29 | +7.19 |
| 滚轮 | 30.00 | 36.64 | +6.64 |
| 缩放 | 56.27 | 65.69 | +9.41 |
| 追加 | 38.79 | 46.98 | +8.19 |
| Stable | 4.92 | 12.00 | +7.08 |

`active_ms` 为输入、frame 和 GPU wait 的合计；P95、P99、逐次耗时与配置见
`baseline-off-on/comparison.json` 和 `receipts.json`。当前复用路径有净回退，
不能用 assembly 次数减少支持默认启用。

单独的细阶段 trace 在 `baseline-fine/`，不参与净收益对照。
`baseline-fine-analysis/` 保存 schema、全部 SQL 导出、health 和逐操作分母；
没有 error/data_loss、未结束 scope、负 exclusive 时长或回放失败。
40 次有效拖拽各有 29 次资格读取、27 次校验；资格读取 exclusive 平均
5.875 ms，校验 inclusive 平均 6.564 ms，两者存在包含关系，不能相加。
40 次缩放合计 3388 次资格读取、2870 次校验；资格读取 exclusive
平均 15.149 ms。缩放还有平均 2.059 ms 的 shaping，当前主要削减方向仍是输入检查。

通过既有 LaunchServices launcher 的原生 probe 已完整回放，128 个 operation，
122 次 `native_backend_presented`、6 次 `no_damage`，初始化有一次 `not_presented`，
没有 failure，末尾有 `replay_complete`。原始记录在
`native-probe/timings-25823.json`。此次原生 drawable 可用，将补匹配的原生前后对照；
单次 probe 不用于净收益，也不等同于 WindowServer 或 scanout 延迟测量。

原生基线也已完成相同的六进程交替采样，每配置 753 个操作。
`native-baseline-off-on/` 保存逐操作文本和位移匹配、配置、收据与分位数。

| 操作 | 默认完整组装 active ms | assembly reuse active ms | 差值 |
|---|---:|---:|---:|
| 拖拽 | 31.03 | 37.41 | +6.38 |
| 滚轮 | 30.74 | 37.94 | +7.20 |
| 缩放 | 57.56 | 68.11 | +10.55 |
| 追加 | 40.31 | 47.68 | +7.37 |
| Stable | 4.95 | 12.17 | +7.22 |

复用配置中，单次有效拖拽新增两个 assembly hit，新增 assembly build、
styled layout 和 source geometry 均为零。每进程两个 cycle 的累计 assembly build
为默认 1379、复用 87，styled layout 两者均为 170、source geometry 均为 814。
减少组装工作没有抵消输入检查成本；原生与离屏都支持当前净回退判断。

注意 binary 内嵌的 Git 字段为旧构建脚本产物，probe 记录中的 `7902c118…`
不是此次源码起点。对照使用冻结 binary 的 SHA-256 与独立收据识别版本，
不以这个内嵌字段推断逐文件源码。采样脚本新增 `--native`，每次将冻结 binary
复制到独立 app bundle 并通过 LaunchServices 执行；原有离屏默认不变。
采样还要求 `replay_complete` 和匹配的 backend outcome。

## 布局失效扫描试验的裁决

试验版已冻结于 `step2/textflow-large-app`，SHA-256 为
`1c450905fa0ddd1d063b10477f7d0570894ea3324d5b7c8b62947bd2e01dc6f3`，
构建及定向测试共约 194 秒，源码散列、命令与 patch 在 `step2/receipt.json`。
`step2-native-comparison/` 为与原冻结基线交替的三轮原生回放；
双方均显式开启 assembly reuse，关闭 tracing，GPU wait 口径相同。
753 个操作的文本摘要和位移一致，主要操作各 240 个样本。

| 操作 | before P50 active ms | after P50 active ms | before P95 | after P95 |
|---|---:|---:|---:|---:|
| 拖拽 | 35.790 | 35.694 | 39.462 | 38.862 |
| 滚轮 | 35.861 | 35.364 | 39.285 | 38.750 |
| 缩放 | 63.704 | 63.429 | 69.974 | 70.103 |

变化很小，不能据此确认所提机制有效。`step2-fine-analysis/` 的完整 Perfetto
trace 通过相同 health 门禁，但资格读取仍为每次拖拽／滚轮 29 次、校验 27 次，
缩放合计仍为 3388 次资格读取、2870 次校验，完全未减少。
目标样例在这个局部扫描内没有可合并的重复绑定；较低的细 trace 耗时不能算净收益。
因此该试验实现不保留，撤回局部缓存，继续在真实重复读取的区间寻找削减点。
不能靠扩大缓存到任意回调之后来制造命中。

## 当前结果

release 基线构建通过，本轮 503 份文档及索引检查通过。
局部扫描复用试验已完成并决定撤回；有效输入区间削减、纯滚动分离与最终门禁尚未完成。
撤回前的局部扫描试验通过 17 个 Preparation input 测试和 8 个独立消费者测试；
其中 5 个专门覆盖已撤回机制的测试随实现退役，日志仍保留。
新增嵌套日志测试首次误把捕获时已有的 inner proof 当作空日志，已修正为检查
验证不增加原日志；失败原始日志保留在 `step2/input-tests.log`，
修正后结果在 `step2/input-tests-retry.log`。没有因此改变 reader 或合并语义。
设计契约检查为 103 份 Markdown、0 错误；它不替代完整文档链接门禁。

纯滚动的静态定位发现：`ViewportHandle::set_view_location` 本身只发出属性、
坐标变化和 repaint，未直接标记排版失效。`ScrollBar::sync` 在 range snapshot
变化时请求 invalidate；`mark_validation_path_invalid_for` 沿父链失效。
随后 `PreparationLayoutRead::preferred_measurement` 通过权威测量管线调用
`ViewportLayout::preferred_measurement`，后者继续测量 contents。
这解释了“纯投影时仍有两个文字测量 cache hit”的调用路径。
目前属于静态证据，尚未据此改动传播规则。

后续设计必须让自定义布局、track policy、contents 宽度、range extent、
inline、连接与选区的实际依赖继续驱动失效。不能直接删除 scrollbar invalidate、
按滚轮事件名跳过布局，或把准备快照尺寸当成任意约束下的首选尺寸。
只读参考发现 Draw2D `ScrollBar.java:354–359` 明确覆盖 `revalidate()`：
只 invalidate 自身并加入 UpdateManager，因首选尺寸只有 orientation 改变才变化；
`Viewport.java:143–149` 也有 local revalidation。后续需据此定义通用局部重验合同，
并核对未知自定义父布局与动态输入的恢复条件，不能为内置 ScrollBar 留特权路径。
默认并发的 GPU Area 精确像素波动沿
[迁移验证](parley-upstream-migration-2026-11-05.md#gpu-对照波动)保留，
不通过放宽像素判据关闭。FR-02 与整体尾延迟验收保持未完成。

## 同一次资格扫描的静态读取试验

下一版仅合并 capability 与所属 Figure 的静态受限读取，在同一个
`read_input` 调用内复用预算句柄。两次读取之间没有扩展回调，
树、style 和 attach-time capability 由同一不可变 Scene 借用约束；
外部 resolver、contributor／handler 声明和 provider revision 仍逐次执行。
所有 input 捕获、首次值比较、消费和最终发布检查保持原位置。
本项减少静态查询与句柄复制，不宣称减少动态资格读取次数。

试验版在 `step2b/textflow-large-app`，SHA-256 为
`abb31f440f66886443bfa3fe00a1632bd2d893c92795c4fce7c7bb323f717e2f`。
`step2b/receipt.json` 与 `source.patch` 保留源码、Rust／SDK、命令和构建收据。
9 个 Preparation scene 测试、8 个独立消费者测试和 release 构建通过，
总用时约 187 秒。新增反例覆盖合并读取的子树权限、route geometry
观察、缺失 capability 和预算耗尽。

`step2b-native-comparison/` 的前四个进程完成各 251 个操作，
第五个基线进程在 180 秒后超时，未生成 timing 文件。
进程路径确认属于本轮冻结的 baseline app bundle；普通 `sample` 被系统拒绝，
未获得堆栈。具体原因未确认，没有将它归因于 Core 实现。
已向这个确认过路径的采样进程发送 SIGTERM，失败收据与完整的四进程记录保留。
整批不满足三轮门槛，不从其中挑选较快样本作为净收益证据。
采样器改为持续保存 stdout／stderr，超时写独立 failure receipt。
`step2b-native-retry/` 的首个基线进程完成，随后试验进程再次在 180 秒后超时，
未生成 timing 文件；确认路径后已发送 SIGTERM。该批也不纳入净成本结论。
LaunchServices 的 app stdout／stderr 不继承 launcher 的重定向，
采样器已用 `open --stdout/--stderr` 单独保存应用日志，供后续原生补验。
这些改动改善证据留存，不能解释或关闭两次原生超时的原因。

`step2b-offscreen-comparison/` 已完整完成六个交替进程，每配置三轮、每轮两个 cycle，
关闭 tracing、GPU wait 开启、自然 damage、纹理缓存关闭。
两侧均显式开启 assembly reuse，753 个操作的位移和文本摘要逐项一致。
主要操作各 240 个样本，活跃总成本如下。

| 操作 | before P50 ms | after P50 ms | P50 下降 | before P95 | after P95 | before P99 | after P99 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 拖拽 | 37.154 | 35.177 | 5.32% | 42.033 | 39.909 | 50.283 | 48.414 |
| 滚轮 | 36.585 | 34.976 | 4.40% | 43.019 | 40.234 | 48.179 | 43.377 |
| 缩放 | 65.311 | 61.997 | 5.07% | 72.409 | 69.500 | 76.145 | 72.781 |

三个匹配 run 的主要操作 P50 均下降。Stable 的汇总 P50 为
11.268 → 10.878 ms，追加为 46.855 → 44.879 ms，但只有 9／6 个样本，
不用于强尾延迟结论。最大值没有全部下降；逐操作样本、完整分位数、
配置与 SHA-256 见 `receipts.json` 和 `comparison.json`。
结论限于这一组合静态读取削减在离屏 GPU 下的净收益，
没有分拆 capability 合并与预算句柄复制各自的收益。

`first-operation.json` 单独保存三个首次 Reset 的几何匹配、query、operation、
frame 与初始 GPU wait。首次 query P50 为 50.626 → 38.606 ms，
但一组 after 有回退，且初始 GPU wait 波动明显；三个进程不证明冷设备启动改善。
不合格扩展的正确性由定向反例覆盖，独立成本尚未采样。

`step2b-fine-analysis/` 通过与基线相同的 schema、health、未结束 scope 和
exclusive duration 门禁。每次拖拽／滚轮的资格读取仍为 29 次、校验 27 次，
缩放合计仍为 3388 次资格读取、2870 次校验；
Flow measurement hit 的次数也未改变。
资格读取 exclusive 均值为拖拽 5.875 → 4.683 ms、
滚轮 5.815 → 4.532 ms、缩放 15.149 → 12.222 ms。
这是诊断采样，用于解释成本所在，不替代关闭 tracing 的对照。
资格读取与校验存在包含关系，时长不能相加。
本次收益支持削减每次重读的静态开销，不支持删除动态 reader 或最终发布检查。

本片 workspace fmt、Core `--lib` Clippy（`-D warnings`）、11 个资格／缓存反例
以及完整文档门禁通过；加上前述 9 个 scene 和 8 个独立消费者，共 28 个定向测试。
日志在 `step2b-checks/`，当前全量文档检查为 505 份 Markdown，
设计检查为 105 份、0 错误；计数包含同时存在的其他文档。
原生三轮尚未完成，因此试验代码暂保留，原生接受门槛与 FR-02 整批保持未关闭。

## 纯投影分离的具体候选

[按属性依赖限定布局重验](../../design/architecture/property-scoped-layout-revalidation.md)
已记录即时失效、验证队列再次传播和 LayoutOutput 内 page 部件 resize 的三个传播点。
候选使用 typed 属性依赖声明与未知父布局回退，队列保存实际失效范围；
内部 child 几何反馈只在合格输出提交区间内停止向 owner 以上传播。
外部 callback、宽度变化及普通失效继续完整传播。
执行前还需用已有 typed input reader 重验非目标输入 guard；
value consumer 后续改变 extent 不能继续采用旧局部资格。
该机制尚未接受或实现，不能以新增局部入口代替完整队列与回滚验证。
