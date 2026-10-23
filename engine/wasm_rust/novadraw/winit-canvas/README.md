# Winit Canvas

类型：`implementation-guide`

独立 Rust UI 组件，使用 winit 官方窗口、事件和 redraw 机制实现窗口内多个 Canvas。
组件不依赖 Novadraw、Vello、Figure 或 Editor；WGPU 为可选渲染目标实现。

## 组件契约

- 一个 `CanvasHost` 对应一个 winit `Window`；Canvas 按插入顺序确定绘制和命中顺序。
- 每个 Canvas 拥有稳定身份、窗口逻辑矩形、可见/启用状态和重绘版本。
- 输入使用 winit 原生事件类型。Host 将窗口物理点除以 DPI，再减去 Canvas 的逻辑原点。
  handler 接收局部逻辑点，滚轮 pixel delta 同样转为逻辑单位。
- 焦点、hover、按下按钮和 capture 由 Host 协调。第一个鼠标按下锁定 Canvas，最后一个
  按钮释放才结束 capture；离开区域继续拖拽。失焦、隐藏、禁用、删除和 suspend 取消交互。
- 窗口内其他 UI 可以先处理事件；交给 `CanvasHost` 的事件只路由一次。
  capture 活跃时宿主必须继续向 Canvas 转发指针事件。
- Canvas 不是原生子窗口；操作系统仍只看到所属 winit Window。
  Canvas 不独占线程或 GPU device。
- capture 是组件的逻辑路由机制，不提供操作系统级的全局鼠标捕获。
  winit 未提供原生 Canvas 控件，本库建立在其公开 Window、WindowEvent 和
  ApplicationHandler 机制上。

## 渲染契约

`gpu` feature 提供 `CanvasTarget` 的独立 RGBA 纹理与 `CanvasCompositor`。
调用者可用 WGPU render pass、Vello 或其他 GPU renderer 写入该纹理。
纹理为预乘 alpha，合成使用预乘 alpha 混合和窗口 scissor，不允许越过 Canvas 区域。
GPU device、queue 和窗口 surface 归应用；目标绑定在创建它的 device 上。
目标尺寸为向上取整的逻辑尺寸乘 DPI，渲染坐标范围为像素尺寸除以 DPI；
合成只采样对应逻辑矩形的有效纹理区域，避免小数尺寸拉伸。

`CanvasHost` 的重绘版本在 renderer 成功后显式确认。确认旧版本不清除新 mutation，
失败不确认，下一次仍可重试。位置变动、移除、隐藏和 resize 都请求窗口重新合成，
以擦除旧区域；未变化的 Canvas 可复用已有纹理。
空尺寸不创建 GPU 纹理，也不绘制。非法尺寸、DPI、身份和 GPU 上限使用 `Result` 返回。

Canvas 只负责窗口到画布局部坐标，Novadraw 的 Figure、Viewport 滚动和缩放仍由
Novadraw Runtime 处理。渲染集成消费者应独立组合 Runtime 与 backend，
不得将 Novadraw 的依赖加回本组件。

Novadraw 接入位于 `novadraw-backend-vello::texture`：
`VelloTextureRenderer::new(device, queue)` 共用应用的 GPU，
`bind(target.texture())` 返回当帧的 `RenderBackend`。
消费者用该 backend 的 capabilities 准备完整帧，再调用 submit 与
Runtime 的 complete_submission。资源缓存和绘制命令解释与窗口后端共用。
消费者必须处理 complete_submission 返回的 `SubmissionAcknowledgement`；
离屏 backend 的 `Presented` 表示纹理已提交，窗口呈现仍由宿主负责。
离屏提交成功后由宿主合成并呈现窗口；窗口呈现重试直接复用已生成纹理。
设备丢失和异步 WGPU 错误由应用处理，不能用旧 device 的纹理接入新 device。

## 生命周期与宿主接线

在 `ApplicationHandler::window_event` 中将所属窗口事件交给 Host，
按返回的 `CanvasEvent` 批次调用消费者；若 `needs_redraw()`，调用 `Window::request_redraw()`。
`RedrawRequested` 时更新 dirty Canvas 纹理并合成，再按成功结果确认重绘版本。
应用处理 `ApplicationHandler::suspended` 时调用 Host 的 suspend，并释放窗口 surface；
resume 后更新窗口 metrics 和 GPU 目标，重新绘制。

IME 输入只传给获得焦点的 Canvas。应用将局部候选框转换为窗口逻辑坐标，
再调用 winit 的 `set_ime_cursor_area`；IME enabled、cursor 和 OS 焦点归窗口宿主。
Canvas focus 与整个窗口的 OS focus 是不同层次。

## 验证与示例

库的确定性测试覆盖 DPI 与局部坐标、重叠区域、跨区域 capture、多按钮释放、
焦点/IME、隐藏和删除清理、空尺寸及重绘确认。
独立 Native 示例演示一个窗口内多个纹理 Canvas，消费者在组件外接入 renderer。
平台运行证据以实际执行记录为准，不以跨平台编译推断 Windows/Linux 已人工验收。

```sh
cargo run -p winit-canvas-demo
cargo test -p winit-canvas --test host_contract
cargo xtask verify canvas.native
```

示例左侧直接使用 Vello，右侧使用 Novadraw Runtime。按住矩形拖拽；Tab 切换
Canvas 焦点，方向键移动获得焦点的 Canvas，R 恢复 Canvas 布局。
resize 按新窗口尺寸分配区域与纹理。示例的矩形拖拽是消费者自身行为；
完整 Novadraw Figure 输入集成应在消费者中把 Canvas 局部点交给 Runtime 事件入口。

`canvas.native` 需要可用 GPU 和桌面会话，执行真实像素断言并启动 winit 窗口。
截图与结果写到 `target/verification/winit-canvas/`。
实施与验证记录见 [implementation.md](implementation.md)。
