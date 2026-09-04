# Scroll Pane Demo

M8 Viewport / Scroll / Zoom 的端到端验证入口。

## 运行

```bash
cargo run -p scroll-pane-demo
cargo run -p scroll-pane-demo -- --verify
cargo run -p scroll-pane-demo -- --screenshot-all
```

## 场景

| 场景 | 验证内容 |
|---|---|
| `automatic_scrollbars` | 大内容触发水平和垂直滚动条 |
| `scrolled_content` | RangeModel 驱动 viewport view location |
| `automatic_hidden` | 小内容下自动隐藏滚动条 |
| `scalable_content` | Viewport scroll 与 ScalableLayeredPane zoom 组合 |

窗口内可使用鼠标滚轮或触控板双指滚动内容，点击滚动条两端进行 step，拖动 thumb
调整位置。macOS 在 `scalable_content` 场景支持双指 pinch，并保持指针锚点下的
content 位置不变。
