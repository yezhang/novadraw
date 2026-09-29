# Advanced Figures App

高级 Figure 与路由能力的 Native 可视样例，覆盖连接装饰、障碍避让路由、可缩放
多边形和只读 TextFlow。

## 运行

```bash
cargo run -p advanced-figures-app
```

使用数字键 `0` 到 `3`、左右方向键或 `PageUp` / `PageDown` 切换场景：

| 键 | 场景 ID | 验证目标 |
|---|---|---|
| `0` | `connection-decoration` | 端点装饰方向与切线/法线偏移 |
| `1` | `shortest-path-routing` | 障碍避让、正交路径与批量 snapshot |
| `2` | `scalable-polygon` | Stretch、PreserveAspect、alignment 与 bounds mutation |
| `3` | `text-flow` | 段落、fragment、换行、CJK 与截断 |

完整检查项见
[`../../../doc/verification/manual/advanced-figures.md`](../../../doc/verification/manual/advanced-figures.md)。

## 截图

```bash
cargo run -p advanced-figures-app -- --screenshot=connection-decoration
cargo run -p advanced-figures-app -- --screenshot=shortest-path-routing
cargo run -p advanced-figures-app -- --screenshot=scalable-polygon
cargo run -p advanced-figures-app -- --screenshot=text-flow
cargo run -p advanced-figures-app -- --screenshot-all
```

图片写入 `target/visual-verification/screenshots/`。
