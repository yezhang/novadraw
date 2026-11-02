# ADR-059 B：模板复制与对齐的操作前准入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的 Parley 依赖接口。
`api_semantics`：`text.flow`、`text.interaction`，既有语义保持。
Editor、backend、platform 无新增能力。

B 已补齐模板复制和对齐的操作前准入接口，并通过独立消费者和 Core 会话预算验证。
后续[断行切片](text-stages-b-line-breaking-2026-11-02.md)已完成依赖准入及定向验证；
以下数量和源码保持范围记录本切片当时的状态。
接口位于生产依赖中，当前 Parley 引擎和 Flow 尚未调用它们。
断行、分析／shaping、导出计费及阶段适配仍未完成，B 保持进行中。
本次没有启用宽度复用，也没有新增运行时提速结论。

## 实际改动

[操作模块](../../../dependencies/parley-0.7.0/src/layout/operations.rs)
新增两个入口。它们接受统一的步骤费用回调，回调拒绝后立即返回原始错误：

| 入口 | 准入顺序 | 拒绝后的状态 |
|---|---|---|
| `Layout::try_clone_admitted` | 数组头规划 → 全部复制／容量费用 → 十个 fallible 预留 → 元素复制 | 原模板不变；预留失败时目的数组尚未复制元素，临时预留释放 |
| `Layout::try_align_admitted` | 行／行项规划 → 撤销 Justify 与新对齐的全部扫描费用 → 既有 `align` | 准入拒绝时几何和数组容量保持；已准入的规划费用不退回 |

复制入口对所有 `Copy` brush 开放。它直接复制 brush，包括 underline 和
strikethrough 的 brush，不调用可能自定义的 `Clone`。
其他数组元素均为固定大小值或字体的共享 Blob 引用；
FontData／Blob 的复制只增加 Arc 引用，不复制字体字节，也不创建字体上下文。
有额外分配行为的通用 brush 不属于这个入口的合同。

复制按十个数组的当前长度收费，而非原容量；目的数组用
`try_reserve_exact` 预留当前长度。所有预留完成后才开始复制，
因此中途预留失败不会产生待销毁的部分字体或 brush 元素。
不可变借用把计划和执行绑定到同一模板，不暴露可在模板变化后继续使用的独立计划。
新 LayoutData 字段会使完整结构初始化编译失败，必须同步复核复制与计费。

`LayoutOperationError` 分别保存 admission 原始错误、失败数组及
`TryReserveError`、算术溢出和无效行范围；没有把它们统一转成缓存 miss。
这些是依赖类型，Novadraw 公开协议没有重导出它们。
Core 使用时仍经过 ADR-059 的 `TextStageContext::charge` 和绑定检查。

## 成本依据

| 步骤 | work 上界 | allocation |
|---|---|---|
| 复制规划 | 十个数组头读取及快照遍历，共 20 个元素访问 | 0 |
| 复制执行 | 十个数组初始化元素数之和，每个复制一次 | 同一元素数，表示目的数组新增容量项 |
| 对齐规划 | 当前行数＋行项数，验证范围并累计 cluster 区间长度 | 0 |
| 对齐执行 | 每个对齐 pass 的行扫描；每个 Justify pass 另计全部行项及其 cluster 区间扫描 | 0 |

对齐的来源是 Parley 0.7.0 `alignment.rs::align_impl`。
旧 Justify 的撤销与新 Justify 分别计算，不把撤销隐去。
cluster 回调达到 `num_spaces` 后仍会被迭代器访问，这些访问也在上界内。
计划包含可能因宽度、段尾或空白条件提前结束而未执行的扫描；
未用完的保守预留不退回。规划只读元数据，不先运行对齐来计算费用。
行项区间必须构成完整分区，cluster 区间须有效；不符合时在 mutation 前拒绝。

容量项依据是当前 Rust 1.96.0 的 Global Vec exact-reservation 实现：
[RawVecInner::grow_exact](https://doc.rust-lang.org/1.96.0/src/alloc/raw_vec/mod.rs.html#785-804)
使用 `len + additional`，由 `set_ptr_and_cap` 保存该逻辑容量。
复制从空 Vec 开始预留，每个非零大小数组的实际容量等于当前源长度；
消费者验证了这一关系。该依据不承诺物理 allocator 的分配粒度或 bookkeeping。
升级工具链必须重新核对实现，不能把公共 `reserve_exact` 文档中的
“至少容量”表述直接推广成任意工具链的精确容量证明。
留存字节仍由既有 `owned_storage` 和共享 backing 账本另行负责。

## 已执行验证

| 验证 | 结果 |
|---|---|
| 私有操作测试 | 4 项通过：容量溢出及原始 source、长度复制、无效行范围、非平凡 Clone 的 Copy brush |
| 独立消费者默认／portable | 各 5 项通过，包含各 960 组复制后对齐转换 |
| 四个准入拒绝点 | 模板、容量及几何保持；后续步骤不执行 |
| Core 会话预算 | 3 项通过：累计 work／allocation、复制分配拒绝后的粘滞重试、对齐规划成功后执行拒绝 |
| Wasm portable 消费者 | library／cdylib 构建通过；未执行 Wasm 测试 |
| Core Wasm | `--lib` check 通过 |
| Workspace | `cargo check --workspace` 通过；保留既有 `block 0.1.6` future-incompatibility 提示 |
| Clippy | 消费者 `--lib` 和 Core 定向 test 的 `-D warnings` 通过 |
| Parley 严格 Clippy | 修正新模块两条字面量后缀格式问题；最终与原始版本相同的 7 条上游错误保留，不宣称整体通过 |

960 组由 3 种文本 × 2 种 inline 配置 × 5 种宽度 × 4 种旧对齐 ×
4 种新对齐 × 2 种 overflow 策略构成；复制对照为 120 组。
此前进度消息的 1,280 组为估算错误，已按实际循环修正。
既有只读对照也应为 120 组，旧报告的 160 组在本轮一并更正。
文本覆盖空内容、组合字符／连字／ZWJ、
CJK／Arabic 混排和末尾换行；宽度覆盖无约束、零宽、极窄及普通宽度。
对照检查字体身份、glyph、metrics、位置、styles、inline 和实际数组容量。
这些依赖级对照不替代 C 的 Core 来源／interaction／资源 revision 全输出等价。

Core 首次测试修正了两个测试夹具问题：没有显式指定登记字体导致空 shaping，
以及错误地比较源 Vec 和派生 clone 的容量。最终夹具要求存在字体和多行，
拒绝后容量与源快照比较，几何与独立 clone 比较。

本轮为本 Agent 自审和工具验证，尚无独立评审。
上游 `parley_dev` 不随发布包提供，私有测试用临时副本排除上游测试模块；
未运行上游全套测试，该 harness 不进入生产依赖。

复现入口：

```bash
cargo +1.96.0 test --locked --manifest-path tests/consumers/parley-storage/Cargo.toml --lib
cargo +1.96.0 test --locked --manifest-path tests/consumers/parley-storage/Cargo.toml --no-default-features --features portable --lib
cargo +1.96.0 test -p novadraw --test p2_parley_operations_contract
cargo +1.96.0 clippy -p novadraw --test p2_parley_operations_contract -- -D warnings
```

来源、实际源码及散列见
[操作验证 receipt](../evidence/text-stages-b-2026-11-02/operations-verification.json)。
[操作补丁](../../../dependencies/parley-0.7.0-operations.patch)
在已经接受的存储补丁之后应用；旧存储补丁及历史验证证据保留。
原始 Rust 与许可证仅 `layout/mod.rs` 有模块接入改动；
`alignment.rs`、`line_break.rs` 和 shaping 算法保持上游字节。

## 本切片结束时的后续工作

断行必须在扫描及扩容前准入，并覆盖边界回退、空末行补项、Bidi 重排和
`BreakLines::Drop` 的最终行扫描。预算失败后不能依靠 Drop 隐式完成未准入的工作。
这次新增的对齐接口不能替代断行合同。

分析／shaping 必须覆盖字体选择、冷 ShapePlan、字体表处理、UnicodeBuffer 和
临时集合；Harfrust `max_ops` 不是这些工作的统一上界。
依赖没有提供必要依据时，analyze 在 shaping 前返回成功缺失，
不登记无法履行后续 reflow 合同的资产。

完成上述接口后，继续同一字体集合的显式阶段适配及导出计费，
再进入 C 的 Flow 全输出验证与 D 的匹配性能采样。
当前普通配置仍走完整布局，原有性能样本不用于证明本次接口的提速。
