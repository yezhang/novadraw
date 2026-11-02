# ADR-059 B1：LookupInfo 与子表缓存准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：lookup 子片已实现并通过 Native／portable 定向验证。
AAT、ShapePlan 与 shaping 准入继续 B1；B1 未关闭。
Core 阶段适配、B2–B4、C／D 和默认启用仍待完成。

本片接续[冷缓存基础准入](text-stages-b1-cold-cache-2026-11-02.md)，
补齐其 portable 非零 lookup 拒绝分支与 Native 后续 lazy 初始化。
实施范围沿用用户已批准的[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)。
原始冻结补丁与 receipt 保持，当前生产源码另追加 lookup 增量。

## 构造、查询与拒绝

`ShaperData::try_new_admitted` 现在允许 portable 展开非零 GSUB／GPOS lookup。
它与普通构造共用 LookupInfo 和子表缓存内核；先准入、fallibly 预留，
再写入已完成的候选。Native 仍只在冷构造时初始化空槽。

新增 `ColdLookupTable` 和
`ShaperData::try_prepare_lookup_admitted(font, table, index, admit)`。
Native 查询未初始化槽时，完整构造 LookupInfo 后才发布；
portable 查询已有的完整 eager 缓存。
有效 lookup 返回 `true`，缺失或损坏 lookup 返回 `false`；
字体参数与 `shaper` 一样，必须是构造该对象时的字体。
热查询仍准入目录查找、表读取与槽访问，但没有新增堆分配。
这个入口不构造 ShapePlan，也不执行受准入 shaping。

回调拒绝立即返回原始 source，不发布半成品，不走普通入口回退。
Native 拒绝后同一槽仍可重新构造；已有完整槽继续可用。
两个并发准入候选只有一个发布，未发布候选的释放访问已预付。
普通查询保留 `OnceLock::get_or_init`，避免并发普通调用重复初始化。
portable 失败丢弃整个未发布冷对象。

Full 连字／PairPos 缓存和 Native LookupInfo 原先使用 `Box::new`。
现在以内部单元素 Vec 保存，通过 `try_reserve_exact` 实现分配失败返回。
这保留堆间接访问，避免把大型 atomic 数组放进每个子表枚举。
普通入口与准入入口使用相同内部表示；read-fonts 等传递依赖没有修改。
受准入子表 Vec 保留已收费容量，避免 `shrink_to_fit` 再次分配；
普通入口保留原有 shrink 行为。

扩展子表解析改为循环，正 offset 每次向后推进；
保持原有 `u32` offset 加法检查和最终 type 转换。
零 offset 且类型仍为 extension 的损坏子表直接跳过，
避免原有递归不能终止。该修正只改变不能产生有限子表的损坏路径。

## 操作与费用依据

work 为输入字节或元素访问，包含重复扫描；
allocation 为完整新堆容量槽数，不是字节。
各项费用均在相应操作开始前由同一策略接收。

| 操作 | 本片依据 |
|---|---|
| lookup 数据定位 | LookupList count、list offset 和 selected offset 各为 u16；槽访问另收费 |
| LookupInfo header | reader 的 flag／count，以及后续 type、重复 flag、可选 mark-set 字段；借用 offset 数组 |
| 子表集合 | 按真实 subtable count fallible 预留；逐 offset 收费，并预付 digest union、写入和释放访问 |
| 类型与 header | 先读取 format；各 generated reader 的 count／value-format 字段单列，payload 仅推进跨度 |
| extension | 每一跳读取 type 与 Offset32 后检查加法；先收费再解析，不提前无费用遍历整条链 |
| Coverage digest | 沿用已验证的 format 1／2 逐元素准入，range 不展开成逐 glyph 扫描 |
| CoverageInfo／ClassDefInfo | 仅读取 format、count 和 format 1 的 start；不扫描 payload |
| Context cache_cost | 最多两个 class-def offset、format dispatch、header/count 与 cost getter；`cost()` 只读取 count 或返回常数 |
| 连字 seconds | 每 set offset 在 iterator 解析之前收费；每 ligature offset、component count、首 component 与三个 mask 访问同样先收费；损坏 offset 仍计访问 |
| Full 子表缓存 | 从 `MappingCache::INLINE_SLOTS` 取得 128 槽，PairPos2 有三组；atomic 初始化与单元素堆容量分别收费 |
| 清理 | 子表写入和 heap slot 写入均预付释放访问；失败不退款，不执行 finalize 或 shaping |

只读证明对应 read-fonts 0.35.0 的 generated layout／GSUB／GPOS reader，
ArrayOfOffsets iterator、Coverage／ClassDef `cost()` 和 value record
`ComputeSize`。本片没有调用 class／coverage population 扫描，
没有依赖测试字体名称推断算法上界。
AAT 表仍在其变长 reader 之前返回 `Ok(None)`。

## 验证结果

[lookup receipt](../evidence/text-stages-b-2026-11-02/lookup-verification.json)
按 buffer → instance → variation → cold 四份冻结补丁重放，
再应用本片增量。10 个源码文件改动，其他 67 个源码文件逐字节保持，
补丁重放精确恢复生产源码。

| 范围 | 已取得结果 |
|---|---|
| 原始／生产 Native／portable | 每组 171 个字体输入、两遍、六组文字与双方向，共 4,104 份 glyph／position／cluster／serialization 输出；四组散列一致 |
| lookup 合成输入 | 28 组 GSUB／GPOS 子表路径，覆盖所有缓存种类、Full 前八个与 Small 后两个、正常／损坏连字、context format 1／2／3 及 extension |
| 外部 Native／portable | 各 23 项通过；新增 5 项覆盖全部种类、逐点拒绝、精确重试 trace、热查询零新增容量、512 跳与零进度扩展、三种真实字体全部有效 lookup 后的普通 shaping |
| 私有 Native／portable | 各 196 项通过；新增两项检查 Full／Small 连字缓存内部 metadata、digest 与零进度扩展没有伪造子表 |
| Core Native／Wasm | `novadraw --lib` check 通过；5 项 Parley 操作预算合同测试通过 |
| Parley | 既有存储／断行准入消费者 8 项通过 |
| Wasm 消费者 | portable library 构建通过；未执行 Wasm 测试 |
| Clippy | Native／portable library 与 lookup 成本 binary 均无 warning |
| 格式／文档 | 新增模块、消费者源码格式与 Markdown 链接检查通过；diff 检查通过 |

真实字体为 Inter、Noto Sans Arabic、Noto Sans SC。
对照继续执行普通 shaping，以检查缓存改动可观察语义；
不能把它解释为 shaping 已受准入。

成本采样为三个字体、五种模式，每组 1,000 次，另预热 100 次；
模式按样本轮换，总计 15,000 个样本。
对象冷构造与成功对象释放在计时之外，拒绝包含候选清理。
只采样每个字体的 GSUB lookup 0，不代表整个字体 lookup 集合。
基线使用同一准入内核的空回调；不是普通 shaping API 基线。
原始样本、费用与分母见
[成本 receipt](../evidence/text-stages-b-2026-11-02/lookup-cost-summary.json)。

| 场景 | 冷空回调 P95（µs） | 冷计数回调 P95（µs） | 热计数回调 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|---:|
| Inter | 1.417 | 1.458 | 0.042 | 1.459 |
| Arabic | 0.292 | 0.292 | 0.042 | 0.333 |
| CJK | 0.333 | 0.334 | 0.042 | 0.334 |

热计数查询三组 allocation 均为零，回调均为四次。
采样未控制整机负载，个别尾值较高；不宣称 Core 排版、宽度复用或 GPU 改善。
成稿复核了操作责任、分母、范围与中文表达；未取得独立评审结论。

下一步继续 B1 的 AAT、ShapePlan 与 buffer 恢复／shaping；
只有分析、导出与 backing 合同均完整后，才接入 Core 显式实例适配。
