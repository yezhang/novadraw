# wasm_rust 遗留交付文件迁移

类型：`verification-record`

2026-10-28，按用户要求，将旧 `engine/wasm_rust/novadraw/` 下的 5 篇交付文档与
3 张截图迁入[历史证据目录](../evidence/wasm-rust-delivery-2026-10-28/)。
逐文件核对迁移前后的 SHA-256，8 个文件均一致；
校验值保存在 [SHA256SUMS](../evidence/wasm-rust-delivery-2026-10-28/SHA256SUMS)。

## 归属与阅读入口

三篇长文本文档明确标注为交付副本，与项目正文的差异为副本说明、校验值和绝对链接。
两篇 GEF 文档是阶段交付摘要，完整记录已在项目内维护。
因此原始文件整体保存为历史快照，当前阅读仍使用下表中的项目正文。

下表文件名均相对于旧 `engine/wasm_rust/novadraw/`；
迁移后相对于上述历史证据目录，文件名与内部目录结构保持不变。

| 原文件 | 迁移后的快照 | 项目正文 |
|---|---|---|
| `gef-first-slice-implementation-2026-10-28.md` | [GEF 首批摘要](../evidence/wasm-rust-delivery-2026-10-28/gef-first-slice-implementation-2026-10-28.md) | [Scene 输出实施](gef-scene-target-implementation-2026-10-28.md) |
| `gef-native-first-frame-followup-2026-10-28.md` | [GEF 首帧续验摘要](../evidence/wasm-rust-delivery-2026-10-28/gef-native-first-frame-followup-2026-10-28.md) | [Masonry Native 首帧与 resize 续验](masonry-native-first-frame-followup-2026-10-28.md) |
| `long-text-scroll-resize-implementation-2026-10-28.md` | [长文本实施交付副本](../evidence/wasm-rust-delivery-2026-10-28/long-text-scroll-resize-implementation-2026-10-28.md) | [长文本实施记录](long-text-scroll-resize-implementation-2026-10-28.md) |
| `long-text-scroll-resize-proposal-revision-2-2026-10-28.md` | [修订 2 交付副本](../evidence/wasm-rust-delivery-2026-10-28/long-text-scroll-resize-proposal-revision-2-2026-10-28.md) | [长文本优化提案](../../design/architecture/long-text-scroll-resize-optimization-proposal.md) |
| `long-text-scroll-resize-review-2026-10-28.md` | [评审交付副本](../evidence/wasm-rust-delivery-2026-10-28/long-text-scroll-resize-review-2026-10-28.md) | [固定评审记录](long-text-scroll-resize-review-2026-10-28.md) |

三张截图保留在快照内的 `gef-native-first-frame-assets/`：
[终端初始白屏](../evidence/wasm-rust-delivery-2026-10-28/gef-native-first-frame-assets/native-exposure-final.png)、
[LaunchServices 初始白屏](../evidence/wasm-rust-delivery-2026-10-28/gef-native-first-frame-assets/native-launchservices.png)、
[用户确认后的画面](../evidence/wasm-rust-delivery-2026-10-28/gef-native-first-frame-assets/native-visible-confirmed.png)。
它们与[既有 Masonry 证据](../evidence/masonry-xilem-2026-10-28/)中的同名文件逐字节一致。
保留这一组副本，使交付摘要的相对截图链接继续有效。

按[知识维护规则](../../knowledge-maintenance.md)，快照正文及其历史路径不作改写，
也不覆盖现有方案、评审或实施记录。Finder 元数据 `.DS_Store` 移入忽略的
`target/migrations/wasm-rust-2026-10-28/`；清空后移除了旧 `engine/wasm_rust/` 目录。
