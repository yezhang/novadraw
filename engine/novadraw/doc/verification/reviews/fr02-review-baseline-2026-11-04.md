# FR-02 评审版本核对

类型：`verification-record`

能力归属：文档证据治理；不改变 Core Flow 方案的接受或实施状态。
日期：2026-11-04。

草案评审此前链接到修订 1，导致“尚未单列共享文字内容”的原判断与目标正文冲突。
本次修正导航，原评审要求、判断和完成事实保持。

## 保存的版本与证明范围

| 文件 | 来源与核对结果 | 可证明的范围 |
|---|---|---|
| ADR 草案 | 本地 fine/coarse `untracked.tar.gz` 中字节相同；[45 行原文](../evidence/document-quality-2026-11-04/fr02-early-decision.md.txt)的第 19–25、43 行与评审引文相符 | 可核对草案的关键判断；无法证明整个评审文件集合同时冻结 |
| 早期合同 | 两个归因快照字节相同；[151 行原文](../evidence/document-quality-2026-11-04/fr02-early-contract.md.txt)保留拒绝候选及私有计算输入两种表述 | 早期问题背景；不是评审引用至第 196 行的完整版本 |
| 早期矩阵 | fine 为[105 行版本](../evidence/document-quality-2026-11-04/fr02-early-matrix.md.txt)，coarse 为 40 行版本；均不含评审提到的 FA-R23 | 更早矩阵的演进；完整被审矩阵缺失 |
| 决策修订 1 | [归并前快照](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-060-flow-assembly-reuse.md.txt)第 8 行明示修订 1 | 修订后的决策文本；不能代替草案 |
| 导航修正前评审 | [原文](../evidence/document-quality-2026-11-04/fr02-review-before.md.txt) | 保留本次改写前的链接、判断与历史校验声明 |

早期稿来自 `target/verification/flow-assembly-2026-11-03/fine/untracked.tar.gz`；
本次已将相关字节另存到 evidence，读者不依赖本地 tar 存活。
原路径与 SHA-256 见[manifest](../evidence/document-quality-2026-11-04/manifest.json)。
`09f49e47` 是这些主题首次进入当前 Git 历史的提交，没有更早被审版本。
归并前 tar 中 ADR 也已是修订 1。本次没有找到完整合同与矩阵基线，未拼造旧稿。

## 抽查与维护规则

本次另核对[查询修订 1](adr-038-revision-1-review-2026-10-16.md)与
[图投影增量评审](graph-projection-preparation-review-2026-10-22.md)的阅读关系：
原判断保留，当前规范另有状态和完成入口。抽查只核对导航角色，
没有证明这些评审的全套被审文件均可复原。

历史评审须区分被审快照、后续修订和当前规范。缺失稿注明缺失；
哈希证明特定字节，不能单独证明版本对应。新评审保存基线文件集合和来源，
不以可变设计链接代替冻结版本。
