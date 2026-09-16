# GEF 核心语义覆盖账本

类型：`parity-contract`

参考基线：Eclipse GEF Classic commit `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。

本账本记录 GEF Classic 核心编辑语义到 `novadraw-editor` 的采用关系。它不是 Java
API 翻译清单，也不把 Eclipse Workbench/JFace 集成列为框架核心。

## 状态

| 状态 | 含义 |
|---|---|
| `specified` | Novadraw 目标契约已定义，尚无行为实现证据 |
| `partial` | 已有部分实现，但公共语义或验证未闭合 |
| `verified` | 公开契约、实现和可重复验证均已闭合 |
| `deferred` | 已明确后置，不计入当前 milestone |
| `rejected` | 明确不采用，并记录替代方案 |

crate 骨架和文档不能把 family 提升为 `partial`；至少需要可执行公共行为。

## 核心矩阵

| Family ID | GEF 代表 API / 概念 | Novadraw 目标 | 状态 | Milestone |
|---|---|---|---|---|
| `model.identity` | application model object identity | 应用提供稳定 `ModelId`，框架不拥有业务模型 | verified | G1 |
| `model.notification` | model listener / property change | typed `ModelEvent` + revision + adapter drain；Viewer 拒绝 gap/stale 并接受同 revision 有序批次 | verified | G1/G2 |
| `command.protocol` | `Command`、`CompoundCommand` | typed model command、组合、结构化失败与 fault 边界 | verified | G1 |
| `command.stack` | `CommandStack` | execute/undo/redo、redo flush、limit、drop、event journal | verified | G1 |
| `command.dirty_state` | `markSaveLocation/isDirty` | history identity、branch、fault 与 save location | verified | G1 |
| `part.identity` | EditPart object identity | namespaced generational `EditPartId` | verified | G2 |
| `part.tree` | parent/children、source/target connections | containment 与 connection relation 分离；受检 ConnectionPartId 共享 EditPart 身份域 | verified | G2/G5 |
| `part.factory` | `EditPartFactory` | model/context 到 `EditPartBehavior` | verified | G2 |
| `part.lifecycle` | addNotify/activate/deactivate/removeNotify | 创建、刷新、激活、停用、注销与重建的固定顺序 | verified | G2 |
| `part.visual` | createFigure/getFigure/getContentPane | Part 到主 Figure/content pane 的显式绑定 | verified | G2 |
| `part.refresh` | refreshVisuals/refreshChildren/connections | 通知驱动 visual、containment 与有序 connection snapshot 增量投影 | verified | G2/G5 |
| `viewer.contents_root` | Viewer contents / RootEditPart | 无模型 root + 单 model-backed contents | verified | G2 |
| `viewer.registry` | model/visual part maps | `ModelId -> EditPartId`、`FigureId -> owner part` | verified | G2 |
| `viewer.targeting` | `findObjectAt*` | point hit-test、handle 优先、feedback 穿透、ancestor 与 contents fallback | verified | G2/G3 |
| `viewer.selection` | SelectionProvider / SelectionManager | 有序多选、primary selection、typed delta 与删除 reconcile | verified | G3 |
| `viewer.focus` | focus EditPart | 与 Figure keyboard focus 分离的 viewer state | verified | G3 |
| `root.layers` | primary/connection/handle/feedback layers | keyed LayeredPane + scalable/unscaled feedback 域 | verified | G3 |
| `request.protocol` | `Request` 及 typed subclasses | typed enum/struct，不使用 Any map 作为主协议 | verified | G4 |
| `policy.protocol` | `EditPolicy`、role | target、command contribution、feedback | verified | G4 |
| `tool.lifecycle` | `Tool` / `AbstractTool` | EditorDomain 级 active Tool 状态机 | verified | G4 |
| `tool.tracker` | `DragTracker` | gesture 固定 source/tracker 与 cancel cleanup | verified | G4 |
| `input.arbitration` | `DomainEventDispatcher` | Figure consumed/capture 优先，SelectionTool 复用相同 outcome | verified | G3/G4 |
| `feedback.protocol` | source/target feedback | Figure layer 中的临时 visual，命令前清理 | verified | G4 |
| `interaction.selection` | SelectionTool / marquee | click、modifier、多选与拖拽已闭合；marquee 后置 | partial | G3/G4/G6+ |
| `interaction.create` | CreationTool / CreateRequest | typed creation 与 target validation；专用 CreationTool 后置 | partial | G4/G6+ |
| `interaction.delete` | GroupRequest / component policy | 多选删除、undo；连接清理由 G5 模型 Command 闭合 | partial | G4/G5 |
| `interaction.change_bounds` | ChangeBoundsRequest | move/resize/feedback/undo 已闭合；reparent 后置 | partial | G4/G6+ |
| `connection.part` | ConnectionEditPart / NodeEditPart | 单一有序模型快照投影 source/target relation；endpoint behavior 提供带稳定 key 的 source/target Anchor descriptor，Viewer 仅保留 Chopbox fallback | verified | G5 |
| `connection.create` | CreateConnectionRequest / NodeEditPart request anchor | source-locked start/end 两阶段 Tool；endpoint behavior Anchor preview、反馈、模型 Command 与 undo/redo | verified | G5 |
| `connection.reconnect` | ReconnectRequest / ConnectionEndpointEditPolicy | endpoint handle、稳定 Anchor descriptor preview、source/target 重连、合法性和 undo | verified | G5 |
| `connection.bendpoint` | BendpointRequest / BendpointEditPolicy | create/move/delete handle、typed constraint、feedback 与 undo/redo；Connection behavior 选择应用注册的命名 Router，不污染 ModelAdapter | verified | G5 |
| `viewport.autoexpose` | AutoexposeHelper / ViewportAutoexposeHelper | host 注入单调 elapsed，拖拽期间按 surface edge band 推进 Viewport，并重算 request、target 与 feedback | verified | G5 |
| `document.persistence` | 非 GEF 固定 API | 应用 serializer + 重建一致性门禁 | specified | G6 |
| `clipboard.protocol` | actions / transfer | 平台无关 clipboard payload + host adapter | deferred | G6+ |
| `direct_edit` | DirectEditManager/Request | 文本编辑、IME、commit/cancel | deferred | G6+ |
| `snap.guides` | SnapTo*/rulers/guides | grid/geometry/guide feedback | deferred | G6+ |
| `palette` | PaletteRoot/Viewer/ToolEntry | 可选工具选择 UI，不属于核心闭环 | deferred | G6+ |
| `tree.viewer` | TreeEditPart/TreeViewer | 非图形 viewer | rejected | - |
| `workbench.integration` | Eclipse/JFace actions/properties | 由各平台产品集成替代 | rejected | - |

## 核心语义

### 三棵结构

模型 containment、EditPart tree 和 FigureTree 是三套近似平行但身份独立的结构。
Connection 是例外：它由 source/target part 发现，visual 放在 connection layer。

### Targeting

Viewer 使用通用 Figure hit-test，而不是只查 Figure event handler。命中内部 Figure
后沿 ancestor 链查找首个已注册 visual owner；若没有 part，则回退到 contents。
Handle targeting 必须先于普通 part targeting，并排除不相关 layer。

### 命令

Command 修改模型，不修改 EditPart/Figure。新 execute 清空 redo；undo/redo 产生模型
通知并驱动同一 refresh 链路。Command history 不保存活 controller/view 句柄。

### 输入

Figure-native widget 和 Tool 必须共存。Figure 消费或 capture 后不再进入 Tool；Tool
gesture 自身也必须保持 source/tracker 一致，直到 release/cancel。

## 推进规则

1. 每个 G milestone 必须列出受影响 Family ID。
2. `specified -> partial` 需要公共 API 与至少一个自动契约测试。
3. `partial -> verified` 需要失败路径、生命周期和端到端证据。
4. 为 GEF 需求修改 Draw2D Core 时，必须新增明确 P2 delta，不能回写 M1-M10。
5. 独立 demo 或 probe 的应用行为不计入本账本实现状态。
