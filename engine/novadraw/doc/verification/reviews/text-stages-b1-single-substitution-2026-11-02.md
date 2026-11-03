# B1：单字形替换的操作前准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖；
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：双数组访问费用补正已通过上游重放、消费者、成本和定向静态门禁。
B1、B2–B4、C／D 未关闭，生产阶段缓存仍默认关闭。

## 从无布局表扩大到 SingleSubst

[前一子片](text-stages-b1-shaping-2026-11-02.md)只允许无布局表的 LTR ASCII。
本片扩大同一个 `try_shape_with_plan_admitted` 入口：
选中的 GSUB lookup 可以包含 SingleSubst format 1／2，
coverage 可以使用 format 1／2；lookup flags 必须为零。
已映射 ASCII、LTR Latin／Common／unknown、默认坐标、全局 feature、
无 GDEF／GPOS／AAT 的条件保持。
判断依据是选中算法及输入，未建立字体身份白名单。

SingleSubst 每次消费一个输入并产生一个输出。
因此该路径可以保留原有 buffer 容量，使用原地 output，
不进入扩张、删除、递归匹配或外部上下文缓存。
未选中的 lookup 不执行，不因字体含有其他未选中 lookup 而拒绝。
选中的非 SingleSubst、非零 lookup flags 或复杂脚本仍在 shaping 前返回
`Ok(None)`，保留逻辑输入。
上游忽略的损坏子表仍遵循其原有跳过行为。

普通与准入入口共享 `apply_layout_table_with`、`apply_string`、`apply_forward`
和原有 SingleSubst 内核。`ShapeAdmission::before_lookup` 在实际获取与应用
lookup 之前调用，普通策略使用可消除的空回调。
不复制替换算法，也不先执行一次普通 shaping 来获得成本。

## 按访问责任计费

支持检查通过既有 `ColdAdmission` 进入选中 lookup 的完整缓存构造。
Native lazy cache 的读取、子表展开、coverage digest、集合容量、
初始化和清理费用通过 `ShapeOperation::Cold` 进入同一个回调。
portable 的缓存已经在 data 构造时生成；本入口仍收取缓存访问费用，
不重复收取已经完成的存储费用。data、instance、plan 构造仍使用各自准入入口。
预算拒绝保留原始 `Admission(E)`；
缓存预留失败通过 `Storage(TryReserveError)` 保留 source。

进入实际 lookup 前，再访问完整缓存并分别准入以下遍历：

| 访问 | 上界及源码依据 |
|---|---|
| 选中 lookup 的输入筛选 | 每个 glyph 至多经过一次 forward skip scan；lookup digest 有三个固定 mask |
| 属性及输出 | 当前 glyph、属性读取、属性写入和 output glyph 写入分别计费；新 glyph digest 有三个 mask |
| 子表尝试 | 按当前 glyph 数与实际子表数计费，包括 coverage 未命中的尝试 |
| 子表内当前 glyph 读取 | 每次子表调用重新执行 `cur(0)`，单独收取该重复访问 |
| 表头及 coverage 搜索 | 读取固定生成表头、coverage offset／format／count，再按实际 coverage 长度给出二分搜索上界 |
| format 1／2 的替换值 | delta 或 substitute-array 读取属于各次尝试的表访问，不能只计算 coverage |

`coverage_search_work` 根据 coverage 元素数的 `bit_length(n) + 1`
比较上界计费。format 1 比较 glyph 元素；format 2 比较 range 的端点并读取
最终 coverage 索引。固定表头读取先准入，再构造借用 reader；
获取搜索长度本身不会扫描 coverage。这里核对的是锁定的 read-fonts 0.35.0
helper，未修改其生产源码。

没有使用统一每字符倍数估计任意 shaping。
各笔费用对应已支持算法的一轮访问；实际子表数改变时费用随之变化。
未命中子表可能短路，但已准入的保守费用不退回。
SingleSubst 的 `MatchPositions` 保持一个 inline 元素，
不会启动上下文匹配或 spill。

## 拒绝与恢复

冷缓存只提交完整候选。支持检查失败时，已经完成的缓存允许留存，
逻辑输入仍保持；不会发布半成品，也不会转入普通 shaping 绕过预算。

进入 pipeline 后，任意 lookup 准入失败立即沿 `Result` 传播，
不继续后续 lookup、position 或 flags。
沿用前片已预付的恢复：释放 variable slots、恢复原有上限、
清空逻辑内容并保留可用容量。调用者可以重新填入输入并执行普通排版。
此处不承诺回滚已完成的性能缓存。

## 已完成的定向验证和成本采样

`check_harfrust_single_admission.py` 重放前十三份冻结补丁，
再应用 `harfrust-0.3.2-single.patch`：
相对前片修改四份源码，另外 77 份源码不变，全部 81 份源码逐字节重放一致。
本增量补丁 SHA-256：
`254c766528a2ed83b06ba18e83afc8f4b20a1c74ee24760c2df71c15ed000716`。
该散列与 `single-verification.json` 对应双数组访问费用补正后的通过版本。
补正将 digest 相交从三个 mask 访问改为两组三个 mask，共六次访问；
修改计费，不修改替换算法。
空间不足后，串行低调试构建又在 clang 执行处遇到 I/O 错误。
基础命令和链接器文件仍在；未修改工具链。
Rust 1.96 自带 LLD 使用稳定的 `linker-flavor=ld64.lld` 完成了本机编译与运行烟测；
验证工具新增 `--linker rust-lld` 选项；本次通过收据使用该链接器，
低调试配置、单构建进程，不再依赖当前无法执行的 clang。

原始／生产、Native／portable 四配置中的七组完整输出签名一致。
新 SingleSubst 矩阵每配置 18,432 份观察；
既有六矩阵的输入源码与输出签名保持。
合计每配置 204,144 份观察，四配置共 816,576 份。
两种配置各通过 202 项私有测试及 46 项外部消费者测试，失败为零。

消费者新增三项测试：两种替换格式、两种 coverage、未命中子表及连续 lookup；
冷／热准入序列逐点拒绝后的停止与恢复；
未支持的选中 lookup 在实际执行前拒绝。
普通与准入结果同时比较 glyph、position、cluster 和 flags。
补正后的 Native／portable Clippy、成本程序 Clippy、portable Wasm 构建、
Core Native／Wasm check 均通过。文档检查通过 513 份 Markdown，
冻结补丁、七份 oracle 输入源码和全部原始成本数据的散列核对通过。
Wasm 仅构建，没有执行 Wasm 测试。

受影响的依赖模块按上游 Edition 2021、消费者按 Edition 2024
通过定向格式检查。整个消费者包的格式检查仍命中既有 `lib.rs`
模块排序与 `lookup_cost.rs` 格式差异；本片保持冻结旧子片，
不声明整个消费者包的格式门禁通过。

补正后 Native release 完成 16,000 个有效样本。
首次成本重采在 `proc-macro2` 构建时再次耗尽空间；
包级 dry-run 确认 oracle 有 4,324 个重复构建文件、合计 2.6 GiB，
随后仅清理可重建的 oracle 包缓存，释放约 2.5 GiB，串行重采通过。
每组 960 个字符、100 次预热、1,000 个样本；
普通、空回调、计费和最后点拒绝按样本轮换。
字体 data、plan、输入准备以及 hot cache 预热在计时之外，
lookup 冷构造和失败清理在计时之内。

| 配置 | 普通 P95（µs） | 计费 P95（µs） | 计费 work | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| format 1／coverage 1／单子表、单 lookup，冷缓存 | 22.708 | 26.000 | 69,415 | 2 | 1,049 |
| 同配置，热缓存 | 18.208 | 20.917 | 69,353 | 0 | 1,032 |
| format 2／coverage 2／三子表、双 lookup，冷缓存 | 26.500 | 29.000 | 188,737 | 8 | 1,130 |
| 同配置，热缓存 | 25.459 | 28.333 | 188,511 | 0 | 1,076 |

`allocation` 是整个新增容量槽数，不是字节。
逐样本成本一致；最后点拒绝保留同一容量费用和准入次数，
已完成 work 比成功路径少 960。
相对补正前，单 lookup 的 work 增加 3，双 lookup 增加 6；
容量费用和准入次数不变。
fixture 是用于算法对照的合成字体，数据不代表生产字体、Core 布局或 GPU 性能。
系统负载未受控制，本片没有宣称端到端性能改善。

另完成 24,000 个原始／生产普通入口样本，分三轮交换执行顺序：
简单冷缓存 P95 原始 18.958–19.667 µs、生产 18.333–25.125 µs；
简单热缓存原始 18.583–19.166 µs、生产 18.542–18.833 µs；
双 lookup 冷缓存原始 26.542–27.000 µs、生产 26.375–29.084 µs；
双 lookup 热缓存原始 25.709–25.917 µs、生产 25.542–26.125 µs。
该对照记录普通路径的本机波动，不作为稳定加速或退化的结论。

原始数据和汇总位于
[证据目录](../evidence/text-stages-b-2026-11-02/README.md)中的
`single-cost.ndjson`、`single-cost-summary.json`、
`single-ordinary-*.ndjson` 与 `single-ordinary-cost-summary.json`。
报告成稿经过技术结构和中文表达自审，保留构建配置、计费单位及结论限制；
本次未取得独立评审结论。

## 剩余工作

下述为本片冻结时的边界；后续 MultipleSubst 的接入与验证见
[多字形替换记录](text-stages-b1-multiple-substitution-2026-11-02.md)。

当前仍不支持实际 MultipleSubst、ligature、context、GPOS、复杂归一化与 RTL
的完整准入。下一步从 MultipleSubst 的实际 sequence 长度确认增长上界，
在 lookup 执行前预留两个数组并覆盖潜在迁移、初始化、output 前缀复制和重复访问。
零长度删除与递归路径须分别完成证明，不能套用 SingleSubst 的费用。
之后继续 Fontique／Parley 分析编排与导出；本片不关闭 B1。
