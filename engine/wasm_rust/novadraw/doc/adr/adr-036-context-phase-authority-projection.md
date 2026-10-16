# ADR-036：Context 阶段权限投影与 Query 命名

类型：`architecture-decision`

## 状态

已接受并验证（2026-10-13）。实现与验证证据见
[完成记录](../verification/reviews/adr-036-context-phase-authority-completion-2026-10-13.md)。

本决策统一 Novadraw Core 与 Editor callback context、只读 query 和延迟写入出口的
角色模型。第一批迁移覆盖 Event、Layout 与 Connection Routing；既有 Paint、Measure、
Stable Observation 和 Editor Tool 边界按本决策审计，不为了命名一致制造无意义包装。

## 背景

Runtime 是 attached scene 的唯一状态 owner 和提交权威，但扩展 callback 需要读取当前
阶段允许观察的事实，并产生受检输出或延迟 effect。把 `&mut Runtime`、`&mut FigureTree`
或其他 owner 直接交给 callback 会造成：

- callback 在 owner 已借用目标对象时重入 Runtime；
- 绕过 source commit、derived convergence、damage、journal 和 stable publication；
- 暴露与当前阶段无关的全局权限；
- 把 Runtime 或 FigureTree 的内部组织冻结成扩展 API；
- 让外部实现获得无法声明、审计和测试的隐式依赖。

现有协议已经分别采用受限视图：

- `EventContext` 记录 FIFO callback effects；
- `LayoutSnapshot` 提供只读布局事实，`LayoutOutput` 承载原子候选；
- routing `SceneQuery` 提供 tracked geometry query，`RouteOutput` 承载路径候选；
- `MeasureContext` 借用文字测量服务；
- `PaintContext` 只追加绘制命令；
- `StableSceneQuery` 只读取已发布 stable epoch；
- `ToolContext` 分派 typed request，不暴露 Runtime、Viewer 或 CommandStack。

这些边界方向一致，但命名没有表达 phase、subject 和 role，`EventContext` 还容易不断增加
单值 convenience getter。UML resize 场景临时增加的
`EventContext::parent_content_bounds()` 暴露了该问题：业务需要的是 Event 阶段的
scene query，而不是 Core 为每个业务问题增加一个扁平 getter。

Draw2D/GEF 允许 Figure、LayoutManager、Router 和 Tool 访问较宽对象图。Novadraw 保留
其扩展语义，但基于 Rust 所有权、事务提交和 stable publication，将权限投影为短生命周期
只读 query、纯服务和受检 output/effect。

## 决策

### 1. Runtime 是 owner，Context 是阶段权限投影

统一概念模型为：

```text
Runtime / EditorDomain owner
→ freeze current causal prefix or stable revision
→ construct phase-scoped authority projection
→ invoke callback
→ release callback-owned borrows
→ validate output/effects
→ commit through the original owner
```

一个 callback context 可以由以下角色组成：

```text
<Phase>Context
├── identity / metadata
├── <Phase><Subject>Query
├── <Phase>Services
├── <Phase>Capabilities
└── <Phase>Output / Effects / Commands
```

这是角色模型，不要求每个 Context 都包含五类成员，也不要求所有 callback 都增加一个
包装 struct。某个阶段只需要 query 与 output 时，可以继续把二者作为独立参数，从签名
直接表达“读事实、写候选”的边界。

Context、Query、Services、Capabilities 和写入出口都不拥有 Runtime 状态。它们只能在
callback 生命周期内借用 owner 的受限投影或 callback-local sink。

### 2. 禁止从 Context 逃逸 owner

公开 callback context 和 query 不得返回：

- `&Runtime` 或 `&mut Runtime`；
- `&FigureTree` 或 `&mut FigureTree`；
- `&UpdateManager` 或其可变引用；
- `&FigureNode`、`&dyn Figure` 或 concrete Figure downcast；
- 可以同步提交 scene mutation 的任意 closure；
- backend、surface、window 或平台句柄。

只读 query 返回 backend-neutral owned value、稳定身份、短生命周期 iterator 或明确的
不可变结果。写操作只能进入本阶段公开的 Output、Effects 或 Commands。

### 3. 统一命名语法

公开类型采用：

```text
<Phase><Subject><Role>
```

| 位置 | 含义 | 示例 |
|---|---|---|
| Phase | 权限生效的计算或事务阶段 | Event、Layout、Route、Paint |
| Subject | 被查询或服务的领域 | Scene、Resource、Text、Style |
| Role | 类型承担的角色 | Context、Query、Services、Output、Effects、Commands |

规则：

1. callback 聚合入口命名为 `<Phase>Context`；
2. 场景只读入口命名为 `<Phase>SceneQuery`；
3. 单领域服务命名为 `<Phase><Subject>Services` 或既有明确领域 Context；
4. 完整候选结果命名为 `<Phase>Output`；
5. 有因果顺序、callback 后提交的操作命名为 `<Phase>Effects`；
6. 只追加的录制语言命名为 `<Phase>Commands`，具体公开类型可以是 Recorder；
7. 不使用无法表达 subject 的 `LayoutQuery`、`PaintQuery` 等缩写；
8. 不为了凑齐名字创建没有权限内容的空 Query。

规范名称包括：

| 阶段 | 规范名称 | 说明 |
|---|---|---|
| Input callback | `EventContext`、`EventSceneQuery`、`EventEffects` | 读取当前 committed prefix，延迟写入 |
| Layout | `LayoutSceneQuery`、`LayoutOutput` | 读取布局事实，生成完整候选 |
| Routing | `RouteSceneQuery`、`RouteOutput` | tracked geometry/anchor query 与路径候选 |
| Text measurement | `MeasureContext` | 已明确属于 `text` 领域，不增加空包装 |
| Figure paint | `PaintContext` | 绘制命令权限，不提供 scene query |
| Stable observation | `StableSceneQuery` | `Stable` 表示 publication 状态，不是 callback phase |
| Editor tool | `ToolContext` | typed request/command 权限投影，不暴露 Viewer/Runtime |
| Figure update effect | `FigureUpdateEffectContext` | committed tree 只读查询、target-scoped 意图声明与整批预检；不授予外部 commit 权限 |

### 4. Query、Snapshot 与 owned result 不混用

- `Query` 是 callback 期间借用的只读接口，不能离开其 owner/context 生命周期；
- `Snapshot` 是带 revision 的 owned immutable value，可以在 callback 外保存；
- `Output` 是尚未提交的完整计算候选；
- `Effects` 是按产生顺序记录、在 callback 返回后提交的受限意图；
- `Commands` 是只追加的有序录制流，不表示提交或呈现完成。

因此，现有 `LayoutSnapshot` 实际是短生命周期借用 query，应直接迁移为
`LayoutSceneQuery`，不保留 deprecated alias。真正需要持久化布局状态时必须另建带
revision 的 owned snapshot 类型。

`StableSceneQuery` 保留该名称：它是对最新 stable epoch 的短借用 query，不是历史
scene snapshot。历史 effect payload 与当前 stable query 不得混为同一时刻的完整状态。

### 5. EventContext 分离身份、Query 与 Effects

目标结构为：

```rust,ignore
pub struct EventContext<'a> {
    target: EventTarget,
    scene: EventSceneQuery<'a>,
    effects: EventEffects<'a>,
}
```

实际实现可以为借用约束保留私有字段，不冻结上述字段布局。公共语义是：

- target/event metadata 由 `EventContext` 直接提供；
- scene facts 只从 `context.scene()` 进入；
- mutation、repaint、focus、capture、property notification 只从
  `context.effects()` 进入；
- `EventSceneQuery` 查询 callback 开始时的 committed prefix；
- 当前 callback 新记录的 effect 在 callback 返回前不可由 query 观察；
- effect 按记录顺序提交，query 不隐式 flush、validate 或 stabilize。

`EventSceneQuery` 第一批提供通用身份、拓扑与几何值查询。它不提供原始 node/figure
访问。UML 通过 `parent_id(target)` 与 `child_content_bounds(parent)` 计算拖拽边界。

删除 `EventContext::parent_content_bounds()`。不得保留 alias；该 getter 的业务组合逻辑
应由消费方通过 query 明确表达。

### 6. Layout 与 Route 保持 Query → Output 的显式边界

LayoutManager 继续消费只读 query 并写入独立 output：

```rust,ignore
fn layout(
    &mut self,
    container: FigureId,
    query: &LayoutSceneQuery<'_>,
    output: &mut LayoutOutput,
) -> Result<(), LayoutError>;
```

不增加持有 query 与 output 的万能 `LayoutContext`，因为独立参数清楚表达：

- query 不观察本轮 output；
- output 完整校验后才原子提交；
- measurement 可以只接收 query，不获得写权限。

Router 同理消费 `RouteSceneQuery` 与 `RouteRequest`，返回 `RouteOutput`。tracked
dependency recording 是 Route query 的领域语义，不进入通用 Event query。

### 7. Paint 不获得 SceneQuery

Paint 阶段消费最终 geometry、resolved style 和已准备 presentation，只通过
`PaintContext` 追加受检绘制命令。不得为了命名对称增加 `PaintSceneQuery`。

需要拓扑、layout、text shaping 或资源状态决策的工作必须在 paint 前完成。PaintContext
可以读取当前绘制状态和已 lease 的资源，但不能查询 FigureTree、damage 或 frame owner。

### 8. Context capability 必须类型化且按阶段授权

未来真实外部消费者需要注入额外服务时，可以引入：

```rust,ignore
ContextKey<Phase, Capability>
ContextCapabilities<Phase>
```

约束：

- key 使用 typed identity，字符串只作诊断；
- 内置与外部 capability 使用同一 registration 和 lookup；
- capability 在 callback 安装前完成声明与校验；
- lookup 只返回当前 phase 授权的只读服务；
- 不使用公开 `AnyMap`、裸 `TypeId` 或字符串 service locator；
- capability 不得携带 `&mut Runtime` 或任意同步 mutation closure。

本 ADR 不因缺少真实消费者而立即增加通用 capability registry。新增 registry 必须提供
外部消费者证据和 phase-specific 生命周期。

### 9. 错误与因果语义

- callback target 在 EventContext 构造前已验证；
- query 任意 FigureId 时，unknown/disposed/foreign 与合法无 parent 必须可区分；
- query 不触发 layout、routing、resource completion 或 stable publication；
- effect 入队成功不表示提交成功，deferred outcome 保留既有错误通道；
- Layout/Route output 失败不发布部分候选；
- Paint command 失败不出版残缺帧；
- callback panic 按现有 fault boundary 处理，不声称回滚外部 interior mutation。

第一批 Event query 可以复用现有 tree identity 错误；若底层 API 尚未区分 foreign，
不得用 `None` 同时表达身份错误和合法无 parent。

### 10. 直接 breaking 与迁移范围

项目仍为 `0.1.0`。第一批直接迁移：

1. 新增 `EventSceneQuery` 与 `EventEffects`；
2. Event callback 调用点迁移到 `scene()` / `effects()`；
3. 删除 `EventContext::parent_content_bounds()`；
4. `LayoutSnapshot` 重命名为 `LayoutSceneQuery`；
5. routing `SceneQuery` 重命名为 `RouteSceneQuery`；
6. 更新公开领域导出、API snapshot、examples、tests 与文档；
7. 审计 `PaintContext`、`MeasureContext`、`StableSceneQuery`、`ToolContext`，只修订不符合
   权限模型的入口，不做机械重命名。

不保留 deprecated alias、双入口或 root re-export。

## 不采用

### Context 持有 Runtime 或 FigureTree

这会把最大权限 owner 变成 service locator，并允许 callback 绕过提交事务。

### 一个全阶段 UniversalContext

运行时检查“当前阶段是否支持某能力”不如由不同具体类型在编译期限制权限，也会使外部
实现依赖偶然可用的服务。

### 所有 Context 都包含 SceneQuery

Paint、纯测量和 command recording 不需要场景拓扑。强制提供只会扩大权限并形成阶段
反向依赖。

### 每遇到一个业务需求就在 EventContext 增加 getter

`parent_content_bounds`、`siblings`、`viewport_size` 等扁平 getter 会让 Context 演变成
不完整 Runtime。通用 scene fact 应由 `EventSceneQuery` 组合，业务策略留在消费方。

## 外部消费者证据

第一批迁移必须证明：

1. 外部 Figure event handler 只通过 `EventSceneQuery` 读取 parent 与 content geometry；
2. 外部 handler 只通过 `EventEffects` 排入 bounds mutation、repaint 和 typed property；
3. callback 无法取得 Runtime、FigureTree、FigureNode 或 mutable Figure；
4. 外部 LayoutManager 使用 `LayoutSceneQuery + LayoutOutput`；
5. 外部 Router 使用 `RouteSceneQuery + RouteOutput`；
6. 内置与外部消费者使用相同 API、错误和提交路径。

UML example 是 Event query 的产品消费者：窗口扩大或缩小后，拖拽边界使用当前 parent
child-content area，不依赖初始窗口常量。

## 验证

1. source gate：公开 Context/Query 不返回 Runtime、FigureTree、FigureNode；
2. source gate：不存在 `EventContext::parent_content_bounds`；
3. API snapshot 冻结 Event/Layout/Route 新名称并删除旧名称；
4. event contract 覆盖 committed-prefix query 与 deferred effect 不可见性；
5. UML 覆盖扩大、缩小 logical viewport 后的拖拽边界；
6. layout、routing、paint、stable observation 和 Editor Tool 定向 suite 通过；
7. `cargo xtask docs`、workspace fmt/check；
8. 因涉及公共跨 crate API，完成边界执行一次 workspace fmt、Clippy 与 test。

## 实施与回滚

按以下顺序实施：

1. 冻结 ADR 与命名；
2. 新增 Event query/effects，并迁移 Event callback；
3. 删除临时 `parent_content_bounds`；
4. 迁移 Layout 与 Route query 名称；
5. 更新 API snapshot、外部消费者和文档；
6. 完成定向、quick 与 full gate；
7. 写完成记录。

代码提交按 Event、Layout/Route、文档与验证主题保持原子化。任一批次可以通过
`git revert` 回退；不通过恢复旧 alias 形成长期双入口。

## 关系

- 延续 [ADR-009](adr-009-runtime-dynamic-mutation-contract.md) 的 callback FIFO effect；
- 延续 [ADR-011](adr-011-derived-state-convergence.md) 的 Query → Output 原子提交；
- 延续 [ADR-025](adr-025-unified-graphics-and-glyph-preparation.md) 的阶段受限视图；
- 延续 [ADR-031](adr-031-graphics-recording-and-publication-boundary.md) 的 Paint 权限；
- 延续 [ADR-033](adr-033-runtime-subsystem-facade.md) 的 Runtime 组合根边界；
- 延续 [ADR-034](adr-034-editor-tool-request-handle-protocol.md) 的 ToolContext 权限投影；
- 补充 [动态协议](../design/architecture/dynamic-architecture.md) 的 callback/query
  命名与权限规则。

## 日期

2026-10-13
