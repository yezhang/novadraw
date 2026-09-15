# G5.4 Connection Bendpoint 人工验证

类型：`manual-verification`

状态：`pending`

入口：

```bash
cargo run -p node-editor-demo
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
2. 确认默认回环的每条线段中点都有青色 create handle。
3. 确认自动路由生成的拐角没有橙色 move handle；它们不是模型 bendpoint。
4. 使用任一青色 handle 创建一个显式 bendpoint。
5. 确认应用注册的 bendpoint Router 保持 self-loop 两端独立。
6. 删除最后一个 bendpoint，确认恢复应用安装的外侧 self-loop Router。
7. 验证 endpoint reconnect、connection delete/undo 仍正常。

## 结果模板

```text
G5.4-A create bendpoint: PASS / FAIL
G5.4-B move/delete bendpoint: PASS / FAIL
G5.4-C multi-bendpoint/history: PASS / FAIL
G5.4-D self-loop/regression: PASS / FAIL
```
