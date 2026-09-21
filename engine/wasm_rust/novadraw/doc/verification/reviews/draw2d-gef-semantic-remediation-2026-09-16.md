# Draw2D / GEF 语义审计整改状态

类型：`verification`

状态：`complete`

日期：2026-09-16

最近更新：2026-09-21

本页追加记录
[`../reference/draw2d-gef-semantic-baseline-2026-09-16.md`](../reference/draw2d-gef-semantic-baseline-2026-09-16.md)
及
[`2026-09-16 审计证据`](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/review_groups.md)
中 17 条 P1 的关闭证据。原始审计快照、反例和判断保持不变。

2026-09-20 的独立后续批次见
[`Draw2D / GEF 核心语义后续审计`](draw2d-gef-core-semantic-follow-up-audit-2026-09-20.md)。
该批次不改写 2026-09-16 的 17/17 历史关闭结论。

## 当前结论

- 2026-09-16 批次：已关闭 17，待关闭 0；
- 2026-09-20 后续批次：优先整改 5，已修复 5，待修复 0；
- G5 自动门禁通过，检查点 C 仍待人工验收；
- G6 尚未启动，不应在检查点 C 人工验收前提升状态。

## 2026-09-20 后续审计

| 审计项 | 状态 | 当前证据 / 下一步 |
|---|---|---|
| F01 `NdCanvas::rotate` 将 degrees 当作 radians | fixed | `rotate_uses_draw2d_degree_units`：PASS；`novadraw-render` 全量测试：PASS |
| F02 Vello 丢弃 Dash/Dot line style | fixed | Vello Stroke pattern 与 Scene 多段展开测试、Viewport guide 契约：PASS |
| F03 XYLayout 自动轴测量丢失固定轴 hint | fixed | 固定宽/自动高 wrapping Figure 回归与 `m5_layout_contract`：PASS |
| F04 失效 gesture target 导致同一 session 重定向 | fixed | 双 Viewport hide/remove 回归确认 tombstone 保留到 End/Cancel；M6/M8/core.runtime：PASS |
| F05 Policy 激活失败破坏成对生命周期 | fixed | A 成功/B 失败只逆序回滚 A，Behavior 单次停用，Viewer Drop 不重复；G2/G5.1/editor crate：PASS |

后续审计的范围、触发链、18 条次级候选与验证命令统一记录在
[`draw2d-gef-core-semantic-follow-up-audit-2026-09-20.md`](draw2d-gef-core-semantic-follow-up-audit-2026-09-20.md)。

## 已关闭

| 审计项 | 修复 | 关闭证据 |
|---|---|---|
| Web wheel 未处理 `DispatchOutcome` | Web 适配显式丢弃已分发结果，保留既有 `prevent_default` 和重绘语义 | `cargo xtask verify web.build`：PASS；`cargo xtask check --full`：PASS |
| release 构建跳过 listener scope 写入 | 八类 scoped listener 无条件登记 owner，`debug_assert!` 只检查结果 | 修复前 release 定向测试：FAIL；修复后同一测试：PASS |
| enter/exit 使用几何 hover source | Entered、Exited 与 Hover 只由 `mouseTarget` 路由；capture 期间 cursor 继续跟踪物理命中，tooltip source 冻结 | `runtime::event::tests`、`m6_event_contract`、`core.runtime`：PASS |
| Native pointer leave 保留 capture | pointer leave 清理 capture 与 pointer pressed，并向原 `mouseTarget` 发送一次 Exited | `pointer_exit_clears_capture_and_exits_the_mouse_target`、`event-app:pointer_leave_cleanup`：PASS |
| Policy target 身份未参与路由 | 候选 policy 解析首个 target；校验 target 后在目标 Part 聚合 command/feedback，同 target 去重 | `g4_editing_loop_contract::policy_target_*`、G3-G5.5 replay：PASS |
| F13 Viewer 初始投影失败遗漏停用 | Viewer Drop 从 synthetic root 遍历实际 Part，并仅按 `PartNode::is_active` 停用已激活项，不再依赖 contents 最终提交 | `initial_projection_failure_deactivates_every_activated_part_once`、`late_initial_projection_failure_deactivates_the_complete_live_prefix_once`、`g2.viewer-projection`：PASS |
| F15 Viewer 扩展 panic 绕过 fault | `refresh` 以 unwind guard 包围完整投影事务，panic 时先 fault 再继续 unwind；`model_mut` 在 fault 后拒绝业务写入 | `refresh_panic_faults_viewer_after_partial_visual_mutation`、`g2.viewer-projection`、`cargo xtask check --full`：PASS |
| G2-F1 Freeform 溢出 child 旧 damage 丢失 | 几何或拓扑变更前将旧 visual 沿当时父链冻结到 logical-surface 队列；repair 合并 frozen 与当前 local dirty，panic 时恢复两类快照 | `moving_overflow_visible_child_damages_old_and_new_surface_regions`、`test_update_panic_restores_frozen_surface_damage`、`novadraw-scene` crate 测试、`core.runtime`：PASS |
| G2-F2 布局 client area 重复应用 inset | LayoutSnapshot 将 node-local client box 转成零原点 child-content 可用区；inset 只由 parent child transform 应用一次 | `stack_layout_applies_container_insets_once`、`xy_layout_applies_container_insets_once`、`m5_layout_contract`、`m8_viewport_contract`：PASS |
| G2-F3 Border 南/东区预留与摆放不一致 | BorderLayout 先确定各侧最终尺寸，再由 Center 与对应 child 共同消费；South/East 不再在摆放阶段重复应用半轴上限 | `border_layout_uses_the_reserved_south_size_for_placement`、`border_layout_uses_the_reserved_east_size_for_placement`、`m5_layout_contract`：PASS |
| G3-F01 删除 Ready 图片后保留旧引用 | ImageFigure 对缺失资源进入 `Unavailable` 并清除旧 `ImageResourceRef`；共享 dependent 同帧刷新，Remove submission 不再携带旧 Image command | `removing_ready_image_clears_all_shared_figure_references`、`m10_label_contract`：PASS |
| G3-F02 CompoundBorder 丢失 TitleBar owner 快照 | BorderSnapshot 与 Compound outer/inner 结构同构，递归解析、合成指标并按累计 inset 绘制；共享 Compound 仍按 owner 字体隔离 | `compound_border_resolves_title_bar_snapshots_at_every_nesting_position`、`shared_compound_title_bar_border_keeps_metrics_per_owner`、`m10_label_contract`、`m10_reusable_shape_border_contract`、`novadraw-scene` crate 测试：PASS |
| G3-F03 Label North/South 方向反转 | TextPlacement 统一表示文字相对图标的方向；North 将文字置于图标上方，South 将文字置于图标下方 | `text_placement_positions_text_relative_to_icon_in_all_four_directions` 同时核对 glyph origin、Image command、gap 与 icon named geometry；`m10_label_contract`：PASS |
| G4-F1 route 预检拒绝后丢失恢复依赖 | geometry/Locator 预检失败时先合并 batch 中各 calculation 的当前 observations，再转 Unresolved 并清理旧 route；不提交 generation、points 或 child placement | `first_locator_preflight_failure_recovers_when_an_observed_owner_moves`、`locator_preflight_failure_after_success_refreshes_recovery_dependencies`、`m9_connection_runtime`、`core.runtime`：PASS |
| G4-F2 反向 Fan connection 重合 | 无向 Anchor pair 内按主轴规范统一向西/向北的几何方向，再应用稳定 child-order lane；各 connection 保留自身端点与 metadata 顺序 | `fan_router_separates_bidirectional_connections`、`fan_router_uses_stable_mixed_direction_order_and_recenters_after_removal`、`m9_connection_runtime`、`m9_connection_contract`：PASS |
| G4-F3 reparent 改变 absolute bendpoint 坐标含义 | topology commit 前将内置 BendpointConstraint 的 absolute 点从旧 parent child-content 映射到新域，relative 点保持不变；未知自定义 constraint 结构化原子拒绝 | `reparenting_connection_maps_absolute_bendpoints_into_the_new_routing_domain`、`reparenting_connection_with_unknown_constraint_is_rejected_atomically`、`m9_connection_runtime`、`d3_runtime_mutation`：PASS |
| G5-F02 未变化连接触发平方级投影 | Viewer 每批构造一次完整 connection Figure 顺序，Runtime 以一次 O(E) 比较/校验原子提交；等价 None/Bendpoint 路由配置 no-op，未变端点不主动 resolve | `unchanged_connection_projection_does_not_reroute_each_connection` 以 64 条独立连接确认属性 refresh 为 0 次 route；`runtime_child_order_controls_paint_and_reverse_hit_order_atomically`、`bendpoint_router_preserves_absolute_and_relative_constraints`、`g5.1.connection-projection`：PASS |

release 定向测试：

```text
cargo test -p novadraw-scene --release \
  runtime::runtime::adr014_tests::disposal_removes_owned_listener_but_preserves_shared_registrations \
  -- --exact
```

## 推进顺序

1. 保留两个批次共 22 条 P1 的反例与自动回归；
2. 完成 G5 检查点 C 人工验收；
3. 再启动 G6 保存加载与 Native/Web/Headless 等价。

G5/G6 状态仍以
[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md) 为唯一入口。
