# G5.4 Connection Bendpoint 契约

类型：`normative-design`

状态：`target`

适用范围：连接 bendpoint 模型投影、create/move/delete handles、拖动反馈和 history。

## 1. 模型事实

应用的 Connection `EditPartBehavior` 通过 `connection_bendpoints()` 暴露供通用 handle
使用的有序点，并通过 `connection_routing()` 返回 Router selection 与 typed
constraint。`ModelAdapter` 不规定应用如何存储 bendpoint；`ModelConnection` 继续只保存
id/source/target，保持轻量 Copy 契约。

Native demo 的产品策略要求 self-loop 从创建完成起持有两个显式 bendpoints；两个回环
拐角因此都是普通 `BendpointMove` handle。两点共享节点右侧的外侧 x，source/target
Anchor 位于节点右边界且 y 分别跟随首尾 bendpoint，使 route 始终保持水平、垂直、水平
三段正交。移动一个拐角不得删除另一个，节点平移时两点随模型同步平移。该策略属于 demo
模型，不上升为 Draw2D/GEF parity。

Viewer 在投影 ConnectionPart 时读取 behavior 配置：

- Factory 通过稳定 key 注册共享 Router，behavior 选择 inherited 或命名 Router；
- 应用可选择 `BendpointConnectionRouter` 并提供 `BendpointConstraint`，框架不根据
  bendpoint 是否为空自动切换 Router；
- bendpoint 变化保留 ConnectionPart/Figure/endpoint Anchor identity；
- behavior 暴露的非有限点和未知 connection 必须在投影提交前拒绝。

应用 Command 是 bendpoint 的唯一写入者；Tool、Policy 和 Viewer 不直接修改模型。

## 2. Request

`BendpointRequest` 保存：

- `ConnectionPartId`；
- `BendpointOperation::{Create { index }, Move { index }, Delete { index }}`；
- pointer location；
- modifier snapshot；
- interaction revision。

index 是应用 bendpoint 列表索引。create handle 位于每个 route segment 中点，其 index
表示插入位置；move handle 位于每个显式 bendpoint。

## 3. Handle 与 Tool

连接单选时：

- 每个显式 bendpoint 显示 move handle；
- 每个相邻 route segment 显示较小的 create handle；
- endpoint handles 与 bendpoint handles 可同时存在；
- handle 坐标来自已提交 route，不能从模型端点 bounds 猜测。

Editor 负责从 committed route 与模型 bendpoint 列表推导 handle role、index 和 surface
位置；应用负责具体 Figure、颜色、尺寸和额外 Router 策略。是否允许 self-loop 以及
如何生成可见回环都不属于 Draw2D/GEF 核心语义。

`ConnectionBendpointTool` 使用 press-drag-release：

- press 锁定 connection、operation 与 index；
- move 超过 threshold 后持续生成 feedback；
- release 前清除 feedback，再生成一个模型 Command；
- move 到相邻 route segment 的容差带可转换为 Delete；
- Escape、focus loss、history transition 或 connection retirement 清理手势。

## 4. Policy 与反馈

ConnectionPart bendpoint policy：

- 验证 index 与当前 bendpoint 数量；
- create 插入一个点；
- move 替换一个点；
- delete 移除一个点；
- feedback 使用应用 behavior 暴露的 bendpoints 与 framework-resolved endpoint route，
  不修改稳定 Connection Figure；
- Command 保存 before/after 有序点列表，用于原子 execute/undo/redo。

## 5. 与 GEF 的对应与差异

保留 GEF：

- `BendpointRequest` 的 connection/index/location；
- move handle 对应显式 bendpoint；
- create handle 位于 segment midpoint；
- 拖回相邻 segment 可删除 bendpoint；
- feedback 先清理，再提交 Command。

Novadraw 差异：

- 不在 feedback 期间原地替换稳定 Connection Figure constraint；
- Request 使用 typed operation；
- bendpoint 列表和 routing descriptor 来自 Connection behavior；
- Viewer 只解析应用注册的 Router key，并通过 Runtime typed constraint 原子投影。

## 6. 验证门禁

至少覆盖：

1. typed request 的 connection/operation/location/revision；
2. bendpoint snapshot 到 Runtime constraint 的初始投影；
3. create/move/delete index 验证；
4. segment midpoint create handle 与 bendpoint move handle；
5. create/move/delete feedback cleanup；
6. execute/undo/redo 保持 connection identity、endpoint 和列表位置；
7. bendpoint 变化只更新 constraint 与 route；
8. 非有限点和 revision drift 在提交前拒绝；
9. demo self-loop 的两个显式 bendpoints、正交 route、独立 move handles、节点平移和
   undo/redo；
10. workspace fmt/check/clippy/test。
