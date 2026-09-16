# 分组审计任务

本次是用户明确要求的当前实现全量语义评估，`scope: full_file`，不仅检查 diff。
项目根目录 `/Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw`；
Git 根目录 `/Users/bytedance/Documents/code/GitHub/drawjs`。
先读项目 `AGENTS.md`、`CLAUDE.md`、ADR-014/015 相关部分与本组设计入口。
主 Agent 已完整读取并执行 bits-code-guard 及 analyzing-gef-code 技能。

参考源码只允许：
`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/`、
`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.gef/src/`。
基线 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。官方文档包括这些源码中的
Javadoc、package.html 和 GEF doc-files/mvc.html/terminology.html。
禁止 Zest；不扫 doc/archive。当前 Novadraw 基线 HEAD `6b83ac0` 加工作区修改。

任务顺序：

1. 从 Draw2D/GEF 源码和官方 Javadoc 提取目标语义，再查 Rust 公共入口与调用链。
2. 阅读本组 diff（减号为旧代码，加号为新代码），完整读取受检函数、直接调用方与共享定义；
   当前代码已经修复的旧问题不能报告。
3. 按逻辑、领域语义、安全、并发、健壮性、性能、质量七维度检查。
   规则参考 `/Users/bytedance/.trae-cn/skills/bits-code-guard/references/review-dimensions.md`
   与 `review-rule.md`；Rust 无额外语言专项。风格问题不列为缺陷。
4. 缺陷需具体触发条件和可定位证据，分类核心功能/条件功能/防御性不足并按 P0/P1/P2、
   置信度 1–10 定级；<5 丢弃；外部假设依赖降信。当前调用方均符合前置条件时，
   缺少防御最多 P2。限制最多 3 个高价值确定缺陷；其他语义差异可以完整记录。
5. 每组生成 `group/group_N.md`：至少 10–15 行详细语义映射（复杂组可更多）；
   列为 Family ID、Java 类/方法+源码行号、不可丢失语义、Rust 公共 API+源码行号、
   等价/合理变体/部分/后置/缺口、测试名称与证据级别。列出实际阅读的源码文件；
   不得把全文索引、测试名字存在或旧报告通过当成此次运行通过。
6. 分析扩展性和 Rust 设计合理性：ID/所有权、借用、trait/object safety、错误、
   typed constraints、可替换性；记录哪些限制改变了核心语义，不能一概用“合理迁移”豁免。
7. 给出可执行迁移建议、依赖顺序、验收场景。未实现但已排期能力不是现存 bug。
8. 确认的缺陷写入 `group/group_N.jsonl`，无缺陷可空文件；字段
   title,file,start_line,end_line,severity,category,confidence,rationale,suggestion。
   file 以 Git 根为基准，包含 engine/wasm_rust/novadraw/ 前缀。

仅写本组报告和 JSONL，不修改源代码、现有测试、规则、账本或路线图。
主 Agent 统一运行 cargo，分组不要同时运行 cargo。可提出最小复现场景。
自己的报告要区分静态确认、已有测试断言和未验证场景。
报告输出目录就是本文件所在目录。
