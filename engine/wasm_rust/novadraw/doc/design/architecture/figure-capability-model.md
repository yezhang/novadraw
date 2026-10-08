# Figure Capability 统一扩展模型

类型：`normative-design`

状态：`accepted`，P2-F02 已实现

日期：2026-10-08

`api_semantics`：`paint.protocol`、`figure.lifecycle`、`figure.box.client_area`、
`figure.properties`、`event.input_listeners`、`accessibility.bridge`、
`viewport.scroll_zoom`、`connection.figure`、`text.flow`、`widgets.basic`

## 1. 目标

Draw2D 依赖 Java 继承、虚方法和对象内状态，让子类覆盖 `paintFigure`、
`getPreferredSize`、`containsPoint`、坐标转换、输入和生命周期方法。Novadraw 不复制
继承层次，而是将同一扩展能力拆成：

```text
固定 Runtime phase scheduler
    + attach-time typed capability registry
    + Runtime-owned prepared mutation
    + immutable derived/presentation snapshots
```

本模型必须同时满足：

1. 新增第三方横切能力不修改 `Figure` 基础 trait 或 Core 中心枚举；
2. Figure 不能覆盖树遍历、validation、damage、notification 和 frame publication 顺序；
3. 公共 API 不暴露任意 `&mut dyn Figure`、字符串 capability 名或调用方 downcast；
4. capability discovery、生命周期、错误和 mutation 使用同一基础协议；
5. 行为、共享模型、marker 和 provider 保持各自领域类型，不合并成万能回调接口；
6. 内置能力与外部能力通过同一路径接入并接受相同门禁。

Capability 的判定门槛不是“Figure 是否拥有独特数据或方法”，而是：

> 是否存在一个独立消费者，它不知道具体 Figure 类型，却必须按同一语义合同发现并
> 消费多个可替换实现？

不满足该条件的图形特性保留在具体 Figure、共享具体模型或 typed component update 中。

## 2. 非目标

- 不实现 Java 多重继承、mixin 或任意 around-hook；
- 不让未知 capability 自动进入 Runtime 调度；没有显式消费者的能力只是已登记数据；
- 不把 Figure 私有字段全部迁入全局属性表；
- 不以 `HashMap<String, Any>`、公开 `TypeId` downcast 或进程全局 registry 作为扩展面；
- 不改变 `NodeState`、`LayoutState` 和 Presentation Plane 的事实归属；
- 不在本 delta 中重写递归渲染遍历顺序；
- 不把跨多个 Figure/registry 的复合操作伪装成可自动回滚的 capability update。
- 不因为两个 Figure 复用了同一种数据结构，就为该数据结构创建 behavior trait 或
  capability；
- 不要求具体类型的专用 facade 对外伪装成开放的多态扩展点。

## 3. 与 Draw2D 的对应

| Draw2D 扩展方式 | Novadraw 对应 |
|---|---|
| 覆盖 `paintFigure` | 固定 Paint 阶段消费 `FigurePreparation` / `FigurePresentation` |
| 覆盖 preferred/minimum size | Measure/Preparation 阶段产出受检 measurement |
| 覆盖 `containsPoint` | 标准 hit-test 行为；首批保留 Figure 核心方法 |
| 覆盖 child 坐标与 clip | `ContainerCapability` descriptor |
| listener / event override | `InputCapability` descriptor |
| `addNotify/removeNotify` | `LifecycleCapability` descriptor |
| `ScalableFigure` | `ScaleCapability` + `ScaleModel` |
| Layer/Freeform 接口 | marker capability |
| Connection 等跨类型服务接口 | typed capability descriptor |
| Polyline/TextFlow/Image 等具体类型 | 具体状态 + inherent behavior + typed component update |
| 子类 setter | scoped facade + prepared capability/component update |

差异的核心是：Draw2D 的对象决定“方法如何继续调用”，Novadraw 的 Runtime 决定
“何时调用哪个阶段”；Figure 只提供该阶段所需的 typed 行为。

## 4. 状态所有权

### 4.1 `NodeState`

`NodeState` 继续是所有 Figure 共有事实的唯一真源：

- bounds、insets、style；
- visible、enabled、opaque、focusable；
- size override 和 child clipping override。

Capability 不得复制这些字段形成第二份真值。查询和 mutation 需要这些值时，由 Runtime
提供只读上下文或通过通用 `FigureMut` 提交。

### 4.2 Figure component state

`Box<dyn Figure>` 保存具体 Figure 的私有内容和算法配置。只有该 Figure 自身需要理解的
状态继续使用 `FigureComponentUpdate`；它不需要进入 capability registry。

不同 Figure 可以自然拥有不同的数据结构与内在行为。例如 `PolylineFigure` 持有
`PointList` 和 `StrokeStyle`，`TextFlowFigure` 持有 paragraph/fragment 与排版结果。
这些差异由具体结构体和 inherent method 表达，不要求抽象成 behavior trait。

### 4.3 Shared concrete model

多个具体 Figure 可以组合复用普通结构体和算法，例如 `PointList`、`StrokeStyle`、
`ScaleModel` 或文本 fragment。共享实现不等于动态多态：

- 没有独立跨类型消费者时，使用 concrete model、泛型函数或 crate-private helper；
- 专用 Runtime facade 可以显式支持一组内建类型，并在 crate 内使用闭合 adapter；
- 只有出现真实的外部替换需求和统一消费者后，才升级为 capability。

### 4.4 Capability model

需要被其他引擎模块发现的领域状态使用 capability model，例如：

- `ScaleModel`；
- Clickable selection/visual state；
- Connection route presentation；
- 外部 Figure 的自定义可观察模型。

Model 可以提供可克隆只读 handle，但写入方法必须保持 crate-private，并由 Runtime facade
执行。

### 4.5 Derived state

`LayoutState`、prepared drawing、border snapshot、route 和 accessibility snapshot
仍是可重建派生状态。Capability mutation 只声明 invalidation，不直接写这些缓存。

### 4.6 Presentation state

Animation override 和 temporary visual 由 Runtime-owned Presentation Plane 持有。
`PresentationBinding<V>` 通过 capability registry 发现目标，读取 committed value，
并把采样值准备为完整、不可变的 `FigurePresentation`；它不把采样值写回 Figure 或
capability model。

opacity 与 node-local transform 是所有 Figure 共有的结构通道，继续由 Runtime 直接
提供，不要求每个 Figure 重复登记。route、dash 或外部 Figure 自定义视觉属于领域
内容投影，通过 binding 登记。属于同一 typed presentation family 的多个 channel
共享一次 prepare，例如 Connection route 与 dash；不同 family 不能同时替换同一
Figure content，会按 interruption policy 仲裁。opacity/transform 始终可与 content
family 正交组合。

## 5. 安全 Rust 注册形状

Rust 无法在不使用 unsafe 或闭集枚举的情况下，把任意借用的 `&dyn Trait` 放入通用
TypeMap。为避免自引用对象和公开 downcast，本项目采用 **owned descriptor registry**。

目标形状：

```rust,ignore
pub trait Figure: AsAny {
    fn initial_bounds(&self) -> Rectangle;
    fn name(&self) -> &'static str;

    fn register_capabilities(
        &self,
        out: &mut FigureCapabilityBuilder,
    ) -> Result<(), FigureCapabilityRegistrationError> {
        Ok(())
    }
}

pub struct CapabilityKey<C> {
    // opaque TypeId + diagnostic name
}

pub struct FigureCapabilityBuilder {
    // one owned descriptor per CapabilityKey
}
```

Animation descriptor 使用相同 registry：

```rust,ignore
const LEVEL_PRESENTATION: CapabilityKey<PresentationBinding<f64>> =
    CapabilityKey::new("example.level-presentation");

out.register(
    LEVEL_PRESENTATION,
    PresentationBinding::of::<MyFigure, MyPresentationFamily>(
        MyFigure::committed_level,
    ),
)?;

let channel = runtime.animations().bind_presentation(
    figure,
    LEVEL_PRESENTATION,
    InteractionGeometryPolicy::Committed,
)?;
```

`MyPresentationFamily` 实现 `PresentationFamily<MyFigure>`；同 family 的所有 channel
由类型系统保证共享该 prepare。具体类型适配只存在于 descriptor 内。family prepare
通过 `PresentationValues` 读取同 family 的 sampled/committed typed values并构造一次
immutable presentation；
渲染器不匹配外部 Figure 类型或 capability key。不同 family 不依赖绘制顺序叠加。

Figure 挂载前调用 `register_capabilities`。该方法只登记 owned descriptor，不把 `&self`
或字段引用保存到 registry。每个 descriptor：

- 是 `'static` 的普通值；
- 保存函数指针或无捕获 typed adapter；
- 接受 `&dyn Figure` / `&mut dyn Figure` 后在引擎内部完成受检 downcast；
- 不向调用者暴露 downcast；
- 不持有 Runtime、FigureTree、UpdateManager 或平台对象。

`FigureNode` 在 admission 成功时持有冻结的 `FigureCapabilitySet`。同一 key 重复登记、
descriptor 类型不符或 admission 校验失败时，不发布 FigureId。

## 6. 分层与 Capability 分类

### 6.1 Concrete Figure

具体 Figure 定义自身数据、构造不变量、绘制、测量和精确命中。类型专用属性通过
`FigureComponentUpdate<T>` 或专用 facade 修改。不得仅为消除内部 downcast 或减少几行
重复代码而创建公开 behavior trait。

### 6.2 Pipeline behavior

参与固定阶段的行为，例如 Input、Lifecycle、Container projection、Accessibility。
对应 descriptor 提供领域方法，但不能改变 scheduler 顺序。

### 6.3 Shared model

提供只读 snapshot 或 handle，例如 Scale 和 Clickable。状态仍由 Figure 或
共享 model 拥有，descriptor 只负责发现和适配。

### 6.4 Marker

Layer、Freeform 等无操作分类使用零状态 descriptor。Marker 只影响明确读取该 marker 的
算法，不触发隐式行为。

### 6.5 Provider

向 Connection、Animation、Text 或其他服务提供 typed 数据和 prepared adapter。
Provider 必须声明输入、输出、失败、生命周期和 visual envelope，不得返回 backend 类型。

### 6.6 Trait 引入判据

Behavior trait 只有同时满足以下条件才成立：

1. 至少存在两个语义可替换实现，或已有明确的第三方实现需求；
2. 存在独立于具体类型的消费者；
3. 输入、输出、错误、生命周期和 invalidation 合同一致；
4. 消费者不应知道 concrete type；
5. concrete model、泛型函数或 typed update 不能更简单地表达问题。

## 7. 标准 Capability 与具体类型

首批标准 key：

| Key | 类型 | 标准 descriptor |
|---|---|---|
| `PREPARATION` | Pipeline | `PreparationCapability` |
| `INPUT` | Pipeline | `InputCapability` |
| `LIFECYCLE` | Pipeline | `LifecycleCapability` |
| `ACCESSIBILITY` | Pipeline/Provider | `AccessibilityCapability` |
| `CONTAINER` | Pipeline | `ContainerCapability` |
| `LAYER` | Marker | `LayerCapability` |
| `FREEFORM` | Marker | `FreeformCapability` |
| `SCALE` | Model | `ScaleCapability` |
| `CONNECTION` | Behavior/Provider | `ConnectionCapability` |
| `CONNECTION_DECORATION` | Behavior | `ConnectionDecorationCapability` |
| `BORDER` | Model/Provider | `BorderCapability` |
| `CLICKABLE` | Model/Behavior | `ClickableCapability` |

以下项目明确不是标准 capability：

| 项目 | 归属 | 原因 |
|---|---|---|
| `PointList` | 共享具体模型 | Polyline/Polygon 复用数据结构不构成动态多态 |
| `ScalablePolygonFigure` | Concrete Figure | 只有一个具体实现和专用 facade |
| `TextFlowFigure` | Concrete Figure | 排版、交互几何和 mutation 属于该类型 |
| Label 内容 | Concrete subcomponent | Label/Button/Toggle 的闭合组合由专用 adapter 处理 |
| `ViewportFigure` | Concrete Figure + handle | range/origin 由 Viewport 专用合同拥有 |
| `ImageFigure` | Concrete Figure | 资源与 alignment 是类型专用状态 |
| RoundedRectangle corner / Triangle direction | Concrete Figure | 只有具体类型消费 |

若未来出现外部顶点编辑器需要统一消费多个可替换图形，应新建表达消费者语义的
`VertexGeometryCapability`，而不是把内部存储类型 `PointList` 直接升级为 capability。

## 8. 查询协议

公共查询使用 typed key：

```rust,ignore
let snapshot = tree.capability_snapshot(figure, SCALE)?;
let custom = tree.capability_snapshot(figure, MY_CAPABILITY)?;
```

规则：

- foreign/stale/unknown Figure 返回 `CapabilityQueryError`；
- Figure 存在但未登记能力返回 `Ok(None)`；
- descriptor 失败返回结构化领域错误；
- `Option` 只表达合法缺失，不表达身份错误；
- 普通消费者不能取得 registry 的 `&mut` 引用；
- query 不触发 validation、mutation、damage 或 notification。

Core 内部阶段可以借用标准 descriptor 执行同步行为，但借用不能逃出当前调用栈。

## 9. Mutation 协议

Capability mutation 与 `FigureComponentUpdate` 共享以下提交骨架：

```text
validate Runtime/Figure/capability identity
-> capture old visual envelope and revision
-> descriptor.prepare(read-only old state, owned command, context)
-> validate prepared value and invalidation
-> descriptor.commit(&mut dyn Figure, prepared value)
-> increment revision
-> invalidate layout/geometry/paint
-> invalidate dependent Connection/Accessibility/Presentation work
-> enqueue damage and typed committed facts
```

公开目标形状：

```rust,ignore
    .capability(figure, SCALE)?
    .apply(SetScale::new(2.0))?;
    .apply(SetScale::new(2.0))?;
```

`ScaleMut` 等 capability facade 是统一 capability update 的薄封装。`LabelMut`、
`PointListMut` 等具体类型/闭合集合 facade 走 typed component update 或 crate-private
concrete adapter。两类更新共享 identity、revision、damage 和 notification 提交原语，
但不要求类型专用状态登记到 registry。

外部 capability 的 commit 必须是无失败赋值；prepare 完成所有可预见校验。commit panic
沿 ADR-014 使 Runtime faulted，不承诺回滚扩展内部副作用。

## 10. 固定 Pipeline

递归主流程继续由 Runtime/renderer 独占：

```text
resolve NodeState/style/presentation
-> paint prepared or core Figure drawing
-> apply child transform and clip
-> recurse children
-> paint border/decoration
```

Capability 只能提供阶段输入。禁止：

- 替换或递归调用主 renderer；
- 重排 self/children/border 顺序；
- 在 paint 中修改 topology、layout 或 source state；
- 在 hit-test/input 中同步重入 Runtime mutation；
- 通过 unknown custom key 自动执行代码。

Connection route/dash 已登记为标准 `PresentationBinding<PointList/f64>`。递归主循环
只读取统一的 optional content presentation，不再识别 Connection、route 或 dash；
self/children/border 顺序与 10,000 层递归门禁保持不变。

## 11. 生命周期

### Admission

```text
validate initial Figure state
-> build capability descriptors
-> reject duplicate/invalid descriptors
-> allocate FigureId and FigureNode
-> publish topology
-> invoke Lifecycle capability
```

失败不发布 topology、notification、damage 或可复用 ID。

### Disposal

Lifecycle 按既有 descendant-first 顺序执行。Registry 与 Figure 同一节点退役；
descriptor 不得保活 Runtime identity。共享资源和 Runtime-scoped provider 继续由各自
registry 管理。

### Reparent

同 Runtime reparent 保持 FigureId、component revision 和 capability registry。
跨 Runtime 仍通过模型/描述重建，不迁移 live descriptor。

## 12. 错误边界

至少区分：

- foreign、stale、disposed Figure；
- capability absent；
- duplicate descriptor；
- descriptor/command 类型不匹配；
- prepare rejected；
- revision exhausted；
- invalidation/envelope invalid；
- extension panic / Runtime faulted。

错误不得退化为字符串 capability 名、`false` 或静默 downcast miss。

## 13. 扩展点

外部 Figure 可以：

1. 实现最小 Figure 核心协议；
2. 登记标准 capability 的 typed adapter；
3. 定义自有 `CapabilityKey<C>` 和 descriptor；
4. 通过 `FigureComponentUpdate` 修改私有状态；
5. 通过 `CapabilityUpdate` 修改需被其他服务发现的领域状态；
6. 为 Animation 提供 `PresentationBinding<V>`。

外部能力只有在消费者显式持有其 key 时才会被调用。Core 不维护第三方 key 列表。

## 14. 已完成迁移

1. 建立 registry、query、错误和外部 consumer，不改变现有行为；
2. 迁移 Scale、Layer、Freeform 等 Model/Marker；
3. 迁移 Container、Input、Lifecycle、Accessibility；
4. 将 PointList、ScalablePolygon、TextFlow、Label、Viewport、Image 收口为 concrete
   model/type + typed update，删除伪 behavior trait；
5. 迁移 Border、Clickable、Connection、Decoration 等真实跨类型 capability；
6. Runtime capability facade 改用 key；类型专用 facade 改用 concrete adapter；
7. 删除 `Figure` 上领域 accessor；只禁止用 concrete downcast 判断横切 capability；
8. Animation 使用开放 `PresentationBinding`，Connection route/dash 删除中心化分派；
9. 旧伪 behavior trait 已退出 root，capability 与 animation descriptor 不进入 prelude；
   完整 root/prelude allowlist 与 compile-fail snapshot 归 TC-13，不在 P2-F02 重复建设。

每步保持旧/new 行为等价；迁移一个 key 时，其实现、消费点、外部测试和旧 accessor 必须
在同一原子批次闭合，不保留长期双入口。

## 15. 验证

P2-F02 至少覆盖：

1. 外部 Figure 登记标准 Scale capability，无 Core 类型分支即可 zoom；
2. 外部 Figure 登记自定义 capability，外部消费者可 typed query/update；
3. duplicate、foreign、stale、absent 和 prepare rejection 有结构化结果；
4. callback deferred update 与同步 facade 使用同一提交原语；
5. Layer/Freeform/Container/Input/Lifecycle 行为迁移前后等价；
6. Connection/Widget 横切消费者不再依赖基础 Figure accessor；
7. PointList/TextFlow/Image/Viewport 等类型专用 API 不再伪装成 behavior capability；
8. Core 不通过 concrete downcast 判断横切 capability；专用 facade 的受控 concrete
   adapter 有明确闭集和错误语义；
9. 新增 capability 不修改 `Figure` trait；
10. render traversal、damage、notification 和 10,000 层门禁不变；
11. 无 capability query/update 时不增加逐帧全树扫描或持续工作。
12. 外部 `PresentationBinding<V>` 不修改 `ChannelBinding` 或 renderer 分支即可完成
    typed sample、damage 和 frame recording；同 Figure content owner 冲突结构化拒绝。

验证入口登记在 `verification/suites.toml`，完成证据进入独立 verification record。
完成证据见
[P2-F02 Figure Capability 实现记录](../../verification/reviews/p2-f02-figure-capability-completion-2026-10-08.md)。

## 16. 关系

- 接受并细化 ADR-014 的外部组件扩展边界；
- 保持 ADR-019 的 Runtime scoped mutable facade；
- 保持 ADR-023 的 Core package 与扩展边界；
- 复用 ADR-025 的 prepare/presentation 结果；
- 为 ADR-026 的外部 Animation target 提供发现基础；
- 关闭
  [临时概念设计审计 TC-02](../../verification/reviews/temporary-concept-design-audit-2026-10-08.md#32-tc-02capability-仍由-core-列举)。
