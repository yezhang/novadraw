# P2-E02a Direct Text Edit Session 完成记录

类型：`verification`

日期：2026-09-29

结论：`complete`

## 1. 完成范围

- `DirectTextFeature`、descriptor、request 与 `PolicyRole::DirectTextEdit`；
- Viewer-scoped 单一 session identity 与固定 source/feature/source revision；
- Editor-owned draft、directed text selection、preedit range 与 composition base；
- Policy-owned session plan、TextFlow feedback projection 与最终模型 Command；
- Core P2-T02 驱动的 point、caret、selection、visual cluster 和 word 查询；
- accept、cancel、无变化收口、recoverable rejection 与 stale revision；
- source retire、Viewer fault、Tool/history 仲裁和 feedback cleanup；
- accept 后单条 CommandStack history，以及 undo/redo 模型投影。

## 2. 自动验证

Suite：`editor.p2-e02-direct-text-edit`

结果：PASS，覆盖：

- 草稿不修改应用模型与历史；
- accept 前移除 feedback，accept 后只产生一条 Command；
- Unicode grapheme movement/delete；
- 单 Viewer 单 session 与 unsupported feature；
- cancel 和无变化 accept 不产生 history；
- stale source revision 与 recoverable command rejection 保留 draft；
- preedit update、无 caret preedit、非法 UTF-8 range 与 composition cancel；
- source retire 自动清理 session 和 feedback；
- Viewer selection 与 text selection 独立；
- surface-domain caret/selection geometry。

回归结果：

- `cargo test -p novadraw-editor`：PASS；
- `cargo clippy -p novadraw-editor --tests -- -D warnings`：PASS；
- `cargo xtask verify editor.p2-e02-direct-text-edit`：PASS。

## 3. 后续边界

P2-E02a 不绑定 Winit 或 DOM。P2-E02b 将增加带 session identity 的 host input lease、
平台无关 event/effect、Native Winit IME 和 Web DOM input/composition bridge。
