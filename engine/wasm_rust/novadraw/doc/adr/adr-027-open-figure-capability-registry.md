# ADR-027: 开放的 Figure Capability Registry

类型：`architecture-decision`

状态：`accepted`，P2-F02 已实现

日期：2026-10-08

## 背景

Draw2D 通过 Java 继承和虚方法让子类覆盖绘制、测量、命中、输入、坐标投影和生命周期
行为。ADR 接受前，Novadraw 把其中一部分迁移为 `Figure` 上的可选 accessor：

- `event_handler`、`lifecycle`、`accessible`、`container`；
- `layer`、`freeform`、`scale_model`；
- `connection`、`point_list`、`text_flow`、`label`、`clickable` 等领域能力。

其中前两组和 Connection/Clickable 等确有独立跨类型消费者；PointList、TextFlow、
Label 等则主要表达具体类型的数据和专用 facade。旧设计把两者都放进 `Figure` accessor，
同时造成“横切能力集合闭合”和“具体类型被伪装成 behavior capability”两个问题。

Rust 的安全 TypeMap 不能直接保存任意带借用生命周期的 `&dyn Trait`。若 registry
保存 Figure 字段引用，会形成自引用对象；若公开 `Any` downcast，则把类型安全和错误
边界转移给每个消费者。

## 决策

### 1. Runtime 保持固定调度权

Figure capability 不能覆盖渲染遍历、validation、damage、notification 或 frame
publication 主流程。Runtime 按固定阶段查询标准 capability，未知能力只有显式消费者
持有其 typed key 时才执行。

### 2. Figure 在 attach 时登记 owned descriptor

`Figure` 提供 `register_capabilities(&mut FigureCapabilityBuilder)`。登记值是 `'static`
owned descriptor，不保存 Figure 或 Runtime 借用。

Descriptor 可以保存函数指针或无捕获 typed adapter；执行时由引擎把 `&dyn Figure` /
`&mut dyn Figure` 交给 descriptor，descriptor 在内部完成受检 concrete type 适配。
调用方不接触 `Any`、`TypeId` 或 downcast。

`FigureNode` 与 Figure 同生命周期持有冻结的 capability set。同一 key 重复登记在
topology 发布前拒绝。

### 3. Capability 身份和操作保持 typed

`CapabilityKey<C>` 提供 opaque identity 和诊断名称。`C` 是具体 descriptor 类型，
定义自己的领域操作：

- Pipeline behavior；
- shared model；
- marker；
- provider；
- 可选 prepared mutation。

统一的是 registration、discovery、identity、lifecycle 和 Runtime commit，不要求不同
领域共享一个万能方法集合。

### 4. Capability 以独立跨类型消费者为门槛

Figure 拥有独特字段或方法，不足以构成 capability。只有独立消费者需要在不知道
concrete type 的情况下按统一合同发现多个可替换实现时，才登记 capability。

- Input、Lifecycle、Accessibility、Container、Scale、Layer/Freeform、
  Connection、Border、Clickable 属于该边界；
- PointList、ScalablePolygon、TextFlow、Label、Viewport、Image 默认属于具体类型、
  共享具体模型或 concrete subcomponent；
- 后者通过 inherent behavior、`FigureComponentUpdate<T>` 和专用 facade 暴露；
- 专用 facade 可以在 crate 内对明确闭合的内建类型做受控 concrete adapter，
  但不得把该 adapter 宣称为外部可扩展 capability。

Behavior trait 只在已有多个可替换实现、独立消费者和一致失败/生命周期合同后引入，
不作为代码复用或消除 downcast 的默认工具。

### 5. 私有组件与横切能力分离

只有具体 Figure 自身消费的私有状态继续使用 `FigureComponentUpdate`。需要被其他引擎
模块发现的状态或行为才进入 capability registry。

两类更新共享 Runtime 的 prepared commit、revision、invalidation、damage 和 notification
原语，但不合并成 untyped mutation closure。

### 6. 不使用全局注册和 unsafe trait-object cast

Capability registry 归每个 FigureNode 所有，不使用 Singleton、inventory 或进程全局
类型目录。首版以 owned descriptor 和安全 concrete downcast adapter 实现，不引入裸
fat pointer 转换。

### 7. 直接迁移，不保留长期双入口

项目仍为 0.1。每个标准 capability 的实现、消费点、外部测试和旧 `Figure` accessor
在同一切片迁移。具体 Figure 的伪 behavior accessor 同样直接删除并替换为 typed
component update 或 concrete adapter。全部完成后，Core 不再通过 concrete downcast
判断横切 capability；类型专用 facade 的闭合适配不在此禁令内。

### 8. Animation content binding 使用同一 registry

`PresentationBinding<V>` 是 Animation 独立消费者使用的标准 descriptor：

- descriptor 读取 concrete Figure 的 committed value；
- sampled value 只生成 immutable `FigurePresentation`；
- Runtime 私有层负责类型擦除、owner、damage revision、dispose 与 cancel；
- renderer 只消费统一 content presentation，不匹配 Connection 或外部 Figure；
- opacity/transform 作为全 Figure 结构通道保留在 Runtime，不要求重复登记；
- route/dash 和外部 Figure 内容投影通过 registry 接入。

同一 typed presentation family 的多个 channel 可以正交组合并共享一次 prepare，
例如 Connection route 与 dash。不同 family 不能同时替换同一 Figure content，在
admission 阶段按 Replace/Ignore 仲裁；结果不依赖渲染顺序。

详细合同见
[Figure Capability 统一扩展模型](../design/architecture/figure-capability-model.md)。

## 后果

### 正面

- 新增第三方 capability 不修改 `Figure` trait 或 Core 中心枚举；
- Runtime 继续拥有唯一副作用和调度边界；
- 内置与外部能力使用同一 discovery/mutation 协议；
- 为 Animation 外部 presentation binding 提供稳定基础；
- 避免自引用 Figure、公开 downcast 和全局状态。

### 负面

- 每个 attached FigureNode 需要保存小型 descriptor registry；
- 标准 capability 需要定义 typed adapter 和领域错误；
- 当前大量 accessor、伪 behavior trait 和专用 facade acquisition 需要分类迁移；
- descriptor 分派比直接虚方法多一次 key lookup，需在完成后做工作量基线；
- capability 只能扩展已知消费者，不能自动插入任意 pipeline 阶段。

## 失败处理

- duplicate descriptor：admission 失败，不发布 Figure；
- foreign/stale Figure：返回 query/mutation identity error；
- capability absent：合法 `Ok(None)` 或 acquisition error；
- prepare rejection：不提升 revision，不产生 invalidation、damage 或 notification；
- commit panic：Runtime faulted，沿 ADR-014 处理；
- descriptor type mismatch：结构化内部一致性错误，不回退为 capability absent。

## 验证

- 外部标准 capability consumer；
- 外部自定义 capability + 自定义消费者；
- 同步/deferred prepared mutation 等价；
- duplicate/foreign/stale/absent/fault 测试；
- 横切 capability concrete downcast 扫描；
- PointList/TextFlow/Image/Viewport 等类型专用 API 的分类测试；
- render/layout/event/connection 回归；
- `cargo xtask check --quick`，最终完成边界运行 full。

实现和工作量证据见
[P2-F02 Figure Capability 实现记录](../verification/reviews/p2-f02-figure-capability-completion-2026-10-08.md)。

## 关系

- 细化 ADR-014、ADR-019 和 ADR-023；
- 保持 ADR-025 的统一 Graphics/Preparation；
- 为 ADR-026 的开放 presentation target 提供依赖；
- 替代将 `Figure` accessor 持续扩张为 capability 注册表的做法。
