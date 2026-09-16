# G5.4 Connection Bendpoint 行为验证

类型：`verification`

日期：2026-09-15

结论：`behavior_verified`。G5.4 自动契约与 headless 产品重放闭合；最终 Native 交互体验
并入 G5 检查点 C。

## 已验证行为

- Connection behavior 提供有序 bendpoints、命名 Router 与 typed constraint；
- create/move/delete 通过 `BendpointRequest -> EditPolicy -> Command -> ModelEvent`
  提交；
- feedback 清理发生在唯一 Command 执行之前；
- undo/redo 保留 ConnectionPart、endpoint 与模型顺序；
- 非有限 bendpoint 在投影提交前拒绝；
- demo self-loop 从创建起拥有两个显式 bendpoints 和两个独立 move handles；
- self-loop 两点共享外侧 x，source/target Anchor 跟随首尾 y，路径保持三段正交；
- 节点平移、reconnect、delete 与 undo/redo 保存完整 bendpoint 快照。

## 自动证据

- suite：`g5.4.connection-bendpoint`
- Editor connection editing contract：PASS
- Draw2D connection contract：PASS
- Native editor headless replay：PASS

## 剩余门禁

- 按 `doc/verification/manual/g5-connection-bendpoint.md` 完成检查点 C 合并验收。
