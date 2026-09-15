# G5.3 Connection Reconnect 契约

类型：`normative-design`

状态：`implemented`

适用范围：已提交连接的 endpoint handle、拖动重连、feedback、Command 与取消语义。

## 1. 目标

```text
select ConnectionPart
-> show source/target endpoint handles
-> press one endpoint handle
-> lock connection identity and moving endpoint
-> drag resolves candidate endpoint and replaces feedback
-> release clears feedback and executes one model Command
-> G5.1 rebinds the retained ConnectionPart
```

重连不得删除并重建业务连接，也不得让 Tool 直接修改 Runtime binding。

## 2. Request

`ReconnectConnectionRequest` 至少保存：

- stable `ConnectionPartId`；
- `ConnectionEndpoint::{Source, Target}`；
- optional target candidate `EditPartId`；
- latest entry-domain location；
- modifier snapshot；
- monotonic interaction revision。

Request 不保存 ModelId、FigureId、AnchorId、ConnectionId、Policy 或 Command。

## 3. Policy 与 Command

ConnectionPart 的专用 policy 返回 gesture-scoped `ConnectionReconnection<A>`：

- plan 固定 connection ModelId、原 source/target ModelId 和 moving endpoint；
- candidate 必须是同 Viewer 的 active containment Part；
- connection visual、contents、handle、feedback 和 retired Part 不是 endpoint；
- self-loop 在框架层合法，业务 policy 可拒绝；
- 默认 inherited router 通过 `SelfLoopRouter<DirectRouter>` 为 same-owner anchors
  生成 owner 外侧回环；显式 bendpoint/router 约束优先；
- final Command 只更新被移动的一端，保持 connection ModelId 和列表位置；
- undo/redo 恢复相反端点并复用 G5.1 endpoint rebind。

同一 ConnectionPart 最多一个 policy 接受 reconnect；歧义必须结构化拒绝。

## 4. Handle 与 Tool

`HandleRole` 增加 source/target endpoint role。handle：

- owner 是 ConnectionPart 的 canonical `EditPartId`；
- 位于 unscaled handle layer；
- 位置跟随已解析 route 的首尾点；
- 只在单选 ConnectionPart 时存在；
- selection 改变、连接退休或 refresh 前必须清理。

`ConnectionEndpointTool` 使用 press-drag-release：

- press 固定 connection 与 moving endpoint；
- move 超过 drag threshold 后更新 candidate 和 feedback；
- release 前清理 feedback；valid candidate 生成一个 Command，否则无副作用结束；
- Escape、focus loss、tool switch、connection retirement、普通 command 和 history
  transition 均取消并清理；
- Figure-native consumed press 不启动重连。

## 5. Feedback

重连 feedback 保留固定端，移动另一端：

- source reconnect：target 端固定，source 端跟随 candidate/pointer；
- target reconnect：source 端固定，target 端跟随 candidate/pointer；
- endpoint 使用与稳定连接相同的边界 anchor 语义；
- feedback 不参与 targeting；
- invalid candidate 可显示自由端，但不能提交。

## 6. 与 GEF 的对应与差异

保留 GEF `ReconnectRequest`、`ConnectionEndpointHandle`、
`ConnectionEndpointTracker` 的端点锁定、拖动 target、先清 feedback 后提交和
cancel 语义。

Novadraw 不在 Request 中保存 Java 对象引用，也不原地替换稳定 Connection Figure 的
anchor 来显示反馈；临时路径位于 feedback layer，提交后由 G5.1 原子 rebind retained
ConnectionPart。

Draw2D NullConnectionRouter 会让同 owner 的 Chopbox anchors 退化为重合点。
Novadraw 保留 `DirectRouter` 的两点等价语义，另以可组合 `SelfLoopRouter` 包装默认
inherited router，避免把产品策略硬编码进 DirectRouter 或 Demo。

## 7. 验证门禁

至少覆盖：

1. typed request 保存 connection、moving endpoint、candidate 和 revision；
2. source/target handle role 与 owner identity；
3. press 锁定 connection 和 endpoint；
4. move 不改变 selection；
5. invalid target release 无模型副作用；
6. source reconnect 与 target reconnect；
7. feedback 在 commit/cancel 前清理；
8. reconnect 保持 connection ModelId、ConnectionPartId、FigureId 和列表位置；
9. 未变化端 AnchorId 保持，变化端替换；
10. undo/redo 恢复端点和身份；
11. connection retirement 自动清理 endpoint handles 与 active gesture；
12. workspace fmt/check/clippy/test；
13. same-owner anchors 生成 owner 外侧的非退化 route，普通 route 仍委托 base router；
14. endpoint handles 读取已提交 route 首尾点，self-loop 下不得重合。
