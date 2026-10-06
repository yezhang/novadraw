# ADR-019: 可组装 API 与 Scoped Mutable Facade

类型：`architecture-decision`

## 状态

已接受，已验证

## 背景

ADR-017 与 ADR-018 已将构建期、查询期和运行期写入分开，并确立 Runtime 为挂载后
状态的唯一提交边界。但“唯一提交边界”被过度映射成“所有 mutation 都必须平铺为
`Runtime::*` 方法”，产生两类问题：

- detached Figure、LayoutManager、Border、Router 等普通 owned value 的直接构造与
  配置，容易被误解为也必须经过 Runtime；
- 随着节点状态、容器、内置 Figure 和扩展组件能力增加，Runtime 会持续膨胀为包含
  全部领域动词的巨型 facade。

Draw2D 允许直接构造和组合 Figure，但该便利依赖 Figure 自身拥有 parent、children，
并沿父链取得 UpdateManager。Novadraw 使用 arena、代际 ID 和独立运行时状态，不能照搬
这套对象所有权；仍可以通过短生命周期 facade 保留对象式调用体验。

## 决策

### 1. 提交权威与 API 命名空间分离

Runtime 继续是挂载后 source mutation、派生状态失效、通知和 damage 的唯一提交权威。
这不要求所有领域操作都直接定义为 Runtime 的平铺公共方法。

公开 API 按生命周期分为：

| 阶段 | 入口 | 职责 |
|---|---|---|
| detached | 具体类型构造器和 `with_*` | 构造 Figure 私有数据、布局和绘制策略 |
| build | `FigureTreeBuilder` | 分配 FigureId，组装拓扑和初始 NodeState |
| attached | Runtime scoped mutable facade | 提交 update-aware 运行期 mutation |
| query | `FigureTree` / stable snapshot | 只读查询，不触发隐式 mutation |
| drive | `Runtime` | 输入、稳定化、资源、帧和生命周期协调 |

### 2. Detached value 保持直接构造

具体 Figure、LayoutManager、Border、Router、Anchor 和其他策略对象可以在进入树前
直接构造、校验和自由组合。它们不拥有 FigureTree 拓扑、Runtime registry、
InteractionState 或 UpdateManager。

Figure 进入 FigureTree 后，NodeState、拓扑和运行时关系以引擎状态为准。调用方不能
通过保留的对象引用绕过 Runtime 修改已挂载状态。

`FigureTreeBuilder` 是 pre-Runtime 拓扑与通用节点状态的唯一写入口，不是所有 detached
value 的唯一构造入口。

### 3. 挂载后使用短生命周期可变 facade

Runtime 提供按能力分组的借用 facade：

```rust
runtime.figure(figure_id)?.set_bounds(bounds)?;
runtime.container(container_id)?.add(child)?;
runtime.container(container_id)?.set_layout_manager(layout)?;
runtime.figure(label_id)?.update_component(SetLabelText::new(text))?;
```

初始能力面至少包含：

- `FigureMut<'_>`：通用节点状态、size override、父子约束、reparent、
  repaint/revalidate 和 typed component update；
- `ContainerMut<'_>`：add/remove、LayoutManager、child order 和 child clipping；
- 现有 `LayeredPaneMut<'_>` 等容器专用 facade 继续采用相同借用模型；
- Viewport/Scale 的运行期修改迁移到借用 Runtime 的可变 facade，cloneable handle 仅保留
  identity 或只读 snapshot 能力。

scoped mutable facade：

- 内部只保存 `&mut Runtime` 与目标 handle；
- 不能超过 Runtime 可变借用生命周期；
- 不公开 `&mut FigureTree`、`&mut UpdateManager` 或其他可拆分运行时状态；
- 所有方法委托同一 Runtime mutation primitive，不复制校验、失效和通知逻辑；
- capability acquisition 必须验证 namespace、attached 状态和所需能力。

### 4. Runtime 保留组合根操作

以下操作仍直接属于 Runtime：

- `set_contents` 和 Runtime 整体生命周期；
- 平台无关输入 dispatch；
- frame preparation、submission acknowledgement 和稳定化；
- 资源注册与 backend session；
- 跨多个 Figure/registry 的 connection、focus、listener 等协调服务；
- scoped mutable facade 的获取。

Runtime 内部 mutation primitive 可以是 `pub(crate)`，但不得作为第二套公共写 API。

### 5. 不引入任意 update closure

不提供接收 `&mut Runtime`、`&mut FigureTree` 或 `&mut dyn Figure` 的任意 closure。
也不把多个 facade 调用包装成名为 transaction、但实际上不能整体回滚的 convenience。

当前原子性仍以单个 source operation 为单位，多个 operation 按调用顺序提交。需要
跨状态原子性时，增加 `reparent`、`replace_contents` 等命名复合操作；只有能完整
预验证和提交的受限 mutation batch 才可以声明整体原子。

### 6. 直接 breaking 迁移

项目版本仍为 `0.1.0`。本批次将被 scoped mutable facade 覆盖的 Runtime 平铺公共 mutator
收为 crate 内部，并迁移 workspace 调用方，不保留 deprecated 双入口。

## 错误处理

- scoped mutable facade 获取失败返回现有结构化 Runtime/capability error；
- facade mutation 保持原方法的 `Result` 和幂等 `Ok(false)` 语义；
- 获取 facade 不修改状态；
- facade 方法失败不得修改树、派生 worklist、通知或 damage；
- detached 构造失败使用对应值类型错误，不转换为 RuntimeMutationError。

## 扩展点与稳定性

- 第三方 Figure 继续使用 `FigureComponentUpdate`，但从目标 Figure 可变 facade 提交；
- 新容器能力优先增加专用 scoped mutable facade，不向 Runtime 平铺同类方法；
- facade 只组织调用体验，不拥有第二份状态，也不改变 trait 扩展边界；
- 只读 identity handle 与可变 facade 在命名和生命周期上必须可区分。

## 验证

- detached Figure/Layout/Border 可以不创建 Runtime 而直接构造；
- Builder 与 scoped mutable facade 构造等价场景时得到相同拓扑和节点状态；
- facade 对 foreign、disposed、detached 或 capability 不匹配目标返回结构化错误；
- facade mutation 产生与原 Runtime 路径相同的 validation、damage 和 notification；
- 外部测试 crate 不能调用被收口的 Runtime 平铺 mutator；
- 不存在接受 `&mut FigureTree + &mut UpdateManager` 的公开 attached mutation；
- `cargo xtask docs`、`cargo xtask check --quick`；
- 最终交付执行一次 `cargo xtask check --full`。

## 关系

- 澄清 [ADR-017](adr-017-core-public-api-boundary.md) 的构建期与运行期 API 表面；
- 保持 [ADR-018](adr-018-runtime-driving-and-measurement-api.md) 的 Runtime 帧边界；
- 保持 [ADR-014](adr-014-extensibility-and-lifecycle-boundaries.md) 的 prepared component
  update 与 fault 边界；
- 不改变 [ADR-009](adr-009-runtime-dynamic-mutation-contract.md) 的逐操作原子性和 FIFO
  callback mutation；
- 对应 `api_semantics`：
  `figure.tree`、`figure.geometry.bounds`、`layout.manager`、
  `validation.protocol`、`damage.repaint`。

## 日期

2026-09-28
