# Group 2：布局、更新、视口与自由坐标语义审计

日期：2026-09-15。范围：`scope: full_file`，以 `evidence/review_groups.md` Group 2 为准。

## 1. 结论与证据边界

不能确认 Group 2 已完整等价于 Draw2D。本组保留 3 项高价值确定缺陷：布局输出缺少几何预验证、Freeform 旧位置 damage 被裁掉、BorderLayout 为缺席区域预留空间。六类布局均有实现，但算法存在不等价分支，不能以“六类已存在”替代 preferred/minimum/constraints 的完整性结论。

- Novadraw 基线：`6b83ac0fa980dc60fd1284607a51460858fa883b` 加工作区修改。
- Draw2D 基线：任务书指定 `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`；参考范围仅 `org.eclipse.draw2d/src` 与 `org.eclipse.gef/src`，未使用 Zest，也未扫描 `doc/archive`。
- 本组完整读取 Group 2 的 20 个生产文件和 4 个契约测试文件；共享大文件按有关函数、直接调用方、类型定义读取。读取了本组及共享边界 diff；本组布局、容器和 update 文件未出现待审 diff，但本次明确审计存量实现。
- 审计期间共享 `runtime/runtime.rs` 因外部 Connection/Locator 修改增加行数，已重读其 diff 及本组调用方；下文共享 Runtime 行号采用此次重读版本。未把已替换的旧 route 提交逻辑作为缺陷。
- **S**：此次静态源码/Javadoc 对照确认。**T**：此次实际读取已有测试的断言，但没有执行。**U**：提出的验收场景，尚未执行。
- 协调方提供 workspace 结果“644 通过、2 忽略”，并说明 5 个 connection 相关文件发生外部变化、最终将重跑必要门禁。本组没有运行 cargo，没有将该结果或旧 `verified` 标签推导为本报告 U 场景通过。
- 只写 `group_2.md`、`group_2.jsonl`；不修改源码、测试、规则、账本和路线图。使用 `bits-code-guard` 分组审计流程及 `analyzing-gef-code` 方法，不生成总报告或重复运行主 Agent 门禁。

路径缩写：`J` = `/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/`；`S` = `novadraw-scene/src/`；`T` = `novadraw-scene/tests/`。Family ID 沿用 `doc/parity/draw2d/api-coverage.md`，没有新建 family。

## 2. 确认缺陷

### G2-01 [P1，置信度 10/10] LayoutOutput 在提交前没有校验几何值

类型：条件性功能缺陷；健壮性问题。位置：`S/graph/mod.rs:1998-2002`。

- 契约：ADR-014 决策 1/2 要求候选几何经校验后原子提交；`J/LayoutManager.java:52-62` 定义布局执行职责。Java 整数几何不能作为 Rust 浮点 NaN/Infinity 可接受的依据。
- 触发路径：`Runtime::set_layout_constraint` 接受 `XYConstraint::at_size(0, 0, INFINITY, 10)`，因为 `XYLayout::validate_constraint` 只检查类型（`S/layout/xy_layout.rs:124-137`）。扩展布局也可通过公开 `LayoutOutput::set_child_bounds` 产生同样输出。
- `validate_layout_output` 将 `Bounds(child, _)` 的矩形丢弃，只验证 direct-child 身份；随后 `apply_layout_output` 调用 `set_bounds_with_update`，后者及 `set_bounds` 均不校验有限值（`S/graph/mod.rs:2038-2058,3834-3872,3887-3933`）。因此非法矩形能写入 NodeState；该问题不是调用者均满足前置条件时的纯防御建议。
- 最小 U 场景：普通容器两个 children，输出先移动 A，再给 B 写入无限宽度；预期返回 `NonFiniteGeometry` 且 A/B、damage、通知均不变，当前提交路径会写入两个结果。改用合法负 origin 应成功，显式零尺寸应成功。
- 已有 T：`m5_layout_contract::layout_output_is_validated_before_any_change_is_committed` 只检查错误 child ID；`wrong_constraint_type_is_reported_and_invalid_work_is_preserved` 只检查错类型。二者都没有覆盖合法 ID + 非法几何。
- 建议：在提交任何 change 前，对整个 LayoutOutput 检查坐标/尺寸有限性、非负尺寸及必要的边界算术溢出；两条提交路径共用该校验。内置约束构造/接受策略也应拒绝非法数值。保留合法负坐标，不以 clamp 掩盖错误。

### G2-02 [P1，置信度 10/10] Freeform 子节点移动时旧位置 damage 丢失

类型：条件性功能缺陷；业务语义问题。位置：`S/runtime/update/repair.rs:116-119`。

- 契约：`J/Figure.java:375-383,1675-1702` 在 bounds 修改前 erase，`J/DeferredUpdateManager.java:272-307` 合并修复旧区域。Novadraw 不再扩大 Freeform bounds，故 `doc/design/architecture/layer-and-freeform.md` 要求 OverflowVisible 在 paint/hit/damage 三条路径保持一致。
- `set_bounds_with_update -> erase` 把旧 child visual bounds 转成 parent-local dirty region，登记在 parent ID 下（`S/graph/mod.rs:3907-3920,3935-3955`）。repair 对每个 dirty source 无条件先交 `current.visual_bounds()`；只在随后祖先步骤中考虑 OverflowVisible（`repair.rs:141-146`）。
- FreeformLayerFigure 未覆盖 `Figure::visual_bounds_in`，所以其自身 visual bounds 仍是有限 presentation box（`S/figure/mod.rs:447-450`）。已转移到 Freeform parent 的旧区域因此在进入祖先传播前被裁掉；新 child repaint 则可以穿过 OverflowVisible。这是 old/new 不对称。
- 最小 U 场景：稳定 root `400x300`；Freeform host bounds `(100,100,20,20)`；child 从 `(-40,0,10,10)` 平移至 `(-20,0,10,10)`，尺寸不变，无 layout manager、无 viewport clamp。旧 surface 区域为 `(60,100,10,10)`，新区域为 `(80,100,10,10)`。旧区域与 host local box 不相交而被丢弃，当前 partial damage 只覆盖新位置。
- Runtime 的 `set_bounds_inner` 只对顶层 contents 的移动强制 Full（`S/runtime/runtime.rs:1887-1905`）；这个嵌套移动不会触发该补偿。保留式后端会收到不完整 damage；全量重绘可以掩盖视觉问题，但不能证明 damage 元数据正确。
- 已有 T：`d2_freeform_contract::overflow_visible_damage_is_not_clipped_to_host_bounds` 仅对 child 原位 repaint；`freeform_extent_shrink_clamps_origin_and_repaints_viewport` 又有 viewport repaint 掩盖旧区丢失，且只断言 damage 非空。
- 建议：区分 self repaint 与 descendants 的 erase contribution，或在源几何改变前冻结旧区域至有效祖先/表面域。不能简单扩大所有 self damage，也不能用整幅 Full 长期代替正确传播。验收还应覆盖隐藏、reorder、不同 scale 下旧区，以及 viewport 裁剪。

### G2-03 [P1，置信度 10/10] BorderLayout 为不存在的边区扣减中心空间

类型：核心布局功能缺陷，影响限定于该布局；业务语义问题。位置：`S/layout/border_layout.rs:238-241`。

- `J/BorderLayout.java:178-226` 仅在对应 child 存在且 visible 时消耗该边区域；中心获得剩余 client area。
- Rust 从四个默认 `50` 开始计算区域尺寸，有 child 才覆盖，但无 child 不清零；计算 center 时始终减去全部四边（`:254-263`）。
- 最小 U 场景：`300x200` 容器，`BorderLayout::new()`，唯一 child 使用 `BorderConstraint::new(Center)`。Draw2D 结果为 `(0,0,300,200)`；当前算法为 `(50,50,200,100)`。preferred/minimum 的 `measure` 又只累计存在的 child，造成 measure/arrange 自身不一致。
- 已有 T：本文件测试仅覆盖构造和区域字符串；`m5_layout_contract` 使用 BorderConstraint 制造 XY 错类型，没有断言 BorderLayout 实际五区分配。
- 建议：从存在且参与布局的区域构造 region set，统一用于 measure/arrange；无对应 child 的边区占用为零。默认尺寸只能应用到真实成员；明确重复 region、隐藏成员和无约束 child 的策略。验收 center-only、各单边、删除/隐藏边区和五区组合。

## 3. 详细语义映射

表中“部分/缺口”不是“已排期”的同义词；未找到明确排期的差异不擅自承诺后置。测试名称均为 T，不表示本组运行通过。

| Family ID | Java 类/方法及源码行 | 不可丢失语义 | Rust 公共入口及源码行 | 判定与差异 | 测试及证据 |
|---|---|---|---|---|---|
| `layout.manager` | `J/LayoutManager.java:21-79`，get/setConstraint、remove | constraint 属于 parent layout 与 direct child 关系 | `LayoutConstraint`、`LayoutSnapshot::constraint_as`，`S/layout/mod.rs:30-46,111-127`；Runtime set/remove，`:1630-1689` | 合理变体：Any + checked downcast，Runtime 写入校验，树保存 parent-owned map；构建期仍可产生错类型 | T `wrong_constraint_type_is_reported_and_invalid_work_is_preserved`；S |
| `layout.manager` | `J/LayoutManager.java:50-55`，invalidate；`AbstractLayout.java:126-141` | 丢弃缓存；再次测量不得复用旧约束结果 | `LayoutManager::invalidate`，`S/layout/mod.rs:352`；`LayoutState::invalidate`，`S/graph/mod.rs:383-395` | 主干等价：generation + hints cache，类型可 override invalidate；不能沿用旧账本“未来才有 invalidate hook”描述 | T `layout_measurements_are_cached_until_generation_changes`；S |
| `layout.manager` | `J/Figure.java:865-910`，preferred/minimum override | 显式尺寸优先，之后 layout，最后 intrinsic | `FigureTree::{preferred_size,minimum_size}`，`S/graph/mod.rs:2155-2181,2220-2242` | 主干合理变体：Option 表达显式零；minimum 不自动追随 explicit preferred 的所有 Java fallback 行为 | T `explicit_zero_size_is_not_treated_as_a_missing_measurement`；S |
| `layout.manager` | `J/StackLayout.java:33-80`，preferred/minimum；`XYLayout.java:41-69` | 扣除父 insets 后向 child 传 hints，测量加回 insets，并考虑 border preferred | `StackLayout`，`S/layout/stack_layout.rs:14-56`；tree 测量 `S/graph/mod.rs:2155-2250` | 部分：manager 分支直接返回/project，不调用 owner-scoped border helper；snapshot 无 insets/border 查询。内置 layout 未普遍补齐，非零边框不能视为等价 | T stack 用例为零 insets；U bordered nested layout |
| `layout.manager` | `J/XYLayout.java:41-69,93-119` | 无 constraint 忽略；`-1` 轴用带 constraint hints 的 preferred；位置范围 union | `XYLayout`，`S/layout/xy_layout.rs:81-115,161-210` | 部分：忽略无约束等价；所有负尺寸当 fallback；measure 用父 hints、arrange 用 `(-1,-1)`，未将指定宽度传给自动高度测量；负 origin 的 measure 只取右/下最大值，不做完整矩形 union | T wrong-constraint；U 固定宽度+自动高度、负 origin |
| `layout.manager` | `J/StackLayout.java:33-91` | 最大 child preferred/min，全部 children 同 client 区域，Z-order 不变 | `StackLayout`，`S/layout/stack_layout.rs:14-70` | 零 insets 主干等价；缺 `observeVisibility` 可选策略；snapshot 的 client area 坐标见后续说明 | T `stack_layout_places_every_child_in_the_client_area`；S |
| `layout.manager` | `J/BorderLayout.java:53-226` | 仅存在/可见区域占空间，依次传剩余 hints | `BorderLayout`，`S/layout/border_layout.rs:125-174,220-346` | 缺口：G2-03；此外强制半区 clamp、无约束自动填空、重复 region 全部 arrange，均不是 Java 的 region replacement 语义 | U center-only；T 构造测试不覆盖算法 |
| `layout.manager` | `J/BorderLayout.java:248-299`，setConstraint | 重设 child region，后来设置的 child 替代旧 region 成员 | `BorderConstraint`、`validate_constraint`，`S/layout/border_layout.rs:44-60,183-198` | 部分：强类型有收益；兼容 Rectangle 从 width/height 符号推 region，不能当 Java Rectangle 布局契约；缺区域级提交验证 | S；U 重复 region、重新分配、remove |
| `layout.manager` | `J/GridLayout.java:192-219,238-305,308-473` | 列行占位、span、grab；不足空间能压缩；equal-width 分配后仍相等 | `GridConstraint`、`GridLayout`，`S/layout/grid_layout.rs:20-193,199-298,339-450` | 部分：span/正额外空间已有；`distribute_extra` 遇负 extra 直接返回；equal width 后仅给 grab 列增加，会再次不等宽；span 的 excess 分配规则也不同 | T `grid_layout_uses_track_maxima_and_fill_alignment`、`grid_layout_honors_column_span_and_excess_space`；U deficit/mixed grab |
| `layout.manager` | `J/GridLayout.java:192-219,475-519` | preferred 响应 hints，列宽确定后按实际宽度重测 wrapping child 高度 | `GridLayout::{get_preferred_size,get_minimum_size,layout}`，`S/layout/grid_layout.rs:319-339,359-407` | 部分：父 hints 被忽略，只有 constraint hints 进入首次测量；没有列宽后的第二次测量 | T Grid 用例固定 intrinsic；U wrapping child |
| `layout.manager` | `J/FlowLayout.java:128-194,308-419` | 按方向保留一个 hint，换行；行内及整体对齐、minor stretch | `FlowLayout`，`S/layout/flow_layout.rs:70-103,143-207,214-244` | 部分：默认换行与横纵轴已有；measure 传两轴 hints，arrange 只传一轴；缺 align/stretch 选项；Java minorSpacing 对应 Rust spacing，默认数值不同 | T 本文件创建测试仅空 children；U 非空两方向及 hints |
| `layout.manager` | `J/ToolbarLayout.java:92-191,280-389` | main 按 preferred-min 容量压缩；minor 不低于 min；传 minor hint | `ToolbarLayout`，`S/layout/toolbar_layout.rs:82-106,135-207` | 部分：比例压缩主干一致；arrange 总用 unbounded；不 stretch 时 `natural_minor.min(available)` 可低于 min；缺 matchWidth 等选项 | T `toolbar_layout_compresses_main_axis_and_stretches_minor_axis`；U non-stretch/min、wrapped child |
| `layout.manager` | `J/StackLayout.java:88-91` 仅作对照 | 区分“全部 stack”与“只填第一个” | `FillLayout`，`S/layout/fill_layout.rs:30-84` | Novadraw 附加策略，不计入六类 Draw2D 等价；其他 children 保持原位 | S；U 非首 child 保持不变 |
| `layout.manager` | `J/LayoutManager.java:57-62`；ADR-014 候选提交 | 自定义 layout 只能提交合法结果，失败不写半批 | `LayoutOutput`、`LayoutManager::layout`，`S/layout/mod.rs:238-258,345-350`；tree `:1991-2130` | 部分：归属检查及 Result 丢弃输出已有；G2-01 几何验证缺口；测量 tuple 无错误出口 | T `layout_output_is_validated_before_any_change_is_committed`；S/U G2-01 |
| `validation.protocol` | `J/Figure.java:1111-1125,1615-1623,2174-2182` | 失效缓存、向上请求、父先子后 validate | tree `mark_invalid`、`perform_validation_cycle`，`S/graph/mod.rs:1526-1529,1664-1843` | 合理变体：一般父先子后，Viewport 可声明 child-first；隐藏分支保留 stale，disabled 不阻断；节点成功后才标 valid | T `non_converging_validation_returns_diagnostic_and_keeps_work_queued`；S |
| `validation.protocol` | `J/DeferredUpdateManager.java:198-217` | 当前轮新增 invalid 仍处理；防重入 | tree `:1671-1714`，update `S/runtime/update/deferred.rs:553-621` | 合理变体：预算错误、保留 invalid；Runtime 路径错误后补回所有 invalid；低层 `perform_validation` 与完整 phase 的恢复/通知契约不同 | T non-converging、`test_update_panic_restores_manager_state_and_requeues_invalid_graph_nodes`；S |
| `update_manager.two_phase` | `J/UpdateManager.java:21-43`；`DeferredUpdateManager.java:172-192` | validation 先于 repair；repair 冻结 dirty snapshot，后续工作留待下一轮 | `UpdateManager::perform_update`，`S/runtime/update/deferred.rs:517-617` | 主干等价；Runtime 独占事务，Host 调度替代 SWT Display；`runWithUpdate` 的任意 after-update Runnable 未一一提供 | T `test_take_dirty_snapshot_freezes_current_cycle`、`test_validation_figure_effects_preserve_causal_order`；S |
| `frame.preparation` | `J/DeferredUpdateManager.java:172-217,272-307` | 不在未稳定布局上绘制 | `Runtime::stabilize/prepare_submission_state`，`S/runtime/runtime.rs:3448-3659` | 合理变体：typed worklist + Ready/Idle/Suspended/AwaitingCompletion/Error；失败不创建 submission，panic 进入 faulted；并非任意副作用可回滚 | S；U 外部 layout 错误的完整 frame |
| `damage.repaint` | `J/DeferredUpdateManager.java:90-105,272-307` | 同 source 合并、坐标链投影、旧新范围修复 | `UpdateManager::add_dirty_region`，`:391-395`；`S/runtime/update/repair.rs:34-156` | 普通父链主干等价；多 region 是保守变体；Freeform erase 缺口见 G2-02 | T `test_parent_chain_propagation_intersects_clip_regions`、`test_write_damage_set_preserves_regions_and_union`；S/U G2-02 |
| `notification.layout_update` | `J/UpdateManager.java:116-126` | validating/painting 与几何事件职责分离 | `LayoutEvent/UpdateEvent/ObservationListener`，`S/runtime/update/listener.rs:49-75,150-235`；deferred `:254-360` | 已接受变体：稳定后回放阶段事实，不是 before-hook；删除/重入能力不同；不能把 RangeListener 同步通知混作 stable observation | T `test_validation_figure_effects_preserve_causal_order`；S |
| `viewport.scroll_zoom` | `J/RangeModel.java:17-141`；`DefaultRangeModel.java:118-207` | min/max/extent/value，value clamp，通知顺序 | `RangeModel`、`DefaultRangeModel`，`S/container/range_model.rs:85-127,147-275` | 合理变体：f64、输入拒绝、extent 规范化，先原子更新再解锁通知；Java 可保留超 span extent，故快照值并非完全相同 | T `range_model_clamps_extent_and_value_atomically`、`range_model_rejects_invalid_input_without_partial_state` |
| `viewport.scroll_zoom` | `J/Viewport.java:79-139,274-279,308-313` | contents 与两轴模型，模型可替换及订阅 | `ViewportHandle`，`S/container/viewport.rs:195-224,300-317,671-728` | 部分/限制：单 contents 强制；公共 handle 仅给 range snapshot，with_models 为 crate-private，外部 RangeModel 无法注入运行中 viewport | T `viewport_handle_replaces_contents_without_leaving_two_children`、`viewport_rejects_a_second_contents_child_atomically`；S |
| `viewport.scroll_zoom` | `J/ViewportLayout.java:33-77,104-140` | tracks 轴才传 hint，min 决定可压缩下限；非 tracks 保持自然尺寸 | `ViewportLayout`，`S/container/viewport.rs:385-410,489-535` | 部分：arrange 不论 tracks 均传 area 的两轴 hints；preferred 未先用 min 修正 hints；minimum 固定零未加 insets | T `viewport_track_width_uses_available_width_until_content_minimum` 只覆盖固定尺寸；U hint-sensitive contents |
| `viewport.scroll_zoom` | `J/Viewport.java:186-215,323-360` | scroll 更新 origin/坐标系统/paint，value 唯一真源 | `ViewportHandle::set_view_location`、Figure child_transform，`S/container/viewport.rs:226-295,592-600` | 主干合理变体：双轴先有限检查、range clamp，产生 viewLocation/coordinate/repaint；范围提交 effect 为内置 sealed 能力 | T `viewport_handle_scroll_clamps_and_repaints_the_viewport`；S |
| `viewport.scroll_zoom` | `J/ScrollPaneLayout.java:57-115`；`ScrollPaneSolver.java:59-121` | scrollbar policy、tracks/min 共同决定 viewport 与条显隐 | `ScrollPaneLayout`，`S/container/scroll_pane.rs:640-738` | 部分：固定 preferred 的两轮条显隐解算；没有 Java solver 的 guaranteed-size、track-axis minimum 修正；测量未先扣条宽 hints | T automatic/visibility/resize 三用例；U tracks + min 小于 preferred |
| `viewport.scroll_zoom` | `J/ScrollPaneLayout.java:108-119`，page increment | step、page、drag 与共享 range 联动 | `ScrollBarFigure`，`S/container/scroll_pane.rs:511-575` | 部分：press step、thumb 比例映射已有；page 用 max(extent,step)，Java 用 max(extent-step,step)；无公开 increments 配置 | T `vertical_scroll_bar_step_updates_shared_viewport_model`、`vertical_scroll_bar_thumb_drag_updates_shared_viewport_model_continuously`；U page |
| `viewport.scroll_zoom` | `J/RangeModel.java:17-24`；项目 scroll 输入契约 | logical pixels 与 content units 不能混同 | `ScrollPaneFigure::scroll_model`，`S/container/scroll_pane.rs:213-224` | 部分：Lines 与 LogicalPixels 有区分，但直接写 range；freeform scale 不为 1 时 range 为未缩放单位，需要单位换算；嵌套余量不是 bool 能完整表达 | T `touchpad_pixel_scroll_uses_logical_distance_without_line_multiplier` 的普通 viewport；U scaled freeform |
| `viewport.scroll_zoom` | `J/ScalableFigure.java:23-37`；`ScalableLayeredPane.java:115-124` | scale 可读写、尺寸/变换一致，并发起失效 | `ScalableFigure/ScaleHandle`，`S/container/scalable.rs:92-142,498-517` | 部分：有限正 scale、失效/repaint 已有；ScalableFigure 只有 read，ScaleHandle factory downcast 两个内置类型，自定义实现不能接 ZoomManager 写协议 | T `scalable_layered_pane_rejects_invalid_scale_without_state_change`、`scalable_projects_explicit_unscaled_preferred_size_through_scale`；S |
| `viewport.scroll_zoom` | `J/zoom/AbstractZoomManager.java:348-355,389-396`；`IZoomScrollPolicy.java:20-40` | policy location -> scale -> validate -> scroll；可替换策略 | `ZoomManager::prim_set_zoom_at`，`S/container/zoom.rs:210-265` | 主干顺序一致；policy 输入改为 owned snapshot、freeform 有单位转换；Result 失败原子性不能仅由这段顺序证明 | T `pinch_zoom_keeps_content_point_under_the_entry_anchor`、freeform zoom；S/U 错误路径 |
| `viewport.scroll_zoom` | `J/zoom/DefaultScrollPolicy.java:32-37`；`MouseLocationZoomScrollPolicy.java:26-67` | 明确默认与鼠标锚点公式，不只按类名比较 | `DefaultScrollPolicy/MouseLocationZoomScrollPolicy`，`S/container/zoom.rs:30-90` | 默认算法非逐值等价：Java default 为 oldLocation + center*(ratio-1)，Rust 为 (oldLocation+center)*ratio-center；old origin 非零时不同。Rust 鼠标公式与 Java 鼠标策略一致 | T default 测试起始 origin=0；U 非零 origin 对照 |
| `viewport.scroll_zoom` | `J/zoom/AbstractZoomManager.java:415-431,500-508` | zoom levels、fit 后落到 range minimum；fit 可绕普通 min 限制 | `ZoomManager::{set_zoom_levels,fit_all,fit_width,fit_height}`，`S/container/zoom.rs:174-186,310-401` | 合理变体：levels 严格递增校验，fit 绕 min 可与 Java 对应；扩展 fit contribution 与 zoom listener 未完整提供 | T `zoom_manager_uses_configured_levels_for_step_zoom`、`freeform_fit_uses_derived_extent_instead_of_presentation_bounds` |
| `layer.freeform` | `J/Layer.java:30-66` | 透明 layer 只返回 descendants；opaque 可改变 Java self hit | `LayerFigure`，`S/container/layer.rs:129-180` | 已接受变体：DescendantsOnly 与 opaque 解耦，显式 background/custom layer 代替；不应宣称 opaque 分支也等价 | T `transparent_layer_returns_descendant_but_never_itself`；S |
| `layer.freeform` | `J/LayeredPane.java:30-90,112-154` | keys 与唯一 child/Z-order 同步，before/after | `LayerKey/LayeredPaneHandle`，`S/container/layer.rs:12-44,84-127,369-426`；Runtime `:1053-1230` | 合理变体：唯一非空字符串 key、双向索引不复制顺序；reparent 保身份，remove 是 dispose 而非 Java detach；失败返回结构化错误 | T duplicate、reverse-z、reparent/remove、callback FIFO 四类用例；S |
| `layer.freeform` | `J/FreeformHelper.java:35-59,71-108` | 缓存子范围，nested freeform 用 extent 而非 presentation bounds；失效向上 | `FigureTree::freeform_extent`、`recompute_freeform_extent`，`S/graph/mod.rs:1565-1641` | 已接受变体：content-domain Rect、dirty generation、查询不 lazy mutate；空为 ZERO，host insets 不混入；不是 Java parent-domain 返回值逐值等价 | T empty、positive/negative、nested、old-stable、visibility；S |
| `layer.freeform` | `J/FreeformLayout.java:46-62,70-100`；`XYLayout.java:41-69` | 默认保留负坐标；可选 positiveCoordinates；测量 prospective constraint | `FreeformConstraint/FreeformLayout`，`S/layout/freeform_layout.rs:16-51,93-193` | 默认坐标合理变体，Option 避免 sentinel；无约束 child 计 current bounds 不同于 Java XY 忽略；固定宽+自动高未传宽 hint；positive 模式明确不交付 | T `freeform_layout_preserves_negative_origin_and_uses_intrinsic_fallback`、invalid constraint；S |
| `layer.freeform` | `J/FreeformViewport.java:26-31,68-81` | extent 与原点 viewport baseline union，范围能扩也能缩 | `ViewportLayout` freeform branch，`S/container/viewport.rs:422-486` | 主干合理变体：不 setFreeformBounds，range 为 content-domain；child-first 校验后消费 extent。preferred 路径仍通用 contents preferred，未直接消费 freeform extent | T `freeform_viewport_range_uses_unscaled_content_domain`、shrink/clamp；U 无 layout 的 freeform contents + automatic scrollbar |
| `layer.freeform` | `J/ScalableFreeformLayeredPane.java:24,104-112` | scalable、freeform、layer 能力同节点组合 | `ScalableFreeformLayeredPane`，`S/container/scalable.rs:418-495` | 合理变体：一个 scale state、统一 child transform；普通 ScalableLayeredPaneFigure 并无 Layer/Layered capability，不能因名称视作 Java 同类完全移植 | T freeform zoom、fit；S |

## 4. 扩展性与错误路径

### 4.1 已有合理 Rust 边界

`Box<dyn LayoutManager>` 是对象安全策略，`LayoutConstraint: Any` 的泛型 downcast 位于 snapshot，不破坏 manager object safety。parent-owned constraint map 与 namespaced FigureId 避免 Java 活对象别名关系；Runtime 写入与 immutable snapshot/output 分离避免 mutable tree 重入。`layout()` 返回 Err 时 output 被丢弃，manager 在 unwind 前放回原槽；Runtime guard 的 faulted 语义不能与独立 UpdateManager 的可重试测试混为一谈。

### 4.2 不能豁免的限制

- **测量与盒模型**：`LayoutContext::get_container_bounds` 返回 node-local `client_area`（含 left/top），但注释称 child domain；`FigureNode::child_transform` 又加 left/top。Stack/XY/Flow 等直接使用 area.x/y 作为 child bounds，非零 insets 存在重复 origin 的静态差异。加上 manager 测量缺少 insets/border metadata，需以统一盒模型契约修正，不能要求每个外部 layout 猜父坐标。
- **扩展测量错误**：preferred/minimum 返回裸 tuple，`FigureMeasurement` 也是公开裸数值。没有统一 Result/受校验 candidate；G2-01 只是最终 arrange 提交的确定缺陷。`d4_constrained_measurement` 证明一个手写 ConstrainedColumnLayout 能成功测量/复用 glyph IR，不证明内置六算法传播 hints 正确，更不证明非法 measurement 会被拒绝。
- **扩展私有派生快照**：LayoutOutput 只公开 bounds/visibility/invalidate，没有扩展布局私有快照的 prepared/commit 端口。`layout(&mut self)` 可以在错误前修改 manager 内部状态；当前只能要求实现者自行保证纯计算，不能把“树输出丢弃”描述成“所有派生状态回滚”。ADR-014 要求的候选发布能力仍需真实扩展用例确认。
- **可见性策略**：snapshot 不公开 child visibility。BorderLayout 的 Java 固有 visible 过滤以及 AbstractLayout 的可选 observeVisibility 不能仅靠现有公开 snapshot 实现；这是真实策略能力差异，不是借用检查器必然要求。
- **容器替换性**：Layer/Freeform marker 可由第三方实现；ViewportHandle 和 ScaleHandle 获取依赖具体类型 downcast。公开 RangeModel trait 不代表可注入 Viewport；公开 ScalableFigure trait 不代表可接入 ZoomManager。需区分“能实现 trait”与“能够接入现有运行时”。
- **Zoom 错误语义**：`zoom.rs:255-262` 先提交 scale，再 fallible validate/set_view_location。自定义 layout 返回 Err，或自定义 policy 返回非有限 point 时，没有候选 scale/origin 的两阶段提交；Runtime 又映射为 `Rejected`。现有无效 zoom 测试均在提交前拒绝，不能证明后半段失败原子性。与 ADR-014“已接受 source 不回滚”及手势契约“任一步失败不提交部分 scale/origin”必须明确划分；本组不将它描述为已排期或已解决。
- **Range 通知**：独立 DefaultRangeModel 的 mutex 内状态更新、解锁后回调是合理并发保护；不同线程的 listener 到达顺序并不由该锁覆盖。当前 Viewport/ScrollPane 使用私有默认模型，不能假设外部 RangeListener 已能挂到它们并据此报告必现死锁。ViewportLayoutEffect 顺序 set 两轴，仅默认且预规范化模型能支持当前成功路径，不能外推至任意第三方模型。
- **ScrollPane 派生状态**：`ScrollPaneLayout::layout` 末尾直接写共享 pane/viewport bounds（`:734-736`），并非 LayoutOutput candidate；错误 child 拒绝后的共享状态一致性尚无本组测试证据。该实现与 Viewport sealed effect 的预提交模式不同。

### 4.3 已排期与显式不交付

这些不是本次新增 bug：

- Draw2D `FreeformLayout::positiveCoordinates`、公开递归 `setFreeformBounds` 已由 Layer/Freeform 设计明确不交付；迁移应使用导入/导出归一化与 extent 查询，不能强行恢复 bounds 双重含义。
- 滚动条按住连发随 P2 widget repeat scheduler 延后，来源为当前语义账本 M8 备注；单次 step 通过不能冒充 repeat 已实现。
- `performUpdate(exposed)` overload 在账本中明确作为扩展；当前 full redraw/dirty API 不是同名兼容承诺。
- 完整 TextFlow、图布局等不属于本组六类容器布局；已后置能力不纳入当前缺陷数量。
- 本报告列出的 Grid deficit/hints、Border region、盒模型、第三方 scalable/range 接入和 zoom 错误语义，不能因为已有 Core 1.0 标签自动归入上述后置清单；须由主 Agent 明确裁定修复或登记独立 delta。

## 5. 迁移顺序与验收

1. **先候选结果门禁**：修复 G2-01，并统一 measurement/bounds 的有限数值契约；验收合法 ID 的非法第二项、负 origin、零尺寸、溢出、错误后 pending work 和 frame 状态。不要用更宽泛的 catch-unwind 代替预验证。
2. **再旧几何修复**：修复 G2-02，冻结旧可见区域且沿同一 clip/transform 链传播；验收普通/Freeform/Viewport、scale=0.5/2、移动/隐藏/reorder，断言 old 与 new 的具体 damage 覆盖，不只断言非空。使用可保留 partial 的后端或命令/region 断言，避免 Full 掩盖。
3. **然后盒模型与六算法**：明确 snapshot area 是哪个坐标域、测量返回 content 还是 border box。统一后修复 G2-03；按 Stack -> XY/Freeform -> Toolbar/Flow -> Border/Grid 的依赖顺序补齐 hints、min、hidden、span、grab/deficit 验收。测试用 hint-sensitive Figure，不能只用固定 RectangleFigure。
4. **最后容器扩展契约**：用非内置 layout、ScalableFigure、RangeModel 和 ZoomScrollPolicy 的成功及失败用例固定 prepare/commit 接口；明确 sealed effects 的能力边界、source operation 与 derived calculation 的错误分类。再验证 ScrollPane automatic 与 tracks/min、freeform extent 和逻辑像素单位的组合。
5. 主 Agent 统一重跑必要 cargo 门禁，并确认外部 Connection 改动后的共享行号。workspace 测试总数只作为基线健康信息；上述 U 场景必须有独立断言才能升级证据等级。

## 6. 实际阅读清单

**规范与规则**：AGENTS.md、CLAUDE.md、ADR-014/015 全文；Layer/Freeform、Scroll/Zoom 输入、UpdateManager 设计全文；Draw2D api-coverage 相关 family/备注；reviewer-brief、review_groups、baseline 元数据；两个 skill 及 bits-code-guard 的 general-workflow/review-dimensions/review-rule；项目与当日记忆。记忆中历史 Zest 表述没有用作语义依据。

**Rust 完整文件**：

- `S/layout/mod.rs`、`xy_layout.rs`、`stack_layout.rs`、`border_layout.rs`、`grid_layout.rs`、`flow_layout.rs`、`toolbar_layout.rs`、`freeform_layout.rs`、`fill_layout.rs`。
- `S/container/mod.rs`、`layer.rs`、`range_model.rs`、`scalable.rs`、`scroll_pane.rs`、`viewport.rs`、`zoom.rs`。
- `S/runtime/update/mod.rs`、`deferred.rs`、`listener.rs`、`repair.rs`，包括其中实际断言。
- `T/m5_layout_contract.rs`、`m8_viewport_contract.rs`、`d2_freeform_contract.rs`、`d2_layer_contract.rs`、补充 `d4_constrained_measurement.rs`。

**Rust 共享范围**：`S/graph/mod.rs` 的 LayoutState/NodeState 边界、测量、validation、输出提交、constraint、set_bounds/erase、freeform 派生和 LayoutContext；`S/runtime/runtime.rs` 的 layer admission/membership、layout mutation、set_bounds、zoom facade、fault guard、stabilize 与 frame preparation；`S/figure/mod.rs` 的 FigureMeasurement、Figure/Container/Layer/Freeform capability；scene/editor lib 与 editor viewer 的共享 diff。未宣称完整审计 Group 1/4/6 所有函数。

**Java 全文或相关完整方法/Javadoc**：

- 全文：`J/LayoutManager.java`、`AbstractLayout.java`、`XYLayout.java`、`StackLayout.java`、`BorderLayout.java`、`FreeformLayout.java`。
- 算法方法：`J/GridLayout.java` preferred、placement/track 核心分支、wrapping、position；`FlowLayout.java` preferred/layout/layoutRow/setBoundsOfChild；`ToolbarLayout.java` layout（含 hint、min、压缩、alignment 全函数），并检索其 preferred/minimum 声明与说明。
- 全文：`J/UpdateManager.java`、`DeferredUpdateManager.java`、`ViewportLayout.java`、`RangeModel.java`、`DefaultRangeModel.java`、`ScrollPaneLayout.java`、`ScrollPaneSolver.java`、`FreeformViewport.java`、`FreeformHelper.java`、`Layer.java`、`LayeredPane.java`、`ScalableFigure.java`、`ScalableLayeredPane.java`、`ScalableFreeformLayeredPane.java`。
- 相关完整方法：`J/Figure.java` erase、setBounds、preferred/minimum、invalidate/invalidateTree/revalidate/validate；`J/Viewport.java` range、view location、tracks、readjust、propertyChange、translate/validate。
- `J/zoom/IZoomScrollPolicy.java`、`DefaultScrollPolicy.java`、`MouseLocationZoomScrollPolicy.java`、`package.html` 全文；`AbstractZoomManager.java` primSetZoom、setZoom、fit 分支、levels/policy 相关方法。

七维度复核：逻辑与领域语义见映射/缺陷；健壮性见候选几何及错误传播；并发仅确认独立 RangeModel 锁边界，不推测私有模型的外部回调；本组没有发现可证实的注入/权限类安全缺陷。性能方面 invalid Vec 去重、全祖先失效和 Grid occupancy 有规模成本，但未跑基准，不把复杂度观察升格为性能缺陷。命名、注释、日志和风格问题不进入 JSONL。
