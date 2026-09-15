# G5.3 Connection Reconnect 行为验证

类型：`verification`

日期：2026-09-15

状态：`passed`

## 范围

- typed `ReconnectConnectionRequest`；
- source/target endpoint handle；
- press-drag-release reconnect Tool；
- connection-owned reconnect policy plan；
- model-only reconnect Command 与 undo/redo；
- G5.1 retained ConnectionPart endpoint rebind；
- same-owner self-loop route 与 endpoint handle 定位。

设计契约：
[`../../design/editor/g5-connection-reconnect.md`](../../design/editor/g5-connection-reconnect.md)。

## 结论

- source/target reconnect 均通过模型 Command 提交；
- invalid drop 与 Escape 无模型副作用并清理 feedback；
- reconnect 保持 connection ModelId、ConnectionPart identity 和顺序；
- delete/undo 与节点移动回归通过；
- Demo 在后续 G5.4 校准中将 self-loop 建模为两个显式 bendpoints，由标准
  `BendpointConnectionRouter` 生成外侧回环；
- endpoint handles 读取已提交 route 首尾点，普通连接与 self-loop 均显示两个独立 handle。

Draw2D `NullConnectionRouter` 对同 owner Chopbox anchors 会退化为重合点。Novadraw
保留 `DirectRouter` 的两点等价语义；是否接受 self-loop 以及应用采用显式 bendpoints
还是额外 Router 均由应用决定，不计入 Draw2D/GEF parity。

## 自动门禁

- `novadraw-editor/tests/g5_connection_creation_contract.rs`：包含应用 Router 扩展与
  外侧 route 契约；
- workspace `fmt/check/clippy/test`：通过。

## 人工验收

[`../manual/g5-connection-reconnect.md`](../manual/g5-connection-reconnect.md) A-E 全部
通过。期间发现并关闭：

1. self-loop route 退化为中心点；
2. self-loop 两个 endpoint handles 重叠于中心。

G5.3 完成；G5 整体仍为 `in_progress`，下一切片为 G5.4 bendpoint。
