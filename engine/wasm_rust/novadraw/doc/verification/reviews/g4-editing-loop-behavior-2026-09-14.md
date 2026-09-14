# G4 Tool / Request / EditPolicy 编辑闭环行为记录

类型：`verification`

日期：2026-09-14

状态：`complete`

人工门禁：`PASS`

## 1. 范围

G4 建立第一个可用编辑事务闭环：

```text
normalized input
-> SelectionTool / drag tracker
-> typed EditorRequest
-> role-keyed EditPolicy
-> Command / CompoundCommand
-> application model notification
-> GraphicalViewer refresh
```

Tool 不直接修改应用模型或 Part 主视觉。Feedback 是显式 layer 中的临时 Figure，
并在 command 执行前清理。

## 2. 公开契约

- `InteractionRevision`、`RequestModifiers` 与 closed `EditorRequest`；
- `ChangeBoundsRequest`、`CreateRequest`、`DeleteRequest`；
- `PolicyRole`、`EditPolicy`、`PolicyHost` 与 `FeedbackVisual`；
- 显式区分 policy rejection 和 no contribution；
- `EditorDomain` 持有 active `SelectionTool` 与 `CommandStack`；
- `HandleRole::Resize` 将 handle 视觉身份与 tracker 语义关联；
- `DomainPointerRelease` 报告 dispatch 与 command commit。

与 GEF 相同的是 Request 只表达意图、Policy 贡献 Command、Command 只改模型。与 GEF
不同的是 Novadraw 使用 typed enum/struct 代替 `Object type + extendedData`，并使用
结构化错误代替 `null` / `UnexecutableCommand` 混合协议。

## 3. 行为闭环

- press 固定 source parts 与 tracker kind；
- Figure handled/capture 时 SelectionTool 不启动；
- move 只重建 policy feedback；
- release/cancel 先删除 feedback，再解析并执行 Command；
- 多选 move 由确定顺序的 `CompoundCommand` 执行；
- create/delete 由 layout/component policy 贡献；
- execute/undo/redo 后只通过有序 model notification 刷新 Part；
- plain press 已选节点保留 operation set，click without drag 才折叠单选；
- resize command 拒绝小于示例最小尺寸的结果，拒绝不进入 history。

## 4. 自动验证

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
git diff --check: PASS
```

`novadraw-editor/tests/g4_editing_loop_contract.rs` 共 7 项，覆盖：

- typed bounds transform 与 revision；
- deterministic multi-part Compound move；
- Policy rejection 与 no contribution 区分；
- create/delete 与 Part 增量投影；
- move feedback cleanup 与 release commit；
- typed resize handle；
- multi-selection drag 与 click collapse；
- execute/undo/redo 的模型和视觉一致性。

## 5. Native Probe

`apps/native/node-editor-demo` 已接入真实 `EditorDomain`：

- drag 普通节点执行 move；
- 黄色 corner handle 执行 resize；
- `N` 执行 create；
- `Delete/Backspace` 执行单选或多选 delete；
- `Command/Control-Z` 与加 `Shift` 执行 undo/redo；
- `Widget` 继续验证 Figure consumed/capture 优先。

人工步骤见
[`../manual/g4-editing-loop.md`](../manual/g4-editing-loop.md)。检查点 B 已于
2026-09-14 通过人工确认。

人工验收期间修复了两项运行时问题：

- Native `CursorMoved` 在 Tool 更新 feedback 后请求下一帧，`Escape` 清理 feedback
  后同样请求重绘；
- retained partial 渲染的 clip 向外对齐到纹理复制使用的整数设备像素边界，避免
  scratch 背景色覆盖 damage 逻辑边界外的 retained 像素。

最终确认 move/resize feedback 实时且流畅，release/cancel 清理正确，重叠节点无
背景色细描边。

## 6. 明确后置

- marquee selection 后置 G6+；
- 专用 CreationTool / palette 后置 G6+，当前 create 仍完整经过 typed Request/Policy；
- reparent 与跨容器 bounds change 后置 G6+；
- connection delete cascade、连接创建/重连与 bendpoint 归入 G5；
- viewport/zoom feedback 与 auto-expose 归入 G5。
