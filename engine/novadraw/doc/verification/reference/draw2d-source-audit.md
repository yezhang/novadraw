# Draw2D / GEF 文档源码审计

类型：`verification`

## 1. 基线与判定规则

- 参考仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic`
- 参考提交：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
- 提交日期：2026-01-01
- 审计日期：2026-08-29

本文档只记录能由上述源码直接支持的事实。文档中的内容分为三类：

1. **Draw2D/GEF 源码事实**：必须能定位到 class、method 或接口注释。
2. **架构推导**：必须标明是从多个源码事实得到的归纳，不能伪装成源码原话。
3. **Novadraw 设计选择**：可以不同于 Draw2D，但必须明确写成映射或合理变体。

行号只用于阅读当前基线，不作为长期锚点；长期锚点使用
`package/class/method`。

## 2. 核心结论

| 主题 | 源码结论 | 主要证据 |
|---|---|---|
| Figure 树 | `Figure` 直接持有 parent/children；Draw2D 没有独立 SceneGraph 对象 | `Figure` fields、`add/remove` |
| 遍历 | 同层枚举使用 loop/iterator，但 paint、validate、hit-test、translate 等树路径是递归的 | `Figure.paintChildren/validate/findFigureAt/primTranslate` |
| Paint | `Figure.paint()` 自己执行 style、state、self、client/children、border 全流程；方法不是 `final` | `Figure.paint` |
| Bounds | 当前 Figure 的 bounds 位于 parent 提供的坐标域；`useLocalCoordinates()` 控制它为 children 提供的坐标域 | `Figure.useLocalCoordinates/primTranslate/translate*` |
| Client area | `getClientArea()` 从 bounds 减去 border insets；local 模式下原点重置为 `(0,0)` | `Figure.getClientArea` |
| Child clip | 默认按 child bounds 裁剪；`IClippingStrategy` 可返回多个矩形裁剪区 | `Figure.paintChildren` |
| Layout | `LayoutManager` 是策略接口，但实现可以保存 constraint 和尺寸缓存 | `AbstractLayout`、`AbstractConstraintLayout`、`GridLayout` |
| Validation | `revalidate()` 沿父链到 root/validation root；`validate()` 先 layout，再递归 children | `Figure.revalidate/validate` |
| Update | 更新顺序是 Validation → Damage Repair；默认实现异步合并请求 | `UpdateManager`、`DeferredUpdateManager` |
| Damage | dirty rect 先裁进 source bounds，再沿祖先链转换、裁剪并 union | `DeferredUpdateManager.repairDamage` |
| Input | 抽象 `EventDispatcher` 定义契约；默认 `SWTEventDispatcher` 持有 target、focus、hover、capture 状态 | `EventDispatcher`、`SWTEventDispatcher` |
| Event point | `MouseEvent` 构造时调用 source 的 `translateToRelative()`，回调接收 source-relative 点 | `MouseEvent` constructor |
| Host scope | 每个 `LightweightSystem` 实例持有 root、manager、dispatcher 和 Canvas；不是进程级 singleton | `LightweightSystem` fields |
| GEF MVC | EditPart 连接 model 与 Figure；content pane 是 child Figure 的容器，不是“叶子” | `GraphicalEditPart`、`AbstractGraphicalEditPart` |
| GEF commands | Tool/EditPolicy 产生的编辑操作通常经 CommandStack；框架不能禁止应用直接修改 model | GEF tools、EditPolicy、CommandStack |

## 3. 已核对文档

### 直接源码分析

- `doc/reference/draw2d/architecture/design-axioms.md`
- `doc/reference/draw2d/architecture/history.md`
- `doc/reference/draw2d/figure/*.md`
- `doc/reference/draw2d/rendering/*.md`
- `doc/reference/gef/core-principles.md`

### 对标与 Novadraw 设计文档

以下文档引用 Draw2D 事实，但其权威角色不是外部源码说明。审计只把引用的外部
事实作为源码一致性对象；Novadraw 内容必须明确写成映射、合理变体或目标契约：

- `doc/parity/draw2d/*.md`
- `doc/design/**/*.md`
- `doc/archive/render-iterative-poc.md`

`doc/archive/architecture-legacy.md` 已明确标为历史文档，不作为当前 Draw2D
语义真源。`doc/archive/trampoline-rendering.md` 也只保留历史方案。

## 4. 本次纠正的错误类型

- 将 Draw2D 树遍历误写成“迭代、避免递归”。
- 将 `Figure.paint()` 误写为 `final`，或误写成由 `LightweightSystem` 编排 children。
- 虚构 `FigureListener.figureResized()` 和 `Figure.fireRequestLayout()`。
- 颠倒 `translateToParent()` 与 `translateFromParent()` 的方向。
- 把 `useLocalCoordinates()` 误解释为改变当前 Figure bounds 的存储域。
- 把 `FreeformLayer`、`FreeformLayeredPane` 无条件列为坐标根。
- 把 `LayoutManager` 具体实现误写成无状态。
- 把 `EventDispatcher` 抽象基类误写成直接保存交互字段；字段实际在
  `SWTEventDispatcher`。
- 把 manager/dispatcher 误写成进程级“全局唯一”。
- 把 `FigureCanvas` 误列为 Figure 类型。
- 把 GEF content pane 误写成叶子节点。
- 把鼠标 press 的 capture 时序写反；实际是 handler 消费事件后建立 capture。
- 把 `GC.translate` 当成 SWT GC API；逻辑平移由 `SWTGraphics` 管理。
- 把旧版本行号和历史推测当作稳定源码事实。

## 5. 防回归规则

1. 新增 Draw2D 事实时，同时写明源码 class/method。
2. 不以固定行号作为唯一证据。
3. 不把 Novadraw 的所有权、递归深度或无状态约束反写成 Draw2D 原生设计。
4. 不把 `EventDispatcher` 与 `SWTEventDispatcher`、`ScalableFigure` 与具体 pane
   实现混为一谈。
5. 修改 paint、coordinate、validation、damage 或 event 语义时，同时检查
   `doc/reference/draw2d/architecture/design-axioms.md` 与
   `doc/parity/draw2d/api-coverage.md`。
