# ADR-059 B：断行准入已补齐

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的 Parley 依赖。
`api_semantics`：`text.flow`、`text.interaction`。
Editor、backend、platform 无新增能力。

B 已补齐候选 Layout 的断行准入，包括回退重扫、行数组扩容和最终汇总。
依赖消费者、Core 会话预算及上游普通入口对照已通过定向验证。
分析／shaping、导出计费和阶段适配仍待完成；宽度复用尚未启用。
本记录延续[复制与对齐切片](text-stages-b-operations-2026-11-02.md)，
不将依赖级接口验证记为 Flow 性能验收。

## 候选失败后直接释放

`Layout::try_break_all_lines_admitted` 消费一个候选 Layout。
它先对旧 Justify 的撤销准入，再使用同一断行算法生成新行。
成功时显式准入最终的尺寸汇总，然后返回候选。
任一步准入或容量预留失败时，调用者只收到错误，候选释放；
另行保留的模板不变，已准入费用不退回。

原来的 `BreakLines::Drop` 会扫描已生成行并发布尺寸。
新入口在构造 breaker 后立即关闭隐式汇总，失败和 panic 退出均不会走该扫描。
成功路径显式执行汇总和交换；普通迭代器仍保留原来的 Drop 行为。
私有哨兵测试分别验证了拒绝后不发布尺寸、普通 Drop 仍发布部分行。

新入口限定 `Copy` brush，避免候选销毁涉及任意 brush 的自定义 Drop。
底层普通入口仍支持原有 Brush 类型。Novadraw 公开协议不重导出这些依赖类型。

## 扫描和扩容先计费

| 步骤 | 操作前费用依据 |
|---|---|
| item／cluster 主循环 | 每次 item 与 box／run、cluster 与 style 访问；边界回退后的再次访问重新计费 |
| ligature 探测 | 每次 `Run::get` 探测前计费，包括最后一次未命中 |
| 行提交 | 当前 item 区间的 6 倍：正向定位、反向定位、构造遍历，以及 run 和两个端点 cluster 的保守上界 |
| 行 metrics | 当前行与前行读取、行项与 run／box；每个 text run 的完整 cluster 区间分别预留空白检查与 advance 求和 |
| 尾部空白 | 再预留对应 cluster 区间的完整扫描；空末行另计反向 run 查询及新增行项 |
| Bidi 重排 | 先计一次行项扫描，得到实际 level 范围；每个 level 预留 3 倍行项数，覆盖外层、内层探测和不相交区间的交换 |
| 行数组扩容 | 先计潜在迁移的已有元素及整次新容量，再 `try_reserve_exact`；每个写入另计一次 |
| 最终汇总 | 新行数；空文本另预留一次首行项规范化 |

`work` 表示元素访问上界，`allocation` 表示集合容量项；
allocator bookkeeping 与物理分配粒度排除。
行和行项数组使用容量翻倍的几何增长，空数组从一个容量项开始。
扩容费用保守计入完整新容量，包含原槽位；因此不会少计临时搬迁，
也不会因逐项 exact 预留产生二次增长成本。
逻辑 exact 容量继续依赖[复制切片记录](text-stages-b-operations-2026-11-02.md)
中的 Rust 1.96 Global Vec 依据，升级工具链须重新核对。

普通与准入入口共享计算内核，通过私有泛型策略区分费用回调和容量预留。
普通策略没有准入回调、检查算术或 fallible 扩容；既有断行顺序、数学计算、
空文本处理和迭代器回退行为保持。
本次修改不触及递归渲染主循环。

## 已验证的范围

| 验证 | 结果与限制 |
|---|---|
| 独立消费者 default／portable | 各 8 项测试通过，包含 640 组准入／普通断行对照 |
| 640 组构成 | 4 种文本 × 2 种 inline 配置 × 4 种 wrap 配置 × 2 种旧宽度 × 2 种旧对齐 × 5 种新宽度 |
| 逐个拒绝点 | 三种文本的每次回调均逐个拒绝；保留原错误、后续回调停止、模板容量保持 |
| 长段落成本 | 128／256／512 个重复段落；输入翻倍时计费上界保持线性，输出与普通断行一致；这项不证明毫秒延迟 |
| 原始 Parley 对照 | 960 组，每组分别比较对齐后、再次断行、迭代器回退并 Drop 的结果，共 2,880 个输出观察 |
| 私有 Drop 测试 | 2 项通过；隔离副本排除了发布包缺失的上游测试支持，不宣称上游全套测试通过 |
| Core 会话预算 | 5 项定向测试通过；累计 work／allocation、分配拒绝粘滞、最终汇总拒绝均覆盖 |
| Core 库回归 | 446 项通过，1 项忽略；包含既有文字、Flow、资源及 Runtime 测试 |
| Wasm portable | 消费者 library／cdylib 构建通过；未执行 Wasm 测试 |
| Clippy | 消费者 `--lib` 与 Core 定向测试通过；Parley 严格检查仍为原始版本的 7 条上游错误，新增修改未增加错误 |
| Workspace 与文档 | check、fmt、差异空白检查通过；496 份 Markdown 通过文档门禁；既有 `block 0.1.6` future-incompatibility 提示保留 |

上游对照先验证 crates.io 原始归档 SHA-256，再核对 baseline 的全部 `src/` 文件。
相同夹具在原始依赖和生产补丁中独立运行，结果文件 SHA-256 一致；
包括字体身份、glyph、行 metrics、位置、styles 和 inline。
这些对照仍不能替代 C 的 Core 来源、interaction 和 revision 全输出等价。
本次为 Agent 自审与工具验证，没有独立评审结论。

复现入口：

```bash
python3 tools/scripts/check_parley_line_admission.py \
  --archive /path/to/parley-0.7.0.crate \
  --target-dir /tmp/novadraw-text-stage-consumer \
  --output target/text-line-admission-evidence
cargo +1.96.0 test --locked \
  --manifest-path tests/consumers/parley-storage/Cargo.toml --lib
cargo +1.96.0 test -p novadraw --test p2_parley_operations_contract
```

源码、补丁及检查结果见
[断行 receipt](../evidence/text-stages-b-2026-11-02/line-verification.json)；
上游对照详情见
[oracle receipt](../evidence/text-stages-b-2026-11-02/line-oracle-receipt.json)。
新增[断行补丁](../../../dependencies/parley-0.7.0-line-breaking.patch)
在只读存储与复制／对齐补丁之后应用；旧补丁和原始证据保持。

## 分析计费仍是下一项前置条件

源码核对确认，Parley `LayoutContext::analyze_text` 会增长 info 与 Bidi 临时数组；
`ShapeContext` 的三个 LRU 查询有线性扫描、键集合复制和冷构造。
`shape_item` 还会进入字体查询、冷 ShaperData／Instance／ShapePlan、
UnicodeBuffer 扩容和 glyph 导出。

锁定的 Harfrust 0.3.2 中，`hb_buffer_t::enlarge` 直接增长 info／pos 两个 Vec，
`ShapePlan::new` 则先收集 feature 再编译计划。
这些操作发生在 glyph 输出和部分 `max_ops` 递减之前。
现有接口未提供覆盖全部必要扫描与容量项的操作前计划，
因此当前不能把 `max_ops`、字符数倍数或已有 warm cache 当作完整计费证明。

继续 B 时须先补齐这些依赖操作的费用依据与拒绝通道，再接阶段生产者。
依据未知的 analyze 必须在 shaping 前返回成功缺失；不能先 shaping 后记零费用，
也不能登记无法履行后续 reflow 合同的资产。
详细边界继续以 [ADR-059](../../adr/adr-059-staged-text-width-reuse.md) 为准。
