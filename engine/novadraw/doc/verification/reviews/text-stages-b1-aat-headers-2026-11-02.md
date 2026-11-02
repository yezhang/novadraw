# ADR-059 B1：空 AAT 缓存的表头准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：空 AAT 缓存子片已实现；Native／portable 对照通过。
非空 AAT、ShapePlan 与 shaping 继续 B1；B1 和默认启用未关闭。

本片接续 [lookup 缓存准入](text-stages-b1-lookup-cache-2026-11-02.md)，
沿用用户已批准的[分析准入范围](../../design/architecture/text-analysis-admission-proposal.md)。
此前所有 AAT 表都在 reader 前返回未支持。现在允许缓存表缺失或
标量 count 为零的输入；非零和截断表头仍提前返回 `Ok(None)`。

## 表头检查与返回

受准入入口依次查询 `morx`、`kern`、`kerx` 的目录记录。
表缺失时继续；存在时先准入，再读取以下标量：

| 输入 | 本片读取 | work |
|---|---|---:|
| morx | offset 4 的 u32 chain count | 4 |
| kerx | offset 4 的 u32 subtable count | 4 |
| OpenType kern | offset 0 的 u16 dispatch，offset 2 的 u16 count | 2 + 2 |
| Apple kern | offset 0 的 u16 dispatch，offset 4 的 u32 count | 2 + 4 |

目录查询沿用既有计费；每次标量读取前分别回调。
只要任一 count 非零或读取失败，就返回未支持，不进入变长 reader。
所有表均缺失或 count 为零时，直接返回三个空 Vec 的 `AatCache`。
这个路径没有 glyph 集合、class cache 或堆分配，也无需查询 `maxp`。
表头后的任意字节不参与缓存构造；测试包含 1,024 字节的损坏尾部。

判断依据来自锁定 read-fonts 0.35.0 的 `generated_morx.rs`、
`generated_kerx.rs`、`generated_kern.rs`、`tables/kern.rs` 与 `read.rs`：
morx／kerx 的 `total_len_for_count` 在 count 为零时不执行循环；
kern iterator 先检查 count，再读取子表长度。
kern 的 dispatch 与锁定 reader 一致，不额外要求某个版本号或字体名称。
这些传递依赖源码保持不变。

普通 `AatCache::new` 保留原 reader、子表集合和缓存构造。
受准入路径只在证明结果为空后返回同一空缓存表示。
回调拒绝保留原始 source，停止后续检查，不发布候选；
失败后重新构造会得到同一准入 trace，普通 shaping 仍可用。

非空 AAT 的费用仍未证明。morx／kerx 顶层 reader 会扫描变长记录，
后续还有 state-table／lookup 的 glyph 集合构造；
kern 子表缓存也会构造 glyph 集合和堆上的 class cache。
不能用零 count 的证明覆盖这些操作，也不在未知构造后补收费。
本片保留这个明确拒绝边界，下一步推进 ShapePlan。

## 验证与来源

[AAT receipt](../evidence/text-stages-b-2026-11-02/aat-verification.json)
依次重放 buffer、instance、variation、cold、lookup 五份冻结补丁，
再追加 `harfrust-0.3.2-aat-headers.patch`。
两个既有源码文件改动，新增一个模块；其他 75 个源码文件逐字节保持。
补丁重放精确恢复生产源码。

| 范围 | 结果 |
|---|---|
| 原始／生产 Native／portable | 四组各 207 个字体输入、两遍、六组文字与双方向，共 4,968 份输出，散列一致 |
| 外部 Native／portable | 各 24 项通过；新增测试覆盖四种空表头、损坏尾部、所有截断前缀、非零 count、逐点拒绝和精确重试 trace |
| 私有 Native／portable | 各 196 项通过 |
| Core Native／Wasm | `novadraw --lib` check 通过 |
| Wasm 消费者 | portable library 构建通过；未执行 Wasm 测试 |
| Clippy／格式 | Native library 与成本 binary、portable library 无 warning；新增模块与消费者源码定向格式检查通过 |

对照观察 glyph、position、cluster 和 serialization；
shaping 使用普通入口，这些结果不代表 shaping 已受准入。
207 个输入包含既有 171 个输入与 36 个 AAT 表头／尾部输入。
原始 receipt、源码散列及日志保存在同一 evidence 目录。

成本采样记录四种空 AAT 表头的完整 Native `ShaperData` 冷构造。
普通入口、准入空回调、准入计数回调和最后点拒绝各采样 1,000 次，
每组预热 100 次，模式逐样本轮换，共 16,000 个有效样本。
成功释放在计时之外，拒绝包含候选清理。
四种计数路径的新增 heap allocation 均为零；费用包含整个冷构造，
不是只计上述表头读取。
原始样本和分母见 [成本 receipt](../evidence/text-stages-b-2026-11-02/aat-cost-summary.json)。

| 空表头 | 普通 P95（µs） | 准入空回调 P95（µs） | 准入计数 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|---:|
| OpenType kern | 0.625 | 0.584 | 0.750 | 0.542 |
| Apple kern | 0.667 | 0.708 | 0.625 | 0.583 |
| morx | 0.708 | 0.708 | 0.916 | 0.625 |
| kerx | 0.375 | 0.416 | 0.500 | 0.458 |

计数路径 work 分别为 573、575、573、573，回调分别为 75、75、74、74。
采样没有控制整机负载，普通 Apple kern 最大值为 230.708 µs，
不能将这些尾值归因为准入机制，也不据此声明 Core 排版、
宽度复用或 GPU 性能改善。

成稿已按技术结构和中文表达规范自审，未取得独立评审结论。
