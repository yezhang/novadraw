# Editor 框架实施计划

类型：`roadmap`

本计划展开 G0-G5 的引擎实施步骤，并记录已移交独立产品包的原 G6 范围。架构以
[`../../design/editor/architecture.md`](../../design/editor/architecture.md) 为准；本文不
通过任务描述改变架构。

## G0：架构与工程启动

### 分析过程

1. 以 GEF Classic commit `4463d9d0c` 为外部事实基线。
2. 检查 `EditPart`、`AbstractEditPart`、`AbstractGraphicalEditPart`、
   `EditPartViewer`、`GraphicalViewerImpl`、`DomainEventDispatcher`、
   `EditPolicy`、`Tool`、`CommandStack` 和 root layer 实现。
3. 对照 Draw2D API 账本、Core 1.0 最终审计、ADR-014 与当前 Rust public API。
4. 抽查 `Runtime::tree()`、TreeSearch、运行期 mutation、LayeredPane 和历史
   Figure selection/input probe。
5. 将发现分成 Draw2D Core 缺口、GEF 必需能力、可后置产品能力。

### 结论

- Draw2D Core P0/P1 不存在阻塞 GEF 启动的未解释缺口。
- 历史 Figure selection/input probe 只有 Figure 级单选和外置描边，不是 GEF 实现，
  已在 G3 启动后移除。
- FigureTree 的通用 hit-test/ancestor query、Runtime mutation、Layer/Freeform、
  Viewport 和 Connection 足以承载首个 Editor 垂直切片。
- Figure 与 Tool 的输入消费仲裁尚无公开 outcome，是 G1/G2 前必须验证的跨层边界。
- Editor 应独立成 `novadraw-editor`，初期保持单 crate。

### 交付

- ADR-015；
- Editor normative architecture；
- GEF parity ledger；
- G0-G5 engine roadmap；
- `novadraw-editor` crate/module skeleton；
- workspace 与文档入口。

## G1：Model Adapter 与 CommandStack

状态：`complete`

### 设计批次

1. 用一个最小 diagram model 固定 `ModelId`、只读查询和通知接口。
2. 定义 `Command<M>` 的拥有数据、prepare、execute、undo、redo 和 dispose。
3. 定义 CompoundCommand 的顺序、失败和补偿边界。
4. 定义 CommandStack event、undo limit、save location 与 dirty。
5. 保证 Command 不依赖 `novadraw-scene::FigureId`。

### 自动门禁

- execute 后进入 undo，redo 清空；
- undo/redo 顺序和 label 稳定；
- failed/rejected command 不错误进入 history；
- compound 逆序 undo；
- save location 跨 execute/undo/redo 正确；
- history flush 释放 owned resource；
- panic/外部副作用不被错误宣称为可回滚。

完成证据：

- 16 项公开契约测试通过；
- recoverable error 保持模型/命令前态的责任由 Command 契约明确；
- unknown-state error、compound compensation failure 和 extension panic 均 fault stack；
- Command API 不依赖 FigureId、Runtime 或平台类型。

## G2：EditPart Tree 与 Viewer 投影

状态：`complete`

### 设计批次

1. 实现 namespaced generational EditPartId 和 PartArena。
2. 实现 PartNode/PartTree 拓扑与生命周期。
3. 固定 EditPartBehavior 和 EditPartFactory 的对象安全边界。
4. 实现 model registry 与 visual registry。
5. 实现 contents/root/content pane。
6. 实现 refreshVisuals 与 refreshChildren。
7. 将模型批次通知映射为确定的 refresh worklist。

### 自动门禁

- 初始模型递归投影；
- children reuse/reorder/create/remove；
- compound Figure targeting 回溯到 owner part；
- duplicate model 原子拒绝；visual 只能由受限构建上下文创建并注册；
- deactivate/unregister/dispose 顺序；
- undo 后以新 EditPart/Figure 身份重建；
- revision gap、stale replay 与同 revision 多事件批次。

阶段边界修正：source/target connection discovery、单 connection part 去重和
connection layer 挂载必须共同交付，统一属于 G5。G2 只闭合模型 containment 投影，
不在 G3 root layer 建立前引入临时连接挂载规则。

## G3：Selection、Targeting 与输入仲裁

状态：`complete`。自动门禁与检查点 A 人工验收均通过。

### 设计批次

1. 实现有序 selection、primary selection 和 EditPart focus。
2. 实现 handle 优先、part ancestor fallback、contents fallback。
3. 建立 root/scalable/printable/primary/connection/handle/feedback layers。
4. 用嵌入 Button 场景验证 Figure consumed/capture 与 Editor fallback。
5. 为 Runtime 增加最小 `DispatchOutcome` P2 delta，正式 Tool 在 G4 接入同一仲裁结果。

### 自动门禁

- click/shift/control selection；
- selection 删除自动 reconcile；
- handle 不误映射到宿主 part；
- Figure widget 消费后 Editor fallback 不启动；
- Figure capture 在完整 pointer gesture 内保持所有权；
- scaled/unscaled layer 域与 handle/feedback targeting 顺序稳定。

Viewport/zoom 下 Tool feedback 与 auto-expose 统一在 G5 验证，不在 G3 缺少
Viewport 组合根时建立临时缩放协议。

## G4：Tool / Request / EditPolicy

状态：`complete`，检查点 B 已于 2026-09-14 通过人工验收。

按垂直切片实现，不一次迁移全部 GEF 类：

1. SelectionTool 与 drag tracker；
2. typed ChangeBoundsRequest；
3. component/layout policy；
4. source/target feedback；
5. move/resize Command；
6. CreateRequest 与 DeleteRequest；
7. 多选 CompoundCommand。

完成标准是 create/move/resize/delete 全部由 Request -> Policy -> Command -> Model
notification -> Part refresh 闭环，不允许 Tool 直接调用 Runtime 改 Figure。

## G5：Connection 编辑

状态：`complete`。G5.1 Connection Projection、G5.2 Connection Creation、G5.3
reconnect/endpoint handle、G5.4 bendpoint 与 G5.5 viewport/auto-expose 自动门禁已完成，
检查点 C 已于 2026-09-22 通过人工验收。

设计入口：

- `doc/reference/gef/connection-editing.md`；
- `doc/design/editor/g5-connection-projection.md`。
- `doc/design/editor/g5-connection-creation.md`。
- `doc/design/editor/g5-connection-reconnect.md`。
- `doc/design/editor/g5-connection-bendpoint.md`。
- `doc/design/editor/g5-viewport-autoexpose.md`。

1. 有序 connection snapshot、source/target relation 与单 ConnectionPart 去重；
2. ConnectionPart/NodePart 关系；
3. CreateConnectionRequest start/end；
4. source/target reconnect；
5. endpoint 和 bendpoint handle；
6. Connection Runtime binding；
7. drag auto-expose；
8. viewport/zoom 下 feedback 与 target。

删除节点时，关联 connection 的处理必须由模型 Command 决定，不由 Figure dispose
猜测业务级联关系。

## G6：下游产品毕业（外部）

本阶段已移交独立产品包，不属于本仓库引擎实施计划。产品侧负责 document schema、
serializer、保存/加载、产品级 Native/Web 入口和业务 history 断言。

引擎侧只保持以下可复用契约：

1. 外部模型可重新注入 `GraphicalViewer`；
2. Runtime、EditPart 和 Figure 身份按新 session 重建；
3. G3-G5 Headless Replay 继续验证确定性编辑事务；
4. 产品验证发现的通用引擎缺口以独立 Editor delta 回流。

## Crate 拆分门禁

G0-G4 期间不拆 crate。G5 完成后只有满足以下全部条件才评估拆分：

- 至少两个独立产品入口复用该模块；
- 公共 API 已跨两个 milestone 稳定；
- 依赖方向无循环；
- 模块可独立测试；
- 拆分带来实际编译、feature 或发布收益；
- 不需要大量 facade 转发或共享可变状态。
