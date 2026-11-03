# Harfrust 输入、实例与冷缓存准入

能力归属：Core 文字引擎的生产依赖 Harfrust 0.3.2。

验证日志、原始样本和收据保存在本地 `target/verification/text-stages-b-2026-11-02/`，
不纳入 Git，克隆仓库后不会自带这些文件。脚本拒绝向 `doc` 或未被 Git
忽略的仓库目录写入中间产物；生产补丁保存在 `dependencies/`。

本消费者覆盖 `try_reserve_admitted`、`try_push_str_admitted` 和
`try_add_admitted` 的外部调用、错误 source、调用方粘滞预算及拒绝后恢复。
新增 `try_from_coords_admitted` 的坐标、SmallVec spill、条件选择、逐点拒绝和
完整候选验证。后续普通 shaping 仍待接入会话准入。

新增 `try_from_variations_admitted` 覆盖无 avar／avar1 用户坐标、重复标签、
分段重扫、逐点拒绝与粘滞预算。avar2 在固定 header 读取后返回 `None`；
完整冷缓存、计划与 shaping 仍待完成。

新增 `try_new_admitted` 覆盖目录、cmap、Native 空 lookup 槽与 mark digest。
AAT 表在目录识别后、reader 之前返回 `None`；
portable 非零 lookup 现通过相同内核完整展开。
`try_prepare_lookup_admitted` 补齐 Native lazy LookupInfo 和子表缓存准入；
portable 热查询已有完整缓存。后续 shaping 不受这两个入口准入。
当前消费者 Native／portable 各 32 项通过。

```bash
cargo test --manifest-path tests/consumers/harfrust-admission/Cargo.toml --lib
cargo test --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --lib --no-default-features --features portable
cargo build --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --lib --target wasm32-unknown-unknown --no-default-features --features portable
```

原始 buffer 子片的对照校验归档、逐字节重放补丁，并用同一 oracle 源码分别编译
原始／生产依赖的 Native 与 portable 库。每组有 216 个输入，
包含三种字体、坐标变化、两种方向、组合字符、CJK／Arabic、重复追加和清空后复用；
每个输入观察首次与恢复后的 glyph 信息及位置。
脚本另运行两种配置各 5 项私有测试，私有清单只移除未使用的上游 benchmark
开发依赖，不修改生产清单。

instance 脚本对应追加用户坐标补丁之前的历史生产快照。
当前生产源码按冻结 buffer、instance、variation、cold、lookup，
再追加空 AAT 表头补丁重放。
实例脚本保留旧 buffer receipt，并核对增量以外 71 个源码文件，
用同一实例 oracle 编译原始／生产 Native／portable 四组：
每组 1170 个实例（同时检查重复 reset）和 162 份真实字体 glyph 观察。
覆盖 0／1／11／12／64 轴、短输入、空／默认坐标及真假／损坏条件。
旧 `check_harfrust_buffer_admission.py` 对应未追加实例补丁的历史生产快照。

```bash
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin buffer-cost
```

用户坐标脚本对应追加 cold 补丁之前的历史快照。它保留前两份冻结 receipt，
核对另外 73 个源码文件，并执行用户／命名实例的原始版本四组对照。

```bash
python3 tools/scripts/check_harfrust_variation_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin variation-cost > target/variation-cost.ndjson
python3 tools/scripts/summarize_harfrust_variation_cost.py \
  --samples target/variation-cost.ndjson \
  --binary target/release/variation-cost \
  --output target/variation-cost-summary.json
```

冷缓存脚本对应追加 lookup 前的历史快照，保留前三份冻结补丁并核对其他 68 个源码文件。
原始／生产 Native／portable 每组有 143 个字体及 3,432 份 glyph 观察。

```bash
python3 tools/scripts/check_harfrust_cold_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin cold-cost > target/cold-cost.ndjson
python3 tools/scripts/summarize_harfrust_cold_cost.py \
  --samples target/cold-cost.ndjson \
  --binary target/release/cold-cost \
  --output target/cold-cost-summary.json
```

`cold-cost` 只测 Native 冷对象构造，未包含 lazy LookupInfo、ShapePlan、
shaping、Core 或 GPU；普通／准入／最后点拒绝每组 1,000 次，逐样本轮换。

lookup 脚本对应追加空 AAT 表头补丁前的历史快照，保留前四份冻结补丁，
核对其他 67 个源码文件；四组各 171 个字体输入及 4,104 份输出一致。

```bash
python3 tools/scripts/check_harfrust_lookup_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin lookup-cost > target/lookup-cost.ndjson
python3 tools/scripts/summarize_harfrust_lookup_cost.py \
  --samples target/lookup-cost.ndjson \
  --binary target/release/lookup-cost \
  --output target/lookup-cost-summary.json
```

`lookup-cost` 每组 1,000 次、五种模式与三种字体共 15,000 次。
只测 GSUB lookup 0 的冷 fill、热 hit 和最后点拒绝；
空回调基线是同一准入内核，不是普通 shaping。
不包含 ShaperData 构造、成功释放、AAT、ShapePlan、Core 或 GPU。

AAT 表头脚本对应追加计划补丁前的历史快照，保留前五份冻结补丁，
新增一个模块、修改两个既有文件，其他 75 个源码文件保持。
四组原始／生产 Native／portable 各 207 个字体输入、4,968 份输出一致。
空 AAT 表头准入不包含非空 AAT 构造、ShapePlan 或 shaping 准入。

```bash
python3 tools/scripts/check_harfrust_aat_headers.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin aat-cost > target/aat-cost.ndjson
python3 tools/scripts/summarize_harfrust_aat_cost.py \
  --samples target/aat-cost.ndjson \
  --binary target/release/aat-cost \
  --output target/aat-cost-summary.json
```

`aat-cost` 只测四种合成空 AAT 表头的完整 Native 冷构造。
四种模式各 1,000 次，共 16,000 次；普通入口与准入空回调分别保留。
成功释放在计时之外，拒绝包含候选清理。

计划脚本对应追加配置补丁前的历史快照，重放前六份冻结补丁并追加 `plan`。
无布局表计划准入覆盖 feature、合并、排序、增长、stage 与 SmallVec 存储；
语言、布局表、脚本计划回调和 trak 在未知操作前返回 `None`。
这些结果不代表 shaper 配置或 shaping 已受准入。

```bash
python3 tools/scripts/check_harfrust_plan_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin plan-cost > target/plan-cost.ndjson
python3 tools/scripts/summarize_harfrust_plan_cost.py \
  --samples target/plan-cost.ndjson \
  --binary target/release/plan-cost \
  --output target/plan-cost-summary.json
```

`plan-cost` 只测 fresh plan，使用无布局表字体和 0／4／6／72 项用户 feature；
四种模式各 1,000 次，共 16,000 次。成功释放在计时之外，拒绝包含候选清理。
原始／生产 Native／portable 普通路径每组 39,744 份观察一致。

配置脚本对应追加布局表计划补丁前的历史快照，重放前七份冻结补丁并追加 `configuration`。
`ShaperBuilder::try_build_admitted` 覆盖字体映射、指标、OT／AAT 借用表头及
坐标扫描，未知 AAT count 在可变长 reader 前返回 `None`。
逐点拒绝保留 source，已有缓存和实例可继续使用；实际 shaping 准入仍待完成。

```bash
python3 tools/scripts/check_harfrust_configuration_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin configuration-cost > target/configuration-cost.ndjson
python3 tools/scripts/summarize_harfrust_configuration_cost.py \
  --samples target/configuration-cost.ndjson \
  --binary target/release/configuration-cost \
  --output target/configuration-cost-summary.json
```

`configuration-cost` 只测借用配置，四种模式各 1,000 次、四组字体配置共 16,000 次。
既有缓存和实例构造、计划与 shaping 均在计时之外。
四组原始／生产 Native／portable 各 39,744 份普通观察一致，
配置消费者另对结构字体及真实字体的准入／普通配置作等价验证。

布局表计划脚本保留追加 `plan-layout` 后的历史快照，重放前八份冻结补丁。
GSUB／GPOS 查询覆盖 script、LangSys、必选 feature、变体替换及 lookup 收集；
语言、脚本专用计划回调、非空 AAT 与实际 shaping 尚未支持准入。
结构字体逐点拒绝检查 source、trace 前缀、相同重试和下一次普通调用。

```bash
python3 tools/scripts/check_harfrust_plan_layout_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin plan-layout-cost > target/plan-layout-cost.ndjson
python3 tools/scripts/summarize_harfrust_plan_layout_cost.py \
  --samples target/plan-layout-cost.ndjson \
  --binary target/release/plan-layout-cost \
  --output target/plan-layout-cost-summary.json
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-ordinary-cost
```

`plan-layout-cost` 采样四组字体、四种模式各 1,000 次，共 16,000 次。
普通路径成本脚本将相同源码编译为原始／生产 release，三轮轮换先后顺序，
各组 1,000 次，共 24,000 次；成功释放均在计时之外。
采样范围是计划构造，尚不证明 Core 或 GPU 性能提升。

私有语言历史快照使用对应脚本，重放前九份冻结补丁，再追加 `private-language`。
支持任意长度前缀中的有效 `hbot`，并覆盖计划语言值的 inline／spill 存储。
通用语言映射和脚本计划回调仍提前返回未支持；本片不支持实际 shaping 准入。

```bash
python3 tools/scripts/check_harfrust_private_language_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin private-language-cost > target/private-language-cost.ndjson
python3 tools/scripts/summarize_harfrust_private_language_cost.py \
  --samples target/private-language-cost.ndjson \
  --binary target/release/private-language-cost \
  --output target/private-language-cost-summary.json
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind private-language \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-ordinary-cost
```

私有语言准入成本为三组输入、四种模式各 1,000 次，共 12,000 次；
普通路径使用同一份源码分别编译原始／生产 release，
三轮轮换执行顺序，共 18,000 次。计时不含语言解析、缓存和 Shaper 配置，
成功释放在计时之外，准入拒绝包含候选清理。
原始对照矩阵新增四组结构字体、两份真实字体、28 种语言、
两种脚本、四方向与五种文本，共 6,720 份观察。

该历史生产快照核对使用通用语言脚本，重放十份冻结补丁后追加 `general-language`。
新规则核对全部 148 条条件、分组和输出顺序，普通与准入入口共用内核。
新增完整输出矩阵包含 167 种语言、六种字体、两种脚本、四方向与三种文本，
共 24,048 份观察。脚本计划回调与实际 shaping 准入仍待完成。

```bash
python3 tools/scripts/check_harfrust_general_language_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin general-language-cost \
  > target/verification/text-stages-b-2026-11-02/general-language-cost.ndjson
python3 tools/scripts/summarize_harfrust_language_cost.py \
  --input-kind general-language \
  --output target/verification/text-stages-b-2026-11-02
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind general-language \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-ordinary-cost
```

通用语言成本为三组输入、四种模式各 1,000 次，共 12,000 次；
普通入口原始／生产三轮匹配共 18,000 次。计时与限制见
[B1 通用语言记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-general-language-2026-11-02.md)。

## 脚本专用规划回调

Arabic、Syriac、Hangul 的规划回调接入同一准入策略；
消费者核对普通／准入完整输出和逐点拒绝。Syriac 专用用例使用
保留 Arabic lookup 数据、按 tag 排序后将 `arab` 改为 `syrc` 的合成夹具，
并核验专用规划数据存储已执行，避免误测默认规划。
这些测试不证明实际塑形操作准入已经完成。

脚本规划回调的历史快照使用以下工具，重放十一份冻结补丁后追加该片补丁：

```bash
python3 tools/scripts/check_harfrust_script_callbacks_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-harfrust-cold-oracle
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin script-callbacks-cost \
  > target/verification/text-stages-b-2026-11-02/script-callbacks-cost.ndjson
python3 tools/scripts/summarize_harfrust_language_cost.py \
  --input-kind script-callbacks \
  --output target/verification/text-stages-b-2026-11-02
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind script-callbacks \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-ordinary-cost
```

实现、成本边界和验证结论见
[B1 脚本规划回调记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-script-callbacks-2026-11-02.md)。

非扩张塑形首片工具保留其历史快照，成本采样入口继续可用：

```bash
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin shape-cost \
  > target/verification/text-stages-b-2026-11-02/shape-cost.ndjson
python3 tools/scripts/summarize_harfrust_shape_cost.py \
  --output target/verification/text-stages-b-2026-11-02
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind shape \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-ordinary-cost
```

范围、费用、buffer 恢复和未完成门禁见
[B1 非扩张塑形记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-shaping-2026-11-02.md)。
本入口支持已映射 LTR ASCII，不表示复杂文本的完整分析准入已经可用。

当前生产源码核对使用多字形替换工具，重放十四份冻结补丁再追加本片：

```bash
python3 tools/scripts/check_harfrust_multiple_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_multiple_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind multiple \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_multiple_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

单字形替换的两种格式、两种 coverage 和冷／热 lookup 计费，
以及上游完整输出对照见
[B1 单字形替换记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-single-substitution-2026-11-02.md)。
后续正长度多字形增长已接入，完整输出、扩容费用、逐点拒绝及普通路径成本对照见
[B1 多字形替换记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-multiple-substitution-2026-11-02.md)。
输出原始字节无损压缩保存，散列针对解压内容。
连字匹配位置 inline／spill、嵌套组件求和、逐点拒绝与混合替换见
[B1 连字替换记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-ligature-substitution-2026-11-02.md)。

一至八位置 Format 3 上下文已接入准入，嵌套叶子限定零 flags 单字形替换。
四项新测试核对完整输出、逐点拒绝前缀、未知路径提前拒绝、
错误 source 和 buffer 复用；完整原版对照矩阵保持独立，见
[B1 上下文替换记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-context-substitution-2026-11-02.md)。

Format 3 堆匹配与备份消费者覆盖 9／17／64 位置、逐点拒绝、
65 位置未支持，以及此前五组件连字后接 52／53 位置的求和边界。
两份匹配数组每次 shape 预留一次，所有嵌套单字形记录复用存储。
独立 spill oracle 与成本程序分别覆盖完整输出与分配费用；
实施状态和复验命令见
[B1 上下文堆存储记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-context-spill-2026-11-02.md)。
当前仍不支持删除、递归替换或完整复杂文字分析。

GPOS 单字形定位消费者覆盖两种格式、两种 coverage、标量值、
子表与 lookup 链，以及前置 MultipleSubst 后的完整输出与实际 glyph 数。
三项新测试逐点拒绝冷／热路径，核对原始 source、费用前缀与 buffer 复用，
并确认设备值、非零 flags、GDEF 和 RTL 在 pipeline 前未支持。
定位本身不分配；前置替换的扩容与冷 lookup 存储仍独立收费。
十二矩阵四配置、32,000 个准入成本和 24,000 个普通对照样本
及复验命令见
[B1 GPOS 单字形定位记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-gpos-single-2026-11-02.md)。

GPOS 配对定位消费者新增两种 PairPos、coverage／class 格式、
第一／第二标量记录、三种 cluster level 和前置替换的完整输出对照。
四项新测试覆盖冷／热逐点拒绝、source 与 buffer 恢复、
原始设备／保留位，以及非法 class 值在矩阵寻址前未支持。
SinglePos 同步补齐原始保留位反例。
十三矩阵四配置的冻结输出、成本与跨目标复验入口见
[B1 GPOS 配对定位记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-gpos-pair-2026-11-02.md)。

ASCII RTL 消费者覆盖 Latin／Common／unknown／未指定 script、
真实镜像缺字、原生方向与最终反转、三种 cluster level、备用容量，
以及前置替换、SinglePos／PairPos 和逐准入点错误恢复。
SinglePos 的旧 ASCII RTL 负例改为非 ASCII 反例，冻结普通 oracle 保持。
该片源码核对入口为 `check_harfrust_rtl_admission.py`；
十四矩阵四配置共 1,619,904 份完整观察一致，
Native／portable 各 203 项依赖与 75 项消费者测试通过。
64,000 个准入成本、48,000 个普通对照样本和跨目标定向门禁通过，
Wasm 只构建未执行。范围与复验命令见
[B1 ASCII RTL 记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-rtl-2026-11-02.md)。

简单 Unicode 消费者新增六项测试，覆盖已映射的 Ll／Lu／Lt／Lo／Zs、
Han 默认 shaper、四种 cluster level、六种 cmap reader、
MacRoman／Symbol、冷／热逐点拒绝、source 和 buffer 恢复。
旧 shaping 负例改用含组合标记的输入；此前十四份 oracle 保持冻结。
该片源码核对入口为 `check_harfrust_simple_unicode_admission.py`；
十五矩阵四配置共 3,831,744 份完整观察一致，
Native／portable 各 203 项依赖与 81 项消费者测试通过。
192,000 条准入成本、144,000 条普通对照样本及跨目标定向门禁通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。范围及复验命令见
[B1 简单 Unicode 记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-simple-unicode-2026-11-02.md)。

GDEF 消费者新增五项测试，覆盖 ClassDef 1／2、空与损坏 reader、
字体 mark、替换后的分类变化、上下文重复 record 和冷热逐点拒绝恢复。
该片源码核对入口为 `check_harfrust_gdef_admission.py`；
十六矩阵四配置共 4,274,112 份完整观察一致，Native／portable
各 203 项依赖与 86 项消费者测试通过；旧 GDEF 负例改为 mark attachment flag。
192,000 条准入成本、144,000 条普通对照与跨目标定向门禁通过。
库 Clippy 新增诊断为零，Wasm 只构建未执行，范围和复验命令见
[B1 GDEF 记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-gdef-2026-11-02.md)。

一般字符消费者新增四项测试，覆盖已映射标点、数字、符号、
单独 emoji、非提前返回类别、缺失镜像、逐点拒绝与费用隔离。
当前源码核对入口为 `check_harfrust_general_scalar_admission.py`；
十七矩阵四配置共 4,569,024 份完整观察一致，Native／portable
各 203 项依赖与 90 项消费者测试通过。
192,000 条准入成本、144,000 条普通对照与跨目标定向门禁通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。范围和复验命令见
[B1 一般字符记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-general-scalar-2026-11-02.md)。

一对一规范化消费者新增四项测试，覆盖 canonical singleton 分解、
`.notdef`、完整字段、逐点拒绝、原始 source 与普通恢复，
并隔离输出数组切换的容量与访问费用。旧缺字负例迁移到实际未知的 mark 路径，
fixture 和普通 oracle 保持原字节。
二十五份补丁精确重放；十八矩阵四配置共 5,453,760 份完整观察全部重新执行并一致。
Native／portable 各 204 项依赖内部测试与 94 项消费者测试通过；
192,000 条准入成本、144,000 条普通对照和跨目标定向门禁通过。
库 Clippy 新增诊断为零，Wasm 只构建未执行；复验命令见
[B1 一对一规范化记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-singleton-normalization-2026-11-02.md)。

`space_tests.rs` 补齐原始 Zs 和 U+2011 缺字回退，覆盖 SPACE、
数字和标点映射、GSUB 后空格定位以及逐点拒绝恢复。
内部 invisible 普通参考另在临时库副本中注入测试。
二十六份补丁精确重放；十九矩阵四配置共 5,945,280 份完整观察一致，
invisible 另有 160 份一致观察。
Native／portable 各 205 项依赖内部测试与 98 项消费者测试、
192,000 条准入成本、144,000 条普通对照和十三项定向门禁通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。当前复验入口见
[B1 空格回退记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-space-fallback-2026-11-02.md)。

`decomposition_miss_tests.rs` 新增四项消费者测试，覆盖 a／b 映射组合、
直接终止和单字符链后终止、完整输出、原始 source 与逐点拒绝恢复。
b 命中仍在 Recovery 前返回未支持，原 fixture 和普通 oracle 保持冻结。
二十七份补丁精确重放；二十矩阵四配置共 6,338,496 份完整观察一致，
旧原始版本 38 份配置逐字节核对后复用，其余配置重新执行。
Native／portable 各 205 项依赖内部测试与 102 项消费者测试、
192,000 条准入成本、144,000 条普通对照和十三项定向门禁通过。
库 Clippy 新增诊断为零，Wasm 只构建未执行；见
[B1 第二字符缺字记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-decomposition-miss-2026-11-03.md)。

`normalizer_kernel_oracle.rs` 增加包含分解增长、重排、重组、CGJ、
variation selector 与 Hangul 的独立普通输出矩阵；
每配置 614,400 份。内部 normalizer 与排序测试另覆盖逐点拒绝及恢复。
二十八份补丁精确重放；二十一矩阵四配置共 8,796,096 份完整观察一致，
旧原始版本 40 份配置逐字节核对后复用，其余配置重新执行。
Native／portable 各 211 项依赖内部测试与 102 项消费者测试通过。
192,000 条准入成本、两组各 144,000 条普通对照及十四项定向门禁通过。
格式修正的源码证明与最终源码成本重采通过；库 Clippy 新增诊断为零，
Wasm 只构建未执行。公共 mark、非 singleton 输出与未知回调继续拒绝。
当前复验入口见
[B1 共享规范化记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-normalizer-kernel-2026-11-03.md)。

`dotted_circle_kernel_oracle.rs` 增加圆圈映射存在／缺失、flags／context
抑制和规范化增长的独立普通矩阵，每配置 497,664 份观察。
二十九份补丁精确重放；二十二矩阵四配置共 10,786,752 份完整观察一致。
Native／portable 各 214 项内部测试与 102 项消费者测试通过。
192,000 条准入、144,000 条普通 singleton、144,000 条普通 mark
及 288,000 条圆圈普通成本样本、十一项边界检查通过。
全部七份采样前 Clippy 收据与十九份原始采样文件复核通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。公共组合输入尚未开放。
当前冻结快照复验入口见
[B1 dotted-circle 记录](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-dotted-circle-kernel-2026-11-03.md)。

当前 [grapheme 合并与反向分组](../../../doc/verification/evidence/knowledge-maintenance-2026-11-03/doc/verification/reviews/text-stages-b1-grapheme-kernel-2026-11-03.md)
的定向测试、完整普通输出对照与跨目标检查通过，成本记录完成。
三十份补丁精确重放；二十二矩阵四配置共 10,786,752 份完整观察一致。
Native 与 portable 各 218 项内部测试、102 项消费者测试通过。
原版 44 份配置逐字节核对后复用，生产 44 份配置重新执行；
fixture、oracle 和成本输入保持冻结，RTL 消费者费用断言适配逐比较与逐区间计费。
768,000 条成本样本、七份采样前 Clippy 收据及十九份原始样本文件核对完成；
十一项边界检查通过，库 Clippy 新增诊断为零，Wasm 只构建未执行。
普通路径尾延迟波动明显，本记录不证明没有性能回退或 Core 排版提速。
公共组合输入和 Core 阶段适配保持未启用，完整范围与最终收据见该片记录。

将 NDJSON 保存后，用 `tools/scripts/summarize_harfrust_buffer_cost.py`
的 `--samples`、`--binary` 和 `--output` 参数校验分母与实际费用并生成汇总。

`buffer-cost` 采样冷／复用容量、普通／准入／追加前拒绝六组各 1000 次，
输出逐样本 NDJSON。范围只有输入扫描与 buffer 存储，准入回调为本地计数；
顺序采样可能受系统负载和预热影响，不能据此宣称 Core 性能提升。
Wasm 命令只构建，不执行测试。

实例成本 binary 采样三种轴场景，每种普通／准入／最后准入点拒绝各 1000 次，
总计 9000 个样本；模式逐样本轮换，预热每组 100 次。
成功实例释放位于计时之外，拒绝包含内部候选清理。
只统计规范化实例构造及本地计数回调，不包含 Core／GPU 或完整冷字体缓存。

```bash
cargo run --manifest-path tests/consumers/harfrust-admission/Cargo.toml \
  --release --bin instance-cost > target/instance-cost.ndjson
python3 tools/scripts/summarize_harfrust_instance_cost.py \
  --samples target/instance-cost.ndjson \
  --binary target/release/instance-cost \
  --output target/instance-cost-summary.json
```
