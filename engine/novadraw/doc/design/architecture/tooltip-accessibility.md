# Tooltip 与 Accessibility Bridge 契约

类型：`normative-design`

状态：`accepted / complete`

范围：M10.5；`figure.properties`、`event.dispatcher`、`accessibility.bridge`

## 1. 目标与边界

M10.5 在现有 FigureStyle、InteractionState、focus traversal 与 stable scene 基础上补齐：

- Tooltip hover delay、show/hide、切换、超时和边界放置；
- Runtime 拥有的 accessibility snapshot/delta；
- Native、Web、Headless 共用的平台无关语义；
- focus 与 default action 的平台回调入口。

本阶段不实现：

- 富文本或任意 Figure 组成的 Tooltip；
- 可交互、可聚焦或可固定的 Tooltip；
- 完整 ARIA、AT-SPI、UI Automation 或 NSAccessibility 覆盖；
- accessibility selection provider、表格、文本编辑 range；
- 多 pointer、pointer cancel 与完整 IME 协议；
- GEF EditPart、handle、anchor 的 accessibility。

Tooltip 是平台呈现效果，不进入 FigureTree，不参与 layout、hit-test、selection 或
damage。Accessibility 是 stable scene 的派生语义投影，不建立第二棵可变业务树。

## 2. Draw2D 对应与 Novadraw 变体

Draw2D `SWTEventDispatcher` 独立维护 mouse target、cursor target 和 hover source；
hover source 沿祖先查找首个 Tooltip。`ToolTipHelper` 在指针附近显示 popup，优先下方，
空间不足时翻到上方，并在 source 变化、离开 control 或超时后隐藏。

Draw2D/SWT accessibility 通过 `LightweightSystem` 把平台查询转发给
`AccessibilityDispatcher`。GEF 再为 EditPart 注册独立 accessible ID。

Novadraw 保留：

- cursor、event target、hover source 分轨；
- Tooltip 继承和 source identity；
- 延迟显示、source 切换、离开隐藏与上下翻转；
- 稳定 node identity、name、role、state、bounds、children、focus 和 default action。

Novadraw 的合理变体：

- 不在 Figure 或 helper 内创建线程、Timer 或全局时钟；
- Runtime 保存 Tooltip 状态与 deadline，Host 只提供单调时间和 wake-up；
- 不复制 SWT 的同步逐字段查询，Runtime 发布不可变 snapshot/delta；
- accessibility ID 包含 Runtime namespace 与 Figure generation；
- 纯装饰 Figure 可被过滤，其 accessible descendants 提升到最近 accessible ancestor；
- 平台 adapter 不反向修改 Figure，只能调用 Runtime 的受控 action 入口。

## 3. 时间与平台边界

时间使用调用方提供的单调 tick，不读取 wall clock：

```rust
pub struct MonotonicTime(u64); // microseconds in one Host-defined epoch

pub struct TooltipTiming {
    pub show_delay: Duration,
    pub hide_delay: Duration,
}

impl Runtime {
    pub fn advance_time(&mut self, now: MonotonicTime) -> Result<bool, TimeError>;
    pub fn next_wake_deadline(&self) -> Option<MonotonicTime>;
}
```

同一 Runtime 接收的时间必须单调不减。倒退时间返回 `TimeError::NonMonotonic`，不得改变
Tooltip、interaction 或 accessibility 状态。默认延迟使用具名常量，不在业务路径写
magic number；测试可注入 `TooltipTiming`。

`PlatformHost` 扩展为：

```rust
pub trait PlatformHost {
    fn request_redraw(&self);
    fn surface_info(&self) -> SurfaceInfo;
    fn set_cursor(&self, cursor: CursorIcon);
    fn set_ime_state(&self, state: ImeState);
    fn schedule_wake(&self, deadline: Option<MonotonicTime>);
    fn update_tooltip(&self, update: TooltipUpdate);
    fn update_accessibility(&self, update: AccessibilityUpdate);
}
```

Runtime 不持有 Host。`novadraw-example-support` 在输入事务、timer wake、稳定帧发布和场景替换后，
按固定顺序同步：

```text
advance Runtime time
-> dispatch input / stabilize scene
-> drain Tooltip and Accessibility updates
-> Host update
-> schedule next wake
-> request redraw when scene pixels changed
```

Host wake 只要求“不早于 deadline 后最终唤醒”，不要求实时线程精度。HeadlessHost
记录 deadline，测试显式推进时间；Winit 使用 event loop deadline；Web 使用
`setTimeout`/`requestAnimationFrame` 组合并通过 generation 丢弃过期 callback。

## 4. Tooltip 状态机

### 4.1 输入与状态

Runtime 保存 primary pointer 的 logical-surface position。Tooltip source 继续使用
现有 hit-test traversal 与 FigureStyle 最近祖先解析，不新增第二套递归搜索。命中
Figure 后沿既有 parent chain 解析 source：

- `tooltip = Some(Some(text))`：当前节点成为 source；
- `tooltip = Some(None)`：显式关闭，终止解析且没有 source；
- `tooltip = None`：继续查找 parent。

因此同一带 Tooltip 的容器内跨 child 移动不会改变 source 或重启 delay。

```rust
enum TooltipState {
    Hidden,
    Waiting {
        source: FigureId,
        text: String,
        anchor: Point,
        show_at: MonotonicTime,
    },
    Visible {
        source: FigureId,
        text: String,
        anchor: Point,
        hide_at: MonotonicTime,
    },
}
```

状态持有 owned text，避免 style mutation 或 Figure dispose 后借用失效。状态中的
FigureId 必须属于当前 Runtime，reconcile 时验证 attached、effective visible/enabled
和当前 resolved tooltip。

### 4.2 转移规则

```text
Hidden + source(text)
  -> Waiting(show_at = now + show_delay)

Waiting + same source move
  -> 保留 show_at，只更新 anchor

Waiting + different source(text)
  -> 新 Waiting，重新计算 show_at

Waiting + no source / press / wheel / key / focus lost / scene replacement
  -> Hidden

Waiting + now >= show_at
  -> Visible(hide_at = now + hide_delay)，发布 Show

Visible + same source move
  -> 保持 visible，更新 anchor，只有位置变化时发布 Replace

Visible + different source(text)
  -> 立即 Replace，不再次等待；重置 hide_at

Visible + no source / press / wheel / key / focus lost / timeout
  -> Hidden，发布 Hide
```

Tooltip style 在 Waiting 期间变化时，以当前 resolved value 重新校验；变为 None 时
取消。Visible 内容变化发布 Replace。相同 update 不提升 revision。

### 4.3 平台更新与放置

```rust
pub enum TooltipUpdate {
    Show(TooltipSnapshot),
    Replace(TooltipSnapshot),
    Hide { revision: u64 },
}

pub struct TooltipSnapshot {
    pub revision: u64,
    pub source: FigureId,
    pub text: String,
    pub anchor: Point,
    pub placement: TooltipPlacement,
}
```

`TooltipPlacement` 表达 logical-surface anchor、首选下方、上方 fallback、具名 gap 和
可用 surface bounds。共享纯函数接收平台测得的 popup size：

```text
place below anchor
-> bottom overflow 时翻到 above
-> clamp x to surface
-> 两侧均不足时 clamp y to surface
```

Runtime 决定 source、文本、可见性、revision 和 anchor；Host 决定原生 popup/DOM/
surface overlay 的实际测量与绘制，但必须使用共享放置规则。Headless 测试以固定 popup
size 验证翻转与 clamp。首批 Tooltip 只接受纯文本。

## 5. Accessibility 语义模型

### 5.1 Figure capability

`AccessibleFigure` 扩展为只读语义提供者：

```rust
pub trait AccessibleFigure {
    fn accessible_name(&self) -> Option<&str>;
    fn accessible_description(&self) -> Option<&str> { None }
    fn accessible_value(&self) -> Option<&str> { None }
    fn accessible_role(&self) -> AccessibilityRole;
    fn accessible_default_action(&self) -> Option<AccessibilityAction> { None }
    fn accessibility_hidden(&self) -> bool { false }
}
```

首批 role：

```rust
pub enum AccessibilityRole {
    Group,
    Text,
    Image,
    Button,
    ToggleButton,
}

pub enum AccessibilityAction {
    Focus,
    Default,
}
```

内置映射：

- `LabelFigure`：`Text`，name 为完整 source text，不使用截断后的 glyph 文本；
- `ImageFigure`：只有显式 name 时进入树，role 为 `Image`；
- `ButtonFigure`：`Button`，name 为内部 Label source text，default action 为激活；
- `ToggleFigure`：`ToggleButton`，name 为内部 Label source text，包含 selected；
- `ClickableFigure`：只有自定义 accessible capability 时进入树，避免无名按钮。

第三方 Figure 通过 capability 提供语义，不需要 Runtime 增加具体类型分支。Runtime
只从通用 NodeState、InteractionState 与 Clickable capability派生 enabled、focused、
pressed、selected 等状态。

### 5.2 Identity 与树投影

```rust
pub enum AccessibilityNodeId {
    Root(RuntimeNamespace),
    Figure(FigureId),
}
```

Figure node ID 继承 namespaced generational identity；dispose 后旧 ID 永久失效。
每个 snapshot 包含一个 synthetic Runtime root。遍历从 contents 开始，遵守 10,000
深度上限和稳定 child order：

- detached、effective hidden 或 `accessibility_hidden` 节点不进入 snapshot；
- 没有 accessible capability 的装饰节点不进入 snapshot；
- 装饰节点的 accessible descendants 提升到最近 accessible ancestor；
- disabled 节点保留并带 `enabled = false`；
- bounds 使用 logical-surface domain，不使用 physical pixel；
- focus owner 只有在对应 node 已进入 snapshot 时才发布。

### 5.3 Snapshot 与 Delta

```rust
pub struct AccessibilitySnapshot {
    pub revision: u64,
    pub stable_epoch: u64,
    pub root: AccessibilityNodeId,
    pub nodes: Vec<AccessibilityNode>,
    pub focus: Option<AccessibilityNodeId>,
}

pub struct AccessibilityNode {
    pub id: AccessibilityNodeId,
    pub parent: AccessibilityNodeId,
    pub children: Vec<AccessibilityNodeId>,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    pub role: AccessibilityRole,
    pub state: AccessibilityState,
    pub bounds: Rectangle,
    pub default_action: Option<AccessibilityAction>,
}

pub enum AccessibilityUpdate {
    Snapshot(Arc<AccessibilitySnapshot>),
    Delta(AccessibilityDelta),
}
```

首次发布、场景替换和 Host 重建必须发送 Snapshot。后续只在语义内容变化时提升
revision 并发送 Delta；Delta 至少包含 base/new revision、upserts、removed、focus 和
root children 变化。顺序错误或 base revision 不匹配时，adapter 必须请求/等待下一份
Snapshot，不得拼接到未知基线。

Accessibility 只能消费 stable scene。source mutation 尚未收敛、Runtime faulted 或
frame preparation 失败时，不发布半成品更新。snapshot 生成失败不替换上一个已发布
版本，并按 Runtime faulted 边界处理 extension panic。

### 5.4 Action 回流

```rust
impl Runtime {
    pub fn perform_accessibility_action(
        &mut self,
        node: AccessibilityNodeId,
        action: AccessibilityAction,
    ) -> Result<bool, AccessibilityError>;
}
```

- `Focus` 复用 `request_focus`，保持 focus eligibility 与事件顺序；
- `Default` 仅对声明 default action 的节点执行；
- Button/Toggle 复用 `do_click`，保持 selected property change 先于 ActionEvent；
- unknown、foreign、disposed、hidden、disabled 或 unsupported action 返回结构化错误；
- action 仍经过 Runtime 事务、faulted 和 notification 边界。

## 6. 更新因果与脏标记

Tooltip dirty 输入：

- hover source 或 pointer position；
- resolved tooltip style；
- visible/enabled、reparent、dispose、contents replacement；
- clock deadline。

Accessibility dirty 输入：

- topology、visible/enabled、bounds 或坐标链；
- accessible capability source revision；
- Label source text、Image accessible name；
- clickable selected/pressed；
- focus owner；
- contents replacement。

Accessibility 不在每帧无条件扫描。首批实现可在 semantic dirty 时生成完整候选
snapshot，再与已发布 snapshot 比较并降低为 delta；正常纯 repaint 不触发更新。
候选计算和发布必须整体完成，不能按节点边遍历边调用 Host。

## 7. 扩展点与稳定性

- 新 Figure 通过 `AccessibleFigure` 加入语义树；
- 新 role/action 只扩展平台无关枚举，不暴露平台常量；
- 自定义 Tooltip visual、rich content 和 interactive popup 需要后续独立协议；
- Native adapter 可接 AccessKit，Web adapter 可映射 DOM/ARIA，二者不能改变 engine
  snapshot 的父子关系或状态；
- accessibility snapshot/delta 是公共稳定数据，不暴露 SlotMap local key、Figure
  可变引用或 backend 对象。

## 8. 错误模式

- `TimeError::NonMonotonic`：Host 时间倒退，状态不变；
- `TimeError::Overflow`：deadline 溢出，拒绝本次推进；
- `AccessibilityError::UnknownNode`：ID 不属于当前 snapshot；
- `AccessibilityError::ForeignRuntime`：namespace 不匹配；
- `AccessibilityError::Unavailable`：节点 hidden、disposed 或不在 stable snapshot；
- `AccessibilityError::UnsupportedAction`：节点未声明该 action；
- `AccessibilityError::RuntimeFaulted`：Runtime 已 faulted；
- adapter baseline mismatch：丢弃 delta，等待 Snapshot；
- extension accessibility callback panic：Runtime faulted，不发布候选 snapshot。

## 9. 实施批次

### M10.5a Tooltip Core

- 单调时间、timing 配置与 TooltipController；
- pointer/source/style/lifecycle reconcile；
- TooltipUpdate、共享 placement 与 HeadlessHost；
- 状态机、超时、source 切换、边界放置契约测试。

### M10.5b Accessibility Core

- role/state/action、扩展后的 AccessibleFigure；
- stable snapshot/delta 与 namespaced node identity；
- Label/Button/Toggle 内置语义；
- focus/default action 回流与 Headless 契约测试。

### M10.5c Platform 与产品验收

- Winit wake 与 Tooltip surface overlay；
- Web timer、Tooltip DOM 与 accessibility DOM bridge；
- Winit adapter 消费同一 accessibility snapshot；完整 AccessKit/VoiceOver provider
  保持在 Core 1.0 之后；
- Tooltip/Accessibility demo；
- Native/Web/Headless 自动、视觉与人工验收；
- API coverage、产品清单、demo 矩阵和路线图同步。

## 10. 完成门禁

- Tooltip 延迟、source 切换、隐藏、超时、style mutation 和 dispose 顺序可重复；
- below/above 与横纵 clamp 有确定性测试；
- Runtime/Figure 不创建线程、Timer、全局 clock 或平台对象；
- Accessibility snapshot 只来自 stable epoch；
- node identity、层级提升、bounds、focus、enabled、selected、pressed 正确；
- Button/Toggle default action 复用既有事务与 notification 因果；
- Snapshot/Delta baseline mismatch 有明确恢复；
- Headless、macOS、Web 使用同一引擎 snapshot；
- Tooltip demo 和键盘可达性人工验收通过；
- `figure.properties`、`event.dispatcher`、`accessibility.bridge` 覆盖账本同步；
- 不修改 `render_recursive.rs` 主循环语义。
