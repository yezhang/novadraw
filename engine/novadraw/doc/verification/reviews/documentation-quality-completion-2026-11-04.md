# 文档质量修订记录

类型：`verification-record`

能力归属：文档治理及 xtask 文档检查；涉及 Core／Editor／backend／platform 的规范表达。
日期：2026-11-04。依据[后续事项](documentation-quality-follow-up-2026-11-04.md)及用户授权。
本次没有改变引擎设计的接受范围、生产执行顺序或既有验收结论。

## 确定改动

| 事项 | 结果与入口 |
|---|---|
| DQ01 | [Graphics](../../design/rendering/p2-g01-graphics-extension.md#5-stroke-与包围盒)与[AA](../../design/rendering/shape-antialias-proposal.md#hairline-与低-miter-单独决策)同步有限非负低 miter、原参数与 Bevel；Core 零宽、Cocoa 样例映射和 hairline 分别解释。取消默认产物体积比较，保留运行成本验收 |
| DQ02 | 草案评审不再用修订 1 冒充被审稿；保存可核对 ADR 草案、较早合同／矩阵及哈希，明确完整被审版本缺失。[基线记录](fr02-review-baseline-2026-11-04.md)给出来源与限制 |
| DQ03 | 长文本、GEF 接入与 DisplayList 按实际边界选择 design 或 ADR；[知识维护](../../knowledge-maintenance.md#冲突与替代关系)明确独立接受、局部替代与保留条款 |
| DQ04 | 查询、Flow、有向图、图投影区分规范返回类别与不可直接编译的协议示意；长文本明确局部接受。图投影旧缺口和逐项回应、Flow 官方入口交付转入[证据导航](flow-and-graph-design-evidence-2026-11-04.md)，当前集成规则保留 |
| DQ05 | Capability 提交顺序只引用[组件更新](../../design/architecture/component-update.md#3-执行顺序)；公共 API 的四组选择依据并入所属章节，历史批准统一追溯 |
| DQ06 | 总入口、归档入口与评审导航反映 52 → 10 的归并及本轮状态；已核对的现行规则改用专题链接，历史编号继续保留 |
| DQ07 | 为 51 份基础或跨层设计补充能力与状态 owner、依赖和消费者；其中一份是版本语义分析，不改为规范设计。99 份 design Markdown 的开头归属及可达性检查通过 |
| DQ08 | `cargo xtask docs` 接入[设计检查器](../../../tools/scripts/check_design_docs.py)，检查全域 design 归属、可达性、标题锚点，以及[登记规则](../../../tools/scripts/design_doc_rules.json)中的有限替代、效力、示例与基线关系 |

DQ07 的补充以正文职责为依据，不从目录名称直接推断 owner。
本次只定向读取文档工具，没有扫描引擎 Rust 来重新定义合同。
普通设计仍可独立接受；本次未创建 ADR。

## 原文与验证

修订前文档与工具保存在 `target/verification/document-quality-2026-11-04/before.tar.gz`，
本地 `before-sha256.json` 记录原路径及哈希。需要长期回看的 Flow／图投影原文、
FR-02 稿件及修正前评审另保存在
[证据 manifest](../evidence/document-quality-2026-11-04/manifest.json)。
已有原始 archive、evidence 与字体报告的字节保持；归档导航页按 DQ06 更新。

验证包括 8 个 Python 反例测试和 6 个 xtask 测试，覆盖坏锚点、孤立页、归属缺失、
示意代码分类、旧条款回流、基线缺失声明及证据字节变化。
测试还验证冻结历史锚点的豁免和路径检查，未编译设计中的 Rust 示意片段。
最终文档门禁、差异及哈希检查结果在下文记录。

`cargo xtask docs` 通过 477 份 Markdown，design 子检查覆盖 99 份。
`git diff --check`、xtask 的定向 rustfmt 检查通过。
1059 份已有原始 archive／evidence／字体文件的 SHA-256 无变化；
更新归档导航不计入原始字节保护范围。新增评审链接与锚点、被删章节的入站链接
未发现新断链。

## 中文与事实复核

先核对规范状态、版本、数字及错误边界，再检查中文，最后回读长句。
低 miter 的含义变化有既有接受依据；其他修订保留 owner、方向、失败与验收范围。
新增信息由实际快照、工具结果或项目规则支撑。
原来的拟定接口、实施流水与当前合同混在一起；现在用正文、示意标记和证据入口
分别承担阅读任务，读者可以先理解当前行为，再追溯某次交付。

本轮主线是让现行合同和历史证明各有明确入口。改后更清楚；
未把全文技术审计、生产实施或缺失稿恢复算作完成结果。
机械扫描只检查本轮新增及改写文字，原编号标题、历史引文、路径与代码按其用途保留。
扫描的编号标题、空泛元话语、中文半角标点与破折号均为 0 命中；
发现 1 处对比句式候选，已改为直接说明候选尚未确认为缺陷。

## 尚未证明的范围

FR-02 的完整被审合同和矩阵仍缺失；早期字节与修订 1 均不代替它们。
工具列出 50 处迁出编号段落候选供人工复核，其中可能包含历史说明；
这些候选尚未逐项确认为缺陷；design 全域技术语义也未完成逐篇审计。
有限规则只能发现已登记的误用，不能自动裁定新旧合同冲突或被审版本对应。
平台性能、GPU 像素、引擎运行行为与设计接受状态不由本轮验证授予。
