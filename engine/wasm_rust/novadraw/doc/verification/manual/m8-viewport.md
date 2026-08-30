# M8 Viewport / Scroll / Zoom 手工验证

类型：`verification`

本文用于签收 M8 的 Viewport、RangeModel、ScrollPane、ScrollBar、wheel fallback
和 ScalableLayeredPane。M8 自动契约通过不等于窗口验收完成；M9 应在本文步骤全部
通过后启动。

## 1. 自动门禁

在 workspace 根目录执行：

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test

cargo run -p scroll-pane-demo -- --verify \
  --report=target/visual-verification/scroll-pane-demo.json
```

通过标准：

- 四项仓库门禁退出码均为 0。
- `scroll-pane-demo` 输出 `PASS auto_visibility`、`PASS wheel_scroll`、
  `PASS scale_chain`、`PASS pinch_anchor`。
- JSON 顶层 `"passed": true`。

## 2. Viewport 与 Scalable

启动：

```bash
cargo run -p viewport-app
```

| 步骤 | 动作 | 通过标准 |
|---|---|---|
| 1 | 按 `Home` 进入 `clip_to_viewport` | 所有 content 都被黑色 viewport 边界裁剪，红色越界块不能画到框外 |
| 2 | 按 `1` 进入 `origin_scroll` | 黄色原点块不可见，绿色参考块位于 viewport 左上区域 |
| 3 | 从场景 1 按 `2` 进入 `zoomed_content` | 场景会重新构造；绿色锚点位置不变，同一网格的偏移和矩形尺寸严格放大 2 倍，裁剪边界保持不变 |
| 4 | 按 `3` 进入 `nested_viewports` | 内层内容同时受内外两层 viewport 裁剪，父子坐标无漂移 |
| 5 | 在四个场景间往返切换 | 不出现黑帧、残影、裁剪扩大或缩放累积 |
| 6 | 调整窗口大小并最小化后恢复 | 当前场景恢复完整，无 surface 丢失或永久空白 |

## 3. ScrollPane 自动布局

启动：

```bash
cargo run -p scroll-pane-demo
```

| 步骤 | 动作 | 通过标准 |
|---|---|---|
| 1 | 按 `Home` 进入 `automatic_scrollbars` | 右侧垂直条和底部水平条同时可见，content 不覆盖 scrollbar |
| 2 | 按 `2` 进入 `automatic_hidden` | 两条 scrollbar 自动隐藏，viewport 使用完整 pane 区域 |
| 3 | 放大窗口后返回场景 0，再缩小窗口 | scrollbar visibility 随可用空间收敛，无来回抖动 |
| 4 | 按 `3` 进入 `scalable_content` | 图块按 1.5 倍显示，仍被 viewport 正确裁剪 |

## 4. Wheel、触控板、Step 与 Thumb

在 `automatic_scrollbars` 场景执行：

1. 将指针放在图块上滚轮向下。
2. 确认 content 向上移动，右侧 thumb 向下移动。
3. 在右侧 scrollbar 最下端按钮区域单击。
4. 确认 content 和 thumb 再移动一个 step。
5. 单击 thumb 下方、末端按钮上方的空白 track，确认按 page increment 滚动。
6. 按住右侧 thumb 向下拖动并在条内释放。
7. 确认拖动期间持续滚动，释放后停止；再次移动鼠标不应继续滚动。
8. 滚动到末端后继续滚轮和点击，确认位置被 clamp，不越出内容范围。
9. 使用 macOS 触控板双指纵向和横向移动，确认内容连续跟手且不按 24 px 行高跳动。
10. 在 `scalable_content` 场景将指针放在一个网格交点上执行双指缩放。
11. 确认缩放过程中该网格交点保持在指针下，缩放结束后滚轮和鼠标拖动仍可用。
12. 持续缩小和放大，确认 ZoomManager 分别停在默认最小 `0.5` 和最大 `4.0`。
13. 缩放后分别拖到水平/垂直范围两端，确认画板左上角和右下角都能完整到达。
14. 缩小到画板小于 viewport，确认画板按 Draw2D 语义保持左上对齐且不越过裁剪边框；
    再重新放大并重复步骤 13，确认滚动范围没有累积漂移。
15. 鼠标按住可拖动目标时使用触控板滚动，确认 pointer capture 与 gesture session
    不互相抢占。

通过标准：

- wheel 在普通 content 上未被消费时，由最近的 ScrollPane 处理。
- 触控板 PixelDelta 按逻辑像素滚动，鼠标 LineDelta 按 line step 滚动。
- pinch 更新 scalable scale 与 viewport view location 时保持入口锚点不动。
- pinch 返回后 RangeModel 立即使用缩放后的内容范围，不依赖下一帧修正。
- ScrollBar 与 Viewport 始终显示同一个 RangeModel 值。
- thumb 尺寸随 extent/maximum 比例变化。
- 释放鼠标后 capture 清除。
- 每次滚动只重绘 pane/viewport 区域，不留下旧内容残影。

## 5. UpdateManager 等价

在两个 app 的每个场景中按 `U` 切换 UpdateManager 路径：

- 切换前后几何、裁剪、scroll position 和 zoom 必须一致。
- 切换后继续滚轮、点击 scrollbar 和调整窗口，行为必须保持一致。
- 不允许出现一次性透明帧、黑帧或旧 scrollbar 残影。

## 6. 截图留存

```bash
cargo run -p viewport-app -- --screenshot-all
cargo run -p scroll-pane-demo -- --screenshot-all
```

应分别生成 4 张截图。至少检查：

- `viewport-app_zoomed_content`
- `viewport-app_nested_viewports`
- `scroll-pane-demo_automatic_scrollbars`
- `scroll-pane-demo_automatic_hidden`
- `scroll-pane-demo_scalable_content`

静态截图不能替代第 4 节的 wheel 和 thumb 交互验证。

## 7. 验收记录

```text
Commit:
操作系统:
图形设备 / 后端:

[ ] 自动门禁全部通过
[ ] scroll-pane-demo verification 4/4 PASS
[ ] viewport-app 四场景通过
[ ] ScrollPane 自动显示/隐藏通过
[ ] wheel fallback 通过
[ ] 触控板双指横纵滚动与 pinch 锚点缩放通过
[ ] scrollbar step/page/thumb 通过
[ ] scalable pane zoom 通过
[ ] resize/最小化恢复通过
[ ] UpdateManager 开关前后等价

失败步骤与复现:
结论: PASS / FAIL
```
