# P2-S07 Tree 与 Stable 查询实施证据

类型：`verification`

日期：2026-10-16

状态：切片已实现。本记录保留切片执行时的进度；后续消费者迁移与剩余验收
已完成，P2-S07 当前状态见[完成核验](p2-s07-query-identity-completion-2026-10-16.md)。

规范：[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)；
用例：[验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)。

## 实现

任意 ID 的 node、bounds、revision、style、布尔事实、children、depth、
ancestor/descendant、layout validity 先验证 namespace 和 live slot。
parent、连接/点列可选内容返回受检合法缺失。child_z_index 和 is_ancestor_of
在判断关系或相等前验证两个主体。contains 继续只判断 live 集合成员。

layout_manager 保留合法 None；layout_constraint 的错型现在返回
ConstraintTypeMismatch。专用 freeform 查询区分 foreign、unknown、NotFreeform、
Unvalidated；排除查询不再把派生几何失败变成零矩形。

StableSceneQuery 的 bounds/is_attached 直接保留 TreeQueryError，准入仍由 owner
判断 Faulted/NotStable。Event 查询保留领域身份转换与 detached 准入。
focus 请求区分 foreign 与 unknown。Node 三个弱化的旧尺寸 getter 已移除。

## 验证

| 范围 | 结果与证据 |
|---|---|
| `core.p2-s07-tree-query` | 3 项外部测试及 1 项内部 never-live 测试通过；`verification/evidence/p2-s07-tree-query-suite.log` |
| Core 库编译、Editor 库编译 | 通过；`p2-s07-tree-check.log`、`p2-s07-tree-editor-check.log` |
| Core 单测回归 | 370 项执行，368 项初次通过；2 项旧身份预期已修订并各自定向复跑通过 |
| contents 替换复跑 | `verification/evidence/p2-s07-tree-regression-retry.log` |
| direct focus 复跑 | `verification/evidence/p2-s07-focus-regression-retry.log` |
| 原全量单测记录 | `verification/evidence/p2-s07-tree-lib-tests.log` 保留初次失败，不改写为全量通过 |
| 文档门禁与 diff 检查 | 通过；`verification/evidence/p2-s07-tree-docs.log`，108 commands / 60 suites |

新增测试覆盖 S07-ID01–ID07 的 Tree/Stable 主体事实、双主体关系及 immutable 值
再入 live query。same-owner never-live 在 Core 私有测试构造，无公开注入入口。
外部 Stable 用例先完成正常 frame submission；仅 stabilize_for_query 仍可能因
待绘制工作拒绝准入，这是原有 owner 合同。

## 待完成

本记录不宣称全部任意 ID 入口或整个 S07 已验收。仍须核对 container handle、
image 等专用可选查询的身份通道，迁移 workspace integration tests、consumer、
Inspector、平台与示例，完成 quick 门禁及规范/API/parity 同步。
prepared、Border、projection、descriptor/freeform 组合负测、完整候选通知/damage/
stable epoch 隔离，以及 Route 删除旧绑定后替换为新有效绑定恢复，继续按矩阵推进。
最终关闭前执行一次 full 门禁。

## 本轮补充验证

scale_handle 使用 CapabilityQueryError；viewport_handle 和保存 handle 的 contents
查询使用 TreeQueryError；合法 absent 仍为 Ok(None)。image_display_state 区分
foreign 与 unknown。freeform 排除查询受检所有输入 ID，extent union 溢出不发布缓存。

当前 `core.query-contracts` 的 4 项外部 Tree/Stable 与 1 项内部 never-live 测试通过：
[执行记录](../../../verification/evidence/p2-s07-core-query-contracts.log)。
新增保存 viewport handle 在 foreign tree 与删除后查询的独立负测。

Core 全部 integration targets 已通过编译迁移：
[编译记录](../../../verification/evidence/p2-s07-core-consumer-compile.log)。
Inspector 与共享场景、示例宿主的库调用点已迁移；Editor integration、独立消费者、
可执行示例及最终公开 API/规范/门禁仍待完成。
