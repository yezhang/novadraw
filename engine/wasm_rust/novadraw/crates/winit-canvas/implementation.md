# Winit Canvas 实施记录

类型：`implementation-guide`

日期：2026-10-23

## 已实现的边界

`winit-canvas` 是独立 UI 库。默认只依赖 winit，`gpu` feature 增加 WGPU；
不依赖 Novadraw 或 Vello。它使用一个 winit Window 内的逻辑矩形定义 Canvas，
为每个 Canvas 分配独立纹理，并协调输入、焦点和重绘。
操作系统看到一个窗口，窗口 surface 和 GPU device 仍归应用。

组件按事件发生顺序返回 delivery batch，消费者在路由状态更新后处理事件。
鼠标按下、触摸和连续滚动锁定起始 Canvas；目标失效后保留取消状态，
迟到的事件不会转交另一 Canvas。DPI 转换和窗口到局部坐标转换归组件。
IME 的窗口配置与候选框位置归应用，输入内容只交给有焦点的 Canvas。

每个 Canvas 的重绘版本与窗口合成版本分别确认。纹理成功写入后确认 Canvas；
窗口成功合成后确认窗口版本。旧确认不会抹掉更新的重绘请求。
移动、隐藏、删除、resize 和 DPI 变化触发合成，旧位置通过清屏重建消除。
空尺寸暂停分配和绘制，resume 请求全部重绘。

## Novadraw 在组件外接入

`crates/novadraw-backend-vello/src/texture.rs` 提供 `VelloTextureRenderer` 和
`TextureBackend`。它们使用应用已有的 device、queue 和 RGBA8 storage texture，
不创建 Window 或 Surface，也不依赖 `winit-canvas`。

窗口后端和离屏后端共用私有 `SceneEncoder`，包括状态栈、裁剪、几何、文字、
图片、资源同步和会话校验。现有窗口后端的 retained partial 策略保持在原后端内。
离屏后端声明 `FULL_FRAME_ONLY`，拒绝尺寸不符、格式不符及 partial submission。
Runtime 的 prepare/complete_submission 仍负责提交与资源游标确认。
最终窗口呈现、异步 GPU 错误和设备重建归宿主。

## 可运行示例

`examples/native/winit-canvas-demo` 只创建一个窗口：
左 Canvas 直接用 Vello 绘制，右 Canvas 使用 Novadraw Runtime。
两个矩形分别处理局部坐标拖拽。Tab 切换 Canvas 焦点，方向键移动 Canvas，
R 恢复区域布局；窗口 resize 后重新分配区域和纹理。

示例的拖拽是消费者自身的行为，验证 Canvas 的事件边界与渲染接入。
它不代表完整 Novadraw Figure 输入适配器，也不接管官方示例的 K07 工作。

## 验证入口与证据

- `winit-canvas` 的 `host_contract` 测试：DPI、命中顺序、鼠标 capture、多按钮、触摸、滚动、
  pinch、focus/IME、取消、身份隔离、空尺寸及重绘确认。
- `canvas.native`：Canvas 纹理合成的像素检查、Novadraw 离屏后端契约、
  双 Canvas Native 示例自检。
- 双 Canvas 证据：`target/verification/winit-canvas/native-before.png`、
  `native-after.png` 和 `result.txt`。

macOS 的实际运行已验证一个窗口内两个纹理区域和 DPI=2 的绘制。
Windows/Linux 尚未做原生人工验收。

本次最终门禁结果：

- workspace fmt、check、严格 Clippy 和文档检查通过。
- workspace 测试含 doctests：1,340 项通过，0 项失败，3 项忽略。
  忽略项中的两个 GPU 测试已另行显式执行，均通过；另一项为既有忽略测试。
- Vello 原有 24 项单元测试通过。离屏 GPU 测试还覆盖带 mip 的外部纹理、
  目标尺寸检查、完整帧重建、旧像素清除与过期会话拒绝。
- Native 自检通过：局部坐标拖拽、输入隔离、未变化纹理复用、Canvas 移动，
  以及 Vello/Novadraw 各自输出的像素颜色。

workspace 测试编译保留了既有 Core 测试代码的 warning；严格 Clippy 的范围为
仓库要求的默认 workspace 目标。执行日志保存在
`target/verification/winit-canvas/`。

## 纯 WGPU 独立样例增量

新增 `crates/winit-canvas/examples/standalone/`，通过
`cargo run -p winit-canvas --example standalone --features gpu` 启动。
该 example 不依赖 Novadraw、novadraw-platform-winit 或 Vello。
直接与开发依赖为 winit、WGPU、pollster 和 png；库的生产依赖未增加。

示例把应用入口、绘制和截图分为 `main.rs`、`paint.rs`/`paint.wgsl`、
`readback.rs`。CanvasHost 负责区域输入与重绘，WGPU shader 只绘制示例网格和矩形。
焦点由 CanvasHost 路由，方向键通过 CanvasEvent::Keyboard 传给当前画布。
窗口标题显示 Canvas 局部指针坐标。

macOS 原生自检已通过跨区 capture、DPI 局部拖拽、未变化纹理复用、
隐藏/恢复后的旧像素清除和两侧像素颜色检查，严格 example Clippy 通过。
截图与自检结果为 `standalone.png` 和 `standalone-result.txt`，
位于 `target/verification/winit-canvas/`。
复制组件目录到 workspace 外后，example 的独立编译也通过。
本增量的仓库文档检查被并行编辑的 K07 文档阻塞：
`k07-1-scene-capture-evidence-2026-10-23.md` 使用了校验器不支持的
`verification-report` 类型，因此不记为本增量文档门禁通过。

两库的职责与当前接入限制已写入 README：它们互不依赖，
`novadraw-platform-winit` 的现有 WinitPlatformHost 仍按整个窗口提供服务，
尚未提供子 Canvas 的 Novadraw 宿主适配。

## 独立样例 resize 呈现修复

沿用 2026-09-12 已验收的 macOS 修复：左上角固定旧 drawable、
配置 opaque 背景，以及仅 resize 帧采用 Core Animation 事务呈现。
机制位于 `gpu::presentation`，不依赖 Novadraw。
`SurfacePresentation` 在 acquire 前开启模式，在 present 后或提前退出时恢复。
样例的 resize 标记保持到成功呈现；surface lost 重建后重新配置 layer。

绘制前重新核对 Window 的实际尺寸和 DPI，处理 redraw 早于 Resized 的事件顺序，
避免以旧 Canvas 几何获取新尺寸 drawable。
`--verify-resize` 已通过七次真实窗口缩小/放大/恢复，
并检查窗口、surface、Canvas 尺寸一致以及事务模式的开启/恢复。
这证明了呈现策略与几何协议；自动截图不作为连续手动拖动无抖动的充分证据。
原有交互自检和严格 example Clippy 另行运行。

## Novadraw Canvas 宿主桥接

`novadraw-platform-winit` 已新增可选 `canvas` feature，
通过 `canvas::WinitCanvasHost` 和 `WinitCanvasCoordinator` 接入本组件。
该 feature 只使用 CanvasHost，不引入 WGPU/Vello；本组件仍不依赖 Novadraw。
此增量取代前面“尚未提供子 Canvas 宿主适配”的阶段状态。
整窗 WinitPlatformHost 保持原有语义。

双 Canvas Native 示例已使用新 Host 获取区域 SurfaceInfo、请求重绘，
通过 Coordinator 路由输入和确认各纹理版本。
当前工作区的 workspace check、13 项 canvas+editor 单元测试、
平台库严格 Clippy 以及 Native 自检通过；文档与格式检查通过。
中途并行 Core 变更引起编译失败，曾在已提交 Core 的临时副本验证，
随后已在当前工作区重新完成上述代码验证。

具体接线与窗口服务仲裁见
[Winit 平台适配](../novadraw-platform-winit/README.md)。

## 双 Canvas 样例统一使用 Novadraw

`examples/native/winit-canvas-demo` 的两个区域现在都由 Novadraw 绘制，
分别拥有 FigureTree、Runtime、字体资源与 VelloTextureRenderer。
移除左侧直接编写 Vello Scene 的路径。矩形位置从各自 FigureTree 查询，
拖拽通过 Runtime Figure mutation 修改，宿主仅保存指针偏移。
两侧 Label 各自在 Runtime 中注册内置 Inter 字体。

Native 自检覆盖独立 backend session、局部输入隔离、未变化纹理复用、
Canvas 移动、矩形旧像素清除，以及两侧的矩形颜色与 glyph 提交。
当前运行截图为 `target/verification/winit-canvas/native-after.png`。
样例接线见
[双 Canvas Novadraw 样例](../../examples/native/winit-canvas-demo/README.md)。

## Novadraw 双 Canvas 的 resize 呈现接入

前面的 macOS 呈现修复最初只接入了纯 WGPU 独立样例。
`winit-canvas-demo` 已补齐同一呈现策略：surface 创建/恢复时配置
左上角固定与 opaque 背景，resize drawable 在 acquire 前启用事务呈现，
成功后记录呈现尺寸，失败则保留待重试状态。
绘制前核对实际窗口尺寸，避免旧 Canvas 几何与新 drawable 混用；
零尺寸后恢复会重新走事务呈现，同尺寸输出纹理直接复用。

九次真实窗口尺寸变化回归通过，覆盖宽度、高度、双轴缩放与恢复，
并验证两个 Runtime/Canvas/纹理尺寸一致、Figure 位置保持及事务模式恢复。
这验证了呈现协议，不将自动截图等同于连续手动缩放的视觉验收。
证据为 `target/verification/winit-canvas/novadraw-resize-result.txt` 和
`novadraw-resize.png`。
