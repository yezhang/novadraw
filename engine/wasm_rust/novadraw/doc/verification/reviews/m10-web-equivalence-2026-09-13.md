# M10 Text / Image / Widget Web 等价验收

类型：`verification`

日期：2026-09-13

状态：`complete`

代码基线：本报告所在提交

## 1. 范围

本次验收补齐 M10 Native 产品场景在 Web/Vello 上的共享入口和等价证据：

- Text / Image：Typography/CJK、Ellipsis、Icon Placement、Style Inheritance、
  TitleBarBorder、Image Resources；
- Widgets：Button States、Toggle States、Interactive Widgets、
  Tooltip Boundary Visual、Tooltip and Accessibility；
- WebGPU glyph/image resource、DOM Tooltip 与 accessibility projection。

M10.1-M10.5 的引擎契约不在本批次重新设计；本批次只补齐共享 Demo、Web 尺寸适配、
平台验证和状态收口。

## 2. 实现增量

- `text::suite()` 将 `text-app` 六个 Runtime 场景接入共享 `DemoSuite` catalog；
- TitleBarBorder 场景使用 full-surface host contents，并将 bordered panel 作为 child，
  避免 logical viewport resize 改写产品 panel；
- Image Resources 四张卡调整为在 Web 最小 logical width 内完整显示；
- Tooltip bottom-edge source 调整到 Web 最小 logical height 内；
- Tooltip Boundary Visual 使用独立长时展示窗口，避免静态视觉验收在取证前超时；
- 增加共享场景构建、资源 capability、TitleBar 层级、Image 卡片边界和 Tooltip
  预热状态回归测试。

## 3. 自动门禁

定向测试：

```text
m10_widget_contract: 10 passed
m10_tooltip_contract: 3 passed
m10_accessibility_contract: 4 passed
novadraw-demo-scenes: 15 passed
```

Wasm：

```text
./scripts/build_web_validation.sh
release build: PASS
web_validation_bg.wasm: HTTP 200, Content-Type application/wasm
```

完整 workspace 门禁：

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
```

## 4. Text / Image WebGPU 验收

Chrome/Vello WebGPU 实测：

- Typography/CJK：Inter、Noto Sans SC、JetBrains Mono 均正常，无 tofu；
- Ellipsis：英文、中文和 emoji 截断正确；
- Icon Placement：East、West、North、South 布局正确；
- Style Inheritance：字体与前景色继承正确；
- TitleBarBorder：bordered panel 保持 `(80, 100)` 产品位置，标题与 child client area
  分离；
- Image Resources：PNG Ready、SVG Ready、Pending、Failed 四态完整可见。

## 5. Widget Web 验收

自动与人工证据共同确认：

- Button normal、pressed/focus、disabled visual 正确；
- pointer release-inside 激活，drag-out 取消，drag-back 重新 armed；
- Toggle 每次 pointer 或 Enter/Space 激活只切换一次，并保持 selected；
- Tab 焦点遍历与 Enter/Space 键盘状态正确；
- Tooltip 默认 delay、source inheritance、replace、输入取消和 timeout 正确；
- Bottom edge Tooltip 在空间不足时翻到指针上方，不遮挡指针且不越出 surface；
- Tooltip 跟随 logical-surface pointer anchor；允许覆盖 source Figure 的部分区域；
- accessibility DOM 正确投影 heading、label、button、toggle、enabled 与 selected 状态。

Tooltip Boundary Visual 的 DOM 几何证据：

```text
display = block
canvas bounds = (24, 206.5) .. (791, 781.75)
tooltip bounds = (536.609375, 718.3046875) .. (791, 748.5)
inside canvas = true
```

浏览器自动化工具不能可靠向 Canvas ref 投递真实 pointer/key 事件，因此不使用脚本
合成事件冒充端到端证据；实际交互由人工验收完成，状态机由 Rust 契约测试固定。

## 6. 平台结果

- 页面状态：`data-ready=true`、`data-backend=vello-webgpu`、首帧成功；
- Console：无 panic、WebGPU validation error 或 Wasm exception；
- 场景切换后旧 Tooltip 隐藏且文本清空；
- accessibility tree 只保留当前场景节点，无旧节点残留；
- DPR 与 responsive logical viewport 下 Text/Image/Widget 内容均在 surface 内。

浏览器截图工具曾超时；Text/Image/Widget 视觉结果由浏览器人工复核，Tooltip 另有
DOM 非零 bounds 与 surface containment 证据。该工具失败不作为产品失败。

## 7. 结论

M10 的契约层、产品层、Native 端到端和 Web 等价场景均已闭合。M10 可在路线图中从
`in_progress` 提升为 `complete`。下一阶段仅执行 Draw2D Core 1.0
macOS/Web/Headless 总审计与 R9.4 capability 消融复查，不扩张 P2 或 GEF 能力。
