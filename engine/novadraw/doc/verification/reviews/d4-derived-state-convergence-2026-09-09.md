# D4.1 派生状态收敛验证

类型：`verification-record`

日期：2026-09-09

## 结论

D4.1 已完成。Runtime 的正常 frame preparation 不再依赖应用显式 reroute 或首帧后的
补偿性 full redraw。

## 实现证据

- `Runtime` 使用固定优先级 `DerivedWorkSet` 调度 intrinsic metrics、layout、
  dependency invalidation、routing、post-route geometry 与 presentation。
- FigureTree validation 与 damage recording 已拆开，派生状态稳定后才进入 recording。
- ConnectionRuntime 根据 tracked dependency generation 自动发现直接修改、callback、
  LayoutOutput geometry 和 routing-domain child order 变化。
- dirty Connection 参与 `Runtime::has_pending_update`，routing space 从 Connection
  parent child-content domain 推导。
- Label 分离无宽度约束的 intrinsic cache 与依赖最终 client area 的 presentation cache。
- ViewportLayout 只生成 sealed `ViewportLayoutEffect`；完整 LayoutOutput 校验通过后才
  提交 RangeModel 与 content scale。
- `prepare_submission`、`prepare_frame`、`record_full_frame` 共用 stabilization；
  full redraw 不能绕过 stable epoch 门禁。

## 回归验证

- `first_submission_shapes_label_after_parent_layout`
- `normal_frame_automatically_resolves_dirty_connection_routes`
- `layout_output_geometry_automatically_invalidates_and_reroutes`
- `invalid_layout_output_does_not_commit_viewport_range_effect`

## 自动门禁

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过
- `cargo clippy --workspace -- -D warnings`：通过
- `cargo test --workspace`：通过
- `cargo check -p web-validation --target wasm32-unknown-unknown`：通过

补充：`cargo clippy -p novadraw-scene --all-targets -- -D warnings` 仍被三处既有测试
lint 阻断，分别为 `graph/mod.rs` 的 `type_complexity`、`border_layout.rs` /
`flow_layout.rs` 的 `items_after_test_module` 和 `repair.rs` 的 `useless_vec`；本次修改
未新增这些告警。
