# P2 TextFlow 交互几何与 Direct Text Edit

类型：`normative-design`

状态：`target`

范围：P2-T02、P2-E02

GEF 与 Draw2D 源码事实见
[`../../reference/gef/direct-editing.md`](../../reference/gef/direct-editing.md)。

## 1. 目标

本批次在 P2-T01 只读 TextFlow 上增加两层能力：

```text
P2-T02 Core
TextFlow immutable layout
-> document position / hit-test / caret / selection geometry

P2-E02 Editor
DirectEditRequest
-> transient text edit session
-> draft / selection / composition
-> DirectEditPolicy
-> CommandStack
-> application model
```

最终需要支持：

- point、UTF-8 document position 与 visual caret 的双向映射；
- 跨行、跨 fragment、跨 paragraph 的选择几何；
- 单一 active direct-edit session；
- 键盘编辑、pointer selection、commit、cancel 和 undo/redo；
- Native 与 Web 的 IME preedit/commit、候选窗定位和焦点清理；
- 草稿只作为临时编辑状态，业务模型仍是唯一持久事实源。

## 2. 非目标

首批不包含：

- 富文本样式编辑和 inline embedded Figure；
- 多光标、矩形选择、拼写检查与协同编辑；
- clipboard、系统 DnD 或 accessibility text provider；
- 原生控件作为可见文本渲染真值；
- 把 Parley、Winit、DOM 或平台 control 类型暴露到公共契约；
- 在 Core `TextFlowFigure` 中保存 mutable caret、selection 或 composition；
- 自动合并并发模型修改。

## 3. 状态所有权

| 状态 | 唯一 owner | 说明 |
|---|---|---|
| 已提交文本 | application model | Command 修改，模型通知刷新 Figure |
| TextFlow page 与 layout cache | Core Runtime/Figure | 持久视图投影与不可变布局事实 |
| cluster、line、caret、selection geometry | immutable text interaction map | 与 layout revision 同生共灭 |
| draft text、text selection、preedit | Editor direct-edit session | 不进入业务模型或普通 Figure state |
| active feature、source revision | Editor direct-edit session | commit 时进行 stale 校验 |
| IME enable、candidate area、native focus | platform adapter | 只执行 Editor 发出的 host effect |
| Viewer part selection | Viewer `SelectionModel` | 与 session 内文本 selection 完全分离 |

Core 定义平台无关的文本输入 event/effect 值类型和 TextFlow 查询契约，但不拥有直接编辑
会话。Editor 拥有会话状态机。平台 adapter 不解析业务 feature，也不生成模型 Command。

## 4. Core 文档位置

公开位置使用 Novadraw 自有类型：

```rust
pub struct FlowTextPosition {
    pub paragraph: usize,
    pub byte_offset: usize,
    pub affinity: TextAffinity,
}

pub enum TextAffinity {
    Upstream,
    Downstream,
}

pub struct FlowTextRange {
    pub anchor: FlowTextPosition,
    pub focus: FlowTextPosition,
}
```

约束：

- `byte_offset` 是 paragraph flatten 后文本中的 UTF-8 byte boundary；
- fragment boundary 由 layout interaction map 保留，但不是第二套 offset 域；
- affinity 区分 soft-wrap、hard-break 和 bidi boundary 上共享同一 logical offset 的两个
  visual caret；
- range 保留 anchor/focus 方向，规范化范围只用于删除和绘制；
- paragraph 间存在稳定顺序，但不伪造可被单独删除的隐藏 separator character。

非法 paragraph、越界 offset、非 UTF-8 boundary 和已被截断而不可定位的位置必须返回
结构化错误，不能 clamp 到邻近字符。

## 5. Immutable Interaction Map

P2-T02 为每个可交互 `TextLayout` 生成不可变 `TextInteractionMap`。它至少保存：

- paragraph 与 fragment 的 UTF-8 range 映射；
- grapheme/cluster boundary 与 bidi level；
- line index、baseline、ascent、descent 和 visual order；
- logical position 到一个或两个 visual caret edge 的映射；
- point hit-test 所需的 cluster visual bounds；
- visible/truncated range 与 layout revision。

默认 Parley engine 可以消费其 editing/cluster 能力生成该 map，但公开 API 只暴露
Novadraw 类型。外部 `TextLayoutEngine` 必须通过受检的 parts 构造同等 map；不能返回
可绘制 layout 却在交互查询时静默退回平均字符宽度。

Core 提供只读查询：

```rust
hit_test(point) -> Result<FlowTextPosition, TextInteractionError>
caret_geometry(position) -> Result<CaretGeometry, TextInteractionError>
selection_geometry(range) -> Result<Vec<SelectionQuad>, TextInteractionError>
move_position(position, movement) -> Result<FlowTextPosition, TextInteractionError>
```

`TextMovement` 至少覆盖 visual cluster、word、physical line、paragraph 与 document
边界。selection geometry 按 visual run/line 输出有序 quad，不能用单个 logical
rectangle 代替 bidi 或换行选择。

TextFlow paint 仍只消费 immutable layout。caret、selection highlight 和 preedit
underline 由 Editor feedback projection 绘制，不写入 `TextFlowFigure`。

## 6. Direct Edit Feature

同一 EditPart 可以暴露多个稳定 feature。应用通过 policy 返回 typed descriptor：

```rust
pub struct DirectTextEditDescriptor<FeatureKey> {
    pub feature: FeatureKey,
    pub initial_text: String,
    pub source_revision: ModelRevision,
    pub mode: TextEditMode,
    pub focus_loss: FocusLossPolicy,
}
```

`FeatureKey` 必须在该 EditPart 生命周期内稳定、可比较且可调试。descriptor 不携带
Figure、DOM element、Winit window 或应用模型借用。

首批 draft 是 plain Unicode text。Policy 负责在 session 启动时把业务 feature
投影为文本，并在 accept 时根据最终文本构造 Command。应用若以 FlowPage fragments
保存富结构，必须显式定义 flatten/replace 规则；框架不猜测 fragment 合并语义。

## 7. Session 状态机

Viewer 同时最多存在一个 active direct-edit session：

```text
Idle
-> Starting
-> Editing
-> Composing
-> Accepting
-> Idle

Editing/Composing
-> Cancelling
-> Idle

Editing/Composing
-> Faulted
```

会话固定：

- Viewer namespace；
- source EditPartId；
- feature key；
- source model revision；
- draft document；
- text selection；
- layout/interaction revision；
- feedback owner；
- host text-input lease。

启动新会话前必须先按当前 policy 明确 accept 或 cancel 旧会话，禁止同时存在两个 IME
owner。source part retire、Viewer dispose、Editor fault 或 host lease 丢失会强制
cancel；不生成模型 Command。

## 8. 输入仲裁

直接编辑会话建立后优先于 active Tool：

1. pointer 命中 editor feedback 时更新 text selection；
2. keyboard/text-input event 交给 session；
3. session 已处理或持有 text-input lease 时，不进入 SelectionTool；
4. feedback 外 pointer press 先结束当前 session，再由结束结果决定是否继续分发；
5. `Escape` 在无 composition 时 cancel；accept key 由 mode 决定；
6. multiline 模式的 Enter 插入 hard break，显式 shortcut 才 accept。

Viewer selection 不因 caret 移动而改变。direct-edit source retire 时，Viewer selection
按既有 reconcile 规则独立处理。

## 9. Draft 与 Composition

Editor draft 保存 committed portion、selection 和可选 preedit range。平台事件归一化为：

```rust
pub enum TextInputEvent {
    Preedit { text: String, selection: Option<Range<usize>> },
    InsertText(String),
    Delete(TextDelete),
    Move(TextMovement, ExtendSelection),
    SelectAll,
    FocusLost,
}
```

preedit selection 使用相对 preedit string 的 UTF-8 byte range。收到 `Preedit` 时：

- 首次 preedit 替换当前 selection，并记录可撤销的 composition base；
- 后续 preedit 只替换当前 preedit range；
- `selection: None` 隐藏 composition caret，但不伪造空 selection；
- `InsertText` 先清除 preedit 标记，再把 committed text 写入 draft；
- IME disable/lost lease 清除未提交 preedit，并恢复 composition base。

平台 IME 的 text commit 只提交到 Editor draft，不等于 accept 编辑会话，也不进入
CommandStack。只有显式 accept 才产生一个模型 Command。

删除和 movement 以 grapheme/cluster、word 和 line 语义执行，不按 Unicode scalar 或
UTF-8 byte 盲删。Parley 可作为默认内部算法来源，但其 cursor/selection 类型不进入
公共 API。

## 10. Feedback Projection

draft、caret、selection 和 preedit 是 Viewer-owned transient feedback：

- 使用目标 TextFlow 的 resolved text style、constraints 和 transform；
- selection/caret geometry 来自 draft 的 immutable interaction map；
- preedit 使用独立 underline/selection presentation；
- caret surface rectangle 每次 layout、viewport、scale 或 transform 变化后重新输出；
- blink 使用 host 注入的单调时间与 wakeup，不使用全局 timer；
- feedback 不进入模型 serialization、Figure semantic snapshot 或 Command history。

Policy 提供 feature 到 feedback projection 的映射。原文本的临时隐藏也属于该
projection，不能直接改业务模型。cleanup 必须先恢复稳定视图，再执行模型 Command；
Command 完成后由模型通知产生新稳定文本。

## 11. Accept、Cancel 与冲突

accept 顺序：

```text
validate draft
-> require no active preedit
-> build Command with source revision
-> erase feedback and release host input
-> execute through CommandStack
-> model notification refreshes Figure
-> retire session
```

空变化不生成 Command。Command 必须只保存业务 identity、old/new value 与应用所需
revision，不保存 EditPartId、FigureId、layout 或 platform handle。

可恢复的 validation/command rejection 保留 session 和 draft，并重新建立 feedback；
target retire、Viewer fault 或无法证明模型一致时强制 cancel。source revision 过期必须
返回 `StaleSourceRevision`，不得覆盖外部修改。首批不做三方文本合并。

cancel 清除 preedit、feedback、blink wakeup 和 host lease，恢复模型通知对应的稳定
Figure，不产生 history。

## 12. Platform Host Protocol

Editor 通过平台无关 effect 请求：

```text
AcquireTextInput(session, purpose)
SetTextInputArea(session, surface_rect)
ReleaseTextInput(session)
RequestWakeup(deadline)
```

host 返回带 session identity 的 normalized event。迟到、重复或属于旧 lease 的事件必须
拒绝，不能写入当前 draft。

Native Winit adapter：

- active session 时显式允许 IME；
-把 `Enabled/Preedit/Commit/Disabled` 映射到 normalized protocol；
- caret 变化时更新 IME cursor area；
- dead-key 与 IME composition 期间不重复处理 keyboard text；
- session 结束时关闭 IME 并丢弃迟到事件。

Web adapter：

- 不依赖 Winit Web 的 IME API，因为其 IME allow/cursor area 当前不实现；
- 使用受控、不可见的 input host 获取 `compositionstart/update/end`、
  `beforeinput/input` 和 selection；
- `beforeinput` 可能缺失或不可取消，必须以 `input` 后的 host value 做确定性 reconcile；
- DOM input 只承载平台输入与软键盘，不作为可见文本或业务模型真值；
- input host 跟随 caret surface rectangle，session 结束后移除焦点和 composition 状态。

## 13. 失败模型

至少区分：

- `UnsupportedDirectEditFeature`；
- `DirectEditSessionAlreadyActive`；
- `ForeignOrRetiredPart`；
- `InvalidTextPosition`；
- `StaleTextLayoutRevision`；
- `StaleSourceRevision`；
- `InvalidPreeditRange`；
- `TextInputLeaseLost`；
- `HostTextInputUnavailable`；
- `DraftRejected`；
- `CommandRejected` / `CommandFailed`；
- `FeedbackCleanupFailed`。

非法位置、平台缺能力或 layout 过期不得 panic、clamp 或静默忽略。cleanup 失败破坏
Viewer 一致性时进入 Editor faulted。

## 14. 分批顺序

### P2-T02 Core Text Interaction Geometry

1. `FlowTextPosition`、affinity、range 与结构化错误；
2. immutable interaction map 与外部 TextLayoutEngine 构造校验；
3. point/position、caret、selection 和 movement 查询；
4. UTF-8、grapheme、wrap、hard break、bidi、truncate 与 transform 契约。

### P2-E02a Direct Edit Session

1. feature descriptor、typed request/policy role 与单 session lifecycle；
2. draft、selection、feedback、accept/cancel；
3. CommandStack、revision conflict、retire/fault cleanup；
4. headless deterministic contract。

### P2-E02b Platform Text Input

1. platform-neutral event/effect 与 lease identity；
2. Winit IME adapter 和 candidate area；
3. Web DOM input/composition bridge；
4. Native/Web 人工验收与输入序列 replay。

每批独立提交。P2-E02 只有在 Core、Editor、Native、Web 与失败路径全部闭合后才能标记
complete。

## 15. 验证入口

计划 suite、用例矩阵与人工验收见
[`../../verification/plans/p2-text-direct-edit.md`](../../verification/plans/p2-text-direct-edit.md)。
