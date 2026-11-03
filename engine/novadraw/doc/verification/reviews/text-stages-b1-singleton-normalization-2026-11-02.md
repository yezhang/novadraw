# B1：缺字与一对一规范化准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、完整输出、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

本片接续[一般字符准入](text-stages-b1-general-scalar-2026-11-02.md)，
补齐缺失字形的 `.notdef` 和 canonical one-to-one 分解。
普通入口与准入入口仍使用原始规范化内核；
新增逻辑只检查所选路径，并在执行前支付访问成本。

## 缺字如何保持单字形组

原始输入仍须满足 singleton 属性限制：不含 Unicode mark、
default-ignorable、特殊 continuation 或 fraction 状态。
字体缺失原始字符时，预检读取锁定的 Unicode 分解表。
只有每条分解边都满足 `b == 0` 才继续沿 `a` 查询，
在第一个已映射且满足 singleton 属性的目标处结束。

这与普通 normalizer 的 shortest 路径一致。
例如 Kelvin sign 可以输出 K，Ohm sign 可以输出 Omega，
CJK compatibility ideograph 可以输出对应汉字。
Angstrom sign 的目标 Å 已映射时可输出一个字符；
若 Å 也缺字，下一条分解产生 A 与 ring mark，预检返回未支持。
预检不生成替换结果，实际输出仍由共享 `decompose` 函数完成。

整个链未找到字形时，普通内核会对原始字符执行 fallback。
本片只允许 `.notdef`：原始字符为 space separator 或 U+2011 时，
在未知 fallback 执行前返回未支持。
判断使用原始字符，不能用分解链末端字符替代。

原输入没有 mark，普通 normalizer 的 `all_simple` 始终为 true，
第二轮排序和第三轮重组不会运行。每个实际替换的长度仍为一；
既有 GSUB 增长、GDEF 分类和定位准入继续适用。
新增私有测试遍历锁定 Unicode 表的全部 codepoint，
验证 `b == 0` 分解链无环，不为递归引入猜测的深度限制。

## 规范化的访问与复制

Unicode 分解先执行固定 Hangul 算术，再最多读取三个生成索引和一个 payload。
预检在调用前支付四次 Unicode 表访问与对应标量判断；
每次 `a` 的字体映射也先支付独立费用。
普通执行会再次读取这些数据，执行费用不能因预检预热而免除。

短路映射在第一个缺字处停下，随后 `decompose_current_character`
再次查询该字符；其后的字符各查询一次。
因此每个含缺字的输入，除原有的 n 次映射费用外，还支付一次重复映射。
分解查询与 `a` 映射另按预检已证明的链长度计费。

`output_char` 调用 `output_glyph(0, 1)`。
即使输出长度保持不变，第一次替换也会切换到独立输出数组，
复制已经输出的完整前缀。后续 `next_glyph`／`next_glyphs` 复制剩余记录；
`sync` 交换数组所有权。前缀和后续记录分别计费，
不能用“一对一”推断不存在复制。

输出字符只执行 `init_unicode_props`，不重新执行完整
`set_unicode_props` 的特殊分支。对应 GC、default-ignorable、
glyph info 与 scratch 字段访问单独支付。
最终长度仍为 n，进入 pipeline 前的准入预留已经拥有两份完整 n-slot 数组；
这一规范化路径不额外扩大容量。GSUB 的后续增长仍由各 lookup 独立准入。

令 n 为规范化前的输入长度，q 为预检中的分解查询次数，
r 为已找到 singleton 替换目标的次数，m 为字体映射查询的访问上界。
含原始缺字时，执行阶段在既有费用之外支付下表上界。
RTL 镜像可能使原始缺字转成已映射字符，因此实际分解次数可以少于 q 和 r；
费用仍保留预检证明的上界。

| 操作 | 新增 work | 依据 |
|---|---:|---|
| Mapping | `(q + 1) × m` | 第一个缺字的重复查询及每条分解边的 a 查询 |
| Unicode | `4 × q + 5 × r` | 分解表、输出 GC 与 default-ignorable |
| Planning | `20 × q + 15 × n + 2 × r` | 分解与递归分支、输出／ensure 状态、属性初始化 |
| Information | `9 × n + 3 × r` | 输入与 index、singleton 输出、前缀／后续复制和属性字段 |
| allocation | `0` | 长度不增长，双数组已预留；独立 lookup 存储另计 |

work 计元素访问，包括重复扫描；allocation 计整份新增容量 slots，不是字节。
这些公式对应已限制的普通算法分支，不用于估算任意 shaping。
预检自身的 Unicode、Mapping、Planning 和 Input 访问另行先准入。

## 拒绝与恢复

消费者逐个准入点拒绝，检查精确费用前缀、原始 Error source、
停止位置和后续普通调用。进入 pipeline 前拒绝保留原输入；
进入 pipeline 后拒绝清空逻辑输入并保留容量。
预算拒绝不调用普通 fallback，也不返回完整候选。
两字符分解、组合标记、空格与不换行连字符的未知 fallback
在 Recovery 前返回未支持。

旧消费者中“所有缺字都未支持”的负例随本片契约调整：
GPOS、RTL 和普通 shaping 的负例改为实际未覆盖的 mark 输入；
缺失 CJK cmap 的消费者现在对照完整 `.notdef` 输出。
旧 fixture 与普通 oracle 保持原字节。

首轮编译使用了错误的相对模块路径，已改为 `crate::hb::unicode`。
随后两处新测试的口径不一致：恢复输入使用 UTF-8 字节 cluster，
对照 fixture 使用 scalar 序号；容量测试同时包含 GSUB 输出增长和冷 lookup 分配。
恢复测试现显式使用一致的 cluster 设置；容量测试使用不触发替换的前后缀，
并先预热独立 lookup 缓存。失败日志以 `normalization-before-` 保存，
不作为通过证据。

## 验证场景

新 fixture 只替换 cmap，复用冻结的八组 GSUB／GPOS 内核和四组 GDEF。
四种 mapping 配置分别覆盖规范化目标存在、目标缺失、
原始字符已经映射，以及 Angstrom 的一对一目标存在。
输入包含前缀／后缀复制、多个缺字、`.notdef`、递归到两字符的拒绝、
原始 mark 与未知 fallback。

完整普通矩阵比较原始／生产、Native／portable 四种配置的全部
glyph info 与 position。每配置 221,184 份，组合为
8 内核 × 4 GDEF × 4 mapping × 2 冷热轮次 × 3 script × 2 方向
× 6 输入 × 3 cluster 模式 × 4 cluster level × 2 容量。
原来的十七矩阵按冻结 fixture、oracle 和输出签名核对。

准入成本使用 `ABKΩ\u{f900}。C🙂\u{0378}DEF` 重复 128 次，
共 1,536 个字符。无 GPOS、PairPos 2 和 Format 3 分别组合 ClassDef 1／2，
覆盖 LTR／RTL、冷／热缓存、0／16,384 备用容量。
普通、无操作回调、计数回调与最后准入点拒绝逐样本轮换。
计时包含成功结果释放或拒绝清理；输入、字体、计划、预热和保留 buffer 的释放
位于计时之外。原始／生产普通入口另采三个会话，并交替版本顺序。

本机系统负载未控制。这些样本衡量依赖路径与准入开销，
不能替代 Core 完整排版、GPU 或 16 ms／4 ms 验收。

## 已通过的完整输出

二十五份补丁精确重放，恢复全部生产源码。
本片只修改 `src/hb/shape_admission.rs`，其余 80 份源码保持原字节。
增量补丁 SHA-256：
`d12a75f8bb38807cb8ef68283d2957701f47db5a3ac329698ea9f7c7b2e719e5`。

十八矩阵的原始／生产、Native／portable 四种配置全部重新执行，
共 5,453,760 份完整观察一致，没有复用历史输出。
新增矩阵每配置 221,184 份，四份签名均为
`2a482796baa1677d97f92b3467ab8e150210154715862dee0f70426bbc4ed04d`。
Native／portable 各 204 项依赖内部测试与 94 项消费者测试通过。
workspace fmt、新增 Rust 模块格式与四份 Python 语法检查也已通过。

## 准入成本

准入成本共 192,000 条，192 组各 1,000 条，预热各 100 次。
下表为 ClassDef 2、RTL、热缓存、零备用容量的 P95，单位为微秒。
每格来自同条件的 1,000 条样本，四种模式逐样本轮换。

| 塑形路径 | 普通 | 无操作回调 | 计数回调 | 最后准入点拒绝 |
|---|---:|---:|---:|---:|
| 无 GPOS | 126.291 | 157.875 | 161.417 | 159.041 |
| PairPos 2 | 200.291 | 233.292 | 238.708 | 234.084 |
| Format 3 | 90.167 | 119.667 | 123.709 | 122.208 |

对应计数回调的 work 为 695,099／2,567,402／5,245,910，
allocation 为 9,216／9,216／22 个完整新增容量 slots。
前两组实际输出各 2,304 个字形，并按 lookup 最大输出预留双数组；
Format 3 输出 1,536 个字形。它的 22 个 slots 包括两份 9 位置匹配数组，
以及 nested lookup 的三个 `SubtableInfo` slots 和一个 `LookupInfo` heap slot。
普通预热没有匹配到该 nested leaf，准入预检仍须完成并支付它的冷构造。
三组 Unicode work 均为 33,920，Unicode 查询本身不分配集合。

最后准入点拒绝少收最终 2,304／2,304／1,536 个字形访问，
保留此前全部容量与 Unicode 费用；全部分组的精确前缀校验通过。
计数回调在上述三组的 P95 比普通入口多 35.126／38.417／33.542 微秒。
本片补齐准入边界，数据体现检查开销，不据此宣称文字排版提速。

## 普通内核对照

原始／生产普通入口共采 144,000 条：24 组输入、两个版本、三个会话，
每组每会话各 1,000 条，预热各 100 次；版本执行顺序交替。
下表为 ClassDef 2、RTL、热缓存、零备用容量的三个会话 P95 范围，
单位为微秒。端点来自各会话的 P95，不合并会话重新计算分位数。

| 塑形路径 | 原始版本 | 生产版本 |
|---|---:|---:|
| 无 GPOS | 123.542–127.375 | 124.125–128.958 |
| PairPos 2 | 192.667–201.542 | 188.583–198.542 |
| Format 3 | 91.709–92.375 | 91.042–93.959 |

普通内核对照存在会话波动，不据此推导整体提速。
Clippy 在首轮成本 binary 中发现，fixture 中的 oracle 文本常量未使用。
两个成本程序已对该 fixture 模块标明允许未使用成员；
生产源码和 fixture 保持不变，全部准入成本与普通对照重新采集。
旧样本、汇总和诊断以 `normalization-before-unused-fixture-fix` 为前缀保留，
不作为最终采样收据。

## 已通过的定向门禁

库 Native／portable Clippy 与冻结前片的六项既有诊断一致，新增诊断为零。
消费者库及两份成本 binary 的 Clippy 通过 `-D warnings`。
Core Native／Wasm `--lib` check 与 portable 消费者 Wasm build 通过；
Wasm 未执行测试。workspace fmt、新增模块格式与四份 Python 语法检查通过。
文档门禁通过 523 份 Markdown 与当前 xtask 引用检查。

## 可复验入口

```bash
python3 -B tools/scripts/check_harfrust_normalization_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_normalization_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind normalization \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_normalization_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

收据为 `normalization-verification.json`、`normalization-cost-summary.json`、
`normalization-ordinary-cost-summary.json` 与 `normalization-gates.json`，
保存在 [B 切片目录](../evidence/text-stages-b-2026-11-02/README.md)。
完整规范化、复杂 shaping、B2–B4、C／D 仍待完成；
本片不关闭 B1，也不启用生产宽度复用缓存。

成稿再次按好好说话与技术文档清晰度复核，检查准入条件、费用单位、
复制与容量、拒绝恢复、完整矩阵和采样口径。正则未发现编号标题、
翻案句式、空泛判断或中文半角标点；节奏候选逐处复核，
修正 subtable 容量的表述和未使用常量那句，保留必要的术语与验证条件。
这是作者自审，没有独立评审结论。
