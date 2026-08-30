# Novadraw 图形引擎开发之书（大纲）

本书从第一性原理出发，讲解如何构建一个基于 Figure 树的 2D 图形引擎。
全书以“不变量（axioms）→ 失败模式 → 最小实现 → 验证闭环”的方式组织。
参考模型是 Draw2D/GEF（g2），但目标是教会读者图形引擎设计，而不是复刻某个 Java 代码库。

目标读者：
- 能写代码（Rust/TypeScript/Java 等），但从未从零实现过图形引擎的人。
- 希望理解 Novadraw 核心概念与设计动机的贡献者。

非目标：
- Novadraw 的终端用户使用手册（API 文档）。
- UI 组件库（本书聚焦引擎：几何、scene、update、render、event）。
- 超出引擎边界所需的 GPU 深入实现细节（只讲到后端边界与契约）。

----------------------------------------

## 第 0 部分：问题与解法的形状

### 0.1 为什么“画图很简单”，但“引擎很难”
目标：
- 区分“绘制原语（drawing primitives）”与“引擎不变量（engine invariants）”。

核心观点：
- 一致性胜过特性：hit-test、clip、update、坐标变换必须严格一致。
- 不变量一旦被破坏，局部小 bug 会演化为系统性失败。

里程碑：
- 写一个只画矩形的小程序；然后加入嵌套与事件，观察它如何在没有不变量支撑时崩溃。

验证：
- 形成一份“失败症状清单”，贯穿全书作为验收标准。

### 0.2 最小架构：Scene → Update → Render
目标：
- 建立一条“标准主链路”作为全书总线。

核心观点：
- Scene：持有 Figure 树与运行时状态。
- Update：把 mutation 转换为“已验证几何（validated geometry）”与“damage（重绘区域）”。
- Render：消费提交（submission）并产出像素。

里程碑：
- 实现一个 no-op 引擎骨架，让它能“tick”一个 update loop（哪怕什么都不画）。

----------------------------------------

## 第 1 部分：把“公理（Axioms）”当成真正的 API

### 1.1 Draw2D 设计公理（以 g2 为参考模型）
目标：
- 引入“公理 = 系统级不变量”的思维方式。

来源：
- doc/reference/draw2d/architecture/design-axioms.md

交付物：
- 一张“公理表”：每条公理由哪个模块/哪条链路负责落实。

### 1.2 三个闭包（Closures）：几何 / 坐标 / 更新+交互
目标：
- 解释为什么某些概念必须一起设计（不能后补）。

里程碑：
- 形成“闭包映射”：当某个概念变更时，哪些不变量必须重新审计。

----------------------------------------

## 第 2 部分：几何闭包（bounds / insets / clientArea / clip）

### 2.1 bounds 是几何真相的唯一来源（Single Source of Truth）
目标：
- 精确定义 bounds，并解释“语义含糊”的代价。

核心观点：
- contains、intersects、damage repair、layout、viewport 都必须共享 bounds。
- 如果某个特性引入自己的矩形，必须声明它与 bounds 的从属关系。

里程碑：
- 实现 `Bounded` + 一个最简单的 `RectangleFigure`。

验证：
- contains/intersects/union/intersection 的单元测试。

### 2.2 盒模型：insets 与 clientArea
目标：
- 把 border/insets 讲成几何协议，而不是视觉装饰。

核心观点：
- `clientArea = bounds - insets`
- clientArea 同时定义“子元素布局区”和“children clip 区”。

里程碑：
- 实现一个 border，并证明子节点永远不会画进 border 区域。

验证：
- 渲染 trace + clientArea 数学验证单测。

### 2.3 裁剪策略：谁在何处 clip，为什么
目标：
- 定义一致的 clip 契约（clip contract）。

核心观点：
- 先 clip 到 figure bounds，再在 paint children 时 clip 到 clientArea。
- optimizeClip 是策略选择；不变量是“绝不在允许区域外绘制”。

里程碑：
- 做一个嵌套场景，用 clip 阻止 overdraw（部分越界绘制）。

----------------------------------------

## 第 3 部分：坐标闭包（坐标根与坐标变换）

### 3.1 坐标域是分段的（useLocalCoordinates）
目标：
- 讲清核心概念：坐标根会切分坐标域（coordinate domains）。

核心观点：
- “绝对坐标（absolute）”不是单一全局原点，而是通过父链协议递归计算。
- 当 `useLocalCoordinates=true` 时，子节点 bounds 不再被父移动直接传播。
- `primTranslate` 体现了分段：要么传播位移，要么触发 `coordinateSystemChanged`。

里程碑：
- 实现一个 coordinate-root 容器：子元素在局部空间（local space）布局与绘制。

验证：
- `translateToParent / translateFromParent / translateToAbsolute / translateToRelative` 的单测。

### 3.2 父链变换协议（translate*）
目标：
- 让坐标变换 API 成为跨坐标域的唯一合法途径。

核心观点：
- `translateToAbsolute/Relative` 必须通过“父协议递归”完成（g2 模型）。
- bounds 与 insets 都必须反映在变换中。

里程碑：
- 实现一个 hit-test：在嵌套 coordinate roots 下仍能命中正确目标。

----------------------------------------

## 第 4 部分：渲染闭包（遍历、提交、后端）

### 4.1 渲染是一种确定性的树遍历
目标：
- 解释 paint order、z-order，以及为什么递归遍历在工程上有风险。

核心观点：
- 迭代遍历 vs 递归遍历。
- `PaintSelf -> PaintChildren -> PaintBorder` 的阶段化。

里程碑：
- 用显式栈任务实现迭代渲染器（trampoline）。

验证：
- 快照测试：给定树结构，渲染顺序必须可预测且稳定。

### 4.2 RenderSubmission 作为后端契约（Backend Contract）
目标：
- 分离“画什么（what）”与“怎么画（how）”。

核心观点：
- submission 持有 commands 与 damage set。
- 后端只消费 submission，因此后端可替换。

里程碑：
- 做一个 CPU debug backend + 一个 GPU backend，让它们消费同一份 submission。

----------------------------------------

## 第 5 部分：更新闭包（两阶段更新与 damage 修复）

### 5.1 为什么更新必须两阶段（Validation → Repair）
目标：
- 把 update 教成“事务（transaction）”，而不是一堆回调。

核心观点：
- Validation：稳定几何（geometry stabilization）。
- Repair：在几何稳定后计算最小重绘区域（minimal repaint region）。
- repair 期间新增 dirty 必须进入下一轮（snapshot semantics）。

里程碑：
- 实现一个 deferred update manager：合并 invalid + dirty。

验证：
- snapshot 语义与调度（scheduling）的单测。

### 5.2 父链 damage 修复（Parent-chain Damage Repair）
目标：
- 解释为什么 dirty 不等于最终 damage。

核心观点：
- 先与自身 bounds 相交，再 translateToParent，再与父 bounds 相交，直到根。
- damage 的正确性依赖坐标闭包的正确性。
- regions vs union 的权衡。

里程碑：
- 实现 region-aware 的 damage set，并提供安全的规范化与回退到 union。

验证：
- propagation、clip、region normalization 的单测。

----------------------------------------

## 第 6 部分：交互闭包（命中、分发、焦点、捕获）

### 6.1 命中测试：从 bounds 到形状包含（shape containment）
目标：
- 建立可靠的目标选择模型（target selection）。

核心观点：
- z-order 规则与可见/可用过滤。
- 先 bounds hit-test，再可选进入 shape hit-test。

里程碑：
- 鼠标目标选择必须与 paint order 一致。

### 6.2 事件分发状态机
目标：
- 解释为什么 dispatch 必须集中且有状态（stateful）。

核心观点：
- hover/mouse_target/capture/focus 是全局交互状态。
- capture 会绕过普通 hit-test。

里程碑：
- 拖拽过程中鼠标移出 bounds 后也不丢事件。

----------------------------------------

## 第 7 部分：通知与可扩展性（Listeners 作为基础设施）

### 7.1 为什么通知不是可选项（coordinateSystemChanged）
目标：
- 解释为什么高级特性依赖通知体系。

核心观点：
- 坐标根移动会改变坐标域映射。
- viewport/scroll/router/thumbnail 依赖坐标与几何变化信号。

里程碑：
- 最小 listener 系统（UpdateListener + CoordinateListener）端到端接入。

### 7.2 把 Viewport / Scroll / Thumbnail 当作扩展来实现
目标：
- 展示扩展模式，避免污染核心闭包。

里程碑（选 1–2 个即可）：
- Viewport container
- Scroll pane
- Thumbnail/overview

----------------------------------------

## 第 8 部分：为演化而工程化

### 8.1 测试策略：把公理变成回归测试（Axiom Regression Tests）
目标：
- 把公理转成可执行验证。

核心观点：
- 用最小测试阻止语义漂移（semantic drift）。
- 避免“复述实现”的低价值测试。

交付物：
- 一套稳定测试集，能捕获 coordinate/damage/hit-test 的语义回归。

### 8.2 工作流：delta、拆分与架构债务
目标：
- 教会读者如何安全演化架构。

核心观点：
- 一次只做一个 delta，范围受控；验证通过才能标记 done。
- 何时拆分 delta、如何记录决策。

里程碑：
- 把 backlog 与 checkpoint 循环作为仓库的一部分（可复用方法）。

----------------------------------------

## 附录

- A：术语表（bounds/clientArea/insets、coordinate root、damage、submission、capture）
- B：常见失败模式与调试方法
- C：参考资料（Draw2D/GEF 源码锚点、Novadraw 文件地图）
