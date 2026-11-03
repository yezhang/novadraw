# ADR-059 B1：Harfrust 输入 buffer 准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的
Harfrust 0.3.2 生产依赖；Editor、backend、platform 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
状态：输入 buffer 子片已实现并通过定向验证；B1 冷构造与 shaping 未完成，
B／C／D 保持未关闭，宽度复用未默认启用。

用户已批准[分析准入提案](../../design/architecture/text-analysis-admission-proposal.md)
限定的三个依赖版本。本片先补输入 buffer 的实际拒绝通道。
此前断行、复制和对齐证据保持，不将本片通过解释为完整分析准入。

## 输入拒绝通道与费用

新增 `UnicodeBuffer::try_reserve_admitted`、`try_push_str_admitted`、
`try_add_admitted`。入口接收泛型回调，申报操作、work 与 allocation，
不依赖 Novadraw 类型。`BufferOperationError<E>` 保存回调原始错误或
`TryReserveError` 的 source，并区分长度限制和成本溢出。

| 操作 | 执行前申报的依据 |
|---|---|
| Scan | 首次 UTF-8 计数以输入字节数为访问上界 |
| Planning | 读取 info／pos 两个数组的长度与容量 |
| Storage | 需扩容的数组申报完整新容量槽数，以及可能迁移的旧初始化元素 |
| Initialize | 两个数组新暴露区间的元素数之和 |
| Append | 第二次 UTF-8 解码的字节上界，加实际字符记录写入次数；单字符入口只收一次记录写入 |

allocation 是集合容量槽，不是字节。精确预留使用 Global
`Vec::try_reserve_exact`，在 Rust 1.96 下验证逻辑容量；
物理分配器的对齐和 bookkeeping 不进入此单位。
有 spare capacity 时不重复收取新容量，但清空后重新初始化仍收费。
不以每字符固定倍数替代冷构造或 shaping 费用。

两次预留和初始化准入都成功后，才同时调整数组长度。
普通和准入入口共享初始化及字符写入内核，普通分配策略保持。
追加准入成功后才增加逻辑字符数；拒绝不发布额外输入、
不清空已有 post-context，也不破坏两个数组长度一致的约束。
拒绝可能保留已完成的 spare capacity，已发生费用不退回。
这些数组的元素为 Copy 数据，无元素析构或隐式 shaping finalize。

普通 shaping 内的 `ensure` 仍是原路径。本片不承诺后续冷缓存、字体表 helper、
计划、变量轴或 glyph 替换已受准入；Parley／Core 尚未选择新增输入入口。
这些边界将在 B1 后续片继续补齐。

## 定向验证

证据目录为 [text-stages-b](../evidence/text-stages-b-2026-11-02/README.md)，
原始来源和补丁重放见
buffer receipt（`target/verification/text-stages-b-2026-11-02/buffer-verification.json`）。

| 范围 | 结果与分母 |
|---|---|
| 来源及补丁 | 73 个原始 `src/` 文件核对；新增一个模块，修改 buffer／lib 两个入口；未改源码逐字节保持；补丁重放精确恢复所有修改字节 |
| 外部 Native／portable | 各 3 项消费者测试通过；三种字体下 36 组普通／准入输入对照；逐点拒绝、source downcast、预算粘滞与后续普通 shaping 可用 |
| 私有 Native／portable | 各 5 项通过；冷／复用容量、迁移／初始化计费、每步拒绝、数组不变量、cluster、post-context、长度限制及溢出 |
| 原始／生产普通路径 | Native 与 portable 四组各 216 个输入，每个观察首次与恢复后输出，共 1728 份完整 glyph 信息／位置签名；四组散列一致 |
| Wasm | portable 消费者构建通过；没有执行 Wasm 测试 |
| Core 接入回归 | 既有 5 项 Parley 会话预算测试及 Core Native／Wasm `--lib` check 通过 |
| 既有 Parley 消费者 | 8 项回归通过 |
| 格式及消费者 lint | 新模块格式与独立消费者 Clippy 通过 |

原始发布包的私有测试命令被未缓存的 benchmark 开发依赖 `plotters` 阻挡。
对照脚本生成仅包含原始 build dependencies 的隔离 library manifest，
运行同一生产 Rust 源码及私有测试；没有改动生产开发依赖声明。
该脚本同时保留原始归档、混合换行补丁与原始版本输出。

输入成本采样完成六组各 1000 个原始记录，6000 个记录全部符合
预期成功／拒绝和逐样本费用；每组样本编号均完整。
环境为 macOS 14.7.8 arm64、Rust 1.96.0 release，
输入为 41984 字节、25600 个字符。汇总与二进制／源码散列见
采样 receipt（`target/verification/text-stages-b-2026-11-02/buffer-cost-summary.json`）。

| 输入路径 | 冷 buffer P95（μs） | 复用容量 P95（μs） |
|---|---:|---:|
| 普通输入 | 68.791 | 48.000 |
| 准入输入 | 49.666 | 52.708 |
| Append 前拒绝 | 19.375 | 20.083 |

顺序采样仅供观察本机输入开销，冷样本计时包含前一空 buffer 存储的释放；
复用容量的初始化仍在计时和费用内。范围不包含 shaping、Core 预算或宽度复用，
回调只有本地计数，不执行真实 Core 会话检查。
普通与准入的尾延迟差异受采样顺序、系统负载和预热影响，
不能据此宣布 Core 性能提升，也不能代替 C／D 验收。
报告交付前已复核能力归属、费用依据、分母和中文表达；该自审不作为独立评审结论。

## 下一步

继续 B1 的冷缓存、变量轴、计划构造和必经 shaping 拒绝通道。
支持判定必须来自实际算法路径及源码依据，未知费用在操作前拒绝支持；
完整候选才允许提交缓存，不留下半成品或丢失可恢复 buffer。
Fontique、Parley 分析与导出、backing 及 Core 显式适配仍按提案顺序推进。
