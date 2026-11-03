# 文档内容一致性与质量分析

类型：`architecture-review`

能力归属：文档治理；涉及 Core 坐标、资源、Flow、Graphics，以及跨包执行状态。
本报告评价知识表达，不裁定实现缺陷，也不修改已接受契约或执行授权。

日期：2026-11-03。基线：文档分类提交 `f30a31ef` 后的工作区。

## 范围与判断方法

检查 `doc/` 的当前入口、ADR、设计、路线图和验证清单，重点复核上轮分类后的
正文一致性。排除 archive、冻结 evidence 和独立字体原始报告的内部内容；
这些目录之外共有 428 篇 Markdown，其中仍包含独立历史评审，不能全部视为现行规范。
本次做了全域文字定位和重点主题上下文核对，未逐篇完成全部技术契约审计。

判断依据为[知识维护规则](../../knowledge-maintenance.md#冲突与替代关系)：
明确替代的决策和范围更窄的规范决定现行契约；proposal 不覆盖已接受规范；
roadmap 维护执行状态；历史报告只证明其指定基线。
公开 API 仅针对字体注册做了定向源码核验，没有据实现现状重新定义架构。

问题分为三类：当前表述与权威契约不一致、已有明确裁决但旧条款未同步、
解释和归属质量问题。优先级表示误用文档的风险，不表示代码缺陷等级。

## 已确认的当前表述冲突

### Q01：坐标总览扩大了可逆保证，静态结构写反了映射方向

优先级：P1。归属：Core 坐标文档。

[架构总览](../../design/architecture/overview.md#2-从-draw2d-继承什么)第 33 行
把父子坐标转换概括为“每条树边共享同一可逆 2D 变换”。
但[坐标专题](../../design/coordinates/coordinate-system.md#5-world-transform)
第 150–158 行明确允许有限输入数学奇异，逆查询返回 `Ok(None)`；
[查询协议](../../design/architecture/query-identity-and-failure.md#5-坐标查询)
也区分合法奇异、身份错误和几何失败。“共享同一变换”成立，无条件“可逆”不成立。

[静态结构](../../design/architecture/static-architecture.md#31-nodestate)第 138 行
又把 `child_transform` 写成“content domain 到 children domain”的变换。
[坐标公式](../../design/coordinates/coordinate-system.md#3-边变换)第 70–71 行规定
`child_content_to_node_local = Translate(insets) * child_transform`，方向相反。
该偏差可能让扩展作者错误应用 scale、scroll 或逆矩阵。

建议按坐标专题修正总览和字段说明：共享正向边变换，逆向查询受检；
明确 `child_transform` 从 child-content 映射到 node-local，并保留 insets 的组合顺序。
这属于表达修正，不需要重新选择坐标架构。

### Q02：整改清单的当前队列和下一片已经过时

优先级：P1。归属：文档治理／跨包状态。

[执行总清单](../checklists/audit-remediation-execution.md#2-当前执行队列)
第 48、55 行仍写“S09-A 已完成，下一片为 Observations／Inspector／S09-B”，
并在第 50 行要求按该表推进。
[唯一执行队列](../../roadmap/p2-delta-backlog.md#当前推进顺序)第 22–25 行则规定：
长文本优先设计 FR-02；S09 A／B／C 完成，D 定向通过，下一步补成本及最终门禁。
[S09-D 记录](p2-s09-d-implementation-2026-11-02.md)支持后者。

清单第 34 行还限制最多一个 `in_progress`，而
[工作流](../audit-remediation-workflow.md#43-backlog--原子批次队列)
第 105–110 行已明确：限制的是正在 Execute 的批次数；交付状态
`in_progress` 可以同时表示多项仍待证据。这两种限制不能混用。

建议清单只保留来源映射和未关闭验收要求，当前顺序直接链接 backlog；
单链限制统一表达为正在 Execute 的批次数不超过 1。不要通过改状态数掩盖未验收项。

### Q03：文字专题仍推荐已退出公开面的字体注册方法

优先级：P1。归属：Core 资源／文字文档。

[文字专题](../../design/architecture/text-layout.md#4-字体描述)第 122–125 行
要求应用使用 `Runtime::register_builtin_font`。
[Facade 协议](../../design/architecture/runtime-subsystem-facade.md#1-runtime-只保留组合根职责)
第 37–42 行限定 Runtime 本体职责，第 62、96 行把资源操作交给 `RuntimeResources`。

定向源码核验确认：`crates/novadraw/src/runtime/runtime/frame_resource.rs`
第 67 行的方法是 `pub(crate)`；`service_facades.rs` 第 206 行才是公开入口。
现行调用应为 `runtime.resources().register_builtin_font(...)`。
照抄旧示例会遇到公开访问错误。

建议更新该示例并引用资源 facade，不恢复 Runtime 顶层别名。
本条只确认调用路径错误；“显式注册字体”的行为要求保持。

## 已有裁决，但旧条款未同步

### Q04：低 miter 的旧准入条件仍留在现行 Graphics 专题

优先级：P2。归属：Core Graphics／后端。

[Graphics 扩展](../../design/rendering/p2-g01-graphics-extension.md#5-stroke-与包围盒)
第 125–126 行仍要求 miter limit 至少为 1。
[低 miter 专题](../../design/rendering/low-miter-and-cocoa-sample-profile.md#低-miter-契约)
已接受有限非负值，并在小于 1 时采用 Bevel；正文明确替代旧限制。

因此选择已经明确，不能把它判为待决架构问题。问题是读者从基础专题进入时
仍会看到错误准入条件。建议在旧条款位置写出现行规则并链接专题，
同时保留 Core 零宽不绘制与样例 Cocoa profile 的归属差异。

### Q05：部分验收正文仍把产物体积比较列为默认要求

优先级：P2。归属：文档治理／验证成本。

[ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md#验收条件)
第 168 行要求“二进制体积门禁继续通过”；
[抗锯齿方案](../../design/rendering/shape-antialias-proposal.md#性能与包体积退出条件)
第 134–137 行要求实施后必须比较 Native／Wasm 和压缩产物。
它们与[项目现行规则](../../../CLAUDE.md#包体积与性能要求)的默认执行要求不同。

项目规则明确覆盖既有 ADR、计划及验收清单，因此不是授权不明。
建议旧条款改为仅在用户明确请求比较产物体积时执行；
布局、渲染延迟、内存和依赖边界评估仍保留。不要删除独立历史体积测量结果。

## 正文质量与分类问题

### Q06：设计接受、实施事实和拟议写法仍混在同一篇正文中

优先级：P2。归属：Core Query／Flow／图布局文档。

[查询协议](../../design/architecture/query-identity-and-failure.md#状态)
第 10 行说明已实现并完成行为验收，但第 193–195 行仍以
“不代表迁移已完成”“建议返回值”引入签名表。
这句话在逻辑上不直接否定实现完成，却让读者难以判断表中的名字是否已冻结。

[结构化 Flow](../../design/architecture/structured-flow-layout.md#领域模型)
第 43 行仍写“拟定的公开协议骨架”“名称在实现阶段可调整”，
后半篇又从第 128 行起展开“已提供／已增加”的实施叙述。
[图布局](../../design/architecture/directed-compound-graph-layout.md#图模型算法与投影协议)
第 59、111、231–233 行保留拟定字段、拟定协议和待验证缺口；
[投影准备](../../design/architecture/graph-projection-preparation.md)
开头则详细列出已实现的解决机制。

建议同一设计只维护现行行为与必要限制。签名若为示意，明确不用于精确调用，
链接公开 API；独立实施结果和测试分母放到 verification。
投影准备已标明“下表保留实施前基线”，这避免了直接状态矛盾，
但该基线仍可退出日常规范阅读路径。

### Q07：ADR 与 design 的正文职责尚未完全分离

优先级：P2。归属：文档治理。

[ADR-035](../../adr/adr-035-explicit-domain-imports-and-root-boundary.md)
第 47–62、71–80、184–207 行仍保留完整模块清单、路径表、门禁和迁移步骤。
[ADR-027](../../adr/adr-027-open-figure-capability-registry.md)
第 103–116、139–157 行展开 presentation channel 仲裁、失败分类和测试清单；
[能力模型](../../design/architecture/figure-capability-model.md#9-mutation-协议)
与[组件更新](../../design/architecture/component-update.md#3-执行顺序)
又维护相同提交及失败机制。

必要的决策摘要应保留，例如“不提供库级 prelude”“私有组件和跨类型能力分离”。
完整表和执行步骤则容易成为第二处维护点，违背
[一处正文负责一个问题](../../knowledge-maintenance.md#一个问题由一处正文负责)。
本轮分类完成了目录调整，但尚未完成全部正文去重。
建议按行为 owner 合并后用链接承接，不凭行数压缩，也不删掉替代方案和决策代价。

### Q08：部分开头未按项目规则明确能力 owner

优先级：P3。归属：文档治理。

[静态结构](../../design/architecture/static-architecture.md)开头讲“核心类型”，
[文字专题](../../design/architecture/text-layout.md)只写范围编号，
[目录设计](../../design/architecture/directory-structure.md)只写 Rust 模块与 crate。
它们没有在开头明确列出 Core、backend、platform 等能力 owner。
正文能够推断部分归属，但不满足[项目要求](../../../CLAUDE.md#规划与方案必须明确能力归属)；
跨层读者还需自行判断设计约束由谁执行。

建议补明确 owner，并说明其他层是依赖还是实际改动归属。
自动扫描的关键词缺失只作为定位线索，不把所有命中都计为确认问题。

## 整体评价与修正顺序

导航和类型标记能帮助找到材料；主要质量不足是多处维护同一事实，
新裁决未回到旧条款，以及设计正文仍带实施阶段叙述。
目前适合按专题寻找规范，但依赖总览或执行清单作单一判断时仍可能误用。
没有证据支持“全部文档相互一致”或统一质量评分。

优先修正 Q01–Q03 的坐标、执行状态和公开调用路径，
再同步 Q04–Q05 已裁决的旧条款，最后按 Q06–Q08 合并正文并补足 owner。
历史报告保留原判断，新的规范入口说明承接关系；不要将旧测试结果改写为最新验收。

## 检查结果与限制

分析前 `cargo xtask docs` 通过 503 篇 Markdown；加入本报告与导航后通过 504 篇。
`git diff --check` 通过。门禁分母包括工具纳入的历史入口，和上述 428 篇分析范围不同。
机械通过不证明正文语义一致。

额外按标题 slug 近似规则检查了 214 个本地 Markdown 片段引用，
得到 11 个候选，全部来自 `goal-design-code-audit-2026-09-30.md` 的 `#L…` 行号引用。
它们不是标题 slug；渲染器对行号片段的支持不同，不能直接宣布 11 个链接失效。
本轮没有发现其他标题片段候选，但这不等于所有渲染器的锚点验证通过。

本报告是基于仓库材料的自审，没有独立读者复述验证，也未重跑各能力实现测试。
源规范和执行状态保持原样；本报告中的修正顺序是建议，未建立新的 Execute 队列。
