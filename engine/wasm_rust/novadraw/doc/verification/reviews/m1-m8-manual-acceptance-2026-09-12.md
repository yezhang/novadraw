# M1-M8 核心管线与 Viewport 验收报告

类型：`verification`

日期：2026-09-12

状态：`complete`

代码基线：`7608f0c`（包含 `c9cdccd` logical viewport 增量）

## 1. 范围

本次验收关闭 M1-M6 与 M8 从 `behavior_verified` 到 `complete` 的产品和人工验证
差额。M7 已在 D3.3 完成 Runtime listener 公共面收口，本次随核心管线重新回归。

验收入口：

- [`../manual/core-pipeline.md`](../manual/core-pipeline.md)
- [`../manual/m8-viewport.md`](../manual/m8-viewport.md)
- [`macos-live-resize-transaction-2026-09-12.md`](macos-live-resize-transaction-2026-09-12.md)

平台：

- Native：macOS 14.7.8 (23H730), arm64, Vello/Metal；
- Web：Chrome, Vello WebGPU；
- Rust：1.94.1。

## 2. 自动门禁

以下命令全部通过：

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

无窗口 verification：

| 应用 | 结果 | 覆盖 |
|---|---|---|
| `update-app --verify` | 6/6 PASS | damage、通知、dirty 合并、panic 恢复、1,024 Figure、submission lifecycle |
| `event-app --verify` | 4/4 PASS | capture、focus/key、wheel/hover/double-click、target-domain 降域 |
| `scroll-pane-demo --verify` | 7/7 PASS | visibility、wheel、scale、pinch、freeform range/layer/zoom |

定向契约包括：

- `runtime_resize_contract` 3 项；
- `m1_product_existence`；
- `m2_product_existence`；
- `m4_coordinate_contract`；
- `m5_layout_contract` 10 项；
- `m6_event_contract` 2 项；
- `m8_viewport_contract` 24 项；
- `responsive_clip_resizes_parent_without_rewriting_child_bounds`；
- `root_viewport_resize_reflows_border_regions`。

## 3. M1-M7 Native 人工结果

### Update

- baseline 图形完整；
- 缩放窗口无空白帧；
- partial damage 能清除旧位置；
- 场景往返无残影；
- validation 布局稳定；
- 32 x 32 stress 网格完整。

### Event

- hover、press、release 与 focus visual 顺序正确；
- drag-out 期间 capture 保持，release 后解除；
- key、wheel 与窗口失焦恢复正常；
- pointer target 与 gesture session 未互相污染。

### Transform

- nested coordinate roots 层级正确；
- absolute/local outline 重合；
- coordinate root move 保持 child 相对位置；
- target-domain 点击只在目标内部选中。

### Layout 与 Clip

- `root_viewport_resize` 随 logical viewport 重排五区布局；
- resize 不修改 Figure 世界坐标或 ScalablePane scale；
- macOS 缩小窗口无白色拖影和垂直抖动；
- nested、multi-layer 与 responsive clip 均保持祖先裁剪；
- responsive clip parent 使用真实 `LineBorder`，4 px border insets 进入 client area，
  border 在 children 后绘制且不会被覆盖；
- 场景切换和 resize 后无裁剪漂移、残影或兄弟样式污染。

## 4. M8 Native 人工结果

### Viewport

- `clip_to_viewport` 的越界内容不穿透 viewport；
- `origin_scroll` 的原点与参考块位置正确；
- `zoomed_content` 保持锚点，尺寸与偏移按 2 倍变化；
- `nested_viewports` 同时受内外 viewport 裁剪；
- 四场景往返、resize 和最小化恢复无黑帧、残影、裁剪扩大或缩放累积。

### ScrollPane

- Automatic policy 随窗口空间稳定显示或隐藏双向 scrollbar；
- wheel、触控板横纵滚动、step、page 与 thumb drag 均更新共享 RangeModel；
- thumb release 后不继续拖动；
- pinch 保持入口锚点，缩放后范围立即更新；
- 最小/最大 zoom 与四边 range clamp 正确；
- `U` 切换前后几何、裁剪、scroll position 与 zoom 等价；
- resize 和最小化恢复无状态丢失。

ScrollBar 端点在 Core 1.0 中每次 mouse press 执行一个 step。Draw2D
`Clickable.REPEAT_FIRING` 的按住连发随完整 widget repeat scheduler 延后，不属于
M8 当前门禁；该差异不影响 step/page/thumb/range 语义。

## 5. Web 复核

- `./scripts/build_web_validation.sh` release 构建通过；
- Vello WebGPU `viewport/nested-viewports` 正常绘制双层裁剪；
- Vello WebGPU `scroll-pane/automatic-scrollbars`、`automatic-hidden` 和
  `scalable-content` 正常绘制；
- DPR 从 2x 切换到 1x 时 logical surface 保持 `767 x 575`，physical surface 从
  `1534 x 1151` 切换为 `767 x 575`；
- 页面保持 `READY`，浏览器 Console 无 panic、WebGPU validation error 或 Wasm
  exception；
- D2.5 已有 Web wheel、Ctrl+wheel/pinch、freeform range 与 layer hit-order 人工
  证据继续有效。

## 6. 结论

M1-M6 与 M8 的契约、产品入口、自动验证和人工窗口验收均已闭合，可从
`behavior_verified` 提升为 `complete`。M7 保持 `complete`。

下一阶段进入 M10 Text/Image/Widget Web 等价场景，不在本次验收中扩张
ScrollBar repeat firing、完整 widget toolkit 或 GEF 层能力。
