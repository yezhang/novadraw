# M10.5 Tooltip 与 Accessibility 手工验证

类型：`verification`

状态：`complete`

## 1. 启动

```bash
cargo run -p widgets-app
```

切换到 `Tooltip_Accessibility` 场景。

正常启动时该场景是默认第 5 个场景。不要使用 `Tooltip_Boundary_Visual` 做 delay
验收；它只为截图预热 Tooltip。

## 2. Tooltip

按顺序验证：

1. 指针移到窗口外再移回中部空白区；约 500ms 前不得出现 Tooltip，约 500ms 后应显示
   `Inherited from the scene root`。
2. 移入蓝色容器，约 500ms 后应显示
   `Inherited from the blue container`。
3. 在蓝色容器与内部 `Inherited tooltip` Label 之间移动；文本应保持不变且不闪烁，
   证明 child 复用同一祖先 source。
4. 在 Tooltip 已显示时移入 `Accessible action`；应立即替换为
   `Button role with a default action`，不重新等待 500ms。
5. 先移出窗口隐藏，再移入 `Accessible action`；这次必须重新等待约 500ms。
6. 移入 `Bottom edge`；Tooltip 应显示
   `Flips above and clamps inside the surface`，位于按钮上方、右侧不越界且不遮挡按钮。
7. Tooltip 可见时分别执行鼠标按压、滚轮、任意按键和移出窗口；每次都应立即隐藏。
8. 再次 hover 后保持不动，约 5 秒后应自动隐藏。
9. 缩小并放大窗口后重复第 6 步；popup 始终不得越出 logical surface。

## 3. Accessibility 与键盘

1. 指针移出窗口，按 Tab；`Accessible action` 应获得 focus visual。
2. 再按 Tab；`Bottom edge` 应获得 focus visual。按 Shift+Tab 应返回
   `Accessible action`。
3. 聚焦任一 Button 后按住 Enter 或 Space：按下时显示 pressed，释放时恢复并只触发
   一次 action。
4. 切到 `Toggle_States` 或 `Interactive_Widgets`，聚焦 Toggle 后按 Enter/Space，
   selected 状态每次只切换一次。
5. 键盘按住期间执行无关鼠标 release，不得提前完成键盘 action。
6. 确认鼠标 press/capture 与键盘 focus 不互相污染。

底层 node identity、Snapshot/Delta、logical bounds、dispose removal 和 action 因果由
`m10_accessibility_contract` 自动测试判定，不以肉眼替代。

## 4. Web 复核

```bash
./scripts/build_web_validation.sh
./scripts/serve_web_validation.sh
```

打开：

```text
http://127.0.0.1:4173/?backend=vello
```

进入 Widgets → Tooltip and Accessibility（第 5/5 场景）：

- hover canvas 后 DOM Tooltip 可见，超时后隐藏；
- 浏览器 accessibility tree 包含 heading、label 和两个 button role；
- Console 无 panic、WebGPU validation error 或 Wasm exception；
- 切换场景后旧 Tooltip 与 accessibility node 不残留。

## 5. 验收记录

```text
Commit: 54a0100 后续验收修订
日期: 2026-09-11
操作系统: macOS
浏览器: Chrome
GPU / WebGPU adapter: Vello WebGPU

[x] Native inherited/local source 与 delay
[x] Native replace/hide/timeout
[x] Native bottom/above/clamp
[x] Native Tab/Shift+Tab 与 Enter/Space
[x] Native resize 与窗口失焦
[x] Web Tooltip DOM
[x] Web accessibility tree
[x] Web Console / Network

补充修复：

- Tooltip 场景的 FigureStyle replacement 显式保留 scene/panel background；
- 交互场景与预热截图场景分离，人工验收使用默认 500ms delay；
- Native 首帧在临时 Skipped/Retry 后执行有界重试，无需等待鼠标事件触发 redraw。

结论: PASS
```
