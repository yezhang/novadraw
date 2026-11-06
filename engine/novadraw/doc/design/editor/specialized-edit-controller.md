# 统一会话与专用编辑控制器

类型：`normative-design`

能力归属：Editor 管理会话协调、输入仲裁、反馈生命周期和 Command；
richtext 管理文档草稿、文本选区、composition 与文本操作；Core 发布场景和几何；
应用持有已提交模型，platform 执行输入 lease。

状态：公开协议与样例已实施，定向验证通过；完整矩阵未关闭。
2026-11-06 用户“执行 ADR-061”授权本方案范围。
决策入口：[ADR-061](../../adr/adr-061-edit-session-and-specialized-controller-boundary.md)。

## 接口和状态

`EditController` 是可克隆的领域状态协议。一次输入先作用于候选控制器，反馈成功
挂载并替换旧反馈后才发布候选状态；可恢复拒绝保留旧草稿和输入归属。
richtext 的克隆共享不可变文档，不能为事务检查再复制全文布局。

`EditControllerKey<C>` 提供类型化控制器身份。会话贡献将该身份、controller 和
`EditSessionPlan<A, C>` 绑定，内部擦除仅用于异构存储；公开查询按 typed key 检查。
`StartEditSession` 使用既有 Request/Policy 注册与分派机制，外部控制器与 plain
控制器共用同一活动槽、输入与提交路径。旧 `StartDirectTextEdit` 仅保留发现适配，
其贡献在激活前转换成同一会话贡献；不维护第二个活动会话。
旧启动接口保留“已有会话时返回 `SessionAlreadyActive`”的兼容行为；
新 `start_edit_session` 则先执行旧会话的 focus-loss 策略。两者激活后使用同一
控制器槽和输入、反馈、命令路径。

协调记录只保存 session、source、feature、source revision、mode、focus-loss，
以及反馈与 blink 生命周期。草稿、selection、composition 只在 controller 中。
`DirectTextEditState` 成为 plain 控制器；旧 getter 和定位接口只作为受检 plain
适配，通用输入不通过这些接口转发。

`EditSessionPlan` 按具体 controller 构造反馈、检查草稿和生成模型 Command。
它不保存活模型借用。接受前统一检查 Part、模型 revision 和 composition；
NoChange 不生成 Command。执行拒绝且模型保持不变时恢复会话；未知状态与 panic
沿用 Viewer fault 边界，不继续向 Tool 分发。

## 几何和渲染

`EditGeometryContext` 在输入与选区回调中只允许读取当前反馈；
typed component 更新与 bounds 修改会被拒绝，防止候选输入修改旧反馈。
在新反馈的 geometry 回调中允许目标限定的 typed component 更新、准备发布、
只读树和文本几何查询，不暴露可变 Runtime。`EditGeometry` 返回 surface caret、
viewport、selection 和 preedit 几何；Viewer 负责装饰挂载、裁剪与清理。
plain 控制器用 TextFlow 同版几何；richtext 适配器用 RichTextLayoutSnapshot。
反馈索引只标识交互目标，不再表示该 Figure 必须是 TextFlow。

richtext 草稿反馈仍是 RichTextContentFigure，正文不 flatten 成 TextFlow。
平台 buffer snapshot 是输入适配投影，按需生成，不作为第二份文档或布局真源。
平台不支持的跨段样式输入必须明确拒绝，不能丢失格式后静默接受。
`GraphicalViewer::edit_input_snapshot` 在扩展失败边界内按需调用控制器，未知状态
与 panic 触发 Viewer fault 和会话清理。
控制器返回 `ViewerError::InconsistentState` 表示不能继续使用该会话，
不会包装成未处理输入再交给 Tool。Plan 的 `PolicyError` 表示可恢复拒绝，
Plan 不得在反馈检查中保存另一份可变领域草稿。

当前白底样例用应用提供的遮盖 Figure 与草稿正文形成不透明反馈层；取消后清理
整个反馈层，已提交正文始终由模型投影持有。该策略适用于本样例的白底内容区域，
不是透明内容的通用显隐协议。正文与草稿仍都会生成绘制命令，成本采样包含这项
额外工作；不据此宣称完成渲染或长文优化。

## 使用与提交

独立宿主直接调用 richtext controller，并把受检文档更新发布到组件。
嵌入宿主通过统一会话使用同一 controller；应用模型保存文档，Command 原子
替换文档并发模型通知，Figure 只保存不可变投影。

会话内不新增草稿撤销栈；文档撤销先结束当前草稿，再操作 CommandStack。
IME Commit 只改草稿。会话接受才产生一个文档历史项。取消、Part retire 和
lease lost 都释放反馈与输入；旧 session 输入被拒绝。
宿主销毁 Viewer 前应通过 Domain 取消编辑，并在窗口输入资源仍存活时执行
Release effect。直接丢弃 Rust 对象会释放领域资产，但不能替宿主执行平台输入操作；
完整销毁组合仍在验证矩阵中保留。

滚动、resize、zoom 后通过同一反馈重建及 SetArea 更新几何。指针落在文本反馈内
只修改文本选区；落在其他对象或 handle 上先按 focus-loss 结束会话。
切换遇到可恢复拒绝时保留旧会话，新的 Tool 和 controller 不激活。

## 成本与验收

分别测 controller mutation、草稿准备、会话反馈、accept、模型投影与 backend。
沿用 edit-app 的 mixed/large/giant 数据与字体条件，对比独立组件和 Editor。
不将本协议改造记作增量排版或长文黑帧整改完成。

完整验收见[验证矩阵](../../verification/plans/specialized-edit-controller.md)。
接口迁移先跑 plain 回归，再跑独立外部消费者与 richtext 接入，最后核验 Native
输入、同版几何和成本。未完成项保留开放状态。
