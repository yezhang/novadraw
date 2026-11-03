# B1：grapheme 合并与反向分组准入

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core 文字引擎的 Harfrust 0.3.2 生产依赖。
`api_semantics`：`text.flow`、`text.interaction`。
状态：实现、定向测试、完整普通输出对照、成本记录及跨目标检查完成。
成本记录不等于性能目标通过，尾延迟波动与限制见下文。
B1 保持未关闭，生产宽度复用缓存继续默认关闭。

[前片](text-stages-b1-dotted-circle-kernel-2026-11-03.md)补齐圆圈插入和输出同步，
但组合字符后续还要经历分组、簇合并和方向调整。本片为读取已存储
continuation 属性的分组内核增加操作前准入，公共组合字符预检仍返回未支持。
continuation 是用于判定后续字符是否属于同一字素组的已存储标记。
这里没有覆盖任意回调的成本，也没有替代完整的 Unicode 属性和 mark 定位准入。

## 共享内核与费用依据

普通入口使用 `OrdinaryShape`，内部准入入口使用相同泛型内核。
`group_end_with`、`reverse_groups_with`、`reverse_range_with`、
`merge_clusters_with` 和 `unsafe_to_break_with` 在读取或修改记录前传播拒绝。
`form_clusters_with` 与 `ensure_native_direction_with` 使用这些入口。

| 操作 | 操作前费用 | 依据与边界 |
|---|---|---|
| 分组比较 | 每次支付控制操作和三次记录访问 | 两个记录引用及一个已存储 continuation 字段；不包括任意比较回调 |
| 簇合并 | 每次按当时输入长度与输出前缀长度支付保守扫描上界 | 最小值查询、区间延伸和 cluster 写入可能重扫；不将多次合并视为互斥访问 |
| unsafe 标记（不可安全断开） | 每次单独支付区间检查及记录扫描 | 保留原 `_set_glyph_flags` 的 2 至 255 元素条件，条件不满足时不扫描记录 |
| 反转 | 按每个实际反转区间支付 info 访问 | 已有 positions 时另外支付位置记录访问；组内反转和全文反转分别收费 |
| 方向检查 | 固定控制费用及实际属性读取分别计费 | 保留 Arabic 数字与区域指示符检查的短路顺序及完整初始化数组扫描 |

簇合并继续调用原 `merge_clusters`，保留 `max_ops` 扣减和
`successful` 的既有变化。准入失败立即返回原始错误，不继续后续操作，
也不通过普通入口重做。分组和反转不新增集合容量。

## 定向测试与普通输出对照

新增四项内部测试覆盖：

- 空、单记录及 2、4、6、255、256 记录，四种 cluster level、
  有无 positions 及独立输出前缀。
- unsafe 标记的长度条件、单记录合并、原 `max_ops` 失败与输出前缀重写。
- 每个准入点拒绝后的精确费用前缀，以及清理 buffer 后普通塑形的完整输出。
- 输入与输出长度求和溢出在记录访问前拒绝。

新增测试比较共享内核的普通与准入结果，只用于验证准入行为。
普通语义另通过前片二十二个冻结矩阵的原始 Harfrust 0.3.2 输出核对；
fixture、oracle 和成本输入保持冻结。原版输出仅在签名、解压字节和分母
核对后复用，生产 Native 与 portable 配置重新执行。

RTL 消费者只调整对应的费用断言，以适应逐比较和逐区间准入；
输入、普通输出 oracle 和拒绝恢复要求保持。源码收据保存该断言修改前后的身份。

## 验证结果与证据

证据位于 [B 阶段证据目录](../evidence/text-stages-b-2026-11-02/README.md)，
使用 `grapheme-kernel-` 前缀。完整对照、成本采样和门禁分别保存收据。

三十份补丁已精确重放并核对全部 84 份源码；本片改动五份已有生产模块，
另增一份内部测试模块，其余 78 份源码保持原字节。
增量补丁 SHA-256：
`e29a5e98c90e8736559d3fdf774e058da524ecb4dbcb2e5f71d1dcd33a5f507e`。

二十二个矩阵、四配置共 10,786,752 份完整观察一致。
原版 44 份配置逐字节核对后复用，生产 Native 与 portable 各 22 份配置重新执行；
额外 invisible 普通参考的原收据继续核对。
Native 与 portable 各 218 项内部测试、102 项消费者测试通过。
收据为 `grapheme-kernel-verification.json`，其中保存生产源码、
fixture、oracle 和消费者的身份，以及 RTL 费用断言的修改前后身份。

Core 定向回归已通过 32 项：长文本布局 7 项、绘制 4 项、
阶段协议消费者 19 项及共享 glyph 存储 2 项。
日志为证据目录中的 `grapheme-kernel-core-regression.log`。
该日志 SHA-256：
`0437bfa208e5e5538724bc89c1568a153bd59ee47a05aa05d211d1bb169fdc23`。
该结果只覆盖现有 Core 调用，不代表新阶段适配已经接入。

最终源码重新采集了 768,000 条成本样本，没有沿用前片数据。
其中准入组 192,000 条，普通 singleton 与 mark 对照各 144,000 条，
圆圈与真实分解增长普通对照 288,000 条。
七个成本程序均先通过 Clippy，再构建 release 二进制并计时；
采样期间没有并行运行本轮其他编译或测试，外部系统负载仍未受控。
字体构造、计划构造、输入准备及结果释放在计时范围之外。

准入组每个配置的费用稳定，最后准入点拒绝保留预期费用前缀。
以下为各配置分位数的范围，单位为微秒，不能相加得到总延迟。

| 准入组路径 | P50 范围 | P95 范围 |
|---|---:|---:|
| 普通 | 50.458–125.625 | 60.625–156.417 |
| 无操作回调 | 92.708–168.375 | 109.417–200.875 |
| 计数回调 | 100.333–176.750 | 113.791–211.417 |
| 最后准入点拒绝 | 99.000–175.625 | 111.792–203.166 |

普通路径按照原版／生产、生产／原版、原版／生产的顺序采集三轮。
下表保留每个配置、每轮生产／原版 P95 比值的全部范围；
最后一列先对同配置的三轮比值取中位数，再列出各配置中位数的范围。
后者不能抹去前者的尾延迟波动。

| 普通路径组 | 样本数 | 原版 P95 范围（微秒） | 生产 P95 范围（微秒） | 各轮 P95 比值范围 | 同配置三轮比值中位数范围 |
|---|---:|---:|---:|---:|---:|
| singleton | 144,000 | 52.833–141.792 | 53.834–249.125 | 0.556–3.409 | 0.842–1.038 |
| mark，保留 precomposed 映射 | 144,000 | 75.833–462.459 | 78.042–391.583 | 0.355–3.255 | 0.940–1.126 |
| 圆圈与真实分解增长 | 288,000 | 100.208–464.875 | 104.375–938.167 | 0.233–2.901 | 0.987–1.109 |

个别配置的 P95 比值最高为 3.409，原版与生产两边均有明显波动。
当前数据不能区分补丁影响与系统负载影响，也不能证明普通路径没有性能回退。
本片完成的是操作计费与输出边界验证；Core 布局提速、受控尾延迟和默认配置决策
继续归 B 后续与 C／D，不据此宣称实际 Flow resize 或 16 ms／4 ms 目标通过。

十一项边界检查通过，包括消费者 Native 与 portable 严格 Clippy、
Core Native 与 Wasm check、消费者 Wasm build，以及库 Clippy 的前片对照。
库的六项既有 Clippy 诊断保持，新增诊断为零；Wasm 只构建，未执行测试。
`grapheme-kernel-gates.json` 核对七份采样前 Clippy 收据及十九份原始采样文件，
并再次重放补丁、检查生产源码字节与成本使用的源码身份。

本片新增 Rust 模块格式及四份 Python 脚本语法检查通过，
工作区 `cargo fmt --all -- --check` 通过。
普通 `git diff --check` 将第三方 `buffer.rs` 两行保留的 CRLF 判为尾随空白；
按 `cr-at-eol` 检查通过，一方代码、状态文档和消费者的普通空白检查也通过。
未重写上游换行字节，也未修改 Git 持久配置。

续验时发现共享重放工具会覆盖前片同名日志。本轮修改
`check_harfrust_context_admission.py::reconstruct`，已有日志保持原文件，
新日志使用当前临时重放目录的唯一名称。补丁哈希、应用顺序及源码字节检查保持。
隔离验证连续重放两次，检查旧日志字节不变、新日志名称互异及补丁输出相同，已通过。
此修改保护后续重放记录，不声称恢复了此前已被覆盖的日志。

复验入口：

```bash
python3 -B tools/scripts/check_harfrust_grapheme_kernel_admission.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_grapheme_kernel_cost.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target --linker rust-lld
python3 -B tools/scripts/check_harfrust_grapheme_kernel_gates.py \
  --output target/verification/text-stages-b-2026-11-02 \
  --target-dir /tmp/novadraw-text-stage-target \
  --wasm-target-dir /tmp/novadraw-text-stage-wasm --linker rust-lld
```

## 当前限制与后续

公共 raw mark、variation selector 及未知脚本回调仍在预检拒绝。
Unicode CCC 与 continuation 的生成、fallback 类别和 mark 定位仍待补齐，
已存储属性的分组内核通过不能证明这些阶段通过。

之后继续 B1 的复杂 GPOS 与脚本路径，再推进 B2 Fontique 与 Parley 分析编排、
B3 导出、B4 backing 和 C／D。Core 阶段适配、实际 Flow 宽度复用和默认启用
不属于本片完成范围，16 ms 更新与 4 ms 查询／IR 的整体验收状态保持。

成稿分别回读事实、逻辑和中文表达，保留普通输出复用范围、计费责任、
全部尾延迟波动及未启用状态。报告由作者自审，没有独立读者复述验证。
最终文档门禁通过 528 份 Markdown。中文机械扫描的标题序号、翻案句式、
元话语、中文半角标点和连续破折号均为零命中；
记录保存为证据目录中的 `grapheme-kernel-chinese-review.json`。
零命中只表示未发现字面候选，不代替技术事实与逻辑复核。
