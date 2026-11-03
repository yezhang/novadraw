# B1：已映射 Unicode 字母与空格的塑形准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：本片实现、完整输出对照、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续 [ASCII RTL](text-stages-b1-rtl-2026-11-02.md)，
允许已映射、非 default-ignorable 的 Ll／Lu／Lt／Lo／Zs 字符进入准入路径，
并加入默认 shaper 的 Han script。默认坐标、全局 feature、
既有 GSUB／GPOS 算法限制、零 lookup flags 与无 GDEF／AAT 条件保持。
这只覆盖可证明的简单字符路径，完整规范化与复杂脚本仍待后续切片。

## 已通过的结果一致性验证

二十二份补丁可从原始 Harfrust 0.3.2 归档依次重放，精确恢复生产源码。
本片只修改 `src/hb/shape_admission.rs` 与 `src/hb/charmap.rs`，
其余 79 个源码文件保持上一片的字节。
增量补丁 SHA-256：
`c42e4ff83f587c2ee8d88d85d6369f37ed059d3e961c97e73e11892fb921bd76`。

十五份普通 oracle 在原始／生产、Native／portable 四种配置下，
共 3,831,744 份完整输出观察一致。新增 Unicode oracle 每配置
552,960 份，四种配置的输出签名均为
`76d5c25bcd053823cf5863111e9c351b1c8555a31d314162f10031218a7754f1`。
此前十四份 oracle、fixture 与输出签名保持冻结。
Native／portable 各 203 项依赖内部测试与 81 项消费者测试通过。
这些观察验证塑形结果和拒绝恢复；完整 Core 布局等价仍属于 C。

## 在预检中确认简单路径

普通内核初始化非 ASCII 字符时，先读取 general category，
再判断 default-ignorable。Ll／Lu／Lt／Lo／Zs 在类别分支直接返回，
不会创建 continuation；组合标记、RI、emoji modifier、ZWJ、
半角假名声调、tag 与 fraction slash 有其他分支，本片仍提前返回未支持。

预检为每个非 ASCII 字符先收费 general category 的四份生成表访问，
再收费类别判断和实际 default-ignorable 检查。执行阶段的同样检查再次收费。
U+115F／U+1160／U+3164／U+FFA0 按锁定内核的 spacing-glyph 例外处理；
消费者直接比较普通结果，不另造 Unicode 属性。

简单字符的 nominal mapping 必须存在。normalizer 在整个输入上短路，
不调用分解、reorder 或 recompose，也不增长规范化存储。
RTL 镜像仍在预检中收费并检查镜像类别，再执行受准入的映射查询。
镜像缺字保留已映射原字符；原字符缺字提前返回未支持。

## 字体映射覆盖非 ASCII 查询

新增通用映射上界复用已核对的 format 0／4／6／10／12／13 reader。
这些锁定 reader 借用现有字体表，不分配查询集合。
format 4／12／13 的搜索上界来自实际段或 group 数；
不支持的 reader 在查询前返回未支持。

MacRoman 转换最多访问固定表的 128 项，查询前覆盖完整扫描。
Symbol fallback 至多再次映射一次，两次都覆盖 cache 与 reader 访问；
两种属性同时存在时，也覆盖第二次转换。
原有普通映射内核保持，包括 MacRoman 对 codepoint 的原始转换语义。
预检、RTL 镜像与 normalizer 的重复查询各自付费，缓存命中不免除费用。
`allocation` 仍表示完整新增容量 slots；这两个映射分支本身不增加容量。

## 消费者与冻结输出

新增消费者覆盖 Latin／Han／Common／unknown／未指定 script，
LTR／RTL、四种公开 cluster level、顺序／等值／倒序 cluster、
备用容量、前置替换和 SinglePos／PairPos。
format 12 显式映射 Latin 扩展、汉字、补充平面 Han 与 Zs，
并映射组合标记等负例，避免把类别拒绝误记为缺字拒绝。
另逐个核对六种 reader、MacRoman 与 Symbol 的非 ASCII 查询。

冷热路径在每个准入点拒绝，检查原始错误 source、精确费用前缀、
预检保留输入和 pipeline 恢复空 buffer。后续普通调用与新 buffer 的结果对照；
前置 GSUB 会扩张映射到 glyph 66 的 `é`，不能用字符数断言 glyph 数。
粘滞预算测试确认 Unicode 检查拒绝先于未支持返回，
随后另一请求也不能绕过调用方已失败的预算。

旧 shaping 负例此前把显式 Latin/default shaper 下的非 ASCII 字母一律视为未支持。
本片改用含组合标记的输入维持负例意图。既有十四份普通 oracle、
fixture 和签名保持；新增独立 oracle 覆盖四种 cluster level。
第 21 片的三种 cluster level 证据范围不因此扩大。

旧负例与 `nexté` 的 glyph 数断言修正前，失败日志保存在
`simple-unicode-before-negative-test-update.log` 与
`simple-unicode-before-next-glyph-expectation-fix.log`。
这两份日志不作为通过证据，最终测试收据保存在
`simple-unicode-verification.json`。

## 成本采样使用相同的输入

准入与原始版本普通对照使用同一批输入，每请求 3,072 个字符。
format 12 使用 `BCé中中(` 重复 512 次；MacRoman／Symbol 使用
`BCé(` 重复 768 次。前置 GSUB 将 glyph 66 扩成两个、67 扩成三个，
format 12 的 `é` 也映射到 66，另两种映射的 `é` 保留各自 nominal glyph。
因此前置替换后分别为 5,120／5,376 个 glyph；
三种 cmap 都实际经过扩张路径。

准入采样区分普通、无操作回调、计数回调和最后准入点拒绝，
并分别测冷／热 cache、两种 script 状态与两种备用容量。
输入、字体数据、计划和热 cache 准备在计时之外；
准入计时包含结果释放或拒绝清理，普通原版对照的成功结果释放在计时之外。
两种计时口径分别报告，系统负载未控制，不据此推导 Core 整体延迟。

首次采样的预期 glyph 数误用了另一组输入，另两种 cmap 也未触发扩张。
该批 192,000 条数据保存在 `simple-unicode-before-cost-input-fix.ndjson`，
不作为最终成本证据；修正后重新采样。

修正后的准入采样共 192,000 条，192 组各 1,000 条，预热各 100 次。
普通原版对照共 144,000 条：24 组输入、两个版本、三个会话，
每组每会话各 1,000 条，预热各 100 次，版本执行顺序交替。
两批样本均通过身份、分母、输出及收据检查。

下表为 Han、RTL、前置替换、热 cache、零备用容量的 P95，
单位为微秒。每格来自该模式的 1,000 条样本，四种模式逐样本轮换。

| 字体映射 | 普通 | 无操作回调 | 计数回调 | 最后准入点拒绝 |
|---|---:|---:|---:|---:|
| format 12 | 339.375 | 372.375 | 376.000 | 365.958 |
| MacRoman | 283.666 | 321.167 | 325.917 | 315.834 |
| Symbol | 283.167 | 324.459 | 327.583 | 318.416 |

同条件下，计数回调的 work 为 4,954,706／6,330,194／5,162,066，
allocation 均为 18,432 个完整新增容量 slots，分别覆盖两个数组。
备用容量为 16,384 时，三组 allocation 均为零；
work 增为 5,018,194／6,393,682／5,225,554，
因为普通内核仍访问完整初始化数组，不能只按逻辑 glyph 数计费。
最后准入点拒绝保留精确费用前缀，与成功路径相比少收最后的
5,120／5,376／5,376 个 glyph 访问；调用数和已准入容量一致。

普通原版对照另使用相同计时口径。Han、前置替换、热 cache 的三个会话
P95 范围如下，单位仍为微秒；范围端点来自各会话的 P95，不重新合并样本。

| 字体映射 | 原始版本 | 生产版本 |
|---|---:|---:|
| format 12 | 332.459–335.250 | 333.167–336.958 |
| MacRoman | 278.459–287.625 | 278.458–282.458 |
| Symbol | 278.250–284.292 | 278.458–287.000 |

新增检查与回调有可见开销；普通版本对照在本机呈现会话波动。
全部冷／热、script、替换与容量分组的 P50／P95／P99、最大值和费用
保存在两份 summary 收据。本轮没有控制系统负载，也没有启用 Core
宽度复用，不能将这些塑形数据当作完整布局、GPU 或 16 ms／4 ms 验收。

## 已通过的定向门禁

库 Native／portable Clippy 与前二十一片的六项既有诊断一致，
新增诊断为零。消费者的 Native／portable Clippy，以及两份成本 binary
的 Clippy 均通过 `-D warnings`。
Core Native／Wasm `--lib` check 与 portable 消费者 Wasm build 通过；
Wasm 未执行测试。workspace fmt 与四份 Python 脚本语法检查通过。
文档门禁通过 520 份 Markdown 与当前 xtask 引用检查。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_simple_unicode_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_simple_unicode_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind simple-unicode \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_simple_unicode_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

最终结果以 `simple-unicode-verification.json`、`simple-unicode-cost-summary.json`、
`simple-unicode-ordinary-cost-summary.json` 与 `simple-unicode-gates.json` 为准，
保存在 [B 切片证据目录](../evidence/text-stages-b-2026-11-02/README.md)。
本片通过不关闭 B1。GDEF 属性查询、完整规范化与复杂 shaping，
以及 B2–B4、C／D 仍待完成，生产宽度复用缓存继续默认关闭。

成稿再次按好好说话与技术文档清晰度检查：保留来源、预算单位、
两种计时口径、分母、拒绝后的状态和未关闭范围；结果与限制分别说明。
编号标题、对比句式、空泛判断和中文半角标点的正则命中均为零。
节奏检查标出 20 段，复核后保留技术标识符、类别清单和表格，
不因字符计数拆散其含义。
本次为作者自审，不代表取得独立评审结论。
