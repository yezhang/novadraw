# egui Editor Canvas 接入契约

类型：`normative-design`
能力归属：其他——`novadraw-ui` 的 egui／eframe Canvas 宿主适配。

用户已要求用 egui／eframe Canvas 承接 Masonry `EditorCanvas` 的职责。
本次使用现有 Core、Editor 和 Vello 协议，不增加引擎能力。

## 分层与所有权

`canvas` feature 提供 `CanvasSurface`：共享 eframe 的 WGPU device／queue，
持有 Vello renderer、离屏纹理及 egui texture registration。
普通工作台不启用该 feature 时不引入这些依赖。
`editor-canvas` 再单向依赖 Editor，提供 `EditorCanvas<A, F>`，
持有一份 Viewer 和 Domain。模型、选区、工具、编辑会话及命令历史仍是 Editor
对象中的状态；UI 不复制文档草稿或创建第二份 CommandStack。
Core／Editor 不依赖 UI。所有 egui／eframe 依赖使用官方 crates.io 发布版。

应用每次在面板 body 中调用 `show`，返回 egui Response 和错误。
工具栏通过同一份 `viewer`／`domain` 执行命令；外部 Factory、Tool、Policy
走现有 Editor 扩展接口。隐藏、切换文档或移走 Canvas 前调用 `deactivate`；
Drop 取消临时编辑并注销纹理。替代的是宿主职责，Masonry 适配保留给仍使用它的消费者。

## 输入与生命周期

Canvas 使用稳定 egui Id。仅当前可交互区域接收新按下与滚轮；拖拽由 egui
控件身份仲裁，已捕获的手势可在区域外继续接收移动和对应按钮释放。
坐标先从 egui panel points 转为 Canvas logical surface points，再交给 Editor；
UI 不计算模型坐标或重写 Figure 命中链。中键平移和滚轮调用 Domain 的视口接口，
缩放以指针处为基准。禁用、失焦、窗口失焦和隐藏取消手势，文本失焦遵循会话策略。
错误取消残留捕获并返回调用者。

文本输入使用本地 Composer 的同一状态机，按平台事件顺序处理 preedit、commit、
键盘和 pointer。输入绑定当时的 Editor session id，旧 lease 不得写入新会话。
不每帧导出全文草稿；仅显式复制／剪切时读取文本快照。
候选窗口位置使用 Editor 发布的逻辑 caret，再映射到 egui 全局坐标。
官方接口没有原生 composition reset acknowledgement；不宣称这项保证。

尺寸和 DPI 先校验有限值及 device 的 texture dimension limit；零尺寸暂停，
不分配零大小纹理。纹理只在物理尺寸变化时重建；重建触发完整重绘。
Viewer 驱动负责 prepare／submit／complete，包含失败提交的 completion。
Idle 不再次执行 Vello。Runtime deadline 与 auto-expose 决定下次唤醒，
不设置常驻刷新循环。设备重建由应用提供新的 RenderState，并重置后端 session。
最终窗口合成和设备丢失通知仍由 eframe／应用负责。

## 验收

独立应用以自定义模型和 Factory 消费组件；验证选择、区域外拖拽释放、
中键平移、滚轮、键盘取消、禁用／失焦、统一撤销重做、IME 事件顺序与尺寸拒绝。
Native 截图验证离屏纹理与停靠面板合成；Web 编译作为依赖检查。
真实操作系统输入法、跨原生窗口捕获及 Windows／Linux 人工行为分别记录，
不能由无窗口测试或编译替代。性能检查关注 Idle 不提交、尺寸稳定不重建、
候选位置更新不复制全文；本次不预设加速比例。
