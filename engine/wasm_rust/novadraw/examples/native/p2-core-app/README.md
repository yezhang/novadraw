# Core P2 Validation App

P2-C01、P2-C02、P2-F01 与 P2-T01 的 Native 可视验证入口。

## 运行

```bash
cargo run -p p2-core-app
```

使用数字键 `0` 到 `3`、左右方向键或 `PageUp` / `PageDown` 切换场景：

| 键 | 场景 ID | 验证目标 |
|---|---|---|
| `0` | `p2-c01-connection-decoration` | 端点装饰方向与切线/法线偏移 |
| `1` | `p2-c02-shortest-path` | 障碍避让、正交路径与批量 snapshot |
| `2` | `p2-f01-scalable-polygon` | Stretch、PreserveAspect、alignment 与 bounds mutation |
| `3` | `p2-t01-text-flow` | 段落、fragment、换行、CJK 与截断 |

完整检查项见
[`../../../doc/verification/manual/p2-core-features.md`](../../../doc/verification/manual/p2-core-features.md)。

## 截图

```bash
cargo run -p p2-core-app -- --screenshot=p2-c01-connection-decoration
cargo run -p p2-core-app -- --screenshot=p2-c02-shortest-path
cargo run -p p2-core-app -- --screenshot=p2-f01-scalable-polygon
cargo run -p p2-core-app -- --screenshot=p2-t01-text-flow
cargo run -p p2-core-app -- --screenshot-all
```

图片写入 `target/visual-verification/screenshots/`。
