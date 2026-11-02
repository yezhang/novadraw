# ADR-059 B1：用户坐标归一化准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：用户坐标的无 avar／avar1 路径已实现并通过定向验证；
avar2、完整冷缓存、计划与 shaping 准入仍待补齐。B1 未关闭，
B2–B4、C／D 与宽度复用默认启用仍待完成。

本片继续已批准的[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)。
它在[规范化实例子片](text-stages-b1-instance-2026-11-02.md)之后，
补齐 Parley 使用的用户空间坐标转换入口。

## 正常与拒绝行为

新增 `ShaperInstance::try_from_variations_admitted(font, &[Variation], admit)`，
返回 `Result<Option<ShaperInstance>, VariationOperationError<E>>`。
成功值是完整实例；支持缺失或损坏的 avar，以及 avar 1.0。
其他 avar 版本在读取固定 header 后返回 `Ok(None)`，
不分配坐标、不执行归一化或 delta-store 求值，也不执行普通入口回退。
该缺失结果不代表下层已经实现完整分析协议。

普通 `set_variations` 与准入入口共用实例内核，
并继续调用锁定 read-fonts 的 `fvar.user_to_normalized`。
普通入口仍支持原有 avar2 行为；本片没有复制或替换归一化算法。
重复用户标签以后值为准，重复字体轴标签均更新；未知标签忽略，
值仍按原有 Fixed 转换、clamp 和 avar 分段规则处理。
命名实例入口也使用同一用户坐标内核。

准入候选拥有独立坐标存储。回调拒绝立即传播原始 `E`，
`Error::source` 返回该对象；SmallVec 分配错误仍保留原始
`CollectionAllocErr` 包装。费用算术溢出返回 `Arithmetic`，
不执行归一化回调。候选清理费用在初始化前覆盖，
已完成的费用不退款，调用方已有完整实例保持可用。

## 重复扫描如何计费

work 使用输入字节或集合元素访问上界；allocation 是完整新堆容量槽数。
本片在只读 helper 前计划费用；计划所需的扫描也先准入。
费用不按字体名称或暖缓存决定，不使用每字符固定倍数。

| 执行位置 | 费用依据 |
|---|---|
| fvar 读取 | 目录搜索、16 字节 header 与 u16 axis count；沿用已有实例准入 |
| avar 版本规划 | 先准入固定 8 字节 header，再读取版本与声明轴数；未支持版本到此停止 |
| avar1 解析 | generated reader 按声明轴数调用 `total_len_for_count`；每个 map 读取 u16 长度并访问记录，先按 `8 + count × 3` 收费；payload 数组只借用，无新集合 |
| 实际 map 规划 | `VarLenArray::get` 不限制声明轴数，因此从表内位置 8 开始，逐次准入长度读取，最多检查 fvar 轴数个可达记录；保留尾随／不完整记录的上界 |
| 坐标容量 | 空候选超过 inline 11 槽时先申报整个轴数，`try_reserve_exact` 成功后再 resize；初始化与后续清理分别覆盖一次 |
| 用户转换 | 每个输入访问及其两个字段访问，乘上全部轴的标签检查、可能匹配的三个 Fixed 字段读取、写入和 avar 映射上界 |
| avar 映射 | 每次 get 最多重读前 `axis_count − 1` 个 u16 长度，再读目标长度；apply 包含 cap 检查、exact／duplicate／interpolation 三轮分段读取和最终 from／to 读取 |
| 汇总与条件选择 | 第二遍坐标 fill、fvar 数组借用、avar version 读取，以及已有默认检测、GSUB／GPOS 条件选择 |

映射上界依赖规划中实际观察的最大分段长度，包含声明轴数之外的可达记录。
对于每个潜在匹配轴，按最多三个分段扫描保守收费；
保留上游提前结束行为，不先执行转换以获取费用。
上界覆盖重复标签造成的重复访问，也包含无匹配时仍执行的轴标签检查。
所有依赖输入长度的乘加均检查 usize 溢出。

证明依据为 read-fonts 0.35.0 的 `tables/fvar.rs`、`tables/avar.rs`、
`generated_fvar.rs`、`generated_avar.rs`、`array.rs`、`read.rs`、
`tables/instance_record.rs` 和 SmallVec 1.16.4 的预留／resize／清理实现。
这些第三方源码保持未改。

avar2 的 `compute_float_delta` 有 delta-set／region 重复扫描，
本片未完成该路径的计费证明；没有据此断言它会分配 Vec 或必须修改 read-fonts。
当前先明确拒绝该支持路径，后续继续在已授权责任域核对。

## 已完成验证

来源与对照见 [variation receipt](../evidence/text-stages-b-2026-11-02/variation-verification.json)。
冻结 buffer 和 instance 补丁按顺序重放后，仅改 lib、face 两个入口，
新增 `variation_admission.rs`；其余 73 个源码文件逐字节保持。
新增增量补丁重放精确恢复当前生产源码，不重写前两份证据。

| 范围 | 结果 |
|---|---|
| 原始／生产 Native／portable 四组 | 每组 7,854 个用户实例、935 个命名实例与 378 份真实字体 glyph 观察；含重复 reset 断言；四组输出散列一致 |
| 外部 Native／portable | 各 12 项消费者测试，其中 5 项为本片新增；逐点拒绝、原始 source、已有实例、普通重试、粘滞预算、未知版本与重复扫描费用验证通过 |
| 私有 Native／portable | 各 193 项库测试通过，含已有 5 项 buffer 私有测试及新增算术溢出测试 |
| 合成输入 | 0／1／11／12／64 轴、重复轴标签、排序／未排序目录、无／空／损坏 avar、0／1／多分段、重复端点、exact／duplicate／interpolation 与声明轴数之外的尾随记录 |
| 真实字体 | Inter、Noto Sans Arabic、Noto Sans SC；5 个用户值、6 组文字、双方向，共 180 组普通／准入 shaping 对照 |
| Parley 与 Core | 8 项 Parley 消费者通过；Core Native／Wasm `--lib` check 通过 |
| Wasm | portable 消费者构建通过；未执行 Wasm 测试 |
| lint 与格式 | 消费者 `--lib`、`variation-cost` Clippy 及新增模块格式通过 |

重复新建实例的成本采样共 9 组，每组 1,000 次，另各预热 100 次；
普通／准入／最后准入点拒绝逐样本轮换。成功实例释放在计时之外，
拒绝计时包含内部清理，回调只累计本地费用。
分母、源码／二进制散列和实际费用见
[成本 receipt](../evidence/text-stages-b-2026-11-02/variation-cost-summary.json)。

| 场景 | 普通 P95（µs） | 准入 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|
| Inter | 0.208 | 0.250 | 0.209 |
| 12 轴 | 0.375 | 0.375 | 0.417 |
| 64 轴 | 1.125 | 1.166 | 1.167 |

采样未控制整机负载，粒度接近小对象构造耗时，只作为本地开销证据。
它不包含 Core 会话预算、完整冷 shaper、排版或 GPU，不能证明宽度复用收益。
交付前已复核源码依据、分母、费用责任与中文表达；自审不代表独立评审。

下一步继续 B1 的冷缓存、计划与必经 shaping 准入，
随后按 B2–B4 完成查询编排、导出和显式实例适配。
