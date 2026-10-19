# 文档知识修正记录

类型：`verification`

日期：2026-10-19

基线：`44373f248644ef6b57e1d445e253cfef1d7b7d67`，开始时工作区干净。
范围为 `doc/` 的知识表达、文档引用和必要的启动入口同步，不修改引擎行为。

## 扫描范围与判据

盘点全部 274 篇 Markdown、59,036 行，包含 archive。
扫描正文、标题、类型、重复段落、引用关系和替代措辞；按设计、参考、ADR、
parity、路线图、策略、迁移及验证分类复核。以现行 ADR、规范与完成证据判断
重复或失效，按疑点定向核对官方源码及现行公开路径。

这次扫描不等于重新验证全部市场调查、历史运行结果或每个源码分析结论。
策略报告按其证据日期保留；历史审计、性能和验收结论保持基线，不推定为当前状态。

## 处置与承接

| 问题 | 处置 | 当前入口 |
|---|---|---|
| 总入口复制状态、ADR 摘要及专题全表 | 重写为任务阅读路线，规则独立维护 | [总入口](../../00-index.md)、[知识维护](../../knowledge-maintenance.md) |
| 公共 API 综合提案已被取代却仍被规范引用 | 删除当前全文；提炼 owned/scoped，历史 review 指向原稿 | [公共 API 契约](../../design/architecture/public-api-contract.md) |
| 09-15 迁移提案保留旧 enum/G6 方案 | 删除当前提案，方法论归入迁移指南 | [语义迁移](../../migration/java-rust/migration-guide.md) |
| 已完成 Editor 实施计划与 roadmap 重复 | 删除计划，roadmap 收敛为状态、门禁与证据；拆分判据归目录规范 | [Editor 路线图](../../roadmap/editor/00-index.md) |
| ADR-021 正文继续推荐旧聚合/prelude/feature | 改为决策身份及 ADR-023/031/035 替代指引 | [ADR-021](../../adr/adr-021-public-facade-and-feature-boundary.md) |
| DisplayList 协议与实验计划重复 | 探索与晋级门禁归一页，删除独立计划 | [休眠候选](../../design/rendering/display-list-protocol.md) |
| Figure 概览、bounds 与管线多份展开同一协议 | 概览改为概念地图；bounds 专注存储更新，转换与命中链接专题；管线去除重复大图 | [Draw2D 参考](../../reference/draw2d/00-index.md) |
| Rust 教程混淆继承、转型和覆盖 | 流程与语法分工；纠正 overlapping impl、Any、泛型通配符及对象分配表达 | [迁移入口](../../migration/00-index.md) |
| 外部参考混入旧 Novadraw owner/类型/通知方案 | GEF、Zed、SWT 引用规范或 parity，移除过时映射 | [参考入口](../../reference/00-index.md) |
| P2 backlog 保存草案到完成的流水及旧 concrete facade 例外 | 用完成报告承载时间线；保留关闭范围和 S06-PL02/G01 限制 | [P2 backlog](../../roadmap/p2-delta-backlog.md) |
| 能力规划重复定义动画公式、模式及逐条验收 | 规划只维护分组与建议；补齐领域集成中的 Reveal/Activity、trail/handoff 和 fallback | [能力规划](../../roadmap/draw2d-capability-enrichment-plan-2026-10-05.md)、[动画集成](../../design/animation/capability-integration.md) |
| 已接受专题仍标 target/提案；动画 API 留旧入口 | 修正规范效力、标题及已接受的子 facade 路径 | [Editor](../../design/editor/00-index.md)、[动画](../../design/animation/00-index.md) |
| 入口类型错误、专题入口缺失、review 漏收 | 统一入口类型，补渲染/动画/性能索引，补全审计导航 | [设计](../../design/00-index.md)、[验证](../00-index.md) |

保留坐标、输入、组件、Capability、收敛、布局表现、文字、资源等独立契约；
它们回答不同 owner/阶段的问题，不能因共享术语就合并。
M1–M10、G0–G5、长期能力分母和未完成平台门禁保持原有含义。

## 语义修正依据

官方 `Figure.paint/paintClientArea/paintChildren` 与 `SWTGraphics.restoreState` 证明：

- restore 恢复完整保存状态且不弹栈；
- child/clip 区域绘制后必须恢复状态；
- client area clip 受 local-coordinate、Border 与 optimizeClip 分支影响；
- 默认模板可以覆盖，多矩形 clip 不等于任意路径 clip。

`Figure.findMouseEventTargetInDescendantsAt` 使用转换后的点、visible/enabled 资格，
无 descendant 返回 null；当前对象资格由上层方法判断。
`setBounds` 同时移动与改尺寸时只发一次 FigureMoved。

现行 ADR-033/035 及公开 facade 核对用于更新调用形式，不把代码现状提升为新设计。
Rust 教程删除重复且无效的 pseudo implementation，保留默认方法与 blanket impl
的区别、所有权转交及真实扩展要求。

## 原始证据

既有 `verification/evidence/`、此前 archive 原稿和 SHA-256 manifest 保持不变。
退出当前使用的方案及归并前参考/教程保存逐字快照，见
[原稿与替代映射](../../archive/knowledge-revision-2026-10-19/README.md)。
历史 review 仅修复必要链接；原判断、数字及通过/失败结果不重写。

51 份修订前文档保存逐字原稿；原路径、字节数和 SHA-256 全部通过校验。

## 验证

- `cargo xtask docs`：PASS，覆盖 doc/book 共 331 篇 Markdown 及规定入口的命令引用；
- `git diff --check`：PASS；
- `doc/` 全部 276 篇 Markdown 从总入口可达，本地缺失路径 0；
- 当前文档的标题锚点检查：0 个无效锚点。历史报告的 `#L…` 为原基线行号引用，
  不作为标题锚点，也不据新稿行号重写历史证据；
- 264 个既有证据及归档文件核对：只更新归档导航，原始证据异常 0；
- 51 份新原稿的字节数和 SHA-256：PASS；
- 中文回读及机械扫描完成：修订正文中中文字符间半角标点命中 0，
  代码、命令和 URL 按原语法保留。

本批仅修改文档与启动入口，不重新运行 GPU、像素或 workspace 行为测试。
文档与证据链接修正不代表未完成平台项已验收。
