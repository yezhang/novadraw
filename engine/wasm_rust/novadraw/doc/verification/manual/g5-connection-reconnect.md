# G5.3 Connection Reconnect 人工验证

类型：`manual-verification`

状态：`passed`

验收日期：2026-09-20

入口：

```bash
cargo xtask manual g5.3
```

## A. Endpoint Handles

1. 按 `C`，依次点击蓝色和绿色节点创建连接。
2. 单击稳定连接线。
3. 确认连接两端各出现一个黄色方形 handle，节点 resize handle 不出现。

结果：`PASS`。普通连接与 self-loop 均显示两个独立 endpoint handle；self-loop
handle 绑定已提交 route 的首尾点，不再重叠于节点中心。

## B. Target Reconnect

1. 按住绿色端的 handle，拖到蓝色节点后释放。
2. 确认拖动中橙色反馈保留蓝色端并跟随鼠标。
3. 确认释放后连接变为蓝色到蓝色 self-loop，连接 identity 和列表位置保持。
4. 执行 undo/redo，确认端点在绿色与蓝色之间往返。

## C. Source Reconnect

1. undo/redo 恢复蓝色到绿色连接。
2. 按住蓝色端 handle，拖到绿色节点后释放。
3. 确认连接变为绿色到绿色 self-loop。
4. 执行 undo，确认恢复蓝色到绿色。

## D. Invalid Drop 与 Cancel

1. 拖动任一 endpoint handle 到空白区或 `Widget` 后释放。
2. 确认模型连接不变、feedback 清除、无新增 history。
3. 再次拖动 endpoint，途中按 `Escape`。
4. 确认 feedback 清除、连接不变。

## E. 回归

1. 重连后拖动任一节点，确认连接重新路由。
2. 选中连接并按 `Delete`，确认不崩溃；undo 恢复连接。
3. 确认无残留 handle、橙色 feedback、黑块或重复连接。

## F. Self-loop 跨节点迁移

1. 创建蓝色 self-loop，确认两个黄色 endpoint handle 和两个显式橙色 bendpoint handle。
2. 将其中一个 endpoint handle 拖到绿色节点，确认连接变为蓝绿普通连接，既有
   bendpoint 列表保持稳定。
3. 将仍位于蓝色节点的另一个 endpoint handle 拖到绿色节点。
4. 确认连接变为绿色 self-loop，两个 bendpoints 已重建到绿色节点右侧，路径为水平、
   垂直、水平三段正交线，不得保留蓝色节点坐标域中的旧绝对位置。
5. 拖动绿色节点，确认 self-loop 与两个 bendpoints 整体同步平移。
6. 连续 undo/redo 两次，确认蓝色 self-loop、蓝绿普通连接、绿色 self-loop 按历史顺序
   恢复，且每个状态的 endpoint 与 bendpoint 一致。

## 结果模板

```text
G5.3-A endpoint handles: PASS / FAIL
G5.3-B target reconnect: PASS / FAIL
G5.3-C source reconnect: PASS / FAIL
G5.3-D invalid/cancel: PASS / FAIL
G5.3-E regression: PASS / FAIL
G5.3-F self-loop owner migration: PASS / FAIL
```

验收结果：

```text
G5.3-A endpoint handles: PASS
G5.3-B target reconnect: PASS
G5.3-C source reconnect: PASS
G5.3-D invalid/cancel: PASS
G5.3-E regression: PASS
G5.3-F self-loop owner migration: PASS
```

2026-09-20 复验确认跨 owner self-loop 的最终 route 与拖放中橙色 feedback 均使用新
owner 的 endpoint 和 bendpoint 几何。
