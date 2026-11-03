# B1：缺字空格与不换行连字符准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、完整输出对照、成本重新采样与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

本片接续[一对一规范化准入](text-stages-b1-singleton-normalization-2026-11-02.md)，
补齐原始缺字的空格与 U+2011 回退。普通入口与准入入口共用原始
normalizer 和定位内核。新增逻辑只证明路径并在执行前支付成本，
不自行生成替换字形或字宽。

## 原始字符决定回退

预检仍先沿 canonical `b == 0` 分解链查询。
找到满足 singleton 条件的已映射目标时，沿用前片的一对一输出证明。
分解没有找到字形时，普通内核才根据原始字符及其 Unicode 属性执行回退。
不能把分解链末端字符当作原始输入。

原始属性为 Zs 时，`space_fallback` 通过固定 scalar match 选择类型。
U+1680 没有可用的回退类型，保持 `.notdef`。
其余已定义的空格类型查询 U+0020，然后使用 `buffer.invisible` 作为备选。
找到字形后，普通内核写入空格回退类型并设置 scratch flag；
没找到时继续输出 `.notdef`。

U+2011 单独查询 U+2010，找到就输出它，否则输出 `.notdef`。
这发生在 canonical 分解尝试之后。U+2011 自身已有一条指向 U+2010
的单字符分解边，因此目标缺失时会重复查询；两次访问分别计费。

这两类回退保持输出长度和 singleton 属性，既有 GSUB 增长、
GDEF 分类、cluster 与方向准入继续适用。mark、variation selector、
特殊 continuation 和两字符分解仍在未知操作及 Recovery 前返回未支持。

## GSUB 后调整空格字宽

`position_default` 先填充默认字宽，再根据实际
`HAS_SPACE_FALLBACK` 调用空格定位；GPOS 在此后运行。
本片只支持水平路径，不执行垂直 origin 或 outline 查询。

EM 及其分数使用固定枚举值与 UPEM 算术；
FIGURE 从 0 到 9 查找第一个已映射数字，再读取一次水平 metric。
PUNCTUATION 先查句点，再查逗号，命中后读取一次 metric。
NARROW 把已有字宽减半。原始类型、ligated 状态和实际映射结果
仍由普通内核决定，准入不改这些判断。

费用使用 GSUB 后的实际逻辑长度 n，不能使用原始字符数。
GSUB Multiple 可以扩展空格记录，必须覆盖扩展后的完整 zip，
以及每条记录可能重复进行的字体查询。
scratch flag 检查计入 Position 已有的固定标量费用。

## 额外费用

令 f 为预检中需要空格或连字符回退查询的次数，
m 为所选 cmap 映射的访问上界，n₀ 为规范化前的输入长度，
n 为 GSUB 后的逻辑长度。含原始缺字时，本片在前片费用之外
支付以下执行费用：

| 阶段／操作 | 新增 work | 依据 |
|---|---:|---|
| Substitute／Mapping | `f × m` | 普通内核再次查询 SPACE 或 U+2010 |
| Substitute／Planning | `8 × n₀` | 固定类型匹配、invisible、结果及 scratch 状态 |
| Substitute／Information | `3 × n₀` | 原始属性、回退类型与记录状态 |
| Position／Information | `4 × n` | zip 记录、类别、ligated 与类型 |
| Position／Positions | `3 × n` | 记录访问及字宽读写 |
| Position／Planning | `32 × n` | 各类型的固定分支、UPEM、算术和方向 |
| Position／Mapping | `12 × n × m` | 最多十次数字查询和两次标点查询 |
| Position／Metrics | `2 × n` | 数字与标点各一次 metric 的保守上界 |
| allocation | `0` | 本回退不扩大集合；独立 GSUB／lookup 存储另计 |

Position 的新增费用只在实际 scratch flag 置位时收取。
数字与标点是互斥分支；此处使用两者之和作为保守上界，
不是宣称每条记录实际执行十二次查询。
work 计元素访问，包括重复查询；allocation 计整份新增容量 slots，
不是字节。预检自身的 Unicode、Mapping、Planning 与 Input 费用另行先准入。
未知路径不会因预算回调拒绝而转入普通 fallback。

## 验证范围与口径

新 fixture 只替换 cmap，复用冻结的八组 GSUB／GPOS 内核和两组 GDEF。
十种映射覆盖 SPACE 存在／缺失、数字首位／中间／末位／全部缺失、
句点／逗号／全部缺失、canonical 目标存在，以及原始空格已经映射。
输入涵盖全部 17 个 Zs、U+1680、U+2011 和多个前后缀。

普通输出矩阵每配置 122,880 份：
8 内核 × 2 GDEF × 10 mapping × 2 冷热轮次 × 2 script × 2 方向
× 4 输入 × 3 cluster 模式 × 4 cluster level × 2 容量。
消费者另覆盖 Han script，逐个准入点拒绝并验证完整费用前缀、
原始 Error source、停止位置及下一次普通调用。
没有进入 pipeline 的拒绝保留原输入；进入后的拒绝清空逻辑输入并保留容量。

`buffer.invisible` 没有公开 setter。额外的普通参考测试只注入临时原始／生产
库副本，覆盖 SPACE 存在／缺失、invisible 有／无、两个方向和五种输入，
每配置 40 份完整观察。冻结源和生产源均不加入该测试模块。
生产依赖的私有测试另外比较普通／准入输出，覆盖相同的内部设置。

准入成本输入为
`AB\u{2007}\u{2008}\u{202f}\u{2011}\u{2000}\u{2001}\u{a0}CDE`
重复 128 次，共 1,536 个字符。
无 GPOS、PairPos 2 和 Format 3 分别组合 ClassDef 1／2，
覆盖 LTR／RTL、冷／热缓存与 0／16,384 备用容量。
普通、无操作回调、计数回调与最后准入点拒绝逐样本轮换。
计时包含成功结果释放或拒绝清理；输入、字体、计划、预热和保留 buffer
释放位于计时之外。原始／生产普通入口另采三个会话并交替版本顺序。
系统负载未控制；这些样本不替代 Core 完整排版、GPU 或 16 ms／4 ms 验收。

首轮私有 fixture 使用 Format 0 的零字形条目模拟 SPACE 缺失；
该 reader 返回 `Some(0)`，没有进入 invisible 备选。
现改为 Format 6 真正省略 SPACE 条目。
旧 GDEF 消费者的缺字空格负例已改为完整普通结果对照；
原 fixture 和普通 oracle 保持原字节。
临时 portable 参考测试显式导入测试用 std，生产 no_std 边界不变。
初始编译与测试日志保留，不作为最终通过证据。
首轮完整矩阵的脚本分母误写为每配置 491,520，实际循环为 122,880；
已按上述维度修正并重新执行。逐点拒绝输入在全部 Zs 后追加 U+2011，
同时覆盖连字符的重复映射路径。

## 完整输出与成本收据

Native／portable 各 205 项依赖内部测试与 98 项消费者测试通过，
包括完整字段对照及逐点拒绝恢复。workspace fmt、新增 Rust 模块格式、
四份 Python 语法与 524 份文档检查通过。
依赖模块按 Edition 2021、消费者按 Edition 2024 核对格式。
二十六份补丁精确重放，恢复全部生产源码。本片只修改
`src/hb/shape_admission.rs`，其余 80 份源码保持原字节。
增量补丁 SHA-256：
`d4df7dc47fffa43889310e8d47dd95374678a54a73bc65fbfac65843ea821856`。

十九矩阵的原始／生产、Native／portable 四种配置全部重新执行，
共 5,945,280 份完整观察一致。新增空间矩阵每配置 122,880 份，
四份签名均为
`7897d9a09a502c41646ee7aac7a045accbbc328a1f921b59db97ae01b011f67d`。
额外 invisible 普通参考每配置 40 份，共 160 份观察一致，
四份签名均为
`3b760b7233e91e54646c190cf06b2d35e794f28f51d740b5b63c21ae1365ebfb`。
消费者与内部测试的输入、fixture 和源码身份均保存于完整收据。
十三项跨目标定向门禁通过：Native／portable 库 Clippy 与冻结前片的
六项既有诊断一致，新增诊断为零；消费者和两份成本程序 Clippy、
Core Native／Wasm check、消费者 Wasm build 均通过。
Wasm 只构建，未执行测试。

## 准入成本

以下数值来自样式修正后的完整重新采样。
本轮 Clippy 发现冻结 fixture 的区间判断样式，
两个成本程序在模块引用处声明对应豁免，fixture 与生产源码不变。
旧源码、样本、汇总与诊断以 `space-before-fixture-range-` 前缀保存；
准入成本与普通对照均已重新采集，费用计数与原轮次一致。

准入成本共 192,000 条，192 组各 1,000 条，预热各 100 次。
下表为 ClassDef 2、RTL、热缓存、零备用容量的 P95，单位为微秒。
每格来自同条件的 1,000 条样本，四种模式逐样本轮换。

| 塑形路径 | 普通 | 无操作回调 | 计数回调 | 最后准入点拒绝 |
|---|---:|---:|---:|---:|
| 无 GPOS | 119.416 | 167.584 | 172.291 | 168.791 |
| PairPos 2 | 164.416 | 214.292 | 220.750 | 208.000 |
| Format 3 | 102.958 | 141.750 | 149.834 | 144.083 |

对应计数回调的 work 为 1,230,903／2,791,014／5,688,402，
allocation 为 9,216／9,216／22 个完整新增容量 slots。
前两组实际输出各 1,920 个字形，并按 lookup 上界预留双数组；
Format 3 输出 1,536 个字形。其 22 个 slots 来自两份 9 位置匹配数组、
三个 `SubtableInfo` slots 和一个 `LookupInfo` heap slot。
这些 lookup 存储独立于本片不增长集合的规范化与空格定位。

三组 Mapping work 为 555,282／555,282／472,338，
Metrics work 为 8,448／8,448／4,608；
GSUB 增长后的定位费用按实际 n 支付，默认 metric 填充还访问完整数组。
最后准入点拒绝少收最终 1,920／1,920／1,536 次字形访问，
保留此前全部容量、映射与 metric 费用；全部分组的精确前缀校验通过。
计数回调在上述三组的 P95 比普通入口多 52.875／56.334／46.876 微秒。
本片补齐准入边界，采样体现检查开销，不据此宣称文字排版提速。

## 普通内核对照

原始／生产普通入口共采 144,000 条：24 组输入、两个版本、三个会话，
每组每会话各 1,000 条，预热各 100 次；版本执行顺序交替。
下表为 ClassDef 2、RTL、热缓存的三个会话 P95 范围，单位为微秒。
端点来自各会话的 P95，不合并会话重新计算分位数。

| 塑形路径 | 原始版本 | 生产版本 |
|---|---:|---:|
| 无 GPOS | 111.208–117.417 | 113.291–116.584 |
| PairPos 2 | 153.875–158.125 | 151.375–155.625 |
| Format 3 | 92.666–96.625 | 92.916–97.041 |

系统负载未控制，普通内核对照存在会话波动。
这些数据不替代 Core 完整排版、GPU 或 16 ms／4 ms 验收。

## 可复验入口

```bash
python3 -B tools/scripts/check_harfrust_space_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_space_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_space_ordinary_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_space_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

收据为 `space-verification.json`、`space-cost-summary.json`、
`space-ordinary-cost-summary.json` 与 `space-gates.json`，
保存在 [B 切片目录](../evidence/text-stages-b-2026-11-02/README.md)。
前 25 片的脚本与补丁保留为历史快照；后续源码核验使用本片入口。
完整规范化、复杂 shaping、B2–B4、C／D 仍待完成。
