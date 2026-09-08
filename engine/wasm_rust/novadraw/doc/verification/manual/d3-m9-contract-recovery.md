# D3.1 M9 契约恢复人工验收

类型：`verification`

## 启动

```bash
cargo run -p connections-demo
```

使用数字键切换到：

- `3`：`shared_manhattan`
- `7`：`unsupported_viewport_topology`

## shared_manhattan

检查：

- 左右节点之间存在四条正交连接；
- 四条连接的内部垂直 lane 清晰分离，能逐条辨认；
- 水平首尾 stub 允许完全重合，这是相同 Anchor endpoint 下的预期表现；
- 每条连接的端点落在节点边缘；
- 没有斜线、零长度折返、箭头错位或超出窗口的路径。

## unsupported_viewport_topology

检查：

- 左侧蓝色 viewport 内显示绿色 source；
- 右侧 root 上显示红色 target；
- 两个节点之间没有连接线、残留箭头或旧路径像素；
- 场景切换前后没有崩溃或脏区残影。

## 通过条件

两个场景全部符合预期。通过后：

- D3.1 标记 `complete`；
- M9.4、M9.6 与 M9 恢复 `complete`；
- 当前执行阶段进入 D3.2 Runtime 动态 mutation 公共面。
