# Group 6：Editor 交互、请求与扩展端口

类型：`verification`

源码基线、范围与验证命令见上级 `baseline.json` / `checks.json`。
以下路径相对 Novadraw 项目根；Java 路径相对
`gef-classic/org.eclipse.gef/src/org/eclipse/gef/`。

## 语义映射

| Family ID | GEF 类 / 方法证据 | 必须保留的语义 | Rust 实现证据 | 本次判断 | 验证证据与限度 |
|---|---|---|---|---|---|
| `viewer.targeting` | `tools/TargetingTool.java:458-471` `updateTargetUnderMouse`; `EditPartViewer.findObjectAtExcluding` | 视觉命中与 Request 目标解析分离；命中内部 Figure 后找 owner | `viewer/mod.rs:939-972` `part_for_visual_or_ancestor/target_at` | 视觉 owner 映射等价；Request 二次目标解析缺失 | G3 `handles_win_targeting_and_feedback_is_transparent`；未覆盖 policy 重定向 |
| `root.layers` | `editparts/ScalableFreeformRootEditPart`；GEF `LayerConstants` | handle/feedback 与内容拥有明确 Z-order 和缩放域 | `viewer/mod.rs:500-625` `create_root_layers`，`:1052-1098` overlay API | 基础行为已覆盖；root factory 尚不可替换 | G3 `root_layers_have_stable_scaled_and_unscaled_z_order`；未证明任意变换场景 |
| `viewer.selection` | `SelectionManager`、`EditPart.SELECTED/SELECTED_PRIMARY` | 有序多选与 primary；变化通知 | `selection/mod.rs:60-177` `SelectionModel`；`viewer/mod.rs:761-798` | 有序集合与 typed delta 合理迁移；尚无 SelectionPolicy 注入 | G3 selection 3 项契约；delta 由调用者消费，不等于完整 JFace provider |
| `viewer.focus` | `EditPartViewer.setFocus` / `EditPart.setFocus` | 编辑焦点区别于 Figure 键盘焦点 | `selection/mod.rs:144-165`；`viewer/mod.rs:793` | 合理迁移 | `focus_is_independent_but_reconciled_with_retired_parts` |
| `request.protocol` | `Request.java:18-88`；`requests/ChangeBoundsRequest` | 请求携带意图和上下文；客户端可添加新的请求种类 | `request/mod.rs:203-326`；`:588-655` `EditorRequest` | 内置请求强类型是改进；封闭 enum 缩减框架扩展语义 | G4 typed bounds、G5 typed connection 测试仅覆盖内置枚举 |
| `policy.protocol` | `EditPolicy.java:130-267` | 按 role 组合行为，understands/target/command/feedback 分工 | `policy/mod.rs:19-51,135-191,282-322` | trait + typed role 合理；只在构建/退休时安装删除 | G4 `policy_rejection_is_distinct_from_no_command_contribution` |
| `policy.protocol` | `editparts/AbstractEditPart.java:637-650`；`tools/TargetingTool.java:138-143,458-471` | 返回的目标身份决定命令/反馈接收者 | `viewer/mod.rs:1175-1185,1218-1229` | **实现缺口：只调用 `.is_some()`，丢弃返回的 EditPartId** | 源码直接确认；本次没有动态 probe |
| `policy.protocol` | `AbstractEditPart.java:676-710,923-939` | active policy 替换/卸载必须执行 deactivate/activate | `policy/mod.rs:282-322` `PolicyStore` 为 crate-private，重复 role 拒绝 | 动态替换缺口；拒绝重复 role 本身是明确行为 | 未有外部动态替换入口/测试；不是 crash 缺陷 |
| `command.protocol` | `AbstractEditPart.java:486-491` command chain | 多个策略的贡献组合顺序确定；拒绝与无贡献可辨 | `viewer/mod.rs:1158-1197` | BTreeMap role 顺序与 CompoundCommand 合理变体 | G4 compound move、policy rejection |
| `tool.lifecycle` | `Tool.java:27-101`；`EditDomain.java:414-424` | domain 可切换任意 Tool，旧工具停用并清理，再激活新工具 | `domain.rs:114-139,175-195,439-459` | 内置工具行为部分实现；没有 Tool trait / `set_active_tool` 扩展端口 | 内置 create/cancel 契约；不能据此称通用 Tool 协议 verified |
| `tool.tracker` | `DragTracker.java:16-38`；`SelectionTool.java:198-214,648` | press 选定 tracker，后续输入固定发给 tracker，release/cancel 清理 | `tool/mod.rs:24-31,133-266` `DragGesture` | 内置 move/resize 可用；外部 tracker 和自定义 HandleRole 不可接入 | G4 selection drag/resize；没有外部 tracker 测试 |
| `input.arbitration` | `Tool.java:47-62` 键盘约束；`DomainEventDispatcher` | Figure 消费/捕获与 Tool 的归属清晰；键盘也经过 active Tool | `viewer/mod.rs:975-1045`；`domain.rs` 仅 pointer 入口 | pointer 的已消费分支覆盖；缺统一键盘入口，capture 单独拒绝条件不完整 | G3 widget/capture 测试通过，但未覆盖“仍捕获而本次未消费”的分支 |
| `feedback.protocol` | `Tool.java:47-51`；`EditPolicy.java:157-175,229-250` | source/target feedback 独立管理，执行命令前全部移除 | `tool/mod.rs:223-249,512-553,611-625,781-792`；`policy/mod.rs:84-107` | 内置 cleanup 顺序等价；统一 `feedback()` 不完整表达 source/target 两路 | G4/G5 cleanup 契约；第三方 feedback 部分安装失败的清理未验证 |
| `interaction.selection` | `SelectionTool`、`MarqueeSelectionTool` | click、多选、drag、marquee、可选选择规则 | `tool/mod.rs:133-192,223-249` | click/multiselect 合理；marquee 与选择策略后置 | G3/G4 当前切片已测；不宣称完整 SelectionTool |
| `interaction.change_bounds` | `DragEditPartsTracker.java:301-334`；`ResizeTracker` | move/resize，跨 parent 的 orphan/add 目标处理；约束域转换 | `tool/mod.rs:207-218,732-779`；`request/mod.rs:317-325` | 基础 move/resize 等价；reparent、非单位变换下请求转换未闭合 | G4 使用单位内容变换；不能从此推出 scroll/zoom 对等 |
| `interaction.create` | `CreationTool` / `CreateRequest` | 工具接收输入，策略创建模型命令 | `request/mod.rs:353-414`；`domain.rs:197-214` | typed request/command 已有，专用 CreationTool 后置 | G4 create/delete 测试；Native N 键是 demo 编排 |
| `interaction.delete` | `Tool.java:52-62`、`ComponentEditPolicy` | 删除动作须服从 active Tool 状态与模型 command | `domain.rs:197-214`；`apps/native/node-editor-demo/src/main.rs:1555-1588` | 模型 command 闭环；键盘直接在 app 解释，通用键盘仲裁缺口 | G4/G5 删除 undo 测试；Native 未把按键先交 Figure |
| `connection.create` | `ConnectionCreationTool` / `CreateConnectionRequest` | 锁定 source，选择合法 target，撤销恢复模型 | `tool/mod.rs:568-722`；`policy/mod.rs:200-233` | 两阶段 plan/command 是合理 Rust 变体 | G5 two-stage/invalid-target/widget/undo 测试 |
| `connection.reconnect` | `ConnectionEndpointTracker` / `ReconnectRequest` | endpoint 身份固定，另一端保持，校验目标 | `tool/mod.rs:423-564`；`viewer/mod.rs:1308-1432` | 已实现当前节点端点切片 | G5 source/target reconnect/cancel/undo；connection-as-node 尚不支持 |
| `connection.bendpoint` | `ConnectionBendpointTracker.java:90-91`、`BendpointEditPolicy` | 有序折点创建/移动/删除通过命令，route 显示与模型约束一致 | `request/mod.rs:115-200`；`tool/mod.rs:305-420`；`viewer/mod.rs:875-914` | 当前工作区已实现；人工与跨平台验收未闭合 | G5 `bendpoint_create_move_delete_round_trips_without_rebuilding_connection`；不是 G5 整体完成 |
| `viewport.autoexpose` | `AutoexposeHelper` / `ViewportAutoexposeHelper` | 拖拽贴边持续滚动，并重算输入到内容域映射 | `domain.rs/tool/mod.rs` 无对应调用；root 无 Viewport 组合入口 | 计划能力，尚未实现 | G5 后续切片；不列为现有缺陷 |
| `direct_edit` / `clipboard.protocol` / `snap.guides` / `palette` | `DirectEditManager`、GEF actions、`SnapToHelper`、palette | 文本编辑/IME、剪贴板、吸附、工具选择扩展 | GEF 账本明确 deferred | 后置 | 不计入当前 G1-G5 已交付，也不因后置扣成实现 bug |

## 关键差异与 Rust 判断

`PolicyRole::Custom(Arc<str>)` 只开放策略的命名空间，不开放新交互语义。
`CreationType` 只决定创建什么模型，不能替代可扩展 Request。
为避免 Java `Object/Map` 的运行时转换而使用 typed enum 是合理的；
将所有客户端新请求都变成修改框架源码的任务，则损失 GEF 的核心扩展能力。

建议保留内置请求枚举，再增加应用定义的关联类型请求载荷；或提供边界明确的
request trait。先用“应用新增一种业务请求，外部 Tool 发起，外部 Policy 贡献 Command”
的完整用例确定对象安全和借用边界，不预先建设通用 plugin registry。

Tool 的重复控制流目前分布在 `EditorDomain` 的多个 Option 字段与 pointer 分支。
应由单个 active Tool 接口与显式 gesture ownership 表达，在 ToolContext 中提供受控
viewer/query/feedback/command 服务。只读 query 与 owned effects 继续保留，不将
任意可变 Runtime 暴露给插件。键盘、pointer cancel、focus lost 同样进入这个接口。

Policy targeting 要先解析返回的 Part，验证 namespace/活性，检测重定向循环，再将
target request 路由到相应目标。source command contributions 与 target contributions
不能机械合并成“所有命令都重定向”，应对标 `DragEditPartsTracker#getCommand` 的
MOVE/ORPHAN/ADD 分工。

## 确认缺陷

`EditPolicy::target` 的返回身份在 `command_for_request` 与
`show_feedback_for_request` 中被丢弃。一个 child policy 返回 parent 作为目标时，
当前仍调用 child policy 的 command/feedback，并提供原 child 的 `PolicyHost`；
甚至返回 foreign/disposed ID 也仅因 `Some` 而通过。这会使委托容器接收请求的扩展
产生错误命令或错误反馈。按条件性语义缺陷 P1、置信度 10 记录。

## 实施顺序与验收

1. 修复目标身份路由并补同 host、parent retarget、foreign/stale、cycle 与 command/feedback
   一致性契约；保留 source/target 贡献区别。
2. 设计 Tool/Tracker + typed custom request 扩展端口，以外部 crate 的实际交互验证；
   同时统一键盘和 cancel 入口。
3. 明确 SelectionPolicy、RootLayerFactory 与动态 policy replacement 的生命周期。
4. 继续 G5 viewport/zoom/autoexpose，分别验证 0.5×/2×、滚动、嵌套 parent 下
   move/resize/reconnect/bendpoint 的模型坐标与 feedback。
5. G6 做共享模型的多 Viewer/document ownership、保存加载和 Native/Web/Headless 毕业；
   当前单 Viewer 通过结果不得扩展为多 Viewer 等价声明。

## 实际读取证据

- Rust：`novadraw-editor/src/{domain.rs,request/mod.rs,policy/mod.rs,tool/mod.rs}` 全文；
  `viewer/mod.rs` 的 root layers、selection、targeting、overlay、command/feedback 与
  connection gesture 路径；`feedback/mod.rs` 和 `selection/mod.rs` 公共面。
- 测试：G3 selection/viewer、G4 editing loop、G5 connection creation 中与上述路径有关的
  用例；运行结果由主审计 `test.log` 给出。
- 平台：Native node-editor demo 的 key/pointer/focus 输入分支。
- Java：`Request`、`Tool`、`DragTracker` 完整接口与 Javadoc；
  `AbstractEditPart` 的 target/install/remove/command；
  `TargetingTool` 的 target resolution 与 command；
  `SelectionTool` tracker 转发；`DragEditPartsTracker` source/target command 分工；
  `ConnectionBendpointTracker` command 入口。
