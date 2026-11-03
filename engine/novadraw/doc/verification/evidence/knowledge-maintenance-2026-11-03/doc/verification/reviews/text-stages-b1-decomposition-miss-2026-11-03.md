# B1：分解后第二字符缺字的准入

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：本子片实现、Native／portable 测试、完整输出对照、
成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

本片接续[空格与不换行连字符回退](text-stages-b1-space-fallback-2026-11-02.md)。
前片遇到 canonical 分解的第二字符非零时，一律返回未支持。
普通内核在第二字符缺字时会立即终止这次分解，输出仍可保持一个字形。
本片补齐这一有限路径的证明与费用，仍不放行实际组合字符输出。

## 按普通内核的查询顺序检查

`ot_shape_normalize.rs::decompose` 先查询 canonical 分解，再查询第一个
字符 a 的字形，随后查询第二字符 b 的字形。
当 b 非零且没有字形时，函数立即返回零；它不会沿 a 递归，
也不会输出 a 或 b。a 是否有字形不改变这个失败分支。

预检现在按相同顺序分别准入 a 和 b 的字体查询。
b 有字形时，入口在递归、输出及 Recovery 前返回 `Ok(None)`；
b 缺字时，记录这一终止分支并回到原始输入字符的回退证明。
普通入口和准入入口仍调用同一个 normalizer，预检不生成替换字形。
任何准入回调拒绝都立即返回原始错误，不转入普通入口重试。

本片不对终止分支中的 a 调用 `simple_scalar`。
例如 U+01FB `ǻ` 的分解是 `å` 加 U+0301；
第二字符缺字时，普通内核仅查询 `å` 的字体映射，没有输出或初始化它。
给这个未执行的 Unicode 属性操作收费，会改变准入轨迹并掩盖实际访问依据。

沿单字符链遇到已映射中间字符时，既有 shortest 规则仍立即停止。
例如 `Å` 分解到 `Å`：若 `Å` 已映射，输出它；
若没有，再检查 `Å` 的 `A` 加 U+030A 分解。
第二字符缺字时，分解失败后仍根据原始 `Å` 判断回退，
不能把链中间的 `Å` 当作原始输入。

## 输出长度与费用

原始输入仍须通过无 mark、无特殊 continuation 的 singleton 证明。
新增终止分支没有输出分解字符，长度保持原始 n；
已有 GSUB 增长、GDEF、cluster、方向及定位合同继续适用。
第二字符实际有字形时可能输出组合字符，仍需后续独立补证。

令 t 为预检确认的第二字符缺字终止次数，
m 为所选 cmap 每次 nominal 查询的访问上界。
前片每个分解查询的 Mapping 费用已经覆盖 a；
本片只为独立的 b 查询及结果判断增加以下费用：

| 阶段／操作 | 新增 work | 依据 |
|---|---:|---|
| 预检／Mapping | `2 × m`，每次遇到非零 b | 按顺序分别查询 a 和 b；b 命中也先付费 |
| 预检／Planning | `2`，每次遇到非零 b | 第二查询结果与终止分支 |
| Substitute／Mapping | `t × m` | 普通内核独立查询 b，a 已由原分解查询界覆盖 |
| Substitute／Planning | `2 × t` | b 的 Option 结果与返回分支 |
| allocation | `0` | 此分支没有输出或集合增长 |

分解表、普通内核的重复查询、原始字符回退和复制费用仍按前片合同收取。
计数使用 checked 算术；work 计元素与重复访问，
allocation 计整份新增容量 slots。准入并非事后统计。

## 验证输入与失败恢复

新 fixture 复用冻结的八组 GSUB／GPOS 内核及两组 GDEF，
只建立独立的 cmap。三个 bit 分别控制：
a（`A`、`e`、`å`）存在、b（U+0301、U+030A）存在、
中间字符 `Å` 存在，合计八组映射。
输入包含 `é`、`Å`、`Å`、`ǻ` 和此前 singleton／空格回退。
原 fixture 与普通 oracle 保持原字节。

新普通矩阵每配置 98,304 份：
8 内核 × 2 GDEF × 8 mapping × 2 重复轮次 × 2 script × 2 方向
× 4 输入 × 3 cluster 模式 × 4 cluster level × 2 容量。
消费者另覆盖 Han script，并比较 infos 和 positions 的全部字段。
b 存在的输入仍须在 Recovery 前返回未支持并保留原输入。

逐点拒绝测试覆盖直接终止与单字符链后终止、
LTR／RTL、冷／热缓存、已映射及未映射 a。
测试核对原始 Error source、完整费用前缀和下一次普通调用。
进入 pipeline 前拒绝保留输入；进入后拒绝清空逻辑长度并保留容量。

独立费用测试比较 `xéy`、`xǻy` 与 `x\u{0378}y`。
在相同字体和预热状态下，两个终止输入各多收三次 nominal 查询上界：
预检 a、预检 b、普通执行 b；普通执行 a 的保守界两侧原本均已收取。
Unicode work 相同，未输出的非 ASCII a 不增加属性初始化费用。
既有容量下 allocation 为零。

原消费者把所有非零 b 当作未知，相关负例现改为实际 mark 或 variation
selector 输入。新增测试独立覆盖旧 `é`／`Å` 缺字输入的普通输出；
迁移不修改冻结的输入 fixture 或普通 oracle。

## 当前验证状态

Native／portable 各 205 项依赖内部测试与 102 项消费者测试通过。
新增五份 Rust 模块格式与四份 Python 语法检查通过。
二十七份补丁精确重放，恢复全部生产源码；本片只修改
`src/hb/shape_admission.rs`，其余 80 份源码保持原字节。
增量补丁 SHA-256：
`0138d545fc8135ce78d93a22e6b5b0eb010fb6aee1af235b82fa92beaf38116a`。

二十矩阵四种配置共 6,338,496 份完整观察一致。
新增矩阵每配置 98,304 份，四份签名均为
`adfc326e0a8021d4c8eb175e74db84ffd43e8d3368e2c7b50db32f86c95c8292`。
旧十九矩阵的生产版本及新矩阵四种配置已重跑；
旧十九矩阵的原始版本共 38 份配置，仅在解压后的全部字节、
签名和分母核对后复用。
额外 invisible 普通参考保留前片的冻结收据，并重新核对源与输出身份。
复用范围与新执行范围分别记录。初稿将新矩阵分母多算一倍；
脚本始终按上述维度计算并核对实际 98,304 份，本次已修正正文。
两组成本收据已通过分母、样本身份、稳定费用及原始数据散列核对。
十三项定向门禁通过：依赖库 Native／portable Clippy 与冻结前片
六项既有诊断一致，新增诊断为零；消费者和两个成本程序 Clippy 通过，
Core Native／Wasm check 与消费者 Wasm build 通过。
Wasm 只构建，未执行测试。

成本输入为 `ABéÅÅǻCDE` 重复 128 次，共 1,152 个字符。
无 GPOS、PairPos 2 和 Format 3 分别组合 ClassDef 1／2，
覆盖 LTR／RTL、冷／热缓存与 0／16,384 备用容量。
四种模式逐样本轮换，每组采 1,000 条及 100 次预热，
共 192,000 条。原始／生产普通入口另采三个会话，交替版本顺序，
共 144,000 条。
系统负载未控制，这些结果不替代 Core 完整排版、GPU 或 16 ms／4 ms 验收。

## 成本与延迟

第一组计时包含 shaping、成功结果释放或拒绝后的内部清理；
输入准备、缓存预热和保留 buffer 的最终释放不在计时内。
下表是各模式 48 组输入的 P50、P95 范围，单位为微秒。

| 模式 | P50 范围 | P95 范围 |
|---|---:|---:|
| 普通入口 | 49.709–125.625 | 58.583–162.375 |
| 准入／空回调 | 85.958–162.750 | 101.208–201.542 |
| 准入／计数回调 | 87.917–166.250 | 101.208–211.167 |
| 最后一点拒绝 | 86.041–160.917 | 98.083–195.291 |

准入需要预检、计费和回调，因此本轮耗时高于普通入口。
这些数据没有证明 Core 排版提速，也不说明未来宽度缓存命中的耗时。
最后一点拒绝停止于结果转移前：各组与成功计数模式的 calls 相同，
少付恰好最终 glyph 数的 work；allocation、Mapping 和 Unicode
费用相同。脚本逐组验证这一关系。

以下是 ClassDef 2、LTR、热缓存和 16,384 备用容量的费用。
work 按元素访问计，allocation 按整份新增容量 slots 计。

| 内核 | 最终 glyph | work | allocation | calls |
|---|---:|---:|---:|---:|
| 无 GPOS | 1,536 | 625,847 | 0 | 8,987 |
| PairPos 2 | 1,536 | 1,864,934 | 0 | 9,137 |
| Format 3 | 1,152 | 4,057,298 | 22 | 9,220 |

三组 Mapping work 均为 85,266，Unicode work 均为 17,024。
Format 3 的 22 slots 延续前片的两个匹配数组及 lookup 缓存分配；
普通预热未执行所有嵌套 leaf，不能将“热”理解成所有缓存已初始化。
同一 Format 3 输入无备用容量时 work 为 3,935,442；
已有备用容量仍计入全初始化数组的 digest、metrics 和位置访问。
分解终止本身没有增长，这些分配来自其他已支持阶段。

普通入口对照共 72 组匹配输入和会话。原始版本 P95 范围为
53.666–219.417 微秒，生产版本为 52.416–135.084 微秒。
逐组生产／原始 P95 比值为 0.386–1.219，既有下降也有上升；
负载未控制，不能将范围端点或个别会话用于证明稳定提速。
例如 Format 3、ClassDef 2、LTR、热缓存的三个会话分别为：

| 会话 | 原始 P95／微秒 | 生产 P95／微秒 |
|---|---:|---:|
| 0 | 78.541 | 58.708 |
| 1 | 60.250 | 58.708 |
| 2 | 59.375 | 58.709 |

该组计时仅包含普通 shaping 和所有权转移，成功结果释放在计时外。
两组成本实验的计时边界不同，不能直接计算它们之间的比例。

成稿经过技术结构与中文表达自审；正文按共享内核顺序说明终止行为，
将完整输出、拒绝前缀和延迟分别列证。没有独立读者复核。

## 可复验入口

```bash
python3 -B tools/scripts/check_harfrust_decomposition_miss_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_decomposition_miss_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_decomposition_miss_ordinary_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_decomposition_miss_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

收据为 `decomposition-miss-verification.json`、
`decomposition-miss-cost-summary.json`、
`decomposition-miss-ordinary-cost-summary.json` 与 `decomposition-miss-gates.json`，
保存在 [B 切片目录](../evidence/text-stages-b-2026-11-02/README.md)。
前 26 片脚本与补丁保留为历史快照，不能对后续生产源码重新覆盖冻结补丁。
完整规范化、复杂 shaping、B2–B4、C／D 仍待完成。
