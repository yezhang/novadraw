# 本地工作台与 GPU 升级契约

类型：`normative-design`
能力归属：其他——`novadraw-ui` 工作台组件、Vello 后端和原生宿主。

用户已授权升级 WGPU，并将 craft-ui 的工作台代码迁为本地实现。
本方案记录这次迁移的边界，不增加 Core 或 Editor 能力。

## 依赖与版本

共享 GPU 链路使用 crates.io 官方 Vello 0.11、WGPU 30、egui／eframe 0.37，
原生输入使用官方 winit 0.30。egui 全族和 winit 不使用其他项目的定制分支，
也不使用 Git patch。Rust 使用系统默认工具链。
根 Cargo.lock 按既有规则被 Git 忽略，只记录本地解析结果。
可选 Masonry 宿主保留其独立 GPU 依赖，不能与这条链路交换 GPU 对象。

`novadraw-ui` 本地维护 craft-libs 提交
`9be9a63782298ab59cd9b7a42ee9cba4f42a5ccb` 中工作台模块的代码。
不依赖任何 craft crate。字体服务、国际化、印刷服务与业务资产不在本次范围内。
保留源代码许可证、来源说明和涉及的测试夹具。

## 所有权与公开接口

应用持有 `Layout<P>` 或 `Workspace<P>`，`P` 由应用定义。`DockContent<P>`
提供内容、标签和权限；外部面板与内置面板共用此接口。
`DockArea` 绘制停靠树并返回动作；`apply_all` 验证整个动作批次，
成功后才提交布局。失败保留原布局，交由应用展示或处理错误。
布局深度、面板数量和有限几何值在操作前校验。

UI 持有拖动、焦点和浮窗展示状态。文档数据、选区、工具会话及撤销历史
由应用和 Editor 持有；UI 不创建另一套文档命令栈。
Canvas 面板可以由应用权限禁止关闭或移动。
浮窗身份保留持久化语义，生成器不使用全局可变状态；
布局校验继续拒绝重复身份。

IME composer 只将输入转换为编辑意图，不修改文档。
composer 的 owner 只用于本地窗口记录的发布和撤回，不伪装成官方平台的身份字段。
官方 `IMEOutput` 只发布 purpose、rect、cursor_rect 和 composition interruption。
原生重置完成 fence、context generation 和按字节删除事件不在官方接口中，
本轮不提供，也不使用 Debug 字符串解析来模拟。RTL 读取官方 Ui Layout。
接入富文本编辑时仍使用 Editor 的统一会话协议。
Core／Editor 不依赖 egui，也不依赖 `novadraw-ui`。
后续 Canvas 接入通过 UI 的可选 `canvas`／`editor-canvas` feature
单向依赖 Core、Editor 和 Vello；普通停靠组件的默认依赖不变。
输入、会话与 GPU 生命周期见 [egui Editor Canvas](egui-editor-canvas.md)。

## 性能与验收

沿用上游动作驱动的绘制和事务模型，避免每帧重建布局或浮窗身份。
保留模型、拖动、浮窗、权限、隐藏恢复、布局序列化和 IME 测试。
新增独立消费者检验自定义面板和原子拒绝路径，并提供可运行工作台样例。
GPU 升级验证覆盖离屏渲染、Canvas 合成、原生 egui 宿主编译和 Web 编译。
操作系统跨窗口拖动、真实输入法及 Windows/Linux 行为须区分自动测试与人工验收。
本次未提出性能加速结论，不将编译通过写成生产性能或跨平台验收通过。
