# B1：上下文匹配位置的堆存储与备份准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：补丁重放、完整输出、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[上下文替换](text-stages-b1-context-substitution-2026-11-02.md)，
将零 flags、嵌套单字形叶子的 Format 3 上下文扩展至 Harfrust 现有
`MAX_CONTEXT_LENGTH` 上限 64。输入仍限定已映射 ASCII、
LTR Latin／Common／unknown、默认坐标与全局 feature。
匹配、移动、递归调用及替换继续使用普通入口的算法内核。

## 先预留匹配位置和备份，再执行嵌套记录

普通 `recurse` 保存当前匹配位置时调用 `SmallVec::clone`。
逻辑长度超过八项时，克隆会分配；仅预留原匹配数组不能覆盖这次增长。
本片在外层 lookup 前增加一份可选备份数组。
两份数组都按实际上下文长度先准入，再通过 `Vec::try_reserve_exact`
预留，由 `SmallVec::from_vec` 接管分配。
新增 capacity slots 全额计入 allocation，旧元素迁移、初始化和释放责任
仍由既有 matching storage 计划覆盖。

执行嵌套记录时，内核将当前匹配位置复制到已预留的备份，
执行零 flags 的 SingleSubst 叶子，再交换两份数组恢复原匹配位置。
备份留在当前 apply context 中，后续记录、输入位置和子表复用它。
叶子不继续递归、不改变 glyph 数，也不改变匹配数组；
因此复制、恢复与后续 resize 都在已预留容量内完成。
复制、resize 初始化、交换及最终清理由外层费用上界覆盖，
没有在嵌套内核中吞掉准入错误，也没有调用普通入口重做。

八项以内且尚未使用备份时，保持原 inline clone 路径。
一旦当前 apply context 已有备份，后续较短上下文也复用备份，
避免把其容量丢弃后再次分配。普通 shaping 不创建可选备份，
仍使用原 clone 行为；两种入口共用同一份替换内核。
此前 lookup 的扩张或收缩已同步输出，
本片的无长度变化约束继续保证 `out_len == idx`。

## 组件求和也要在塑形前检查

`match_input` 即使没有输出组件计数，仍会将首 glyph 之后的
组件字段相加，累加器类型为 `u8`。
原 inline 范围至多相加七个四位组件字段；
扩大上下文长度后，之前的连字 lookup 可能让这个和超出 255。
支持检查沿用此前 lookup 的组件上界，检查
`(context_count - 1) * component_bound <= 255`。
不能满足时，在 shaping 前返回 `Ok(None)` 并保留输入。

消费者构造五组件连字后接上下文的反例：
52 位置的 51 × 5 可表示，准入与普通完整输出一致；
53 位置的 52 × 5 超限，在进入 pipeline 前返回未支持。
超过 64 位置也在 pipeline 前返回未支持。
未支持的递归、增长、删除、非零 flags 和未知子表仍保留原边界。
这些约束按算法与字体表结构判断，不按字体身份或缓存冷热区分。

## 拒绝前缀与恢复

9、17、64 位置的成功消费者比较全部 glyph info 与 position，
覆盖逆序和重复嵌套记录、不同 cluster 身份与 glyph 备用容量。
两份匹配数组每次 shape 各分配一次，后续内核访问不再增长。
9、64 位置分别对冷、热完整路径的每个准入点拒绝，
检查精确费用前缀、原始错误 source、后续步骤停止与普通 buffer 复用。
支持检查拒绝保留输入；pipeline 中拒绝清空并保留可复用 buffer。
既有粘滞预算消费者继续覆盖统一拒绝通道。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_context_spill_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_context_spill_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind context-spill \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_context_spill_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

十八份补丁依次重放后，全部 81 份源码与生产逐字节一致。
本片修改准入模块、lookup 驱动和递归备份存储三份源码，
其余 78 份保持不变。第十八份补丁 SHA-256：
`4dd3d51f9f69b32f8792fd9c71fa7b19c15e98e7142803f6c15c0e363c786644`。
既有十份矩阵与 fixture 保持冻结，完整输出签名保持；
新增矩阵覆盖 9／17／64／65 位置，每配置 3,840 份观察。
十一矩阵每配置共 302,448 份，四配置共 1,209,792 份完整观察一致。
原始输出以无损 gzip 保存，散列针对解压后的字节。
Native／portable 各通过 203 项依赖测试。
最终 Native／portable 各通过 64 项消费者测试，本片新增四项。
源码重放期间 Native 曾通过 63 项，随后补入组件边界测试；
最后门禁已复验完整 64 项，收据分别保留。
构建串行、关闭增量与 dev／test 调试信息；
Native 使用 Rust 1.96 Darwin LLD，Wasm 移除 Darwin flags。

## 成本与普通入口对照

Native release 完成 32,000 个准入成本样本，每组 100 次预热、
1,000 个正式样本，输入 1,040 字符，四种模式按样本轮换。
每个样本检查 glyph 数、费用、准入次数与拒绝状态。
下表列出无 glyph 备用容量的结果；完整数据存于
`context-spill-cost.ndjson` 和 `context-spill-cost-summary.json`。

| 上下文与缓存 | 普通 P95（µs） | 计费 P95（µs） | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| 九位置、五记录、三子表，冷 | 61.083 | 51.584 | 2,908,516 | 26 | 1,425 |
| 同配置，热 | 56.750 | 48.917 | 2,908,290 | 18 | 1,377 |
| 六十四位置、五记录、三子表，冷 | 33.584 | 35.750 | 10,975,806 | 136 | 1,755 |
| 同配置，热 | 32.583 | 34.917 | 10,975,580 | 128 | 1,707 |

两组均输出 1,040 个 glyph。热 lookup 不再分配冷缓存，
但匹配与备份归当前 shape 所有，每次仍分别新增九或六十四个 capacity slots。
glyph 备用容量为 0 或 4,096 时，热 allocation 均为 18 或 128。
最后一点拒绝的 allocation 和准入次数与完整路径一致，
work 少最终 glyph 数，失败后 buffer 为空且没有普通回退。
九位置样本中复用备份减少了普通路径的反复克隆分配；
六十四位置样本中，表检查与费用规划仍增加成本。
这些 work 数是保守访问上界，不能解释为实际 CPU 指令数。

另完成 24,000 个原始／生产普通入口对照样本，分三轮交换执行顺序。
九位置冷缓存 P95 原始为 55.041–67.792 µs，生产为 56.917–63.958 µs；
六十四位置冷缓存原始为 31.875–32.000 µs，生产为 32.875–33.042 µs。
普通入口继续使用原 clone，新增备份仅由准入路径启用。
系统负载未控制，以上数据记录本机成本与波动，不作稳定加速结论。

Native／portable 消费者与两份新成本程序的严格 Clippy 通过。
依赖库与前十七份补丁的基线 Clippy JSON 一致，
两配置各有六项既有诊断，新增诊断为零。
Core Native／Wasm check 和 portable Wasm 构建通过，没有执行 Wasm 测试。
准入模块与四份新增或修改的 Rust 消费者定向格式检查通过，
四份 Python 脚本语法检查通过。

成稿分别经过事实、逻辑、中文措辞与节奏自审；
数字、单位、支持范围、拒绝顺序与未关闭状态保留。
中文正则四组均零命中，机械扫描不代替逻辑审查；
本片没有独立评审结论。

## 后续边界

其他上下文格式、继续递归或改变长度的嵌套替换、GPOS、
复杂归一化、RTL 与脚本专用实际 shaping 仍待 B1。
Fontique／Parley 编排、导出与 Core 接入分别继续 B2／B3／B4；
C／D 完整等价与默认启用门禁保持。
