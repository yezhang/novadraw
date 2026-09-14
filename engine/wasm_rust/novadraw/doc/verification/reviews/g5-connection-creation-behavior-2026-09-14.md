# G5.2 Connection Creation 行为验证

类型：`verification`

日期：2026-09-14

状态：`behavior_verified`

自动门禁：`PASS`

人工验收：`pending`

## 1. 范围

- typed `CreateConnectionRequest`；
- source-locked `ConnectionCreation` policy plan；
- two-press `ConnectionCreationTool`；
- target validation、feedback replacement 与 cleanup；
- model-only create Command、undo/redo；
- G5.1 ConnectionPart/Runtime 投影复用；
- Native `node-editor-demo` 连接创建入口；
- 节点删除时关联连接的模型级联与恢复。

设计契约：
[`../../design/editor/g5-connection-creation.md`](../../design/editor/g5-connection-creation.md)。

## 2. 实现结论

连接创建遵循：

```text
C arm
-> first press locks source Part and source ModelId plan
-> move resolves target and replaces feedback
-> second valid press clears feedback
-> one model Command enters CommandStack
-> model notification
-> G5.1 creates the stable ConnectionPart and Runtime route
```

Request 只保存 Viewer 交互身份与值数据。手势期 `ConnectionCreation` 计划只保存应用
模型身份，不进入 history；最终 Command 不保存 EditPart/Figure/Runtime identity。

保留 GEF 的 start/end 两阶段和 source lock，但不把 mutable start Command 放入可克隆
Request。这个差异避免 trait object 混入主协议，并让最终历史项保持原子。

## 3. 已验证行为

- connection mode 不改变既有 selection；
- Figure-native widget consumed press 不启动或完成连接；
- invalid target、contents、handle 和已有 connection visual 不提交模型；
- self-loop 在框架层合法，应用 policy 可自行限制；
- 多个 source policy 同时接受时显式拒绝；
- 每次 move 先清旧 feedback，再安装最新 source/target feedback；
- 成功提交前清空 feedback；
- Escape、tool switch、普通 command、undo/redo 前取消未完成手势；
- create/undo/redo 保持 connection ModelId 与规范顺序；
- 选中连接后 Delete 通过 connection-specific policy 删除，undo 恢复同一模型身份；
- 已提交连接只由 G5.1 投影一次；
- Demo 删除节点时由模型 Command 同事务删除关联连接，undo 恢复原顺序。

## 4. 自动证据

`novadraw-editor/tests/g5_connection_creation_contract.rs` 共 10 项：

1. typed request 字段；
2. source lock、selection 隔离、feedback cleanup 与提交；
3. invalid target 与 Escape；
4. widget 输入隔离；
5. duplicate source plan；
6. create/undo/redo identity；
7. self-loop；
8. existing connection visual 作为 invalid target；
9. history transition 取消半成品手势；
10. selected connection delete 与 undo identity。

全量门禁：

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
git diff --check: PASS
```

## 5. 阶段边界

`connection.create` 已达到 `verified`。G5 整体仍为 `in_progress`：

- G5.3 reconnect 与 endpoint handle 尚未实现；
- G5.4 bendpoint 尚未实现；
- G5.5 viewport/zoom feedback 与 auto-expose 尚未实现；
- 检查点 C 尚未开放。

本次 Native 人工验证只确认 G5.2 产品入口和视觉行为，不替代完整 G5 检查点 C。
