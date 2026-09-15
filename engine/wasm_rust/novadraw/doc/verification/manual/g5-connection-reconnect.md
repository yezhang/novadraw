# G5.3 Connection Reconnect 人工验证

类型：`manual-verification`

状态：`passed`

验收日期：2026-09-15

入口：

```bash
cargo run -p node-editor-demo
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

## 结果模板

```text
G5.3-A endpoint handles: PASS / FAIL
G5.3-B target reconnect: PASS / FAIL
G5.3-C source reconnect: PASS / FAIL
G5.3-D invalid/cancel: PASS / FAIL
G5.3-E regression: PASS / FAIL
```

验收结果：

```text
G5.3-A endpoint handles: PASS
G5.3-B target reconnect: PASS
G5.3-C source reconnect: PASS
G5.3-D invalid/cancel: PASS
G5.3-E regression: PASS
```
