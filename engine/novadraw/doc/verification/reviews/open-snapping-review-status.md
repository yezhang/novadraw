# 开放吸附依赖与评审结论

类型：`architecture-review`

更新：2026-11-03。能力归属：Editor；Core、platform 和应用为独立依赖与验收 owner。
本页归并 ADR-058 的依赖核验、修订回应和复核结果。

## 当前结论与依赖

ADR-058 修订 2 于 2026-11-02 复核通过，满足用户“如果复核通过，则推进方案”的授权条件，
随后接受并进入 E04 实施。七项修订意见已在设计中解决；这不证明整体验收通过。
[ADR-058](../../adr/adr-058-open-editor-snapping.md)、
[开放吸附专题](../../design/editor/open-snapping.md)和
[矩阵](../plans/editor-open-snapping.md)分别维护决策、行为与验收要求。

GEF → Draw2D → Core 核验确认：所查坐标、committed bounds、有效可见性和反馈基础
契约已有 Core 对应入口。受检特征查询、会话有效性、策略与实际计划编排归 Editor。
来源或模型／权限变化沿用 E03 取消，独立配置与坐标变化按受限重算合同处理。
若后续确认必要 Core 缺失，仍须停止新增 Editor 能力，先提出独立 Core 增补建议。

## 最新实施与限制

开放注册、Grid／Geometry／Guide、第三方策略、prepared 复用、移动／创建／drop
和单源八方向及比例角 resize 已通过定向验证。release 采用最新位置，
实际几何须满足已采用事实；错误不重查、不降级为未吸附提交。
居中和多源 resize 不属于该切片。

六组无窗口成本记录显示：prepared 命中不能消除完整 owner／版本检查和分配。
10000 sibling 的完整 release 被 Runtime 默认 validation 上限阻塞，
正式成功样本为 0；这不是 preparation 预算或递归深度限制。
最新行为、数值、样本和失败证据统一见
[E04 实施与成本](p2-e04-a-implementation-2026-11-02.md)。

E04-C／D、完整成本、平台 capture／DPI 和最终门禁未关闭；
workspace fmt／check 已通过，Clippy 被既有 Core 三项告警阻断。
[校验成本提案](../../design/editor/snapping-validation-cost-proposal.md)仍待评审，
不授权 Core 改动。执行状态以[P2 backlog](../../roadmap/p2-delta-backlog.md)为准。

## 原评审依据

四份依赖核验、修订评审和回应已移出当前正文，原判断和完整依赖链见
[维护前快照](../../archive/knowledge-maintenance-2026-11-03.md)。
本页不把历史“待复核”改成当时已通过；只汇总后续已有事实。
