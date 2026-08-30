# 理想目录结构设计

类型：`normative-design`

本文档记录“理想架构如何映射到目录结构”的正式决策，用于指导后续代码重组。目标不是一次性把 workspace 拆成最终形态，而是先让目录表达清晰的职责边界，再逐步把稳定子域提升为独立 crate。

## 当前目录结构的主要问题

当前 `novadraw-scene` 仍同时承载了多类不同性质的内容：

- 图形能力层：`figure`、`border`、`layout`
- 图关系层：`scene`
- 运行时服务层：`event`、`update`、`mutation`、`context`、`system`
- 平台宿主与扩展容器层：`scene_host`、`viewport`

这种组织方式在项目早期是高效的，但随着理想架构逐渐定稿，会产生两个长期问题：

1. **边界只存在于文档，不存在于目录**
   - 实现者需要靠记忆区分“这是核心域还是运行时服务”
   - 一旦实现压力变大，逻辑容易重新混回 `novadraw-scene` 根层
2. **未来拆 crate 缺乏自然迁移路径**
   - 如果目录本身没有先按职责分层，后续再拆 crate 时会同时引入“文件移动 + 依赖重组 + 语义澄清”三类成本

## 目录优化的核心决策

### 决策 1：先做模块边界优化，再做 crate 边界优化

这是本文最重要的目录决策。

原因：

- 当前最需要稳定的是**职责边界**，不是 workspace 数量
- 如果现在立即大拆 crate，会在接口尚未完全落地前引入大量循环依赖与 facade 调整
- 先在 `novadraw-scene` 内部把目录层次整理清楚，可以让后续 crate 拆分变成“机械迁移”，而不是再做一轮架构设计

结论：

- **短期目标**：让 `novadraw-scene/src` 的目录结构表达理想架构
- **中期目标**：当边界稳定后，再把稳定子域升级为独立 crate

### 决策 2：按“核心域 / 运行时服务 / 平台宿主 / 扩展容器”四层整理

按理想架构，目录应该优先表达以下四种不同性质的对象：

1. **图形与布局能力层**
   - `Figure`
   - `Border`
   - `LayoutManager`
2. **场景图核心层**
   - `FigureBlock`
   - `FigureGraph`
   - hit-test / 坐标转换 / mutation apply
3. **运行时服务层**
   - `EventDispatcher`
   - `UpdateManager`
   - `NovadrawContext`
   - `MutationContext`
   - `NovadrawSystem`
4. **平台宿主与扩展容器层**
   - `SceneHost`
   - `Viewport`
   - 后续 `Layer`
   - 后续 `Scroll`

这么分的原因是：

- 这四层正好对应理想架构中的四类对象
- 它们的依赖方向天然不同
- 后续即使扩展功能，也可以明确知道新增代码应该落在哪一层，而不是继续堆到根目录

## 当前短期目录结构

在不立刻拆 crate 的前提下，`novadraw-scene/src` 已通过 AD-019A / AD-019B 调整为如下结构：

```text
novadraw-scene/src/
  lib.rs

  figure/
    mod.rs
    traits.rs
    shape.rs
    root.rs
    border/
      mod.rs
      line.rs
      margin.rs
      rectangle.rs

  layout/
    mod.rs
    traits.rs
    xy.rs
    flow.rs
    border.rs

  graph/
    mod.rs
    block.rs
    graph.rs
    hit_test.rs
    coords.rs
    render.rs

  runtime/
    mod.rs
    context.rs
    event/
      mod.rs
      dispatcher.rs
      types.rs
      dispatch_context.rs
    update/
      mod.rs
      manager.rs
      deferred.rs
      repair.rs
      listener.rs
    mutation/
      mod.rs
    system/
      mod.rs

  host/
    mod.rs
    scene_host.rs

  container/
    mod.rs
    viewport.rs
    layer.rs
    scroll.rs
```

## 关键命名调整

### 决策 3：`scene/` 更名为 `graph/`

原因：

- 当前理想架构的真正核心对象是 `FigureGraph`
- `scene` 这个词过于宽泛，容易把运行时服务、平台宿主、视口等都理解成“scene 的一部分”
- `graph` 更准确表达“树关系 + 节点状态 + 命中测试 + 坐标转换”的职责域

结论：

- 后续目录与模块命名中，优先使用 `graph` 而不是泛化的 `scene`

### 决策 4：把 `event / update / mutation / context / system` 收入 `runtime/`

原因：

- 这些都不是图结构本身，而是围绕图结构运转的系统服务
- 单独平铺在 `src/` 根下，会弱化“服务层”和“核心域”的区别
- 收入 `runtime/` 后，未来无论是否拆 crate，边界都更稳定

结论：

- `runtime/` 是理想架构中的“运行时机制层”
- 任何新的调度/上下文/服务型能力，默认应优先归入 `runtime/`

### 决策 5：把 `scene_host` 提升为 `host/scene_host.rs`

原因：

- `SceneHost` 是宿主接口，不属于图结构核心域
- 把它从根层移动到 `host/`，可以防止后续实现者把它误当成 `FigureGraph` 相关模块继续耦合
- 这也为未来的 `WinitSceneHost / WebSceneHost / HeadlessSceneHost` 留下自然位置

### 决策 6：把 `viewport` 明确为扩展容器层，而不是根层杂项模块

原因：

- `Viewport` 已经被确定为稳定扩展点
- 它会自然演化出 `Layer / Scroll / ScrollPane`
- 如果继续放在根层，后续扩展容易变成若干零散模块，而不是围绕“扩展容器”形成清晰子域

结论：

- `viewport` 后续应进入 `container/`
- `Layer / Scroll` 也应与它并列，而不是散落到根目录

## 为什么不建议现在立刻拆成更多 workspace crate

### 决策 7：先稳定接口，再稳定物理边界

按理想架构，长期上可以演进到类似下面的 crate 结构：

```text
novadraw-core
novadraw-geometry
novadraw-math

novadraw-figure
novadraw-layout
novadraw-graph
novadraw-runtime
novadraw-render

novadraw
apps/*
```

但本文不建议现在立刻这么拆，原因是：

- `FigureGraph / UpdateManager / EventDispatcher / SceneHost / Context` 的接口虽然已经定稿，但代码层面的最小垂直链路还没有全部落地
- 如果现在就拆 crate，接下来很容易把主要时间花在：
  - 依赖方向调整
  - facade 导出维护
  - trait 所属位置争论
- 这些成本不会提升架构质量，反而会拖慢关键基础结构的落地

结论：

- **目录先行，crate 后移**
- 这是为了把“架构稳定”优先级放在“物理拆分漂亮”之前

## 执行顺序

为了让目录调整服务于长期稳定，而不是制造一次性的大迁移成本，执行顺序如下：

1. **第一步：在 `novadraw-scene/src` 内部做目录重组（已完成）**
   - `scene -> graph`
   - `event/update/mutation/context/system -> runtime`
   - `scene_host -> host`
   - `viewport -> container`
2. **第二步：保持 facade crate `novadraw` 不变（已完成）**
   - 先不改变外部导出入口，避免同时引入 API 破坏
3. **第三步：优先实现最小垂直链路**
   - `FigureGraph`
   - `UpdateManager`
   - `EventDispatcher`
   - `SceneHost`
   - `RenderBackend`
4. **第四步：等子域边界稳定后，再按目录自然升级为 crate（暂缓）**
   - `figure/layout`
   - `graph`
   - `runtime`

## 目录调整时的禁止项

为了避免目录优化过程中反向破坏理想架构，补充以下约束：

- 不要为了“目录好看”把 `EventDispatcher` 放回 `graph`
- 不要为了“少文件”把 `SceneHost` 放回 `system`
- 不要为了“方便访问”让 `viewport` 直接侵入 `FigureGraph` 的核心职责
- 不要在 crate 尚未稳定前反复移动 facade 导出

## 最终判断

理想架构下，目录结构优化的目标不是“拆得越多越好”，而是：

- **让目录表达职责边界**
- **让未来 crate 拆分变成机械迁移**
- **优先保障核心运行时结构的长期稳定**

因此，本文正式建议：

> **先把 `novadraw-scene` 从“大而全场景 crate”整理为 `figure / layout / graph / runtime / host / container` 六个子域，再在实现稳定后逐步提升为独立 crate。**
