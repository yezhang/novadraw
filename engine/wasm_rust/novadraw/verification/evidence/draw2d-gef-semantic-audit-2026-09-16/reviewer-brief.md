# 分组审计契约

范围：当前工作区全文件语义审计（用户明确要求），不受 diff 行过滤限制。
项目：`/Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw`。
Git 根：`/Users/bytedance/Documents/code/GitHub/drawjs`。
参考：`/Users/bytedance/Documents/code/GitHub/gef-classic`，只允许
`org.eclipse.draw2d/src`、`org.eclipse.gef/src` 及两者官方 `*.doc.isv/guide-src`。
禁止使用 Zest；默认不读 `doc/archive`。只写本次 evidence/group 下本组报告。
本次不修复产品代码、测试或既有文档，不提交 Git。

主 Agent 已完整读取 analyzing-gef-code 和 bits-code-guard 的技能与通用评审、
维度、评级说明。各组执行：

1. 先读项目 AGENTS.md、CLAUDE.md，再读相关官方指南、Java 方法/Javadoc，
   写出基线契约，然后核对 Rust 完整函数、直接调用方及验证。
2. 对逻辑、业务语义、并发、健壮性、性能、安全、质量逐项判断；
   Rust 无该技能语言专项规则。不要仅因类型名称相似就断言等价。
3. 按 bits-code-guard/references/review-dimensions.md 与 review-rule.md
   的原则定级：有具体触发路径才报功能缺陷；防御性不足最高 P2；
   纯风格不报缺陷；不确定外部假设须降信。P1 应有明确输入或时序。
4. 产物 `group_N.md`：
   - 实际阅读范围与官方证据；
   - 至少 8 条详细语义映射（Java 类/方法、语义、Rust API、状态、证据/局限）；
   - 缺陷（精确 Rust 行号、触发、结果、严重度、置信度、建议）；
   - 合理迁移、明确收窄、后置能力分别记录；
   - 外部可扩展性、算法复杂度和 Rust API 评价；
   - 不声称未运行的测试通过。
5. 确认缺陷另写 `group_N.jsonl`，每行对象字段：
   title,file,start_line,end_line,severity,category,confidence,rationale,suggestion。
   file 以 Git 根为准，即 `engine/wasm_rust/novadraw/...`。
   至多列本组最明确 3 条，能力收窄不必伪装 bug；没有缺陷可为空。

已有 2026-09-15 报告仅可作线索，必须用当前文件重新核实；尤其已修复连接原子性、
Locator runtime、Anchor descriptor 与增量索引，不得把历史问题照搬。
共享文件按本组方法职责阅读；测试统一由主 Agent 运行，避免 Cargo 竞争。
