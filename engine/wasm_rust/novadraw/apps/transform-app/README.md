# Transform App - M4 坐标域验证

`transform-app` 是 M4 坐标域与变换闭环的端到端验证入口。它只验证
Draw2D Figure 父链坐标协议，不承载通用矩阵动画或 M8 Viewport 能力。

## 运行

```bash
cargo run -p transform-app
cargo run -p transform-app -- --screenshot-all
cargo run -p transform-app -- --screenshot=2
```

## 场景

| 场景 | 名称 | 验证内容 |
|---|---|---|
| 0 | `nested_coordinate_roots` | 两层坐标根与 border insets 的渲染位置 |
| 1 | `coordinate_roundtrip_overlay` | local → absolute → local 往返；红色绝对域描边应与白色本地域描边重合 |
| 2 | `coordinate_root_move` | 坐标根移动和 resize 后，子 bounds 保持局部值不变；灰框标记旧区域 |
| 3 | `event_point_reduction` | 点击红色目标后出现选中框，验证入口域命中与 target 域事件点一致 |

对应自动契约测试位于
`novadraw-scene/tests/m4_coordinate_contract.rs`。
