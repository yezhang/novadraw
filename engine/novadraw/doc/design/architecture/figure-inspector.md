# FigureInspector

类型：`normative-design`

规范效力：`accepted`

实现状态：`partial`（本地快照与事件观察已实现；跨进程宿主与 wire protocol 延后）

## 1. 目标

FigureInspector 为开发、测试和工具宿主提供对稳定 Figure 场景和提交后事件的只读观察。
它用于解释以下问题：

- 某个 Figure 在树中的 containment、Z-order、bounds 和节点状态；
- 一次 geometry/property/layout/update effect 的发生顺序；
- 为什么当前画面与上一个稳定事务不同。

它不负责修改场景、控制 Runtime、记录 GPU 命令流或替代应用日志。

## 2. 所有权与依赖

```text
Runtime
├── FigureTree
├── UpdateManager -> committed NotificationRecord
└── stable_query()
        |
        v
novadraw-inspector
├── FigureTreeSnapshot
└── bounded EventTimeline
        |
        v
Native / Web / headless host
```

`novadraw-inspector` 只依赖 `novadraw` Core。它不依赖 `novadraw-editor`、RenderBackend
或平台 crate。Editor 关联属于宿主 adapter 的可选增强：

```text
FigureId -> VisualOwner -> EditPartId -> ModelId
```

不得把上述映射复制到 Runtime，也不得从 `FigureId` 推断业务模型身份。

## 3. 进程与开发态协议

Figure 场景与 Inspector 必须作为两个可独立启动的 app，而非同一窗口中的内嵌面板：

```text
Figure App                         Inspector App
┌────────────────────┐            ┌──────────────────────────┐
│ Runtime             │            │ connection manager       │
│ FigureTree          │            │ outline tree             │
│ devtools bridge     │ <--------> │ properties / timeline UI │
│ host-only overlay   │            │ offline trace reader     │
└────────────────────┘            └──────────────────────────┘
```

- Figure App 是被观察对象，拥有 Runtime、平台窗口和真实用户输入；
- Inspector App 是独立诊断工具，即使未连接 Figure App 也可以浏览离线 trace；
- `novadraw-inspector` 保持本地观察与快照模型；
- 首个跨进程消费者出现前，wire message schema 与 transport adapter 属于具体宿主；
  只有出现独立版本兼容或多个真实消费者时，才评估
  `novadraw-inspector-protocol` package；
- 若后续拆分 protocol package，它不得依赖 Runtime、Vello、winit 或具体 UI；
- Figure App 只在明确启用开发态 `devtools` feature 时创建 bridge；产品构建不监听
  Inspector 端点。

该边界遵循 ADR-023：协议 package 是有版本与复用证据后的可选结果，不是当前设计的
预先交付项。

### 3.1 本地传输与会话

macOS 开发态默认使用 Unix domain socket，端点文件权限必须为 owner-only。其他平台使用
loopback TCP。浏览器 Inspector 只能通过明确配置的 loopback WebSocket bridge 连接，
不得直接暴露公网端口。

Figure App 启动时生成带协议版本、endpoint、随机 capability token 和进程身份的会话
descriptor。descriptor 可以通过 CLI、环境变量或受限临时目录文件交给 Inspector。
Inspector 的连接管理器支持零个或多个 session；断连、版本不兼容或 capability 校验失败
必须显示为结构化状态，不能降级为未认证连接。

首版 wire framing 使用 length-prefixed JSON，优先保证 trace 可读性和协议演进可审查。
只有在存在可测量的吞吐或延迟瓶颈时，才能引入二进制编码。

### 3.2 消息边界

Figure App 只向 Inspector 发布已提交的事实：

```text
Hello / SessionState
StableSnapshot / SnapshotDelta
EffectBatch
InputTrace
```

`InputTrace` 由 Figure App 的输入适配层在调用 `Runtime::dispatch_*` 后记录，至少包含
归一化输入摘要、`DispatchOutcome` 的 target、handled 与 capture。它不是
`NotificationEffect`，也不能伪造为 Runtime 的历史事务事件。

Inspector 只可发送以下诊断控制请求：

```text
RequestSnapshot
SetInspectionFocus
HighlightFigure / ClearHighlight
```

协议不得开放任意 Runtime mutation、模型写入、Command 执行或任意 Rust 回调。
跨进程 Figure 身份必须使用带 session namespace 的协议 identity；不得将 `Debug` 文本或
SlotMap 内部表示当作稳定 wire format。

### 3.3 高亮与选择

Inspector 选中 Figure 后可以请求 Figure App 高亮对应 bounds。高亮由 Figure App 的
平台宿主 overlay 完成，不作为 Figure 添加到受观察的 FigureTree，也不产生场景 mutation、
damage 语义或 Inspector event。这样诊断工具不会改变被诊断场景。

## 4. 稳定快照

`FigureTreeSnapshot` 必须只在 `Runtime::stable_query()` 成功后创建，并携带该 query 的
epoch。每个 `FigureSnapshot` 至少包含：

- `FigureId`、parent、child count、tree depth 和直接 child order；
- Figure 的稳定 diagnostic name；
- local bounds；
- visible、enabled、valid、opaque、focusable 状态；
- 是否仍挂接到当前根。

快照使用 FigureTree 的真实 containment 顺序。它不展示连接依赖、Editor PartTree 或
渲染命令树为 containment child。

## 5. 事件时间线

Inspector 的 observer 只接收已经提交的 `NotificationRecord`，并按
`(source_epoch, sequence)` 保存原始 effect。历史记录是有界 FIFO：

```text
record arrives
-> append
-> if count > capacity: discard oldest
```

观察者不得取得 `&mut Runtime`，不得在 callback 中排入 mutation，也不得将当前
`StableSceneQuery` 误标为历史时刻的完整场景。事件发生时的旧值只来自 effect payload；
Inspector 属性面板显示的是当前稳定快照。

## 6. 可视化方案

主视图采用三栏诊断工作台，而非图自动布局：

```text
outline tree | selected Figure properties | event timeline
```

### 5.1 Outline tree

- 使用可虚拟化的展开/折叠树；
- 行顺序严格等于 FigureTree 的 parent/child 与 child order；
- 行首展示 Figure name、identity 短表示、状态标记和 bounds 摘要；
- 选中一行时展开 ancestor path，并在画布 overlay 高亮对应 bounds；
- 过滤命中时保留 ancestor context，不能只显示孤立节点。

树是主视图，因为 containment、Z-order 与深度是调试语义，而不是待优化的图布局。
面对 10,000 层树，虚拟列表只渲染已展开节点的可见行。

### 5.2 属性面板

显示选中 Figure 的完整 snapshot，并按稳定次序分组：

1. Identity 与 tree relation；
2. Geometry、visibility、enabled、validity；
3. Style、clip、transform 和 capability 的后续扩展字段；
4. 可选 Editor owner adapter 数据。

### 5.3 Event timeline

- 默认按 `source_epoch`、`sequence` 升序；
- 支持 effect 类别、FigureId、epoch 和文本过滤；
- 选中事件时高亮 payload 涉及的 Figure，并显示 old/new 数据；
- 时间线不能声称当前树就是该事件发生时的树。

首版不使用 force-directed graph、DAG layout 或完整历史回放。这些视图可以作为连接依赖
或事件因果的后续辅助视图，但不能取代 containment tree。

### 6.4 Native Inspector host

首个独立 Inspector App 使用 `winit + egui` 构建开发工具界面。egui 的虚拟列表、树、
表格和可调整分栏适合高密度诊断工作台；它运行于 Inspector 自己的进程和 GPU surface，
不共享 Figure App 的 Vello renderer、Runtime 或事件循环。

UI 的 composition root 只创建 protocol client、session store 与 view state。FigureTree
flattening、展开状态、过滤和选中状态均属于 Inspector UI，不得回写 `FigureTree`。

## 7. 性能与失败边界

- Snapshot 和 event buffering 不得置于 paint/validation/damage/render 热路径；
- Tree flattening 由宿主按展开状态执行；引擎 snapshot 不预先构造 UI 行；
- event capacity 必须显式且有界；
- Runtime 未稳定或 faulted 时捕获失败必须原样返回 `StableQueryError`；
- inspector 自身内存分配失败或宿主渲染失败不得影响 Runtime 事务。
- IPC 背压、慢 Inspector、断连或高亮 overlay 失败不得阻塞 Figure App 的输入、更新或渲染；
- Inspector 不得把一个事件对应的当前 stable snapshot 表述为该事件发生时的历史完整场景。

## 8. 首期验证

1. 稳定场景捕获返回真实 parent/child 顺序和节点诊断属性；
2. 未稳定 Runtime 拒绝快照；
3. observer 按 sequence 保留提交后事件；
4. 缓冲区满时淘汰最旧事件；
5. 捕获与事件观察不改变 Runtime 的稳定 epoch、Figure bounds 或通知顺序。
6. Figure App 与 Inspector App 可以分别启动、认证连接、断连重连；
7. `InputTrace` 与 `DispatchOutcome` 一一对应，且不改变 Runtime dispatch 行为；
8. Inspector 的高亮请求不会改变 FigureTree、notification sequence 或场景 damage；
9. Inspector 断连或消费滞后不阻塞 Figure App 的 frame preparation。

## 关系

- [UpdateManager](../rendering/update-manager.md)
- [ADR-016](../../adr/adr-016-figure-inspector-observability.md)
- [ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md)
- [P2 delta backlog](../../roadmap/p2-delta-backlog.md)
