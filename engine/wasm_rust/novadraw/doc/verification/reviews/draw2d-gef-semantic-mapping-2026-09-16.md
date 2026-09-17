# Draw2D / GEF 语义映射对照表

类型：`reference-analysis`

本表对比 2026-09-16 固定工作区与官方 GEF Classic
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`。
结论按行为契约分解，不以类名数量计算兼容百分比。
审计期间后续整改不回写原始证据；恢复工作时的状态见
[差异报告](draw2d-gef-semantic-differences-2026-09-16.md)。

## 阅读方法与证据

官方指南、源码版本及状态定义见
[固定基线](../reference/draw2d-gef-semantic-baseline-2026-09-16.md)。
以下 G1—G6 是审计分组简称，不是 Editor roadmap 里程碑：

| 简称 | 方法级证据（含 Java/Rust 行号、调用链、测试边界） |
|---|---|
| G1 | [Figure/事件](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_1.md)、[生命周期/平台补充](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_1_supplement.md) |
| G2 | [布局/更新/Viewport](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_2.md) |
| G3 | [绘制/几何/资源/文本/widget](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_3.md) |
| G4 | [Connection/Anchor/Router/Locator](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_4.md) |
| G5 | [Model/Command/EditPart/Viewer](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_5.md) |
| G6 | [Tool/Request/Policy/Feedback](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/group/group_6.md) |

“迁移”指保留核心行为的 Rust 表达变化；“收窄”不等于与 Java 全部等价。
无明确延期依据的算法差异标为“部分”，不能自动视为已批准收窄。
“保留”仅限本行所述行为。下表与分组全文合起来构成详细映射交付物。

## Figure、事件与生命周期

| 编号 | 官方类/接口/方法 | 核心语义 | Rust 对应 | 评估 / 差异 | 证据 |
|---|---|---|---|---|---|
| S01 | IFigure.add/getParent/getChildren | 单父、有序组合，paint/hit 共用 Z-order | FigureTree + FigureNode + Vec children | 迁移；SlotMap 与反向 parent 替代对象引用 | G1 |
| S02 | Figure.add(index)/remove/reparent | 拓扑、顺序及通知同步 | Runtime::try_reparent/move_child_to_index | 部分；重排可用，indexed add 与保活 detach 不完整 | G1 |
| S03 | Java Figure identity | 引用对象不能误指新对象 | RuntimeNamespace + generational FigureId | 迁移；拒绝 foreign/stale，业务持久 ID 另属 Model | G1 |
| S04 | addNotify/removeNotify | 挂载及资源订阅成对释放 | complete_attachment/RetiredSubtree | 迁移；移除即退休，不保留 Java detached live object | G1 |
| S05 | findFigureAt/TreeSearch | 逆序遍历、prune 子树、accept 候选 | hit_test_with/ExclusionSearch | 收窄；普通几何查找也剪枝 disabled，而 Java 普通查找主要过滤 visible | G1 |
| S06 | findMouseEventTargetAt | 几何命中加输入资格 | MouseEventTargetSearch | 基本保留；不能将它与普通 hit API 合并为同一语义 | G1 |
| S07 | SWTEventDispatcher.receive | mouseTarget 变化产生 enter/exit | EventDispatcher::refresh_mouse_target | 原快照缺陷：用几何 hover source 派发；详见 F01 | G1 |
| S08 | MouseEvent/dispatchMouseHover | receiver 坐标；hover 发到事件目标 | EventContext/SceneDispatchContext | 坐标迁移合理；原 hover 目标与 S07 同根因 | G1 |
| S09 | capture/release | 拖动固定接收者，终止后释放 | handled press + Runtime capture | 部分；离窗 release 丢失的终止协议需闭合，F03 | G1 |
| S10 | MouseEvent.stateMask | 按钮与实时 modifier 可读 | MouseEvent/KeyModifiers/RequestModifiers | 收窄；Figure mouse 缺完整 modifier，Tool 保存 press snapshot | G1/G6 |
| S11 | FocusTraverseManager | 单一焦点、遍历与失效清理 | Runtime::request_focus/FocusTraversalPolicy | 迁移；policy 可替换并校验返回身份 | G1 |
| S12 | Figure/Ancestor/Coordinate listeners | 观察不同因果事实；注销释放 | typed journal + scoped ListenerId | 迁移；原 release scope 写入被 debug_assert 跳过，F02 | G1 |
| S13 | LayoutListener.layout / phase hooks | 同步介入与事后通知不同 | LayoutListener/Observation journal | 收窄；stable journal 无法替代同步 veto/布局介入 | G1/G2 |
| S14 | LightweightSystem | 平台适配与引擎语义分层 | PlatformHost + Runtime + apps input | 迁移；输入点转换在引擎，Editor key 仲裁仍待补全 | G1/G6 |

## 布局、更新与坐标

| 编号 | 官方 API | 核心语义 | Rust 对应 | 评估 / 差异 | 证据 |
|---|---|---|---|---|---|
| S15 | LayoutManager/AbstractLayout | 替换算法、父拥有约束、失效缓存 | LayoutManager/LayoutConstraint/LayoutSnapshot | 迁移；typed constraint、generation+hints cache 真实存在 | G2 M01 |
| S16 | Figure.getClientArea | 布局使用正确 child 坐标域 | LayoutContext::get_container_bounds | 缺陷 F05；node-local inset origin 再经 child transform 重复平移 | G2 M02 |
| S17 | minimum/preferred/hints | 内容受限测量后合成外框尺寸 | FigureTree size queries | 部分；带 layout 的盒模型、border preferred 与 hints 未统一 | G2 M02 |
| S18 | XYLayout | 无约束跳过，自动轴按固定轴 hint 测量 | XYLayout/XYConstraint | 部分；arrange 用无约束 hints，宽度敏感内容高度可错 | G2 M03 |
| S19 | StackLayout | children min/pref 最大值，全铺 client | StackLayout | 核心保留；受 S16/S17 影响，缺 visibility 配置；FillLayout 是独立本地策略 | G2 M04 |
| S20 | BorderLayout | 区域唯一，按实际占用递减剩余空间 | BorderLayout/BorderConstraint | 部分；固定厚度是本地策略；South/East 两套上限导致空带，F06 | G2 M05 |
| S21 | GridLayout/GridData | span、grab、对齐、等宽及收缩重测 | GridLayout/GridConstraint | 部分；equal+单列grab、负extra压缩、wrapping 重测不足 | G2 M06 |
| S22 | FlowLayout | 行换行、主副轴 hints、对齐/stretch | FlowLayout | 部分；换行成立，缺对齐/stretch，measure/arrange hints 不一致 | G2 M07 |
| S23 | ToolbarLayout | 副轴约束测量、主轴按 min 收缩 | ToolbarLayout | 部分；副轴 hints 缺失，非stretch可低于 minimum | G2 M08 |
| S24 | FreeformLayout | 负坐标、内容 extent 与位置 | FreeformLayout/FreeformLayer | 迁移；extent 不改 presentation bounds；positive-coordinates 后置 | G2 M09/M18 |
| S25 | Figure.revalidate/validate | 向上失效、向下布局，可再次入队 | validation queue + recursive layout | 迁移；显式预算/错误；预算不是实际栈安全或大批量收敛证明 | G2 M10 |
| S26 | LayoutManager.layout | 布局结果合法提交 | LayoutOutput buffered batch | 增强；结构预检存在，数值校验和任意扩展私有状态回滚不完整 | G2 M11 |
| S27 | DeferredUpdateManager.performUpdate | validation 后 repair、非重入 | DeferredUpdateManager + Runtime stabilize | 迁移；pending/in-flight/retry 契合保留式 GPU；Runtime fault 与独立manager重试不同 | G2 M12 |
| S28 | Figure.erase/repairDamage | 移动修复旧、新可见区域 | DamageSet/repair parent chain | 缺陷 F04；freeform 溢出 child 的旧 damage 被父 visual box 裁掉 | G2 M14 |
| S29 | DefaultRangeModel.setAll/setValue | clamp 与一致通知 | RangeModelSnapshot/RangeModel | 迁移；有限值验证、原子状态和锁外通知增强，不保证任意跨线程通知全序 | G2 M15 |
| S30 | ViewportLayout/tracks | 跟踪轴与非跟踪轴有不同 hints | ViewportFigure/ViewportLayout | 部分；固定内容主干成立，约束敏感内容可能改变滚动需求 | G2 M16 |
| S31 | IScalablePane/ZoomManager | scale→validate→scroll，保持锚点 | ScalablePane/ZoomManager/ZoomScrollPolicy | 迁移；内容单位 range 合理；外部 scalable 可写端口仍限内置类型 | G2 M17 |
| S32 | ScrollPaneLayout/FreeformViewport | 卷动条互相影响、负 extent | ScrollPane/FreeformViewport | 部分；固定 preferred 验证不覆盖完整受限测量求解 | G2 M18 |

## 几何、绘制、文本与资源

| 编号 | 官方 API | 核心语义 | Rust 对应 | 评估 / 差异 | 证据 |
|---|---|---|---|---|---|
| S33 | Rectangle/PrecisionRectangle | 点、尺寸、相交及外包围 | f64 geometry/ApproxEq/Affine | 迁移；包含边界与 Java 整数半开不同，Precision 别名非全部方法对等 | G3 M01/M02 |
| S34 | Graphics.push/pop/restoreState | restore 不弹栈，clip/style/transform恢复 | NdCanvas state stack | 保留已检查路径；命令固化样式，无须后端重复乘alpha | G3 M03/M07 |
| S35 | clipRect/setClip | 累计与替换裁剪，恢复历史 | clip stack + Vello clip_restore_plan | 矩形路径保留；getClip/path clip 后置 | G3 M04 |
| S36 | Figure.paint | self→children→border，状态隔离 | render_recursive.rs | 保留；统一 node-local 是迁移，不开放任意重写主遍历 | G3 M05/M06 |
| S37 | Shape/Ellipse/Polyline/Polygon | fill/outline、点列与精确命中 | FigureStyle + shape capabilities | 基础保留；整数像素修边及全部高级stroke不等价 | G3 M08–M10 |
| S38 | Graphics line style / path arc | 已暴露操作必须实际生效或显式拒绝 | LineStyle/Path/NdCanvas/Vello | 部分；部分dash/arc被静默忽略，不能用命令存在证明后端实现 | G3 §5 |
| S39 | CompoundBorder | 组合内外指标与绘制 | CompoundBorder/BorderSnapshot | 静态边框保留；动态TitleBar子边框丢失，F08 | G3 M11/M12 |
| S40 | ImageFigure.setImage | 换图/清空使尺寸和绘制失效 | ResourceRegistry/ImageFigure | 迁移；Ready资源删除后仍引用旧revision，F07 | G3 M13/M14 |
| S41 | Label text metrics | 测量、截断、绘制用同一字体事实 | TextLayoutEngine/Parley/Glyph IR | 迁移；外部引擎可构造布局；单行非完整TextFlow | G3 M15/M16 |
| S42 | Label.setTextPlacement | 文字相对图标方向 | TextPlacement/Label presentation | 缺陷 F09：North/South 反向；四轴API也窄于Java组合方向 | G3 M17 |
| S43 | Clickable/ButtonModel/ToggleModel | arm/press/selected/action顺序 | ClickableModel + Runtime input | 内置事务迁移；任意模型替换、ButtonGroup/repeat后置 | G3 M18/M19 |
| S44 | Accessibility / tooltip | 语义快照与平台展示分离 | AccessibleFigure/tooltip controller | 部分；已有引擎快照，不能据此认定平台桥接全交付 | G1/G3 M20 |

## Connection、Anchor、Router、Locator

| 编号 | 官方 API | 核心语义 | Rust 对应 | 评估 / 差异 | 证据 |
|---|---|---|---|---|---|
| S45 | Connection/PolylineConnection | 端点与路由驱动几何 | ConnectionId/ConnectionFigureBehavior | 迁移；Resolved/Unresolved明确，node-local path与child envelope分离 | G4 M01/M04 |
| S46 | ConnectionAnchor.getReferencePoint/getLocation | 对端reference求锚点，坐标与法向正确 | Anchor + tracked SceneQuery | 迁移；显式CoordinateSpace与逆转置normal，五类策略已存在 | G4 M02/M03/M07 |
| S47 | AnchorListener/ancestorMoved | 依赖变化触发重路由 | observations + reverse dependency index | 迁移；预检拒绝未并入恢复依赖，F10 | G4 M06 |
| S48 | ConnectionRouter | 策略替换、constraint及失效 | RouterId/typed erased constraint | 迁移；prepare/validate/commit已实现，非“缺少原子提交” | G4 M05/M09 |
| S49 | BendpointConnectionRouter/RelativeBendpoint | 有序点和端点相对offset | BendpointConstraint/Absolute/Relative | 固定域保留；Connection reparent未迁移/拒绝约束，F12 | G4 M10 |
| S50 | AutomaticRouter/FanRouter | 无向端点分组，稳定分离平行线 | FanRouter/RoutingDomain | 分组保留；反向两线法向与index双翻转造成重合，F11 | G4 M12 |
| S51 | ManhattanConnectionRouter | 正交方向和共享lane | ManhattanRouter | 部分；批次共享scope迁移合理，全部outward normal分支未证等价 | G4 M13 |
| S52 | ConnectionLayer router | 默认策略与显式覆盖 | inherited/explicit router binding | 迁移；移出layer后归属变化证据不足 | G4 M11 |
| S53 | ConnectionLocator/MidpointLocator | 中央点/段、端点与child定位 | Locator + Runtime binding | 已接入；保留现有child尺寸是收窄，PathFraction独立命名 | G4 M14 |
| S54 | ArrowLocator/EndpointLocator | 位置、reference、方向及u/v | LocatorPlacement | 部分；reference未形成Runtime旋转装饰闭环 | G4 M15 |
| S55 | viewport-aware connection clipping | 端点容器可见域参与连接裁剪 | viewport-chain admission | 收窄；同chain允许、分叉显式拒绝，多矩形clip后置 | G4 M16 |
| S56 | NodeEditPart anchor hooks | 模型与请求决定anchor | AnchorDescriptor/AnchorSemanticKey | 稳定端点扩展已实现，不能重报“固定Chopbox” | G4 M17 |

## Model、编辑事务与开放扩展

| 编号 | 官方 API | 核心语义 | Rust 对应 | 评估 / 差异 | 证据 |
|---|---|---|---|---|---|
| S57 | application model / listener | 模型事实源、通知驱动投影 | ModelAdapter/ModelEvent/Revision | 迁移；集中drain和revision校验，稳定持久ID仍属应用责任 | G5 M01/M02 |
| S58 | Command/CompoundCommand | 正序执行、逆序undo、明确拒绝 | Command/CompoundCommand | 迁移并增强；补偿失败与未知状态fault，不承诺任意外部副作用回滚 | G5 M03–M05 |
| S59 | CommandStack/save location | 新分支清redo，dirty区分保存身份 | history identity/limit/Drop/journal | 迁移；committed journal窄于Java PRE/POST，limit=0语义不同 | G5 M06–M08 |
| S60 | EditPart/factory/registry | 模型、controller、Figure身份分离 | PartTree/Behavior/Factory/双registry | 迁移；undo重建运行时身份；root只在构造时绑定 | G5 M09–M13 |
| S61 | activate/deactivate | 外部订阅成对释放 | create_subtree/Viewer::drop | 原快照缺陷 F13：初始创建失败时contents尚未提交，清理提前返回 | G5 M11 |
| S62 | createFigure/getContentPane/refreshVisuals | compound内部视图可更新 | VisualBuildContext/VisualUpdateContext | 部分；可构造pane，但更新API主要限primary bounds/style | G5 M12/§4 |
| S63 | refreshChildren/connections | 复用、增删、重排及关系去重 | ModelSnapshot + reconciliation | 行为主干迁移；未变顺序仍逐边全局扫描，F14 | G5 M14–M19 |
| S64 | extension failure | 错误后不能继续使用半提交视图 | Viewer fault/CommandStack fault | stack已增强；Viewer回调panic漏fault，F15（项目额外保证） | G5 M20 |
| S65 | EditDomain/Tool/DragTracker | 外部工具可安装，gesture固定source | EditorDomain/四类内置Tool | 部分；内置闭环成立，Tool/Tracker不可外部注入 | G6 M01–M03 |
| S66 | Request typed subclasses | 开放意图及强类型payload | EditorRequest enum/typed structs | 强类型合理；封闭六类请求丢失外部意图扩展 | G6 M04/M05 |
| S67 | EditPolicy.getTargetEditPart | 返回操作目标身份 | EditPolicy::target | 原快照缺陷 F16：只判is_some，仍调用source policy | G6 M07 |
| S68 | EditPolicy.getCommand | 确定顺序组合、拒绝区别于无贡献 | PolicyRole/BTreeMap/CompoundCommand | 核心保留；Custom role开放，live replace/remove不完整 | G6 M06/M08 |
| S69 | Tool feedback/commit/cancel | 临时反馈先清理，命令只改模型 | FeedbackVisual/VisualOwner/Domain | 正常路径保留；异常cleanup与keyboard仲裁仍待补证 | G6 M09–M11/M15 |
| S70 | create/reconnect/bendpoint | 锁定source/endpoint/index，末端目标更新 | ConnectionCreation/Reconnection plan | 迁移；真正显式折点支持self-loop，非路由模拟业务模型 | G6 M12–M14 |
| S71 | AutoexposeHelper | 边缘step、viewport变化重算request | host elapsed + autoexpose_tick | 迁移；根viewport-only，Native人工检查点C待验 | G6 M16/M17 |
| S72 | Selection/RootEditPart扩展 | 应用可定制规则及root组合 | SelectionModel/固定root helper | 部分；设计列出SelectionPolicy/RootLayerFactory但缺公开注入出口 | G6 M18 |
| S73 | document persistence / editor integration | 保存后重建同一模型事实 | serializer + G6 roadmap | 后置/尚待实现；save location不是serializer，Web/Core构建也不是Web Editor闭环 | G5/G6 |

## 对 Rust API 的总体评价

值得保留的是分域 generational ID、值类型几何、关联类型模型、短借用上下文、
typed constraint、私有字段构造校验、纯计算输出和统一 Runtime mutation。
这些设计减少 Java 对象别名、宽继承层次、隐式可变缓存和平台依赖。

尚需补全的是“外部实现者实际能否替换”：开放 trait 必须有完整输入上下文、
输出构造入口、注册/运行期调用链及错误清理。当前 LayoutManager 缺部分测量上下文，
VisualUpdateContext 缺内部visual更新，动态Border快照特化内置类型；
Tool/Request/Tracker 则连注入入口都未闭合。这些不能用增加空trait或改名解决。

算法层面应区分正确性和成本：常见布局排列大体 O(n)，投影仍存在 O(E²) 或
O(Σd²) 路径；tracked依赖和局部索引并不自动保证整条更新链线性。
深度限制10,000是协议上限，不是原生栈与GPU性能实测结论。
具体修正顺序与外部扩展验收用例见
[迁移实施方案](draw2d-gef-migration-plan-2026-09-16.md)。
