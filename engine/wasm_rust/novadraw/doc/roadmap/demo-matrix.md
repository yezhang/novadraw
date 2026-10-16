# Demo 与验证矩阵

类型：`roadmap`

> 本文档承载每个 draw2d 核心 milestone **配套的 demo + 验证策略**。
>
> **编号约定**：本文统一使用 `M1-M10`，并与 `doc/parity/draw2d/api-coverage.md` 中的 milestone 映射保持一致。
> **与 product-deliverables.md 的关系**：产品清单回答 *what to ship*，本文回答 *how to verify*。

M1-M10 的 demo 基线已完成；本文维护回归覆盖，不是当前工作队列。后续能力与
待采证状态见 [P2 backlog](p2-delta-backlog.md)，不以测试增量数量判断完成。

## 验证分层

每个 milestone 完成必须三层验证齐过：

| 层 | 来源 | 度量 |
|----|------|------|
| **契约层** | `doc/parity/draw2d/api-coverage.md` 中的语义覆盖检查点 | 单元测试、契约属性测试 |
| **产品层** | `product-deliverables.md` 清单 | 可观察行为、组合与失败路径 |
| **端到端层** | 本文 demo + 验证策略 | demo 截图断言 / 帧率断言 / 集成测试 |

建议把里程碑收敛按两级判断：核心行为已验证时至少通过前两层；标记完成时三层全过。

## Demo 矩阵

| Milestone | Demo 名称 | 路径 | 验证策略 |
|------|-----------|------|----------|
| M1 | 无独立 demo | — | 类型单测 + Graphics 状态栈嵌套测试 |
| M2 | 无独立 demo | — | active core Figure 的树、盒模型与三段式 paint 契约测试 |
| M3 | `clip-app` nested clip 场景 | `examples/native/clip-app` | 嵌套裁剪截图验证 + paint/hit-test 一致性测试 |
| M4 | `transform-app` ✅ | `examples/native/transform-app` | 深层嵌套坐标转换 + 坐标根移动 + 入口域降域可视化 |
| M5 | `layout-app` ✅ + `update-app` ✅ | `examples/native/layout-app`、`examples/native/update-app` | 6 布局截图验证 + bounds 契约 fixture；三种失效粒度 + 1,024 Figure 事务门禁 |
| M6 | `event-app` ✅ | `examples/native/event-app` | 4 类监听 + hit-test 全图元 + capture/focus + gesture session 状态机断言 |
| M7 | 集成入 `event-app` + `update-app` | 同上 | bounds/坐标根变化通知；UpdateManager validating/painting 通知 |
| M8 | `scroll-pane-demo` ✅ + `viewport-app` ✅ | `examples/native/scroll-pane-demo`、`examples/native/viewport-app` | ScrollBar、滚动、缩放、pinch；契约与 CLI 验证 |
| D2 | `scroll-pane-demo` ✅ + Web `layer-freeform` suite ✅ | `examples/native/scroll-pane-demo`、`examples/web/web-validation` | layer 顺序/透明命中、负坐标四方向滚动、range 与锚点缩放 |
| M9 | `connections-demo` | `examples/native/connections-demo` | Anchor/Router/Locator/Decoration、ConnectionLayer 与 viewport topology 八场景 |
| M10 | `shape-app` + `border-app` + `text-app` + `widgets-app` + Web `text-image`/`widgets` suites ✅ | `examples/native/shape-app`、`examples/native/border-app`、`examples/native/text-app`、`examples/native/widgets-app`、`examples/web/web-validation` | Figure/Border、文本/图像、控件、Tooltip 与 accessibility；Native/Web 等价 |

## 通用验证规范

### 截图断言

- 工具：`--screenshot` 参数（见 CLAUDE.md）
- 工作流入口：按各 demo 的 `--screenshot` 输出与集成测试组合执行
- 报告：`target/visual-verification/report.md`
- AI 审查请求：`target/visual-verification/ai-review-request.md`
- 背景色 RGB(238, 238, 238)，图形颜色禁止与此重复

截图断言分为 6 层：Unit Contract Tests、RenderCommand Snapshot、Screenshot Capture、Pixel / Semantic Check、AI Visual Review、Visual Report。

### 统一 CLI

- `cargo xtask list`：列出跨工具 suite
- `cargo xtask verify runtime.apps`：验证 Update、Event、Viewport、UML 和 Animation
  应用探针
- 单个契约直接运行对应 `cargo test -p <crate> --test <target>`
- 最终 workspace 门禁直接运行 fmt、Clippy 与 test
- 当前命令集合见 [`../../verification/README.md`](../../verification/README.md)

### 性能采证

固定场景、硬件、DPI、输入轨迹和原始样本后，按 GA-2 记录 p50/p95、GPU 与
input-to-present。旧 30/60fps 建议不作为当前未完成门禁；帧预算在测量后校准。
常规回归保留确定性工作量，不能用 demo 帧率或命令录制耗时证明总体性能达标。

### 等价测试

- M3 paint/hit-test 一致性：同一 border-inset clientArea 同时约束绘制裁剪、hit-test descent 和 mouse event target
- M5 draw2d 反向等价：本项目 6 布局的输出与 g2 同输入下的 `bounds` 结果**位级一致**或在 ±1px 容差内
- 路径：`novadraw/tests/` + `benchmarks/`

### 阻塞收口规则

每个 demo 启动前必须确认依赖 milestone 已 `behavior_verified`。历史暂停项只在对应 milestone 执行时收口。

## 状态同步规则

每个 demo 完成时：

1. 在本文对应行追加 ✅ 标记
2. 视需要在相关 milestone 标题后追加进展标记
3. 同步补齐对应测试与截图证据

## Demo 完成清单（勾选区）

- [x] M3 `examples/native/clip-app` nested clip 场景
- [x] M4 `examples/native/transform-app`
- [x] M5 `examples/native/layout-app`
- [x] M5 `examples/native/update-app` stress 场景
- [x] M6 `examples/native/event-app`
- [x] M8 `examples/native/scroll-pane-demo`
- [x] M8 `examples/native/viewport-app` 4 场景视觉验证
- [x] D2 `layer-freeform` Native/Web 人工验收
- [x] M9 `examples/native/connections-demo`（自动截图、视觉复核与六场景人工窗口验收完成）
- [x] D3.1 `connections-demo` 增量（`shared_manhattan`、`unsupported_viewport_topology` 自动截图、视觉复核与人工窗口验收通过）
- [x] D3.2 `d3_runtime_mutation`（Runtime layout/constraint/size/Z-order/clipping 与 callback FIFO 自动契约）
- [x] D3.3 `d3_runtime_listener`（七类 Runtime listener、统一注销与 callback self-removal 自动契约）
- [x] D4.0 `architecture-review-probe`（身份、释放、资源、backend 重建、route 与 Label 首帧证据复跑）
- [x] M10.1 `examples/native/shape-app`（自动契约与人工窗口验收完成）
- [x] M10.1 `examples/native/border-app`（含非对称 MarginBorder client-area 可视验证）
- [x] M10.2-M10.3 `examples/native/text-app`（字体/CJK fallback、ellipsis、图标 placement、style inheritance、TitleBarBorder、PNG/SVG 与资源状态截图复核通过）
- [x] M10.4 `examples/native/widgets-app`（自动契约、三场景截图与 macOS 人工窗口验收通过）
- [x] M10.5 `widgets-app` Tooltip/Accessibility 场景（自动契约、Native 边界上翻截图、macOS 人工交互与 Web DOM/AX 验证完成）
- [x] M10 Web `text-image`/`widgets` suites（11 个共享场景、WebGPU 资源状态、交互、Tooltip 与 accessibility 验收完成）

---

## 附录 A：GEF 层历史探索（非 draw2d 核心）

> ⚠️ 以下 demo **不计入 draw2d 核心 milestone 完成判据**，不挂在 M1-M10 任何项下。
> 它们用于验证 draw2d 协议层的承载能力，但其能力本身（创建/拖拽/连接/删除节点等）属于 GEF 层。
> 当前 GEF 实施状态见 [`doc/roadmap/editor/`](editor/00-index.md)。

### 节点编辑器探索 demo

- **路径**：`examples/native/node-editor-demo`（已作为 G3-G5 验证入口）
- **阶段**：历史探索已完成，当前 Editor 增量进入 P2 backlog
- **能力范围**：创建节点 / 拖拽 / 连接 / 删除 / 滚动+缩放 / Tooltip
- **GEF helper**：`AutoexposeHelper` 在拖拽期间驱动 Viewport 自动滚动，属于本层，
  不计入 M8 Draw2D 核心完成门禁
- **验证目的**：
  - draw2d 核心协议在端到端编辑场景下是否仍自洽
  - 回归 Tool / Command / Request 与模型投影的现行契约
- **不验证目的**：
  - ❌ 不作为 draw2d 核心毕业判据
  - ❌ 不强求 60fps / 100 节点性能基线
  - ❌ 不允许为通过本 demo 而修改 draw2d 核心协议

### 边界守门

如果探索过程中发现协议层缺口，正确做法：

1. 在整改清单登记 finding 与最小反例
2. 建立独立 P2 delta 并记录受影响 API family，不重开已完成 M/G milestone
3. 通过 contract probe 把缺口收口
4. **禁止**为单独让 demo 跑通而在 apps 层堆便利方法

GEF G0-G5 已完成；后续新增执行状态只写入 P2 backlog。本附录保留为
Draw2D 阶段的历史边界说明。
