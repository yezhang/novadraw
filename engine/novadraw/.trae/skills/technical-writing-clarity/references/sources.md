# 依据与适用边界

本技能从[方案与评审文档表达审查](../../../../doc/verification/reviews/document-writing-audit-2026-10-29.md)提炼执行方法。审计中的文档样本、原句位置和扫描器复现是项目证据；技能中的操作流程是据此整理的写作建议，不是对所有技术文档的实证结论。

## 技术写作参考

| 来源 | 采用的方法 | 使用边界 |
|---|---|---|
| [Google：Paragraphs](https://developers.google.cn/tech-writing/one/paragraphs) | 建立主题、保持单一段落主题、解释是什么及为什么 | 不强制固定句数或每段同一种起句 |
| [Google：Clear sentences](https://developers.google.cn/tech-writing/one/clear-sentences) | 明确动作主体、使用具体动词 | 不删除所有“是”“有”“进行”等常用词 |
| [Google：Words](https://developers.google.cn/tech-writing/one/words) | 定义陌生术语、统一命名、明确指代 | 不把专业术语全部通俗化 |
| [Microsoft：Scannable content](https://learn.microsoft.com/en-us/style-guide/scannable-content/) | 重要信息前置、长文导航、可比较的一致结构 | 不让列表和表格代替完整推理 |
| [Gopen 与 Swan：The Science of Scientific Writing](https://www.americanscientist.org/blog/the-long-view/the-science-of-scientific-writing) | 按读者需要安排信息顺序、避免主谓被大量插入语隔开 | 不把英文位置偏好机械规定成中文句法 |
| [Nygard：Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) | 解释决策所回应的条件、选择与后果 | 不按固定页数拆分同一契约，不以 ADR 接受推断实施完成 |

中文表达与技术含义保护继续参考[好好说话](../../haohao-shuohua/SKILL.md)。两者交接后应再次核对条件、定义和结论范围，避免措辞清洗造成信息损失。

## 项目内的证据与文档分工

本项目遵循[文档知识维护](../../../../doc/knowledge-maintenance.md)：design 维护行为契约，ADR 维护取舍，roadmap 维护执行状态，review 保存某次快照的发现和结果。编辑建议应作用于正确的正文 owner；冻结评审与原始证据保持原文。

Core、Editor 与其他包的能力归属遵循仓库启动规则和完整项目规则。此约束属于 Novadraw 项目，迁移本技能时应改成目标项目的职责说明，不能照搬包名。

## 扫描器经验如何使用

原审计在 macOS 自带 `/usr/bin/grep` 上复现了 `-P` 不支持；`grep ... || echo "(clean)"` 会把工具错误与无匹配混为一谈。该证据只针对所测命令和环境，不证明所有历史扫描都失败。

本技能不附带扫描器，也不修改好好说话的原命令。需要机械扫描时，选用已确认可用的实现，保留工具错误、候选位置和人工豁免。字面扫描不能证明逻辑清楚或独立读者能够理解。
