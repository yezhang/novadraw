# D3.3 Runtime Listener 公共面验证

类型：`verification`

日期：2026-09-08

## 结论

D3.3 已完成。Runtime 已公开 Update、Figure、Coordinate、Ancestor、Property、
Action 和 Layout 七类 listener 的注册入口，全部返回统一 `ListenerId`，并通过
`Runtime::remove_listener` 注销。

## 实现证据

- 新增 `ListenerDirective::{Keep, Remove}`；
- callback 返回 `Remove` 后完成当前 callback，再从对应 listener 表移除；
- self-removal 不跳过同一 effect 上的其他 listener；
- 已移除 listener 不接收当前 flush 中的后续 effect；
- UpdateListener 的 Validating adapter 完成当前 event 后再执行移除；
- listener callback 不持有 Runtime、FigureTree 或 UpdateManager 可变引用；
- effect queue 仍在稳定事务边界冻结并按 FIFO 分发。

## 自动测试

`novadraw-scene/tests/d3_runtime_listener.rs` 共 4 项：

1. 七类 listener 均可通过 Runtime 注册并观察对应事件；
2. 所有类别共享统一注销命名空间，重复注销返回 `false`；
3. Property listener self-removal 保持当前 effect 的注册顺序并跳过后续 effect；
4. Action listener 可在首次 action 后自注销，输入分发行为保持不变。

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
`VelloRenderer::new_for_surface` 与 `create_renderer`。它们不由 D3.3 引入，也不影响
构建结果。

## 兼容性

Listener callback 返回类型从 `()` 调整为 `ListenerDirective`。仓库内 Runtime、
apps 和测试实现均已迁移。该变更发生在 Core 1.0 前，用于形成明确且无需重入的
listener 生命周期契约。
