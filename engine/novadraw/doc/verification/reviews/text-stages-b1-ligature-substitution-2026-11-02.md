# B1：连字匹配存储与重复访问准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：源码重放、完整输出、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[多字形替换](text-stages-b1-multiple-substitution-2026-11-02.md)，
为零 flags 的 `LigatureSubstFormat1` 增加操作前准入。
输入仍限定为已映射 ASCII、LTR Latin／Common／unknown、默认坐标和全局 feature。
普通入口与准入入口执行同一份连字匹配、替换与 cluster 合并内核。

## 先证明组件数量，再准备匹配位置

`match_input` 将匹配位置写入 `SmallVec<[u32; 8]>`，
原始 context 只初始化一个位置。第九个匹配位置会溢出到堆；
因此不能只核对 glyph 数组扩容，或假定匹配存储是固定数组。
本片将内部 lookup 准入 hook 增加一个匹配存储借用，
在执行 lookup 前预留并初始化本次可能使用的全部位置。
外部消费者仍通过同一个 shaping 准入入口调用，不持有内部 context。

支持检查读取实际 LigatureSet offsets 和各 Ligature 的 component count，
每次读取先准入。无效表项沿用普通内核的跳过行为。
超过 `MAX_CONTEXT_LENGTH = 64` 的模式在原始 `match_input` 中直接失败，
不会增长匹配存储；其表项尝试仍须计费。
有效模式的最长长度用于匹配容量证明，全部候选的长度和用于访问上界。

原始内核用 u8 累加 `total_component_count` 和 `comps_so_far`。
一条连字可由此前的连字组成，每个已有连字的组件属性占四位，
不能直接把本条表项的长度视为组件总数。
本片按计划顺序维护组件上界：初始为一；
每个 lookup 的最长有效模式乘以前序组件上界不能超过 255，
新结果的存储组件上界最多为 15。
单字形或多字形替换不会增大这一上界，包括保留前序连字属性的路径。
无法满足该证明时，在 shaping 前返回 `Ok(None)`，保留逻辑输入；
不进入可能产生 debug overflow 或 release wrap 的原始求和。

## 分配、初始化与访问分别收费

匹配容量足够时，只准入新增位置初始化。
溢出到堆时，以 fallible Vec exact reservation 准入整份新容量槽，
收取旧位置的读取和目标写入，再初始化新增位置。
Vec 容量超过 inline capacity 后，`SmallVec::from_vec` 直接接管其分配，
没有第二次分配。原始 `TryReserveError` 保留在标准错误链中。
u32 元素没有析构操作；替换旧存储或丢弃候选不扫描其元素。

每个实际 lookup 重新读取并准入表项计划。
费用包括 coverage 与缓存读写、seconds digest、
快路径第二字形探测、所有候选的失败匹配、组件属性和匹配位置写入。
在本片输入限制下，不存在 default-ignorable 或带跳过 flags 的 glyph；
每次匹配前进到紧邻 glyph，未知跳过路径不会进入 shaping。

连字形成时，`merge_clusters` 可能扫描相同 cluster 的后缀及已经输出的前缀。
这些范围不能假定在多次匹配之间互不重叠。
本片按当前 lookup 的输入长度与最长正长度替换计算输出前缀上界，
为重复扫描提供保守上界；同一 lookup 可能先扩张输出，再匹配连字。
准入策略分别收取 cluster 最小值、范围比较、cluster 写入、
非单调 cluster 的 flags 和尾部连字组件访问费用。
这一上界可能高于实际访问，尚未消除 cluster 合并的重复扫描成本。
费用证明与 CPU 耗时分开记录，不能把 work 数量解释为实际执行次数。

## 输出、拒绝与可复验入口

九个矩阵在原始／生产 Native／portable 四配置下签名一致。
新增连字矩阵每配置 34,560 份观察，覆盖 30 份合成字体，
包括匹配长度 1／2／8／9／17／64／65、两种 coverage、
未命中子表、多个候选与连续 lookup。
还覆盖不同方向、脚本、feature 范围、cluster、flags 和备用容量。
此前八个冻结矩阵的源码及输出签名保持；
每配置共 275,568 份，四配置共 1,102,272 份完整观察。
原始字节以无损 gzip 保存，散列和分母针对解压后的内容。

Native 与 portable 各通过 203 项依赖测试和 56 项消费者测试。
本片新增依赖测试核对 inline、spill、重复预留和每个存储拒绝点；
新增五项消费者测试核对完整输出、逐点拒绝后的 buffer 复用、
组件溢出的提前拒绝、错误 source 与粘滞预算，
以及“连字→多字形替换→连字”的属性保留。
进入 pipeline 后拒绝立即传播，清空并保留可复用 buffer；
支持检查拒绝保留输入，不执行普通 shaping 回退。

重放工具将前十五份冻结补丁逐一应用到原始归档，再追加本片补丁，
核对全部 81 份源码逐字节一致；本片仅改两份，另外 79 份保持。
最终补丁身份及每配置输出收据见
[证据目录](../evidence/text-stages-b-2026-11-02/README.md)中的
`ligature-verification.json`。
第十六份补丁 SHA-256：
`209e0168a9c05800a3b2e608f4f81e978b2eb73d354d8021b81e14725b9d44c6`。

```bash
python3 tools/scripts/check_harfrust_ligature_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_ligature_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind ligature \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_ligature_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

构建串行、关闭增量与 dev／test 调试信息，Native 使用 Rust 1.96 Darwin LLD。
Wasm 构建移除 Darwin flags，构建结果与实际执行分别记录。
成本场景使用 inline 两组件、spill 九组件及三阶段混合替换，
分别测试冷／热缓存、0／4096 glyph 备用容量和四种准入模式。
匹配 context 每次 shaping 新建；热字体缓存和足够 glyph 容量
不能免除九位置的匹配堆分配。

## 成本与静态门禁

Native release 完成 48,000 个准入成本样本，
每组 100 次预热、1,000 个正式样本，输入为 1,040 字符。
四种执行模式按样本轮换，每次核对实际费用、glyph 数和拒绝状态。
以下为无备用容量的六组结果；其余结果保存在成本收据中。

| lookup 与缓存 | 普通 P95（µs） | 计费 P95（µs） | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| 两组件、单子表，冷 | 32.375 | 35.833 | 16,402,603 | 3 | 1,171 |
| 同配置，热 | 31.750 | 35.500 | 16,402,382 | 0 | 1,146 |
| 九组件、三子表，冷 | 19.875 | 23.542 | 17,131,003 | 16 | 1,260 |
| 同配置，热 | 19.000 | 22.875 | 17,130,389 | 9 | 1,203 |
| 连字→多字形→连字，冷 | 50.625 | 55.000 | 26,179,044 | 2,248 | 1,308 |
| 同配置，热 | 49.541 | 54.625 | 26,178,538 | 2,240 | 1,243 |

三组分别输出 680／400／560 个 glyph。
最后一点拒绝与完整路径的准入次数和已付 allocation 相同，
work 少相应的最终 glyph 数；中断清理没有运行普通回退。
热 lookup 下九组件仍收取 9 个匹配容量槽，
混合替换还收取 2,240 个 glyph 数组容量槽；
有 4,096 个备用 glyph 槽时，混合替换的热 allocation 为零，
九组件仍为 9。
work 包含 cluster 重扫的二次上界，不代表该样本实际扫描了这些元素，
也不证明此保守上界适合最终 Core 分析预算。

另完成 36,000 个原始／生产普通入口对照样本，
分三轮交换执行顺序。
两组件冷缓存 P95 原始为 32.500–40.042 µs、生产为 32.125–39.458 µs；
九组件冷缓存原始为 19.750–20.166 µs、生产为 19.042–19.625 µs；
混合替换冷缓存原始为 50.416–50.875 µs、生产为 49.917–52.375 µs。
系统负载未控制，数据只记录本机成本与波动，不作为稳定加速结论。

Native／portable 消费者、准入成本和普通对照程序的严格 Clippy 通过。
从原始归档重放前十五份补丁后，对比前片与生产库的 Clippy JSON；
Native／portable 均保留六项既有诊断，新增诊断为零。
比较覆盖诊断类别、文本、源码路径及原始代码行。
Core Native／Wasm check 与 portable Wasm 构建通过；未执行 Wasm 测试。

费用补正前的源码重放、完整输出与 32,000／24,000 个成本样本，
以及成本程序的 dead-code lint 失败，保存在 `before-accounting-ligature-*`。
补正后扩充到三组成本场景并重跑全部门禁，通过收据使用当前 `ligature-*`。
五份新增 Rust 消费者文件与生产准入模块按各自 edition 定向格式检查；
四份新增或修改的 Python 脚本通过语法检查。
报告分别经过技术逻辑、事实、中文表达与节奏自审，未取得独立评审结论。

## 后续边界

上下文与递归替换、GPOS、复杂归一化、RTL 和脚本专用实际 shaping
仍待 B1 覆盖；Fontique／Parley 分析编排、导出与 Core 阶段接入
分别保留在 B2／B3／B4。
本片支持范围与完整文字分析验收分开，不能据此关闭 B1 或启用默认缓存。
