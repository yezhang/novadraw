# 图投影 preparation 增量评审

类型：`architecture-review`

## 结论与基线

**建议修改后复审，暂不通过这个增量的实施门禁。**

Runtime 持有候选快照、显式约束测量和必需路由发布条件的方向可以保留。
需要补齐两处 P1 契约缺口，并处理或回应两项 P2 建议。

本文件保留 2026-10-22 会话中的审计结论，供另一个 agent 修改
[图投影的候选快照查询提案](../../design/architecture/graph-projection-preparation.md)。
该增量属于 [ADR-043](../../adr/adr-043-directed-compound-graph-layout.md) 的
Figure/Connection adapter 切片。原 ADR-043 修订 1 的方案通过意见不因本报告改变；
此次拒绝针对新增公开契约。

- 审核对象：状态为 `proposal`、待评审的 173 行提案。
- 仓库提交：`e6100c0830b9dc856e402247d46ebb545e70b4b4`。
- 提案文件 SHA-256：`8d367283f9d800968cc3ebd7768571f1761963f0ac3671b8f43d6fb127a50724`。

以下行号指上述文件基线，修改后应按章节和原句定位。
本次审查采用方案核对及定向静态源码核对，没有通过运行测试复现以下风险。
文档门禁与空白检查通过，不证明新增 API 或运行行为正确。
本报告保存历史判断，不替代当前设计契约。

## 接手范围

接手 agent 按 `AGENTS.md`、`CLAUDE.md` 读取项目规则后，
先修订提案及必要的文档引用，再提交复审。
理想架构修订遵守禁止先扫描本项目 Rust 实现的规则；
核对现有契约时仅定向查阅相关实现。

本次用户授权记录审计结论，记录不表示增量已接受或已授权实施。
修改后逐项给出处理结果、新章节引用和理由；
若不采纳建议，提供契约或源码依据。保留本报告原判断，复审结论另行记录。

## GP01：identity 更新可能反复触发路由

严重级：`P1`。位置：“Runtime 持有候选快照”，45–47 行、68–70 行。

提案规定每次受检 preparation 分配新的 `PreparedSceneIdentity`，
又用该身份作为 `DependencySubject::PreparedScene(owner)` 的 generation。
后代布局或 Locator 更新可能再次触发 preparation；
即使图结果不变，新身份仍可能使整组重新路由。

建议区分 preparation 尝试与快照实际替换。明确相同 source、约束和领域结果
重复准备时如何保留 identity；新请求或路径改变时才更新。
不能仅用“发生了一次 preparation”判断领域结果已改变。

依据：

- 提案保留现有 `PreparationScene.revision` 的用途，同时要求稳定帧不重算。
- `novadraw/src/figure/preparation/scene.rs` 的
  `PreparationScene::revision` 包含子树 bounds 与 layout generation。
- [派生状态收敛契约](../../design/architecture/derived-state-convergence.md)
  要求 NoChange 不制造新派生工作，正常支持链路应沿静态依赖方向完成。

复审条件：规定 identity 的保留与更新条件，补充
“路由提交引起后代更新，但图结果不变”的收敛测试要求；
同时保留“bounds 不变但请求或路径改变”时必须重路由的验收。

## GP02：新增快照依赖缺少结构环检测规则

严重级：`P1`。位置：“preparation 声明必需路由”，126–139 行；
“Runtime 持有候选快照”，77–80 行。

提案引入 preparation 必需路由，同时 Anchor/Router 读取该 preparation 快照。
如果 preparation 的测量又读取连接或 Locator 的派生几何，就会形成循环。
当前只规定共享预算，未说明新增依赖如何参与既有 `DependencyCycle` 检测。

建议明确 `require_route_group` 是发布屏障，单独记录快照计算依赖，
禁止快照反向依赖消费它的 route/Locator 输出。
增加直接环、跨 preparation 环的拒绝测试要求，不能仅等到预算耗尽。

依据：

- [连接契约](../../design/architecture/connection-routing.md)“失效与事务边界”
  禁止 Anchor 或 Locator 依赖会反向依赖 route 的 geometry，
  要求检测 dependency graph 并返回 `DependencyCycle`。
- [派生状态收敛契约](../../design/architecture/derived-state-convergence.md)
  “Stable epoch、预算与错误”要求已知结构环提前拒绝，预算用于捕获其他异常。

复审条件：区分快照计算依赖与发布完成条件，说明新增 PreparedScene subject
如何参加环检测；定义直接环和跨 preparation 环的结构化拒绝及恢复验收。

## GP03：写清测量分派优先级

严重级：`P2`。位置：“候选测量使用显式约束”，87–111 行。

提案尚未明确同时具有 override、LayoutManager 和 preparation 的 Figure
选择哪个分支。若只按是否具有 preparation capability 分派，
可能绕过现有权威测量入口或不必要地返回“不支持”。

建议保留现有权威测量顺序，只替换实际选中的 prepared 测量分支；
同时明确 Border/insets 只组合一次。

依据：

- `novadraw/src/graph/layout_measurement.rs` 的
  `FigureTree::preferred_measurement` 和 `minimum_size`
  先处理对应显式 override，再处理 LayoutManager，随后处理 prepared/intrinsic。
- [ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)
  保留权威测量的既有组合顺序。
- [深层布局与 Presentation 收敛契约](../../design/architecture/deep-layout-presentation-convergence.md)
  区分 content 与 outer measurement，显式 override 不重复增加 insets。

复审检查：明确分派顺序，并覆盖同一 Figure 同时具有多个测量来源、
非对称 Border/insets 和容器投影的验证要求。

## GP04：具体列出结构化测量错误

严重级：`P2`。位置：`measure_scene` 协议及错误说明，90–103 行。

新增接口返回 `TextError`，现有类型不足以直接表达测量主体、约束、
不支持和派生溢出。提案虽要求结构化失败通道，但尚未说明错误如何承载这些信息。

建议明确扩展变体或错误包装，保留原始原因，
避免落入 `InvalidPreparedScene(String)`。

依据：

- `novadraw/src/render/text.rs` 的 `TextError` 当前包含
  `InvalidMetric` 与 `InvalidPreparedScene(String)` 等变体。
- `novadraw/src/figure/preparation/scene.rs` 的 `scene_error`
  当前把错误格式化为 `InvalidPreparedScene` 字符串。
- [ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)
  要求测量错误保留失败 FigureId 和领域分类，
  不能为方便而把结构化失败转为字符串。

复审检查：明确错误类型、测量主体、约束与原始原因的传递方式；
区分不支持、非法尺寸、baseline 和派生溢出的验收要求。

## 修改清单与复审条件

以下项目在本报告基线中均待处理：

- [ ] GP01：区分 preparation 尝试和快照替换，定义 identity 保留与更新条件。
- [ ] GP01：新增重复 preparation 不重路由的收敛验收，保留同 bounds 新路径失效验收。
- [ ] GP02：区分快照计算依赖与必需路由发布屏障。
- [ ] GP02：明确新增依赖的环检测，补充直接环和跨 preparation 环反例。
- [ ] GP03：保留测量来源优先级，明确 Border/insets 单次组合。
- [ ] GP04：明确结构化错误承载与传递，保留主体、约束和原始原因。
- [ ] 给出逐项处理结果、新章节引用及未采纳建议的依据。

通过条件：补齐 GP01、GP02 的 P1 契约，对 GP03、GP04 给出明确处理或依据，
再复审新增公开协议。后续实现仍需兑现外部消费者、失败恢复、
首帧收敛及真实场景验收，不能以本报告替代执行证据。
