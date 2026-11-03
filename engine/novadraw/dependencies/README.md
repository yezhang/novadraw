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
  再应用[ShapePlan 增量补丁](harfrust-0.3.2-plan.patch)，
  再应用[配置增量补丁](harfrust-0.3.2-configuration.patch)，
  再应用[布局表计划增量补丁](harfrust-0.3.2-plan-layout.patch)，
  再应用[私有语言标签增量补丁](harfrust-0.3.2-private-language.patch)，
  再应用[通用语言映射增量补丁](harfrust-0.3.2-general-language.patch)，
  再应用[脚本规划回调增量补丁](harfrust-0.3.2-script-callbacks.patch)，
  再应用[非扩张塑形增量补丁](harfrust-0.3.2-shape.patch)，
  再应用[单字形替换增量补丁](harfrust-0.3.2-single.patch)，
  再应用[多字形替换增量补丁](harfrust-0.3.2-multiple.patch)，
  再应用[连字替换增量补丁](harfrust-0.3.2-ligature.patch)，
  再应用[上下文替换增量补丁](harfrust-0.3.2-context.patch)，
  再应用[上下文堆存储增量补丁](harfrust-0.3.2-context-spill.patch)，
  再应用[GPOS 单字形定位增量补丁](harfrust-0.3.2-gpos-single.patch)，
  再应用[GPOS 配对定位增量补丁](harfrust-0.3.2-gpos-pair.patch)，
  再应用[ASCII RTL 增量补丁](harfrust-0.3.2-rtl.patch)，
  再应用[简单 Unicode 增量补丁](harfrust-0.3.2-simple-unicode.patch)，
  再应用[GDEF 属性增量补丁](harfrust-0.3.2-gdef.patch)，
  再应用[一般字符增量补丁](harfrust-0.3.2-general-scalar.patch)，
  再应用[一对一规范化增量补丁](harfrust-0.3.2-normalization.patch)，
  再应用[空格与连字符回退增量补丁](harfrust-0.3.2-space.patch)，
  再应用[分解后第二字符缺字增量补丁](harfrust-0.3.2-decomposition-miss.patch)，
  再应用[共享规范化内核增量补丁](harfrust-0.3.2-normalizer-kernel.patch)，
  再应用[dotted-circle 输出与同步增量补丁](harfrust-0.3.2-dotted-circle-kernel.patch)，
  最后应用[grapheme 合并与反向分组增量补丁](harfrust-0.3.2-grapheme-kernel.patch)；
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
配置准入覆盖借用字体表和坐标扫描；未知 AAT count 提前返回未支持，见
[B1 配置记录](../doc/verification/reviews/text-stages-b1-configuration-2026-11-02.md)。
布局表计划准入已覆盖 GSUB／GPOS 的 script、feature、变体替换与 lookup 收集，见
[B1 布局表计划记录](../doc/verification/reviews/text-stages-b1-plan-layout-2026-11-02.md)。
私有 `hbot` 语言标签的扫描与计划语言存储已接入准入，见
[B1 私有语言记录](../doc/verification/reviews/text-stages-b1-private-language-2026-11-02.md)。
通用主语言、扩展语言及复合标签已接入同一准入内核，见
[B1 通用语言记录](../doc/verification/reviews/text-stages-b1-general-language-2026-11-02.md)。
Arabic／Syriac／Hangul 规划回调已接入共享准入内核及可失败数据存储，见
[B1 脚本回调记录](../doc/verification/reviews/text-stages-b1-script-callbacks-2026-11-02.md)。
已接入非扩张 LTR ASCII 塑形准入与中断 buffer 恢复，见
[B1 塑形记录](../doc/verification/reviews/text-stages-b1-shaping-2026-11-02.md)。
已接入零 flags 的 GSUB SingleSubst format 1／2 与完整 lookup 缓存准入，见
[B1 单字形替换记录](../doc/verification/reviews/text-stages-b1-single-substitution-2026-11-02.md)。
已接入零 flags 的正长度 MultipleSubst，按 sequence 长度证明增长上界，
并在 lookup 前准入双数组扩容与输出复制，见
[B1 多字形替换记录](../doc/verification/reviews/text-stages-b1-multiple-substitution-2026-11-02.md)。
零 flags 连字匹配的组件边界、匹配位置存储与重复访问已接入准入，见
[B1 连字替换记录](../doc/verification/reviews/text-stages-b1-ligature-substitution-2026-11-02.md)。
已接入 inline Format 3 上下文的嵌套单字形替换、移动与重复访问准入，见
[B1 上下文替换记录](../doc/verification/reviews/text-stages-b1-context-substitution-2026-11-02.md)。
Format 3 上下文已扩展至 64 位置，堆匹配数组与备份在 lookup 前准入并预留，
组件求和按此前连字上界检查；实施与验证范围见
[B1 上下文堆存储记录](../doc/verification/reviews/text-stages-b1-context-spill-2026-11-02.md)。
GPOS SinglePos format 1／2 的零 flags 标量定位已接入同一策略，
lookup 前覆盖查表与 position 读写，定位本身不新增存储；见
[B1 单字形定位记录](../doc/verification/reviews/text-stages-b1-gpos-single-2026-11-02.md)。
GPOS PairPos format 1／2 的零 flags 标量定位已接入同一策略，
pair-set／class 查询、安全标记和矩阵寻址均在执行前准入。
原始 ValueFormat 保留位与非法 class 值在 pipeline 前返回未支持；见
[B1 配对定位记录](../doc/verification/reviews/text-stages-b1-gpos-pair-2026-11-02.md)。
ASCII RTL 的原生方向反转、镜像查询与最终反转已接入同一策略，
真实目标方向在进入 pipeline 前保存；见
[B1 RTL 记录](../doc/verification/reviews/text-stages-b1-rtl-2026-11-02.md)。
已映射的 Ll／Lu／Lt／Lo／Zs 字符及 Han 默认 shaper 已接入准入，
六种 cmap reader、MacRoman 与 Symbol 映射的重复访问各自计费；见
[B1 简单 Unicode 记录](../doc/verification/reviews/text-stages-b1-simple-unicode-2026-11-02.md)。
GDEF glyph／mark 分类、替换后再查询和 mark 零宽读写已接入准入；
完整输出、成本与跨目标定向门禁通过，见
[B1 GDEF 记录](../doc/verification/reviews/text-stages-b1-gdef-2026-11-02.md)。
已映射一般字符的非 early-return 属性判断也已接入准入，
继续排除 mark、特殊 continuation、分数斜线与缺字规范化。
二十四份补丁重放、十七矩阵四配置共 4,569,024 份完整观察、
192,000 条准入成本、144,000 条普通对照和跨目标门禁通过，见
[B1 一般字符记录](../doc/verification/reviews/text-stages-b1-general-scalar-2026-11-02.md)。
缺字 `.notdef` 与 canonical one-to-one 规范化已接入准入，
覆盖分解与映射的重复访问、独立输出数组切换和前缀／后续复制。
二十五份补丁精确重放，十八矩阵四配置共 5,453,760 份完整观察一致，
Native／portable 各 204 项依赖内部测试与 94 项消费者测试通过；见
[B1 一对一规范化记录](../doc/verification/reviews/text-stages-b1-singleton-normalization-2026-11-02.md)。
192,000 条准入成本、144,000 条普通对照与跨目标定向门禁也已通过，
库 Clippy 新增诊断为零，Wasm 只构建未执行测试。
[缺字空格与 U+2011 回退](../doc/verification/reviews/text-stages-b1-space-fallback-2026-11-02.md)
已覆盖原始字符回退、重复映射及 GSUB 后实际长度的空格定位费用。
二十六份补丁精确重放，十九矩阵四配置共 5,945,280 份完整观察一致；
内部 invisible 普通参考另有 160 份一致观察。
Native／portable 各 205 项依赖内部测试与 98 项消费者测试、
192,000 条准入成本、144,000 条普通对照和十三项定向门禁通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。
此前生产源码核对使用 `tools/scripts/check_harfrust_space_admission.py`；
此前脚本保留对应历史快照。
[分解后第二字符缺字](../doc/verification/reviews/text-stages-b1-decomposition-miss-2026-11-03.md)
已接入 a、b 独立查询及缺字终止后的原始字符回退；b 命中仍未支持。
二十七份补丁精确重放；二十矩阵四配置共 6,338,496 份完整观察一致。
旧原始版本 38 份配置逐字节核对后复用，生产及新增配置重新执行。
Native／portable 各 205 项依赖内部测试与 102 项消费者测试、
192,000 条准入成本、144,000 条普通对照和十三项定向门禁通过；
库 Clippy 新增诊断为零，Wasm 只构建未执行。
此前生产核对入口是 `tools/scripts/check_harfrust_decomposition_miss_admission.py`，
保留用于对应冻结快照。
[共享规范化内核](../doc/verification/reviews/text-stages-b1-normalizer-kernel-2026-11-03.md)
已接入分解、排序、重组、输出增长及规范化后实际长度的准入。
二十八份补丁精确重放；二十一矩阵四配置共 8,796,096 份完整观察一致。
Native／portable 各 211 项依赖内部测试与 102 项消费者测试通过。
此前生产核对入口是 `tools/scripts/check_harfrust_normalizer_kernel_admission.py`；
192,000 条准入成本、144,000 条普通 singleton 对照、
144,000 条普通 mark 对照及十四项定向门禁通过。
两处纯格式修正经过严格 Rustfmt 证明，全部普通输出逐字节复核，
定向测试和成本重新执行。库 Clippy 新增诊断为零，Wasm 只构建未执行。
[dotted-circle 输出与同步](../doc/verification/reviews/text-stages-b1-dotted-circle-kernel-2026-11-03.md)
已接入圆圈判定、映射、输出和未消费后缀复制的操作前准入。
二十九份补丁精确重放；二十二矩阵四配置共 10,786,752 份完整观察一致。
Native／portable 各 214 项内部测试与 102 项消费者测试通过。
192,000 条准入、144,000 条普通 singleton、144,000 条普通 mark
及 288,000 条圆圈普通成本样本、十一项边界检查通过。
七份采样前 Clippy 收据和十九份原始采样文件已复核；
库 Clippy 新增诊断为零，Wasm 只构建未执行。
该片冻结快照的核对入口是 `tools/scripts/check_harfrust_dotted_circle_kernel_admission.py`。
[grapheme 合并与反向分组](../doc/verification/reviews/text-stages-b1-grapheme-kernel-2026-11-03.md)
已完成共享内核、定向测试、完整普通输出对照和跨目标检查。
三十份补丁精确重放；Native 与 portable 各 218 项内部测试、102 项消费者测试通过。
768,000 条成本样本已记录，十一项边界检查通过，库 Clippy 新增诊断为零；
Wasm 只构建未执行，普通路径尾延迟波动不作为性能通过依据。
当前生产核对入口为 `tools/scripts/check_harfrust_grapheme_kernel_admission.py`。
公共组合输入与生产缓存默认启用保持未完成。
这些子片仍不覆盖 avar2、普通 shaping 内部的 `ensure`、非空 AAT、
其余脚本专用计划回调、递归替换和其他 GPOS 成本，不作为启用阶段缓存的依据。

```bash
rustfmt --edition 2021 --config disable_all_formatting=false \
  --check dependencies/harfrust-0.3.2/src/hb/buffer_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/instance_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/variation_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/cold_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/aat/cold_headers.rs \
  dependencies/harfrust-0.3.2/src/hb/ot/query_admission.rs \
  dependencies/harfrust-0.3.2/src/hb/grapheme_kernel_tests.rs
cargo test --manifest-path tests/consumers/harfrust-admission/Cargo.toml --lib
```
