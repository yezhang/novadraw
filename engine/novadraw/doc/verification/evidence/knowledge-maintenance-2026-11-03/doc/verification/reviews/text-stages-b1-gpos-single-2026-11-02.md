# B1：GPOS 单字形定位准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：补丁重放、完整输出、消费者、成本与跨目标定向门禁通过。
B1 保持未关闭，生产宽度复用缓存保持默认关闭。

本片接续[上下文堆存储](text-stages-b1-context-spill-2026-11-02.md)，
补齐 GPOS SinglePos format 1／2 的实际定位准入。
支持范围仍限定已映射 ASCII、LTR Latin／Common／unknown、
默认坐标、全局 feature 与零 lookup flags。
设备修正、变体定位、PairPos、字形附着、GDEF 与 AAT 在执行前返回未支持。
支持判断按所选算法和字体表结构执行，不使用字体身份白名单。

## 让定位阶段继续传递同一策略

此前 `shape_internal_with` 在 Position 阶段收费后调用普通 `position`，
定位 lookup 没有继续接收准入策略。因此此前支持检查整体排除 GPOS。
本片让定位、复杂定位、计划定位和 GPOS 驱动返回 `Result`，
把同一策略传递到 `apply_layout_table_with`。
lookup 回调增加 `TableIndex`，分别核对 GSUB 与 GPOS 的费用责任；
GSUB 替换、GPOS 定位和普通入口共用原算法内核。
删除已无调用的普通 lookup 包装，不改变替换和定位算法。

预检先准入 stage、lookup 与子表访问，构造完整冷缓存，
再检查子表种类、flags 和 `ValueFormat`。
只允许 X／Y placement、X／Y advance 标量位。
遇到设备、变体、保留位或其他真实定位 lookup，
在进入 shaping pipeline 前返回 `Ok(None)`，保留逻辑输入。
不存在或已由原 reader 忽略的无效 lookup 沿用原行为。

## 先收费，再原地写入 position

`ot/gpos/single.rs` 的两种格式先查 coverage，再调用 `apply_value`，
每次成功将 buffer 索引前移一项。
format 1 使用同一份标量记录；format 2 用 coverage 索引计算记录位置。
`read-fonts` 的相应 generated reader 只检查借用范围，
读取 value format／count，不遍历整个值数组，也不建立输出集合。
本片没有修改该辅助依赖的生产源码。

每个实际 lookup 前，费用覆盖 buffer 和 lookup digest、
前向 glyph／mask／property 检查、所有可能子表尝试、
重复 reader 与 coverage 搜索，以及每个标量的读写。
coverage format 1／2 沿用已证明的二分比较上界。
标量费用根据实际 format 位数计算；
水平路径跳过 Y advance 值，其预付上界仍允许这一项读取。
Position 阶段同时收费完整 position 初始化、水平指标、
GPOS 起始记录和 stage 访问。

GPOS 使用 `IN_PLACE`，不切换 info／pos 数组，不扩张 glyph 数，
不重建 digest，不使用递归匹配备份，也不创建字形附着链。
零 flags 不触发 mark-filtering 的完整 info 缓存扫描。
因此此路径没有新增定位存储分配；前置 MultipleSubst 的扩容、
初始化与释放仍由既有 GSUB 计划收费。
可用容量不够时不能把 GPOS 的零分配结论扩展到整个 shaping。

## 完整输出与拒绝恢复

消费者覆盖两种 SinglePos 格式、两种 coverage、单／多子表、
一／两 lookup、各标量位和组合值。
前置 MultipleSubst 扩张后，GPOS 使用扩张后的实际 glyph 数计费。
空文本、未命中、顺序／同值／逆序 cluster 与备用容量
均比较完整 glyph info、position 和 flags。
定位有效性由非零 X／Y offset 断言核对；
水平 Y advance 保持为零。

冷、热定位路径分别对每个准入点拒绝，
核对原始错误 source、精确费用前缀和后续操作停止。
支持预检中拒绝保留输入；进入 pipeline 后拒绝清空 buffer、
释放内部变量占用并保留可复用存储。
每次失败后均用恢复的 buffer 完成下一次普通 shaping。
设备值、非零 flags、有效 PairPos、GDEF 与 RTL 反例
在 pipeline 前返回未支持。

## 可复验入口

```bash
python3 tools/scripts/check_harfrust_gpos_single_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_gpos_single_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_plan_layout_ordinary_cost.py \
  --input-kind gpos-single \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 tools/scripts/check_harfrust_gpos_single_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-target-wasm --linker rust-lld
```

十九份补丁依次重放后，全部 81 份源码与生产逐字节一致。
本片修改准入模块、lookup 驱动和两份定位入口源码，
其余 77 份保持不变。第十九份补丁 SHA-256：
`8d3770b5ff64112ca393e0ce910644bd6aec5bc77970126a9035142f6b59a8d8`。

十二矩阵每配置 339,312 份完整输出，四配置共 1,357,248 份观察一致。
新增定位矩阵每配置 36,864 份，既有十一份 oracle 源码及完整签名保持。
输出以无损 gzip 保存，散列针对解压后的字节；
Native／portable 各通过 203 项依赖和 67 项消费者测试，本片新增三项。
构建串行、关闭增量与 dev／test 调试信息；
Native 使用 Rust 1.96 Darwin LLD，Wasm 移除 Darwin flags。

## 成本与普通入口对照

Native release 完成 32,000 个准入成本样本，每组 100 次预热、
1,000 个正式样本，输入 3,072 字符，四种模式按样本轮换。
每个样本检查 glyph 数、费用、准入次数与拒绝状态。
下表列出无 glyph 备用容量的结果；全部数据存于
`gpos-single-cost.ndjson` 和 `gpos-single-cost-summary.json`。

| 路径与缓存 | 普通 P95（µs） | 计费 P95（µs） | work 上界 | allocation | 准入次数 |
|---|---:|---:|---:|---:|---:|
| 两 lookup、三子表的纯定位，冷 | 143.500 | 151.958 | 836,268 | 8 | 3,248 |
| 同配置，热 | 140.709 | 149.084 | 836,042 | 0 | 3,196 |
| 前置扩张替换后定位，冷 | 374.000 | 390.000 | 2,025,485 | 18,444 | 3,359 |
| 同配置，热 | 376.875 | 393.083 | 2,025,146 | 18,432 | 3,280 |

纯定位输出 3,072 glyph，前置替换后输出 6,144 glyph。
纯定位的冷 allocation 八项来自完整 lookup 缓存；
定位执行本身不分配，热路径 allocation 为零。
前置替换的保守最大输出为 9,216 glyph，
两份 shaping 数组共新增 18,432 capacity slots，按整份新容量收费。
备用容量为 16,384 时覆盖此前扩张，热 allocation 为零，
冷 allocation 十二项全部来自 GSUB／GPOS lookup 缓存。
备用数组的全量初始化与访问仍计入 work。
最后一点拒绝的 allocation 和准入次数与完整路径一致，
work 少最终 glyph 数，失败后 buffer 为空且没有普通回退。

另完成 24,000 个原始／生产普通入口对照样本，
三轮交换执行顺序，使用同一 source 与 feature。
纯定位冷缓存 P95 原始为 143.917–151.166 µs，生产为 146.166–159.084 µs；
前置替换冷缓存原始为 372.084–383.042 µs，生产为 374.000–386.333 µs。
准入预检及费用规划仍有 CPU 开销。
系统负载未控制，这些数据记录本机成本与波动，不作稳定加速结论，
也不能代替生产宽度复用或 GPU 延迟验收。

Native／portable 消费者与两份新成本程序的严格 Clippy 通过。
依赖库 Clippy JSON 与前十八份补丁的基线一致，
两配置各有六项既有诊断，新增诊断为零。
Core Native／Wasm check 与 portable Wasm 构建通过，未执行 Wasm 测试。
准入模块、五份新增 Rust 消费者与测试入口的定向格式检查通过，
四份 Python 脚本语法检查通过。

成稿分别经过事实、逻辑、中文措辞与节奏自审；
数字、单位、支持范围、拒绝顺序与未关闭状态保持。
中文正则四组均零命中，机械扫描不代替逻辑审查；
本片没有独立评审结论。

## 后续边界

PairPos、字形附着、GDEF、其余上下文格式、继续递归或改变长度的
嵌套替换、复杂归一化、RTL 与脚本专用实际 shaping 仍待 B1。
Fontique／Parley 编排、导出与 Core 接入分别继续 B2／B3／B4；
C／D 完整等价和默认启用门禁保持。
