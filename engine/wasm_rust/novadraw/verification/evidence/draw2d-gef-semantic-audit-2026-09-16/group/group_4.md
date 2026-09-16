# Group 4：Connection / Anchor / Router / Locator 与 Runtime 原子语义审计

## 结论

本组保留 3 条高置信度 P1，均由当前完整函数和具体输入路径推导，不依赖昨日报告：

| 编号 | 缺陷 | 位置 | 置信度 |
| --- | --- | --- | --- |
| G4-F1 | geometry/Locator 预检拒绝未合并依赖，输入恢复后连接无法自动恢复 | `novadraw-scene/src/connection/runtime.rs:650-658` | 10/10 |
| G4-F2 | 反向 Fan 连接的法向和序号同时翻转，导致两条线路重合 | `novadraw-scene/src/connection/router.rs:428-436` | 10/10 |
| G4-F3 | Connection reparent 未转换或拒绝旧域 absolute bendpoint | `novadraw-scene/src/runtime/runtime.rs:1638-1643` | 10/10 |

prepare/validate/commit 主链、Runtime Locator binding、Editor Anchor descriptor 和逐连接依赖索引更新均已存在，不能继续报告为“未实现”。本次未运行 cargo、测试、产品程序或性能基准；以下测试引用只表示已阅读断言，不表示执行通过。

主审同步结果：`workspace.quality` 与 G3-G5.5 replay 全部通过，来源为主审通知，本组未重复执行。该通过事实与本组三条未被现有断言覆盖的静态触发路径分别记录。按主审汇总要求，本组已停止扩大扫描，保留已修复事实和未验证边界；产物格式检查确认 17 项映射、3 条合法 JSONL，缺陷行号均在当前文件范围内。

## 范围与基线

- 日期：2026-09-16；读取时 HEAD：`e8d54ac063d05c63abb9f2850365a837ec4e8c9d`，以当前脏工作区为准，不只看 HEAD。
- `scope: full_file`；执行已有 `review_groups.md` 的 Group 4，不重新拆组，不做 diff 行过滤。
- 项目根：`/Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw`；Git 根：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- `SKILL_ROOT`：`/Users/bytedance/.trae-cn/skills/bits-code-guard`；`WORK_DIR`：本次审计的 `evidence`；本组产物仅为 `evidence/group/group_4.md` 与 `group_4.jsonl`，汇总及 HTML 由主审计流程负责。
- 已读 `AGENTS.md`、`CLAUDE.md`、`reviewer-brief.md`、分组列表、项目记忆、`analyzing-gef-code`，以及 bits-code-guard 通用流程、七维清单、分级规则。Rust 无额外语言专项。
- 契约 SSOT：`doc/design/architecture/connection-routing.md` 全文；覆盖状态核对：`doc/parity/draw2d/api-coverage.md:316-334`。本次不修改这些文档。

实际 Rust 阅读范围：

- 全文：`novadraw-scene/src/connection/{anchor,figure,locator,mod,query,router,runtime}.rs`。
- 全文：`novadraw-scene/tests/m9_connection_contract.rs`、`m9_connection_runtime.rs`。
- 共享 Runtime 的连接注册、binding、constraint、Locator、route 预检及提交、异常 guard、set_bounds、reparent、状态清理与稳定化调度；主要范围 `runtime/runtime.rs:501-974,1535-1650,1946-1970,3370-3610`。核对该文件当前 diff，相关职责不是已删除旧实现。
- 共享 Graph 的 topology admission/reparent、connection prepare/commit/clear、坐标父链；主要范围 `graph/mod.rs:1072-1101,1255-1320,1407-1453,2813-2915,3997-4028`；导出核对 `lib.rs:28-42`。
- Editor 只核对边界：`part/mod.rs:744-845,1038-1066`、`viewer/mod.rs:1928-1974,2144-2153,2190-2320`、`g5_connection_projection_contract.rs:141-162,456-502`。不扩展为 GEF Tool/CommandStack/Viewer 全量审计。
- 直接调用方检索覆盖 scene tests、Runtime、demo scenes 和 Editor；现有 route 调用方传入 `ChildContent(parent)` 或固定 connection layer。未将测试 fixture 的任意坐标域查询等同于 Runtime 提交契约。

官方参考根为 `/Users/bytedance/Documents/code/GitHub/gef-classic`。以下 `D/` 指 `org.eclipse.draw2d/src/org/eclipse/draw2d/`，`G/` 指 `org.eclipse.gef/src/org/eclipse/gef/`：

- 官方全文：`org.eclipse.draw2d.doc.isv/guide-src/connections.adoc`、`coordinates.adoc`。
- Java 全文：`D/Connection.java`、`ConnectionAnchor.java`、`ConnectionRouter.java`、`PolylineConnection.java`、`AbstractConnectionAnchor.java`、`AbstractRouter.java`。
- Java 全文：`D/{ChopboxAnchor,EllipseAnchor,RoundedRectangleAnchor,LabelAnchor,XYAnchor}.java`。
- Java 全文：`D/{BendpointConnectionRouter,RelativeBendpoint,AutomaticRouter,FanRouter,ManhattanConnectionRouter,ConnectionLayer}.java`。
- Java 全文：`D/{Locator,AbstractLocator,ConnectionLocator,MidpointLocator,ConnectionEndpointLocator,DelegatingLayout,ArrowLocator,ViewportAwareConnectionLayerClippingStrategy}.java`；`G/NodeEditPart.java`。
- 未使用 Zest、示例产品语义或 `doc/archive`；未读取昨日缺陷清单作为结论来源。

先确立的基线契约：连接是 Figure，但 route 而非外部 bounds 决定线路；Anchor reference/location 必须带明确坐标语义；Router 先产生线路，Locator 随后消费新线路；owner/ancestor 变化触发重新验证；更新必须处理旧、新可见区域。Java 的直接对象修改、listener 和共享可变缓存不是 Rust 必须复制的实现形式。严格 viewport chain 是 Novadraw 明确接受的收窄，不是 Java 完整 clipping 算法。

## 语义映射

以下“对齐”仅表示本次静态证据支持所述范围，不替代 suite 的运行状态。

### G4-M01：Connection 身份、端点与未解析状态

- Java：`D/Connection.java` 的 source/target API；`D/PolylineConnection.java:179-195` 只在两端存在时路由。
- Rust：`connection/runtime.rs:384-413,504-531,780-799`，`runtime/runtime.rs:626-643,909-914`。
- 状态：**合理迁移**。独立 `ConnectionId`、可选 `AnchorId`、Dirty/Resolved/Unresolved 由 Runtime 管理；缺端点有结构化原因，失败清空 path，非 Java nullable 对象网络。
- 证据/局限：`m9_connection_runtime.rs:403-480` 检查 missing target、重新绑定和旧路径清空；没有据此宣称所有 descendant decoration 的失败展示语义已验证。

### G4-M02：Anchor 坐标与规范 routing domain

- Java：官方 `coordinates.adoc` 的 connection 示例；`D/ConnectionAnchor.java#getLocation/getReferencePoint` 返回 absolute，`ConnectionRouter.NULL.route` 转到 connection 坐标。
- Rust：`CoordinateSpace` 区分 FigureLocal/ChildContent/LogicalSurface；`query.rs:350-378,432-494` 完成坐标映射及 inverse-transpose 法向；`runtime/runtime.rs:3496-3525` 从真实 parent 推导路由域。
- 状态：**合理迁移，有边界前提**。scene 查询可跨域；route 提交应固定在 parent child-content，随后规范化为 node-local。
- 证据/局限：纯查询测试 `m9_connection_contract.rs:188-233,420-435`；Runtime 的公开 `resolve_connection_route` 仍接收调用方坐标域，现有调用方均遵循规范域，本次不将其视为任意域提交已验证。跨父迁移另见 G4-F3。

### G4-M03：双向 reference 与 Direct Router

- Java：`D/AbstractRouter.java:51-71` 使用对端 reference；`ConnectionRouter.NULL.route` 生成两点。
- Rust：`router.rs:186-214,469-507` 的 `DirectRouter` / `resolve_endpoints`，`RouteOutput::new` 在 `112-141` 校验有限点、点数和端点 metadata 一致性。
- 状态：**对齐**。两次 Anchor 求值不依赖可变 singleton Point；未知 constraint 被显式拒绝。
- 证据/局限：`m9_connection_contract.rs:319-405,455-506`；不复制 Java 整数舍入、像素减一补偿。

### G4-M04：Connection Figure 几何与 custom geometry

- Java：官方 connections 的 route-driven bounds；`D/PolylineConnection.java:93-99` 合并 child bounds。
- Rust：`figure.rs:23-105,178-206` 提供 `PreparedConnectionGeometry` 和 `ConnectionFigureBehavior::prepare_route_geometry/commit_route_points`；`graph/mod.rs:2813-2873` 由 Runtime 提交 node bounds 与 local points。
- 状态：**合理迁移，扩展口确实存在**。path placement 与 child subtree envelope 分离，避免 label 改写路由坐标原点；内置 miter outset 不是只按半线宽计算。
- 证据/局限：`m9_connection_runtime.rs:202-279` 检查 route/bounds/hit/paint；`82-129` 检查无穷 stroke 拒绝。公开 prepared 类型目前只提供 `from_parent_points(points, outset)`，不能据此声称任意自定义路径载荷或独立非对称 bounds 协议已完备；未看到本组测试里的自定义 ConnectionFigure 实现。

### G4-M05：prepare / validate / atomic commit

- Java：`D/PolylineConnection.java:179-195` 先 route、后 child layout、再 bounds/damage；批量失败原子性是 Novadraw 额外契约。
- Rust：`connection/runtime.rs:583-648,851-937` 先纯计算；`runtime/runtime.rs:800-887` 为全批 geometry 和 Locator 预检；`889-905` 先提交 geometry/child bounds，再提交 generation/resolution。
- 状态：**主链已修复，失败恢复存在 G4-F1**。不会把 prepare 失败的线路提前标为 Resolved；已移除“缺少三阶段协议”旧结论。
- 证据/局限：`m9_connection_runtime.rs:82-129,908-998` 分别覆盖单连接 geometry 拒绝和共享 scope 路由失败。panic 由 `runtime/runtime.rs:1535-1582` fault Runtime 后继续 unwind，不承诺外部 capability panic 的事务回滚；这与可恢复 Result 失败分开评价。

### G4-M06：依赖失效与失败恢复

- Java：`D/AbstractConnectionAnchor.java#ancestorMoved/addAnchorListener/removeAnchorListener`；`D/PolylineConnection.java#anchorMoved/revalidate`。
- Rust：`query.rs:258-336` 跟踪 geometry/named region/relative transform/topology；`connection/runtime.rs:680-745,940-954,1065-1131` 失效、合并或替换依赖；稳定帧在 `runtime/runtime.rs:3489-3609` 消费。
- 状态：**listener 替换方式合理，覆盖不完整**。计算阶段失败会合并 observations，预检拒绝却只置 unresolved，见 G4-F1。
- 证据/局限：`m9_connection_runtime.rs:202-279,305-401` 覆盖显式 bounds 和 LayoutOutput 后 reroute；`query.rs:496-530` generation 是数据指纹，不是事件次数。直接 Figure mutation 对全部 RelativeTransform 保守失效，不能称为完全精确或严格 O(E)。

### G4-M07：五类 Anchor 与形状边界

- Java：`D/ChopboxAnchor.java:51-104`、`EllipseAnchor.java:46-72`、`RoundedRectangleAnchor.java:71-120`、`XYAnchor.java:43-78`。
- Rust：`anchor.rs` 的 XY、Chopbox、Ellipse、RoundedRectangle 和 Label；owner-local 求交，normal 经 SceneQuery 变换。
- 状态：**纯算法对齐/合理变体，形状自动供给有限**。XY 不原位修改，使用 replacement；Chopbox/Ellipse 以 f64 求交；圆角使用有界 64 次二分，不复刻 Java 分支及舍入。
- 证据/局限：`m9_connection_contract.rs:167-285,407-435`。圆角默认构造测试使用 QueryFixture 注入 RoundedRectangle geometry；真实 `FigureTreeSceneRead::anchor_geometry` 的 border-box 分支在 `query.rs:410-418` 返回 Rectangle，现有 demo 使用显式 corner dimensions。默认构造自动跟随真实 Figure corner 的集成不能按该 fixture 测试认定已验证。

### G4-M08：LabelAnchor 与 named geometry

- Java：`D/LabelAnchor.java#getBox` 取 Label icon bounds，而非整个 Label。
- Rust：`anchor.rs` 的 `LabelAnchor` 委托 `ChopboxAnchor::with_geometry(icon)`；`query.rs:292-307,410-425`、`runtime/runtime.rs:526-568` 提供查询和显式 region 更新。
- 状态：**合理迁移**。Anchor 不依赖具体 Label 类型；缺失 region 返回结构化错误并被 tracked query 记录。
- 证据/局限：`m9_connection_contract.rs:287-317,437-453`；本组只核对 named-region 输入边界，真实文本布局/icon 生成归 Group 3，不从该测试推断整套 Label Runtime 已通过。

### G4-M09：typed constraint、Router 替换与共享身份

- Java：`D/ConnectionRouter.java#getConstraint/setConstraint/invalidate/remove`；`PolylineConnection.java:242-271`。
- Rust：`router.rs:31-46,156-182`，`connection/runtime.rs:534-580,1005-1026`，`runtime/runtime.rs:703-714`。
- 状态：**合理迁移**。constraint 归 Connection；TypeId/Any 受检擦除；`set_route_configuration` 同步校验新 Router 与新 constraint 后一起赋值；共享 Router 由 Runtime namespace ID 识别。
- 证据/局限：`m9_connection_runtime.rs:483-544` 检查在用 Router/Anchor 禁删、类型不兼容保留 binding；新增自定义 constraint 不需扩展中心 enum，但当前 trait 没有域迁移协议，见 G4-F3。

### G4-M10：Bendpoint、reference 与 reparent

- Java：`D/BendpointConnectionRouter.java:63-94` 使用首尾 bendpoint 作为 Anchor reference；`D/RelativeBendpoint.java:67-78` 将两端 reference 转换后加 offset 加权。
- Rust：`router.rs:255-309,509-537` 保留有序 absolute/relative bendpoint 和 `[0,1]` 权重。
- 状态：**固定 parent 下对齐；跨 parent 不符合已接受契约**。`runtime/runtime.rs:1632-1649` 只迁移树并失效，未转换 constraint，见 G4-F3。
- 证据/局限：`m9_connection_runtime.rs:573-605` 校验两个 bendpoint 与 reference；没有 Connection 自身跨域 reparent 的验证，不能用 owner reparent/topology 测试替代。

### G4-M11：ConnectionLayer Router 默认值

- Java：`D/ConnectionLayer.java#add/remove/setConnectionRouter` 将默认 Router 应用于 connection children。
- Rust：`ConnectionLayerFigure` 是普通 Layer capability；`connection/runtime.rs:357-381,975-988` 提供 inherited/explicit binding、Direct fallback 及批量 constraint 预检。
- 状态：**显式 binding 的合理变体**。layer 更新不覆盖 explicit binding；默认 Router 不在 Figure 内维护第二棵关系树。
- 证据/局限：`m9_connection_runtime.rs:59-79` 仅验证没有 layer default 时的 Direct fallback。该静态对应不构成“Connection 移出 layer 后 inherited binding 自动重新归属”的验证。

### G4-M12：Fan 无向分组与稳定次序

- Java：`D/AutomaticRouter.java:47-60,162-194` 使用无向 anchor pair；`D/FanRouter.java:47-80` 统一几何方向后偏移。
- Rust：`connection/runtime.rs:801-849,957-965,1303-1308` 使用 semantic key 或 AnchorId、无向 pair、child order；`router.rs:392-440` 只处理两点 base route。
- 状态：**分组及扩展方式合理，偏移算法有 G4-F2**。居中 lane 分配可以不同于 Java index 顺序，但不能让反向同组连接重合。
- 证据/局限：`m9_connection_runtime.rs:639-683` 只有同向三条线和删除后 recenter；没有反向 pair。`FanRouter` 给 base 传 `group: None`，不能将任意要求 group 的 base Router 组合视为已支持。

### G4-M13：共享 Manhattan

- Java：`D/ManhattanConnectionRouter.java#route/processPositions/getRowNear/getColumnNear/removeReservedLines` 维护同一 Router 的 rows/cols，并消费端点方向。
- Rust：`router.rs:314-367,547-640`；`connection/runtime.rs:801-849` 的 `RoutingDomain`，每批基于先前计算结果重建 lane reservation。
- 状态：**scope 与无隐藏可变缓存迁移合理，不是 Java 完整算法等价**。Rust 用 normal 的 dominant axis 选择正交候选，再分配内部 lane；不提供 obstacle avoidance，也未复刻 Java 全部法向正负方向分支。
- 证据/局限：`m9_connection_runtime.rs:608-637,686-807,908-998` 覆盖正交、共享 lane、RouterId 隔离和组失败；不覆盖所有自定义 outward-normal、重叠 owner、近距离和极端坐标组合。

### G4-M14：Locator 纯语义及 Runtime 接入

- Java：`D/ConnectionLocator.java:120-147` 的 topological middle；`D/MidpointLocator.java:60-70` 的 indexed segment；`D/DelegatingLayout.java:50-58` 调用 child locator。
- Rust：`locator.rs:22-128,178-196`；`connection/runtime.rs:435-484`；`runtime/runtime.rs:716-745,826-905`。
- 状态：**已接入，不再是纯 helper**。source/target、奇数中央点、偶数中央段、indexed midpoint 保留命名语义；弧长比例用独立 `PathFractionLocator`。Runtime 校验 direct-child 并随 route 设置 child bounds。
- 证据/局限：`m9_connection_contract.rs:509-532`，`m9_connection_runtime.rs:132-199`。与 Java AbstractLocator 自动取 preferred size 不同，当前成功提交保持 child 现有尺寸并居中，这是 SSOT 明确的收窄；失败后自动恢复见 G4-F1。

### G4-M15：端点装饰、位置与方向

- Java：`D/ArrowLocator.java:45-56` 同时设置 location/reference；`D/ConnectionEndpointLocator.java:197-270` 使用 u/v 距离、象限与 preferred size。
- Rust：`LocatorPlacement` 返回 point/reference，但 `runtime/runtime.rs:843-884` 只校验 reference 有限并更新中心位置；`figure.rs:155-175,333-376` 的 inset 仅裁短 paint/hit 路径。
- 状态：**partial / 明确后置**。没有 Runtime rotatable-decoration orientation 消费，不能把 Source/Target locator 称为完整 ConnectionEndpointLocator 或 ArrowLocator。
- 证据/局限：`figure.rs` 内已有 inset 纯测试；parity `api-coverage.md:326` 也明确为 partial。route endpoint 保持 Anchor 真值是合理分离，不要求为箭头改写真值点。

### G4-M16：严格 viewport topology 与环拒绝

- Java：`D/ViewportAwareConnectionLayerClippingStrategy.java#getEdgeClippingRectangle` 根据 nearest common/enclosing viewport 裁剪；ownerless 情况不能推断端点容器可见性。
- Rust：`connection/runtime.rs:1239-1300` 比较 connection parent 与两端 owner 的有序完整 chain；`1310-1358` 拒绝 route 读取自身及 descendant geometry；`query.rs:390-394` 识别内置 ViewportFigure。
- 状态：**严格 topology 是已批准收窄**。同 chain 允许，分叉返回 `UnsupportedViewportTopology`，外层 Runtime 清空受影响 batch；ownerless anchor 不额外声明 owner viewport。
- 证据/局限：`m9_connection_runtime.rs:547-570,810-998`。nearest-common-viewport 多矩形裁剪是后置能力；这里检查的是前置拒绝和错误传播，不重新审计整个 renderer clipping。环检测证据限于自身/子树结构环，不宣称任意跨连接依赖图已证明无环。

### G4-M17：Editor Anchor descriptor 边界

- Java：`G/NodeEditPart.java#getSourceConnectionAnchor/getTargetConnectionAnchor` 允许根据 node model、connection model 选择 Anchor；Request overload 用于反馈。
- Rust：`part/mod.rs:744-747,833-842,1038-1058` 的 descriptor/context 与 source/target hook；`viewer/mod.rs:1928-1974,2220-2254` 构造、fallback、按端点 model + key 复用。
- 状态：**稳定连接边界已实现**。Editor 选择策略和稳定 key，Scene Runtime 负责注册、依赖与路由；不能再报告“所有 Anchor 强制 Chopbox、没有 descriptor”。
- 证据/局限：`g5_connection_projection_contract.rs:456-502` 验证同 key 复用、改 key 仅替换 source；key 必须反映应用策略变化，descriptor key 与 Anchor 自身 semantic group key 不是同一使用位置。本组不认定 Request-feedback 扩展、重连事务及所有模型错误路径已完成审计。

## 确认缺陷

### G4-F1：预检拒绝丢失恢复依赖

- 严重度：P1；类型：逻辑错误；置信度：10/10。
- 精确位置：`novadraw-scene/src/connection/runtime.rs:650-658`。
- 当前代码：`reject_route_batch` 对 calculations 只调用 `set_unresolved`，不读取每项已保存的 `observations`。与 `fail_route_batch:940-954` 的 `merge_dependencies` 不对称。
- 具体触发：在只有一条连接的 Runtime 中，两个不同 Rectangle owner 初始 bounds 完全重合；DirectRouter + 两个 ChopboxAnchor 会得到两个相同中心点。给该连接的直接 child 绑定 `PathFractionLocator::new(0.5)`，第一次 resolve 的 RouteOutput/geometry 合法，但 Locator 返回 `DegenerateRoute`，进入此拒绝路径。
- 结果：初始 state 的 dependencies 原本为空，拒绝后仍为空。仅通过 `Runtime::set_bounds(target, ...)` 将 target 移开，`invalidate_figure_change:706-745` 无索引可命中，`invalidate_stale_dependencies:680-704` 也无 observation 可比较；后续正常 `prepare_frame` 不会重路由，连接保持 Unresolved，尽管线路已可计算。显式再次 resolve 可以成功，但这不满足依赖修复后自动调度契约。
- 额外影响：已有成功 route 的情况下，预检拒绝还可能保留旧 generation；稳定化下一轮持续观察到同一 stale input，引发重复 reroute。这不是“提前提交 Resolved”的旧问题。
- 官方/项目契约：`PolylineConnection.anchorMoved/revalidate`；`connection-routing.md` 第 4.2 节明确要求失败保留旧依赖并合并本次读取。
- 建议：在整批 unresolved 转移中合并各 calculation 的 observations，并采用当前观测 generation 更新已读 subject；不提交 route generation、points 或 Locator 结果。增加“首次零长度 Locator 拒绝后移动 owner 自动恢复”及“成功后拒绝能稳定”的验证。
- 现有证据局限：`invalid_connection_geometry_never_commits_resolved_state` 只断言拒绝瞬间；`manhattan_scope_failure...` 走 route 计算失败，不走 geometry/Locator preflight 拒绝，均未覆盖本路径。

### G4-F2：反向 Fan 线路重合

- 严重度：P1；类型：逻辑错误；置信度：10/10。
- 精确位置：`novadraw-scene/src/connection/router.rs:428-436`；相关分组 `connection/runtime.rs:812-840,1303-1308`。
- 具体触发：同 parent、同 FanRouter，共享两个 XY Anchor，A=(0,0)、B=(100,0)，按 child order 注册 A→B、B→A 两条连接，separation=16。
- 静态代入：第一条 index=0，centered_index=-0.5，perpendicular=(0,1)，中点为 (50,-8)；第二条 index=1，centered_index=+0.5，perpendicular=(0,-1)，中点同样为 (50,-8)。
- 结果：两条完整折线仅点序相反，paint 与 hit geometry 完全重合，失去 Fan 分离双向连接的作用。无向分组本身正确，错误是偏移参考方向依赖每条连接自己的朝向。
- 官方证据：`AutomaticRouter.HashKey.equals` 显式认可反向 pair；`FanRouter.handleCollision` 先按 start/end 位置统一 ray 方向。可以保留 Novadraw 居中分配，不需要照搬 Java index 规则。
- 建议：对同组无向 pair 使用一致的几何方向/法向，再应用稳定 lane index；保留原 source/target 端点顺序和 metadata。增加双向两条、混合方向多条及删除后重排的断言。
- 现有证据局限：`fan_router_uses_stable_child_order_and_recenters_after_removal:639-683` 全部为相同方向，无法发现此错误。

### G4-F3：reparent 静默改变 absolute bendpoint 的坐标含义

- 严重度：P1；类型：业务语义问题；置信度：10/10。
- 精确位置：`novadraw-scene/src/runtime/runtime.rs:1638-1643`。
- 具体触发：root 下两个普通容器 A、B，child-content 到 surface 的变换分别为单位变换、平移 (100,0)，无 viewport 分叉。Connection 初始在 A 下，显式 BendpointConnectionRouter，constraint 为 `Absolute((50,30))`；两端 XY Anchor 固定在 root 域，保持不变。调用 `Runtime::try_reparent(connection.figure(), B)` 后准备正常帧。
- 调用链：`try_reparent_inner` 只校验 attachment/layered parent；Graph `try_reparent/apply_reparent_mutation` 只改变树；Runtime 随后 invalidate。`resolve_dirty_connection_routes:3496-3525` 改用 B 的 ChildContent，`calculate_route:894-907` 仍传原 constraint，`resolve_bendpoint:509-516` 原样返回 absolute 数值。
- 结果：端点重新映射后仍在原 surface 位置，但 bendpoint 从 surface (50,30) 跳到 (150,30)，不报错且可提交为 Resolved。严格 viewport 检查不会阻止两个同 chain 普通容器之间的迁移。
- 官方/项目契约：官方 coordinates 要求跨域转换，Java BendpointRouter 将 bendpoint 视为 connection 坐标；“迁移 constraint 或原子拒绝”是 `connection-routing.md` 第 6.1 节明确新增的 Novadraw 保证，不伪称 Java 自带 reparent 事务。
- 建议：在 topology commit 之前处理 connection routing-domain 变更；支持转换的 constraint 应生成新域值，暂不具备转换协议的自定义 constraint 应结构化原子拒绝。校验失败不得先改 parent、旧 route 或 binding。
- 现有证据局限：`bendpoint_router_preserves_absolute_and_relative_constraints:573-605` 未移动 Connection；`divergent_viewport_topology...:810-862` 移动的是 owner，不能证明 Connection constraint 域迁移正确。

## 合理迁移、收窄与后置

合理迁移：

- 短生命周期 `&mut dyn SceneQuery` 仅用于依赖记录，实际 scene 只读；避免 Java 长期 Figure 引用与 listener 网络。
- ID namespace、Runtime registry、Connection-owned constraint、Result 错误替代 Java object identity、Object cast 和 null。
- 全批纯输出、geometry/Locator preflight、最后统一提交，比照搬 Router 直接 `setPoints` 更适合 Rust。
- node-local path 与普通 child subtree envelope 分离；PathFraction 独立命名；Editor descriptor key 复用 AnchorId。

明确收窄：

- Core 1.0 只接受相同 viewport chain；拒绝分叉不是漏实现 Java multi-clip。
- Locator 是 direct-child、points-only 计算，保持现有 child 尺寸并居中，不是通用 `Locator.relocate(IFigure)` 的全部写能力。
- custom geometry 已有 prepare capability，但现有 prepared 类型以 point list + 单一 outset 表达；审计不授予任意曲线载荷、任意子布局和任意 Router pipeline 的完整等价结论。

明确后置：

- `ShortestPathConnectionRouter` / obstacle avoidance。
- nearest-common-viewport 多矩形 clipping。
- Runtime rotatable-decoration reference/orientation；完整 EndpointLocator u/v 参数产品能力仍不能由 Source/Target 枚举代替。

未证明的差异不自动视为“已批准收窄”：真实 rounded shape 自动供给、完整 Manhattan 方向分支、layer 移出后的 binding 归属、任意跨连接依赖环均只按上述映射中的证据边界记录，不纳入 verified 结论。本 JSONL 按 brief 只保留三条最明确的功能缺陷。

## 扩展性、复杂度与 Rust API

- 外部 Anchor、Router、Locator、constraint 均为公开扩展口，`connection/mod.rs` 与 crate `lib.rs:28-42` 有导出；不需要改中心枚举才能增加策略。Figure geometry 的 prepare 与 commit 分离已真实落地，但自定义实现仍须遵守纯计算和 infallible commit 契约。
- `RouteOutput` 私有字段 + 构造校验阻止少于两点、非有限点及 endpoint/metadata 不一致进入正常提交。`PreparedConnectionGeometry` 私有字段限制合法构造路径；无依据将外部扩展 panic 当成可恢复 Result 错误。
- 设连接数 E、总 Locator 数 L、组大小 K、路径点数 P、树深 h。单线路纯点计算/规范化一般 O(P)，坐标映射另计 O(h) 父链；PathFraction O(P) 时间和 O(P) 临时 lengths 空间，indexed/topological locator 为 O(1)。
- 每条线路调用 `locator_placements` 扫描全体 L，整批为 O(KL)。Fan 逐成员查 index 使组内该部分 O(K²)。Manhattan 每成员重新收集已算线路 lane，lane 冲突搜索还会叠加扫描；不能仅因策略无缓存就称为线性算法。
- `replace_dependencies/remove_reverse_dependencies` 已按单连接增量更新，未全量重建整张索引；但 reverse bucket 是 Vec，contains/retain 线性。同一 subject 被 E 条连接共享时，逐条更新该 bucket 仍可能二次扫描。
- `invalidate_figure_change/invalidate_subjects` 扫描 order，group expansion 又遍历候选与直接受影响列表；Figure 直接变化保守命中全部 RelativeTransform。新增 connection 和多种 binding setter 使用 `invalidate_all`，不能声称严格局部失效或总成本 O(E)。
- 以上为当前源码的复杂度界与性能风险，不是基准结果，也不是重报昨日 Editor 关系索引全量重建问题。本组未审计 Editor 全量索引算法。

## 七维检查与验证边界

| 维度 | 判断 |
| --- | --- |
| 逻辑 | G4-F1、G4-F2 有完全可描述的内置策略触发路径。 |
| 业务语义 | G4-F3 违反已批准 routing-domain 迁移契约；Locator/viewport 的明确收窄不伪装为缺陷。 |
| 并发 | 本组 Runtime mutation 持有 `&mut self`，query 生命周期受借用约束；未发现需依赖线程时序才能成立的缺陷，不把“atomic”误读为多线程原子指令。 |
| 健壮性 | 已核对有限点、metadata、类型不匹配、缺端点、generation checked_add、fault guard 和 preflight；不声称外部恶意 capability、极端 f64 或全局 panic 恢复已穷尽。 |
| 性能 | 已记录真实扫描和分组成本；未运行基准，不下经验性性能达标结论。 |
| 安全 | 本组核心路径无网络、文件、命令执行或鉴权入口，未发现可证实的安全漏洞；不外推为全项目安全审计。 |
| 质量 | 只评价 API 和职责边界，未将命名、函数长度或风格作为缺陷。 |

建议主审计后续验证三条触发路径，另外补充自定义 geometry 拒绝的共享批次、Locator 非有限 placement、正常帧失败恢复、非均匀变换 normal 和严格 viewport chain 的嵌套组合。本组仅提出验证点，未新增或修改测试。

交付检查：MD 包含 17 项映射；JSONL 仅包含 G4-F1 至 G4-F3，`file` 相对 Git 根使用 `engine/wasm_rust/novadraw/` 前缀。没有修改产品、测试、既有文档，也没有提交 Git。
