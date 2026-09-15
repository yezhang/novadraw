# G5.4 Connection Bendpoint 人工验证

类型：`manual-verification`

状态：`pending`

入口：

```bash
cargo xtask manual g5.4
```

## A. 创建 Bendpoint

1. 创建蓝色到绿色连接并选中连接。
2. 确认 endpoint handles 之外，线段中点存在较小的青色 create handle。
3. 将青色 handle 拖到连接线之外。
4. 确认出现橙色折线路径反馈，释放后创建一个橙色 move handle。

## B. 移动与删除

1. 拖动橙色 move handle，确认路径实时跟随。
2. 释放后确认 bendpoint 固定在新位置。
3. 再次拖动该点回到相邻两点构成的直线附近。
4. 释放后确认 bendpoint 被删除，路径恢复无显式折点状态。

## C. 多 Bendpoint 与 History

1. 连续使用青色 handles 创建两个 bendpoints。
2. 分别移动两个橙色 handles，确认索引与路径顺序稳定。
3. 连续 undo/redo，确认每次只回退一个 create/move/delete 操作。
4. 确认 connection identity、endpoint 和 connection layer 顺序不变。

## D. Self-loop 与回归

1. 创建或重连为 self-loop。
2. 确认 self-loop 从创建完成起就拥有两个橙色 move handles；它们是模型中的两个显式
   bendpoints，不是自动 Router 的视觉代理。
3. 确认两个橙色 handles 共享节点右侧的外侧 x，路径为水平、垂直、水平三段直角线。
4. 确认三个线段中点仍分别显示青色 create handle，两个黄色 endpoint handles 保持独立。
5. 拖动任一橙色 handle，确认另一个橙色 handle 与对应拐角仍保留，且路径继续保持三段
   直角线，不会退化为单 bendpoint 路径。
6. undo/redo 该移动，确认每次只恢复被移动的拐角，路径始终保持正交。
7. 移动 self-loop 所属节点，确认两个 bendpoints 与回环整体同步平移。
8. 验证 endpoint reconnect、connection delete/undo 仍正常且完整恢复 bendpoint 快照。

## 结果模板

```text
G5.4-A create bendpoint: PASS / FAIL
G5.4-B move/delete bendpoint: PASS / FAIL
G5.4-C multi-bendpoint/history: PASS / FAIL
G5.4-D self-loop/regression: PASS / FAIL
```
