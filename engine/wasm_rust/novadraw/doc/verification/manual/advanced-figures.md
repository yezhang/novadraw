# 高级 Figure 与路由人工验收

类型：`manual-verification`

## 验收状态

- 日期：2026-09-29
- 平台：macOS Native / Vello
- 结果：PASS
- 覆盖：Connection Decoration、Shortest Path Routing、Scalable Polygon、Text Flow
- 失败项：无

## 范围

本入口验证连接装饰、障碍感知最短路径、可缩放多边形与只读 TextFlow 的
Native/Vello 可视结果。算法边界、失败原子性、Bidi、UTF-8 安全性与增量失效仍由
对应自动 suite 判定。

## 启动

可从任一对应 suite 启动同一个应用：

```bash
cargo xtask manual core.p2-c01-connection-decoration
cargo xtask manual core.p2-c02-shortest-path-routing
cargo xtask manual core.p2-f01-scalable-polygon
cargo xtask manual core.p2-t01-text-flow
```

也可直接运行：

```bash
cargo run -p advanced-figures-app
```

数字键 `0` 到 `3` 直接切换场景；左右方向键和 `PageUp` / `PageDown` 循环切换；
`S` 保存当前帧，`Esc` 退出。

## 0：Connection Decoration

1. 蓝色连接从左上 source 指向右下 target。
2. source 端是蓝色开放箭头，target 端是红色填充箭头；两者都沿连接切线定向。
3. 蓝色中心线在两个装饰前结束，不穿过箭头尖端。
4. 红色 `offset +u / +v` 标签位于 target 下方，与节点边框无重叠。
5. 两个 miter 箭头轮廓完整，没有被 Figure bounds 或面板裁剪。

## 1：Shortest Path Routing

1. 蓝色 A 路径从障碍上方绕行，绿色 B 路径从障碍下方绕行。
2. 每条路径只包含水平和垂直线段，不穿越灰色障碍，也不贴住障碍边框。
3. 连接端点准确落在 source/target 边界，没有进入节点内部。
4. 底部统计显示 `2 routes / 1 obstacle snapshot`。

## 2：Scalable Polygon

1. 四个图形具有相同的不对称五边形模板，所有粗描边和锐角都完整可见。
2. 蓝色 Stretch 图形按宽高独立拉伸；绿色和橙色图形保持模板宽高比。
3. 绿色图形在可用 bounds 内居中；橙色图形经 Runtime mutation 后靠右、靠上。
4. 下方红色图形经过宽 bounds mutation 后仍保持宽高比，没有被横向拉扁。

## 3：Text Flow

1. 左侧第一段的多个 inline fragment 连续排版，不产生额外换行。
2. 第二段从新行开始；中英文和数字字形完整，没有缺字方框。
3. 左侧内容在面板宽度内稳定软换行，不越过面板边界。
4. 右上内容最多两行，并以 `...` 结束；省略位置没有破坏中文字符。
5. 右下 NoWrap 内容保持单行。

## 截图复核

```bash
cargo run -p advanced-figures-app -- --screenshot-all
```

截图输出到 `target/visual-verification/screenshots/`。也可用场景 ID 捕获单项：

```bash
cargo run -p advanced-figures-app -- --screenshot=shortest-path-routing
```

## 失败条件

任一场景出现崩溃、空白帧、缺字方框、路径穿越障碍、斜向 shortest-path 线段、装饰
方向错误、描边裁剪、文字越界或场景切换残影，均判定人工验收失败。
