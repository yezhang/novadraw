# P2-T02 Text Interaction Geometry 完成记录

类型：`verification`

日期：2026-09-29

结论：`complete`

## 1. 完成范围

- `TextLayout` 可携带 backend-neutral `TextInteractionMap`；
- 默认 Parley 实现复用其 cursor、selection、cluster 与 bidi 结果；
- 外部 TextLayoutEngine 可通过 `TextInteractionProvider` 提供同等查询能力；
- 文本位置使用 UTF-8 byte offset、affinity 与 layout revision；
- TextFlow 将 flatten 后 offset 映射为 paragraph-local `FlowTextPosition`；
- Runtime 提供 surface-domain hit-test、caret 与 selection geometry；
- visual cluster、word、line、paragraph 和 document movement 走同一布局事实；
- truncated range、stale revision、非法位置和错误 Figure capability 均结构化拒绝。

## 2. 自动验证

Suite：`core.p2-t02-text-interaction`

结果：PASS，11 项契约覆盖：

- caret surface round-trip；
- emoji ZWJ 与 combining grapheme movement；
- soft-wrap affinity；
- 跨 paragraph selection quads；
- paragraph boundary 与 fragment 内 hard break 区分；
- truncated/UTF-8/stale revision 拒绝；
- nested viewport + scale transform；
- external interaction provider；
- unknown/wrong-capability 错误；
- 查询不产生 Runtime update。

回归结果：

- `cargo test -p novadraw`：PASS；
- `cargo check -p novadraw --target wasm32-unknown-unknown`：PASS；
- `cargo xtask verify core.p2-t02-text-interaction`：PASS；
- `cargo xtask check --full`：PASS。

## 3. 边界

P2-T02 不保存 caret、selection、draft 或 IME composition，也不绘制编辑 feedback。
这些 mutable 状态继续由 P2-E02 Editor direct-edit session 承担。
