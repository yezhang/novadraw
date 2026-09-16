# Group 1：Figure、身份、生命周期、查询与输入

类型：`verification`

基线与范围见 `../baseline.json`。本组由主审执行。
实际读取：`graph/search.rs` 全文、`runtime/event/mod.rs` 全文、
`identity.rs`、`runtime/interaction.rs`、`runtime/focus.rs` 全文，
`graph/mod.rs` 的拓扑准入/生命周期方法、`runtime/runtime.rs` 的
dispose/reparent/状态修改方法、`runtime/context.rs` 的事件点转换与投递，
`runtime/update/listener.rs` 的通知类型；相关 M6、D3、D4 契约与平台调用入口。

## 外部基线

官方 `org.eclipse.draw2d.doc.isv/guide-src/{overview,hittest,coordinates,layout,painting}.adoc`
规定轻量有序树、paint/hit 同坐标与裁剪、事件由 dispatcher 定向分发。
`Figure.java:391-414,437-452,476-520` 分别说明普通查找只过滤 visible，而事件查找另过滤
enabled 与接收资格。`SWTEventDispatcher.java:418-444` 用 mouseTarget 变化触发
exit/enter；capture 期间保持 target。`MouseEvent.java` 在构造时对接收 Figure 转换坐标。
`Figure.add/remove/addNotify/removeNotify` 的对象存活与 `getBounds` 可变引用语义，
不能直接等同 Rust arena 销毁。

## 语义映射

| Java 类/方法 | 必须保留的语义 | Rust 对应 | 判断与证据边界 |
|---|---|---|---|
| `IFigure.add/getChildren/getParent` | 单父、有序组合，绘制与命中共享顺序 | FigureTree、FigureNode、FigureTreeBuilder | 核心结构保留；Vec children 与反向 parent，由准入统一维护 |
| `add(child,index)` / reorder | 顺序可改变 | Runtime::move_child_to_index/bring_child_to_front/send_child_to_back | 重排已实现；原子 indexed add 仍收窄 |
| `setParent` / reparent | 旧新父关系同步维护 | Runtime::try_reparent | 同域身份保持、先校验；Layered parent 要专门 key 协议，合理强化 |
| Java 对象身份 | 对象引用不能错指另一图对象 | RuntimeNamespace + SlotMap generation | 拒绝 foreign/stale，移除全局分配器；不能据此声称持久模型身份 |
| `remove` vs `removeNotify` | 从父树移除不必销毁活对象 | Runtime::remove_figure/dispose_subtree | **明确收窄**：旧句柄退休；由 ADR-014 接受，无通用同域保活 detach |
| `addNotify/removeNotify` | 必需组件激活/停用、子树传播 | complete_attachment、RetiredSubtree::complete | parent-first attach、descendant-first release；提交后只读旧关系，非 Java 原调用时机 |
| `get/setBounds` | 几何影响 paint/layout/hit/damage | NodeState、Runtime::set_bounds/translate | Runtime 唯一运行期写入口；独立 Figure 的 Bounded 不代表树内真源 |
| `findFigureAt/TreeSearch` | 逆 Z-order、prune 子树、accept 候选 | hit_test_with、ExclusionSearch | 顺序/策略保留；**visible disabled 被提前剪枝**，与官方普通几何查询不等价 |
| `findMouseEventTargetAt` | 另加输入资格，非纯几何命中 | MouseEventTargetSearch::accept | handler.wants_mouse_events 检查存在；复用遍历合理，但 enabled 不应强加所有查询 |
| `MouseEvent` 坐标转换 | 按 receiver 域投递 | entry_point + with_target_point、SceneDispatchContext | entry 坐标只读保留；point reduction 集中引擎，合理迁移 |
| `SWTEventDispatcher.receive` | target 变化生成 enter/exit | EventDispatcher::refresh_mouse_target | **确认缺陷 F-EVENT**：使用 hover_source 生成事件，非实际 mouse target |
| `dispatchMouseHover` | hover 回调交 mouseTarget，tooltip 独立 | dispatch_mouse_hover | 同根因：发给 hover_source，非交互父级 |
| `setCapture/releaseCapture` | 拖动锁定接收者、结束释放 | handled press 自动 capture、release 清理 | 单主指针路径存在；移出窗口保留 capture 为不同平台策略，应配 cancel 验证 |
| `MouseEvent.stateMask` / drag | 鼠标按钮、modifier 随事件可读 | MouseEvent 只有 button；Key/Wheel 有 modifiers | **收窄**：Figure 鼠标回调无 modifier 集合，Moved/Dragged 由 capture 推断 |
| `requestFocus/FocusTraverseManager` | 单一 focus、顺序遍历、失效清理 | Runtime::request_focus/traverse_focus、FocusTraversalPolicy | 可替换 policy，返回节点二次校验；先提交 owner 再 lost→gained 是显式变体 |
| Figure/Ancestor/Coordinate/Property listeners | 各类因果事实分别通知 | typed events、scoped ListenerId、Observation journal | 观察语义保留；延迟 latest stable query 非事件时历史快照 |
| `LayoutListener.layout` 等同步 hook | 可介入布局，不仅事后观察 | LayoutListener 观察类型 | **收窄**：journal 不能替代同步 layout/veto；由 ADR-014 分层但需另保留扩展用例 |
| `LightweightSystem` | 平台事件适配和图形语义隔离 | novadraw-apps input → Runtime | engine 做命中和点降域；Editor 有独立键盘适配，后续统一规范输入 |

## F-EVENT：mouseTarget 与 hover source 混用

- 严重度：P1；置信度 10/10。
- 位置：`novadraw-scene/src/runtime/event/mod.rs:377-410`，及 `dispatch_mouse_hover`。
- 触发：交互父节点 P 下放置无 handler 子图形 C，指针直接移入 C。
- 期望：P 是 mouseTarget，因此 P 收到 Entered、Moved；随后 C→P 空白区只收到 Moved。
- 实际：第一次 P 仅收到 Moved；第二次 mouseTarget 未变却新增 Entered。
- 原因：`SceneDispatchContext::find_hover_source_at` 使用普通几何命中得到 C；
  refresh 在计算 captured.or(hit_target) 之前对 hover source 派发 enter/exit。
- 当前公共 API 重编译复现：`../event-probe.log`，源和链接库哈希见 `../event-probe.json`。
  这是本次新运行，不沿用旧二进制结果。
- 修复方向：mouseTarget transition 专管 enter/exit 与 MouseHover；
  物理 hover、tooltip、widget drag-out/back 的视觉状态独立更新。
  capture 下 P 不应因指针经过 Q 就改变事件接收语义。
- 测试缺口：现有 M6 probe 主要使 handler 与几何 hit 重合；其包含式事件断言未覆盖
  “交互父 + 非交互子”的完整有序事件序列。

## 明确收窄 S-SEARCH

`graph/search.rs:238-240` 对所有搜索应用 `!node.is_enabled`，普通 hit 返回父而非可见子。
复现日志确认。该行为符合 `design/architecture/tree-search-and-focus.md` 当前批准设计，
因此应先修订该设计与 parity，再改 shared traversal 策略；不把它描述为偶然实现偏差。
Rust 并不要求这一限制，也不是保证输入安全所必需。

## Rust / 算法评价

身份与受控写入边界合理；`&self` 不证明扩展无 interior mutation，panic/fault
仍属于运行期契约。通用搜索最坏 O(N)，路径/祖先转换依树高变化；没有性能测量不宣称
比 Java 更快。默认 focus 每次构建 descendants Vec 是明确 O(N) 空间/时间取舍。
公共 API 同时有 bool convenience 与 Result 入口，推荐未来用 Result 区分 unchanged、
foreign、fault；现有 bool 返回不能提供完整诊断。

本组未修改产品代码；其他分组与统一验证日志负责补充全局结论。
