# ADR-059 B1：规范化变量坐标与条件选择准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖；Editor、backend、platform 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：坐标子路径已实现；B1 的完整冷构造、用户坐标转换和 shaping 尚未完成，
B／C／D 保持未关闭，宽度复用未默认启用。

本片在已批准的[分析准入范围](../../design/architecture/text-analysis-admission-proposal.md)
内推进。此前 [buffer 子片](text-stages-b1-buffer-2026-11-02.md) 的证据和
补丁保持原始字节，新补丁按顺序追加。

## 完整候选才返回给调用方

新增 `ShaperInstance::try_from_coords_admitted(font, coords, admit)`，
输入为按轴顺序排列的规范化坐标 slice。它与普通 `from_coords` 一样，
最多消费 `fvar.axis_count` 个坐标，不补齐短输入；全零坐标清空为默认实例。
拒绝立即停止，丢弃尚未返回的候选，不重新调用普通入口。
调用方已有的完整实例与输入 slice 保持可用；已完成准入的费用不退款。

普通 `set_coords` 与新增入口共享坐标消费、默认检测及 GSUB／GPOS
feature variation 选择内核。用户坐标入口仍使用原始归一化 helper，
但其后续默认检测和条件选择也共用同一内核。
没有按字体名称划分生产支持范围，也没有利用暖缓存跳过费用证明。

`InstanceOperationError<E>` 保留准入错误 source。SmallVec 的
`CollectionAllocErr` 没有实现标准 `Error`，因此用
`InstanceStorageError` 包装，原始枚举保存在公开字段中；不转成字符串。
候选拥有自己的坐标存储，不从既有 shaper 或 buffer 中取走所有权。

## 费用按实际读取路径推导

work 仍是输入字节或集合元素访问；allocation 仍是完整新堆容量槽数。
固定字段读取使用锁定格式的字节数上界，不代表 CPU 指令数。
数组 header 与条件 offset 的读取分别收费，未先执行算法获取计划。

| 操作 | 费用依据 |
|---|---|
| Planning | directory header、坐标 capacity／length 或空检查之前的 header 访问 |
| Directory | read-fonts 对未排序目录使用线性搜索，对已排序目录使用二分；以 `n + 1` 次比较及选中 record 的两个字段读取为保守上界 |
| TableRead | `fvar` 16 字节固定 header 加 axis count；GSUB／GPOS 最多 14 字节 header；feature variation offset 与 count 各一个 u32；借用 record array 的 header 单列 |
| Storage | 空候选需 spill 时，先申报完整 axis count 容量，再调用 `SmallVec::try_reserve_exact`；11 轴以内不新增堆容量；短输入也保留普通入口先按 axis count 预留的语义 |
| Coordinates | 最多 axis count 次输入读取、写入及后续清理访问，各一遍；清理预留发生在写入前，覆盖后续拒绝或默认坐标清空，不在拒绝清理中再次请求预算 |
| DefaultCheck | 按已写入坐标长度申报全零检查上界；保留原始提前退出行为 |
| VariationRecord | 一个 record 访问及 condition-set offset 的空检查、解析两次读取 |
| ConditionSet | u16 count 读取及借用 offset array 的 header；generated reader 只检查长度并推进 cursor，不扫描 offset 内容 |
| Condition | 直接读取一个 Offset32、u16 format，格式 3／4 最多另读一个 u8 count |
| Predicate | 格式 1 的 axis、min、max 三个字段和一个坐标访问；其他格式保留原始 false 结果 |

这些依据来自 read-fonts 0.35.0 的 `FontRef::table_data`、
`offset_array.rs`、`generated_fvar.rs`、`generated_gsub.rs`、
`generated_gpos.rs`、`generated_layout.rs`，
以及 SmallVec 1.16.4 的 `try_reserve_exact`、`clear`／`truncate` 与 `Drop`。
上述依赖源码没有修改。

条件格式 3／4／5 的 generated reader 不解析子条件，
当前 Harfrust 也不递归求值这些条件。因此本入口没有借用未经计费的条件树遍历。
解析错误沿用原有 `filter_map(Result::ok)` 的行为：
全部条件解析失败时仍可产生空集合的 true 结果；无效 condition-set 被跳过；
null condition-set 是通用匹配。这里保持原始语义，没有把格式错误改成预算拒绝。

坐标清理按实际所有权覆盖：SmallVec 清空可能走 `truncate` 的元素循环，
候选丢弃释放其初始化 slice。全零清空后的 Drop 不再访问相同的初始化元素。
费用包含每个写入坐标最多一次清理，不以“Copy 类型”声称清空算法没有访问。
集合 allocator 的物理对齐与 bookkeeping 不进入逻辑容量槽单位。

## 验证范围

原始证据位于 [B 证据目录](../evidence/text-stages-b-2026-11-02/README.md)；
增量重放和输出散列见
instance receipt（`target/verification/text-stages-b-2026-11-02/instance-verification.json`）。

| 范围 | 结果与分母 |
|---|---|
| 来源与增量重放 | 输入补丁之后仅改 lib、face、ot 三个源码入口，新增一个模块；另 71 个源码文件逐字节保持；顺序重放精确恢复生产源码 |
| 原始／生产普通路径 | Native／portable 四组各 1170 个实例、162 份真实字体 glyph 信息与位置观察；同时断言重复 reset 的结果；共 4680 个实例及 648 份 glyph 观察，四组散列一致 |
| 外部 Native／portable | 各 7 项消费者测试，其中 4 项为新增坐标测试；每个实际准入点逐次拒绝，检查后续停止、原始 source、已有实例及普通重试可用；预算粘滞验证通过 |
| 坐标与条件输入 | 0／1／11／12／64 轴，0／1／11／12／64／65 个输入，短输入、默认坐标、spill、排序／未排序目录、缺失／损坏 fvar、null／无效 condition-set、真假范围、越界轴及无效／非范围条件 |
| 真实字体 | Inter／Noto Sans Arabic／Noto Sans SC；三个变量坐标、六组文字和双方向，共 108 组普通／准入实例 shaping 输出对照 |
| Wasm | portable 消费者构建通过；未执行 Wasm 测试 |
| 消费者 lint | `--lib` 和成本 binary 的 Clippy 通过；新模块格式通过 |
| Parley／Core 回归 | 8 项 Parley 消费者、5 项 Core 预算回归及 Core Wasm check 通过 |

原始／生产对照使用完全相同的 oracle 源码；合成字体只构造 table-level
输入来触发轴数、spill 和条件分支，不用它们证明完整 shaping 的支持范围。
真实字体的 glyph 对照也不替代未知冷缓存和 glyph 算法的费用证明。

本机重复构造成本完成九组各 1000 个样本，9000 个样本编号和费用均通过校验。
每组先预热 100 次，普通／准入／最后准入点拒绝的执行次序逐样本轮换。
采样只包含新规范化实例的构造，
成功候选释放位于计时之外；拒绝包含函数内部的候选清理。
回调只累计本地费用，没有执行 Core 会话预算。
散列、逐组费用和时间分布见
成本 receipt（`target/verification/text-stages-b-2026-11-02/instance-cost-summary.json`）。
采样同时存在其他构建负载，计时粒度也接近这些小对象的构造耗时；
结果只供检查本地开销，不代表环境受控的延迟结论或 Core 宽度复用收益。

报告交付前已复核费用责任、输出分母和中文表达；此自审不作为独立评审结论。

## 剩余 B1 工作

Parley 生产路径还调用用户空间 `from_variations`。
`fvar.user_to_normalized` 的 axis 扫描、avar segment 前缀重放及 avar2
delta 计算尚未接入本片，不能将本片记为变量路径全部完成。
ShaperData 的 OT／AAT 冷缓存、首次 lookup 子缓存、ShapePlan 及必经
shaping 操作仍需逐步准入。未知操作必须在发生前拒绝支持；
需要修改其他依赖时，按已批准提案另列具体依据。
