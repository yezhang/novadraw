# G4 Tool / Request / EditPolicy 编辑闭环人工验收

类型：`manual-verification`

状态：`passed`

验收日期：2026-09-14

入口：

```bash
cargo run -p node-editor-demo
```

窗口初始应显示蓝色节点、绿色节点和右上角 `Widget`，标题栏包含
`0 selected | undo 0 redo 0`。

## A. Create 与 History

1. 按 `N`。
2. 确认右下区域新增紫色节点，标题中的 undo 数量增加。
3. macOS 按 `Command-Z`；Windows/Linux 按 `Control-Z`。
4. 确认新节点消失且 redo 数量增加。
5. 按 `Shift-Command-Z` 或 `Shift-Control-Z`。
6. 确认同一节点恢复到原位置和尺寸。

## B. Move 与 Feedback

1. 点击蓝色节点。
2. 从节点内部按下并拖动，暂不释放。
3. 确认橙色 feedback 边框跟随光标，原节点在 command 前保持原位。
4. 释放鼠标，确认 feedback 先消失，节点再移动到预览位置。
5. 执行 undo/redo，确认节点分别回到原位和移动后位置。

## C. 多选 Compound Move

1. 点击蓝色节点，再按住 `Shift` 点击绿色节点。
2. 不按 modifier，直接拖动任一已选节点。
3. 确认两个 feedback 保持相同位移。
4. 释放后确认两个节点一起移动，selection 仍为两个节点。
5. 执行一次 undo，确认两个节点同时回到原位，而不是只恢复一个。
6. 单击其中一个节点但不拖动，确认 selection 折叠为该单个节点。

## D. Resize

1. 单选任一普通节点。
2. 拖动黄色角 handle。
3. 确认橙色 resize feedback 跟随对应角，其他角保持正确锚定。
4. 释放后确认节点尺寸和 handle 位置同步更新。
5. 执行 undo/redo，确认位置和尺寸都准确恢复。
6. 尝试把节点缩小到极小尺寸，确认操作被拒绝但应用不退出、feedback 能清除。

## E. Delete

1. 单选一个普通节点并按 `Delete` 或 `Backspace`。
2. 确认节点与 handle 消失，selection 自动 reconcile。
3. undo 后确认节点恢复；redo 后确认再次删除。
4. 多选两个普通节点后执行删除，再执行一次 undo。
5. 确认两个节点及原有顺序一次性恢复。

## F. 输入隔离与渲染回归

1. 点击并拖出 `Widget` 后释放，确认没有启动 move Tool，也没有新增 history。
2. 在 move/resize 中按 `Escape`，确认 feedback 清除且模型不变；既有 selection 保留。
3. 调整窗口大小后重复 create/move/resize/delete。
4. 确认没有黑区、描边残留、handle 错位或命中偏移。

## 通过标准

```text
G4-A create/history: PASS
G4-B move/feedback: PASS
G4-C compound move: PASS
G4-D resize: PASS
G4-E delete/history: PASS
G4-F arbitration/rendering: PASS
```
