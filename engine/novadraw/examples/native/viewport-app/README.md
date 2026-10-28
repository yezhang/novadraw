# Viewport App - Figure 树视口验证

## 功能说明

验证 `ViewportFigure` 作为 Draw2D 风格 Figure 树节点时的关键可视化行为：

- content 坐标域裁剪
- origin 滚动偏移
- `ScalableLayeredPaneFigure` zoom 缩放
- 嵌套 Viewport 父链变换

## 运行方式

```bash
cargo run -p viewport-app
```

## 场景说明

| 场景 | 名称 | 验证内容 | 目视判定 |
|------|------|----------|----------|
| 0 | clip_to_viewport | content 超出 Viewport bounds 时被裁剪 | 红色越界块只能在黑框内露出一部分，不能画到黑框外 |
| 1 | origin_scroll | `origin=(80,60)` 后 content 坐标降域 | 黄色 content 原点块不应可见，绿色块应贴近黑框左上角 |
| 2 | zoomed_content | 对场景 1 的同一份逻辑 content 设置 `scale=2.0` | 绿色锚点仍在左上角；网格偏移与矩形尺寸严格放大 2 倍，远端红块可完全移出裁剪区 |
| 3 | nested_viewports | 外层和内层 Viewport 叠加父链变换 | 内层内容只出现在灰色内框范围内，外层内容仍被外层黑框裁剪 |

## 操作说明

- 按数字键 `0`-`3` 切换场景
- 按左右方向键或 `PageUp` / `PageDown` 切换场景
- 使用 `--screenshot=<name|index>` 或 `--screenshot-all` 自动截图
- 按 `U` 切换 UpdateManager 渲染路径
- 按 `S` 保存当前场景截图
- 按 `ESC` 退出程序

## 验证重点

- 开启和关闭 UpdateManager 后画面应一致。
- 所有彩色 content 都只能出现在 Viewport 黑色边框内。
- origin / zoom 场景中，绿色锚点用于判断 content 坐标是否正确映射到 Viewport 左上角。
