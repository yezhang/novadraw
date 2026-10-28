# G3 Selection / Targeting / 输入仲裁人工验收

类型：`manual-verification`

状态：`complete`

入口：

```bash
cargo run -p node-editor-demo
```

窗口初始应显示：

- 左侧蓝色节点；
- 中部绿色节点；
- 右上角 `Widget` 按钮；
- 标题栏显示 `0 selected`。

## A. 单选与清空

1. 点击蓝色节点内部。
2. 确认蓝色节点四角出现黄色 handle，标题变为 `1 selected`。
3. 点击绿色节点内部。
4. 确认 handle 从蓝色节点移到绿色节点，仍为 `1 selected`。
5. 点击空白画布。
6. 确认全部 handle 消失，标题恢复 `0 selected`。

## B. 多选、Primary 与 Toggle

1. 点击蓝色节点。
2. 按住 `Shift` 点击绿色节点。
3. 确认蓝色节点四角为青色 handle，绿色节点四角为黄色 handle，标题为
   `2 selected`；黄色表示 primary selection。
4. macOS 按住 `Command` 点击蓝色节点；Windows/Linux 按住 `Control` 点击蓝色节点。
5. 确认蓝色节点被移出 selection，绿色节点保持黄色 primary handle，标题为
   `1 selected`。

## C. Widget 输入隔离

1. 保持绿色节点已选中。
2. 按下右上角 `Widget` 按钮并拖出按钮范围后释放。
3. 确认按钮在按下期间呈现 pressed 状态。
4. 确认绿色节点 selection、handle 和标题计数均未变化。

该步骤验证 Figure handler consumed/capture 优先，Editor selection fallback 不启动。

## D. Handle Targeting 与窗口 Resize

1. 点击任一黄色或青色 handle。
2. 确认 selection 不被清空，也不会切换到其他节点。
3. 调整窗口大小。
4. 确认节点、Widget 与 handle 均保持可见，没有黑帧、残影或坐标偏移。
5. 再次点击节点和空白区，确认 resize 后 targeting 仍与光标位置一致。

## 通过标准

```text
G3-A single/clear: PASS
G3-B multi/primary/toggle: PASS
G3-C widget arbitration/capture: PASS
G3-D handle targeting/resize: PASS
```

2026-09-13 人工验收结果：全部 PASS。

验收期间发现 selection handle 删除后的 retained partial frame 可能用透明 scratch
覆盖不透明背景，表现为描边残留或矩形黑区。修复后重复执行单选清空和多选 toggle，
均无需 resize 即恢复正确背景与描边。
