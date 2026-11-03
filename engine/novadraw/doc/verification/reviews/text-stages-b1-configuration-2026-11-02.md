# ADR-059 B1：Shaper 配置准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：配置子片已实现并通过定向验证；布局表计划、非空 AAT、
buffer 恢复及实际 shaping 继续 B1，默认关闭。

本片接续[计划子片](text-stages-b1-plan-2026-11-02.md)，
实施范围沿用已批准的[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)。
新增 `ShaperBuilder::try_build_admitted`，返回完整配置、提前未支持或
保留 source 的错误。已有 `build` 和准入入口共用配置内核。
冷缓存、实例与计划的构造分别使用各自入口；本片不登记 Core 资产。

## 配置阶段具体读了什么

配置只持有字体、缓存、坐标和表数组的借用，不创建 glyph 或 lookup 集合。
各组件在调用原构造内核前预付费用；费用来自锁定源码中的固定表头，
坐标扫描按实际 slice 长度计费。不存在每字符倍数或新 allocation 单位。

| 组件 | 已核对操作与费用依据 |
|---|---|
| Charmap | cmap 表头及至多两个所选 encoding record；Offset32、format 与 count 读取。各 format reader 借用 payload，不遍历 glyph 数组 |
| GlyphMetrics | hmtx／vmtx／loca／glyf 的 span wrapper；HVAR 20、VVAR 24、VORG 8、GVAR 20、MVAR 12 字节的完整固定表头保守预付，另覆盖 wrapper 和缓存标量访问 |
| OtTables | 坐标 `any` 的逐元素访问；GSUB／GPOS 固定表头；GDEF 的 nullable offset、ClassDef、mark-set 和可选 ItemVariationStore 表头 |
| AatTables | 检查实际储存的 morx／kern／kerx offset；确认 count 为零后，预付上述表及 ankr／trak／feat 固定表头 |

依据为 read-fonts 0.35.0 的 generated reader、`src/tables/loca.rs`、
`src/tables/kern.rs` 以及 Harfrust 对应构造函数。
Loca 的 `read_array` 借用原始 bytes；没有调用 ascending 检查。
GDEF 不查询任何 glyph class，ItemVariationStore 不计算 delta，
指标构造也不解析 outline 或 phantom point。
ankr／trak／feat 只构造顶层表，不访问 anchors、tracks 或 settings。
因此配置的 allocation 为零；这些表后续的 glyph 查询尚未据此获准。
本片没有修改 read-fonts 或其他传递依赖。

morx／kerx 的顶层 reader 会计算可变长数组跨度，因此必须先检查 count。
检查使用随后 `resolve_table` 实际消费的储存 offset，
避免未排序目录上的另一种查询结果替代实际输入。
非空或截断 count 在 variable-size reader 之前返回 `Ok(None)`。
普通入口仍允许原有全部路径。
拒绝时丢弃未发布的借用配置，原 ShaperData／ShaperInstance 保持可用；
不会调用普通入口补做，不会进入 shaping。

## 验证与成本

配置 receipt（`target/verification/text-stages-b-2026-11-02/configuration-verification.json`）
重放前七份冻结补丁，再应用 `harfrust-0.3.2-configuration.patch`。
本片修改六个既有源码文件，其他 72 个 src 文件逐字节保持；
测试结束后再次核对生产源码与重放结果。

| 范围 | 结果 |
|---|---|
| 原始／生产 Native／portable | 四组各 39,744 份观察，SHA-256 一致 |
| 外部 Native／portable | 各 32 项通过；新增配置、逐点拒绝、source、重试、未知 AAT、粘滞预算覆盖 |
| 私有 Native／portable | 各 197 项通过 |
| Core Native／Wasm | `novadraw --lib` check 通过 |
| Wasm 消费者 | portable library 构建通过，未执行 Wasm 测试 |
| Clippy／格式 | Native library／configuration-cost 与 portable library 无 warning；新增测试、采样及 cold_headers 定向格式通过 |

新增等价测试使用既有结构字体与 Inter、阿拉伯文、中文字体，
比较默认／非默认坐标和 point size 下的四方向 glyph／position／cluster。
输出仍通过普通 shaping 获得；本片不证明 shaping 已受准入保护。
每个已支持配置准入点逐一拒绝，确认错误 source、严格 trace 前缀、
相同重试 trace 和后续普通调用。未知 AAT 输入先返回未支持；
同一输入在首个准入点拒绝时，错误优先传播。

配置成本 receipt（`target/verification/text-stages-b-2026-11-02/configuration-cost-summary.json`）
保存四组输入、四种模式各 1,000 次有效样本，共 16,000 次，
各组预热 100 次，模式逐样本轮换。
计时只包含配置，不包括已有缓存和实例构造，也不包含计划或 shaping。

| 输入 | 普通 P95（µs） | 空回调 P95（µs） | 计数 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|---:|
| Inter 默认 | 0.250 | 0.250 | 0.292 | 0.167 |
| Inter 非默认 | 0.209 | 0.209 | 0.250 | 0.125 |
| 阿拉伯文非默认 | 0.167 | 0.208 | 0.209 | 0.125 |
| 中文默认 | 0.167 | 0.167 | 0.208 | 0.125 |

成功计数模式 work 为 282／284／284／282，均为 10 次回调、零 allocation；
最后点拒绝尚未执行 OT 配置，所以费用与耗时更少。
计时粒度与未控制的整机负载限制这些亚微秒数据的解释，
不据此声明 Core 排版、宽度复用或 GPU 性能提升。
后续继续 B1 的布局表计划和实际 shaping，再推进 B2–B4。
成稿已按技术结构和中文表达规范自审，未取得独立评审结论。
