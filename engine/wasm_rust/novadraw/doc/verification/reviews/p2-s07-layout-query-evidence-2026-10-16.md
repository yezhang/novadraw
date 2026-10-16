# P2-S07 布局查询实施证据

类型：`implementation-verification`

状态：布局切片已实施。本记录保留切片执行时的进度；后续验收已完成，
P2-S07 当前状态见[完成核验](p2-s07-query-identity-completion-2026-10-16.md)。

规范：[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)。
验收范围：[验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)。

## 已实施合同

Tree preferred/minimum/maximum 与 LayoutSceneQuery 的任意 ID 方法使用 Result。
LayoutManager preferred/minimum 具有错误通道，内置布局器与外部布局器共用查询入口。
TreeQueryError 区分 foreign 与本 owner 的 unknown/retired generation；
LayoutError 通过 Identity 保留来源，另保留 Freeform 与 Capability 领域原因。

测量 owner 校验选中分支的尺寸和 baseline，再校验 inset 组合及 container projection。
maximum 允许正无穷；preferred/minimum 拒绝非有限或负尺寸。非法 callback Ok 值、
非法 baseline 和派生溢出有结构化原因，失败不写成功缓存。

XY、Border、Freeform 与 Grid 的测量路径移除了吞掉约束或子测量错误的逻辑。
callback 返回错误时，既有提交边界丢弃整轮未发布 LayoutOutput。
这不承诺回滚此前的 source mutation 或其他已提交前缀。

## 执行结果

`core.p2-s07-layout-query` 通过：6 项外部 integration 测试、4 项 Core 内部测试。
完整日志：[布局查询证据](../../../verification/evidence/p2-s07-layout-query.log)。

| 矩阵范围 | 实际证据 | 覆盖边界 |
|---|---|---|
| ID01–03、L01 | foreign/retired 外部测试与同 namespace missing 内部测试 | LayoutSceneQuery 全部方法及 Tree 测量；其他 Tree 方法尚未迁移 |
| ID05、L02、L07 | 直接父子关系、空 children、缺 constraint/freeform/scale、零尺寸与无 maximum override | Freeform Unvalidated 与 descriptor 错型沿用结构化错误，专项负测仍待补充 |
| L04–05、L08 | 外部布局器先追加合法 bounds，再查询无效 ID 并传播错误；稳定化拒绝候选 | bounds 未提交；notification、damage 与 stable epoch 的独立断言仍待补充 |
| L09–10 | callback NaN/负值/无穷/非法 baseline，preferred/minimum/maximum override 各轴非法值 | maximum 正无穷与零值保持成功；prepared/Border 独立分支仍待补充 |
| L11–12 | container projection 与 inset 溢出，重复失败不命中缓存，修正后重试成功 | inset 修正通过正常 invalidation；其他分支与 Runtime publication 证据仍待补充 |

M5 layout、结构化测量、深层 presentation 和原子 child insertion 的 30 项回归通过。
这些回归证明既有几何组合顺序和合法布局行为，不能替代尚未执行的 S07 负测。

workspace quick gate 通过，包含 Native/Web 编译与公共 API 边界检查：
[执行日志](../../../verification/evidence/p2-s07-layout-quick.log)。
本记录不关闭任何审计条目。

## 规范同步与剩余工作

ADR-018 的测量签名、公共 API 契约、Draw2D layout.manager 账本、
API symbol snapshot 与验证 manifest 已同步。坐标的 accepted 目标合同已标明迁移状态。

五个坐标入口、可靠求逆及 Route 身份已进入
[后续切片](p2-s07-coordinate-route-query-evidence-2026-10-16.md)。
后续继续 Tree 结构/状态、Stable/Event 同规则核对及 Route 完整绑定恢复；
移除 Node 非权威尺寸 getter，补齐矩阵剩余证据后执行最终门禁。
S08/S09 未启动，受保护的递归渲染主循环未修改。

## 本轮补充验证

命令体系调整后，S07 用例统一进入 `core.query-contracts`。本轮执行通过：
[完整记录](../../../verification/evidence/p2-s07-core-query-contracts.log)。
布局部分为 7 项外部测试和 6 项内部测试。

新增 prepared 构造准入、prepared 与 Border 组合、Border baseline/尺寸溢出；
新增内部候选隔离测试，确认失败 callback 的合法 bounds 前缀和 repaint 候选均未发布，
没有派生 geometry 通知或 damage，仅允许 Started 诊断。
外部候选失败测试确认 latest stable epoch 保持原值。
普通 revalidate 驱动成功缓存失效、非法测量重复失败和合法修正后重试均通过。

Core 全部测试目标已完成编译迁移；Editor integration、独立消费者、可执行示例与
最终规范/门禁仍待完成。本轮不关闭 S07。
