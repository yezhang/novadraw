# Masonry 编辑画布与 Xilem View 接入

类型：`normative-design`

2026-10-28，按用户批准的
[GEF 对等样例方案](gef-official-examples-parity-proposal.md)继续实施。
本切片保持 [ADR-053](../../adr/adr-053-embedded-vello-scene-target.md)的完整帧发布与宿主
所有权边界。迁移后由独立 imaging RenderBackend 输出 recording，不交换 Vello Scene。
不修改 Core、Editor 所有权或第三方参考源码。

## 依赖与范围

Native UI 直接依赖未修改的上游 Git 提交
`66c8893f12232b9d5cdf4296f773aa3dc07ee776`，由 Cargo 下载并锁定；本地 vendor 已删除。
UI 包使用系统默认 Rust 工具链，最低支持版本由依赖声明约束。上游使用
imaging 0.0.1、Vello 0.8 / WGPU 28，
Novadraw 现有 Vello 0.10 / WGPU 29 后端保持原有版本。双方不交换 GPU 对象。
UI 适配和示例纳入根 workspace，支持 `cargo run -p gef-editor-demo`，
通过 `default-members` 排除在普通引擎构建之外。Core/Editor 消费者不依赖 UI 包；
显式 `--workspace` 验证包含 UI 包，也需要满足 UI 工具链要求。
首次切片验证 Widget/View 编译、真实输入编辑、Scene 合成与稳定 rebuild；
完整 IME bridge、窗口外 capture、系统 AT、模板拖放和框选分别继续验收，
不由最小画布的成功自动升级完成状态。

## 状态归属

Masonry Widget 持有一个 Editor session controller，controller 组合 Viewer、
EditorDomain 与独立 imaging recording renderer。平台 Host 效果桥不拥有业务模型或 Runtime。
Xilem View 是可重建描述，只持有 Send+Sync 配置和 callback；
不要求应用模型、Viewer 或 Runtime 实现 Send/Sync。

View build callback 创建一次 session。rebuild callback 只提交应用动作和受控配置；
不替换 Viewer、Runtime、CommandStack 或活动会话。Widget 的 typed action
把稳定 UI snapshot 传回应用，工具栏/属性 UI 通过同一应用动作入口更新 session。
避免在 View 中保存 Rc<RefCell<Viewer>>，其线程约束不满足 WidgetView 协议。

## Widget 契约

输入先使用 EventCtx.local_position 转为 content-box 逻辑坐标，DPI 不重复换算。
Viewer 按 direct-edit/Figure/Tool 仲裁，Widget 用 outcome 和有效 gesture/capture
决定 handled 与 Masonry capture。窗口失焦、禁用、移出树和 teardown 清理工具反馈。
普通画布可以键盘获焦；IME 接受资格固定，后续通过独立 TextInputBridge 处理 session
启停，不让 plain Canvas 默认获取平台 IME。

layout 记录画布内容尺寸和 clip，通过 `mutate_self_later` 请求一次动画回调。
输入、rebuild、layout、DPI 改变和 Retry 通过该回调以同一尺寸/DPI 构造 surface，
通过 ViewerDrive 调整 viewport、prepare、backend submit 与 complete。
重复 paint 仅重放缓存 recording，不重复执行业务 Command。
prepare/submit 失败保留旧 Scene 并报告错误；Retry 保持待更新状态，
需要宿主再次驱动，不能标为成功。无活动工作不持续 tick。
`host::run` 委托上游 `MasonryState`；DPI / exposure 变化请求一次树动画，
新 scale 在随后动画回调读取。该扫描只在生命周期事件发生时执行。

输出适配支持默认 AA 的图元、路径、捕获变换的裁剪、完整描边、渐变、图像源矩形和
Core 默认文字轮廓。imaging 0.0.1 不能完整表示逐操作 AA 和原生 glyph brush/dash，
因此不声明 `PerOperationAntialias` / `GlyphRuns`，遇到要求这些能力的命令明确拒绝。
失败或缺资源 Retry 不改变已发布 recording 和已接受资源状态。
这是该目标的明确能力边界，不改变现有 Vello 后端的完整绘制能力。

编码成本为 O(commands + resources)，会增加一份 retained recording；
资源负载使用共享所有权，paint 重放不进行 GPU readback。GPU device、光栅化与窗口
present 仍由上游宿主管理。实际产物体积与拖动开销在迁移验证中单独测量。

## View 契约与验证

build 创建 Widget 并注册 typed action source；rebuild 消费配置 revision 与动作；
message 把正确 CanvasAction 交给应用，错误类型/路径返回 Stale；
teardown 先清理 session，再移除 action source，旧 Widget ID 从路由表移除。
反复重建必须保留业务 ID、Runtime namespace、Viewer namespace 和 undo history。
重新插入 View 则创建新的宿主身份；旧 action 不能通过旧 ID 路由到新 session。

外部消费用例需要：

- 相同模型在 headless与 Masonry 画布运行，使用公开 Editor/Runtime/Scene API；
- Widget drag →模型 command →通知 →Figure/Scene，undo/redo恢复；
- UI button触发 rebuild后保持 session身份和历史；
- pointer capture、焦点与禁用取消不会产生重复模型提交；
- 实际 GPU画面可检查，默认字体/媒体和 package 成本单独记录。

实施状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
