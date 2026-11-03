# B1：共享规范化内核的操作准入

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、Native／portable 测试、独立普通输出矩阵、
最终源码成本采样及跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

前片的[第二字符缺字准入](text-stages-b1-decomposition-miss-2026-11-03.md)
只证明输出长度不变的路径。进一步支持分解输出，需要在 normalizer 内部
按实际操作计费，并在数组增长、排序及重组前传播拒绝。
本片先建立这个共享内核；组合字符的其余必经阶段仍待补齐，
公共入口继续在预检时返回未支持。

## 共享算法与操作顺序

普通入口使用 `OrdinaryShape` 的无操作策略，准入入口使用同一泛型内核。
Unicode 分解、字体映射、shortest 规则、排序比较、重组和数组切换顺序保持。
预算拒绝立即返回原始错误，不转入普通入口重做。

| 路径 | 操作前准入 | 状态／费用依据 |
|---|---|---|
| 分解 | 每次 Unicode 查询、a／b 映射、递归与输出分别计费 | 重复查询仍付费；未知脚本回调由公共预检拒绝 |
| 输出 | 先检查长度，再预约两个数组，之后复制和写入 | 新容量计整份 slots；迁移、初始化、输出前缀和后续复制分别计费 |
| 重排 | 每次比较和移动前计费 | insertion sort 的每次移动重新覆盖簇合并，不假定扫描区间互斥 |
| 重组 | composition 查询、映射、复制、簇合并、属性初始化分别计费 | 合并上界取当时的输入与输出长度，每次重组重新支付 |
| 后续准备 | normalizer 返回后进入 `Phase::Normalized` | masks、映射、GDEF 和 digest 按当前逻辑长度或完整初始化数组计费 |

原先 `Phase::Substitute` 的 singleton normalizer 预付已删除，
该阶段只保留 rotate／mirror 费用。这样每笔实际规范化操作由内部策略负责，
不会再由外层估算重复收费。

Unicode 分解保守界覆盖固定 Hangul 运算、生成表索引及一份 payload。
组合查询从锁定的 `ucd_table::ucd` 数组读取长度，
按 Rust 1.96 的 `binary_search_by` 比较上界 `bit_length(n) + 1`
及一次结果读取收费。它不执行搜索来反推费用。
输出属性初始化先支付 GC、default-ignorable、CCC 与 remap 的保守界，
不先查类别再决定是否收费。

排序沿用上游的 `merge_clusters` 行为，包括原有 `while end < start` 条件。
本片不调整该语义。每次移动按当前 `len + out_len` 重新支付簇访问界，
另加真实移位距离的读取与写入。
令 R 为该次 `len + out_len`，d 为移位距离：
Information 上界为 `12R + 2d + 2`，Planning 为 `16R + d + 16`。
前者覆盖 minimum、扩展比较、簇读写、unsafe 标志及移位；
后者覆盖每轮索引、边界和分支。重组的每次簇合并另收
`16R + 2` 次 Information 与 `16R + 32` 次 Planning。
两种 R 都在对应合并之前读取，不以最初输入长度替代。

`ensure_admitted` 在任一 resize 前完成两份数组的预约和初始化准入。
失败不会留下不同的初始化长度；已经取得的容量和已完成的费用保留。
单个输出的复制与数组第一次切换的前缀复制分别准入，
一对一替换也不被当成零复制。

## 内部测试与独立原版对照

新增三项排序测试用冻结的原始排序函数比较 32 组簇与输出配置，
覆盖每个拒绝点的状态保持、重复合并费用及算术溢出。
新增三项内部 normalizer 测试覆盖分解增长、排序、重组、缺字、
192 组输入配置、逐点拒绝后的 buffer 复用和 `max_len`。
Native／portable 各 211 项依赖内部测试与 102 项消费者测试通过。

内部普通／准入比较共用生产算法，因此另用未修改的 Harfrust 归档
运行普通完整输出矩阵。新矩阵每配置 614,400 份：
8 内核 × 2 GDEF × 8 mapping × 2 轮次 × 5 script × 2 方向
× 10 输入 × 3 cluster 模式 × 4 cluster level × 2 容量。
输入包含分解增长、连续 mark、重排、CGJ、variation selector、
Hangul 与超过排序长度限制的 mark run。
script 包括 None、Latin、Han、Hebrew、Arabic；
这些普通输出对照不表示公共准入支持了对应脚本或组合输入。

原二十矩阵的 fixture 与 oracle 保持冻结。首轮验证工具核对原版输出的
全部解压字节、签名和分母后复用旧原版配置，生产版本重新执行；
新增矩阵重新执行原版／生产 Native／portable 四份配置。
二十八份补丁已精确重放，核对全部 82 份源码；
本片涉及五份生产修改及一个新增内部测试模块，其余 76 份源码保持原字节。
增量补丁 SHA-256：
`c089c66bbc58bc13e86af6aaa2b9e98311b84a9d48cfea53313b995ae780d59b`。
二十一矩阵四配置共 8,796,096 份观察一致，
新增四配置签名均为
`d691d1be0ca98c7eaf25c58a130cd9b2afe302635a168a241d586920f262c733`。
首轮旧原始版本 40 份配置逐字节核对后复用；
生产四十份及新增四份配置重新执行。额外 invisible 参考的
160 份观察仍使用前片冻结收据，源与输出身份再次核对。

两项既有消费者断言随费用归属迁移：
三个输入映射由预检与实际执行分别付费，记录六次独立 Mapping；
第二字符缺字与普通缺字的差值改为四次独立查询，
分别对应预检 a／b 和实际执行 a／b。
冻结输入、普通输出和原始错误合同不变。

初次内部测试中，普通 `push_str` 的几何容量掩盖了逻辑长度从 2 到 4 的增长，
现使用精确准入输入容量；恢复后的普通调用也明确设置 LTR。
相关失败日志保留。旧聚合费用断言失败的日志同样保留，
不作为最终通过证据。
首轮完整矩阵在压缩期间主动中断，保留 `before-fast-compression` 输出；
新增测试的格式已修正，最终轮使用较低压缩级别。
原始观察的全部解压字节、签名与分母仍逐一核对。
随后修正 `shape_admission.rs` 两处链式调用的格式。
格式复用模式重放首轮补丁，要求新文件严格等于旧文件的 Rustfmt 输出，
其他 81 份源码保持原字节，fixture 与 oracle 身份不变。
该证明已通过，原版与生产各 42 份普通输出配置已逐字节复核；
每份都解压核对全部字节、签名和分母。
定向测试、成本采样及最终门禁重新执行，首轮收据保留于
`normalizer-kernel-before-formatting-`。
最终门禁再次重放旧、新补丁并核对全部源码，
重新运行 Rustfmt 证明，还比较格式前后采样的全部 work、allocation、
calls、glyphs、Mapping／Unicode work。上述费用与操作次数均保持一致，
延迟数据重新采样。首次格式复用脚本误将共享 fixture 当作同名独立文件，
现按冻结 fixture 集合校验；失败日志保留，不作为通过证据。

## 成本采样与完成边界

成本驱动沿用前片冻结的 `ABéÅÅǻCDE` 输入和采样程序。
准入四种模式共采 192,000 条；原版／生产普通入口三个交替会话
共采 144,000 条。另用包含分解查询、排序和重组的 1,280 字符输入，
对普通原版／生产入口采三个交替会话，共 144,000 条。
五个成本二进制在第一条计时样本前均通过 Clippy。
准入四种模式只测已支持的 singleton 路径；其余为普通入口对照，
不测公共组合字符准入。
系统负载未控制，数据不替代 Core 完整排版、GPU 或 16 ms／4 ms 验收。

普通 mark 成本输入将 `ÅÅéǻA`、U+030A、`a`、
U+0315／U+0300／U+0316 重复 128 次。覆盖三种布局内核、
glyph class 为空或 Format 2 mark class 两种 GDEF、
mark 映射存在／缺失、LTR／RTL 和冷／热 lookup，共 48 组输入。
每组每会话取 500 条有效样本及 100 次预热。
本组用合成字体检查共享内核开销，没有测真实字体轮廓定位或 GPU。
两组映射保留预组合字符字形，只切换 mark 的字形是否存在。
因此本组测分解查询、重排和重组，没有单独测分解输出导致的数组增长；
增长、复制与长度限制的费用由内部测试核对。

下表为准入成本程序各 48 组输入的延迟范围，单位为微秒。
计时含 shaping、成功结果释放或拒绝后的内部清理；
输入准备、预热和保留 buffer 的最终释放不在计时内。

| 模式 | P50 范围 | P95 范围 |
|---|---:|---:|
| 普通入口 | 48.708–120.583 | 51.042–126.833 |
| 准入／空回调 | 91.875–164.583 | 95.583–171.583 |
| 准入／计数回调 | 98.084–171.542 | 102.917–179.167 |
| 最后一点拒绝 | 98.375–172.875 | 101.167–178.584 |

独立普通对照的计时不含构造、准备和结果释放。
singleton 三会话配对 P95 的生产／原版比值为 0.979–1.121；
mark 三会话为 0.623–1.126。mark 原版各组 P95 为
72.583–211.000 μs，生产为 71.959–195.333 μs。
这些波动没有控制系统负载，不能据此认定提速或回退；
更不能作为 Core 完整排版、缓存命中或 GPU 性能的结论。

最终十四项定向门禁通过。库 Native／portable Clippy
与冻结前片六项既有诊断一致，新增诊断为零。
消费者及三个生产成本程序 Clippy 通过；Core Native／Wasm check
与消费者 Wasm build 通过。Wasm 只构建，未执行测试。
三份成本汇总、原始 NDJSON、源码及编译前 Clippy 收据均核对身份。

公共 mark、variation selector、非 singleton 输出与未知
compose／decompose／reorder 回调继续由预检拒绝。
variation selector 内部 helper 仍走普通路径，没有完成其准入证明。
原始 mark 的 Unicode 初始化、dotted circle、簇形成、方向转换、
fallback mark 定位、GSUB 增长计划与恢复责任仍需单列补齐。
本片不能关闭 B1，也不能作为启用生产缓存的依据。

复验按以下顺序执行，旧切片脚本仅用于其冻结快照：

```bash
python3 -B tools/scripts/check_harfrust_normalizer_kernel_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target
python3 -B tools/scripts/check_harfrust_normalizer_kernel_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target
python3 -B tools/scripts/check_harfrust_normalizer_kernel_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm
```

报告交付前按「好好说话」及技术写作规则显式自审，
核对状态、观测分母、成本单位和未完成边界；机械检查结果见下文。

成稿的编号标题、翻案句式、元话语和中文半角标点四项扫描均为零候选。
节奏检查有 8 处候选，逐处回读后确认属于链接、技术名、
表格摘要或完整验收判断，保留原文。
分别回读事实、推理和措辞，确认普通输出复核、费用验证与延迟结论的范围分开，
未将内部 normalizer 通过写成公共组合字符支持或 Core 提速。
本次为作者自审，没有独立读者复核；机械扫描不替代逻辑审查。
