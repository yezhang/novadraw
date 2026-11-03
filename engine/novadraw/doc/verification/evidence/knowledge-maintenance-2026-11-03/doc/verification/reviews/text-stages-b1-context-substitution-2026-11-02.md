# B1：上下文匹配、移动与嵌套单字形替换准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：源码重放、完整输出、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[连字替换](text-stages-b1-ligature-substitution-2026-11-02.md)，
接入零 flags 的 `SequenceContextFormat3`，匹配一至八个 glyph，
嵌套记录只调用零 flags 的 SingleSubst。
输入仍限定已映射 ASCII、LTR Latin／Common／unknown、默认坐标和全局 feature。
普通入口与准入入口执行同一份匹配、移动、递归调用和替换内核。

## 先核对叶子 lookup，再进入上下文内核

支持检查逐个准入 coverage offset／格式／count 的读取，
并检查每个有效 sequence index 引用的 lookup。
冷 lookup 经既有 `get_with` 完整构造后才发布；
执行阶段重复读取计划也重复收费。
缺失 lookup 沿用普通内核的跳过行为，
无效 sequence index 在 lookup 查询前跳过。

九个以上位置会让 `recurse` 中的匹配位置克隆分配。
本片只接入 inline 范围，`SmallVec::clone` 通过 `from_slice` 构造，
源存储即使保留此前 lookup 的堆容量，逻辑切片不超过八项时目标仍使用 inline。
克隆和恢复的元素访问均先收费。
超过八个位置、继续递归的上下文、增长或删除 glyph 的叶子、
非零 lookup flags 和未知子表，在 shaping 前返回 `Ok(None)`。
这不按字体身份区分，也不依赖热缓存恰好存在。

## 无长度变化保证 move_to 不扩容

外层 lookup 开始时，前一 lookup 已同步输出。
本片的外层只有 SingleSubst 和 ContextFormat3，所有嵌套叶子都是 SingleSubst；
每次消费一个 glyph 也输出一个 glyph，始终保持 `out_len == idx`。
因此 `move_to` 回退至已输出的位置时，不会满足 `idx < count`，
无需 `shift_forward`；前进时仍在原 info 数组内，
不会切换独立输出或增长 glyph 数组。
此前 lookup 的多字形扩张或连字收缩不破坏新 lookup 的这个起点不变量。

每条嵌套记录以及最后一次移动，最多访问本上下文长度的 glyph；
费用分别覆盖源读取和目标写入。
匹配位置初始化、输入索引转输出索引、克隆和恢复另计。
叶子不改变长度，因此原内核的 delta 调整、位置扩容和 shift 分支不执行。
无需复制或重写原算法，也不将 `max_ops` 当作准入上界。

## 表读取、重复匹配和拒绝

每个外层输入位置最多尝试每个子表一次。
费用按当前逻辑输入长度、实际 coverage 查询上界、
实际 nested record 数和实际叶子子表数量预付，
包括未命中子表、缺失 lookup、重复位置与反向移动。
匹配时 ASCII 和零 flags 排除跳过 glyph 的路径，
每轮匹配最多读取相邻输入位置。
成功和失败的 unsafe flags 分别覆盖范围最小 cluster、比较及写入。
费用是保守上界，不是实际 CPU 执行次数。

进入 pipeline 后，任一步拒绝立即保留原始 source 并传播，
清空并保留可复用 buffer，不执行普通 shaping 回退。
支持检查拒绝保留逻辑输入。
消费者逐点比较拒绝执行与完整执行的费用前缀，
并核对粘滞预算和下一次普通调用。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_context_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_context_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind context \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_context_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

构建串行、关闭增量与 dev／test 调试信息，Native 使用 Rust 1.96 Darwin LLD。
Wasm 移除 Darwin flags；构建与实际执行分别记录。
费用和完整输出收据存于
[证据目录](../evidence/text-stages-b-2026-11-02/README.md)。

十个矩阵在原始／生产 Native／portable 四配置下签名一致。
新增上下文矩阵每配置 23,040 份完整观察，覆盖 20 份字体、
一／二／八／九位置、两种 coverage、未命中子表、
重复及逆序记录、无效 sequence index、缺失 lookup 和未知路径。
此前九个冻结矩阵保持相同源码与输出签名；
每配置共 298,608 份，四配置共 1,194,432 份观察。
原始观察以无损 gzip 保存，散列针对解压后的字节。

Native／portable 各通过 203 项依赖测试和 60 项消费者测试，
本片新增四项消费者测试。原始算法源码保持，
仅修改准入模块；其余 80 份源码逐字节一致。
重放前十六份冻结补丁再追加本片后，全部 81 份源码与生产一致。
第十七份补丁 SHA-256：
`9b5c96e0e6a1908e64cb6a401784d8aefc60f9e970d218aefce0334efcb14607`。

## 成本与静态门禁

Native release 完成 32,000 个准入成本样本，每组 100 次预热、
1,000 个正式样本，输入 1,040 字符，四种模式按样本轮换。
每个样本检查 glyph 数、费用、准入次数和拒绝状态。
下表为无 glyph 备用容量的结果；完整费用保存在成本收据中。

| 上下文与缓存 | 普通 P95（µs） | 计费 P95（µs） | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| 二位置、一记录，冷 | 43.000 | 46.000 | 249,114 | 6 | 1,195 |
| 同配置，热 | 40.042 | 43.333 | 248,926 | 0 | 1,147 |
| 八位置、五记录、三子表，冷 | 37.084 | 41.000 | 2,761,822 | 8 | 1,413 |
| 同配置，热 | 36.208 | 40.292 | 2,761,596 | 0 | 1,365 |

两组均输出 1,040 个 glyph。热缓存下，无论 glyph 备用容量为
0 或 4,096，inline 匹配和嵌套单字形替换不新增分配。
最后一点拒绝的 allocation 和准入次数与完整路径一致，
work 少最终 glyph 数，中断后没有普通回退。

另完成 24,000 个原始／生产普通入口对照样本，分三轮交换执行顺序。
二位置冷缓存 P95 原始为 40.167–40.500 µs、生产为 40.875–44.125 µs；
八位置冷缓存原始为 36.875–37.166 µs、生产为 37.416–38.042 µs。
系统负载未控制，数据记录本机成本与波动，不作稳定加速结论。
本片增加费用依据和拒绝通道，未消除原算法的重复 lookup 或匹配访问。

Native／portable 消费者与两份成本程序的严格 Clippy 通过。
依赖库与重放前十六份补丁的基线 Clippy JSON 一致，
Native／portable 均有六项既有诊断，新增诊断为零。
Core Native／Wasm check、portable Wasm 构建通过；没有执行 Wasm 测试。
五份新增 Rust 消费者与生产准入模块定向格式检查通过；
四份新增或修改的 Python 脚本通过语法检查。

成稿分别经过事实、逻辑、中文措辞和节奏自审；
保留支持条件、单位、拒绝顺序和未关闭状态，未取得独立评审结论。

## 后续边界

其他上下文格式、带增长或继续递归的嵌套替换、GPOS、
复杂归一化、RTL 和脚本专用实际 shaping 仍待 B1。
Fontique／Parley 编排、导出和 Core 接入分别保留在 B2／B3／B4；
C／D 完整等价与默认启用门禁保持。
