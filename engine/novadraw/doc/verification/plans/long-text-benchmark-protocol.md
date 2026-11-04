# 长文本基准协议

类型：`verification-plan`

能力归属：Core Text/Flow/Runtime 的 CPU 验证，backend 编码/GPU，
platform 实际呈现；不同 owner 的证据分别报告。

本页承接长文本方案 LT-00 的采样方法，不新增性能目标或实施授权。
当前结果与不足见[优化验证](../performance/text-optimization-validation.md)；
命令入口见[tests/README.md](../../../tests/README.md)，
`text.long-layout` 由 xtask 编排 Core 契约测试和 Perfetto 研究宿主。
2026-11-04 起自研采样器及 `textflow-perf` 已删除，当前采集与丢失检查按
[Perfetto 合同](../../design/architecture/perfetto-observation.md)执行；
本页的冻结要求继续用于历史证据与后续匹配比较。

## 可恢复基线

| 类别 | 冻结内容 |
|---|---|
| 源码 | 完整 commit、可恢复 dirty patch/未跟踪源码及 SHA-256、Cargo.lock、驱动版本 |
| 构建 | rustc/cargo、target、profile/features、优化/debug/lto/codegen、环境与依赖 |
| 字体 | 字节 SHA-256、face、描述符/字号/变体、注册/fallback 顺序及状态 |
| 输入 | fixture/hash、字节/段落/来源/节点、style、border、inline 与每轮变更 |
| 几何 | 实际 client width、insets、wrap/方向、origin、DPI、transform、surface |
| 环境 | OS、CPU/GPU/backend/capability、刷新率、供电/负载、worker 与预算 |
| 采样 | case、预热/样本数、会话、种子/顺序、超时/失败及统计方法 |

比较只允许待测实现差异，其他输入一致；不可匹配项单列。
不能由初始 bounds 推断实际 width-tracking viewport 约束，
只保存 HEAD 或分支名不足以恢复 dirty 基线，字体来源缺失不能作为可比样本。

## 场景与事件

驱动支持 cold-process、fresh-runtime、warm-layout、source-append、scroll、
resize 与 render/present，输出机器可读原始样本。
每轮 append 从相同基准追加相同字符，重建/恢复在计时外完成并验证快照；
不让文本量随样本数增长。冷/热缓存、字体/GPU 未就绪、场景切换、旧宽度缓存
及淘汰分开编号，不用一次 Reset 冒充全部条件。

输入组包括原官方 40 段/501 来源、10/100 倍段落、同总量巨段、NoWrap 长行、
1/10 MiB 文档及百万逻辑行、CJK/Arabic/Hebrew/ligature/emoji/组合字符、
混合字号/fallback、嵌套 block/跨段 border/margin/真实 inline 与 route。
这些是计划分母，不是已经测得的数据。逐组记录字节、段落、来源、spans、行、
glyph、可见范围，分别记录 DPI 1/2、窄/宽 viewport、Native/Web、glyph/outline。

## 计时边界

使用进程单调时钟及整数纳秒；跨进程/OS 时间戳必须受检对齐。

| 指标 | 起点 → 终点 |
|---|---|
| 冷启动 | launcher 发起 → 带请求/帧身份的首次就绪或实际呈现，两者分别报告 |
| 构造 | fixture 工厂调用前 → Runtime 返回 |
| 首次布局 | 首次稳定化前 → 完整稳定快照，不含构造 |
| 分发 | 固定事件投递 → 分发/入队返回 |
| 源提交 | 请求准入 → 对应 receipt/提交事实 |
| 派生 | 已提交工作 → layout/包络/索引稳定发布 |
| 兼容追加口径 | 事件投递 → 本次稳定查询返回 |
| 查询/IR | 稳定快照录制前 → 受检录制/帧准备，发布另计 |
| backend/GPU | 消费帧 → 编码/上传/提交/可用完成，各段分别报告 |
| 输入到呈现 | 固定投递 → 关联新内容实际呈现 |

deferred dispatch 返回不等于源提交，必须关联 request/revision/receipt。
稳定查询、录制、surface 提交分开，不消费另一场景 damage 或在途状态预热。
prepare 返回、fence 或帧率不能替代实际呈现；无法采证填写 unavailable。
同步 CPU 累计前台准备工作，排队、worker 和等待另计，端到端包含全部等待。
inclusive/exclusive 与失败前工作均保留；tracing 模式及开销一致，不能混比开关状态。

## 统计与退出

- 冷启动每 case 至少 200 个独立进程，分布于 5 个会话；构建不计时，
  OS/进程/字体/文件缓存条件逐项记录。
- 热布局、append、scroll、resize 每 case 5 个会话；每会话 20 次预热、
  200 次有效采样，目标合计 1000。对比版本相同协议、交错执行。
- 会话内保持顺序，GPU 按帧身份关联；并发压力另列，不混入单实例分布。
- 排序后分位取 `ceil(p × n)`；分别报告会话、合并 p50/p95/p99、最大值及 n，
  不把各会话 p95 平均称为总 p95。
- 失败、超时、预算拒绝与无呈现轮次保留并单列；不删离群、不填零。
  未达计划标为未完成，较大 case 的协议须在采样前修订冻结。

每会话 p95 满足冻结目标，工作量门槛及行为反例同时通过才关闭性能验收。
p99 不构成任意尾延迟上界；置信区间须先冻结算法、抽样单位与种子。
记录最大连续 UI 阻塞、峰值内存、预算原因、缓存命中/淘汰、输入字节、
段落重算、fragment 检查、复制、geometry writes、revision 扫描、候选及 worker 副本。
热路径只收集，离开后汇总，不打印逐帧日志。

原始 NDJSON/CSV、manifest、构建/运行退出码、汇总版本和输出保存在
`target/verification/text-*/`，不纳入 Git；汇总重算分位和阶段关联，
校验 n、身份及单位，不仅保存摘要。不同日期或环境的基线分别保留，不覆盖旧结果。

正确性以同版本/字体/宽度的未优化精确路径对照测量、断点、glyph/source、
caret/selection、Figure placement 和 Full/Partial/旧区清理。
后续渐进/异步覆盖须最终与精确结果一致，强制乱序、删除、字体替换、快速 resize、
取消、暂停恢复、预算及 reset。方法存在不等于候选已获准或测试已执行。
