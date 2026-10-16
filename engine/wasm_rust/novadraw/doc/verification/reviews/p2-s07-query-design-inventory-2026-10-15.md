# P2-S07 查询设计盘点

类型：`verification`

日期：2026-10-15

状态：`design-draft`；静态反例复核已完成，新增行为用例未执行。

## 结论

R04/TC-09 的默认成功值问题仍存在，API-F06 的 parent/bool 身份折叠和 API-07 的
非权威 Node 尺寸 getter 也仍存在。S07 需要调整任意 ID 查询及 LayoutManager 测量
返回值，并贯通原有候选失败通道；不需要重建 Context 或 LayoutOutput。

设计候选为[ADR-038](../../adr/adr-038-query-identity-and-failure-semantics.md)，
验收要求为[验证矩阵](../plans/p2-s07-query-identity-and-failure-semantics.md)。
草案不等于批准实施，四个来源审计 ID 保持开放。

## 核对顺序与依据

先读取启动规则、现行路线图、ADR-018/036 和四份审计，再检查 Draw2D Figure 的
parent、布尔属性与测量委托语义。目标契约写入 ADR 草案后，才定向核对本项目 query、
layout measurement、Node getter 与阶段 query；未先扫描 Rust 推导理想架构。

参考：`gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/Figure.java`：
getMaximumSize 846、getMinimumSize 865、getParent 882、getPreferredSize 898、
isEnabled 1139、isShowing 1195、isVisible 1226。
允许的参考范围仅为 Draw2D/GEF，不使用 Zest。

审计来源：

- [R04](api-architecture-review-2026-10-09.md#r04查询面默认值吞无效身份扩展-tc-09)；
- [TC-09](temporary-concept-design-audit-2026-10-08.md#39-tc-09查询默认值伪装成功)；
- [API-F06](framework-public-api-follow-up-audit-2026-10-07.md#api-f06树查询混淆合法缺失与无效身份)；
- [API-07](core-public-api-audit-2026-09-22.md#api-07查询命名和权威数据源不统一)。

## 当前反例与处置

行号为本次工作区静态快照，不代表已执行的 runtime 证据。

| ID | 当前证据 | 处置 |
|---|---|---|
| S07-C01 | [graph/query.rs](../../../novadraw/src/graph/query.rs)：node、parent_id、child_order、bounds、depth 等直接读取 arena Option；本地与有效布尔查询将缺失折叠为 false | 任意 ID 读取改为受检；Option 仅表达合法缺失；保留本地/继承区别 |
| S07-C02 | [identity.rs](../../../novadraw/src/identity.rs)：RuntimeArena::get 120 先判 namespace 后读取 SlotMap，两种失败均为 None；[search.rs](../../../novadraw/src/graph/search.rs) TreeQueryError 仅 UnknownFigure | 公共查询先判 namespace，复用 tree 身份错误并增加 foreign；unknown/stale 合用缺失分类 |
| S07-C03 | [layout/mod.rs](../../../novadraw/src/layout/mod.rs)：LayoutSceneRead 55 与 LayoutSceneQuery 97–160 的多数读取不含 Result；constraint_as 仅错型时失败，未验证 container/child 身份与关系 | 将 identity/relationship 贯通 query；保留类型约束领域错误 |
| S07-C04 | [graph/layout_measurement.rs](../../../novadraw/src/graph/layout_measurement.rs) 962 起：get_children 缺失返回空集合并 filter_map；preferred/minimum/maximum 使用 default/ZERO/INFINITY；bounds 返回 ZERO | 删除身份失败替身；未知 child 不被过滤成合法较小布局 |
| S07-C05 | 同文件 1024 起：freeform_extent `.ok()`；scale capability `.ok()?` | NotFreeform 可在可选入口成为 None；Unvalidated/identity/descriptor 错误必须保留 |
| S07-C06 | 同文件 preferred_measurement 605、minimum_size 651 返回 Option，LayoutManager callback 返回确定值并填缓存；[layout/mod.rs](../../../novadraw/src/layout/mod.rs) 330–350 无 measurement error 通道 | preferred/minimum 改 Result，失败不填成功缓存；保留权威组合和 baseline |
| S07-C07 | [graph/mod.rs](../../../novadraw/src/graph/mod.rs) 860–882 的 get_preferred/minimum/maximum_size 仍公开；前两者只读 override/current bounds，不含 LayoutManager/intrinsic/Border | 删除非权威 getter，迁移到 tree/query 测量；不是已经关闭的旧发现 |
| S07-C08 | [graph/query.rs](../../../novadraw/src/graph/query.rs) 五个 transform 都返回 Option，unknown/foreign 与 inverse None 同通道 | 先判身份，正向 Result，逆向 Result<Option>；保留奇异矩阵领域语义 |
| S07-C09 | [runtime/update/listener.rs](../../../novadraw/src/runtime/update/listener.rs) 162–183：StableSceneQuery::bounds 为 Option、is_attached 为 bool | owner 准入已受检，但并未验证调用者任意 ID；补 tree identity 返回 |
| S07-C10 | [connection/query.rs](../../../novadraw/src/connection/query.rs) 289–302：tracked topology/visibility 只记录 generation 后返回 source 值；399–424 tree adapter 的 border_box 先以 is_attached 判 detached，再读 bounds | tracking 不是 identity admission；RouteSceneRead/Query 共用受检读取，先 foreign/missing 后 detached |

S07-C10 是同一身份根因在 Route 阶段的当前反例，修复范围限定为查询与错误传播。
不重做 ADR-036 命名，也不改变 route unresolved、anchor 或 connection 生命周期合同。

## 已有机制与不重复实施的事项

| 机制 | 当前证据 | 本批次安排 |
|---|---|---|
| Event foreign/disposed/detached 分离 | [runtime/context.rs](../../../novadraw/src/runtime/context.rs) 29–69，EventSceneQuery 已受检 | 复用规则并做迁移回归，不再造 EventContext |
| typed capability 合法缺失 | [graph/query.rs](../../../novadraw/src/graph/query.rs) capability 9–23，返回 Result<Option> | 保留 CapabilityQueryError，不为名称一致替换整个领域错误 |
| LayoutOutput 未发布候选 | [graph/layout_measurement.rs](../../../novadraw/src/graph/layout_measurement.rs) 277–304：恢复 manager 后先 `result?`，再 apply_layout_output | 复用提交顺序，新增失败贯通用例；允许 Started 诊断，不承诺整轮回滚 |
| stable owner 准入 | [runtime/runtime.rs](../../../novadraw/src/runtime/runtime.rs) 2514–2523，fault/pending 拒绝构造 query | 保留 Faulted/NotStable，不给 FigureTree 添加 Runtime fault 状态 |
| ADR-036 命名与权限 | [完成记录](adr-036-context-phase-authority-completion-2026-10-13.md) | 已完成基线，不把完成项重复列为 S07 Execute |
| 合法无上限与空 extent | maximum_size 对有效 node 无 override 返回无限；recompute_freeform_extent 对空集合使用 ZERO | 保留合法结果，不能按 default/INFINITY 字面批量删除 |

## 旧建议的修正

R04 的统一 QueryError、所有结构查询统一 Result<Option> 是历史方案建议。
当前执行清单明确要求先分类，再根据真实消费者决定组合方式。草案采用共享 tree
identity、领域错误组合和按结果基数选择返回值；不把错误类型数量作为整改指标。

API-07 的名称迁移不能替代语义收口。此次确认 Node 三个旧 getter 仍在，应删除或改为
明确局部值语义；草案选择删除。已经完成的 get_contents/get_block 等迁移不重做。

## 实施风险与待验证事项

公共查询返回值会影响 Core、Editor、平台、examples、tests 和独立 consumers。
LayoutManager measurement 改 Result 尤其需要检查 nested measurement、cache、minimum
回退和 Border/insets；按 tree、layout、跨 crate 三个切片推进。

本文没有证明组合变换溢出、panic、slot 重用或候选通知/damage 的执行结果；
这些均列入矩阵。实际用例与证据通过前，不关闭四个审计 ID，不修改 parity 行为完成
状态，也不把 S07 标为 in_progress/complete。

S06 已按用户接受的矩阵修订 2 关闭。其完整 prepare 拒绝集成未执行、Web fault
异步退出与 Native surface 保留项不因 S07 设计而变化，详见
S06 的独立完成记录 `p2-s06-failure-isolation-completion-2026-10-15.md`。

## 草案交付检查

- `cargo xtask docs`：PASS；metadata、链接、索引和 public API probes 通过，日志见
  [p2-s07-design-docs.log](../../../verification/evidence/p2-s07-design-docs.log)，退出码 0。
- `git diff --check`：PASS。
- 按好好说话原则回读：保留身份、缺失、fault 与候选保证的差别；新文档中文相邻字符间
  半角标点、破折号和空泛元话语机械扫描为 0 命中。
- 本阶段只新增设计文档并同步索引/队列，没有修改 S07 Rust 实现或注册空 suite。
  文档门禁的 doctest/API probes 不能代替矩阵的新增行为用例。
