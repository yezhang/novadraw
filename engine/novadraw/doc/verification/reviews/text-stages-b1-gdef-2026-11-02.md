# B1：GDEF 字形分类与属性查询准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、完整输出、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

本片接续[简单 Unicode](text-stages-b1-simple-unicode-2026-11-02.md)，
允许已有的简单字符塑形路径查询 GDEF glyph class 与 mark attachment class。
这些分类来自字体；Unicode 字母也可以被字体标为 mark。
非零 lookup flags、组合字符规范化、复杂脚本和其他 GPOS 算法仍在执行前返回未支持。

## 查询费用在执行前确定

`OtTables::glyph_properties_work` 借用已经配置的 ClassDef，
不会查询字形或预热属性缓存。调用方先准入借用 header 的访问，
再按实际 reader 确定单次 cache miss 的工作上界。
GDEF 查询不分配集合，`allocation` 为零。

Format 1 覆盖 start glyph、数组 header 与索引值读取。
Format 2 按实际 range 数覆盖二分比较及选中项的 start／end／class。
已核对[Rust 1.96 源码](https://doc.rust-lang.org/1.96.0/src/core/slice/mod.rs.html)，
`binary_search_by` 在非空数组上比较
`ceil(log2(n)) + 1` 次，采用整数安全上界 `bit_length(n) + 1`；
空数组不比较。损坏或未排序数据保留普通 reader 的行为，
不假定表排序正确，也不在准入路径另造分类结果。

每次查询同时覆盖固定缓存的读取和可能写入，以及标量分支。
glyph classes 存在时，class 3 的 miss 还可能查询 mark attachment classes；
glyph classes 缺失时不会进入该分支。
缓存命中仍收同一 miss 上界，不因先前预检或普通调用预热而免除费用。

## 初始字形与替换输出分别计费

初始分类按逻辑输入字形数准入。GSUB 直接替换按当前 lookup 的最大输出数，
为每个可能发布的字形再次准入查询；多字形替换按 sequence 长度覆盖增长。
Format 3 的每个叶子 record 在每个外层位置至多替换一次，
重复位置和重复 record 分别覆盖。查询费先于共享 GSUB 内核执行。

原始普通入口和准入入口继续使用同一分类、替换与定位内核。
拒绝保留原始错误 source 和精确费用前缀，预检拒绝保留输入；
进入 pipeline 后拒绝清空输入并保留容量，随后仍可普通塑形。
不会因预算拒绝调用普通 fallback。

## GDEF mark 不改变 Unicode 组合字符边界

初始 `lig_props` 为零，各字形的组件贡献为一。
Single 保留该值；Multiple 保留已有 ligature base，或写入 id 为零、
`IS_LIG_BASE` 未设置的 component。只有 ligating 能增加组件数，
存储仍使用四位，沿用既有预检对 `u8` 求和的上界。
Single 改变 GDEF 分类后，最多重新启用此前保存的组件上界。

零 lookup flags 不跳过 mark，简单字符也不是 default-ignorable。
因此匹配位置连续，连字不会为匹配位置之间的字形生成带非零 id 的 mark attachment。
已有尾部扫描与重复 cluster 合并费用继续保留。
普通字母被 GDEF 标为 mark，不等于支持 Unicode 组合字符或 MarkPos。

定位阶段覆盖真实 mark 的两个 advance 清零。
若 plan 允许且方向 forward，另覆盖 offset／advance 的四次读取和两个 offset 写入。
fallback positioning 仍按 Unicode mark 属性划组；本片简单字符保持 singleton，
不因 GDEF class 3 进入组合标记定位。

## 验证范围

新增 fixture 覆盖空 ClassDef、无 glyph classes 但有 attachment reader、
非法 reader、两种 ClassDef format、base／ligature／mark／component／零分类、
19 个 range 的二分与未排序数据。它们与无 GPOS、SinglePos、PairPos、
SingleSubst、MultipleSubst、LigatureSubst 和 Format 3 组合。

消费者比较全部 glyph info 与 position 字段，覆盖冷热、LTR／RTL、
Latin／Han／未指定 script、四种 cluster level、顺序／等值／倒序 cluster 和备用容量。
另逐准入点拒绝并验证 source、费用前缀和普通恢复。
成本输入为 `ABC` 重复 512 次，1,536 个字符；
ClassDef 1／2 均实际将字形标为 mark，分别测两种方向、冷热与备用容量。

初次拒绝恢复测试把“热缓存”误当成整个 shaping 都不分配。
Format 3 每次调用仍新建匹配数组，可能 spill；
已把断言收窄为 GDEF 查询本身不分配。
失败日志保存在 `gdef-before-hot-allocation-assertion-fix.log`，不作为通过证据。
首次完整矩阵采集后，内部测试编译发现三个测试策略构造漏填新增字段。
已补齐测试构造，并将内部测试移到矩阵采集前；普通内核不作调整。
该次编译日志保存在 `gdef-before-private-test-constructor-fix.log`，不作为通过证据。
重跑后，旧 SinglePos 负例仍把 GDEF 表存在视为未支持。
该负例现改为带 GDEF 的非零 mark attachment lookup flag；
普通 oracle 和 fixture 保持冻结。失败日志保存在
`gdef-before-legacy-gdef-negative-fix.log`，不作为通过证据。

更新旧负例只改变消费者测试，生产补丁与普通塑形逻辑保持。
复验先确认当前增量补丁字节与上一轮一致，再解压校验此前十五矩阵的
输出签名与分母；新增 GDEF 矩阵重新执行。
最终收据明确记录冻结矩阵复用与解压校验。
默认复验命令仍会重新执行全部矩阵；`--reuse-frozen-oracles`
只允许复用生产补丁未变、内容与冻结签名一致的旧矩阵。

## 已通过的完整输出与成本

二十三份补丁精确重放，恢复全部生产源码。本片只修改
`src/hb/ot/mod.rs` 与 `src/hb/shape_admission.rs`，其余 79 份源码保持字节。
增量补丁 SHA-256：
`a39a5b74037ecfdc4df3f107d7649132b835beda271d9ee1b9a94e5c00213303`。

十六矩阵在原始／生产、Native／portable 四种配置下，
共 4,274,112 份完整观察一致。新增 GDEF 每配置 110,592 份，
四份签名均为
`6100693ddea574b08e7a4d40e8f1bd7568a024ad47e160c9b45373b0957485f6`。
此前十五矩阵已在本轮执行并对齐，旧负例修正后按冻结签名复用；
最终收据记录该复用方式。
Native／portable 各 203 项依赖内部测试与 86 项消费者测试通过。

准入成本共 192,000 条，192 组各 1,000 条，预热各 100 次。
输入、字体、计划和热缓存准备在计时外；计时包括塑形、
成功结果释放或拒绝清理。拒绝后保留容量的 buffer 最终释放在计时外。
原始／生产普通入口对照另使用同口径计时，成功结果释放在计时外。

下表为 ClassDef 2、RTL、热缓存、零备用容量的 P95，单位为微秒。
每格来自同条件的 1,000 条样本，四种模式逐样本轮换。

| 塑形路径 | 普通 | 无操作回调 | 计数回调 | 最后准入点拒绝 |
|---|---:|---:|---:|---:|
| 无 GPOS，前置多字形替换 | 191.125 | 200.083 | 203.125 | 194.583 |
| PairPos 2，前置多字形替换 | 308.125 | 318.750 | 320.916 | 312.084 |
| Format 3，嵌套单字形替换 | 130.334 | 126.750 | 128.750 | 125.709 |

对应计数回调的 work 为 564,133／3,060,820／5,053,397，
allocation 为 9,216／9,216／18 个完整新增容量 slots。
前两组按每个 lookup 的最大输出上界为双数组预留，
实际输出各 3,072 个字形。Format 3 输出 1,536 个字形；
18 个 slots 属于两份 9 位置匹配数组，GDEF 查询本身不分配。

三组的 GDEF query work 分别为 172,032／172,032／731,136，
冷热缓存均按 miss 上界计费。Format 3 为每个可尝试的 subtable 和 record
覆盖重复访问，费用含保守上界，不等于实际 cache miss 次数。
该组备用容量为 16,384 时，query work 和匹配分配保持，
总 work 增为 5,172,181，因为内核仍遍历完整初始化数组。
最后准入点拒绝保留全部已准入容量和 GDEF 费用，
少收最后 3,072／3,072／1,536 个字形访问，精确前缀检查通过。

本机未控制系统负载，个别无操作回调样本 P95 低于普通入口。
不能据此判断准入带来提速；这批数据不代表 Core 布局、GPU 或
16 ms／4 ms 验收，宽度复用仍未启用。

普通原始／生产对照共 144,000 条：24 组输入、两个版本、三个会话，
每组每会话各 1,000 条，预热各 100 次；版本执行顺序交替。
同条件的三个会话 P95 范围如下，单位为微秒；
端点来自各会话的 P95，不合并会话重新计算分位数。

| 塑形路径 | 原始版本 | 生产版本 |
|---|---:|---:|
| 无 GPOS，前置多字形替换 | 188.458–189.833 | 186.667–192.125 |
| PairPos 2，前置多字形替换 | 310.667–316.833 | 301.708–308.083 |
| Format 3，嵌套单字形替换 | 126.084–127.375 | 122.208–131.833 |

全部冷／热、方向、ClassDef 与容量分组的分位数、最大值和费用
保存在两份成本收据。普通内核对照存在会话波动，不据此推导整体提速。

## 已通过的定向门禁

库 Native／portable Clippy 与冻结前片的六项既有诊断一致，
新增诊断为零。消费者及两份成本 binary 的 Clippy 通过 `-D warnings`。
Core Native／Wasm `--lib` check 与 portable 消费者 Wasm build 通过；
Wasm 未执行测试。workspace fmt、新增模块格式与四份 Python 语法检查通过。
文档门禁通过 521 份 Markdown 与当前 xtask 引用检查。

## 可复验入口

```bash
python3 -B tools/scripts/check_harfrust_gdef_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_gdef_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind gdef \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_gdef_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

最终收据为 `gdef-verification.json`、`gdef-cost-summary.json`、
`gdef-ordinary-cost-summary.json` 与 `gdef-gates.json`。
证据保存在[B 切片目录](../evidence/text-stages-b-2026-11-02/README.md)。
本片通过不关闭 B1。完整规范化、复杂 shaping、B2–B4、C／D
仍待完成，生产宽度复用缓存继续默认关闭。

成稿再次按好好说话与技术文档清晰度复核，保留查询依据、预算单位、
失败后的状态、冻结输出复用方式、计时口径和未关闭范围。
编号标题、对比句式、空泛判断和中文半角标点的正则命中均为零。
节奏扫描标出 17 处，复核后保留技术标识符、字段清单和明确的判断。
本次为作者自审，没有独立评审结论。
