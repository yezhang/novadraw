# 待评审文件列表

scope: full_file

用户指定评审范围：整个 Novadraw 子项目的现行设计与实现，不限于 diff。
REPO_ROOT: /Users/bytedance/Documents/code/GitHub/drawjs
PROJECT_ROOT: engine/wasm_rust/novadraw
SKILL_ROOT: /Users/bytedance/.trae-cn/skills/bits-code-guard
WORK_DIR: engine/wasm_rust/novadraw/verification/evidence/goal-alignment-audit-2026-09-30
ARTIFACTS_DIR: 同 WORK_DIR
基线：811b748f0a510edb90ad198346950ca3752b404d 加审计开始时的工作区。
以下自动列表仅记录已有 diff；完整分组范围见 review_groups.md。
本次按用户要求纳入 Markdown、契约测试和未跟踪的 direct_edit_mode.rs。
不把抽样契约审查描述为逐行穷尽全仓。

diff_direction: base → source（`-` 行 = 旧代码/已删除，`+` 行 = 新代码/待评审）

总文件数: 20
排除文件数: 11（二进制/媒体文件 10 个, 生成/测试目录 1 个）

| 文件路径 | 变更行数 |
| -------- | -------- |
| engine/wasm_rust/novadraw/examples/native/node-editor-demo/src/harness.rs | +13, -1 |
| engine/wasm_rust/novadraw/examples/native/node-editor-demo/src/main.rs | +70, -36 |
| engine/wasm_rust/novadraw/examples/web/web-validation/Cargo.toml | +3, -1 |
| engine/wasm_rust/novadraw/examples/web/web-validation/src/lib.rs | +6, -0 |
| engine/wasm_rust/novadraw/examples/web/web-validation/web/index.html | +1, -1 |
| engine/wasm_rust/novadraw/novadraw-editor/src/direct_edit.rs | +13, -0 |
| engine/wasm_rust/novadraw/novadraw-editor/src/viewer/mod.rs | +226, -23 |
| engine/wasm_rust/novadraw/novadraw-platform-web/src/dom_text_input.rs | +118, -24 |
| engine/wasm_rust/novadraw/verification/suites.toml | +3, -0 |
