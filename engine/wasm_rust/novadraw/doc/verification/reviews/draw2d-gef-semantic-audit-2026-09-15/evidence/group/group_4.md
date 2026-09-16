# Group 4：Connection / Anchor / Router / Locator 语义审计

日期：2026-09-15。范围：`scope: full_file`，包括当前工作区修改，不限于 diff。

## 结论

保留 3 项高价值确定缺陷，均为条件性功能缺陷、P1，无 P0：

1. `SelfLoopRouter` 对宽扁节点生成穿过 owner 内部的回环。
2. `FanRouter` 对同组反向连接使用相反法向，导致分流后仍完全重合。
3. 切换 layer 默认 Router 时，旧共享组内的显式绑定成员没有失效。

Anchor、Router、SceneQuery 的公开对象安全 trait 与 Runtime-owned ID 是合理迁移。
但是“具备 trait”不等于完整可替换性已交付：当前 Connection prepared geometry
仍只承载 bounds 和 PointList；独立曲线几何策略、曲线 Locator/切线、完整
route observation、stroke-only 更新仍不能据此标为等价。

严格 viewport chain 相等是 ADR-014 接受的 Core 1.0 限制，不是本组缺陷。
Self-loop 两个独立 handle、默认矩形外部回环和可扩展圆弧是用户目标；
不是从 Draw2D 或任何上层扩展库推定出来的默认产品行为。

## 范围与证据

- `SKILL_ROOT`：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- `REPO_ROOT`：`/Users/bytedance/Documents/code/GitHub/drawjs`；JSONL 路径以此为基准。
- 项目根目录：`REPO_ROOT/engine/wasm_rust/novadraw`。
- `WORK_DIR`：项目内 `doc/verification/reviews/draw2d-gef-semantic-audit-2026-09-15/evidence`。
- 本组只交付 `group/group_4.md` 和 `group/group_4.jsonl`；总报告、最终 hash 和 cargo 门禁由主 Agent 完成。
- Novadraw HEAD：`6b83ac0fa980dc60fd1284607a51460858fa883b`，叠加实际读取的工作区。
- Draw2D/GEF HEAD：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
- 只读取 `org.eclipse.draw2d/src` 与 `org.eclipse.gef/src` 的参考代码及其 Javadoc；严格未使用 Zest，未扫描 `doc/archive/`。
- 已读 AGENTS、CLAUDE、项目记忆、ADR-014/015、`doc/design/architecture/connection-routing.md`、Draw2D API 账本相关部分与 G5 连接契约相关部分。
- 本文不是冻结整个工作区的报告。审计期间外部持续修改；已复读新增 Locator 注册、预检、提交、清理及测试差异，未把它们尚未接入时的状态当作当前结论。主 Agent 汇总时应再次核对行号/hash。
- **S**：本次源码与调用链静态确认；**T**：本次实际阅读测试断言，但未执行；**U**：未验证或未见覆盖本项的断言。T 不代表本次测试通过。
- 未改源码、现有测试、账本或路线图；未运行 cargo，未进行截图、GUI 或运行时复现。

下面 `C/` 表示 `novadraw-scene/src/connection/`，`R` 表示
`novadraw-scene/src/runtime/runtime.rs`，`G` 表示
`novadraw-scene/src/graph/mod.rs`。Java 短文件名均相对于
`org.eclipse.draw2d/src/org/eclipse/draw2d/`。

## 确认缺陷

### G4-1：宽扁节点的 self-loop 穿过节点内部

- 定级：P1，条件性功能缺陷，置信度 10/10；领域语义问题。
- 位置：`C/router.rs:257-267`，特别是 `outer_x = max(source.x, target.x) + extent`。
- 触发：owner 为 `(0,0,400,40)`，source/target 均为该 owner 的 ChopboxAnchor，使用 `SelfLoopRouter(DirectRouter,32)`，无 constraint。
- 静态推导：中心 `(200,20)`，参考点 `(232,4)`、`(232,36)`；`rectangle_boundary_site` 在 `C/anchor.rs:578-608` 返回 `(240,0)`、`(240,40)`。最终 route 为 `[(240,0),(272,0),(272,40),(240,40)]`。
- 结果：`x=272, 0<y<40` 的线段在节点内部；没有任何点越过 owner 右边界 `x=400`。改变节点宽高即可触发，不需要非法 Anchor 或非有限输入。两个 endpoint 虽然不同，仍不满足“外部矩形回环”。
- 依据：Draw2D `ConnectionRouter.java:41-47` 允许替换 route 算法，`AbstractRouter.java:50-72` 保留 Anchor location 真值；外部回环要求来自用户目标及 `SelfLoopRouter` 自身 `C/router.rs:216` 的公开说明，而非宣称 Draw2D 默认提供该产品策略。
- 现有证据：`m9_connection_contract.rs:235-285` 的 `self_loop_router_routes_same_owner_outside_and_delegates_other_pairs` 只用 `100x50` owner；`g5_connection_creation_contract.rs:1117-1144` 的 `policy_can_accept_a_self_loop` 检查端点不同，但没有宽扁 owner 的全线段外部性断言。S+T，反例未运行。
- 修复方向：通过 SceneQuery 获取并映射 owner 包络；外侧 lane 必须基于 owner 边界而非两个 endpoint 的最大 x。若端点落在顶/底边，增加遵循出射方向的外部 stub/折点，保证所有非端点线段不穿过 owner 内部；不能仅增大固定 extent。
- 验收：`100x50`、`400x40`、窄高矩形、缩放后的 owner；断言 endpoint 分离、端点符合 Anchor、整条非端点路径不穿 owner 内部；非空 bendpoint constraint 不被 wrapper 覆盖。

### G4-2：反向 Fan 连接仍完全重合

- 定级：P1，条件性功能缺陷，置信度 10/10；逻辑错误。
- 位置：`C/router.rs:518-526`。
- 触发：共享同一 Fan RouterId、同一 routing domain、同一对 AnchorId，child order 为 `A->B`、`B->A`。令 XYAnchor A=`(0,0)`，B=`(100,0)`，separation=`16`。
- 静态推导：组索引对应 centered index `-0.5` 和 `+0.5`；两个 direction 分别朝右、朝左，perpendicular 分别 `(0,1)`、`(0,-1)`。两个 midpoint 都是 `(50,-8)`。最终几何互为逆序，仍是同一条折线，影响视觉区分和逐条命中选择。
- 分组本来就是无向的：`C/runtime.rs` 的 `routing_group_members` / `unordered_pair_eq` 接受反向 Anchor pair；不能以“不同方向不应该进同组”豁免。
- Java 对照：`AutomaticRouter.java:21-29,46-73` 定义无向端点组合；`FanRouter.java:47-76` 在偏移前按几何方向规范化 ray。Rust 的对称居中分配可以保留，但组内偏移法向必须一致。
- 现有证据：`m9_connection_runtime.rs:638-681` 的 `fan_router_uses_stable_child_order_and_recenters_after_removal` 只覆盖同向成员。S+T，反例未运行。
- 修复方向：为无向组建立稳定的几何朝向，再按稳定 child index 分配有符号偏移；不要分别按每条边的 source/target 顺序确定法向。
- 验收：水平、竖直、斜向两条反向连接；3/4 条混合方向；重排、删边后仍分离且确定；保留各自 source/target 真值。

### G4-3：layer 默认 Router 切换遗漏旧共享组失效

- 定级：P1，条件性功能缺陷，置信度 9/10；领域语义问题。
- 位置：`C/runtime.rs:363-379`，`ConnectionRuntime::set_layer_router`。
- 触发：同一 parent 中两条同向连接 C1/C2 共用 Fan F。C1 为 `Inherited { layer }`，C2 为 `Explicit { router: F }`，此前完整 resolve 后两者均有偏移。将 layer 默认 Router 从 F 改为 Direct。
- 实际路径：`R:591-609` 直接调用 `set_layer_router`；affected 只筛选该 layer 的 inherited 连接，更新 layer defaults 后只标脏 C1。C1 用 Direct 重算，不再包含旧 Fan 组；C2 的 binding、owner geometry、relative transform、parent child order 均未变化。
- 结果：C2 仍显示两成员组时期的偏移路线，而 F 当前只有一个成员，按 `FanRouter::route` 应回到两点直线。`invalidate_stale_dependencies` 也没有 Router membership generation 可以发现该变化。必须另一次无关失效或显式 resolve C2 才恢复。
- Java 对照：`PolylineConnection.java:242-255` 更换 router 前调用旧 router.remove；`AutomaticRouter.java:103-120,137-151` 管理成员失效。Rust 不需要复制这些 callback，但必须保持旧/新组成员变化后的重算语义。
- 与合理变体区分：保留 C2 的 explicit binding 是正确的；不重算其旧共享组派生结果是不正确的。这不是要求 layer override 覆盖所有连接。
- 现有测试覆盖 remove/recenter、RouterId 隔离、constraint 不兼容原子拒绝，但未覆盖 inherited 成员离开、explicit 成员留在旧组的迁移。S+T，反例未运行。
- 修复方向：预检阶段在旧 binding 下计算受影响旧组，在候选 binding 下计算新组；合并两侧成员并统一预检/标脏。不要先覆盖 layer defaults 再丢失旧组信息。
- 验收：上述 Fan 场景以及 Manhattan 释放 lane；同时存在新 Router 显式成员；不兼容 constraint 时旧 binding/route/membership 全部不变；不同 RouterId/domain 不受污染。

## 语义映射

Family ID 使用现有 Draw2D API 账本，不创建新 milestone 或 family。表中“部分/缺口”是覆盖结论，不自动新增第 4 项缺陷。

| Family ID | Java 方法与源码行号 | 不可丢失语义 | Rust 公共入口与源码行号 | 判定 | 测试/证据 |
|---|---|---|---|---|---|
| `connection.figure` | `Connection.java:20,49-105` | Figure 身份、两端与 route points 可被统一消费 | `C/mod.rs:46-62`；`R:612-658`；`G:2533-2553` | 合理变体：ConnectionId 包装 FigureId，运行期受检 | `removing_connection_figure_cleans_runtime_state`，T+S |
| `connection.figure` | `Connection.java:76-105`；`Polyline.java:23-31,140-143` | 点控制几何，不独立设置任意 bounds 破坏 route | `C/figure.rs:81-104`；`G:2813-2871` | 部分：有统一提交；公共 capability 的安全使用仍依赖契约 | `resolved_route_replaces_dependencies_and_targeted_invalidation_marks_dirty`，T+S |
| `connection.figure` | `PolylineConnection.java:178-195` | 先路由，再 child layout，再旧/新 damage | `R:743-878`；`G:2826-2871` | 部分：新增批次 preflight；完整 subtree damage 尚无本次运行证据 | `runtime_relocates_bound_connection_children_after_route_commit`，T；damage U |
| `connection.figure` | `Polyline.java:73-78,93-95` | 几何 bounds 包含可见 stroke；paint 使用同一 route | `C/figure.rs:183-205,217-247` | 部分：Polyline 一致；最新 miter 包络修订不能等同曲线支持 | `resolved_route_replaces_dependencies_and_targeted_invalidation_marks_dirty`，T+S |
| `hit_test.search` | `Polyline.java:45-58` | 线段精确命中，容差独立于 paint bounds | `C/figure.rs:235-243,327-335`；`graph/search.rs:227-282` | 合理变体：半线宽加容差；cap/join 精确轮廓未逐项等价 | 测试只查 route midpoint；容差外缘、尖 miter U |
| `damage.repaint` | `PolylineConnection.java:184-194` | 旧 visual 和新 visual 均被修复，含超出 path 的 children | `G:2847-2870,3935-3955`；`R:851-868` | 部分：明确 erase/repaint；不能据此证明旧 subtree envelope 已完整覆盖 | 旧/新 overflow decoration、局部 repair 像素验证 U |
| `connection.anchor` | `ConnectionAnchor.java:28-54`；`AbstractRouter.java:50-72` | reference/location 双向求值与空间契约 | `C/anchor.rs:77-101`；`C/router.rs:561-597` | 等价语义，显式坐标域是合理变体 | `direct_router_uses_opposite_reference_points`，T+S |
| `connection.anchor` | `AbstractConnectionAnchor.java:53-72,111-134` | owner/ancestor 变化触发重算，注销解除观察 | `C/query.rs:115-135,258-334`；`C/runtime.rs` 的 `invalidate_stale_dependencies` | 合理变体：tracked query + Runtime 依赖索引替代 listener | `layout_output_geometry_automatically_invalidates_and_reroutes`，T+S |
| `connection.anchor` | `ChopboxAnchor.java:51-103` | owner geometry 与参考射线求交，可调整 anchor box | `C/anchor.rs:194-263,578-608` | 合理变体：浮点几何、named region；不复制 Java 像素偏移 | `chopbox_anchor_maps_owner_geometry_into_requested_space`，T+S |
| `connection.anchor` | `EllipseAnchor.java:46-73` | 椭圆边界交点 | `C/anchor.rs:291-342,611-626` | 部分等价：普通射线等价；中心参考退化选择不同 | `ellipse_and_rounded_rectangle_follow_their_actual_outlines`，T；退化 U |
| `connection.anchor` | `LabelAnchor.java:20-24,50-53` | 随 owner icon geometry 更新，不固定构造时矩形 | `C/anchor.rs:460-505`；`R:525-559` | 合理变体：named icon region 解耦 Label 类型 | `label_anchor_reads_named_icon_geometry`、`missing_named_geometry_is_a_structured_anchor_error`，T+S |
| `connection.anchor` | `XYAnchor.java:43-80` | 无 owner 固定位置，改变位置应失效 | `C/anchor.rs:146-184`；`R:504-523,661-679` | 合理变体：声明坐标域，以新 AnchorId 重新绑定替代原位 mutation | `xy_anchor_maps_from_its_declared_space`，T+S |
| `connection.anchor` | `ChopboxAnchor.java:112-130`；`AutomaticRouter.java:46-73` | 等价 anchor 的分组身份不能依赖临时地址 | `C/anchor.rs:11-53,217-224`；`C/runtime.rs` 的 `anchor_group_key` | 合理变体：semantic key 或 namespaced AnchorId | `anchor_semantic_keys_group_equivalent_owner_geometry`，T+S |
| `coordinate.conversion` | `ConnectionAnchor.java:28-54`；`AbstractRouter.java:50-72` | endpoint/normal 转换不能混用坐标 | `C/query.rs:13-22,363-377,477-493`；`C/anchor.rs:548-573` | 合理变体：显式 space 与 inverse-transpose normal | QueryFixture 仅平移；真实非均匀缩放/skew 场景 U |
| `connection.router` | `ConnectionRouter.java:25,67-94` | 默认直接两点，不擅自给所有连接引入产品路由 | `C/router.rs:187-213`；`C/runtime.rs:284-296` | 等价；最新 diff 已恢复 Direct 默认 | `inherited_router_defaults_to_draw2d_equivalent_direct_routing`，T+S |
| `connection.router` | `ConnectionRouter.java:34-61` | Router 可替换、constraint 可校验、失败不发布半条线 | `C/router.rs:32-76,112-184`；`R:570-609,681-698` | 合理变体：Any+TypeId、Result、owned output | `direct_router_rejects_constraints_without_partial_output`、`incompatible_router_change_is_rejected_atomically`，T+S |
| `connection.router` | `BendpointConnectionRouter.java:63-94` | 有 bendpoints 时首尾折点决定 Anchor reference | `C/router.rs:304-400,599-627` | 等价主语义；f64/weight 校验是合理变体 | `bendpoint_router_preserves_absolute_and_relative_constraints`，T+S |
| `connection.router` | `ManhattanConnectionRouter.java:29-38,49-50,197-280` | 同一共享 Router 的 row/column reservation 与删除失效 | `C/router.rs:403-463,637-731`；`C/runtime.rs:764-812` | 部分：共享 scope 已有；端点方向/完整候选算法不是逐项等价 | `shared_manhattan_reserves_lanes_across_different_anchor_pairs`、`manhattan_reservations_are_isolated_by_router_id`，T+S |
| `connection.router` | `ManhattanConnectionRouter.java:301-382` | 起终点方向参与正交路径选择 | `C/router.rs:425-453,629-631` | 部分：只取 normal 主轴，不能宣称保留 Java 所有有符号出射分支 | `manhattan_router_emits_only_orthogonal_non_duplicate_segments` 只覆盖一种布局，T；反向法向 U |
| `connection.router` | `AutomaticRouter.java:162-191`；`FanRouter.java:47-76` | 只分流两点碰撞路线，无向组方向一致 | `C/router.rs:481-530` | 缺口：反向重合，见 G4-2 | `fan_router_uses_stable_child_order_and_recenters_after_removal`，T+S |
| `connection.router` | `ConnectionLayer.java:42-49,80-99`；`PolylineConnection.java:242-255` | 默认 router 切换同时处理成员退出与加入 | `R:591-609`；`C/runtime.rs:357-381` | 合理 explicit/inherited 变体，但旧组失效缺口 G4-3 | mixed-binding 迁移 U；静态 S |
| `connection.figure` | `PolylineConnection.java:204-208,278-329` | 生命周期清理和重新绑定 | `C/runtime.rs:240-277,304-354,416-432`；`R:645-679` | 合理变体：registry 所有权、InUse 错误、销毁清理 | `shared_router_and_anchor_cannot_be_removed_while_referenced`、`removing_connection_figure_cleans_runtime_state`，T+S |
| `connection.figure` | `PolylineConnection.java:179-195`，并参照 ADR-014 §6 | unresolved 不伪装旧 route 有效；失败批次保持一致 | `C/runtime.rs:567-645,903-917`；`R:775-833,871-876` | 部分：geometry/locator 预检已补；新拒绝路径恢复依赖仍需覆盖 | `invalid_connection_geometry_never_commits_resolved_state`、`manhattan_scope_failure_clears_the_complete_batch_and_recovers_atomically`，T+S |
| `connection.locator` | `ConnectionLocator.java:120-143` | Middle 是中央点或中央 segment 中点，不是总弧长一半 | `C/locator.rs:20-60` | 等价，忽略 Java 整数舍入 | `locators_preserve_topological_middle_and_arc_fraction_semantics`，T+S；偶数点 U |
| `connection.locator` | `MidpointLocator.java:29-32,60-70` | 第 i 与 i+1 点中点，越界不能静默原点 | `C/locator.rs:62-79,178-196` | 合理变体：结构化错误 | 同上测试只覆盖合法 i=0；越界 U |
| `connection.locator` | `ConnectionLocator.java:120-143`，无弧长等价项 | 新语义不得覆盖 Middle 名称 | `C/locator.rs:81-128` 的 `PathFractionLocator` | 合理扩展：折线累计长度，不是 Bézier 曲线弧长 | fraction=0.75 的 T；曲线 U |
| `connection.locator` | `DelegatingLayout.java:50-57,65-66` | 将 Locator 注册给 child，route 后定位，删除时清理 | `R:702-729,789-868`；`C/runtime.rs:435-486` | 部分：最新已接入，不能再报“无 Runtime Locator”；仅中心定位 | `runtime_relocates_bound_connection_children_after_route_commit`、`connection_locator_rejects_non_child_targets`，T+S |
| `connection.locator` | `ArrowLocator.java:44-55`；`ConnectionEndpointLocator.java:197-255` | decoration 朝向、端点偏距、child 尺寸语义 | `C/locator.rs:5-17,31-45`；`R:805-847` | 部分：reference 被校验但未用于旋转；没有 u/v distance 等价产品 API | 当前新增测试只断言 child center，T；朝向 U |
| `connection.figure` | `PolylineConnection.java:298-308,337-346` | decoration 独立于真实端点位置 | `C/figure.rs:160-175,352-379` | 合理变体：仅 paint/hit trim，不改 committed endpoint | `target_decoration_inset_trims_only_painted_geometry`、`decoration_inset_can_cross_short_terminal_segments`，T+S |
| `connection.router` | `ConnectionRouter.java:41-47`；`AbstractRouter.java:50-72` | 自定义 wrapper 仍调用两个 Anchor，不吞显式 constraint | `C/router.rs:216-286` | 部分：可组合，但外部回环缺口 G4-1；不能保证任意固定 port 都分离 | `self_loop_router_routes_same_owner_outside_and_delegates_other_pairs`，T+S |
| `connection.figure` | GEF `editpolicies/ConnectionEndpointEditPolicy.java:111-115` | source/target 是两个独立 handle 角色 | `novadraw-editor/src/viewer/mod.rs:831-859`；`feedback/mod.rs:46-47`；demo `main.rs:1142-1180` | 合理分层：Editor/应用消费 committed endpoints；不在 Draw2D Runtime 合并角色 | `policy_can_accept_a_self_loop`，T+S；两端实际拖拽本次未运行 |
| `connection.anchor` | GEF `NodeEditPart.java:40-64` | 稳定连接的 source/target Anchor 可由节点和连接模型共同选择 | `novadraw-editor/src/part/mod.rs:633-705,899-918`；`viewer/mod.rs:1647-1689,1927-2006` | 最新已开放：descriptor/key + 独立端点 hook；撤销“始终硬编码 Chopbox”的旧结论 | 本次读到 hook、fallback、key 比较与注册调用，S；自定义 port 集成测试 U |
| `connection.router` | `PolylineConnection.java:179-195`，并参照 ADR-014 取舍 | 跨 viewport 显示不可越界泄漏 | `C/runtime.rs:1202-1251`；`C/query.rs:390-394` | 合理但严格的限制：相同完整 viewport chain 才允许；不是 Draw2D 完全等价 | `divergent_viewport_topology_is_rejected_and_reparent_recovers`、`matching_viewport_topology_routes_normally`，T+S |
| `connection.router` | `ConnectionRouter.java:41-47`，并参照项目固定 validation DAG | route 不得依赖自身派生几何形成循环 | `C/runtime.rs:1273-1321` 的 `dependency_cycle` | 部分：直接 self/descendant 检查；不是一般跨连接 DAG 证明 | `connection_cannot_anchor_to_its_own_subtree`，T；跨连接环 U |
| `connection.figure` | `Polyline.java:140-143`；`PolylineConnection.java:254,362-406` | points/binding 变化可观察，已提交通知不冒充拦截 hook | `G:2863-2868`；`C/runtime.rs:614-631` | 缺口：当前读到的是 FigureMoved/状态快照，不是规范设计完整 ConnectionChange old/new journal | typed RouteChanged、同点不虚报 U |

## 当前 Path 修改与可替换性

### 已经改善的部分

`PreparedConnectionGeometry` 在 `C/figure.rs:21-65` 引入有限 bounds/outset 预检，
`ConnectionFigureBehavior::prepare_route_geometry` 可由外部实现覆盖。`G:2813-2823`
不再直接把所有外观硬编码成半线宽包络。`R:775-868` 在提交前准备完整批次的
几何和 Locator placement，最后才提交 connection resolution。

`C/figure.rs:183-193` 与 Vello backend 的新 `DEFAULT_STROKE_MITER_LIMIT=4`
使用同一 miter 上限。此前“几何拒绝但状态已经 Resolved”的顺序已改变，
不作为当前缺陷；新测试 `invalid_connection_geometry_never_commits_resolved_state`
已读到明确 unresolved、零 bounds、空 points 断言，但未运行。

本轮还读到 `R:702-729` 的 Locator 公共绑定/解绑、批次 placement 预检、
`C/runtime.rs:261-263,429-430` 的销毁清理和两个新增测试。不能用审计开始时没有
这些代码的事实覆盖最新实现。

### Editor 最新投影边界与 Group 5 对齐

按用户最新通知复读 `part/mod.rs` 和 `viewer/mod.rs` 的完整相关函数：

- `ConnectionAnchorContext` / `ConnectionAnchorDescriptor`
  （`part/mod.rs:633-705`）传递连接/两端模型身份、Figure 身份与稳定 key。
- `EditPartBehavior::{source_connection_anchor,target_connection_anchor}`
  （同文件 `899-918`）是独立可覆盖的端点策略入口。
- `build_connection_anchor`（`viewer/mod.rs:1647-1689`）实际调用端点 behavior，
  只有返回 None 才 fallback Chopbox；不是仍然强制使用 Chopbox。
- `bind_connection_part`（同文件 `1901-2007`）重新取得两端 descriptor，
  结合模型 ID、AnchorId 存在性和 semantic key 判断复用，只替换发生变化的端点。
  原“节点 ID 不变就没有 Anchor 更新入口”的结论不再成立。

这补上了 GEF `NodeEditPart.java:40-64` 的稳定连接投影扩展入口；不能直接扩大为
`NodeEditPart.java:66-88` 的 Request feedback Anchor 选择也已经等价。
自定义 port 刷新、重连与未变化端身份保持的运行验证交给主 Agent/Group 5。
descriptor key 必须反映 Anchor 策略的语义变化，这是扩展实现应遵守的身份契约。

本次交叉读取的 `group_5.md` 仍保留其较早快照下“无 anchor hook”的映射、
§4.5 及迁移步骤 5 描述。汇总时应采用以上最新源码证据撤销该旧缺口，
而不是为了文字一致重新报告已经修复的限制；本组不覆盖其他组报告。
Router/Constraint 投影和独立几何策略是不同边界，不能由新增 Anchor hook
自动判为完成。本组 G4-1/G4-2/G4-3 的 Router 位置在本次复核时仍未改变。

### 仍需区分的三种扩展能力

1. **替换 Anchor/Router：已开放。** 公共 trait 不依赖 FigureTree 具体类型，
   SceneQuery 短借用不能安全逃逸；Runtime 注册 Box 并分配 namespaced ID。
   不复制 Java 地址身份、Singleton scratch point 和直接 mutation 是合理的。
2. **替换整个 Connection Figure：可以。** 外部 Figure 可实现
   `ConnectionFigureBehavior`，并自行实现 paint/precise_hit，甚至从 route points
   推导曲线。当前不能据此声称“外部绝对无法画圆弧”。但扩展方需要同步维护
   曲线计算、命中、包络与 route-point 定位关系；本组未见外部曲线 Figure
   的完整协议验证。
3. **保留通用 Connection Figure，仅替换几何策略：尚未形成完整公共契约。**
   `RouteOutput` 仍是 PointList+两个 endpoint metadata（`C/router.rs:105-154`），
   prepared geometry 仍是 `Rectangle + PointList`，commit 只传 PointList，
   内置 paint 仍调用 `gc.polyline`。没有携带同一已验证 Path/几何对象的 prepare、
   commit、paint、hit、locator 共用协议。不能把这次修改称为完整 Path 支持。

Render 层已有 `Path::{cubic_to,quad_to,arc_to}`（`novadraw-render/src/command.rs:289-330`），
但这是绘制表达能力，不是 Connection 几何契约。尤其当前
`Path::bounding_box` 的曲线分支只查看终点（同文件 `368-380`）；
将该包络直接用作未来曲线 connection damage 依据并不成立。该共享实现交由
Group 3 汇总，本组不重复追加缺陷条目。

### Rust 迁移不能自动豁免的限制

- `&self` 不禁止 interior mutation；Anchor、Router、Locator 的纯计算与不 panic
  约定需要文档和外部实现门禁。Runtime fault guard 是恢复边界，不是用户代码可回滚证明。
- `Any + TypeId` 能拒绝错误 constraint 类型，但内容有效性仍在具体 router 计算时检查；
  不能声称设置时已验证所有语义。失败源输入与派生 unresolved 的分层符合 ADR-014。
- `ConnectionId::from_figure` 是公开 unchecked wrapper 构造器，角色正确性靠注册 API；
  ID 所有权隔离不能等同静态证明“任何 ConnectionId 都一定存活且是 Connection”。
- 内置 `ConnectionFigure` stroke/cap/join 是构造期设置；没有读到等价
  `setLineWidth` 的运行期 connection style-only 发布流程。
- 新 Locator 默认把现有 child bounds 居中，未使用 reference 设置朝向，
  也不是 Draw2D `ConnectionEndpointLocator` 的 owner-side/u/v distance/测量语义。
  重复 endpoint segment 当前可以返回 `point == reference`，并未选择最近非零切线。
- 当前严格 viewport 检查按 owner() 的主 owner 和内置 ViewportFigure 识别；
  ownerless XYAnchor 不声明其引用坐标域的嵌套视口可见性。这与现有设计限制一致，
  但不是任意自定义 clipping 容器的拓扑扩展证明。
- 新 geometry/locator 拒绝分支的 `reject_route_batch` 只设置 unresolved；
  与 `fail_route_batch` 的 observation merge 不同。首次失败后的依赖恢复场景
  需要主 Agent 验证，不把无限 stroke 的单次拒绝测试当作恢复协议覆盖。
- 当前普通 reparent 调用链（`R:1550-1585`）未展示 absolute bendpoint 的空间迁移
  或 inherited layer binding 的同步重绑定。此项记为迁移契约覆盖缺口，不能拿
  “移动 endpoint owner 后恢复”的 viewport 测试替代“移动 connection 自身”的验收。

## 迁移顺序与验收

1. 先补 G4-1/G4-2/G4-3 的最小回归场景，再修 router 计算与旧/新 group invalidation；
   不改 Anchor 真值，不修改渲染主循环，不在应用写依赖传播补丁。
2. 定义 canonical route 与表现几何的边界。普通折线继续保留有序 bendpoints；
   曲线扩展需有一个已验证的 geometry snapshot，统一提供 paint path、命中、
   stroke envelope、endpoint tangent 和沿路径定位。是否以 trait 或稳定 IR
   承载，应通过真实外部实现验证后决定，不预设继承式方案。
3. 明确外部矩形回环和可选圆弧策略各自如何产生两个独立 Anchor endpoint。
   固定 port 恰好同点时必须显式定义拒绝或其他策略，不能悄悄合并 handle，
   也不能为了画圆弧而修改已经绑定的 Anchor 真值。
4. 在批次 preflight 中统一 geometry/locator 错误和恢复依赖；
   完成 child 定向/尺寸更新后计算新 subtree envelope，再发布稳定通知。
5. 最后由主 Agent 执行 Native/Web/Headless 门禁。至少覆盖：
   宽扁 self-loop、两个 endpoint 分别 reconnect+undo/redo、显式 bendpoint 优先、
   外部曲线 Figure/策略、反向 Fan、layer Router 迁移、跨 domain 隔离、
   geometry/locator 首次失败再恢复、overflow decoration old/new partial damage、
   坐标缩放/skew、相同 viewport chain 的滚动/zoom 不改变 canonical route、
   divergent chain 的明确拒绝与恢复、connection 自身 reparent。

曲线产品、ShortestPath/避障、nearest-common-viewport clipping 未实现但已有后置边界，
不作为“本次代码引入的 bug”。G5.4 折点交互由 Editor 组和主 Agent 验收，
本组只检查其连接几何接口没有被封闭。

## 七维度复核

| 维度 | 本组结果 |
|---|---|
| 逻辑 | Fan 方向规范化缺失，G4-2；已复读 diff 新侧，没有报告删除代码的问题 |
| 领域语义 | self-loop 外部性、旧共享组失效，G4-1/G4-3；其余覆盖差异见映射 |
| 安全 | 本组是本地几何/运行期状态接口，未发现可证实的新增安全漏洞 |
| 并发 | Runtime 独占写入、短借用查询；没有发现本组明确并发竞争，未将纯计算契约当成自动线程安全保证 |
| 健壮性 | RouteOutput 有点/metadata 校验；新 geometry/locator preflight 已读；错误恢复与扩展 panic 仍需门禁 |
| 性能 | 多处 invalidate_all 和共享组重算为保守实现；未运行基准，不将无数据的优化猜测定级 |
| 质量 | 不报告风格、长函数、命名或 magic number 类建议为本组缺陷 |

## 实际阅读清单

完整读取本组 `anchor.rs`、`figure.rs`、`locator.rs`、`mod.rs`、`query.rs`、
`router.rs`、`runtime.rs` 及两个 `m9_connection_*` 测试文件；对审计期间新增
Locator/geometry/runtime 测试差异再次读取。

按相关函数读取共享边界和直接调用方：

- `novadraw-scene/src/figure/mod.rs` 的 connection capability、paint/hit/visual bounds；
  `graph/mod.rs` 的 route prepare/commit/clear、route 查询、erase/repaint；
  `graph/search.rs` 的完整 hit-test descent。
- `novadraw-scene/src/runtime/runtime.rs` 的连接注册/绑定/解析、Locator、依赖调度、
  stabilize、reparent 和 fault guard。
- `novadraw-scene/src/lib.rs`、`novadraw/src/lib.rs` 的公开 re-export；
  `novadraw-render/src/command.rs` 的 Path 和 miter 常量；
  `novadraw-render/src/backend/vello/mod.rs` 的全部本次 miter diff。
- `novadraw-editor/src/lib.rs`、`feedback/mod.rs` 的角色边界；
  `viewer/mod.rs` 的 endpoint/route 查询、连接 Anchor 构造和解析；
  demo `main.rs` 的 self-loop router 安装与两个 handle 创建；
  `g5_connection_creation_contract.rs` 的 self-loop fixture/断言。
- 最终补充复读 `novadraw-editor/src/part/mod.rs:633-705,899-918`，
  `viewer/mod.rs:1647-1689,1901-2007` 的 Anchor hook/descriptor/key 消费；
  交叉读取 Group 5 报告，仅用于识别需要撤销的旧快照结论。

完整读取 Java 源码及类/方法 Javadoc：
`Connection.java`、`ConnectionAnchor.java`、`ConnectionRouter.java`、
`AbstractRouter.java`、`PolylineConnection.java`、`Polyline.java`、
`AbstractConnectionAnchor.java`、`ChopboxAnchor.java`、`EllipseAnchor.java`、
`LabelAnchor.java`、`XYAnchor.java`、`BendpointConnectionRouter.java`、
`ManhattanConnectionRouter.java`、`AutomaticRouter.java`、`FanRouter.java`、
`ConnectionLayer.java`、`Locator.java`、`ConnectionLocator.java`、
`MidpointLocator.java`、`ArrowLocator.java`、`DelegatingLayout.java`、
`ConnectionEndpointLocator.java`。

另完整读取 GEF `NodeEditPart.java`，以及 `ConnectionEndpointEditPolicy.java`
的 `createSelectionHandles` 与周边上下文。
本基线实际类名是 `ConnectionEndpointLocator`，不是 `EndpointLocator`；
未凭账本简写虚构不存在的源码文件。未把索引命中、旧审计通过或测试名称存在
计为本次执行通过。
