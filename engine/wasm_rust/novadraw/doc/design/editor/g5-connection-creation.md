# G5.2 Connection Creation 契约

类型：`normative-design`

状态：`target`

适用范围：两阶段连接创建的 Request、Tool、Policy、Command、feedback 与取消语义。
已有连接的模型投影和 Runtime binding 由
[`g5-connection-projection.md`](g5-connection-projection.md) 定义；重连、bendpoint、
viewport/zoom 与 auto-expose 不在本切片实现。

## 1. 目标

G5.2 建立以下闭环：

```text
activate connection tool
-> first press resolves and locks source Part
-> source policy creates a gesture-scoped ConnectionCreation plan
-> pointer move resolves target candidate and refreshes feedback
-> second press clears feedback and creates one model Command
-> CommandStack executes
-> model notification
-> G5.1 projects the committed ConnectionPart
```

Tool、Policy 和 feedback 均不得直接插入业务连接或稳定 ConnectionPart。

## 2. 两阶段请求

`CreateConnectionRequest` 是 model-independent typed request，至少保存：

- application-defined `CreationType`；
- stable source `EditPartId`；
- optional target candidate `EditPartId`；
- latest pointer location；
- modifier snapshot；
- monotonic interaction revision。

第一阶段的 target candidate 为 `None`。第二阶段只有在 Viewer 命中一个 active
model-backed Part 时才携带 target candidate。Request 不保存 ModelId、Command、
EditPolicy 或 Runtime identity。

## 3. ConnectionCreation 计划

source Part 的 policy 通过专用入口返回一个 gesture-scoped
`ConnectionCreation<A>`。该计划是 GEF start command 在 Rust 所有权模型下的合理
变体：

- 创建时由 `PolicyHost` 固定 source ModelId；
- 生命周期只覆盖当前手势，不进入 CommandStack；
- 可以根据 target `PolicyHost` 判断能否完成；
- 可以为当前 target/location 贡献 feedback；
- 完成时生成一个只保存应用 ModelId 和业务数据的最终 `Command<A>`；
- cancel 或 source 退休时直接丢弃，不修改模型。

同一 source 上最多允许一个 policy 接受一次连接创建。多个 policy 同时返回计划属于
配置歧义，必须结构化拒绝，不能依赖 role 遍历顺序静默选一个。

## 4. Tool 状态机

```text
Selection
  -- activate(type) --> ConnectionArmed(type)

ConnectionArmed
  -- valid left press --> ConnectionStarted(source, plan)
  -- Escape/focus loss --> Selection

ConnectionStarted
  -- pointer move --> replace feedback
  -- invalid/handled press --> stay ConnectionStarted
  -- valid second press --> clear feedback -> execute Command -> Selection
  -- Escape/focus loss/source invalid --> clear feedback -> Selection
```

连接创建采用两个 press，而不是 press-drag-release。Figure-native widget 仍优先消费
输入；被消费的 press 不得启动或完成连接。连接模式不改变 Viewer selection。

成功提交前必须先删除全部 transient feedback。若 Command 或后续 Viewer refresh
失败，错误沿 `EditorDomainError` 返回；CommandStack 与 Viewer 使用既有 fault
规则。

## 5. Target 与 Policy

- source 和 target 都必须属于同一 Viewer namespace；
- source 在整个手势中固定，不随 hover 改变；
- target candidate 每次 pointer move/press 重新命中；
- contents fallback、handle、feedback 和 retired Part 不是 endpoint；
- self-loop 在框架层合法，应用 policy 可以拒绝；
- connection-to-connection endpoint 在 G5.2 拒绝，后续有真实需求再开放；
- target 是否满足业务端口、类型和重复边约束由 `ConnectionCreation` 判断。

Viewer 负责把 target `EditPartId` 解析成只读 `PolicyHost<ModelId>`。Tool 和 Request
不直接访问应用 ModelId。

## 6. Feedback

feedback 由 `ConnectionCreation` 贡献，并通过 Viewer 的标准 feedback layer 注册：

- Viewer 使用 endpoint behavior 的 source/target Anchor descriptor 与 Runtime 只读
  preview 计算 `ConnectionFeedbackRoute`；pointer 端使用 ownerless `XYAnchor`；
- `feedback_with_route()` 优先消费该 route，避免应用重复按节点 bounds 猜测端点；
- source-only 阶段至少可显示 source 到 pointer 的临时路径；
- valid target 阶段可以附加 target highlight；
- 每次更新先清除旧 feedback，再安装新 feedback；
- feedback 不参与 Editor targeting；
- release 不提交，第二次有效 press 才提交；
- cancel、focus loss、tool switch、source retirement 和成功提交都必须清空。

G5.2 使用 model-scaled feedback 域。G5.5 已统一将 logical surface pointer 转换为
Connection routing-domain request location，并在 scroll/zoom 后重新计算 target 与
feedback；应用不得复制 viewport 变换。

## 7. 模型 Command

最终 Command：

- 只保存 source/target connection ModelId 和应用连接数据；
- execute 原子插入一个连接并发布一个稳定 revision；
- undo 删除同一连接；
- redo 恢复同一模型身份和规范 Z-order；
- 不保存 EditPartId、FigureId、ConnectionPartId、AnchorId 或 ConnectionId。

节点删除时，应用 Command 必须在同一模型事务中删除关联连接，并在 undo 时恢复原有
连接顺序。Viewer 不推断业务级联。

## 8. 与 GEF 的对应与差异

保留 GEF：

- source/start 与 target/complete 两阶段；
- source 在第一阶段锁定；
- hover 持续更新 target 和 feedback；
- 失焦、Escape 和非法生命周期变化取消；
- 最终通过模型 Command 提交。

Novadraw 差异：

- Request 保持可克隆、可比较的数据对象，不持有 mutable Command；
- source policy 返回 `ConnectionCreation` 计划，第二阶段再构造最终 Command；
- Viewer 从单一 registry 解析 target ModelId，不依赖 Java 对象引用；
- 成功后复用 G5.1 全量有序 connection snapshot 投影。

## 9. 验证门禁

新增 `g5_connection_creation_contract.rs`，至少覆盖：

1. request 的 source、target、location、modifier 与 revision；
2. 第一端锁定 source，pointer move 不改变 source；
3. connection mode 不改变 selection；
4. widget-consumed press 不启动或完成手势；
5. source policy 缺失、拒绝和重复计划；
6. invalid target 保持手势且不执行 Command；
7. valid target 在清理 feedback 后执行一个 Command；
8. committed model event 经 G5.1 只创建一个 ConnectionPart；
9. self-loop 可由 policy 接受；
10. Escape、focus loss 和 tool switch 清理 feedback 且不改模型；
11. execute/undo/redo 保持 connection ModelId 和顺序；
12. 节点删除级联与 undo 恢复关联连接；
13. workspace fmt/check/clippy/test。

通过门禁后，`connection.create` 提升为 `verified`；`connection.reconnect`、
`viewport.autoexpose` 由 G5.5 闭合。G5.2 不单独开放检查点 C。
