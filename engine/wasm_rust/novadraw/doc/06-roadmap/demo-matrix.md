# Demo 与验证矩阵

> 本文档承载每个 draw2d 核心 milestone **配套的 demo + 验证策略**。
>
> **编号约定**：本文统一使用 `M1-M10`，并与 `doc/01-architecture/draw2d_api_coverage.md` 中的 milestone 映射保持一致。
> **与 product-deliverables.md 的关系**：产品清单回答 *what to ship*，本文回答 *how to verify*。

## 验证分层

每个 milestone 完成必须三层验证齐过：

| 层 | 来源 | 度量 |
|----|------|------|
| **契约层** | `draw2d_api_coverage.md` 中的语义覆盖检查点 | 单元测试、契约属性测试 |
| **产品层** | `product-deliverables.md` 清单 | 类型/能力存在性测试 |
| **端到端层** | 本文 demo + 验证策略 | demo 截图断言 / 帧率断言 / 集成测试 |

建议把里程碑收敛按两级判断：核心行为已验证时至少通过前两层；标记完成时三层全过。

## Demo 矩阵

| Milestone | Demo 名称 | 路径 | 验证策略 | 测试增量预期 |
|------|-----------|------|----------|--------------|
| M1 | 无独立 demo | — | 仅类型单测 + Graphics 状态栈嵌套测试 | +30 |
| M2 | 无独立 demo | — | active core Figure 的树、盒模型与三段式 paint 契约测试 | +60 |
| M3 | `clip-app` nested clip 场景 | `apps/clip-app` | 嵌套裁剪截图验证 + paint/hit-test 一致性测试 | +40 |
| M4 | `transform-app` ✅ | `apps/transform-app` | 深层嵌套坐标转换 + 坐标根移动 + 入口域降域可视化 | +50 |
| M5 | `layout-app` + `update-app` | `apps/layout-app`、`apps/update-app` | 6 布局并排对比 + draw2d 反向等价测试；三种失效粒度 + 1k+ Figure 帧率断言 | +250 |
| M6 | `event-app` | `apps/event-app` | 4 类监听 + hit-test 全图元 + capture/focus 状态机断言 | +100 |
| M7 | 集成入 `event-app` + `update-app` | 同上 | bounds 变化触发 `figureMoved`；坐标根移动触发 `coordinateSystemChanged`；UpdateManager 触发 validating/painting 通知 | +80 |
| M8 | `scroll-pane-demo` + 收口 `viewport-app` | `apps/scroll-pane-demo`、`apps/viewport-app` | ScrollBar + 滚轮 + autoexpose；历史暂停的 4 场景视觉验证全过 | +120 |
| M9 | `connections-demo` | `apps/connections-demo` | 5 anchor × 3 router 组合矩阵 + 节点移动连线跟随测试 | +150 |
| M10 | `shape-app` + `border-app` + 待新增文本/Tooltip demo | `apps/shape-app`、`apps/border-app` | deferred builtin Figure + 6 边框 + 文本布局 + Tooltip 悬停延迟 + Accessible 键盘可达性 | +220 |

**测试增量合计**：+1,100（基线 146，目标 ~1,250）

## 通用验证规范

### 截图断言

- 工具：`--screenshot` 参数（见 CLAUDE.md）
- 工作流入口：按各 demo 的 `--screenshot` 输出与集成测试组合执行
- 报告：`target/visual-verification/report.md`
- AI 审查请求：`target/visual-verification/ai-review-request.md`
- 背景色 RGB(238, 238, 238)，图形颜色禁止与此重复

截图断言分为 6 层：Unit Contract Tests、RenderCommand Snapshot、Screenshot Capture、Pixel / Semantic Check、AI Visual Review、Visual Report。

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

- [x] M3 `apps/clip-app` nested clip 场景
- [x] M4 `apps/transform-app`
- [ ] M5 `apps/layout-app`
- [ ] M5 `apps/update-app` stress 场景
- [ ] M6 `apps/event-app`
- [ ] M8 `apps/scroll-pane-demo`
- [ ] M8 `apps/viewport-app` 4 场景视觉验证
- [ ] M9 `apps/connections-demo`
- [ ] M10 `apps/shape-app`
- [ ] M10 `apps/border-app`
- [ ] M10 文本/图像 demo（待新增）
- [ ] M10 Tooltip demo（待新增）

---

## 附录 A：GEF 层早期探索（非 draw2d 核心）

> ⚠️ 以下 demo **不计入 draw2d 核心 milestone 完成判据**，不挂在 M1-M10 任何项下。
> 它们用于验证 draw2d 协议层的承载能力，但其能力本身（创建/拖拽/连接/删除节点等）属于 GEF 层。

### 节点编辑器探索 demo

- **路径**：`apps/node-editor-demo`（暂定）
- **触发时机**：M1-M10 全部 `behavior_verified` 之后
- **能力范围**：创建节点 / 拖拽 / 连接 / 删除 / 滚动+缩放 / Tooltip
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

如果未来确认要做 GEF 层，新开 `doc/07-gef-roadmap/` 目录承载，本附录迁移过去。
