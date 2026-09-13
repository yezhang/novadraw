# G1 Model Adapter 与 CommandStack 完成记录

类型：`verification`

日期：2026-09-13

状态：`complete`

## 1. 范围

G1 固定 Editor 最底层的模型与历史事务边界，不引入 EditPart、Viewer、Tool、
Request、Policy、Figure mutation 或平台代码。

受影响 API family：

- `model.identity`；
- `model.notification`；
- `command.protocol`；
- `command.stack`；
- `command.dirty_state`。

## 2. Model 契约

- 应用通过 `ModelAdapter` 保持模型所有权；
- `ModelId` 必须 Copy/Eq/Hash/Debug 且具有应用稳定语义；
- `ModelRevision` 从 1 开始、拒绝 0、到 `u64::MAX` 后结构化失败；
- `ModelEvent` 保存 revision、subject 与 typed payload；
- adapter 提供 root、稳定 direct-child order 与 ordered event drain。

G1 不消费通知，因此 revision gap、批次稳定发布和重放错误留在 G2 Viewer projection
中闭合。`model.notification` 在覆盖账本保持 `partial`。

## 3. Command 契约

`Command<M>` 只操作应用模型，不引用 FigureId、EditPartId、Runtime 或平台对象。

- canExecute/canUndo/canRedo 是显式 capability，拒绝不会移动 history；
- execute/undo/redo 返回 recoverable error 时，扩展实现必须保持调用前状态；
- 应用可以用 `CommandError::state_unknown` 明确报告无法保证前态；
- `CompoundCommand` 顺序 execute/redo、逆序 undo；
- compound 中途失败会补偿已执行前缀；
- 补偿失败升级为 unknown-state error。

## 4. CommandStack 契约

- 只有成功 execute 才进入 undo history；
- rejected 或 recoverable execute failure 不清除 redo；
- undo/redo recoverable failure 保留原 history 位置；
- 新 execute 成功后清空 redo；
- undo limit 丢弃最旧 Command，不修改模型；
- flush drop 全部 history 并将当前模型视为新的 clean baseline；
- dirty/save 使用单调 history identity，分支后不会因相同栈深误判 clean；
- committed event 按 execute/undo/redo/save/flush 因果顺序排队。

扩展 panic、unknown-state error 或 compound 补偿失败使 CommandStack faulted。Faulted
stack 视为 dirty，并拒绝 execute、undo、redo、flush 和 save marking；Host 必须重建
模型/editor，而不是继续使用未知状态。

## 5. 自动验证

```text
cargo test -p novadraw-editor --tests: PASS (16 passed)
cargo clippy -p novadraw-editor --all-targets -- -D warnings: PASS
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
```

覆盖：

- revision zero/exhaustion；
- model identity、children order、event drain；
- execute/undo/redo 与 committed events；
- canExecute/canUndo/canRedo rejection；
- rejected/failed execute 与 redo preservation；
- history identity dirty/save/branch；
- failed undo/redo history preservation；
- compound rollback 与 reverse undo；
- compound undo compensation；
- compensation failure fault；
- undo limit；
- flush/drop；
- extension query/execute panic fault；
- explicit unknown model state fault。

## 6. 单测工作流说明

测试按 `bits-unit-test-gen` 的 Step1-Step6 生成流程执行。Step6 的 `utree flush` 因
sandbox 拒绝写入 skill 安装目录 `.skill_update_*` 而未能落盘；测试源码和 Cargo
验证不受影响。未发现存量业务缺陷，首轮编译失败仅用于证明 G1 公共 API 尚未实现，
随后由正式实现闭合。
