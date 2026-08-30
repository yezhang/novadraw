# Novadraw 理想架构

类型：`normative-design`

本文定义 Novadraw 的长期架构原则、核心概念和稳定边界，是总体架构的
SSOT。静态所有权见 [static-architecture.md](static-architecture.md)，运行时事务见
[dynamic-architecture.md](dynamic-architecture.md)。坐标、输入和更新的细节由对应
专题设计定义。

本文只描述目标设计，不描述当前实现、迁移进度或里程碑状态。

## 1. 目标

Novadraw 是面向图形编辑器和交互式二维场景的跨平台 Rust 引擎。架构必须同时支持：

- Web、macOS、Windows 和 Linux；
- 可替换的平台事件循环、窗口表面和渲染后端；
- 大规模轻量 Figure 树及增量更新；
- 可组合的布局、坐标、裁剪、滚动、缩放和输入协议；
- headless 测试、确定性回放和可观测的更新事务；
- 在不污染二维核心的前提下扩展 2.5D 视觉效果或嵌入真正的 3D 场景。

## 2. 从 Draw2D 继承什么

Novadraw 继承 Draw2D 的行为语义和概念关系，不复制 Java 的对象布局。

| Draw2D 核心语义 | Novadraw 保留方式 |
|---|---|
| Figure 是轻量图形单元 | arena 中的 `FigureNode` + 可扩展 `Figure` 行为 |
| Figure 树决定所有权和 Z-order | `FigureTree` 统一维护拓扑和有序 children |
| bounds 是几何协议入口 | `NodeState.bounds` 是布局、命中和 damage 的共同真源 |
| client area 由 bounds 和 insets 推导 | 统一盒模型供布局、绘制和命中下降使用 |
| 父子坐标转换是双向协议 | 每条树边共享同一可逆 2D 变换 |
| paint 使用受控模板 | Runtime 固定执行 self、children、border 的顺序和状态隔离 |
| LayoutManager 是容器策略 | 每个容器拥有独立 `LayoutState` |
| Validation 先于 Damage Repair | `UpdateManager` 执行不可颠倒的两阶段事务 |
| 输入由状态机选择单一 target | `EventDispatcher` + `InteractionState` |
| capture、focus、hover 是持续交互状态 | 独立于树存储，但通过 `FigureId` 引用节点 |
| LightweightSystem 是宿主桥 | `Runtime` 与 `PlatformHost` 明确分工 |

以下 Java 结构不作为 Novadraw 契约：

- 宽 `IFigure` 接口；
- parent/child 对象引用；
- Figure 向上查找 manager 或平台对象；
- 通过继承复用大部分默认行为；
- SWT 类型、UI 线程工具和 listener 对象身份；
- 为所有抽象预先使用动态分发或线程安全包装。

## 3. 北极星原则

> **Figure 定义图形行为，FigureNode 保存通用节点状态，FigureTree 保存拓扑，
> Runtime 原子协调交互、更新与变更事务，PlatformHost 和 RenderBackend 分别隔离
> 平台循环与渲染实现。**

这里的“Figure 语义内聚”指一个 Figure 对外仍表现为完整的轻量图形节点，不表示其
全部数据和协议必须塞入一个 Rust trait 或结构体。

### 3.1 Figure

`Figure` 表达因具体图形类型而异的行为：

- 绘制自身；
- 计算内在尺寸；
- 精确命中形状；
- 暴露可选输入或生命周期能力；
- 保存点集、圆角、文本、路径等图形专属数据。

Figure 不保存 parent、children、全局服务、平台对象或通用节点几何。

### 3.2 FigureNode

`FigureNode` 是 arena 中的运行时节点，组合：

```text
FigureNode
├── NodeState
├── LayoutState
└── Box<dyn Figure>
```

- `NodeState` 保存所有 Figure 都需要的几何、可见性、启用状态、样式覆盖和验证状态；
- `LayoutState` 保存容器布局器、child constraints 和布局缓存；
- `Figure` 保存具体图形的内在数据和可替换行为。

这种拆分使公共机制能连续访问紧凑状态，同时保留异构 Figure 的扩展能力。

### 3.3 FigureTree

`FigureTree` 只负责：

- arena 存储和代际 `FigureId`；
- parent、children 和顺序；
- attach、detach、reparent 的拓扑不变量；
- 基于树关系的祖先、后代和 Z-order 查询。

命中测试和坐标查询可以作为依赖 `FigureTree` 的领域算法存在，但交互 session
不属于树本身。

### 3.4 Runtime

`Runtime` 是单个场景实例的组合根和事务边界：

```text
Runtime
├── FigureTree
├── InteractionState
├── EventDispatcher
├── UpdateManager
└── MutationQueue
```

Runtime 对外提供命名操作，不暴露能够绕过事务约束的多个可变引用。它不是全局
singleton，也不承担应用业务逻辑。

## 4. 所有权与线程模型

### 4.1 单线程核心

Figure 树、交互状态、事件分发和更新事务默认由一个 UI/runtime 线程独占：

- 核心接口不默认要求 `Send + Sync`；
- 容器内部不默认使用 `Arc<Mutex<_>>`；
- 运行时通过短生命周期借用和 effect queue 解决可变访问；
- 多个 Runtime 实例彼此独立。

字体解析、图像解码、网络和 GPU 工作可以跨线程执行，但必须通过消息或资源句柄把
结果提交回 Runtime 边界。线程安全是边界策略，不是所有领域对象的基础属性。

### 4.2 ID 引用

树和持续状态使用代际 `FigureId`，避免自引用结构和悬空引用。可持久化 UUID、业务
对象 ID 或 accessibility ID 是可选外部身份，不强制占用每个运行时节点。

## 5. 几何与坐标

### 5.1 二维核心

Novadraw 的布局和 Figure 几何采用明确的二维语义：

- `bounds` 位于 parent content domain；
- Figure 自身在以 border-box 左上角为原点的 local domain 绘制；
- parent/child 之间使用统一 `Affine2D`；
- paint、hit-test、事件点、clip 和 damage 必须复用同一变换链。

这保留 Draw2D 的双向坐标转换能力，但不复制“移动普通父节点时改写全部后代
bounds”的存储策略。

### 5.2 2.5D 与真正 3D

二维核心不禁止 4x4 矩阵，而是不把 3D 表示强加给所有布局和 Figure API：

```text
FigureTree (2D layout and interaction)
        │
        ▼
VisualComposition
├── Affine2D
└── Projective3D
        │
        ▼
RenderBackend
```

- 卡片翻转、透视旋转等 2.5D 效果位于视觉合成层；
- 投影后的 quad/AABB 参与绘制、damage 和可选的逆投影命中；
- 真正的 3D 使用独立 `Scene3D`，拥有 camera、depth、ray hit-test 和 3D bounds；
- `Scene3D` 可通过专用 Figure、layer 或 texture 嵌入二维树。

因此未来扩展 3D 不依赖“把 `Point` 类型别名改成 `Vec3`”，也不要求重写二维布局。

## 6. Figure 能力模型

Rust API 应使用小而稳定的能力边界，而不是复刻宽 `IFigure`：

```text
Figure
├── paint
├── intrinsic_size
├── hit_shape
└── optional capabilities
    ├── FigureEventHandler
    ├── FigureLifecycle
    └── AccessibleFigure
```

通用 bounds、insets、可见性、enabled、style inheritance 和 validation 位于节点
状态及 Runtime 协议中。Shape 是可复用绘制辅助抽象，不应通过 blanket impl 阻止
具体类型定制 Figure 行为。

## 7. 布局

每个容器的 `LayoutState` 拥有：

- `Box<dyn LayoutManager>`；
- child 到 layout-specific constraint 的映射；
- preferred/minimum size 缓存及其 generation；
- 布局失效状态。

Runtime 为 LayoutManager 构造不可变 `LayoutSnapshot`，布局器通过 `LayoutOutput`
提交 bounds 结果，不长期持有或重入访问 FigureTree。约束属于“父容器与 child 的
关系”，删除或 reparent 时必须原子清理。

尺寸解析顺序保持 Draw2D 语义：

```text
explicit size override
→ LayoutManager measurement
→ Figure intrinsic size / current size fallback
```

缓存与显式 override 必须是两个概念，不能共用同一个字段。

## 8. 输入与交互

平台适配器只负责将原生事件规范化为平台无关输入。`EventDispatcher` 负责：

- 根据 `InteractionState` 和 hit-test 选择 target；
- 维护 hover、capture、focus 和 gesture session；
- 将事件点转换到 target local domain；
- 对单一 target 执行一次回调；
- 在协议明确要求时执行 typed fallback。

`InteractionState` 独立于 FigureTree，至少包含 pointer、focus、hover 和 gesture
session。节点删除后，Runtime 统一清理所有指向失效 `FigureId` 的状态。

普通输入不采用 DOM 式通用冒泡。Scroll/Zoom 查找祖先容器属于显式 typed
fallback，不改变普通事件回调次数。

## 9. 变更事务

Figure 回调不能持有 Runtime、FigureTree 或 UpdateManager 的长期可变引用。回调接收
只读视图和只记录操作的 `EventContext`：

```text
callback
→ append Effect
→ release Figure borrow
→ apply local effects in causal order
→ apply structural mutations at transaction boundary
```

- 几何、状态、repaint、invalidate 是节点 effect；
- add、remove、reparent、layout replacement 是结构 mutation；
- effect 保持产生顺序；
- 结构 mutation 只能在顶层 dispatch/update 遍历之外提交；
- 每次提交同时维护 topology、interaction、layout、validation 和 damage 不变量。

## 10. 更新与渲染

更新事务固定为：

```text
Apply pending mutations
→ Validation and Layout
→ Damage Repair
→ Record paint commands
→ Build RenderSubmission
→ RenderBackend.submit
```

`UpdateManager` 负责 invalidation、damage 和阶段时序，不负责平台 redraw 调度。
`PlatformHost` 负责请求下一帧和 surface 生命周期。

Damage 的规范语义是：

> 提交后，damage 外的可见像素必须与提交前等价。

后端可以采用局部裁剪、保留纹理、tile cache 或全帧重绘实现该结果，不要求所有后端
都字面执行 union clip。

## 11. 平台与渲染边界

### PlatformHost

真实平台替换边界包括：

- redraw scheduling；
- logical/physical size 和 scale factor；
- surface 创建、resize、suspend 和 resume；
- clipboard、cursor、IME 和 accessibility bridge；
- 将原生输入交给对应 adapter。

Winit 可以承载 macOS、Windows 和 Linux，Web 使用独立 adapter。平台类型不得进入
Figure、布局、事件和更新协议。

### RenderBackend

`RenderBackend` 接收稳定的 `RenderSubmission` 和目标 surface 信息。它可以由 Vello、
Skia、软件渲染器或测试后端实现。

后端替换契约关注绘制语义、资源句柄、surface 生命周期和提交结果，不把某个 GPU
API 的对象暴露给 FigureTree。

## 12. Root Figure

Root Figure 是树内虚拟根和继承属性根，不是平台资源桥：

- 提供默认背景、字体和主题值的已解析快照；
- 提供根 content domain 和根 clip；
- 不持有窗口、host、renderer、dispatcher 或 update manager；
- logical viewport 变化由 Runtime 以普通状态变更注入。

平台资源的读取和生命周期始终属于 `PlatformHost`。

## 13. 稳定扩展点

优先保持稳定的动态扩展边界：

- `Figure`
- `LayoutManager`
- `RenderBackend`
- `PlatformHost`
- resource resolver
- 明确需要替换的 hit-test、clip 或 scroll policy

FigureTree、InteractionState、默认 EventDispatcher 和默认 UpdateManager 优先采用具体
类型。只有存在多个真实实现且调用方确实需要替换时，才提升为 trait。

## 14. DisplayList 定位

核心架构只要求存在从绘制录制到后端提交的稳定边界：

```text
Figure paint
→ RecordingCanvas
→ RenderSubmission
→ RenderBackend
```

二进制 ABI、零拷贝映射、chunk patch、远程传输和跨语言读取属于候选
DisplayList proposal。它们必须通过真实用例、benchmark、资源生命周期和版本兼容性
验证后，再由 ADR 提升为规范。

## 15. 不可违反的不变量

1. Figure 树顺序同时决定绘制顺序和默认命中优先级。
2. bounds、client area、坐标转换、hit-test 和 damage 使用同一几何来源。
3. Validation 必须先于本轮 Damage Repair。
4. 事件回调期间不能直接改变正在遍历的树结构。
5. `InteractionState` 中的 ID 必须在节点删除后清理。
6. disabled 影响输入资格，不等同于 invisible，也不阻止布局验证。
7. Runtime 核心不依赖平台窗口或具体渲染后端。
8. 平台和异步任务不能从其他线程直接修改 FigureTree。
9. 2.5D 合成不能静默改变二维布局语义。
10. 真正 3D 场景不能伪装成替换二维 Point 类型。

## 16. 文档分工

| 文档 | 唯一职责 |
|---|---|
| 本文 | 原则、概念边界和长期不变量 |
| [static-architecture.md](static-architecture.md) | 类型、所有权和依赖方向 |
| [dynamic-architecture.md](dynamic-architecture.md) | 输入、变更、更新和渲染事务 |
| [coordinate-system.md](../coordinates/coordinate-system.md) | 坐标域、变换、投影和 damage 映射 |
| [scroll-zoom-gesture-contract.md](../input/scroll-zoom-gesture-contract.md) | Scroll/Zoom 输入契约 |
| [update-manager.md](../rendering/update-manager.md) | Validation、Damage 和提交协议 |
| [display-list-protocol.md](../rendering/display-list-protocol.md) | 非规范 DisplayList 提案 |
| [directory-structure.md](directory-structure.md) | 逻辑边界到目录/crate 的映射原则 |

Draw2D 源码事实以 `doc/reference/` 为准；采用、调整或拒绝关系以 `doc/parity/` 为准。
