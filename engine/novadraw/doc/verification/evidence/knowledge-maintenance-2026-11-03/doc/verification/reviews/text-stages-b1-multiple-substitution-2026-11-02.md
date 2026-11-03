# B1：多字形替换的增长与访问准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖；
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：源码重放、完整输出、消费者、成本与定向构建门禁通过。
生产阶段缓存保持默认关闭，B1 未关闭。

## 在 lookup 执行前证明容量

本片沿用 [SingleSubst](text-stages-b1-single-substitution-2026-11-02.md)
的共享替换内核与输入限制，增加零 flags 的 `MultipleSubstFormat1`。
单次匹配消费一个输入，产生所选 sequence 的 glyph 数量。
对一个 lookup，输出长度上界为当前输入长度乘以最长有效 sequence 长度；
未命中的输入保留一个 glyph，因此乘数至少为一。

支持检查先准入表头读取，再取得借用的 sequence offsets；
每个 offset 的解析与 sequence count 读取也先准入。
读取长度不遍历 glyph payload，最长长度来自实际表数据。
无效 sequence 沿用上游跳过行为；有效零长度 sequence 进入删除路径，
本片在 shaping 前拒绝。上下文与递归 lookup 继续拒绝。

支持检查按计划中的执行顺序累乘增长上界，并与原有 `enter()` 的
长度限制比较。无法表示或超过限制时返回 `Ok(None)`，保留输入。
这一步避免 pipeline 中途才发现后续 lookup 的容量无法保证。
最长 sequence 是容量证明，不是预先执行替换，也不是字体身份白名单。

进入实际 lookup 前重新收取表数据访问费用，按当前长度计算目标容量，
调用既有 buffer 准入预留机制。两个数组的整份新增容量、潜在旧元素迁移、
新增元素初始化分别收费。共享内核后续 `ensure()` 应始终命中已初始化容量，
不能再次进入未受准入的扩容。

## 复制和组件访问也要计费

扩张第一次超出原地输出时，`make_room_for` 将已输出前缀复制到备用数组。
这是一次前缀读取和一次目标写入，不能算作常数切换。
随后未命中 glyph 的 `next_glyph`／`next_glyphs` 也可能复制输入到输出。
`sync` 转移两份 POD 数组的所有权，不扫描其元素。

每个组件还会读取 sequence glyph、访问当前 glyph 的组件属性、
更新 glyph class 与 digest、复制源记录并写入输出 glyph。
这些访问按输出上界分别准入；子表尝试和 coverage 搜索仍按输入上界计费。
连续 lookup 使用前一 lookup 的实际输出长度重新计算。
费用对应算法中的具体访问，不使用统一的每字符费用估计任意 shaping。

正长度替换保留输入的 Unicode 属性。已映射 ASCII 的输出仍不含 Unicode mark、
hidden 或 default-ignorable，因此 fallback mark positioning 不进入字体 extents
与 mark 放置路径；该结论依据复制的属性，而不是 glyph 数与字符数相等。

## 拒绝与验证范围

支持检查拒绝保留逻辑输入；进入 pipeline 后任一步拒绝立即停止，
沿用已预付的清理并保留可复用 buffer。Admission 错误保留原始错误，
数组预留错误保留 `TryReserveError` source，不执行普通 shaping 回退。

消费者矩阵已接入，覆盖 sequence 长度 1／2／3／17、两种 coverage、
未命中子表、连续 lookup、已有 spare capacity、冷／热缓存及逐点拒绝。
另接入原始／生产普通输出矩阵，覆盖零长度删除、方向、脚本、
feature 范围、cluster 和 flags。最终通过证据以 `multiple-verification.json`
及后续成本／门禁收据为准，不能用首轮定向测试替代整片验收。

## 实际改动与复验入口

生产补丁只改 `shape_admission.rs` 和 `buffer_admission.rs`。
前者增加 `ShapeOperation::Buffer`，直接传递输入准入的 work／allocation；
后者开放 crate 内的 `ensure_admitted`，继续使用同一份精确容量预留逻辑。
普通入口仍调用原始 Apply 内核；新增准入入口不预先模拟替换结果。

实际 lookup 会再读取 sequence 长度并准入，两次表访问分别计费。
第一次获得全计划的增长上界；第二次按当前实际输入长度预留。
预留随后暴露并初始化两个数组的新长度，Position 阶段因此按整份
已初始化数组收费，而不是只按最终 glyph 数收费。

成本矩阵包括 960 字符、两组 lookup 配置、冷／热缓存、0／4096 spare capacity
以及 ordinary／noop／count／reject-last 四种模式。
热 lookup 不意味着新建 buffer 的增长免费；
只有已有足够 spare capacity 且 lookup 已热时，才预期 allocation 为零。
最后一点拒绝保留与完整运行相同的已付容量，未付工作为最后一轮输出 flags 写入。

重放工具保存八个普通输出矩阵的四配置原始字节，采用无损 gzip；
散列和观测数量针对解压后的内容。已有冻结补丁和历史输出保持。

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

所有 Cargo 构建串行、关闭增量与 dev／test 调试信息，
沿用固定包身份。由于本机 clang／xcrun 执行异常，Native 使用 Rust 1.96
自带 Darwin LLD；Wasm 构建移除 Darwin 链接 flags，不执行 Wasm 测试。
这些隔离采样只测本片准入开销，不能据此宣称 Core 长文或 GPU 性能提升。

## 已完成的输出与成本核对

前十四份补丁的 SHA-256 保持，追加的第十五份补丁只改两份源码；
另外 79 份源码不变，全部 81 份源码逐字节重放一致。
本片补丁 SHA-256：
`8f5f15be583a4b6437a0a4b000c886b7b251050a12b586c15418d7def19459ea`。

八个矩阵在原始／生产 Native／portable 四种配置下签名一致。
新增 MultipleSubst 矩阵每配置 36,864 份观察，
包括 32 份合成字体与每字体 1,152 组输入；
此前七个矩阵的源码与完整输出散列保持。
合计每配置 241,008 份，四配置共 964,032 份观察。
Native 与 portable 各通过 202 项私有测试、51 项消费者测试，失败为零。
新增五项消费者测试覆盖扩张与连续 lookup、逐点拒绝、
删除／超限的提前拒绝、原始 error source 与粘滞预算、
畸形 sequence 跳过以及非零 lookup flags 拒绝。

Native release 完成 32,000 个准入成本样本。
每组 100 次预热、1,000 个样本，四种执行模式按样本轮换；
每个样本核对实际费用、输出 glyph 数和拒绝状态。

| lookup 配置与缓存 | spare | 普通 P95（µs） | 计费 P95（µs） | work | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|---:|
| max2／coverage1／单子表、单 lookup，冷 | 0 | 28.125 | 32.417 | 109,788 | 3,842 | 1,077 |
| 同配置，热 | 0 | 26.708 | 30.834 | 109,724 | 3,840 | 1,060 |
| 同配置，冷 | 4096 | 31.000 | 33.750 | 127,196 | 2 | 1,075 |
| 同配置，热 | 4096 | 30.708 | 33.458 | 127,132 | 0 | 1,058 |
| max3／coverage2／三子表、双 lookup，冷 | 0 | 58.042 | 66.958 | 372,063 | 13,448 | 1,234 |
| 同配置，热 | 0 | 57.084 | 70.208 | 371,837 | 13,440 | 1,180 |
| 同配置，冷 | 4096 | 60.167 | 62.792 | 372,191 | 8 | 1,230 |
| 同配置，热 | 4096 | 62.958 | 66.208 | 371,965 | 0 | 1,176 |

两组配置分别输出 1,200／1,680 个 glyph。
最后准入点拒绝的费用比完整路径少相应的 1,200／1,680 work，
准入次数和已付 allocation 相同。
新建 buffer 即使 lookup 已热，也分别收取 3,840／13,440 个整份新增容量槽；
足够 spare capacity 加热 lookup 才出现 allocation 为零。
清理保留可复用 buffer，未执行普通路径回退。

另完成 24,000 个原始／生产普通入口样本，
分三轮按原始／生产、生产／原始、原始／生产交换顺序。
简单冷缓存 P95 原始 27.458–34.291 µs、生产 27.041–33.000 µs；
简单热缓存原始 26.750–27.875 µs、生产 26.416–27.750 µs；
双 lookup 冷缓存原始 57.833–60.959 µs、生产 57.792–58.459 µs；
双 lookup 热缓存原始 56.917–57.833 µs、生产 56.417–57.209 µs。
系统负载未控制，这些范围记录本机波动，不作为稳定加速或退化结论。

收据和原始数据位于[证据目录](../evidence/text-stages-b-2026-11-02/README.md)：
`multiple-verification.json`、`multiple-cost-summary.json`、
`multiple-cost.ndjson`、`multiple-ordinary-cost-summary.json`、
`multiple-ordinary-*.ndjson` 与完整输出 `.log.gz`。
中断的未压缩输出另存为 `before-compression-multiple-*`；
其内容不作为最终通过收据。沙箱回收过临时重放副本中的一个源码文件，
后续从冻结补丁重建副本并完成重放，生产源码保持。

## 静态与跨目标门禁

Native／portable 消费者、准入成本程序与普通对照程序的
`Clippy -D warnings` 通过，Core Native／Wasm check 与 portable Wasm
构建通过。Wasm 没有执行测试。
生产改动按上游 Edition 2021、五份新增消费者／oracle／cost 文件按
Edition 2024 通过定向格式检查。
整包格式仍保留前片记录的模块排序与旧成本程序差异，
本片不声明整个消费者包格式检查通过。

直接 lint 第三方库时，六项诊断涉及前片的长度比较／默认值替换、
上游生成表中的两个折叠 match 提示及两个取模提示。
首次 `-D warnings` 失败日志保存为
`before-baseline-comparison-multiple-clippy-native.log`。
随后重新从原始归档重放十四份冻结补丁，分别运行前片与生产的
Native／portable Clippy JSON 输出，比较诊断类别、文本、源码路径和原始代码行。
两种配置均为六项既有诊断、新增诊断为零；
没有豁免新诊断，也未改冻结源码来消除上游写法提示。
完整比较、样本／输出 receipt 核对与定向构建命令记录于 `multiple-gates.json`。

报告成稿经过技术结构与中文表达自审，保留归属、费用单位、
构建差异及结论限制；本次没有取得独立评审结论。

## 本片边界与后续

仍仅支持已映射 ASCII、LTR Latin／Common／unknown、默认坐标、
全局 feature 和零 flags 的 Single／正长度 MultipleSubst；
GDEF、GPOS、AAT、删除、连字／递归 lookup、复杂归一化和 RTL
的完整准入仍待覆盖。下一步继续 B1 的连字匹配与临时匹配存储，
再推进 Fontique／Parley 分析编排、导出和阶段适配。
本片不关闭 B1，不启用生产宽度复用缓存。
