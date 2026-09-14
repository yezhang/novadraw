# G5.1 Connection Projection 设计复核

类型：`verification`

日期：2026-09-14

状态：`reviewed`

实现门禁：`pending_user_approval`

## 1. 复核范围

- GEF ConnectionEditPart、NodeEditPart、两端关系刷新与 registry 去重；
- Novadraw ModelId/EditPartId/FigureId 身份边界；
- connection layer 与 ConnectionRuntime 生命周期；
- 模型快照、确定性顺序、增量刷新和失败恢复；
- G5.1 与 create/reconnect/bendpoint/auto-expose 后续切片的边界。

设计提案：
[`../../design/editor/g5-connection-projection.md`](../../design/editor/g5-connection-projection.md)。

## 2. 已修正问题

### R1：裸 EditPartId 缺少角色约束

原方案直接让 connection 专用 API 接受 `EditPartId`，运行时才能发现普通 containment
Part 被误传。

修正为单一 canonical `EditPartId` 加受检 `ConnectionPartId` 包装。包装不分配第二
身份，但为 connection 专用 API 提供静态类型边界。

### R2：source preorder 不是稳定连接 Z-order

原方案使用 source containment preorder 与 source-list order 推导视觉顺序。节点
重排或 reconnect source 会无意改变连接 Z-order，并可能导致实时编辑结果与重新加载
结果不一致。

修正为 `ModelAdapter::connections()` 返回唯一、有序的完整连接描述。该顺序直接定义
connection layer Z-order，outgoing/incoming 索引由它派生。

### R3：两端枚举形成重复事实源

GEF 允许 source/target EditPart 分别枚举同一 connection，再由 Viewer registry
去重。直接复制到 Rust adapter 会产生重复、遗漏和两侧顺序冲突。

修正为单一 `ModelConnection { id, source, target }` 快照，同时保留 source/target
作为 Viewer 内部独立关系。

### R4：containment-first 删除破坏 anchor 生命周期

原事务顺序先应用 containment 删除，再清理 connection。节点与关联连接同批删除时，
Runtime anchor owner 可能先失效。

修正为联合计划：先验证 desired containment 与 connection snapshot，再解除受影响
connection binding，随后应用 containment 变化，最后创建或重绑连接。

### R5：可恢复 route 状态不应使 Viewer fault

ConnectionRuntime 已区分可恢复的 `UnresolvedConnection`。把所有 route failure
升级为 Viewer fault 会破坏依赖恢复语义。

修正为：结构、注册和 cleanup 失败可 fault；unresolved route 保留 ConnectionPart、
依赖观察和后续重试能力。

### R6：自定义 anchor API 缺少稳定 value identity

原方案允许 endpoint Part 返回 `Box<dyn ConnectionAnchor>`，同时要求 endpoint
不变时保留 AnchorId。如果连接切换 named port 但 endpoint ModelId 不变，框架无法
判断 anchor 是否等价。

修正为 G5.1 只使用 endpoint primary Figure 的 ChopboxAnchor。带稳定 value key 的
自定义 endpoint anchor descriptor 延后到 G5.3，在 reconnect/port 用例下定义。

## 3. 通过项

- ConnectionPart 不进入普通 containment children；
- connection 与 containment model identity 不能冲突；
- self-loop 使用一个 ConnectionPart，并同时进入 outgoing/incoming；
- reconnect 保留 ConnectionPartId 和 FigureId；
- connection visual 只进入 connection layer；
- Viewer drop 和节点级联删除使用 connection-first cleanup；
- opaque model event 首版触发 O(V + E) 全局协调；
- Command 不保存任何 live editor/runtime identity；
- connection-to-connection 与 connection child label 明确后置；
- G5.1 不提前引入 create/reconnect/bendpoint/auto-expose。

## 4. 风险与控制

| 风险 | 控制 |
|---|---|
| 全量协调在大图上成本偏高 | 先以正确性建立基线；有 benchmark 证据后再引入 typed hints |
| Factory/Runtime 提交阶段部分失败 | 前置纯查询和校验；不可补偿失败使 Viewer faulted |
| connection ModelId 与 node ModelId 共用类型 | 全局 model registry 冲突检查 |
| 连接删除与节点删除顺序错误 | 联合变更计划和 connection-first 自动契约 |
| 自定义端口过早固化 | G5.3 以稳定 descriptor/value key 单独设计 |

## 5. 结论

修订后的 G5.1 提案没有已知架构阻塞项，满足扩展性、稳定性、GEF 语义映射和结构化
失败要求。它仍是 `review_required` 提案；用户确认后才能进入契约测试与实现。
