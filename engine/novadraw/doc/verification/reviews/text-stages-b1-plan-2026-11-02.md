# ADR-059 B1：ShapePlan 集合与排序准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖。`api_semantics`：`text.flow`、`text.interaction`。
状态：无布局表的 ShapePlan 子片已实现并通过定向验证；
布局表查询、其他计划路径、buffer 恢复及 shaping 继续 B1。
B1 与默认启用未关闭。

本片接续[空 AAT 表头准入](text-stages-b1-aat-headers-2026-11-02.md)，
沿用用户已批准的[分析准入范围](../../design/architecture/text-analysis-admission-proposal.md)。
新增 `ShapePlan::try_new_admitted`；它接收与冷缓存相同的泛型策略，
返回完整计划、提前未支持或保留 source 的错误，不登记 Core 资产。

## 先检查路径，再构造计划

本片支持已配置 Shaper 的以下计划路径：

- GSUB／GPOS 均未解析为布局表，language 为 `None`。
- 不执行 morx、kern、kerx 计划分支。
- 所选 shaper 没有 collect、override 或 create-data 回调。
- 字体目录中不存在 trak 表。

这些条件来自实际算法分支，未按字体名称建白名单。
无效 direction、语言处理、布局表查询、脚本计划回调和 trak reader
均在未知操作前返回 `Ok(None)`。检查本身先准入；
预算拒绝优先传播为错误，不转换成未支持。
配置 Shaper 仍使用原入口，本片不声称覆盖它的 reader 成本。
非空 AAT 也仍未覆盖。

没有布局表时，script／language tags 不会被消费；
受准入分支直接跳过这段转换，继续使用同一空索引表示。
普通入口保留原标签转换与字体查询。
脚本分类使用既有有限 match，并在调用计划回调前检查支持范围。
字体 classes 的判断只访问已配置对象的 `Option`，不再解析 GDEF。

## 集合与排序的费用

普通和准入入口共用 feature 收集、重复项合并、mask 分配、stage 编排
及计划最终构造。普通入口保留上游排序；
受准入分支使用无额外堆存储的 heap sort，在每次比较和交换前回调。
feature-info 的排序键包含原序号，保持用户覆盖的次序；
相等记录没有需保留的对象身份。
排序为 O(n log n)，没有未准入的标准排序临时数组。

feature-info、map-feature、临时 stage 与输出 stage 的增长都先准入，
随后 `try_reserve_exact`。满容量时按当前容量倍增，空集合先取一个 slot；
策略覆盖旧元素迁移和完整新增容量，避免每次追加都搬移全部前缀。
初始化预付写入与最终清理的元素访问；
feature 合并、重复遍历和每个 mask 查询分别计费。
费用单位继续为输入字节或元素访问、完整新 heap capacity slots，
不换算成字节或按字符估算。

用户 feature 的最终存储仍是 `SmallVec<[Feature; 4]>`。
四项以内预付 inline 复制及清理；超过四项先构造已准入的 Vec，
然后通过锁定 SmallVec 1.16.4 的 `from_vec` 转移堆所有权，
不再调用不可拒绝的隐式 spill。该路径只读依赖源码，未新增生产依赖补丁。
任何拒绝都释放临时候选，下一次构造得到同一 trace；
此前的完整 ShaperData 不被替换。

## 验证与成本

plan receipt（`target/verification/text-stages-b-2026-11-02/plan-verification.json`）
重放前六份冻结补丁，再追加 `harfrust-0.3.2-plan.patch`。
本片修改四个既有源码文件，其他 74 个源码文件逐字节保持；
重放结果精确恢复生产源码，并在测试结束后再核对一次。

| 范围 | 结果 |
|---|---|
| 原始／生产 Native／portable | 四组各 207 个字体输入、两遍、四组 feature、六组文本、四个方向，共 39,744 份观察，散列一致 |
| 外部消费者 Native／portable | 各 28 项通过；计划键与普通 shaping 观察、逐点拒绝、source、重试 trace、未知路径和粘滞预算有定向覆盖 |
| 私有 Native／portable | 各 197 项通过；heap sort 覆盖长度 0–256、重复值和逐点拒绝 |
| Core Native／Wasm | `novadraw --lib` check 通过 |
| Wasm 消费者 | portable library 构建通过；未执行 Wasm 测试 |
| Clippy／格式 | Native library／plan-cost、portable library 无 warning；消费者定向格式检查通过 |

普通对照观察计划键、glyph、position、cluster 和 serialization。
受准入计划另与普通计划比较四方向、四种 script、三组 feature 下的
240 对 shaping 输出；shaping 本身仍使用普通入口。
因此这些结果不能解释为任意字体下的完整 shaping 准入。

成本 receipt（`target/verification/text-stages-b-2026-11-02/plan-cost-summary.json`）
记录无布局表字体、0／4／6／72 项用户 feature 的四组 fresh plan。
普通入口、准入空回调、计数回调和最后点拒绝各采样 1,000 次，
每组预热 100 次，模式逐样本轮换，共 16,000 个有效样本。
成功释放在计时之外，拒绝包含候选清理。

| 用户 feature／方向 | 普通 P95（µs） | 准入空回调 P95（µs） | 准入计数 P95（µs） | 最后点拒绝 P95（µs） |
|---|---:|---:|---:|---:|
| 0／LTR | 1.000 | 1.292 | 1.584 | 1.625 |
| 4／RTL | 1.375 | 1.750 | 2.250 | 2.250 |
| 6／LTR | 1.542 | 1.958 | 2.458 | 2.500 |
| 72／vertical | 2.875 | 4.167 | 5.834 | 5.583 |

四组计数路径的 work／allocation／回调次数分别为
404／74／214、1088／72／554、1179／78／599、4097／335／1945。
allocation 包含整个构造过程中每次增长的新容量，不代表最终留存量；
work 包含前缀迁移、排序、合并及重复查询。
准入仍有可见开销，整机负载未控制；这些数据不声明 Core 排版、
宽度复用或 GPU 性能提升。

下一步补齐布局表计划查询与其集合、shaper 配置、buffer 所有权恢复及
shaping 各阶段的拒绝通道，再推进 B2–B4。
成稿已按技术结构和中文表达规范自审，未取得独立评审结论。
