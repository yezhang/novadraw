# Draw2D 设计公理

类型：`reference-analysis`

本文档只基于 `draw2d/GEF` 源码提炼 Draw2D 的底层设计公理，不以当前项目实现为依据。

## 1. 文档目标

这里的“公理”不是某个类的实现细节，而是跨模块成立的系统级不变量。它们同时约束：

- `Figure` 树的结构语义
- `bounds / clientArea / insets` 的几何语义
- 坐标转换与坐标根
- `UpdateManager` 的更新事务
- 事件分发与交互状态机
- `Viewport` 的滚动与视口语义

如果这些公理被破坏，系统通常不会只在一个点上出错，而会同时污染命中测试、布局、重绘、裁剪、滚动和事件分发。

## 2. 适用范围

本文档讨论的是 Draw2D 运行时的“结构公理”，不讨论以下内容：

- 具体 Figure 子类的绘制细节
- 具体 LayoutManager 算法
- 具体 Anchor / Router / Connection 的策略
- 具体 Graphics 后端实现

这些内容建立在公理之上，但本身不是公理。

## 3. 分析入口

本次提炼主要依据以下源码文件：

- `org.eclipse.draw2d/IFigure.java`
- `org.eclipse.draw2d/Figure.java`
- `org.eclipse.draw2d/UpdateManager.java`
- `org.eclipse.draw2d/DeferredUpdateManager.java`
- `org.eclipse.draw2d/LightweightSystem.java`
- `org.eclipse.draw2d/EventDispatcher.java`
- `org.eclipse.draw2d/SWTEventDispatcher.java`
- `org.eclipse.draw2d/Viewport.java`

## 4. 公理清单总览

| 编号 | 公理名称 | 核心问题 |
|------|----------|----------|
| A1 | 轻量 Figure 树公理 | 系统真正组织单位是什么 |
| A2 | 树即运行时骨架公理 | 树关系除了包含还决定什么 |
| A3 | Bounds 统一几何公理 | bounds 到底是什么 |
| A4 | 局部坐标与坐标根公理 | 坐标相对谁 |
| A5 | ClientArea 盒模型公理 | 子内容在什么区域布局和裁剪 |
| A6 | 几何变更协议公理 | 几何变化必须如何发生 |
| A7 | 两阶段更新事务公理 | 为什么必须先 validation 再 repair |
| A8 | Parent-chain Damage 修复公理 | dirty rect 如何变成最终 damage |
| A9 | 事件分发状态机公理 | 事件由谁决定发给谁 |

## 5. 设计公理

### A1. 轻量 Figure 树公理

#### 定义

Draw2D 的基本运行单元不是 SWT 控件，而是轻量 `IFigure` 对象树。SWT Canvas 只提供宿主和系统事件入口，Figure 自身不对应 OS 原生控件。

#### 推论

- 图形组合能力来自 Figure 父子树，而不是 SWT 控件嵌套。
- Figure 的创建、销毁、重排成本必须远低于重量级控件。
- 绝大多数运行时语义都必须以 Figure 树为中心建模。

#### 破坏后症状

- 设计会错误地向“每个节点都需要平台资源句柄”方向滑动。
- 事件、绘制和更新职责会错误下沉到单个节点，失去统一调度。

#### 源码锚点

- `IFigure` 将 Figure 定义为 lightweight graphical object。
- `LightweightSystem` 明确自己是 SWT 与 Draw2D 的桥。

### A2. 树即运行时骨架公理

#### 定义

Figure 树不仅表示包含关系，还同时承载：

- 所有权
- Z-order
- 可见性传播
- 坐标传播
- 验证传播
- 命中测试遍历

#### 推论

- 子顺序天然就是绘制层级。
- hit-test 必须逆序遍历子节点，以体现“后添加者在上层”。
- `revalidate()` 和坐标换算都必须沿父链传播。

#### 破坏后症状

- 渲染顺序与命中顺序不一致。
- 局部改动无法正确影响祖先验证或祖先裁剪。
- 交互目标与视觉堆叠顺序错位。

#### 源码锚点

- `Figure.add()` 负责父子挂接、循环检测、revalidate、repaint。
- `Figure.findMouseEventTargetInDescendantsAt()` 使用反向子遍历。
- `Figure.isShowing()` 沿父链判断最终显示性。

### A3. Bounds 统一几何公理

#### 定义

`bounds` 是 Figure 的统一几何真相，不只是绘制矩形。它同时参与：

- `containsPoint`
- 擦除旧区域
- dirty region 修复
- 视口范围计算
- 布局尺寸与位置变化判断

#### 推论

- 不能把 `bounds` 当成普通临时值随意原地修改。
- 几何系统中的大部分协议都必须建立在 `bounds` 之上。
- 如果一个特性需要自己的位置矩形，它必须明确说明自己为何不复用 `bounds`。
- `bounds` 的“统一真相”并不意味着它处于单一全局坐标系；它必须始终处于**该 Figure 当前所属坐标域**中，且该坐标域由 A4/A6 的协议决定。

#### 破坏后症状

- 命中区域与实际绘制区域不一致。
- repaint 范围错误，导致漏擦或过绘。
- 视口滚动、裁剪和连接计算互相污染。

#### 源码锚点

- `IFigure.getBounds()` 的注释明确说明返回值可能是按引用返回，不允许调用方修改。
- `Figure.containsPoint()` 默认直接基于 `getBounds()`。
- `DeferredUpdateManager.repairDamage()` 在传播前先与 figure 自身 `bounds` 相交。
- `Figure.primTranslate()` 总是先修改自身 `bounds.x/y`，并以 `useLocalCoordinates()` 决定是否对子树传播位移（坐标域分段的关键体现）。

### A4. 局部坐标与坐标根公理

#### 定义

Draw2D 不是单一全局坐标系，而是父链递归坐标系统。基础 `Figure` 中，
`useLocalCoordinates()` 决定 children 是否相对当前 Figure client area 布局，
`isCoordinateSystem()` 默认等于它；scalable pane、Viewport 等子类可以覆盖
`isCoordinateSystem()` 和 parent/local 变换。

#### 推论

- “绝对坐标”不是简单相对画布原点，而是沿父链通过 `translateToParent / translateFromParent / translateToAbsolute / translateToRelative` 递归换算。
- 坐标变换不只包含 `bounds.x/y`，还包含 `insets.left/top`。
- 坐标域是**分段的**：基础 Figure 在 `useLocalCoordinates() = true` 时为 children
  建立局部域；其他坐标系统 Figure 可通过 scale、scroll 等
  `translateToParent/translateFromParent` 覆盖定义边界变换。
- 因此不能用“bounds 永远是 parent-local”或“bounds 永远是画布绝对”来简化 Draw2D 的坐标模型；正确理解是：`bounds` 处在“当前 Figure 所属坐标域”，跨域由 `translate*` 协议完成。

#### 破坏后症状

- 鼠标命中能对上边框，却对不上内容区域。
- 局部坐标 Figure 的子节点在父节点移动时发生错误传播。
- 视口、缩放、滚动与普通容器的语义混淆。

#### 源码锚点

- `Figure.isCoordinateSystem()` 返回 `useLocalCoordinates()`。
- `Figure.translateFromParent()` 与 `translateToParent()` 同时使用 `bounds` 和 `insets`。
- `Figure.translateToAbsolute()` 与 `translateToRelative()` 沿父链递归换算。
- `Figure.paintClientArea()` 在 `useLocalCoordinates() = true` 时执行 `translate(bounds + insets)` 并将 client area 的 clip 原点重置到局部 `(0, 0)`。

### A5. ClientArea 盒模型公理

#### 定义

`clientArea` 不是 `bounds` 的别名，而是 `bounds - insets` 后得到的“子元素布局区 + 子元素绘制裁剪区”。

#### 推论

- border / insets 是几何系统的一部分，而不只是视觉装饰。
- 标准 LayoutManager 以 container 的 client area 为布局区域；自定义布局仍可显式
  产生越界 bounds。
- 开启本地坐标后，client area 的原点必须被重置到局部 `(0, 0)`。

#### 破坏后症状

- 子节点会侵入边框或被错误裁剪。
- hit-test 与 paintChildren 使用的空间不一致。
- 带边框容器的布局和滚动都会出现“偏一圈”的错误。

#### 源码锚点

- `IFigure.getClientArea()` 注释明确说子节点布局和 children painting clip 都使用该区域。
- `Figure.getClientArea(Rectangle)` 先 `setBounds`，再 `shrink(getInsets())`，本地坐标下重置到 `(0, 0)`。

### A6. 几何变更协议公理

#### 定义

Figure 的几何变化不能直接写字段，而必须通过受控协议完成。`setBounds()` 和 `primTranslate()` 共同定义了这套协议。

#### 推论

- 几何变化至少要处理旧区域擦除、新旧位置比较、传播、invalid、moved 和 repaint。
- 平移与缩放是不同语义：平移影响坐标传播，尺寸变化影响布局失效。
- 几何变化的通知不是单一事件，可能需要区分 `figureMoved` 和 `coordinateSystemChanged`。

#### 破坏后症状

- 节点看起来移动了，但 dirty 区域没有正确提交。
- 子节点随着父节点重复移动或完全不移动。
- 监听器体系只收到“位置变了”，却不知道坐标域也变了。

#### 源码锚点

- `Figure.setBounds()` 执行 `erase -> primTranslate -> 更新宽高 -> invalidate/fireFigureMoved/repaint`。
- `Figure.primTranslate()` 在本地坐标模式下不修改后代 bounds，而是触发
  `fireCoordinateSystemChanged()`；这说明 local-coordinate Figure 的平移会改变
  后代的绝对映射，必须以通知协议显式表达。

### A7. 两阶段更新事务公理

#### 定义

更新必须分成两个阶段：

1. Validation：处理 invalid figures 与布局合法化。
2. Damage Repair：汇总 dirty regions 并执行重绘。

Validation 必须先于 Repair。

#### 推论

- `revalidate()` 不应立即同步做完所有工作，而应进入 UpdateManager。
- 单次用户操作引发的多个 invalid / dirty 请求应该被批处理和合并。
- 布局副作用导致的新 damage 必须落入同一事务更新序列中。

#### 破坏后症状

- 布局还没稳定就开始绘制，导致闪烁或旧区域残留。
- 同一父容器被重复布局多次，更新成本失控。
- 部分 dirty region 在错误几何基础上计算，最终漏绘。

#### 源码锚点

- `UpdateManager` 文档明确声明更新有两个 phase。
- `DeferredUpdateManager.performUpdate()` 先 `performValidation()` 再 `repairDamage()`。
- `Figure.revalidate()` 通过父链上推到 validation root 或根节点后交给 UpdateManager。

### A8. Parent-chain Damage 修复公理

#### 定义

dirty region 的最终 damage 不是局部矩形本身，而是它沿父链不断做：

1. 裁进自身 `bounds`
2. 转到父坐标系
3. 再裁进父 `bounds`
4. 一直传播到根

之后再参与 union。

#### 推论

- damage 修复天然依赖树结构、坐标变换和 bounds 语义的统一。
- dirty region 的输入坐标语义必须与 `translateToParent()` 协议一致。
- repair 不是“画脏了就画”，而是“求出根坐标下真正可见且必须重绘的区域”。

#### 破坏后症状

- 子节点 repaint 溢出父裁剪区域。
- 局部更新在深层嵌套或视口环境下出现错误 union。
- 看似正确的 repaint 在滚动或局部坐标场景下失效。

#### 源码锚点

- `DeferredUpdateManager.repairDamage()` 中先与自身 `bounds` 求交，再循环 `translateToParent()` 并与祖先 `bounds` 求交。

### A9. 事件分发状态机公理

#### 定义

事件不是直接由 Figure 自己“监听 SWT”得到的，而是由每个
`LightweightSystem` 所关联的 `EventDispatcher` 统一调度。默认
`SWTEventDispatcher` 的运行时交互依赖以下实例状态：

- `mouseTarget`
- `cursorTarget`
- `hoverSource`
- `focusOwner`
- `capture`

#### 推论

- 鼠标目标必须从根节点统一命中求解。
- capture 一旦建立，后续鼠标事件应绕过普通 hit-test，直接送给捕获者。
- focus 不是输入设备概念，而是 Figure 树中的交互所有权概念。

#### 破坏后症状

- 拖拽中鼠标移出后事件丢失。
- hover / tooltip / cursor 目标互相打架。
- 焦点切换无法与键盘遍历保持一致。

#### 源码锚点

- `LightweightSystem` 将 SWT 事件接到 `EventDispatcher`。
- `SWTEventDispatcher.receive()` 在 capture 与普通 hit-test 之间切换。
- `SWTEventDispatcher.setCapture()`、`releaseCapture()`、`setFocus()` 共同构成状态机核心。

## 6. 三个高阶闭包

为了后续做架构对齐，可以把 9 条公理进一步收敛成 3 个不可拆的闭包：

### 6.1 几何闭包

- `bounds`
- `insets`
- `clientArea`
- `containsPoint`
- `intersects`

只要其中一个语义变化，其余几项都必须重审。

### 6.2 坐标闭包

- `useLocalCoordinates`
- `isCoordinateSystem`
- `translateToParent`
- `translateFromParent`
- `translateToAbsolute`
- `translateToRelative`
- `fireCoordinateSystemChanged`

只要其中一个语义变化，命中、滚动、子节点传播都会被连带影响。

### 6.3 更新与交互闭包

- `invalidate`
- `revalidate`
- `validate`
- `addInvalidFigure`
- `addDirtyRegion`
- `repairDamage`
- `findMouseEventTargetAt`
- `capture / focus / hover`

只要其中一个被局部改写，更新事务和事件状态机会失去一致性。

## 源码基线

本文结论核对至 eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。

## 7. 对后续架构工作的意义

这些公理最重要的价值，不是“解释 Draw2D 为什么这样写”，而是提供后续对齐和迁移时的判定基线：

- 某个实现是否只是策略差异，还是已经破坏了公理
- 某条 API 是否只是命名不同，还是已经改变了坐标/更新语义
- 某个特性缺失能否局部补齐，还是需要先收口底层契约

## 8. 公理到契约的映射

本章将“设计公理”转写为“可执行契约”，目的是让后续设计审查不再停留在抽象讨论，而能直接回答：

- 当前设计是否仍然满足 Draw2D 的底层不变量
- 某次改动破坏的是局部策略，还是核心契约
- 在进入实现前，是否已经把关键边界讲清楚

建议在架构评审、API 设计评审、迁移方案评审时，逐条过表。

### 8.1 使用方式

对于任一设计方案，按如下顺序审查：

1. 先定位该方案主要触碰哪几个公理。
2. 再检查对应契约是否仍成立。
3. 只要某条契约答不上来，就不应进入实现阶段。

### 8.2 契约清单

#### C1. 轻量对象树契约

对应公理：A1

##### 契约定义

图形节点必须是轻量运行时对象，平台控件只承担宿主职责，不能让节点语义依赖原生控件树。

##### 设计审查问题

- 新设计是否把 Figure 节点与平台资源强绑定了？
- 如果离开当前平台宿主，Figure 树的语义是否还能成立？
- 事件、绘制、更新职责是否仍由系统层统一调度？

##### 通过标准

- Figure 树可以脱离平台控件树独立存在。
- 平台层只提供输入与绘制入口，不拥有业务图形语义。
- 图形节点不依赖“一节点一个原生控件”的模型。

#### C2. 树骨架一致性契约

对应公理：A2

##### 契约定义

父子树必须同时承载包含关系、Z-order、坐标传播、验证传播和命中遍历，不允许为这些语义引入彼此割裂的第二套结构。

##### 设计审查问题

- 是否引入了独立于 Figure 树之外的“命中树”或“更新树”？
- 子节点顺序是否仍与绘制和 hit-test 顺序保持一致？
- `revalidate()`、坐标换算、事件目标解析是否仍沿父链工作？

##### 通过标准

- 结构树是唯一主骨架。
- 渲染顺序与命中顺序的来源一致。
- 父链仍是坐标、验证和裁剪传播的统一路径。

#### C3. Bounds 统一语义契约

对应公理：A3

##### 契约定义

`bounds` 必须继续作为 Figure 统一几何真相，至少覆盖位置、尺寸、命中、擦除、重绘与 damage 修复的共同基准。

##### 设计审查问题

- 新设计是否把位置、命中、重绘矩形拆成了彼此语义不一致的多套字段？
- `bounds` 是否仍然是 repaint / repair / containsPoint 的共同输入？
- 是否存在“绘制位置正确但命中或裁剪使用另一套坐标”的情况？

##### 通过标准

- `bounds` 的语义在命中、更新、裁剪上保持一致。
- 若引入额外矩形，必须明确它与 `bounds` 的从属关系。
- 不允许无说明地绕开 `bounds` 直接手算重绘或命中区域。

#### C4. 坐标根与转换契约

对应公理：A4

##### 契约定义

坐标系统必须通过 `useLocalCoordinates / isCoordinateSystem / translate*` 这一组协议统一表达，不能让调用方自行拼接父子偏移。

##### 设计审查问题

- 当前设计中，“绝对坐标”到底相对谁？
- 子节点进入父节点坐标系时，是否考虑了 `insets`？
- 是否存在绕过统一转换 API、在业务代码里直接手加 `bounds.x/y` 的做法？
- 坐标根的定义是否稳定，还是依赖调用场景临时判断？

##### 通过标准

- 坐标根有唯一明确定义。
- 父子转换全部通过统一 API 完成。
- `bounds`、`insets`、client area 原点在转换语义中保持一致。

#### C5. 盒模型一致性契约

对应公理：A5

##### 契约定义

`bounds`、`insets`、`clientArea` 必须构成统一盒模型，子节点布局区和 children clip 区必须来自同一套定义。

##### 设计审查问题

- 子节点布局到底基于 `bounds` 还是 `clientArea`？
- 裁剪区与布局区是否使用了同一块区域定义？
- 开启局部坐标后，client area 是否仍以局部原点解释？

##### 通过标准

- 子节点布局区与 children clip 区来源一致。
- `insets` 既影响内容区，也影响坐标入口。
- 盒模型变化会同步更新布局、命中和裁剪语义。

#### C6. 几何变更协议契约

对应公理：A6

##### 契约定义

几何变化必须通过受控协议触发，包括旧区域擦除、位置传播、失效标记、移动通知与 repaint；不得直接绕开协议修改几何状态。

##### 设计审查问题

- 新设计中，节点移动或缩放是否仍会触发标准更新链？
- 平移与 resize 是否被区分对待？
- 局部坐标节点移动时，是否会改成“错误传播到子树”？
- 几何变化是否区分 `figureMoved` 与 `coordinateSystemChanged`？

##### 通过标准

- 不存在偷偷写几何字段却不触发更新协议的路径。
- 平移、缩放、本地坐标切换都有明确定义。
- 相关监听语义与更新语义保持一致。

#### C7. 两阶段更新事务契约

对应公理：A7

##### 契约定义

更新事务必须维持 `Validation -> Damage Repair` 的顺序；任何导致布局合法性变化的请求都必须先进入 invalidation 通道。

##### 设计审查问题

- 当前设计是否可能在布局未稳定时提前绘制？
- `revalidate()` 是立即执行，还是纳入统一更新事务？
- 多次 invalid / dirty 请求是否会被批处理和收敛？
- 是否允许“只 repaint，不先验证几何合法性”的路径？

##### 通过标准

- Validation 先于 Repair。
- invalid figures 通过 UpdateManager 统一处理。
- dirty region 计算建立在已经合法化的几何状态之上。

#### C8. Damage 父链传播契约

对应公理：A8

##### 契约定义

dirty rect 必须沿父链传播，并在每层做坐标转换与 bounds 裁剪，直到根层后再形成最终 damage。

##### 设计审查问题

- dirty rect 的输入坐标是相对谁定义的？
- 是否在每一层都做了裁剪，而不是只在叶子或根做一次？
- 坐标传播是否使用与正常父子变换一致的协议？
- 视口、滚动、本地坐标场景下，这条链是否仍成立？

##### 通过标准

- damage 的形成过程可追溯到完整父链。
- 传播与裁剪使用统一几何和坐标语义。
- 深层嵌套、滚动容器、本地坐标容器下结果仍自洽。

#### C9. 统一事件状态机契约

对应公理：A9

##### 契约定义

事件目标、capture、focus、hover、cursor 必须由系统级分发器统一维护，不能由各 Figure 自行持有不一致的输入状态。

##### 设计审查问题

- 鼠标目标是统一命中得到，还是各节点局部猜测？
- capture 建立后，是否仍存在普通 hit-test 抢事件的路径？
- focus / hover / tooltip / cursor 的所有权是否在同一状态机内？
- 键盘事件是否明确发往当前 focus owner？

##### 通过标准

- 输入状态机有唯一中心。
- capture、focus、hover、cursor 之间没有相互矛盾的状态来源。
- 鼠标与键盘事件目标都可解释、可追踪。

### 8.3 快速审查模板

在实际评审时，可以直接套用以下问题：

1. 这次改动触碰了哪些公理和契约？
2. 新语义是否改变了 `bounds / clientArea / 坐标根` 的定义？
3. 新流程是否仍满足 `Validation -> Repair` 顺序？
4. 事件目标是否仍由统一 hit-test 和状态机决定？
5. 若引入新结构，它是策略层扩展，还是已经在复制底层骨架？

### 8.4 审查结论分类

为了让后续评审更稳定，建议把结论分成三类：

- `通过`：契约保持成立，只是策略层差异。
- `需补充定义`：设计意图可能正确，但关键契约未说清。
- `违反公理`：已经改变底层不变量，不能直接进入实现。

如果后续需要继续扩展本文档，建议优先按以下顺序细化：

1. 每条公理补充更细的源码锚点
2. 将契约清单进一步细化为“模块级审查表”
3. 为每条契约增加“迁移实现时的检查项”
4. 建立“公理 -> 契约 -> 测试策略”的映射
