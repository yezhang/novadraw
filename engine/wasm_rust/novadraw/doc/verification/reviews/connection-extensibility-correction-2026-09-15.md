# Connection 扩展边界整改

日期：2026-09-15

类型：`implementation-verification`

## 对标边界

本次整改只使用 `org.eclipse.draw2d` 与 `org.eclipse.gef` 的框架契约：

- Draw2D `Connection`、`PolylineConnection`、`ConnectionAnchor`、
  `ConnectionRouter`、`Locator`；
- GEF `ConnectionEditPart`、`NodeEditPart`、`BendpointRequest` 与连接 EditPolicy。

Zest 的 self-loop、curve depth、LoopAnchor 和其他产品扩展不作为需求、设计或实现依据。

## 已关闭问题

1. Route 采用 calculation、Figure geometry/Locator preflight、commit 三阶段；Runtime
   不再先写 `Resolved` 后尝试 Figure commit。
2. `ConnectionFigureBehavior::prepare_route_geometry()` 提供可替换几何边界；
   内置折线与 Vello 后端共享显式 miter-limit 契约。
3. Runtime 持有 Connection direct-child Locator binding，route 提交时同步 child
   placement、damage 和 freeform extent。
4. Endpoint `EditPartBehavior` 提供带稳定 `AnchorSemanticKey` 的 source/target
   descriptor；Viewer 只提供 Chopbox fallback，相同 key 保留 AnchorId。
5. 创建和重连 feedback 使用同一 endpoint provider，通过 Runtime 只读 preview
   生成 `ConnectionFeedbackRoute`。
6. Connection routing 由应用 behavior 决定；Factory 预注册命名共享 Router，
   `ModelAdapter` 不规定 bendpoint 存储结构。
7. `PartTree` 增量维护 outgoing/incoming；全量索引重建只发生在真实 connection
   order 变化时。
8. 核心已删除 `SelfLoopRouter` 与默认 extent。Native demo 将 self-loop 建模为两个
   显式 bendpoints，并使用标准 `BendpointConnectionRouter`；两个橙色拐角 handle
   从创建完成起就是可独立移动、可 undo/redo 的模型事实。

索引复杂度核算：旧实现初始插入 `E` 条连接会扫描
`1 + 2 + ... + E = E(E+1)/2` 条记录；新实现执行 `E` 次增量 append。以
`E = 10,000` 为例，关系索引工作量由 `50,005,000` 次连接扫描降为 `10,000`
次 append；只有模型确实重排时再执行一次 `O(E)` 重建。

## 状态修正

- `connection.figure`：保持 `verified`，新增 geometry preflight 与无效几何门禁。
- `connection.anchor`：Draw2D Runtime 保持 `verified`；GEF endpoint provider 已闭合。
- `connection.locator`：调整为 `partial`。普通 child relocation 已接入 Runtime，
  RotatableDecoration 的 reference/orientation capability 仍需明确 P2 delta。
- 应用级 self-loop 不进入 Draw2D/GEF parity。

## 验证

- `cargo fmt --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy -p novadraw-scene -p novadraw-editor -p node-editor-demo --lib --bins -- -D warnings`

`cargo clippy --workspace --all-targets -- -D warnings` 仍受既存测试代码告警阻断：
`graph/mod.rs` 测试 type complexity、两个 layout 文件的 items-after-test-module，以及
`runtime/update/repair.rs` 的 useless-vec；这些不由本次 Connection 改动引入。
