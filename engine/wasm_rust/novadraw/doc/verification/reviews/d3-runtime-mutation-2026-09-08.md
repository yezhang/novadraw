# D3.2 Runtime 动态 Mutation 验证

类型：`verification`

日期：2026-09-08

## 结论

D3.2 已完成。Runtime 用户可以在不取得裸 `&mut FigureTree` 或
`&mut UpdateManager` 的前提下动态修改 layout、constraint、size override、
普通 child order 和 clipping strategy。Figure callback 使用同一组 typed
`PendingMutation`，在顶层 dispatch 后按 FIFO 提交。

## 实现证据

- `RuntimeMutationError` 区分未知/脱离 Figure、非法父子关系、LayeredPane、
  child index、size 和 layout constraint 错误；
- `LayoutManager::validate_constraint` 在 constraint 或 manager replacement
  提交前检查兼容性；
- preferred/minimum/maximum size 支持 set/clear 并进入 validation queue；
- 普通 child reorder 同步 paint order、逆序 hit-test、repaint 与 connection
  invalidation；
- clipping 使用 NodeState 显式 override，并在变化时请求 full redraw；
- `Figure::paint_children` 误导性 no-op 已删除，真实 traversal 仍只由递归 renderer
  承载；
- deferred mutation 独立原子、FIFO 执行，失败保存在 Runtime error queue 中且不
  阻断后续 mutation。

## 自动测试

`novadraw-scene/tests/d3_runtime_mutation.rs` 共 6 项：

1. layout manager 与 constraint replacement 预验证和原子失败；
2. preferred/minimum/maximum size set/clear 与非法 size 拒绝；
3. child index/front/back 与逆序 hit-test；
4. LayeredPane 拒绝普通 child-order API；
5. clipping replacement 改变真实 render protocol 并强制 full damage；
6. callback mutation 在中间失败时保持 FIFO、已提交前缀和后续 mutation。

## 门禁结果

以下命令均通过：

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test --workspace
cargo check -p novadraw --target wasm32-unknown-unknown
cargo check -p web-validation --target wasm32-unknown-unknown
```

WASM `novadraw` 构建保留两个既有 `dead_code` warning：
`VelloRenderer::new_for_surface` 与 `create_renderer`。它们不由 D3.2 引入，也不影响
构建结果。

## 实现期发现

在 `EventContext` 中直接导入 `LayoutConstraint` 会使其 blanket `as_any()` 参与
`Box<dyn Figure>` 的方法解析，导致 ScrollPane/Viewport 类型识别失败。最终实现使用
全限定 `crate::LayoutConstraint` 作为 generic bound，避免改变 Figure 的 `AsAny`
分派；M8 的 24 项 viewport/zoom 测试已复验通过。

同时修正 full redraw 与 pending incremental update 同时存在时的提交行为：必须在
validation 后重新记录完整场景，不能把局部命令仅标记成 full damage。
