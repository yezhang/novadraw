# Draw2D / GEF 语义审计整改状态

类型：`verification`

状态：`in_progress`

日期：2026-09-16

最近更新：2026-09-18

本页追加记录
[`../reference/draw2d-gef-semantic-baseline-2026-09-16.md`](../reference/draw2d-gef-semantic-baseline-2026-09-16.md)
及
[`2026-09-16 审计证据`](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/review_groups.md)
中 17 条 P1 的关闭证据。原始审计快照、反例和判断保持不变。

## 当前结论

- 已关闭：12；
- 待关闭：5；
- G5 自动门禁通过，检查点 C 仍待人工验收；
- G6 尚未启动，不应在检查点 C 与优先 P1 整改前提升状态。

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

release 定向测试：

```text
cargo test -p novadraw-scene --release \
  runtime::runtime::adr014_tests::disposal_removes_owned_listener_but_preserves_shared_registrations \
  -- --exact
```

## 推进顺序

1. 按反例逐项关闭剩余生命周期、布局、绘制、Connection 和 Viewer fault/复杂度问题；
2. 每项保留修复前反例，新增自动回归并执行对应 suite；
3. 完成 G5 检查点 C 人工验收；
4. 再启动 G6 保存加载与 Native/Web/Headless 等价。

G5/G6 状态仍以
[`../../roadmap/editor/00-index.md`](../../roadmap/editor/00-index.md) 为唯一入口。
