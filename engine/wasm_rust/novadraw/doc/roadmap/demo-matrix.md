# Demo 与验证矩阵

类型：`roadmap`

> 本文档承载每个 draw2d 核心 milestone **配套的 demo + 验证策略**。
>
> **编号约定**：本文统一使用 `M1-M10`，并与 `doc/parity/draw2d/api-coverage.md` 中的 milestone 映射保持一致。
> **与 product-deliverables.md 的关系**：产品清单回答 *what to ship*，本文回答 *how to verify*。

## 验证分层

每个 milestone 完成必须三层验证齐过：

| 层 | 来源 | 度量 |
|----|------|------|
| **契约层** | `doc/parity/draw2d/api-coverage.md` 中的语义覆盖检查点 | 单元测试、契约属性测试 |
| **产品层** | `product-deliverables.md` 清单 | 类型/能力存在性测试 |
| **端到端层** | 本文 demo + 验证策略 | demo 截图断言 / 帧率断言 / 集成测试 |

建议把里程碑收敛按两级判断：核心行为已验证时至少通过前两层；标记完成时三层全过。

## Demo 矩阵

| Milestone | Demo 名称 | 路径 | 验证策略 | 测试增量预期 |
|------|-----------|------|----------|--------------|
| M1 | 无独立 demo | — | 仅类型单测 + Graphics 状态栈嵌套测试 | +30 |
| M2 | 无独立 demo | — | active core Figure 的树、盒模型与三段式 paint 契约测试 | +60 |
| M3 | `clip-app` nested clip 场景 | `apps/native/clip-app` | 嵌套裁剪截图验证 + paint/hit-test 一致性测试 | +40 |
| M4 | `transform-app` ✅ | `apps/native/transform-app` | 深层嵌套坐标转换 + 坐标根移动 + 入口域降域可视化 | +50 |
| M5 | `layout-app` ✅ + `update-app` ✅ | `apps/native/layout-app`、`apps/native/update-app` | 6 布局截图验证 + bounds 契约 fixture；三种失效粒度 + 1,024 Figure 事务门禁 | +250 |
| M6 | `event-app` ✅ | `apps/native/event-app` | 4 类监听 + hit-test 全图元 + capture/focus + gesture session 状态机断言 | +100 |
| M7 | 集成入 `event-app` + `update-app` | 同上 | bounds 变化触发 `figureMoved`；坐标根移动触发 `coordinateSystemChanged`；UpdateManager 触发 validating/painting 通知 | +80 |
| M8 | `scroll-pane-demo` ✅ + `viewport-app` ✅ | `apps/native/scroll-pane-demo`、`apps/native/viewport-app` | ScrollBar + 行/像素滚动 + ZoomManager/macOS pinch 锚点缩放；24 项契约测试与 CLI 验证 | +120 |
| D2 | `scroll-pane-demo` ✅ + Web `layer-freeform` suite ✅ | `apps/native/scroll-pane-demo`、`apps/web/web-validation` | layer 顺序/透明命中、负坐标四方向滚动、content-domain range 与锚点缩放；自动与人工验收通过 | — |
| M9 | `connections-demo` | `apps/native/connections-demo` | 5 anchor + Direct/Bendpoint/shared Manhattan/Fan + Locator/Decoration + ConnectionLayer + viewport topology 八场景 | +150 |
| M10 | `shape-app` + `border-app` + `text-app` + `widgets-app` + Web `text-image`/`widgets` suites ✅ | `apps/native/shape-app`、`apps/native/border-app`、`apps/native/text-app`、`apps/native/widgets-app`、`apps/web/web-validation` | deferred builtin Figure + 6 边框 + 文本/图像资源 + Clickable/Button/Toggle 交互 + Tooltip 悬停延迟/边界 placement + accessibility Snapshot/Delta/action；Native/Web 等价验收完成 | +220 |

**测试增量合计**：+1,100（基线 146，目标 ~1,250）

## 通用验证规范

### 截图断言

- 工具：`--screenshot` 参数（见 CLAUDE.md）
- 工作流入口：按各 demo 的 `--screenshot` 输出与集成测试组合执行
- 报告：`target/visual-verification/report.md`
- AI 审查请求：`target/visual-verification/ai-review-request.md`
- 背景色 RGB(238, 238, 238)，图形颜色禁止与此重复

截图断言分为 6 层：Unit Contract Tests、RenderCommand Snapshot、Screenshot Capture、Pixel / Semantic Check、AI Visual Review、Visual Report。

### 统一 CLI

- `cargo xtask list`：列出统一 profile 与 suite ID
- `cargo xtask verify core.runtime`：验证 damage、通知顺序、dirty 合并、panic 恢复、
  1,024 Figure 事务、capture、focus 和坐标根降域
- `cargo xtask verify m8`：验证 Viewport、ScrollPane、Freeform 与 Zoom
- `cargo xtask check --full`：运行完整 workspace 提交门禁
- 命令与报告路径的唯一可执行定义：
  [`../../verification/suites.toml`](../../verification/suites.toml)

### 帧率断言

仅 M5 `update-app` 的 stress 场景与 M8 滚动 demo 需要：

- 1k+ Figure 全量更新 ≥ 30fps
- 局部失效 ≥ 60fps（部分场景）

> 注：Year 1 不强求所有 demo 都达 60fps，符合 CLAUDE.md "扩展性 > 稳定性 > 性能" 原则。

### 等价测试

- M3 paint/hit-test 一致性：同一 border-inset clientArea 同时约束绘制裁剪、hit-test descent 和 mouse event target
- M5 draw2d 反向等价：本项目 6 布局的输出与 g2 同输入下的 `bounds` 结果**位级一致**或在 ±1px 容差内
- 路径：`novadraw-scene/tests/` + `novadraw-scene/benches/`

### 阻塞收口规则

每个 demo 启动前必须确认依赖 milestone 已 `behavior_verified`。历史暂停项只在对应 milestone 执行时收口。

## 状态同步规则

每个 demo 完成时：

1. 在本文对应行追加 ✅ 标记
2. 视需要在相关 milestone 标题后追加进展标记
3. 同步补齐对应测试与截图证据

## Demo 完成清单（勾选区）

- [x] M3 `apps/native/clip-app` nested clip 场景
- [x] M4 `apps/native/transform-app`
- [x] M5 `apps/native/layout-app`
- [x] M5 `apps/native/update-app` stress 场景
- [x] M6 `apps/native/event-app`
- [x] M8 `apps/native/scroll-pane-demo`
- [x] M8 `apps/native/viewport-app` 4 场景视觉验证
- [x] D2 `layer-freeform` Native/Web 人工验收
- [x] M9 `apps/native/connections-demo`（自动截图、视觉复核与六场景人工窗口验收完成）
- [x] D3.1 `connections-demo` 增量（`shared_manhattan`、`unsupported_viewport_topology` 自动截图、视觉复核与人工窗口验收通过）
- [x] D3.2 `d3_runtime_mutation`（Runtime layout/constraint/size/Z-order/clipping 与 callback FIFO 自动契约）
- [x] D3.3 `d3_runtime_listener`（七类 Runtime listener、统一注销与 callback self-removal 自动契约）
- [x] D4.0 `architecture-review-probe`（身份、释放、资源、backend 重建、route 与 Label 首帧证据复跑）
- [x] M10.1 `apps/native/shape-app`（自动契约与人工窗口验收完成）
- [x] M10.1 `apps/native/border-app`（含非对称 MarginBorder client-area 可视验证）
- [x] M10.2-M10.3 `apps/native/text-app`（字体/CJK fallback、ellipsis、图标 placement、style inheritance、TitleBarBorder、PNG/SVG 与资源状态截图复核通过）
- [x] M10.4 `apps/native/widgets-app`（自动契约、三场景截图与 macOS 人工窗口验收通过）
- [x] M10.5 `widgets-app` Tooltip/Accessibility 场景（自动契约、Native 边界上翻截图、macOS 人工交互与 Web DOM/AX 验证完成）
- [x] M10 Web `text-image`/`widgets` suites（11 个共享场景、WebGPU 资源状态、交互、Tooltip 与 accessibility 验收完成）

---

## 附录 A：GEF 层历史探索（非 draw2d 核心）

> ⚠️ 以下 demo **不计入 draw2d 核心 milestone 完成判据**，不挂在 M1-M10 任何项下。
> 它们用于验证 draw2d 协议层的承载能力，但其能力本身（创建/拖拽/连接/删除节点等）属于 GEF 层。
> 当前 GEF 实施状态见 [`doc/roadmap/editor/`](editor/00-index.md)。

### 节点编辑器探索 demo

- **路径**：`apps/native/node-editor-demo`（暂定）
- **触发时机**：M1-M10 全部 `behavior_verified` 之后
- **能力范围**：创建节点 / 拖拽 / 连接 / 删除 / 滚动+缩放 / Tooltip
- **GEF helper**：`AutoexposeHelper` 在拖拽期间驱动 Viewport 自动滚动，属于本层，
  不计入 M8 Draw2D 核心完成门禁
- **验证目的**：
  - draw2d 核心协议在端到端编辑场景下是否仍自洽
  - 暴露未来 GEF 层的需求点（Tool / Command / Request 在何处自然涌现）
- **不验证目的**：
  - ❌ 不作为 draw2d 核心毕业判据
  - ❌ 不强求 60fps / 100 节点性能基线
  - ❌ 不允许为通过本 demo 而修改 draw2d 核心协议

### 边界守门

如果探索过程中发现协议层缺口，正确做法：

1. 在本文档附录记录缺口
2. 在对应 milestone 下记录新的架构增量及受影响 API family
3. 通过 contract probe 把缺口收口
4. **禁止**为单独让 demo 跑通而在 apps 层堆便利方法

GEF 层已经确认启动。后续新增状态只写入 `doc/roadmap/editor/`；本附录保留为
Draw2D 阶段的历史边界说明。
