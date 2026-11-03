# B1：已映射一般字符的属性准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、完整输出、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

本片接续 [GDEF 属性准入](text-stages-b1-gdef-2026-11-02.md)，
补齐已经映射的标点、数字、符号，以及不带组合修饰的单独 emoji。
它们仍使用普通塑形内核；新代码只证明允许经过的算法分支并在操作前准入成本。
缺字分解、组合标记、特殊 continuation 和其他 GPOS／脚本路径仍待后续补齐。

## 一般字符为什么仍可按单字形组处理

`ot_shape.rs::set_unicode_props` 先初始化 Unicode 属性，
再让 ASCII 与 Ll／Lu／Lt／Lo／Zs 提前返回。
其他类别还要检查 emoji modifier、区域旗帜、ZWJ、半宽浊音标记、
tag 字符与分数斜线。这些判断决定是否设置 continuation 或 fraction 状态，
不能因为字符已经映射就省略。

准入预检先支付 general category 查询的四次表访问，
再检查类别和具体 codepoint。Unicode mark 在执行前返回未支持；
U+1F1E6–U+1F1FF、U+1F3FB–U+1F3FF、U+FF9E–U+FF9F
和 U+2044 同样返回未支持。default-ignorable 查询另收一次表访问，
拒绝 ZWJ、tag 与其他不可见修饰。字体映射查询仍独立准入；
原始字符缺字不进入本片的规范化路径。

因此，获准字符不会设置 continuation，方向处理和 mark fallback 仍按单字形组执行。
这不要求 cluster 身份互异；GSUB 仍可生成共享同一 cluster 的多个字形。
单独 emoji、Modifier Letter、控制字符、行段分隔符和已映射的未分配字符，
只要满足上述条件，也沿用 singleton 证明，不按字体名或字符集合建立白名单。
RTL 另检查镜像目标的属性和映射；镜像缺字时保留原始映射并沿用普通 `rtlm`。

## 新增访问单独计费

预检中，非提前返回类别多收两次 Planning 访问，
覆盖 mark 判断与具体 codepoint 分支。
执行完整 Unicode 初始化前，每个这种原始字符另收五次 Information 访问，
覆盖 modifier、RI、半宽／tag、fraction 的 codepoint 读取与存储的 ZWJ 位。
另收八次 Planning 访问，覆盖分支和 split header 的标量操作。
这些费用没有集合分配，也不重复计算已收取的 Unicode 表查询。

计数只记录原始输入中会走特殊分支的字符。
预检镜像目标仍支付实际检查费用，但不把目标再计入原始初始化次数。
镜像替换发生在初始化之后，普通内核保留原始 Unicode 属性。
后续 GSUB 输出、GDEF 查询、定位和完整容量访问沿用既有准入。

## 失败与恢复

消费者逐个准入点拒绝，检查原始 Error source、精确费用前缀和停止位置。
预检拒绝保留输入；进入 pipeline 后拒绝清空输入、保留容量，
随后仍可普通塑形。预算拒绝不调用普通 fallback。
缺失原始字形、组合序列和特殊字符在 Recovery 前返回未支持。

首次费用隔离测试使用的两个字符被 fixture 映射到不同字形，
GSUB 输出长度也不同，导致访问量差值出现无符号减法溢出。
现改为两个都映射到 glyph 67 的字符，并先预热 lazy lookup 存储。
第二次测试发现首次调用的 lookup 分配也混入了差值，预热已排除此差异。
生产补丁未因这些测试修正改变。失败日志为
`general-scalar-before-fee-fixture-isolation-fix.log` 与
`general-scalar-before-fee-cache-isolation-fix.log`，不作为通过证据。

## 验证范围

新 fixture 复用已冻结的 GSUB、GPOS、GDEF 构造，只替换 cmap。
覆盖中文与全宽标点、多种数字、数学和货币符号、emoji、
Modifier Letter、控制／分隔符和已映射未分配字符。
普通输出矩阵还包含被准入入口拒绝的组合序列。

新消费者检查八组替换／定位内核、四组 GDEF、
Latin／Han／未指定 script、LTR／RTL、四种 cluster level、
顺序／等值／倒序 cluster 与备用容量。
缺失镜像目标另有完整 glyph info 与 position 对照。
费用隔离测试使用相同目标字形，确认 Unicode 表查询和分配保持，
Information 增加五次，Planning 增加十次。

矩阵工具最初把新矩阵分母多算一倍。实际组合乘积为每配置 73,728 份，
已修正校验值，生产与 fixture 保持原字节。错误日志为
`general-scalar-before-matrix-denominator-fix.log`。
续验只复用已完成且解压后与前片签名、分母一致的旧矩阵；
同时要求当前生产增量补丁完全一致，新增矩阵始终重新执行。
最终收据列出复用的配置，默认命令仍完整执行全部矩阵。

正式成本输入为 `A，🙂≤１２３∉€。！？` 重复 128 次，共 1,536 个字符。
无 GPOS、PairPos 2 和 Format 3 分别组合 ClassDef 1／2，
覆盖两种方向、冷热和 0／16,384 备用容量。
四种模式逐样本轮换：普通、无操作回调、计数回调和最后准入点拒绝。
输入、字体、计划与热缓存准备在计时外；
准入计时包含成功结果释放或拒绝清理，保留容量的 buffer 最终释放在计时外。
原始／生产普通入口另采三会话，成功结果释放在计时外。

本机没有控制系统负载；这些样本衡量 Harfrust 路径及准入成本，
不能推导 Core 布局、GPU 或 16 ms／4 ms 验收。

## 已通过的完整输出

二十四份补丁精确重放，恢复全部生产源码。
本片只修改 `src/hb/shape_admission.rs`，其余 80 份源码保持原字节。
增量补丁 SHA-256：
`306e6de844e1ef7c938ed9121273d8c98248cd28b9166c6ec7725c24761539df`。

十七矩阵在原始／生产、Native／portable 四种配置下，
共 4,569,024 份完整观察一致。新增一般字符矩阵每配置 73,728 份，
四份签名均为
`c3f0aacf2d5b8c5a5167b71872161de46031eab2f0aeed765e40975bd31fc413`。
最终收据复用此次已经采集并与冻结签名、分母一致的旧矩阵原始版本，
共 32 个配置；旧矩阵生产版本与新增矩阵四配置重新执行。
Native／portable 各 203 项依赖内部测试与 90 项消费者测试通过。

## 准入成本

准入成本共 192,000 条，192 组各 1,000 条，预热各 100 次。
下表为 ClassDef 2、RTL、热缓存、零备用容量的 P95，单位为微秒。
每格来自同条件的 1,000 条样本，四种模式逐样本轮换。

| 塑形路径 | 普通 | 无操作回调 | 计数回调 | 最后准入点拒绝 |
|---|---:|---:|---:|---:|
| 无 GPOS | 199.959 | 222.334 | 227.125 | 221.666 |
| PairPos 2 | 319.125 | 345.542 | 349.375 | 339.084 |
| Format 3 | 135.208 | 143.542 | 146.791 | 141.958 |

对应计数回调的 work 为 633,253／3,129,940／5,173,205，
allocation 为 9,216／9,216／18 个完整新增容量 slots。
前两组实际输出各 3,072 个字形，并按 lookup 最大输出预留双数组；
Format 3 输出 1,536 个字形，18 个 slots 属于两份 9 位置匹配数组。
三组 Unicode work 均为 28,160，Unicode 查询本身不分配集合。
Information、Planning 与其他路径的费用包含在总 work 中。

最后准入点拒绝少收最终 3,072／3,072／1,536 个字形访问，
保留此前全部容量与 Unicode 费用；全部分组的精确前缀校验通过。
计数回调在上述三组的 P95 比普通入口多 27.166／30.250／11.583 微秒。
本片补齐准入边界，样本体现检查成本；不据此宣称文字排版提速。

## 普通内核对照

原始／生产普通入口共采 144,000 条：24 组输入、两个版本、三个会话，
每组每会话各 1,000 条，预热各 100 次；版本执行顺序交替。
下表为 ClassDef 2、RTL、热缓存、零备用容量的三个会话 P95 范围，
单位为微秒。端点来自各会话的 P95，不合并会话重新计算分位数。

| 塑形路径 | 原始版本 | 生产版本 |
|---|---:|---:|
| 无 GPOS | 192.875–199.833 | 192.958–206.167 |
| PairPos 2 | 307.291–315.500 | 301.959–318.958 |
| Format 3 | 131.208–133.208 | 129.959–130.875 |

普通内核对照存在会话波动，不据此推导整体提速。
首次定向 Clippy 发现准入成本 binary 丢弃 `black_box` 返回的 `Result`，
未显式处理 `must_use`。结果已先检查，现用 `let _ = black_box(output)` 明确释放；
生产源码与普通对照源码保持不变。修正后重新采集全部准入成本，
旧数据与 Clippy 日志以 `general-scalar-before-result-usage-fix` 为前缀保留。

## 已通过的定向门禁

库 Native／portable Clippy 与冻结前片的六项既有诊断一致，新增诊断为零。
消费者库及两份成本 binary 的 Clippy 通过 `-D warnings`。
Core Native／Wasm `--lib` check 与 portable 消费者 Wasm build 通过；
Wasm 未执行测试。workspace fmt、新增模块格式与四份 Python 语法检查通过。
文档门禁通过 522 份 Markdown 与当前 xtask 引用检查。

## 可复验入口

```bash
python3 -B tools/scripts/check_harfrust_general_scalar_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_general_scalar_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind general-scalar \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_general_scalar_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

收据为 `general-scalar-verification.json`、`general-scalar-cost-summary.json`、
`general-scalar-ordinary-cost-summary.json` 与 `general-scalar-gates.json`。
证据保存在 [B 切片目录](../evidence/text-stages-b-2026-11-02/README.md)。
完整规范化、复杂 shaping、B2–B4、C／D 仍待完成；
本片通过也不关闭 B1 或启用生产宽度复用缓存。

成稿再次按好好说话和技术文档清晰度复核，核对字符准入条件、
费用单位、拒绝与恢复、冻结矩阵复用、采样口径和未关闭范围。
编号标题、对比句式、空泛判断与中文半角标点的正则命中均为零。
节奏扫描标出 9 处，逐处复核后保留技术标识符和明确判断。
本次为作者自审，没有独立评审结论。
