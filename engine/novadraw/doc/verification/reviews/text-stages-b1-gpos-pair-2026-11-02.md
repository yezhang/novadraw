# B1：GPOS 配对定位准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：补丁重放、完整输出、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[单字形定位](text-stages-b1-gpos-single-2026-11-02.md)，
允许 PairPos format 1／2 的零 flags 标量定位进入共享准入策略。
已映射 ASCII、LTR Latin／Common／unknown、默认坐标与全局 feature
的支持条件保持。设备修正、变体定位、字形附着、GDEF 与 AAT
仍在 pipeline 前返回未支持，不通过普通入口重做受拒绝的操作。

## 先确认原始字段与矩阵地址

`read-fonts` 的 `ValueFormat::from_raw` 会丢弃保留位，
因此只检查解码后的 `.bits()` 不能证明原始格式合法。
本片在已准入的表头访问内读取原始 `u16`，
检查 placement／advance 的四个标量位；SinglePos 同步使用原始位检查。
上一片把保留位列入拒绝范围，但解码后的检查并不能履行这项约束；
本片以原始位检查和新增反例补齐，前十九份补丁与历史收据保持原样。
设备字段与保留位反例均须在 shaping 前返回 `Ok(None)`，
不改普通 reader 或普通定位行为。

PairPos1 每次按 coverage 选择 pair set，再二分查找第二字形。
预检逐个准入 pair-set offset 与 count 访问，取得最大 set 长度；
定位前重复预检也收费。记录 payload 保持借用，
不先执行一遍定位来获取费用。

PairPos2 先查第一字形的 coverage，再查询两份 ClassDef，
用 class 值和第二维数量计算矩阵地址。
预检先收费两份 class payload 扫描，再求实际最大 class 值，
检查其落在声明的维度内，并检查整个地址计算能用当前目标的 `usize` 表示。
非法 class 值在进入矩阵寻址前返回未支持。
该检查覆盖 32 位乘法的边界；本片没有改 read-fonts 的生产源码。

## 每个 lookup 前覆盖重复访问与安全标记

普通入口和准入入口继续使用 `ot/gpos/pair.rs` 的同一算法。
每个 lookup 前收费 digest、mask／property 检查、子表尝试、
重复表头和 coverage 访问、pair-set 二分或两次 class 查询、
矩阵地址与标量 position 读写。
ClassDef format 1 使用索引读；format 2 的费用覆盖二分比较及结果字段。
Full 缓存按一次 coverage、两次 class 的潜在读写收费，
Small／None 也按能覆盖其表查询的上界收费。

在当前 ASCII、零 flags、无 GDEF 路径上，skipping iterator 不跳过字形。
一次成功配对只推进一或两项，安全标记最多访问两个区间：
配对的两项，以及第二份 value record 存在时的三项。
一次调用覆盖最小 cluster 查询、端点访问与区间内比较和 mask 读写。
等 cluster、倒序 cluster 和三种 cluster level 均受这个实际范围约束，
不假定这些输入的标记成本为常数单项访问。

PairPos 使用 `IN_PLACE`，不增加字形或创建 attachment。
定位的实际数组读写和固定 MappingCache 访问已预付，
执行中没有新增 glyph、position 或 matching 存储。
前置 MultipleSubst 的扩容仍由此前的 GSUB 计划独立收费。
这里的零定位分配结论不代表冷 lookup 构造或整个 shaping 零分配。

## 拒绝与完整结果

新增消费者覆盖两种 PairPos、两种 coverage／class 格式、
第一／第二记录为空或包含标量、一／多子表、一／两 lookup，
以及前置 MultipleSubst 扩张。
普通对照包含命中、未命中、空文本、最后一个单独字形、
RTL 和非 ASCII；所有输出比较完整 info、position 和 flags。
准入消费者另覆盖三种 cluster level、顺序／等值／倒序 cluster、
备用容量，以及实际非零位置修正。

冷／热路径逐个准入点拒绝，检查原始错误 source、精确费用前缀、
后续操作停止和输入恢复。预检中拒绝保留输入；
pipeline 中拒绝恢复空可用 buffer，再用同一 shaper 完成普通调用。
设备位、保留位和越界 class 各有独立反例。
预算单位仍为输入／元素访问次数和整份新 capacity slots，
不改变 Core 的费用或 lease 协议。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_gpos_pair_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_gpos_pair_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind gpos-pair \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_gpos_pair_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

二十份补丁依次重放后，全部 81 份源码与生产逐字节一致。
本片只修改准入模块，其余 80 份保持不变。第二十份补丁 SHA-256：
`b85ffe195d86b5394bf085f536368edf02a39294dd870d9d78627b9d8c3d71a6`。

十三矩阵每配置 376,176 份完整输出，四配置共 1,504,704 份观察一致。
新增配对矩阵每配置 36,864 份，既有十二份 oracle 和完整签名保持。
输出以无损 gzip 保存，散列针对解压后的字节。
Native／portable 各通过 203 项依赖与 71 项消费者测试，本片新增四项。
生产准入模块与第十九片比较，Native／portable 均保留六项既有 Clippy 诊断，
新增诊断为零。消费者与两份成本源码的严格 Clippy 通过；
Core Native／Wasm check 和 portable 消费者 Wasm 构建通过。
Wasm 未执行测试，不能把构建结果解释为 32 位运行验收。

## 成本与普通入口对照

Native release 完成 64,000 个准入成本样本，每组 100 次预热、
1,000 个正式样本，输入 3,072 字符，四种模式按样本轮换，
开启 unsafe-to-concat 标记。计时包含 shaping、缓存、预留和结果／拒绝清理；
输入、data、plan 与热缓存准备在计时外。
下表使用无备用容量的数据，单位为微秒。

| 格式与路径 | 普通 P95 | 计费 P95 | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| PairPos1 纯定位，冷 | 136.500 | 144.625 | 2,348,688 | 14 | 3,326 |
| PairPos1 纯定位，热 | 136.042 | 144.208 | 2,347,682 | 0 | 3,250 |
| PairPos1 前置扩张，冷 | 440.042 | 452.708 | 5,049,329 | 18,450 | 3,437 |
| PairPos1 前置扩张，热 | 438.583 | 450.750 | 5,048,210 | 18,432 | 3,334 |
| PairPos2 纯定位，冷 | 142.625 | 151.625 | 2,626,932 | 14 | 3,350 |
| PairPos2 纯定位，热 | 141.292 | 150.708 | 2,624,354 | 0 | 3,274 |
| PairPos2 前置扩张，冷 | 408.167 | 416.416 | 5,604,053 | 18,450 | 3,461 |
| PairPos2 前置扩张，热 | 395.125 | 406.583 | 5,601,362 | 18,432 | 3,358 |

纯定位输出 3,072 glyph，前置扩张后输出 6,144 glyph。
冷 allocation 的十四项来自完整 lookup 缓存，定位本身不增长存储。
前置 GSUB 的保守输出为 9,216 glyph，两份数组的新容量共 18,432 slots；
冷路径另有十八项完整 GSUB／GPOS 缓存存储。
备用容量 16,384 覆盖前置扩张，所有热路径 allocation 为零，
冷路径只收十四／十八项缓存存储。
备用容量的已初始化数组仍参与既有完整遍历，费用没有按逻辑 glyph 数缩减。
末尾拒绝的调用次数、allocation 与完整路径一致，
work 少最后一次 glyph 数费用，拒绝状态和输出长度逐样本校验。

原始版本与生产版另完成三轮交替顺序、48,000 个普通入口对照样本。
下表给出三轮 P95 范围，单位为微秒；计时排除成功结果释放。

| 路径 | 原始版 P95 范围 | 生产版 P95 范围 |
|---|---:|---:|
| PairPos1 纯定位，冷 | 139.333–150.750 | 141.083–155.792 |
| PairPos1 纯定位，热 | 138.667–149.500 | 138.083–138.625 |
| PairPos1 前置扩张，冷 | 442.458–444.375 | 435.625–443.625 |
| PairPos1 前置扩张，热 | 440.250–442.916 | 430.708–441.917 |
| PairPos2 纯定位，冷 | 136.750–144.416 | 137.250–150.625 |
| PairPos2 纯定位，热 | 135.291–140.459 | 137.667–138.500 |
| PairPos2 前置扩张，冷 | 400.542–404.041 | 397.459–407.500 |
| PairPos2 前置扩张，热 | 391.500–399.042 | 398.167–414.125 |

系统负载未受控，部分普通入口 P95 上升，部分下降，
不能从这些样本宣称稳定提速或零开销，也不能与别片直接排速度名次。
本片完成支持路径的费用与拒绝验证，尚未改变 Core 的默认宽度复用行为。

证据继续保存在
[B 切片证据目录](../evidence/text-stages-b-2026-11-02/README.md)。
最终收据分别为 `gpos-pair-verification.json`、`gpos-pair-cost-summary.json`、
`gpos-pair-ordinary-cost-summary.json` 与 `gpos-pair-gates.json`。
测试模块配置与成本结果处理修正前的收据保留在相应子目录，
未把当时失败的严格门禁记为通过。
这些定向证据不代替 B2–B4、C 的 Core 全输出等价与 D 的默认启用门禁。

报告按技术结构与好好说话做自审，先核对原始位、扫描顺序、失败行为与证据范围，
再检查措辞和节奏。四项中文正则扫描未发现匹配；这不代表独立评审通过。
