# G5.2 Connection Creation 人工验证

类型：`manual-verification`

状态：`pending`

验收日期：待填写

入口：

```bash
cargo xtask manual g5.2
```

窗口初始应显示蓝色节点、绿色节点和右上角 `Widget`。

## A. 两阶段创建

1. 按 `C`，确认标题栏末尾出现 `CONNECTION`。
2. 单击蓝色节点作为 source。
3. 移动鼠标，确认橙色临时连接从蓝色节点中心持续跟随光标。
4. 移到绿色节点上，确认绿色节点出现橙色 target 边框。
5. 单击绿色节点，确认临时反馈先消失，再出现一条稳定深灰连接。
6. 确认标题栏 `CONNECTION` 消失，undo 数量增加 1。

通过条件：source 在第一次点击后固定；移动过程中不改变 selection；第二次有效点击只
创建一条连接。

## B. History 与路由

1. macOS 按 `Command-Z`；Windows/Linux 按 `Control-Z`。
2. 确认连接消失，两个节点保持不变。
3. 按 `Shift-Command-Z` 或 `Shift-Control-Z`。
4. 确认同一连接恢复。
5. 拖动蓝色或绿色节点。
6. 确认稳定连接端点实时跟随节点边界重新路由。
7. 单击稳定连接并按 `Delete`，确认连接消失且应用不退出。
8. 执行 undo，确认同一连接恢复。

通过条件：create/delete 的 undo/redo 均是单个历史项；连接恢复后没有重复线；移动端点
不重建业务连接。

## C. Cancel 与非法目标

1. 按 `C`，单击蓝色节点，移动鼠标确认反馈出现。
2. 按 `Escape`，确认反馈立即消失、没有新增连接、既有 selection 保留。
3. 再按 `C` 并单击蓝色节点，然后单击空白区。
4. 确认手势仍在进行、没有新增 history；再次按 `Escape` 退出。
5. 按 `C` 后点击 `Widget`，确认不会启动连接。
6. 已有连接存在时，按 `C`、单击一个节点，再单击已有连接线。
7. 确认不会创建连接或退出应用；按 `Escape` 取消。

## D. Self-loop 与重复边

1. 按 `C`，连续两次单击蓝色节点。
2. 确认创建一条 self-loop，并且应用保持可交互。
3. 对已经存在的蓝色到绿色方向再次执行同样创建。
4. 确认重复边不提交，手势保持可取消，undo 数量不增加。
5. 按 `Escape` 清理反馈。

## E. 节点删除级联

1. 保留至少一条蓝色到绿色的稳定连接。
2. 单选蓝色节点并按 `Delete` 或 `Backspace`。
3. 确认蓝色节点及其关联连接同时消失，绿色节点保留。
4. 执行一次 undo。
5. 确认蓝色节点和连接一起恢复，连接端点与顺序正确。
6. 执行 redo，确认再次一起删除。

## F. 渲染回归

1. 连续创建、撤销、重做至少三次连接。
2. 在连接反馈显示时调整窗口大小，再继续移动鼠标并完成或取消。
3. 确认无黑块、残影、重复连接、target 边框残留或点击偏移。

## 结果模板

```text
G5.2-A two-stage create: PASS / FAIL
G5.2-B history/routing: PASS / FAIL
G5.2-C cancel/invalid target: PASS / FAIL
G5.2-D self-loop/duplicate: PASS / FAIL
G5.2-E delete cascade: PASS / FAIL
G5.2-F rendering regression: PASS / FAIL
```
