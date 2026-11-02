# Harfrust 输入、实例与冷缓存准入

能力归属：Core 文字引擎的生产依赖 Harfrust 0.3.2。

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
当前消费者 Native／portable 各 23 项通过。

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
  --output doc/verification/evidence/text-stages-b-2026-11-02 \
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
  --output doc/verification/evidence/text-stages-b-2026-11-02 \
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
  --output doc/verification/evidence/text-stages-b-2026-11-02 \
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
  --output doc/verification/evidence/text-stages-b-2026-11-02 \
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

当前生产核对使用计划脚本，重放前六份冻结补丁并追加 `plan`。
无布局表计划准入覆盖 feature、合并、排序、增长、stage 与 SmallVec 存储；
语言、布局表、脚本计划回调和 trak 在未知操作前返回 `None`。
这些结果不代表 shaper 配置或 shaping 已受准入。

```bash
python3 tools/scripts/check_harfrust_plan_admission.py \
  --output doc/verification/evidence/text-stages-b-2026-11-02 \
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
