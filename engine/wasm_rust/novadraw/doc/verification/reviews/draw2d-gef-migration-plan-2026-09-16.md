# Draw2D / GEF 语义迁移实施方案

类型：`reference-analysis`

状态：建议方案，尚未整体实施。

本方案依据[语义映射](draw2d-gef-semantic-mapping-2026-09-16.md)与
[差异报告](draw2d-gef-semantic-differences-2026-09-16.md)。
实际关闭进度只维护在[整改状态页](draw2d-gef-semantic-remediation-2026-09-16.md)，
不另建冲突的完成清单。下文“阶段一至六”是本方案顺序，不复用M1-M10/G0-G6编号。

## 目标与不可牺牲的契约

1. paint、hit、layout、damage、anchor、locator从同一坐标事实派生。
2. 模型是业务真源，命令不保存可失效的Figure/EditPart身份，undo/redo复用通知投影。
3. 取消和可恢复失败不提交半成品；无法恢复的一致性破坏显式fault。
4. Figure、Layout、Border、Anchor、Router、Locator、Tool、Request、Policy
   的真实扩展不要求修改引擎中心分支。
5. 所有输入目标、capture、focus、hover和gesture身份具有独立职责和结束条件。

保留Rust现有优势：值对象、关联类型、短借用上下文、分域ID、显式Result、
typed payload、prepared output与资源revision。无需复刻Java宽接口、继承层级、
SWT类型、静态临时对象和任意对象别名。

## 当前执行顺序（2026-09-17 校准）

下面是阶段一至六的活动调度顺序，不创建新的 roadmap milestone。当前已关闭
F01/F02/F03/F16/F17；剩余问题以整改状态页为准。

| 顺序 | 工作包 | 范围与依赖 | 退出条件 |
|---|---|---|---|
| 0 | 冻结已关闭基线 | Web、listener scope、事件分轨、pointer leave、Policy target | 按主题原子提交；Core/G3/G4/G5.5/Web 与 workspace full gate 通过 |
| 1A | Viewer 初始失败清理 | F13；先于后续 Editor 扩展和产品验收 | 任意 root/child factory、visual、activate 或 policy 安装失败时，所有已激活 Part 恰好 deactivate 一次，无残留 registry/visual |
| 1B | Viewer panic 隔离 | F15；依赖 1A 的统一生命周期事实 | 从 model drain 到 projection commit 的扩展 panic 立即使 Viewer faulted；revision 不提交，后续写操作拒绝 |
| 2A | Connection 失败恢复 | F10 | 首次退化 route/Locator 拒绝后保留本次 observation；owner 修复后自动重新调度并成功 |
| 2B | Connection 路由方向 | F11 | A→B/B→A 与混合方向 Fan 线路不重合，删除后 lane 顺序稳定 |
| 2C | Connection reparent 契约 | F12；实施前单独评审 constraint 坐标域迁移接口 | built-in constraint 在换父前完成域转换；不能迁移的 custom constraint 原子拒绝，旧 topology/route/binding 不变 |
| 3A | 布局坐标真源 | F05 | node-local paint client box 与 child-content arrange area 分离，inset 只应用一次 |
| 3B | Damage 与 BorderLayout | F04/F06；依赖 3A 的坐标约定 | freeform overflow old/new damage 完整；Border 各区预留与最终摆放使用同一尺寸 |
| 3C | 资源与组合视觉 | F07/F09；F08 依赖 3A | Removed image 不再引用旧资源；Label 四方向正确；嵌套动态 Border 保留测量与绘制 |
| 4 | G5 检查点 C | 依赖 1A/1B、2A-2C、3A，以及影响 retained visual 的 F04 | Native 验证 create/reconnect/bendpoint、scroll/zoom、auto-expose、取消和 undo/redo；通过只提升 G5，不关闭其他审计项 |
| 5 | 开放 Editor 扩展端口 | 原阶段四 | 外部 crate 可增加 Tool/Tracker、typed Request、Policy/feedback、SelectionPolicy 与 RootLayerFactory，无需修改中心分支 |
| 6 | 投影复杂度 | F14；正确性和扩展接口稳定后执行 | 无变化属性 refresh 不再产生 Θ(E²) 框架扫描；E=100/1,000/10,000 有操作计数与耗时证据 |
| 7 | G6 产品毕业 | 依赖剩余 P1 全部关闭、检查点 C 和阶段五/六门禁 | serializer、保存重建、Native/Web/Headless 同事务与最终验收 |

执行约束：

- 1A 与 1B 同属 `viewer.rs`，必须串行、分主题提交，不能在一次大改中混合清理与
  unwind 语义；
- 2A 与 2B 可独立实现，但合并验证必须覆盖共享 Router batch；2C 未完成设计评审前
  不修改 reparent；
- 检查点 C 不等待 Tool 扩展或 F14 性能优化，但不得绕过 Connection、坐标和 retained
  damage 的直接影响项；
- F06/F07/F08/F09 不阻塞检查点 C 的既有节点编辑场景，仍必须在 G6 前关闭；
- 每项先保留修复前反例，再提交根因修复与正向回归。

## 阶段一：稳定基线与关闭已有反例

前置：固定本次audit快照与工作区改动，逐项核查整改记录；已经关闭的
F01/F02/F03/F16/F17只复核证据，不重做修复。后续关闭状态继续以整改页为准。

| 工作包 | 范围 | 目标契约 / 实现方向 | 关闭标准 |
|---|---|---|---|
| 输入与作用域 | F01/F02/F03/F17 | target transition管理enter/exit；物理hover独立；平台提供release/cancel闭环；scope登记不受debug配置影响 | handlerless child→parent无伪enter；press→exit/blur→reenter无残留capture；release配置scope释放；wasm32构建通过 |
| 生命周期与fault | F13/F15 | 部分创建有独立activation账本；清理不依赖contents最终提交；Viewer从drain到projection commit有poison/unwind边界 | child factory/visual/activate/policy失败均正确清理；修改visual后panic立刻fault；后续写操作拒绝 |
| 目标解析 | F16 | target resolution与command contribution分开，校验返回身份，保持source request | source A重定向B时B贡献command/feedback；foreign/stale拒绝；多source到同target的去重规则明确 |
| 派生状态 | F07/F09 | Removed资源清除所有引用与派生尺寸；placement按文字相对图标定义 | Ready→Removed下一帧无旧image命令且可提交；四方向glyph/icon位置、gap与named geometry一致 |
| 连接失败与方向 | F10/F11/F12 | 失败保留恢复依赖；无向分组用统一法向；reparent前转换constraint或拒绝 | 零长度Locator失败→owner移动自动恢复；混合方向Fan不重合；换父后surface折点保持或整次拒绝且状态不变 |

执行时采用“最小反例→根因修复→对应回归→相关suite”，每包独立、中文原子提交。
新增测试按仓库单测工作流执行。本方案没有替用户新增测试或完成这些实现。
不能用全屏重绘、忽略缺资源、吞panic或强制再resolve绕过根因。

## 阶段二：统一布局域与测量协议

依赖：阶段一的失败隔离基础。先确定坐标/盒模型契约，再修改具体算法。
F04/F05/F06与布局未决delta在这里一起闭合，避免仅修零inset演示。

建议接口语义：

- `LayoutContext`明确区分node-local paint client box、child-content arrange area、
  owner外框测量；名称和返回域写入文档。
- child-content区域由统一变换计算。inset只应用一次，scalable/viewport按其坐标协议
  返回有效区域，不能各布局手写减inset或除scale公式。
- min/preferred/max查询规定输入hint域、自动轴、inset扣除和外框加回步骤；
  width受限的文本必须在最终可用宽度重测height。
- 提供外部LayoutManager真正需要的child visibility、盒模型与受限测量查询；
  保持只读snapshot与受检output，不开放任意树写。
- `LayoutOutput`在提交前校验结构及数值；允许的负坐标与不允许的负尺寸分开。
- old/new damage以移动前的可见包络或保守根域区域为依据，不被owner非包络bounds裁掉。

| 算法 | 必须完成的差异校准 | 验证矩阵 |
|---|---|---|
| XY / Freeform | 固定轴传给自动轴测量；无constraint处理明确 | width固定/height自动、反向、零尺寸、负位置、wrapping Figure |
| Stack | child-content区域与外框测量一致；visibility策略 | inset、border、scale、多个child min/pref、hidden |
| Border | 每区唯一最终尺寸用于预留和摆放；重复region和缺region语义明确 | South/East超过半轴、全/空region、重复region、visible切换、hints |
| Grid | equal+grab一致、压缩不能随意越minimum、宽约束后重测 | 单列grab、span、indent、过小容器、换行文本、横纵组合 |
| Flow / Toolbar | 屏蔽不适用hint轴，排列与测量一致 | 横纵、stretch开关、min大于available、alignment、spacing |
| Viewport / ScrollPane | tracks轴hint传播、滚动条互相影响稳定求解 | width-sensitive内容、负extent、双轴bar、resize与zoom |

验收：同一场景比较 arrange geometry、paint command、hit point、old/new damage与
preferred/minimum；不能只检查最终bounds。通过后再调整相应parity family范围。
Draw2D改动作为明确P2 delta管理；这里的P2是路线图类别，不是缺陷严重度。

## 阶段三：完成可组合的组件与资源协议

依赖：阶段二盒模型与阶段一资源失效。主要闭合F08和相关扩展收窄。

| 边界 | 建议 | 避免的长期限制 | 验收 |
|---|---|---|---|
| 动态Border | owner-scoped prepared snapshot可按组合树递归构建；子快照与累计inset匹配 | 顶层downcast TitleBar导致嵌套丢语义 | TitleBar inner/outer/多层；共享border不同字体owner；metrics/glyph/clip一致 |
| Part内部visual | VisualUpdateContext提供受owner校验的内部Figure更新能力 | 只能更新primary或由Host直接改Runtime | 自定义compound Part更新label、pane尺寸、内部样式，外部owner写入拒绝 |
| Scalable扩展 | 在能力边界提供受检scale读写与协调入口 | ZoomManager只认两种内置downcast | 外部Scalable实现接入zoom，保持pointer锚点、测量与damage |
| Connection geometry | 明确prepared数据是否可表达曲线、非对称bounds和装饰；按实际需求扩展 | 有trait但输出只能内置点列格式 | 外部geometry实现、拒绝预检、hit/paint/envelope一致 |
| 后端能力 | 为dash/arc等建立支持或显式拒绝规则 | 公共API接受参数却静默丢弃 | 每个公开命令能lower或报unsupported；不能只测IR存在 |

无需把所有组件都泛化成万能上下文。以两个真实不同实现证明边界必要：
普通静态Border与带文本动态Border、普通Part与compound Part、内置与外部scalable。
能力不在当前交付范围时，公开API和parity必须表达该限制。

## 阶段四：开放GEF编辑扩展

这一步涉及跨模块公共API，先评审方案与兼容策略，再实施。
当前typed内置工具可以保留为默认实现，重点是补齐注入与调用链。

### Tool与Tracker

引入统一平台无关输入协议，包含pointer/button/modifiers/key/focus/cancel及时间信息；
Domain拥有一个可替换active Tool，Viewer只提供受检target/coordinate/feedback服务。
Tool可安装，Tracker由Part/Handle能力产生并在gesture内固定。

优先采用object-safe Tool/Tracker + 受限上下文；当前Domain方法对Factory泛型参数
不宜原样塞进trait对象。先抽取所需Viewer服务，避免为可替换而暴露整个Runtime。
关闭条件是外部crate完成一个新增工具，不修改Domain的match分支。

### Request与Policy

保留内置强类型request；可选方案：

| 方案 | 优点 | 代价 | 建议 |
|---|---|---|---|
| Domain泛型应用Request | 编译期封闭、无运行时擦除 | 泛型传播到Tool/Policy；插件组合困难 | 适合单应用静态组合，可作替代方案 |
| 内置Request + typed extension envelope | 内置易用，外部可加payload，适合策略插件 | 需稳定type key、受检downcast与生命周期设计 | 默认建议，借鉴已有typed constraint，不用无约束Any map作为主协议 |
| 仅增加Custom字符串 | 改动少 | 无类型安全，参数散落，容易静默失败 | 不采用 |

目标解析先找候选，再应用Policy返回目标，最后在目标聚合command/feedback。
默认单次重定向与确定role优先级即可；只有真实需求才支持链式重定向及环检测。
source身份、target身份、是否理解、是否拒绝是四件不同的事实。

同时补齐实时modifier、active Tool键盘仲裁、取消/失焦协议；
SelectionPolicy与RootLayerFactory需实际注册入口，不只列在架构文档。

验收外部crate至少包含：自定义请求/工具、复合Part重定向到父layout policy、
自定义handle tracker、Figure-native键盘控件与Delete Tool互不误触、
gesture中Shift变化、source退休后的取消与feedback释放。

## 阶段五：算法复杂度与稳定性

依赖：正确性反例已关闭，避免优化固定错误语义。主要闭合F14。

- 一批reconciliation只读取/校验一次child/connection顺序；变化时使用受检批量reorder。
- routing order、group membership和locator binding按批建立索引并复用；
  保留prepare/validate/commit，不为性能绕开Runtime校验。
- 将无变化、稀疏变化、全重排、批量删除、高度数端点分别测量；
  不把平均哈希O(1)推成所有数据结构操作O(1)。
- 基准同时记录模型query数、child order复制量、route次数、分配量和耗时。
  对E=100/1,000/10,000及宽树比较增长率；外部router自身成本单独计。
- 关闭条件：未变顺序的一次属性refresh不再产生框架Θ(E²)扫描；
  全量批处理达到所声明O(V+E)范围，复杂router的额外成本明确披露。

递归主渲染继续保持当前路线；10,000深度协议与栈安全、性能测试分别验收。

## 阶段六：证据与产品交付

1. 将上述修复和新扩展映射到已有Draw2D/GEF family，填写方法子集与明确边界。
   “verified”必须包含失败路径、生命周期、外部替换及至少一个端到端证据。
2. 完成G5检查点C人工验收，重点scroll/zoom期间source、connection、endpoint、
   bendpoint index保持锁定，释放只产生一个Command，undo/redo可重放。
3. 再按现有G6 roadmap推进serializer/保存重建与Native/Web/Headless同一事务。
   模型语义相同、运行时ID重新分配；不把历史save location当成保存实现。
4. 验证manifest使用稳定suite/command ID；加入wasm32编译及必要release profile覆盖。
5. 资源删除、partial damage和动态border需后端提交/视觉证据；不以recording IR代替GPU。

已有可用门禁包括workspace.quality、verify.update/event/scroll-pane、
replay.editor-g3/g4/g5.2/g5.3/g5.4/g5.5和web.build；
具体命令定义见[verification/suites.toml](../../../verification/suites.toml)。
新反例或扩展suite应先登记manifest，报告只记录ID及结果。

## 实施控制与风险

每个包交付：契约说明、最小反例、根因实现、失败路径、自动证据、受影响family及
必要人工验收。跨模块/公共API先评审具体取舍；后续实现授权不能由本报告代替。
各包按主题提交，禁止把既有未提交工作混入审计提交。

最大迁移风险是把“移除Java技术限制”误解为“删除扩展能力”。Rust可以限制可变别名，
但必须用受检上下文与typed output保留应用表达力；可以后置高级能力，
但不得把后置、静默no-op和完整等价混为一谈。
