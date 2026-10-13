# ADR-036 Context 阶段权限投影完成记录

类型：`verification`

日期：2026-10-13

状态：`complete`

## 结论

[ADR-036](../../adr/adr-036-context-phase-authority-projection.md) 已完成第一批迁移并通过
全部门禁。Runtime 继续作为唯一 owner；扩展 callback 只取得阶段化 query、service 和
effect/output 权限，不取得 Runtime、FigureTree、FigureNode 或同步 mutation closure。

## 实现

### Event

- `EventContext` 只聚合 target metadata、`EventSceneQuery` 与 `EventEffects`；
- `EventSceneQuery` 提供受检 parent、bounds、child-content area 和 children 查询；
- foreign、unknown/disposed 与 detached Figure 使用结构化错误区分；
- callback effect 只从 `context.effects()` 记录，保持 FIFO 延迟提交；
- 删除 `EventContext::parent_content_bounds()`，不保留 alias；
- UML 拖拽通过 `parent_id(target)` 与 `child_content_bounds(parent)` 组合当前边界。

契约测试证明：callback 内 query 始终读取进入 callback 时的 committed prefix；
`set_bounds_later` 在 callback 返回前不可见，返回后由 Runtime 提交。

### Layout

- `LayoutSnapshot` 直接重命名为 `LayoutSceneQuery`；
- 内部读取协议重命名为 `LayoutSceneRead`；
- `LayoutManager` 继续显式消费 `LayoutSceneQuery + LayoutOutput`；
- 不增加隐藏 Query → Output 原子边界的万能 `LayoutContext`。

### Connection Routing

- `SceneRead` → `RouteSceneRead`；
- `SceneQuery` → `RouteSceneQuery`；
- `TrackedSceneQuery` → `TrackedRouteSceneQuery`；
- `SceneQueryError` → `RouteSceneQueryError`；
- Anchor、Router、Runtime routing service 与外部 consumer 已一次迁移，无兼容双入口。

### 保留的 Context

`PaintContext`、`MeasureContext`、`StableSceneQuery` 与 Editor `ToolContext` 已符合
阶段最小权限模型。本批次未增加 `PaintSceneQuery`、UniversalContext 或无真实消费者的
通用 capability registry。

## 公开 API

`verification/public-api/novadraw-symbols.txt` 已冻结：

- Event：`EventContext`、`EventSceneQuery`、`EventSceneQueryError`、`EventEffects`；
- Layout：`LayoutSceneQuery`、`LayoutOutput`；
- Connection：`RouteSceneRead`、`RouteSceneQuery`、`TrackedRouteSceneQuery`、
  `RouteSceneQueryError`、`RouteOutput`。

旧 `LayoutSnapshot`、`SceneRead`、`SceneQuery`、`TrackedSceneQuery`、
`SceneQueryError` 和 `parent_content_bounds` 已退出当前源码与 API snapshot。

## 验证

| 门禁 | 结果 |
|---|---|
| `cargo xtask verify core.context-phase-authority` | PASS |
| Event committed-prefix / deferred effect contract | PASS |
| Layout external consumer 与完整 M5 contract | PASS |
| Route tracked dependency 与完整 M9 contract | PASS |
| UML expanded/shrunk viewport drag | PASS |
| Public API snapshot | PASS，Runtime 23/25 |
| `cargo xtask docs` | PASS |
| `cargo xtask check --quick` | PASS |
| `cargo xtask check --full` | PASS |
| Native/Web backend、workspace Clippy、tests、doctests | PASS |

UML headless report：
`target/verification/reports/uml-demo.json`。

## 后续边界

新增 Context capability registry 仍需真实外部消费者、typed phase key、安装期校验和
只读生命周期证据。该扩展不得重新引入公开 AnyMap、裸 TypeId、字符串 service locator
或 Runtime mutation closure。
