# ADR-023：发布包、公共路径与第三方依赖边界

类型：`architecture-decision`

能力归属：Core／Editor／Inspector／backend／platform 各自的发布、依赖与公共路径。

状态：已接受，2026-09-29；沿用原编号 021／022／035 的有效边界，替代旧包结构。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

按内部逻辑层拆 package 会迫使共享实现协议公开；聚合 facade 默认安装
Editor 和 GPU 实现又无法满足独立绘图、诊断与移植需求。

## 决策

novadraw 直接拥有 backend-neutral Core；内部领域用 module 分层，
公开领域路径唯一，crate root 不提供平铺重导出或库级 prelude。
Editor、Inspector 单向依赖 Core；Vello backend、Winit／Web adapter 独立发布，
由应用组合根组合。Core 不依赖这些可选实现，backend 不依赖 platform。

中立公共签名使用 Novadraw 领域类型；第三方类型只在私有实现和显式 interop 使用。
不为内部层保留兼容转发包。3D 保留独立扩展边界，真实消费者出现前不建空 crate。
示例与工具不构成公开包。

Perfetto tracing 也遵循该边界：Core 只在显式 `perfetto` feature 下写入 CPU
阶段事件；Native 宿主持有 SDK producer、采集会话、`.pftrace` 导出和文件策略。
`perfetto-sdk`、Trace Processor、Perfetto UI、官方 Perfetto Agent skill 和第三方
MCP wrapper 都属于可选观测工具链，不进入 Core 默认依赖、公开路径或生产 API。
trace 分析优先使用官方 Trace Processor 和 Perfetto UI；Agent 辅助可安装官方
Perfetto skill 复用 CLI 查询。`perfetto-mcp-rs` 等社区 MCP 需要单独完成本地验收后
才能进入工具配置；官方 `server stdio` 协议不能直接当作 MCP server。

## 替代与代价

维持多个无独立消费者的内部 crate 会扩大 API 与版本维护面；
默认 facade 聚合全部实现会让可选安装失效。等待第二 backend 再隔离 Vello
也不能解决当前 Core 依赖问题。代价是 0.1 breaking migration 和显式组合成本；
module 收口仍须保护内部职责，不能形成第二套公共别名。

## 关联设计

[目录与包](../design/architecture/directory-structure.md)、
[公共 API](../design/architecture/public-api-contract.md)、
[第三方依赖](../design/architecture/third-party-dependency-boundaries.md)、
[Perfetto 观测](../design/architecture/perfetto-observation.md)。

trace 诊断和配套工具记录见
[TextFlowLarge Perfetto trace 诊断](../verification/reviews/perfetto-trace-diagnosis-2026-11-04.md)。

原批准范围和修订依据见[原稿 023](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-023-crate-consolidation-and-extension-boundaries.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
