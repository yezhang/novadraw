# ADR-059 B1：冷缓存目录、cmap 与 mark digest 准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：冷缓存基础子片已实现并通过 Native／portable 定向验证。
lookup 展开、AAT 子表缓存、计划与 shaping 准入仍待补齐；B1 未关闭。
B2–B4、C／D 与宽度复用默认启用保持待完成。

本片继续已批准的[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)，
接续[用户坐标归一化](text-stages-b1-variation-2026-11-02.md)。
它补齐冷构造中的目录查询、cmap 选择、Native 空 lookup 槽、
GDEF mark digest 与内联字形映射缓存；尚未覆盖全部字体路径。

## 正常与拒绝行为

新增 `ShaperData::try_new_admitted(font, admit)`，返回
`Result<Option<ShaperData>, ColdOperationError<E>>`。
普通入口与准入入口共用 OT、AAT 及 TableOffsets 构造内核；
普通路径继续支持原有字体与算法。
准入成功只承诺本片覆盖的冷对象构造，后续 `shaper().build()`、
lazy lookup、ShapePlan 和 shaping 仍使用原有入口，尚未接入准入。
Parley／Core 没有调用本片新增入口，生产阶段缓存保持默认关闭。

Native 构造 `Vec<OnceLock<Option<Box<LookupInfo>>>>` 的空槽，
不会提前解析 LookupInfo 或分配 Box。portable 原有构造会立即展开 lookup；
当前在读到非零 lookup count 后返回 `Ok(None)`，发生在预留或展开之前。
零 lookup 的 portable 路径可以继续。

AAT 的拒绝边界更早：`morx`／`kerx` 顶层 reader 自身调用
`VarSize::total_len_for_count` 扫描变长记录。
因此先对目录查询计费，再通过 `table_data` 判断三个 AAT 表是否可达；
任一可达即返回 `Ok(None)`，包括空或损坏的 payload。
此处不调用 AAT reader，也不扫描 chain／subtable。
这不是字体白名单；判断依据是实际算法路径。
目录项不可达时，普通 reader 同样无法取得 payload。

回调拒绝立即传播原始 `E`，`Error::source` 返回该对象。
分配失败保留 `TryReserveError`，长度加法溢出返回 `CostOverflow`。
失败不返回候选，不执行普通入口回退；既有完整缓存及输入保持可用。
初始化前支付候选元素的释放访问费用，已完成费用不退款。
本入口不转移 UnicodeBuffer 所有权；shaping 的 buffer 恢复仍属后续子片。

## 计费依据

work 是输入字节或元素访问，allocation 是完整新堆容量槽数。
不按字体名称、整体文件大小或每字符固定倍数收取费用。

| 执行位置 | 依据 |
|---|---|
| 目录查询 | 先读记录数，再按 `n + 3` 记录／字段访问上界收费；覆盖 TableProvider 未排序目录的线性查询及 TableOffset 原有二分查询 |
| 固定指标表 | maxp 的版本与 glyph count、head 的 units／loca、OS2 版本及 ascent／descent、hhea／vhea metrics 字段分别在读取前计费；generated reader 的跨度推进没有 payload 遍历 |
| TableOffsets | 所有 offset 查询走同一目录准入；继续使用从 offset 到 font 尾部的原有解析语义，不以目录声明长度替换它 |
| cmap 优先选择 | 保留原有十档顺序；每次经过 encoding record 都先收费，包含重复前缀扫描与平台／编码字段 |
| cmap variation 选择 | 另一次完整记录扫描，逐个尝试 subtable；保留早停及损坏格式跳过行为 |
| cmap subtable reader | Offset32、format u16 与最多一个 u32 count；0／2／4／6／8／10／12／13／14 reader 借用数组，无逐元素扫描或分配；format 8 的 8192 字节跨度也只是推进 |
| Native lookup 槽 | LookupList offset 与 count 读取，随后空 Vec 的完整预留、OnceLock 初始化和后续释放访问；不提前构造 LookupInfo |
| mark set | nullable offset、header count、每个 Coverage offset 与 reader；Vec 先 `try_reserve_exact`，逐个 digest 完成后写入，写入前包含释放访问 |
| Coverage digest | format 1 每 glyph 的 u16 读取与三 mask 更新；format 2 的两个端点读取、最多三次全满检测及三 mask 更新，不展开整个 glyph range |
| 映射缓存 | 从 `hb_cache_core_t` 的实际泛型槽数读取常量，初始化 GDEF 128／cmap 256 个内联 atomics；allocation 为零 |

只读证明核对 read-fonts 0.35.0 的 `generated_cmap.rs`、
`generated_layout.rs`、`generated_gdef.rs`、`generated_gsub.rs`、
`generated_gpos.rs`、指标表 reader、TableProvider 与 directory 查询；
同时核对 `generated_morx.rs`、`generated_kerx.rs` 的变长 reader。
read-fonts 及其他传递依赖保持未改。
本片没有据可借用的 API 外形推断所有字体表读取均为常数成本。

## 已完成验证

cold receipt（`target/verification/text-stages-b-2026-11-02/cold-verification.json`）
按 buffer → instance → variation 三份冻结补丁重放后生成本片增量。
8 个既有源码文件有改动，新增 `cold_admission.rs`；
其余 68 个源码文件逐字节保持，包括原始换行。
增量补丁重放精确恢复生产源码，前三份 receipt 保持不变。

| 范围 | 结果 |
|---|---|
| 原始／生产 Native／portable 四组 | 每组 143 个字体输入、两遍冷／热观察、6 组文字及双方向，共 3,432 份 glyph／position／cluster／serialization 观察；四组散列一致 |
| 外部 Native／portable | 各 18 项消费者测试通过，其中本片新增 6 项；覆盖逐点拒绝、原始 source、粘滞预算、普通重试、部分 mark 候选释放、77 次 cmap 记录访问及提前返回 |
| 私有 Native／portable | 各 194 项通过；新增容量算术溢出测试确认分配准入之前停止 |
| 合成输入 | 排序／未排序目录、十种 cmap format／未知格式、六种平台编码位置、两种 coverage、损坏 mark offset、0／1／32 lookup、空／损坏 AAT payload |
| 真实字体 | Inter、Noto Sans Arabic、Noto Sans SC；普通 shaping 核对冷缓存成功后的 glyph、位置及序列化，shaping 本身尚未准入 |
| Core Native | `novadraw --lib` check 通过 |
| Parley | 既有存储／断行准入消费者 8 项通过 |
| Wasm | portable 消费者构建与 Core `--lib` check 通过，未执行 Wasm 测试；Core 首次 check 未取得完成回执，随后使用既有 Wasm 缓存补验通过 |
| Clippy／格式／文档 | Native 消费者 `--lib`／`cold-cost` 及 portable `--lib` 无 warning；新增模块格式、502 份 Markdown 与 diff 检查通过 |

冷构造成本采样有 9 组，每组 1,000 次，另各预热 100 次；
普通／准入／最后点拒绝按样本轮换。
成功候选释放在计时之外，拒绝计时包含内部清理。
分母、源码／二进制散列和逻辑费用见
cost receipt（`target/verification/text-stages-b-2026-11-02/cold-cost-summary.json`）。

| 场景 | 普通 P95（µs） | 准入 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|
| Inter | 0.709 | 0.958 | 1.083 |
| Arabic | 0.958 | 1.291 | 1.416 |
| CJK | 0.750 | 0.917 | 0.917 |

这次只测 Native 空 lookup 槽、cmap 与 mark digest 等冷对象，
回调仅累计本地计数。没有控制整机负载，采样也出现较高尾值；
不能据此宣称 Core 排版、宽度复用或 GPU 性能改善。
成稿已复核费用责任、验证分母与中文表达；未取得独立评审结论。

下一步继续 B1 的 LookupInfo／external cache 准入，
补齐 portable 展开与 Native lazy 初始化，再推进 AAT、计划和 shaping。
