# Parley 存储补丁验证证据

类型：`verification-evidence`

当前 Harfrust 生产核对见 `lookup-verification.json` 与
`harfrust-0.3.2-lookup.patch`；按 buffer、instance、variation、cold 四份冻结
补丁重放后追加该增量。前四份 receipt 保持。
`lookup-cost-summary.json` 与 `lookup-cost.ndjson` 保存局部缓存成本，
范围与限制见[lookup 记录](../../reviews/text-stages-b1-lookup-cache-2026-11-02.md)。

能力归属：Core 文字依赖的隔离验证。
结论与批准范围见[复核记录](../../reviews/text-stages-b-storage-patch-2026-11-02.md)。

`parley-0.7.0-storage.patch` 是可评审的实际补丁，
`layout_storage.rs` 是其中新增模块的可读副本。
`verification.json` 记录原始归档／补丁散列、源码保持核对、测试结果与
上游 Clippy 差异；复核脚本会验证这些条件后重新生成该文件。

日志保留 5 项私有测试、两种 Native 配置各 3 项消费者测试、
Wasm portable 构建及消费者 Clippy。
`baseline-clippy.jsonl` 与 `patch-clippy.jsonl` 记录双方相同的 7 条上游错误。
本目录不提供 Parley 全套上游测试、默认阶段适配或 Core 完整等价通过结论。

复现先按[消费者说明](../../../../tests/consumers/parley-storage/README.md)
准备副本并执行对应命令。以下另生成未打补丁的 Clippy 对照副本：

```bash
python3 tools/scripts/prepare_parley_storage_candidate.py --baseline \
  --archive /path/to/parley-0.7.0.crate
```

在本目录相应日志路径保存 `--message-format=json` 的候选与对照 Clippy
结果后，用同一原始归档执行检查：

```bash
python3 tools/scripts/check_parley_storage_candidate.py \
  --archive /path/to/parley-0.7.0.crate
```

隔离副本为定向私有测试追加独立 workspace、测试 feature 和上游测试模块门控；
这些 harness 改动由准备脚本明确生成，不包含在依赖候选补丁中。

`production-perf-summary.json` 与 `production-perf-samples.ndjson.gz`
保存本轮正式 CPU 采样的汇总和未过滤原始记录；gzip 解压后恢复原始 NDJSON。
`production-perf-receipt.json` 核对全部 1,200 个有效样本的分母、
扩展计数、二进制及样本散列。
`production-perf-manifest.json` 和五份 `production-perf-session-*-environment.json`
记录冻结配置、电池供电与会话后台负载。
结果和适用限制见[验证指南](../../performance/text-optimization-validation.md#本轮实际采样)。
这些文件只记录当前单版本成本，不证明宽度复用收益。

后续操作增量见 `parley-0.7.0-operations.patch`、`layout_operations.rs` 和
`operations-verification.json`；其范围及准确分母见
[复制与对齐记录](../../reviews/text-stages-b-operations-2026-11-02.md)。
旧存储 receipt 的 160 组分母实际为 120 组；原始 receipt 字节保持，
不将后续验证混入此前已接受的只读补丁证据。

`analysis-dependency-source.json` 保存分析必经依赖的锁定归档散列、
源码一致性数量与十个入口文件的散列。
四个 package 的 252 个 `src/` 文件与原始归档一致；
该 receipt 仅确认静态分析来源，不证明完整计费上界或性能。
费用缺口与已批准补丁范围见
[分析准入提案](../../../design/architecture/text-analysis-admission-proposal.md)。

`harfrust-0.3.2.crate`、`harfrust-0.3.2-buffer.patch` 与
`buffer-verification.json` 保存 B1 输入 buffer 的原始来源、精确补丁重放、
Native／portable 普通入口对照及私有测试分母。
`buffer-consumer-*`、`buffer-core-*` 与 `buffer-parley-regressions.log`
保存外部、Wasm 构建与接入回归结果。范围见
[B1 buffer 记录](../../reviews/text-stages-b1-buffer-2026-11-02.md)；
这些证据不声明冷构造或 shaping 已受准入。

`buffer-cost.ndjson` 保留六组共 6000 个输入计时及实际准入费用，
`buffer-cost-summary.json` 校验每组完整分母、预期拒绝及费用，
同时记录环境和源码／二进制散列。它只观察输入 buffer 的本地开销，
不作为 Core 宽度复用收益证据。

`harfrust-0.3.2-instance.patch` 按顺序应用在冻结的 buffer 补丁之后。
`instance-verification.json` 保存增量重放、其他 71 个源码文件的字节保持，
以及原始／生产 Native／portable 四组普通实例和 glyph 对照散列。
`instance-consumer-*` 记录坐标拒绝、spill、条件分支及 Wasm 构建；
`instance-cost.ndjson` 与 `instance-cost-summary.json` 记录重复新建规范化实例的
本地费用。范围见 [B1 instance 记录](../../reviews/text-stages-b1-instance-2026-11-02.md)，
不覆盖用户坐标转换、完整冷构造或 shaping 准入。

`harfrust-0.3.2-variation.patch` 应用在上述两个冻结补丁之后。
`variation-verification.json` 保存增量字节重放、其余 73 个源码文件保持，
四组用户／命名实例对照及私有库测试散列。
`variation-cost.ndjson` 与 `variation-cost-summary.json` 保存 9000 个本地构造样本。
范围见 [B1 用户坐标记录](../../reviews/text-stages-b1-variation-2026-11-02.md)；
avar2、完整冷构造与 shaping 仍未完成，不证明 Core 端到端性能通过。

`harfrust-0.3.2-aat-headers.patch` 接续 buffer、instance、variation、cold、lookup
五份冻结补丁。`aat-verification.json` 保存逐字节重放与四组各 4,968 份普通输出对照；
`aat-private-*` 和 `aat-consumer-*` 保存 Native／portable 验证与 Wasm 构建。
`aat-cost.ndjson` 与 `aat-cost-summary.json` 保存四种合成空表头、
四种模式共 16,000 个冷构造样本。
范围见 [B1 AAT 表头记录](../../reviews/text-stages-b1-aat-headers-2026-11-02.md)；
非空 AAT、ShapePlan 和 shaping 准入仍待完成。

`harfrust-0.3.2-plan.patch` 追加到前六份冻结补丁后。
`plan-verification.json` 核对四个既有源码文件、其余 74 个文件的原始字节、
四组各 39,744 份普通观察、Native／portable 外部与私有测试。
`plan-cost.ndjson` 与 `plan-cost-summary.json` 保存 16,000 个 fresh plan 样本、
分母和逐组费用。范围见 [B1 计划记录](../../reviews/text-stages-b1-plan-2026-11-02.md)；
布局表计划、配置与 shaping 准入仍待完成。
