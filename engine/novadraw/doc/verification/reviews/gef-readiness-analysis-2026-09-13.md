# GEF / Editor 启动就绪分析

类型：`verification`

日期：2026-09-13

Draw2D/GEF 基线：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

Novadraw 基线：`0db62ea`

## 1. 问题

本次分析回答：

1. 当前 Draw2D 实现范围是否足以承载 GEF；
2. 哪些 Draw2D 语义已对等，哪些仍缺失；
3. 哪些缺口必须现在解决，哪些可以后置；
4. GEF 应何时启动、采用什么 crate 和目录边界；
5. 如何避免把现有 editor 探针误当作框架。

## 2. 证据路径

### 外部事实

核对 GEF Classic：

- `org.eclipse.gef.EditPart`；
- `editparts.AbstractEditPart`；
- `editparts.AbstractGraphicalEditPart`；
- `EditPartViewer`、`GraphicalViewer`、`RootEditPart`；
- `ui.parts.AbstractEditPartViewer`、`GraphicalViewerImpl`；
- `ui.parts.DomainEventDispatcher`；
- `EditPolicy`、`Request`、`Tool`；
- `commands.Command`、`CommandStack`；
- `ScalableRootEditPart`、`ScalableFreeformRootEditPart`、`LayerConstants`；
- GEF Developer Guide 的 MVC、EditPart lifecycle、selection 和 interaction 章节。

### 项目事实

核对：

- `doc/parity/draw2d/api-coverage.md`；
- `doc/verification/reviews/draw2d-core-1.0-final-audit-2026-09-13.md`；
- `doc/adr/adr-014-extensibility-and-lifecycle-boundaries.md`；
- `novadraw-scene` public exports；
- `FigureTree` search/ancestor query；
- `Runtime` update-aware mutation；
- Layer/Freeform、Viewport 和 Connection Runtime；
- 当时尚存的 Figure selection/input/feedback probe。

## 3. 外部语义结论

GEF 的最小核心不是更多 Figure，而是以下协作链：

```text
Model
-> EditPart tree and Viewer registry
-> Figure view

Input
-> EditDomain active Tool
-> Request
-> EditPolicy
-> Command
-> CommandStack
-> Model
-> notification
-> EditPart refresh
```

模型、EditPart 和 Figure 构成三套近似平行的结构。Connection 是例外，由 source/target
parts 共同发现，visual 放在专用 layer。

Viewer targeting 不是简单的 Figure event target：它先执行通用 Figure hit-test，再从
命中 Figure 沿 ancestor 链查 visual registry，找到对应 EditPart。Handle 还有独立的
优先 targeting。

DomainEventDispatcher 先允许 Draw2D Figure 处理输入；事件已消费或 Figure capture
时不转给 EditDomain，否则交给 active Tool。Tool 在 gesture 中维护独立状态和 tracker。

Command 修改模型。EditPart 被删除后不会因 undo 复活，undo 会由模型通知创建新的
EditPart。因此 Command 不应保存活 EditPart/Figure。

## 4. Novadraw 对等能力

足以承载 Editor 的 Draw2D 基础已经存在：

- FigureTree、generational FigureId、parent/children/Z-order；
- 通用 TreeSearch、hit-test、exclusion、ancestor/descendant query；
- Runtime update-aware add/remove/reparent/reorder/mutation；
- Figure lifecycle、scoped listener、stable notification；
- LayeredPane、LayerKey、Freeform、Viewport、Scroll/Zoom；
- Connection、Anchor、Router、Locator 和依赖失效；
- 坐标转换、target-domain event、capture/focus/hover；
- validation、damage、frame preparation；
- Native/Web/Headless host 与验证入口。

Core 1.0 最终审计确认 M1-M10、P0/P1 和 ADR-014 门禁完成。当前 HEAD 相比该代码
基线只增加审计文档，没有新的 Rust 行为变更。

## 5. 非阻塞 Draw2D 差额

以下能力仍未完整实现，但不阻塞最小 Editor：

- path clip/query、gradient、image source rectangle、高级 stroke/XOR；
- atomic indexed add、removeAll、统一 Shape mutation、mirrored coordinates；
- TextFlow/fragment/bidi、caret 和富文本编辑；
- 完整 widget toolkit 和 repeat scheduler；
- DirectedGraph、CompoundDirectedGraph、ShortestPath；
- PrinterGraphics、ScaledGraphics、Thumbnail；
- Windows/Linux 发布资格。

这些能力应由具体产品需求触发 P2 delta，不应继续扩张 M1-M10。

## 6. 当前真正缺失

当前没有 GEF 框架实现。缺失项是：

- ModelId 与模型通知 adapter；
- EditPartId、PartTree、PartFactory 和生命周期；
- model/visual registry；
- Viewer contents/root/selection/focus；
- typed Request、EditPolicy role；
- Command/CompoundCommand/CommandStack；
- Tool/tracker 状态机；
- handle/feedback layer；
- create/move/resize/delete/reconnect；
- undo/redo、save/load。

该历史 probe 只保存 `Option<FigureId>` 单选，直接从
`find_mouse_event_target_at` 选择 Figure，并在 RenderSubmission 后追加描边命令。
它不具备上述框架语义，已在 G3 启动后移除。

## 7. 必须先解决的跨层问题

唯一需要在 Editor 启动期重点验证的 Draw2D/Editor 接缝是输入仲裁：

- Runtime 当前公开 `dispatch_mouse_*` 等命令式入口；
- 调用方无法获得稳定的 consumed outcome；
- GEF Tool 不能通过 repaint、hover 或 capture 的副作用猜测事件是否被 Figure 消费；
- 嵌入 Button、ScrollBar 等 Figure-native widget 时需要明确优先级。

结论不是立即修改 Runtime，而是先在 G3 建立失败 probe。若证据成立，再引入
`DispatchOutcome` 或统一输入 API，作为独立 Core P2 delta。

## 8. 启动判定

判定：`ready`。

理由：

- Draw2D Core 1.0 固定门禁已通过；
- Viewer targeting 所需的通用 search/ancestor API 已存在；
- root layers、viewport、connection 和 update-aware mutation 已存在；
- remaining Draw2D P2 不属于首个编辑闭环依赖；
- Editor 可以保持单向依赖而不修改渲染主循环。

启动不等于直接开发节点拖拽。先完成 G0 设计与工程骨架，再按 G1-G6 推进。

## 9. 采用结论

- crate：`novadraw-editor`；
- 文档路线图：`doc/roadmap/editor/`；
- 规范设计：`doc/design/editor/architecture.md`；
- 语义账本：`doc/parity/gef/api-coverage.md`；
- 首个产品入口：`apps/native/node-editor-demo`；
- 不保留旧 Figure selection/input probe；正式产品验证由
  `apps/native/node-editor-demo` 承担；
- G0-G4 保持单 crate，不提前拆包。

关键决策由
[`../../adr/adr-015-editor-framework-boundary.md`](../../adr/adr-015-editor-framework-boundary.md)
接受。

## 10. G0 执行结果

已创建：

- `doc/roadmap/editor/`；
- `doc/design/editor/`；
- `doc/parity/gef/`；
- `doc/adr/adr-015-editor-framework-boundary.md`；
- `novadraw-editor` 单 crate 及 model/part/viewer/request/policy/command/tool/feedback/domain
  模块边界。

`novadraw-editor` 当前不导出行为 API。模块骨架只固定职责和依赖方向，避免在 G1
真实模型与 Command 测试之前承诺对象安全、错误和借用接口。

验证结果：

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
new-document local link check: PASS
git diff --check: PASS
```

G0 判定：`complete`。下一阶段是 G1 Model Adapter 与 CommandStack 契约，不应跳到
Viewer、拖拽或节点编辑器产品实现。
