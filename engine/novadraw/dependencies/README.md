# 生产依赖补丁

类型：`reference-guide`

能力归属：各依赖的直接使用方。本目录保存经批准的生产第三方依赖，
独立于只用于行为参考的 `third_party/`，不作为 Novadraw 一方 workspace package。

## Parley 0.7.0

直接 owner 为 Core 文字引擎；官方样例也使用同一 workspace 依赖。
用户于 2026-11-02 批准只读存储补丁接入，范围见
[B 复核与接入记录](../doc/verification/reviews/text-stages-b-storage-patch-2026-11-02.md)。

- 来源：crates.io `parley 0.7.0`，上游 `https://github.com/linebender/parley`。
- 发布提交：`10fedab4bfd4254a227a1a0f9602b6218e1fe447`。
- 原始归档 SHA-256：`ada5338c3a9794af7342e6f765b6e78740db37378aced034d7bf72c96b94ed94`。
- 许可证：`Apache-2.0 OR MIT`，包内保留原始许可证和版权。
- 本地改动：先应用[存储补丁](parley-0.7.0-storage.patch)，再应用
  [操作准入补丁](parley-0.7.0-operations.patch)，最后应用
  [断行准入补丁](parley-0.7.0-line-breaking.patch)。
- 版本与依赖：生产精确锁定 `0.7.0` 和本地 path；原始 feature 及传递依赖声明保持。

本地模块提供十个 Layout 数组的实际容量观察和共享字体借用，
以及 Copy brush 模板复制、对齐和候选断行的操作前准入；范围及验证见
[操作记录](../doc/verification/reviews/text-stages-b-operations-2026-11-02.md)与
[断行记录](../doc/verification/reviews/text-stages-b-line-breaking-2026-11-02.md)。
共享 backing、brush 外部存储、上下文缓存以及未来操作准入均不包含在浅快照内。
断行入口共享计算内核，新增准入、fallible 扩容及显式最终汇总；
普通入口已与原始依赖对照。shaping 与对齐源码保持；
隔离测试副本的 workspace／feature 门控不进入生产依赖。
宽度复用的阶段适配仍待实施，普通完整布局入口保持。
用户后续已批准[分析准入提案](../doc/design/architecture/text-analysis-admission-proposal.md)
限定的三个依赖版本；Parley 的 Harfrust 依赖现精确指向下述本地 0.3.2，
不改变其他版本的 Harfrust。

`cargo fmt --all` 会递归扫描本地 path 依赖，即使它已排除出 workspace。
本目录的 rustfmt 配置因此保留上游格式，避免在存储补丁中混入上游格式重写。
新增模块单独检查，覆盖本地维护的全部 Rust 改动：

```bash
rustfmt --edition 2021 --config disable_all_formatting=false \
  --check dependencies/parley-0.7.0/src/layout/storage.rs \
  dependencies/parley-0.7.0/src/layout/operations.rs \
  dependencies/parley-0.7.0/src/layout/line_admission.rs
```

维护时先核对原始归档及版本，保留完整变更补丁、来源与验证结果。
更新上游版本后重新核对全部存储字段、浅快照排除项和 Native／Wasm 消费者，
不将旧版测试结果直接用于新版。定向入口：

```bash
cargo test --manifest-path tests/consumers/parley-storage/Cargo.toml --lib
cargo check -p novadraw --lib
cargo check -p novadraw --lib --target wasm32-unknown-unknown
```

## Harfrust 0.3.2

直接 owner 为 Core 文字引擎的 Parley shaping 链路。用户于 2026-11-02
批准本版本的分析准入补丁；当前已完成输入 buffer 与规范化坐标子片，
冷构造及 shaping 准入仍待完成，见
[B1 buffer 记录](../doc/verification/reviews/text-stages-b1-buffer-2026-11-02.md)。
坐标与 GSUB／GPOS 条件选择的范围见
[B1 instance 记录](../doc/verification/reviews/text-stages-b1-instance-2026-11-02.md)。

- 来源：crates.io `harfrust 0.3.2`，上游 `https://github.com/harfbuzz/harfrust`。
- 发布提交：`7e7d63e1122d39e38e25dd9c0ebe4efb57b1d9d5`。
- 原始归档 SHA-256：`92c020db12c71d8a12a3fe7607873cade3a01a6287e29d540c8723276221b9d8`。
- 许可证：`MIT`，原始许可证、版权和归档保持。
- 本地改动：先应用[输入 buffer 补丁](harfrust-0.3.2-buffer.patch)，
  再应用[规范化实例增量补丁](harfrust-0.3.2-instance.patch)；
  再应用[用户坐标增量补丁](harfrust-0.3.2-variation.patch)，
  再应用[冷缓存基础增量补丁](harfrust-0.3.2-cold.patch)，
  再应用[lookup 缓存增量补丁](harfrust-0.3.2-lookup.patch)，
  再应用[空 AAT 表头增量补丁](harfrust-0.3.2-aat-headers.patch)，
  最后应用[ShapePlan 增量补丁](harfrust-0.3.2-plan.patch)；
  普通输入与准入入口共享预留后的初始化、字符及 cluster 写入内核。
- 接入：只有本地 Parley 0.7.0 的 Harfrust 依赖指向该 path，
  workspace 排除该第三方 package；其余版本和 feature 声明保持。

补丁重放保持未改源码的原始字节，包括 CRLF；
新模块单独检查格式。独立消费者和原始版本对照见
[验证说明](../tests/consumers/harfrust-admission/README.md)。
用户坐标的无 avar／avar1 已补齐准入，见
[B1 用户坐标记录](../doc/verification/reviews/text-stages-b1-variation-2026-11-02.md)。
冷缓存基础准入现覆盖目录、cmap、Native 空 lookup 槽及 mark digest，见
[B1 冷缓存记录](../doc/verification/reviews/text-stages-b1-cold-cache-2026-11-02.md)。
lookup 缓存准入已补齐 portable eager 和 Native 显式 lazy prepare，见
[B1 lookup 记录](../doc/verification/reviews/text-stages-b1-lookup-cache-2026-11-02.md)。
空 AAT 缓存准入支持零 count；非空与截断表头仍提前拒绝，见
[B1 AAT 表头记录](../doc/verification/reviews/text-stages-b1-aat-headers-2026-11-02.md)。
无布局表的 ShapePlan 准入已覆盖 feature、排序与存储；未知路径提前返回未支持，见
[B1 计划记录](../doc/verification/reviews/text-stages-b1-plan-2026-11-02.md)。
当前生产源码核对使用 `tools/scripts/check_harfrust_plan_admission.py`；
lookup 和 AAT 脚本保留对应历史快照。
这些子片仍不覆盖 avar2、普通 shaping 内部的 `ensure`、非空 AAT、
布局表计划查询和 glyph 替换成本，不作为启用阶段缓存的依据。

```bash
rustfmt --edition 2021 --config disable_all_formatting=false \
  --check dependencies/harfrust-0.3.2/src/hb/buffer_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/instance_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/variation_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/cold_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/aat/cold_headers.rs
cargo test --manifest-path tests/consumers/harfrust-admission/Cargo.toml --lib
```
