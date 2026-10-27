# 文档知识维护

类型：`documentation-index`

本页维护文档的归属、效力和修正规则。项目开发规则以
[CLAUDE.md](../CLAUDE.md)为准，文档导航见 [总入口](00-index.md)。

## 一个问题由一处正文负责

概览负责边界和阅读路线；专题负责完整行为；ADR 负责为什么选择；parity 负责采用
外部语义；roadmap 负责执行状态；review 负责某次快照的发现和结果。
其他文件用链接引用，不复制完整算法、签名表、状态表或实施日志。

目录按知识来源和规范效力划分，主题作为第二层。小文件若有独立 owner、输入/输出、
错误和扩展边界，应保留；只有表述差异或重复展开才合并。大文件先删除重复，再按
独立问题拆分，避免仅凭行数制造新碎片。

## 冲突与替代关系

1. 已接受 ADR 的明确替代关系决定关键取舍；历史批准可以被有证据的修订取代。
2. `normative-design` 中范围更窄的专题优先于总览；`proposal` 不具有覆盖效力。
3. reference 服从源码事实；parity 解释映射，二者不覆盖 Novadraw 设计。
4. roadmap、review、迁移方法和 archive 不定义运行时契约。

设计与实现不一致时，先判断是实现缺陷、新约束需要设计修订，还是尚未完成。
不得把文档直接改成代码现状。只有核对公开路径或证明已有契约时才定向查实现，
理想架构设计仍遵守禁止先扫项目 Rust 的规则。

## 当前表达与原始证据

可服务的知识表达可以删除、归并或重写。原始日志、JSON、探针、基线快照和校验
manifest 保持只增不改；review 的原判断和完成事实保留在其原基线中。
旧稿全文不应继续处于当前设计阅读路径。

删除已取代方案时，保存原始字节、路径和 SHA-256，并记录新入口。
历史报告引用旧方案时可指向对应快照；其正文判断不改成新结论。
ADR 身份仍被外部引用时，原位置可保留简短替代指引。
默认架构检索排除 archive，历史追溯时显式进入。

提交的原始证据统一放在 `doc/verification/evidence/`；独立字体比较报告及其原始
GPU demos 放在 `doc/reference/fonts/reports/`。这两类快照允许整体迁移，内容仍保持
只增不改；其内部历史路径保留，不按当前文档改写。文档门禁检查当前报告入口，
不把冻结证据和原始 demos 当作现行规范文档。迁移后核对文件 SHA-256。

源码和工具的目录归属见 [根目录说明](../README.md)；可执行测试入口见
[tests/README.md](../tests/README.md)。本地生成物统一放在 `target/`。

## 文档类型

标题后写 `类型：` 标记。入口页使用 `documentation-index`；正文按内容选用：

| 用途 | 类型 |
|---|---|
| 当前契约、关键决策、候选 | `normative-design`、`architecture-decision`、`proposal` |
| 源码事实、采用关系 | `reference-analysis`、`parity-contract` |
| 执行状态、产品输入、迁移方法 | `roadmap`、`product-strategy`、`migration-guide` |
| 验证方法与报告 | `verification`、`verification-plan`、`verification-record`、`manual-verification`、`implementation-verification`、`architecture-review` |
| 失效表达与历史入口 | `archive` |

规范接受与实现状态分别表达，`accepted` 不等于 `implemented/complete`。
参考分析给出仓库、提交和稳定 class/method；规范给出范围、失败、扩展点和验证入口。

## 链接与门禁

文件使用小写 kebab-case，固定入口为 `00-index.md`。
移动或删除后检查 doc、book、启动文件及其他仓库引用；更新正文路径和锚点，
不只检查链接目标是否存在。每篇当前文档都应能从所属入口到达。

纯文档批次运行 `cargo xtask docs` 和 `git diff --check`。
文档门禁检查类型、本地路径、设计索引、parity 状态和规定入口中的命令，
不证明所有锚点或正文语义正确；这两项还需在本次修改范围内复核。
