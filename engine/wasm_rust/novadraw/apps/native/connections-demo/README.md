# Connections Demo

M9 Connection / Anchor / Router 人工验收入口。

## 运行

```bash
cargo run -p connections-demo
```

数字键 `0` 到 `5` 切换场景：

| 键 | 场景 | 验证目标 |
|---|---|---|
| `0` | `anchor_matrix` | Chopbox、Ellipse、RoundedRectangle、Label、XY Anchor |
| `1` | `bendpoint` | absolute / relative Bendpoint 与箭头方向 |
| `2` | `manhattan` | 正交路由、无重复段 |
| `3` | `fan` | 五条平行连接的稳定扇出 |
| `4` | `moved_nodes` | 节点位置变化后的 Manhattan 结果 |
| `5` | `connection_layer` | ConnectionLayer 默认 Router 继承 |

## 截图

```bash
cargo run -p connections-demo -- --screenshot=anchor_matrix
cargo run -p connections-demo -- --screenshot=bendpoint
cargo run -p connections-demo -- --screenshot=manhattan
cargo run -p connections-demo -- --screenshot=fan
cargo run -p connections-demo -- --screenshot=moved_nodes
cargo run -p connections-demo -- --screenshot=connection_layer
```

图片写入 `target/visual-verification/screenshots/`。
