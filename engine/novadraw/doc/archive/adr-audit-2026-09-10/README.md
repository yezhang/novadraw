# 2026-09-10 ADR 修订前历史快照

类型：`archive`

效力：仅用于追溯，禁止作为当前设计、API 或实现完成状态的依据。

当前入口：[ADR 索引](../../adr/README.md)、
[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)、
[逐项审计与替代关系](../../verification/reviews/adr-audit-2026-09-10.md)。

## 保存方式

`original/` 保存修订前原始字节，包括尚未提交的 ADR-013 及 Figure 生命周期规范。
路径保留仓库目录结构，文件增加 `.txt` 后缀，例如：

- [原 ADR-013](original/doc/adr/adr-013-figure-lifecycle-and-runtime-identity.md.txt)
- [原 Figure 生命周期](original/doc/design/architecture/figure-lifecycle.md.txt)
- [原 ADR-001](original/doc/adr/adr-001-webgpu-rust-stack.md.txt)
- [原 ADR-011](original/doc/adr/adr-011-derived-state-convergence.md.txt)

[manifest.json](manifest.json) 列出全部快照的原路径、保存路径、字节数和 SHA-256。
原文中的“已通过”“当前”“complete”等措辞仅代表当时记录，不具有当前效力。
原文链接按原路径解释，不作为归档目录内可用导航。

保存全部 ADR 是为保留完整决策上下文，不表示全部决策被否定。001-012 的合理主线
仍保留；013 的通用活对象迁移承诺撤回，原路径替换为指引。

## 检索规则

默认架构检索排除 `doc/archive/`；只有用户明确要求历史或需要追溯决策时才读取。
`.md.txt` 避免普通 Markdown 文档收集把旧稿当作当前正文；仓库 `.rgignore` 同时
排除归档。其他检索工具必须遵循 AGENTS.md 的相同边界，不能假定它们识别 rgignore。

不得直接复制旧正文恢复为当前设计；恢复需要新 ADR、明确反例与重新验证。
