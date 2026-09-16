# Group 1: Figure 树、生命周期、坐标、输入与通知语义审计

日期：2026-09-15。范围：`full_file`，当前实现，不限 diff。只研究与报告，未修改实现或测试，未单独运行 cargo。

## 1. 结论与确认缺陷

### G1-F1 [P1] 鼠标进入、退出与悬停使用几何目标，丢失交互父节点事件

- 类型：条件性功能缺陷；业务语义问题；置信度 10/10。
- 主位置：`novadraw-scene/src/runtime/event/mod.rs:379-411`，特别是 384-405；同根因位置为 520-560。
- Draw2D：`SWTEventDispatcher.java:419-444` 根据 `mouseTarget` 变化发出 exit/enter；207-211 把 hover 发给 `mouseTarget`。`MouseMotionListener.java:31-59` 明确事件针对被监听对象。tooltip source 不等于鼠标回调目标。
- Rust 调用链：`SceneDispatchContext::{find_mouse_event_target_at,find_hover_source_at}`（`runtime/context.rs:460-474`）分别使用交互搜索与普通 `hit_test_simple`。`refresh_mouse_target` 根据 `hover_source` 发 enter/exit，之后才设置 captured-or-interactive `mouse_target`；主事件发往后者。
- 触发：交互 parent 覆盖无 handler 的 child。鼠标直接进入 child 区域，Moved 发给 parent，却没有 parent Entered；从 child 移到 parent 空白，交互 target 不变，却新增 parent Entered。反向移动还会提前退出 parent。
- 本次运行时证据：主 Agent 编译运行 `evidence/event_probe.rs`；已读取 `evidence/event-probe.log:3-5`：`interactive_parent_target: true`、`actual=[Moved]`、`actual=[Entered, Moved]`。这不是根据测试名称推断。
- 额外静态确认：`dispatch_mouse_hover` 使用 `ctx.hover_source()`，同一组合下 hover 也会丢给无 handler child；该 hover 分支未由本次 probe 单独执行。
- 现有测试：`m6_event_contract.rs:162-233` 的 probe 本身同时是几何与交互目标，未覆盖二者分离；capture 场景仅断言存在 Exited，未锁定完整次序。
- 建议：分离几何 cursor/tooltip source 与事件 mouse target。以交互 target 迁移驱动 enter/exit，hover 发给事件 target；capture 期间是否冻结边界事件应明确选择 Draw2D 契约并测试，不能用几何 hover 替代。
- 验收：进入装饰子节点、child 到 parent 空白、parent 内多个装饰 child、离开 parent、capture 拖出/拖回、每个回调的 local/entry 点；同一 mouse target 内移动不得产生额外 enter/exit。

### G1-F2 [P2] DispatchOutcome.capture 是清理前快照，可能返回已销毁的捕获对象

- 类型：条件性功能缺陷，影响当前公开结果契约；逻辑错误；置信度 9/10。当前直接调用方未证实因此发生编辑错误，按有限影响定 P2。
- 主位置：`novadraw-scene/src/runtime/runtime.rs:3131-3135`（`dispatch_inner`），结果生产位置 `runtime/event/mod.rs:464-484`。
- 契约：`DispatchOutcome::capture`（`runtime/event/mod.rs:295-297`）声明返回 dispatch 后 capture；Runtime 的 dispatch 注释同时承诺返回前提交 callback effects 和结构变更。
- 触发：child 的 `on_mouse_pressed` 调用 `ctx.remove_child_later(parent, ctx.target_id())` 并返回 true。dispatcher 先 capture child 并构造 outcome；Runtime 随后提交删除，`dispose_subtree` 清理真实 capture；最后原样返回旧 outcome。此时 `outcome.capture()==Some(child)`，`runtime.interaction().captured()==None`，child ID 已失效。
- 证据级别：静态完整路径确认，未运行此反例。`EventContext::remove_child_later` 在 `runtime/context.rs:197-199`；`dispose_subtree` 在 `runtime/runtime.rs:1414-1469`，`InteractionState::forget_figures` 在 `runtime/interaction.rs:45-48`。
- 直接调用方：Runtime 的鼠标包装方法在 `runtime/runtime.rs:2861-2904` 直接返回结果。Viewer 使用 handled 与自身读取的 Runtime 状态进行仲裁，不能把这个返回值缺陷扩大描述为“当前 Editor 必然误选”。
- 现有测试：`p2_dispatch_outcome_contract.rs:18-37` 只覆盖不删除 target 的正常 press；不能证明返回后的存活性。
- 建议：在 Runtime 事务结束后重建 outcome 的 capture 字段；target 可保留“曾收到事件的对象”历史事实，不能把历史 target 与当前 capture 混为同一存活承诺。
- 验收：按下时自删、删除祖先、重挂到 hidden parent；逐次比较 outcome.capture 与 Runtime 当前 capture，并验证历史 target 的语义。

JSONL 仅收录上述 2 项。disabled 普通搜索等设计差异列在下文，不混入代码违约缺陷。

## 2. 基线、路径与证据等级

- Git 根：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- 项目根：`/Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw`。下文 Rust 路径相对此根；JSONL 使用 Git 根相对路径。
- Java 根：`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/`。表中 Java 文件均相对此根，基线 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`，不使用 Zest。
- Novadraw：`6b83ac0` 加当前工作区。已读 Group 1 diff 和共享 facade/Viewer diff；连接 geometry 的旧实现已被新 prepared batch 替换，不报告旧实现问题。运行期间共享文件继续变动，引用按本报告写入时最新文件校准。
- 规范入口：ADR-014、ADR-015；`doc/design/architecture/{figure-lifecycle,tree-search-and-focus,dynamic-architecture}.md`；`doc/design/coordinates/coordinate-system.md`；Family ID 来自 `doc/parity/draw2d/api-coverage.md`。
- `S`：已读方法体和直接调用链的静态证据。`T`：已读测试断言，不等于本次执行。`R`：已读本次主 Agent 执行日志。`U`：提出的验收/反例尚未运行。
- 主 Agent 的 `evidence/checks.json` 记录 `cargo test --workspace` exit 0、`cargo check --workspace` exit 0；fmt exit 1、clippy exit 101。因此不能称“所有门禁通过”。表中 R 只引用明确读到的日志行，执行结果也不自动覆盖随后发生的共享文件修改。

写入时关键文件 Git blob：

| 文件 | Blob |
|---|---|
| `graph/search.rs` | `abfd5758f07f1b07661d94b5c849af0636f5a3e2` |
| `runtime/event/mod.rs` | `ee81d13b9e1226ed90cdc5fc08283b62d5331199` |
| `runtime/context.rs` | `d922d3b8cb872b3b90fddcf83210b291aea37645` |
| `runtime/runtime.rs` | `2b3416be3ce2d4676f8b12e72d3cfee615a33633` |
| `graph/mod.rs` | `1ced9ac95183d7e5e1d4cbbd9ee15ac2094ba310` |

上述缩写路径均位于 `novadraw-scene/src/`。若汇总时 blob 不同，应按方法名重新定位，而非机械沿用行号。

## 3. 方法级语义映射

表中 `G`=`novadraw-scene/src/graph/mod.rs`，`Q`=`novadraw-scene/src/graph/search.rs`，`F`=`novadraw-scene/src/figure/mod.rs`，`RT`=`novadraw-scene/src/runtime/runtime.rs`，`E`=`novadraw-scene/src/runtime/event/mod.rs`，`C`=`novadraw-scene/src/runtime/context.rs`。测试文件相对 `novadraw-scene/tests/`；`test.log` 位于本次 evidence 目录。

| Family ID | Java 类/方法与源码行 | 不可丢失语义 | Rust 公共入口与源码行 | 判定 | 测试与证据级别 |
|---|---|---|---|---|---|
| `figure.tree` | `IFigure.add:101-116`; `Figure.add:161-196` | 单父、有序 children；防环；重挂前维护旧关系 | `FigureTreeBuilder::try_add_child_to` G647-653；G1111-1175、1263-1313 | 等价核心，新增受检错误 | `d3_runtime_mutation::checked_topology_mutations_preserve_error_categories`，S/T |
| `figure.tree` | `Figure.getChildren/getChildrenRevIterable:654-665`; `findDescendantAtExcluding:393-414` | child order 决定正向组合与逆序命中 | `FigureTree::child_order/child_z_index` G2447-2484；Q262-269 | 等价 | `runtime_child_order_controls_paint_and_reverse_hit_order_atomically`，S/T/R `test.log:748` |
| `figure.tree` | `Figure.add:166-183` | index/constraint add 是一次操作 | Builder G639-676；`Runtime::move_child_to_index`；G2487-2509 | 部分；缺原子 indexed/constraint add，账本已后置 | `d3_runtime_mutation.rs:106-192` 覆盖 reorder，非 indexed add，S/T |
| `figure.tree` | `Figure.add:178-195`; `remove:1408-1424` | 迁移维护旧/新 parent，旧 constraint 解除 | `Runtime::try_reparent` RT1542-1587；G1407-1452 | 核心等价；ID 保持、bounds 不重写 | `reparent_preserves_identity_and_foreign_checked_mutation_is_rejected` 已读；深度拒绝测试抽读，S/T |
| `figure.tree` | `Figure.add:169-176` | 防环，不制造不一致关系 | G1263-1313；`MAX_TREE_DEPTH` G53 | 合理变体：额外 10,000 深度门禁 | `dispose_supports_the_maximum_tree_depth`，S/T/R `test.log:678`；add/reparent 边界断言 S/T |
| `runtime.identity` | `IFigure:32-39`; `LightweightSystem.getRootFigure:149-151` | 对象/所属图身份不可误命中 | `FigureId/RuntimeNamespace` `identity.rs:13-78`；arena get/remove 113-143；RT1836-1853 | 合理变体：namespace + generational slot，非业务 UUID | `foreign_figure_does_not_read_or_modify_matching_local_slot`、`foreign_listener_does_not_unregister_local_subscription`，S/T；后者 R `test.log:598` |
| `figure.lifecycle` | `Figure.addNotify:315-328` | 父已就绪后实现 realized 子树激活 | `FigureLifecycle::on_attached` F746-749；G712-721、734-744；RT3238-3242 | 合理变体：Runtime 接管时 parent-first activation | `test_figure_lifecycle_hooks_follow_runtime_realization_and_completion` 实现测试已读，S/T |
| `figure.lifecycle` | `Figure.remove/removeNotify:1408-1424,1536-1546` | 清理焦点与子树生命周期，失效引用不能继续使用 | `Runtime::dispose_subtree` RT1414-1469；G764-797、620-635；`EventContext::retired_focus_lost` C45-84 | 合理变体：commit/extract 后 descendant-first deactivate + Drop，不是 Java detach | `remove_releases_subtree_and_invalidates_old_ids`；`invalid_focus_owner_emits_one_lost_event_for_hide_disable_and_remove`，S/T；后者 R `test.log:615` |
| `figure.lifecycle` | `LightweightSystem.setContents:209-221` | contents replacement 清理旧显示子树 | `Runtime::try_set_contents/set_contents_inner` RT907-948；G903-915 | 合理变体：新 Box、新 ID，成功销毁旧子树；无活对象迁移 | `contents_replacement_releases_old_tree_and_preserves_namespace`、`runtime_resize_contract.rs:60-74`，S/T |
| `figure.lifecycle` | `Figure.removeNotify:1540-1546` | 生命周期必须完成后才能继续使用场景 | `Runtime::guarded` RT1471-1480；`FigureLifecycle` F746-757 | 合理变体：panic 后 faulted，不承诺回滚任意扩展副作用 | `lifecycle_panic_happens_after_extraction_and_blocks_recording`、`d4_component_update.rs:299-396`，S/T |
| `figure.geometry.bounds` | `Figure.getBounds:638-648`; `setBounds:1663-1700` | bounds 变更比较 old/new；一次变更驱动失效与重绘 | `NodeState::bounds` G252-254；`Runtime::set_bounds` RT1883-1906；G3838-3933 | 合理变体：NodeState 是运行期真源，不可改 Java 式返回引用 | `m4_coordinate_root_move_and_resize_is_one_atomic_bounds_change`，S/T/R `test.log:866` |
| `figure.geometry.bounds` | `Figure.translate:2036-2039`; `setBounds:1675-1700` | parent 移动必须改变后代最终位置 | `Runtime::translate`；G3780-3812、3997-4019 | 合理变体：只改 parent-local bounds，通过父链改变投影 | `m4_coordinate_contract.rs:214-280`、`bounds_test::test_prim_translate_propagates`，S/T |
| `figure.box.client_area` | `Figure.getClientArea:672-686`; `findDescendantAtExcluding:393-410` | insets 同时影响内容原点及下降裁剪 | G485-505；`FigureTree::insets` G2941-2943；F230-237 | 合理变体：node-local border/client 与 child-content 分域 | `m4_point_roundtrips_across_nested_coordinate_roots_with_insets`，S/T/R `test.log:864` |
| `coordinate.conversion` | `Figure.translateToParent/FromParent:2045-2050,2068-2073` | 相邻坐标域互逆 | `FigureTree::translate_to_parent/from_parent` G3965-3982 | 合理变体：本 API 只映射 placement；parent insets/scroll 属于另一条 edge | M4 嵌套 Insets 往返断言；不能按同名方法直接迁移 Java 调用，S/T |
| `coordinate.conversion` | `Figure.translateToAbsolute/Relative:2055-2062,2078-2085` | 完整父链转换，顺序不可交换 | `local_to_surface_transform/translate_to_relative` G3985-4030 | 合理变体：f64 affine；失败返回 false/None | `m4_point_roundtrips...`、`m4_rectangle_roundtrip...`，S/T/R `test.log:863-864` |
| `coordinate.conversion` | `Figure.isCoordinateSystem:1131-1133` | 坐标根影响 children，而非绕过树 | `FigureContainer::child_transform` F639-642；`ChildTransform` F62-111 | 合理变体：所有边 parent-local，额外 affine 为 capability；无模式开关 | `singular_child_transform_has_no_inverse_mapping` F940-947，S/T；任意非交换 affine 的完整 UI 回归 U |
| `clipping.strategy` | `Figure.findDescendantAtExcluding:393-414` | client clip 约束子树查询 | `hit_test_with` Q232-283；F117-126 | 默认核心等价；OverflowVisible 为显式变体 | `test_hit_test_descends_only_through_parent_client_area`，S/T/R `test.log:479`；Freeform 细节交 Group 2 |
| `hit_test.search` | `TreeSearch.accept/prune:23-38`; `Figure.findFigureAt:437-453` | prune 排整枝，accept 只选自己；子优先、逆 Z | `TreeSearch/TreeSearchContext` Q10-48；`hit_test_with` Q112-120、232-283 | 核心等价；策略只读，不可绕开 engine 门禁 | `hit_test_checks_deepest_candidate_before_parent_acceptance`，S/T/R `test.log:460`；`d1_tree_search_contract.rs:14-58` |
| `hit_test.search` | `Figure.findFigureAtExcluding:458-460` | exclusion 不止排除单一结果，应 prune 整枝 | `ExclusionSearch/hit_test_excluding` Q60-75、122-128 | 等价 | `exclusion_search_prunes_the_complete_topmost_subtree` Q328-342，S/T |
| `figure.tree` | `Figure.getParent/getChildren`; `AncestorHelper.addAncestors:70-74` | 稳定 parent-chain 与后代查询 | `ancestor_ids/descendant_ids/is_ancestor_of/find_in_subtree` Q141-210 | 合理变体：显式 ID 集合，隐藏 synthetic root；unknown 与 no-match 区分 | `structural_queries_have_stable_order_and_hide_the_synthetic_root` Q345-363，S/T |
| `figure.visibility.enabled` | `Figure.findFigureAt:437-453` 与 `findMouseEventTargetInDescendantsAt:500-518` | generic geometry 与事件资格不同；generic 不排 disabled | `hit_test_simple/hit_test_with` Q112-136、242-244 | 设计级收窄，不是实现违背窄规范，见 D1 | R `event-probe.log:2`；`tree-search-and-focus.md` 明确 enabled 门禁 |
| `figure.visibility.enabled` | `Figure.isVisible/isEnabled/isShowing:1139-1141,1195-1197,1226-1228` | local flag 与祖先可见性有区别 | `is_visible/is_enabled` G2922-2935；`is_effectively_visible/enabled` G3457-3464；G4100-4118 | 部分等价；Rust enabled 也施加祖先硬门禁 | `m2_product_existence.rs:212-258`，S/T；不能推导 disabled 不参加布局 |
| `hit_test.search` | `Figure.isMouseEventTarget:1159-1162`; `findMouseEventTargetAt:476-488` | 无监听器的 child 不阻挡交互 ancestor | `find_mouse_event_target_at` Q136-139；`MouseEventTargetSearch` Q94-103 | 交互 target 搜索等价，边界事件有 F1 缺口 | S；R `event-probe.log:3-5` |
| `event.point_reduction` | `MouseEvent:34-49` | callback 点必须处于 source 的坐标域 | `MouseEvent::entry_point/with_target_point` E56-63；C564-646 | 合理变体：node-local 点 + 不变 logical-surface entry 点 | `m4_hit_test_and_mouse_callback_share_the_same_target_coordinate_domain`，S/T/R `test.log:865` |
| `event.dispatcher` | `SWTEventDispatcher.dispatchMousePressed/Released:263-270,292-299` | consumed press 自动 capture；release 发给 capture 后释放 | `Runtime::dispatch_mouse_pressed/released` RT2872-2904；E464-504 | 主路径等价；公开最终 capture 结果有 F2 缺口 | `handled_press_reports_target_and_capture_to_the_caller`，S/T/R `test.log:973`；自删 U |
| `event.dispatcher` | `SWTEventDispatcher.receive:419-444`; `dispatchMouseHover:207-211` | enter/exit/hover 跟随事件目标，不是 tooltip 来源 | E379-411、520-560；C460-474 | 缺口 F1 | R `event-probe.log:3-5`；hover 仅 S |
| `event.input_listeners` | `MouseMotionListener.mouseDragged:24-30`; `SWTEventDispatcher.dispatchMouseMoved:277-285` | 按钮按住即 drag，不要求 press 被消费 | `dispatch_mouse_moved` E506-517；`MouseEvent` E27-35 | 部分：Rust 以 capture 判 drag，未携带 button mask/modifiers | `m6_event_contract.rs:162-233` 只覆盖 handled press；未消费 press 后 drag U |
| `event.input_listeners` | `IFigure.addMouseListener/addKeyListener:151-183` | 独立监听器可附着到现成 Figure | `Figure::event_handler` F498-500；`FigureEventHandler` F666-735 | 部分：单 capability handler；不是通用多 listener 注册 | S；可用外部组合实现 handler，但需应用维护列表 |
| `event.input_listeners` | `SWTEventDispatcher.dispatchMouseWheelScrolled:305-311` | 坐标适配后交给事件处理对象 | `Runtime::dispatch_scroll/zoom`；E575-619；C391-454 | 合理变体：gesture session 与 pointer capture 分离，typed controller fallback | `continuous_scroll_keeps_its_target_and_does_not_follow_pointer_capture`，S/T/R `test.log:889` |
| `event.focus` | `Figure.requestFocus:1600-1610`; `isFocusTraversable:1147-1149` | 直接焦点与遍历资格独立，键盘发给 focus owner | `request_focus/clear_focus` RT3019-3040；G3476-3495；E626-655 | 合理变体：attached/visible/enabled 再验证，结构化失败 | `public_runtime_focus_api_separates_direct_and_traversal_eligibility`，S/T/R `test.log:685` |
| `event.focus` | `SWTEventDispatcher.setFocus:561-576` | 先提交 owner，再 lost -> gained；重复同 owner 无事件 | E413-440；C633-646 | 等价核心 | `focus_transition_commits_owner_before_lost_and_gained_events`，S/T/R `test.log:573` |
| `event.focus` | `FocusTraverseManager.getNext/PreviousFocusableFigure:57-204` | 稳定前序/逆前序，边界交平台 | `TreeOrderFocusTraversal::traverse` `runtime/focus.rs:64-96`；RT3053-3082 | 部分；无 current 时 backward 取末项为接受变体；contents 自身被遗漏见 D4 | `tree_order_policy_is_stable_bidirectional_and_has_boundaries`，S/T/R `test.log:584` |
| `event.focus` | `Figure.removeNotify:1540-1546`; `SWTEventDispatcher.requestRemoveFocus:466-479` | 删除/失效不留焦点对象引用 | `dispose_subtree` RT1414-1469；`retain_interactive_figures` RT3317-3345 | 核心等价；额外 hide/disable 失效 lost | `invalid_focus_owner_emits_one_lost_event_for_hide_disable_and_remove`、`reparent_under_hidden_ancestor_releases_focus`，S/T/R `test.log:615,619` |
| `notification.figure` | `Figure.fireFigureMoved:538-547` | 位置或尺寸改变均发 moved | `Runtime::add_figure_listener` RT2428-2442；G3838-3867；`FigureEvent` `update/listener.rs:73-94` | 合理变体：typed old/new，稳定期外部观察 | M4 atomic bounds test；`d3_runtime_listener.rs:83-148`，S/T/R `test.log:740` |
| `notification.coordinate` | `CoordinateListener:23-31`; `Figure.fireCoordinateSystemChanged:529-534` | 子坐标映射改变需区别于普通 moved | `Runtime::add_coordinate_listener` RT2444-2458；G880-890、3868-3874 | 部分：已有事件类别；自定义 transform 更新自动通知闭环未证明 | M4 parent move 测试 S/T/R；自定义 component transform 变化 U |
| `notification.ancestor` | `AncestorHelper:70-74,88-105,147-162`; `AncestorListener:17-50` | base 所在祖先链发生增删/移动可观察 | `Runtime::add_ancestor_listener` RT2460-2474；G1200-1252、3814-3830 | 部分：移动按 descendant 记录；增删只有被移动根的边事件，见 D3 | `d3_runtime_listener.rs:83-148` 只计数，非叶节点 ancestor 语义证明，S/T |
| `notification.property` | `Figure.firePropertyChange:574-614`; `setParent:1928-1932` | old/new 属性事实，支持按属性观察 | RT2476-2493；`PropertyChangeEvent` `update/listener.rs:125-131`；G3389-3454 | 部分：typed 值、调用方过滤；parent 变更用 AncestorEvent 而非 parent property | `listener_self_removal_finishes_current_effect_and_skips_later_effects`，S/T/R `test.log:739` |
| `notification.layout_update` | `Figure.addLayoutListener:263-279`；ADR-014 接受的观察边界 | 生命周期行为不可被延迟 observer 替代 | RT2511-2544；`ObservationListener/NotificationRecord` `update/listener.rs:181-238`；`update/deferred.rs:255-360` | 合理变体：epoch/sequence + latest stable query；不是事前拦截 hook | `historical_records_keep_event_order_while_queries_read_latest_stable_scene` `d4_notification_epoch.rs:73-192`，S/T |
| `figure.lifecycle` | `Figure.remove*Listener:1442-1567` | 注销不再调用；对象清理不能误删共享订阅 | `add_*_listener_scoped/remove_listener` RT2412-2564；`update/deferred.rs:103-145,233-251` | 合理变体：scope 是所有权，不是事件过滤器；namespace 隔离 | `disposal_removes_owned_listener_but_preserves_shared_registrations`，S/T/R `test.log:596` |
| `figure.properties` | `Figure.getBackgroundColor:621-630`; `SWTEventDispatcher.updateHoverSource:650-676` | 属性继承、tooltip 追溯祖先 | `FigureStyle/ResolvedStyle` `style.rs:19-85`；G2955-2982；RT3347-3357 | 合理变体：逐字段继承、tooltip 三态抑制；仅字符串 tooltip 为收窄 | 已读 style 单测与 tooltip controller deadline 测试，S/T；文本绘制交 Group 3 |
| `event.dispatcher` | `LightweightSystem.addListeners:86-114`; `controlResized:117-129` | 宿主仅桥接，通用 Figure 机制不放 app | `PlatformHost` `host/scene_host.rs:16-26`；`Runtime::resize_logical_viewport` RT2665-2683；`novadraw-apps/src/input.rs:22-174,248-288` | 边界合理；平台重入/多 pointer/焦点恢复仍部分，见 D5 | `logical_viewport_resizes_contents_without_rewriting_child_world_coordinates`，S/T/R `test.log:982` |
| `accessibility.bridge` | `LightweightSystem.EventHandler:379-519`（本次只定位桥接入口） | AT 不能维护另一份 Figure/focus 真源 | `AccessibleFigure` F760-784；`runtime/accessibility.rs:205-367`；PlatformHost 25 | 合理变体：engine snapshot/delta；本组只确认 identity、tree/focus 投影 | 实现 S；完整 AT 测试与原生 provider 验收交 Group 3，不据此宣布可访问性完整 |

## 4. 非缺陷清单中的语义差异与扩展限制

### D1. disabled generic 搜索是明确的设计级语义收窄

`Q242-244` 无条件 prune disabled 子树，导致 generic hit、cursor、tooltip 都不能查询 disabled 可见 Figure。Draw2D generic `Figure.findFigureAt:437-453` 没有 enabled 门禁；事件下降才在 `Figure.java:512` 检查 enabled。

`tree-search-and-focus.md` 的硬门禁明确写 effective enabled，因此实现符合较窄规范。`event-probe.log:2` 已确认 disabled child 返回 parent。不能以“测试通过”称完全 parity，也不能把它写成实现违反设计。迁移 disabled 说明 tooltip、禁用状态检查器、Viewer 几何选取时，需要先批准查询契约 delta：共享遍历内核，但把 enabled 过滤交给 event/focus 策略，保留 generic geometry 查询。

### D2. 生命周期只部分保留 Draw2D 的活对象语义

- Java remove 是从显示树脱离，外部持有对象仍可再 add；Rust `remove_figure` 为 dispose，ID 与对象一起失效。ADR-014 已明确撤回通用 detach/跨 Runtime 自动迁移，按后置能力记录，不报 bug。
- reparent 当前对被移动根调用 detach/attach（RT1569-1587），不对所有 descendant 重放 realized 生命周期。Java `removeNotify/addNotify` 会递归子树。若扩展只依赖“直接 parent 变动”可接受；若依赖祖先级服务绑定则不等价，需要明确 reparent hook 与 scope，而非宣称全部生命周期 parity。
- `FigureLifecycleContext`（F740-744）只有 figure ID、parent ID、namespace；没有旧/新关系只读查询，也没有排队 mutation 端口。不能据 ADR 文本宣称组件已能读取任意旧关系快照或请求结构变更。
- `on_detached` 的源码注释仍写“移除前”，实际 dispose 在提取后执行。报告以实际时序和 ADR-014 为准。

### D3. listener 所有权、过滤与祖先事件不是同一件事

`ListenerScope::Figure(owner)` 只控制销毁时清理，`dispatch_effects` 会把相应类别的所有事件发给所有存活订阅，不能当成 Java `figure.addListener` 的目标过滤。Runtime 全局订阅可以自行过滤 typed payload，但 API 文档应明确区分 owner 和 subject。

Ancestor moved 按 descendant 生成；add/remove/reparent 的 Added/Removed 只记录被移动根的 parent edge。Draw2D `AncestorHelper` 在 base 及整条 parent chain 注册，可直接通知深层叶子祖先增删；Rust 的现有计数测试不证明该语义。外部观察者可自己维护拓扑，但 `StableSceneQuery` 仅公开 bounds/attached，不能靠删除后的当前树恢复全部历史关系。建议设计 subject filter 或带 affected subtree/旧新祖先链的事实，不把同步内部失效委托给外部 listener。

### D4. 焦点遍历和平台焦点不完整

`TreeOrderFocusTraversal` 从 `descendant_ids(scope)` 开始，漏掉 contents 自身；Runtime 又把 contents 作为 scope。Draw2D 的 traversal root 是 LWS synthetic root，其第一个孩子 contents 可以获取焦点。因此 `set_contents(ButtonFigure)` 的独立按钮画布不能通过默认 Tab 获得焦点。静态可定位，现有测试只把可聚焦节点放在 contents 之下；本次未运行该反例，作为需补验收的语义缺口保留，不占高价值缺陷清单。

此外，Java `SWTEventDispatcher.dispatchFocusGained:138-150` 恢复 previous focus owner 或首候选，失焦时仅清当前 owner。Rust 没有对等 host-focus-gained/记忆 owner 入口；Native 失焦 clear，Web blur clear；重新进入不自动恢复。backward + no current 选择末项则是窄规范明确接受的增强，不与上述缺口混淆。

### D5. 输入表达力和取消边界

- `MouseEvent` 不携带 modifiers、按住按钮集合、pointer identity；`PointerId` 虽可构造，dispatcher 仍统一 PRIMARY。不能据 `InteractionState` 中存在 HashMap 就声称多指输入完成。
- Dragged 由 capture 推导而非 button mask，未处理 press 后仍可能需要 drag 的组合。Java MouseMotionListener 语义比当前更宽；按已支持输入面定义后续 delta，不能通过 app 自建通用状态机补洞。
- `cancel_gestures` 只清 scroll/zoom session；`pointer_exited` 保留 pointer capture；`release_focus` 只变 focus。Native `Focused(false)` 把三者串起来仍没有明确 pointer-cancel/pressed 清理入口。保留 capture 以支持平台抓取不等于覆盖焦点丢失/取消。未运行 OS 失焦与窗外释放测试，不能声称已证明卡住；建议 engine 提供明确 cancel 契约，再由平台传取消事实。
- Native/Web Tab 适配在 `novadraw-apps/src/input.rs:61-77`；Web 只在 `Moved` preventDefault，保留 boundary；Native 直接忽略返回值。平台窗口能否自然移出需要集成验收，不能由 headless traversal 通过代替。
- 双击/hover 有 Runtime 入口，但本次读取的 apps 没有完整接入所有等价平台事件；按 adapter 覆盖不足记录，不扩展为 app 产品功能审计。

### D6. Rust 扩展与错误模型

- `Box<dyn Figure>`、只读树视图、短生命周期 `EventContext` 避免拓扑与回调的可变别名；核心 capability object-safe，第三方无需加入 Runtime 类型枚举才能参与命中/事件。
- typed `FigureComponentUpdate` 通过关联类型表达 prepare/commit（`runtime/mutation/mod.rs:8-20`），它本身不是可直接存储的 dyn 队列对象；当前 `PendingMutationKind` 没有 component update variant，EventContext 也没有对应入口。外部组件直接更新已覆盖，不等于 ADR 的“callback 可排队 owned component update”已覆盖。
- prepared 默认保守 layout/geometry/paint 失效是合理选择。`&self` 不证明无 interior mutation，commit/Drop 不 panic 仍是扩展契约。
- `try_*` topology 返回 foreign/disposed/topology 等错误；部分 convenience 返回 null/false、部分坐标方法失败静默 no-op，易混淆“无变化”和“失败”。有限性校验也不是统一的：size override/viewport resize 受检，普通 `set_bounds` 和 `initial_bounds` 接受源值时未做同等校验。未据异常输入猜测崩溃，只列防御与契约一致性待办。
- 任意 local affine mutation、RTL mirroring、任意 clipping provider、活对象迁移、原生 AT provider 不能由当前具备相邻类型或方法推断已支持；文本、渲染、布局策略本体由其他组负责。

## 5. 平台与 app 边界结论

已读 Native 共用应用输入分支、Web validation 的 pointer/key/wheel/blur 分支、node-editor-demo 的输入转发。Figure hit-test、source/target 降域、dispatch、focus owner、gesture controller 均位于 `novadraw-scene`；EditorDomain/Viewer 管选择与 Tool；app 只把平台位置/按键/修饰符映射后转发，未发现这几类通用机制被另写到 app。

允许 app 构造模型策略、反馈视觉和按需快捷键，不把这些直接定性为引擎机制下沉。发现的 adapter 覆盖限制见 D5；未评审示例完整产品交互，未复核 Web Canvas2D 或 Vello 渲染实现。

## 6. 七维度检查

| 维度 | 本组结论 |
|---|---|
| 逻辑 | 交互目标与几何目标错配；结果快照早于清理；已分别收录 F1/F2 |
| 领域语义 | disabled generic、生命周期活对象、ancestor/listener、焦点 scope、输入表达力均单独映射，不因 Rust 迁移自动豁免 |
| 安全 | 本组未证实可利用漏洞；ID namespace/generation 有拒绝路径；不把 UI 状态缺口误标为安全问题 |
| 并发 | Runtime 独占借用、callback deferred effect 避免普通拓扑重入；未证实跨线程 race；`RefCell/Mutex` 的存在不替代业务语义证明 |
| 健壮性 | disposed/foreign/depth/Faulted 有测试；bounds/入口输入的统一有限性校验、cancel/失焦边界需补验收 |
| 性能 | 搜索每次多条 target 查询、焦点遍历创建完整 descendant Vec；未测基准，不列性能缺陷，不建议改受保护渲染主循环 |
| 质量 | 个别注释/账本 API 与实现时序或 public 面不一致，作为文档校准项，不把风格问题写入 JSONL |

## 7. 建议依赖顺序与验收

1. 先固定 F1 的 mouse target / cursor / tooltip / capture 状态转换契约，用本次 probe 反例补齐回归；不需要先改坐标、渲染或业务 demo。
2. 修正 F2 结果发布时间，保留历史 target、返回最终 capture；由 Group 6 复核 Viewer/Tool 仲裁调用方，不混入 ModelId/EditPartId 到 scene。
3. 单独批准 disabled generic search 的设计 delta，再改共享遍历策略。验收 generic 能看到 disabled Figure，而 event/focus 仍拒绝它；同时覆盖 tooltip/cursor 与 Editor geometry 查询。
4. 焦点专项验收 contents 自身、正反向首次进入、边界移出、失焦再进入、按住鼠标失焦/取消；新增公共机制在 engine，平台只传事实。
5. 生命周期/通知扩展再定义 reparent descendants、owner/subject、旧新祖先链和 callback component update。验收有非空状态的外部 Figure，不以空 hook、计数增加或 trait 可编译代替契约通过。

上述为后续建议，本组没有实施。未实现但已排期的能力不计作现存实现 bug。

## 8. 实际阅读清单与范围限制

Java 方法体及 Javadoc：

- `IFigure.java`：Figure 定义、add/监听注册契约。
- `Figure.java`：add/remove、notify、bounds、client area、坐标、hit/event-target、属性/可见性/focus 相关完整方法。
- `TreeSearch.java`、`FocusTraverseManager.java`、`MouseEvent.java`、`MouseMotionListener.java`：全文。
- `SWTEventDispatcher.java`：focus/key/traversal、mouse/hover/wheel、receive/capture、target/cursor/tooltip 相关完整方法。
- `AncestorHelper.java`、`AncestorListener.java`、`CoordinateListener.java`、`FigureListener.java`：全文。
- `LightweightSystem.java`：listeners、resize、contents、dispatcher 注入完整方法；AT EventHandler 只定位入口，不作为完整 AT 契约证据。

Rust 实现与共享边界：

- `novadraw-scene/src/graph/{mod.rs,search.rs}`：本组树/几何/搜索/通知完整相关方法；layout/route/render 仅关联边界，不宣称全文业务审计。
- `novadraw-scene/src/{identity.rs,style.rs,lib.rs,log.rs}`；`host/{mod.rs,scene_host.rs}`。
- `novadraw-scene/src/runtime/{mod.rs,context.rs,interaction.rs,focus.rs,event/mod.rs,mutation/mod.rs}`；`runtime.rs` 的 topology/lifecycle/mutation/focus/dispatch/listener 相关完整方法。
- `novadraw-scene/src/runtime/{tooltip.rs,accessibility.rs}`：状态/identity/树投影；tooltip 文本与 AT 产品闭环不在本组。
- 补充读取 `novadraw-scene/src/runtime/update/{listener.rs,deferred.rs}` 的通知定义、scope 清理和 flush；更新算法归 Group 2。
- `novadraw-scene/src/figure/mod.rs`：核心 Figure、container、event、lifecycle、component 共享能力；不检查图元绘制算法。
- `novadraw/src/lib.rs`、`novadraw-editor/src/lib.rs` 及 Viewer diff/输入直接调用方；Editor 其他逻辑归 Group 5/6。
- `novadraw-apps/src/{lib.rs,prelude.rs,verification.rs,input.rs,platform.rs,app.rs}`：公开边界和平台输入方法；后两大文件只读相关入口及分支。
- `apps/native/node-editor-demo/src/main.rs`：diff 与输入转发；`apps/web/web-validation/src/lib.rs`：pointer/key/wheel/blur 接入，排除文本/渲染。

测试实际证据：

- 已读 `d1_focus_contract.rs`、`d1_tree_search_contract.rs`、`d3_runtime_listener.rs`、`d3_runtime_mutation.rs`、`d4_component_update.rs`、`d4_notification_epoch.rs`、`m4_coordinate_contract.rs`、`m6_event_contract.rs`、`p2_dispatch_outcome_contract.rs`、`runtime_resize_contract.rs` 的断言与 fixture。
- `d4_constrained_measurement.rs` 只用于确认外部组件与 bounds/arrange 共享边界；不对文本正确性出结论。
- `m2_product_existence.rs`、`graph/bounds_test.rs`、`graph/update_integration_test.rs` 的树、bounds、clip、depth、FIFO 相关断言及内嵌 runtime lifecycle/focus 测试已读；后两文件采用相关片段抽读，不能称全文测试审计。
- 初期部分 bounds 测试只检查命令数量或非空，并不证明坐标正确；报告优先使用 M4 的明确数值断言与本次 event probe。
- 已读 `evidence/event_probe.rs`、`event-probe.log`、`checks.json`，并核对本组相关 `test.log` 成功行。后续外部修改应重新跑相应门禁，本报告不借用旧审计“通过”结论。
