# Group 1 增补：生命周期、输入与通知语义

类型：`verification`

范围：当前工作区完整函数语义审计，非 diff-only。读取时 HEAD 为
`e8d54ac063d05c63abb9f2850365a837ec4e8c9d`，工作区包含既有未提交修改。
本报告增补主审的 `group_1.md/jsonl`，不替换其结论或复现证据。

## 1. 新增发现

新增两项 P1；JSONL 只收录这两项。主审已有的 F-EVENT 不重复登记。
本执行者没有运行 cargo、测试、编译、event probe 或平台 UI；以下均为当前源码的
静态证据和明确输入序列推演，不把主审的运行结果算作本次执行结果。

### S1：release 构建跳过 listener scope 写入，删除 owner 后订阅仍存活

- 严重度：P1，条件性功能缺陷；置信度：10/10；类别：业务语义问题。
- 主位置：[runtime.rs:2485-2488](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-scene/src/runtime/runtime.rs#L2485-L2488)。
- 相同根因覆盖八类 `add_*_listener_scoped`，位于 `runtime/runtime.rs:2480-2606`，
  包括 update、figure、coordinate、ancestor、property、action、layout、observation。

关键代码：

```rust
self.validate_listener_scope(scope)?;
let id = self.updates.add_listener(listener);
debug_assert!(self.updates.set_listener_scope(id, scope));
Ok(id)
```

触发序列：

1. 使用 `debug_assertions = false` 的构建，例如未覆写该选项的 release profile。
2. 注册 `add_update_listener_scoped(ListenerScope::Figure(owner), listener)`。
3. 调用 `dispose_subtree(owner)`，随后继续修改其他节点并发布通知。

`debug_assert!` 关闭时，其参数表达式不执行。注册成功，但
`UpdateManager::listener_owners` 没有该订阅的 owner：
`runtime/update/deferred.rs:103-123` 是写入 owner 的方法，
`:125-147` 的退休逻辑只按这张表匹配。`Runtime::dispose_subtree`
在 `runtime.rs:1503` 调用了正确的退休入口，却无法找到这条订阅。
因此 owner 已销毁，listener 仍注册、捕获的资源仍保活，并能继续接收同类通知。
scope 是生命周期归属，不是 payload 过滤；这里不是过滤条件差异。

当前仓库的 `Cargo.toml` 和 `.cargo/config.toml` 未设置 release
`debug-assertions` 覆写。本结论的必要条件仍明确是关闭 debug assertions，
不假设所有外部构建配置相同。

契约依据：ADR-014 决策 3、`figure-lifecycle.md` 第 4 节要求删除 Figure
自动解除 owner-scoped 订阅，Runtime-scoped 订阅保留。
现有 `runtime.rs:5173-5195`
`disposal_removes_owned_listener_but_preserves_shared_registrations`
包含 `assert!(!runtime.remove_listener(owned))`，但普通 debug 执行不会暴露此问题；
本次没有执行它。

建议：把 scope 写入作为无条件业务语句执行，再单独断言其结果，或让注册入口一次性
接收 scope。后续应验证关闭 debug assertions 时八类订阅均停止回调、释放一次，
且 Runtime-scoped 订阅不受影响。此处没有实施修复或新增测试。

### S2：Native 离窗后丢弃 release，同时保留 capture，重新进入会继续拖拽

- 严重度：P1，条件性功能缺陷；置信度：9/10；类别：逻辑错误。
- 主位置：[app.rs:488-495](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-apps/src/app.rs#L488-L495)。
- 引擎协同位置：`runtime/event/mod.rs:498-533`、
  `runtime/runtime.rs:3173-3183`、`runtime/interaction.rs:226-246`。

触发序列：

1. 指针位于可处理 press 的 Figure P 内，左键按下；handled press 设置 capture=P、
   pressed=P。
2. Native 收到 `CursorLeft`：`app.rs:489` 把 `cursor_position` 清为 `None`，
   随后调用 `Runtime::pointer_exited`。
3. 在外部松开按钮。若 Native 收到 Released，`app.rs:493-495` 因无坐标直接返回；
   若平台没有投递窗外 release，本路径同样没有结束通知。
4. 不再次按键，重新进入窗口并收到 `CursorMoved`。

引擎 `dispatch_pointer_exited` 只清理 hover/cursor，最后执行
`ctx.set_mouse_target(ctx.captured())`，没有清 capture 或 pressed。
`dispatch_mouse_moved` 又只按 capture 是否存在选择 `Dragged`，不检查实际按钮状态。
P 仍 attached/visible/enabled，`reconcile_non_focus` 不会删除这个有效 ID，
所以第 4 步将移动继续发给 P，并报告 Dragged。

`Focused(false)` 也只取消 gesture、调用 pointer_exited 和 release_focus
（`app.rs:590-596`），不能补上 pointer capture/pressed 的终止。
标准 `PlatformHost`/`WinitPlatformHost` 在已读接口中没有完成这项补偿。

这不是仅因“离窗保留 capture 不同于 SWT”而报错：跨窗口拖动可以是合理策略，
但必须可靠收到最终 release，或明确 cancel。当前 app 丢弃 release 与引擎保留
capture 的组合形成了不闭合状态机。Draw2D
`SWTEventDispatcher.dispatchMouseExited` 会 releaseCapture，说明它的离窗契约不同。

证据边界：以上事件序列下的代码结果可以静态确定；本次未在 macOS 窗口系统复现，
不声称所有平台必定产生完全相同的离窗事件顺序。

建议：明确平台 capture/cancel 协议。若不支持窗外拖动，在离窗或失焦时通过引擎
cancel 同时清理 capture、pressed，并按组件契约结束交互；若支持，确保平台捕获和
窗外 release 可达，不因 `cursor_position=None` 丢弃已捕获指针的结束事件。
后续覆盖 press → exit → release → reenter、press → blur → reenter 两条序列。

## 2. 基线与实际阅读范围

先读取 AGENTS、CLAUDE、reviewer brief、Group 1 清单和项目记忆；随后读取
`analyzing-gef-code`、`bits-code-guard` 及通用工作流、七维检查和分级规则。
按本组委派契约只交付本组增补文件，不生成汇总报告、不提交 Git。

官方证据路径缩写：

- `J/`：`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/`。
- `G/`：`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d.doc.isv/guide-src/`。
- 完整阅读 `G/hittest.adoc`、`G/coordinates.adoc`，
  `J/SWTEventDispatcher.java`、`J/MouseEvent.java`、`J/FocusTraverseManager.java`、
  `J/TreeSearch.java`、Figure/Coordinate/Ancestor/Focus/Mouse/MouseMotion/KeyListener Javadoc。
- 方法级阅读 `J/Figure.java` 的 add/remove、addNotify/removeNotify、普通搜索、
  mouse target 搜索、输入回调、focus、enabled/visible、通知方法；相应
  `J/IFigure.java:72-277` 的公开契约。未读取 Zest，也未使用第三方示例替代官方契约。

本项目阅读分层：

- 完整阅读 `graph/search.rs`、`identity.rs`、`runtime/{event/mod,interaction,focus,context,mutation/mod}.rs`；
  `graph/mod.rs` 与 `runtime/runtime.rs` 按拓扑准入、reparent、dispose、activation、
  focus、dispatch、scope 注册、interaction 清理、坐标转换方法核对。
- 通知直接依赖：`runtime/update/listener.rs` 及 `update/deferred.rs` 的
  scope 注册/退休、typed dispatch、flush；样式/平台依赖：
  `style.rs`、`host/{mod,scene_host}.rs`、`novadraw-apps/src/{input,platform}.rs`，
  `app.rs` 的输入/失焦/场景切换路径。`tooltip.rs`、`accessibility.rs` 按
  hover source、销毁后快照与 focus 投影方法阅读，不承担其完整产品审计。
- 完整阅读 D1 search/focus、M6 event、P2 dispatch outcome、D3 listener、
  D4 notification epoch、M2 existence 契约；方法级核对 D3 mutation、M4 coordinate
  及 Runtime 内联身份、10,000 层 dispose、scope、panic、focus 测试。
- 公共导出和场景入口：`novadraw/src/lib.rs`、`novadraw-scene/src/{lib,log}.rs`、
  `runtime/mod.rs`、`novadraw-apps/src/{lib,prelude,verification}.rs`、
  `novadraw-demo-scenes/src/{lib,event,focus}.rs`；其余清单内 demo、
  D4 component/measurement、resize 和 bounds 测试仅作入口/相关断言索引检查，
  不宣称逐行覆盖了绘制、布局、文本、连接等其他组职责。

## 3. 增补语义映射

下表 Rust 路径以 `novadraw-scene/src/` 为基准，测试路径以 `novadraw-scene/tests/`
为基准。状态“保留”表示已读实现支持该契约，不表示本次测试通过。

| ID | Java 类/方法与基线语义 | Rust API 与实现证据 | 状态与局限 |
|---|---|---|---|
| M01 | `Figure.add`、`IFigure.add(child,constraint,index)`：单父、有序 child、禁止环，已挂父则移除旧关系 | `FigureTreeBuilder`、`Runtime::try_add_figure/try_reparent`；`graph/mod.rs:1072-1100,1263-1317,1407-1452` | 单父/无环保留；新建 add 接收 owned Figure，移动既有节点走 reparent。indexed add+constraint 并非单个等价入口，不能仅凭 add 名称标全覆盖。 |
| M02 | `Figure.getChildren` 与 `findDescendantAtExcluding`：后绘制的 child 优先命中 | `move_child_to_index/bring_child_to_front/send_child_to_back`；`graph/search.rs:264-274` 逆序 child | 保留；D3 mutation 的 `runtime_child_order_controls_paint_and_reverse_hit_order_atomically` 检查重排与非法 index 不改变顺序。未运行。 |
| M03 | Java Figure 对象引用表示活对象，不等于持久业务身份 | `RuntimeNamespace` + SlotMap generation；`identity.rs:15-156`；Runtime 沿用 tree namespace | 合理迁移。foreign/stale 不能命中同 local slot；整体 move Runtime 保持 registry 身份。不能据此承诺序列化稳定 ID 或跨 Runtime 活对象迁移。 |
| M04 | `Figure.addNotify:322-328` parent-first realization；`removeNotify:1540-1546` descendant-first deactivation | `Runtime::activate_attached_figures`、`attached_ids_parent_first`、`RetiredSubtree::complete`；`graph/mod.rs:620-634,734-749,764-801` | 创建/销毁顺序保留，dispose 先退休身份再以旧关系值回调，是 ADR-014 迁移。reparent 仅对移动子树根调用 completion，不应把该路径写成 Java 递归 removeNotify/addNotify 的完全等价。 |
| M05 | `Figure.remove:1408-1426` 是解父关系，调用者仍可保留对象 | `Runtime::remove_figure` 委托 `dispose_subtree`；`runtime/runtime.rs:1332-1358,1477-1532` | 明确收窄，ADR-014 接受。退休 arena/UUID/interaction/关系/资源依赖后释放；通用保活 detach、owned extraction 不支持。Runtime drop 是 Rust 所有权释放，不据此承诺逐 Figure removeNotify 回调。 |
| M06 | `Figure.findFigureAt:437-452`、descendant search `:391-414`：普通几何搜索不检查 enabled | `hit_test/hit_test_simple/hit_test_with`；`graph/search.rs:237-241` 对 hidden **或 disabled** 剪掉整棵子树 | **明确收窄，不是偶然 bug**。批准规范 `tree-search-and-focus.md` 第 2、3.1 节明确规定 enabled 为所有几何命中硬门禁，策略不能恢复 disabled 候选。主审的 S-SEARCH 分类应保留。 |
| M07 | `Figure.findMouseEventTargetAt:476-520`：搜索 visible/enabled 子树，候选需鼠标接收资格；普通 Figure 的资格来自 Mouse/MouseMotion listeners | `MouseEventTargetSearch::accept`、`find_mouse_event_target_at`；`graph/search.rs:95-103,136-139` 检查 `event_handler().wants_mouse_events()` | 输入过滤保留，和普通命中的实际差异主要是 handler 接受条件，而不是 enabled。FigureEventHandler 是单能力端口，不等于 Java 任意数量 listener 注册表；默认 wants=true。 |
| M08 | `TreeSearch.prune/accept` 与官方 hittest：prune 排除子树，accept 只接受当前候选，子节点先于父节点接受 | `TreeSearchContext`、`ExclusionSearch`、`hit_test_from_with_inner` | 保留；完整遍历为 enabled/visible → inverse node transform → precise geometry/overflow → prune → child clip/transform → reverse children → accept。D1 search 与内联测试验证排除和调用顺序，未运行。 |
| M09 | `IFigure.getParent/getChildren` 可组合成结构查询，不以点命中资格为准 | `ancestor_ids/descendant_ids/is_ancestor_of/find_in_subtree`；`graph/search.rs:142-208` | 合理扩展；结构查询不执行几何或 enabled/visible 门禁，仍可找到 disabled 节点。ancestor 隐藏 synthetic root，descendant 稳定前序；UnknownFigure 与无匹配区分。 |
| M10 | 官方 coordinates、`MouseEvent` 构造：从 Canvas absolute 转到 source 所处坐标域，不是 source 定义的 children 坐标系 | `MouseEvent::entry_point/with_target_point`、`SceneDispatchContext::dispatch_to_target`、`translate_to_relative`；`runtime/context.rs:578-651`、`graph/mod.rs:3985-4010` | 合理迁移为固定 node-local 回调，不能称 Java 默认 inherited 坐标数值完全一致。M4 测试 `:161-211` 验证含嵌套 inset 的入口点 `(161,125)` 降为 `(15,20)`。 |
| M11 | `SWTEventDispatcher.receive`：mouseTarget 变化触发 exit/enter，cursorTarget 由普通命中得到，hoverSource 用于 tooltip 祖先解析 | `refresh_mouse_target`、`find_cursor_target_at/find_hover_source_at`；`runtime/event/mod.rs:375-411`、`runtime/context.rs:441-450` | 主审 F-EVENT 静态复核一致：两种几何查询都返回 hit_simple；exit/enter 实际基于 hover_source，而 mouse_target 最后才赋 captured.or(hit)。不重复计为新增缺陷。 |
| M12 | `dispatchMouseHover` 发给 mouseTarget；tooltip 的 Figure 来源另行求取 | `dispatch_mouse_hover` 发给 `ctx.hover_source()`；`Runtime::sync_tooltip` 再调用 `tree.tooltip_source` | F-EVENT 的同根因表现。Rust interaction.hover_source 实际是几何 hit，不一定是有 tooltip 的 owner；真正 tooltip source 是向祖先搜索并尊重 `Some(None)` 抑制后的 Figure。 |
| M13 | SWT consumed press 捕获，release 释放并重查；capture 时 cursor/hover 不更新；离窗释放 capture | Rust handled press 捕获；move/up 主事件投 captured，release 清 pressed/capture；`event/mod.rs:459-533` | 部分迁移：capture 时几何 cursor/hover 仍更新，tooltip 被 `sync_tooltip` 抑制。pointer_exited 保留 capture；Native release 丢失导致 S2。不可把“主事件目标固定”等同“所有 hover 行为固定”。 |
| M14 | SWT drag 根据 BUTTON_MASK；MouseEvent 经 InputEvent 携带 stateMask | `MouseEvent` 只有 kind、x/y、button、entry_point；`dispatch_mouse_moved` 根据 capture 决定 Dragged | 当前能力收窄：没有鼠标 modifier/button-set/pointer-id payload，未 handled 的按键移动仍是 Moved。PointerId 的查询类型已存在，但分发写入只走 PRIMARY，不代表多指针已交付。 |
| M15 | `Figure.requestFocus` 与 `FocusTraverseManager`：direct 和 traversable 分离；Tab 前序，边界交平台；SWT 记住失焦 owner | `Runtime::request_focus/traverse_focus`、`TreeOrderFocusTraversal`；`runtime/focus.rs:66-104`、`runtime/runtime.rs:3096-3164` | 保留单 owner、顺序、策略后二次资格校验；强化 visible/enabled 检查。backward 无 current 取最后一个为批准变体；默认只遍历 contents descendants。未发现当前 Native 自动恢复失焦 owner 的等价入口，不能宣称覆盖 SWT focus restoration。 |
| M16 | `SWTEventDispatcher.setFocus` 先更新 owner 再 lost/gained；`removeNotify` 清相关引用 | `EventDispatcher::update_focus`；`retain_interactive_figures`；dispose 的 `retired_focus_lost` | owner-before-callback 与 lost→gained 保留；hide/disable/reparent 失格清 focus，dispose 先失效 ID 再用 retired node 投 lost。Runtime 内联相关测试已读，未运行。 |
| M17 | Figure/Coordinate/Ancestor/Property listeners：bounds、子坐标系、parent-chain、property old/new 各有独立语义 | typed `NotificationEffect`、Runtime 注册、`StableSceneQuery`、`update/deferred.rs:255-365` | ADR-014 合理迁移为延迟观察，不是同步 hook。scope 只决定订阅寿命，payload/source 仍需过滤；AncestorEvent 是 Runtime 关系事实，不是自动绑定某个叶子全部祖先的 Java AncestorHelper。scope 清理另有 S1。 |
| M18 | `Figure.fireFigureMoved/fireCoordinateSystemChanged/firePropertyChange` 分离不同因果通知 | `FigureEvent`、`PropertyChangeEvent`、`NotificationRecord { source_epoch, sequence }` | 类型分层保留；epoch 在 flush 时标记稳定发布，同一批多个源修改共享 epoch，old/new 与 sequence 承载历史。D4 notification epoch 检查两次 bounds 变更及 latest query，不能说每条 record 自带历史场景快照。 |
| M19 | LightweightSystem/SWTEventDispatcher 平台桥接，不让 app 实现 Figure 搜索/坐标语义 | `PlatformHost`、Winit/Web adapters、Runtime、SceneDispatchContext；`novadraw-apps/src/input.rs:47-72` | 引擎归属保留；Native physical/DPI，Web client-canvas CSS point，Tab 单独 traversal。SceneSpec 工厂交付完整 Runtime，避免丢 registry。双击/hover 引擎入口存在，不等于每个平台都已生成相应事件。 |

## 4. 普通命中与 target/hover/capture 的结论

| 项目 | Draw2D 实际语义 | Novadraw 当前实际语义 |
|---|---|---|
| visible=false | root 发起的搜索不进入 invisible child | 几何与事件搜索均剪掉节点和子树 |
| enabled=false | 普通 findFigureAt 仍可返回；mouse target 搜索剪掉 disabled child 子树 | 普通 hit_test 也剪枝；cursor、hover、gesture 共享此限制。是批准收窄 |
| 非交互 child 位于交互 parent 内 | 普通 hit 是 child，mouseTarget 可为 parent，enter/exit/hover 回调跟 parent | hit/cursor/hover_source 是 child，mouse_target 是 parent；主审 F-EVENT 所述漏发/多发成立 |
| capture 下跨到另一 Figure | mouseTarget 固定，普通 cursor/tooltip 源更新也暂停 | move/up 发 captured，cursor/hover 继续追几何 hit；tooltip 暂停；Hovered 集合不等于 captured 集合 |
| tooltip source | 从 cursorTarget 向祖先求 tooltip Figure | 从 interaction.hover_source 向祖先求字符串 owner；支持显式抑制；不等同 Hover 事件接收者 |
| pointer exit | SWT 清 mouseTarget 并 releaseCapture | 清 cursor/hover，mouse_target 仍为 captured；Native 结束事件缺口见 S2 |
| hide/disable/dispose | 具体 Java flag 与 removeNotify 各司其职 | Runtime reconcile 清失格 target/cursor/capture/pressed/gesture，focus 经专门 Lost 路径清理；有效节点的失焦/离窗不会因此自动取消 capture |

## 5. 迁移、收窄与后置

- 合理迁移：namespaced arena 身份、Runtime 整体所有权、node-local 点降域、
  只读策略、owned deferred mutation、提交后生命周期 completion 与稳定观察日志。
- 明确批准的收窄：所有几何命中都过滤 enabled；remove 是 dispose；
  无通用活对象 detach/跨 Runtime 自动迁移；focus 资格更严格；
  backward 无 current 对称进入。它们不能伪装成与 Java 完全等价，也不应仅因差异报 bug。
- 当前能力边界：单 primary pointer、无鼠标 stateMask、无普通事件冒泡、
  无 Java 多 listener 形式的输入注册表；必要组合由 FigureEventHandler 实现。
- 后置能力：通用保活 unmount/extraction/cross-runtime rebuild 协议、多 pointer
  分发与 cancel、完整原生 AT provider。接口类型存在、文档设计存在都不等于已实现。
- 文档口径需要主审保留：parity `api-coverage.md:270` 写 enter/exit 跟 mouseTarget，
  但实现跟 hover_source；`:593-595` 的 disabled 差异 probe 建议与批准后的统一
  enabled 门禁不是同一口径。本轮仅指出，不修改既有文档。

## 6. 扩展性、复杂度与七维判断

外部扩展点已落实为 Figure 的可选 capability、TreeSearch、FocusTraversalPolicy、
FigureComponentUpdate 和 typed listeners。策略获只读视图，Runtime 校验候选；
`&self` 仍允许 interior mutability，扩展纯度与 Drop 不 panic 不能靠签名证明。
常规 Figure handler 返回 bool 表示 handled，没有 DOM 冒泡或 Java consumed-listener
链的隐式承诺。建议使用 Result 入口，避免 bool/null convenience 混淆无变化与失败。

设 N 为节点数、H 为树高、S 为待处理子树大小、L 为订阅数、E 为通知数：

- 命中最坏 O(N + H)（含返回 path），工作栈随 H；一次 refresh 执行 target、
  cursor、hover 三次命中，仍为 O(N)，没有空间索引。递归有 stacker 增长，
  树深由准入限制至 10,000；未改渲染主循环。
- 结构 descendants 为 O(S) 时间/空间，ancestor 为 O(H)。
  默认 focus 先分配 O(N) descendants，每个候选再次检查 attachment 和父链 flag，
  最坏 O(NH)，不能简单称整次 focus 为 O(N)；10,000 层性能本次未测量。
- dispose 枚举/提取 O(S)，但冻结每个节点的旧 damage 需沿父链投影，最坏 O(SH)；
  全表退休 scoped listeners 为 O(L)。不能把 arena 删除 O(S) 当作整体 dispose 上界。
- scope 注册目前查找 listener 需要 O(L)，连续注册可累计 O(L²)；
  notification dispatch 按事件访问对应 listeners，常见上界 O(EL)。
  这里是代码复杂度评价，不在没有 workload/测量时另报性能缺陷。

| 维度 | 本组判断 |
|---|---|
| 逻辑 | S2 给出具体事件序列；F-EVENT 与主审去重 |
| 业务语义 | S1 违背 owner-scoped 生命周期；普通命中 enabled 属批准收窄 |
| 并发 | Runtime 入口以独占可变借用串行推进；未发现可证实的数据竞争，不以此保证任意第三方 interior mutation 安全 |
| 健壮性 | namespace/generation、拓扑/深度、faulted 与 deferred error 路径已有实现；panic 不承诺副作用回滚 |
| 性能 | 上述复杂度与重复遍历已明确；未跑 benchmark，不下吞吐/延迟结论 |
| 安全 | 已读路径未发现可证实的安全漏洞；未进行独立安全扫描 |
| 质量 | scope 副作用不是风格问题；过时注释/口径不一致记录为证据局限，不单列缺陷 |

## 7. 验证交接

主审可优先验证 S1 的关闭 debug assertions 配置，以及 S2 的两条输入序列；
已有 F-EVENT probe 不需本组重复运行。补充验证应区分普通几何命中与事件接受策略，
并覆盖非交互 child、capture 跨节点、ancestor tooltip、hide/disable/dispose。

本执行者只写 `group_1_supplement.md/jsonl`，未改产品、测试、主审报告或其他组报告，
未运行 cargo，未声称任何新增测试通过。
