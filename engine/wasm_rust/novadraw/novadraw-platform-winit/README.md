# Winit 平台适配

类型：`implementation-guide`

`WinitPlatformHost` 为整窗 Novadraw 应用提供服务。
启用 `canvas` feature 后，`canvas::WinitCanvasHost` 为一个 Canvas 提供同一
`PlatformHost` 契约，`canvas::WinitCanvasCoordinator` 协调一个窗口内多个 Host。
`editor` 与 `canvas` feature 可以分别或同时启用。

```toml
novadraw-platform-winit = { version = "0.1", features = ["canvas"] }
winit-canvas = "0.1"
```

## 每个 Canvas 对应一个 Host

Coordinator 创建并持有 `winit_canvas::host::CanvasHost`。应用通过 `insert` 创建
Canvas 和 Host，使用 `canvas_id()` 将 Host、Runtime 和渲染目标关联起来。
Host 不拥有 Runtime、FigureTree、backend、GPU 或事件循环。
一个 Host 可以克隆，但克隆共享状态；多个 Runtime 应分别绑定不同 Canvas。

```rust
use std::sync::Arc;
use novadraw::host::PlatformHost;
use novadraw_platform_winit::canvas::WinitCanvasCoordinator;
use winit::window::Window;
use winit_canvas::geometry::{CanvasError, CanvasRect};

fn create_hosts(window: Arc<Window>) -> Result<WinitCanvasCoordinator, CanvasError> {
    let mut coordinator = WinitCanvasCoordinator::new(window)?;
    let left = coordinator.insert(CanvasRect::new(20.0, 40.0, 300.0, 400.0)?)?;
    let right = coordinator.insert(CanvasRect::new(340.0, 40.0, 300.0, 400.0)?)?;
    assert_eq!(left.surface_info().logical_width, 300.0);
    right.request_redraw();
    coordinator.synchronize()?;
    Ok(coordinator)
}
```

`surface_info()` 返回 Canvas 逻辑尺寸和向上取整的物理纹理尺寸。
隐藏、空尺寸、窗口零尺寸、suspend 或删除使绘图尺寸归零。
禁用 Canvas 取消输入与窗口效果，但可见内容仍能绘制。
删除或 Coordinator 释放后，保留的 Host 失效，无法影响新 Canvas。

## 输入与提交顺序

应用将所属窗口的 WindowEvent 交给 `handle_window_event`，再按 Delivery 的
CanvasId 转交对应消费者。输入点已经是 Canvas 局部逻辑坐标；
不要再次减去 Canvas 原点或再次除以 DPI。
键盘、IME、触摸和鼠标 capture 继续由 CanvasHost 路由。
本 Host 增量不引入新的 Runtime 输入协议；应用按现有 Core/Editor 输入入口处理事件。

布局、可见性、焦点和删除通过 `update_canvases` 修改，返回的 Delivery 仍需交给消费者。
生命周期使用 Coordinator 的 `suspend`/`resume`。GPU 重建与 Runtime
backend session 重置仍由应用处理。

Runtime 发布 Host 效果后，应用在读取重绘 token 和调度时间前调用
`synchronize()`。Host 的 void 方法暂存效果，真正的 Canvas mutation 通过该方法
返回 `Result`，失败时保留未消费的重绘请求。纹理成功写入后调用
`acknowledge_canvas`，窗口成功合成后调用 `acknowledge_composite`。
普通窗口 redraw 只复用未变化的 Canvas 纹理。

## 共享窗口服务的仲裁

- cursor 使用 capture 目标，或没有 capture 时的 hover 目标；其他画布只缓存请求。
- IME 使用焦点画布的状态，候选框从 Canvas 局部坐标转为窗口逻辑坐标。
  Canvas 移动时重新转换，失去窗口焦点时关闭窗口 IME。
- `wake_deadline()` 返回所有可用画布的最早唤醒时间，应用将其转换为
  `ControlFlow::WaitUntil`。唤醒后的各 Runtime 时间推进仍归应用。
- tooltip 与 accessibility 保留在各自 Host，消费者用 CanvasId 识别来源。
  tooltip 的局部坐标转换和 accessibility 的原生树组合归应用；
  本增量不声称提供原生 accessibility provider。
- 隐藏、禁用、suspend 和删除清理缓存效果。恢复后消费者需重新发布平台效果。

## 依赖与消费者证据

`canvas` 只引入 `winit-canvas` 的默认功能，不引入其 `gpu` feature。
不开启 `canvas` 时，原有整窗 Host 与依赖边界不变。
`winit-canvas` 继续不依赖 Novadraw；platform adapter 继续不依赖 Vello/WGPU。

跨 crate 消费者为
[双 Canvas Native 示例](../examples/native/winit-canvas-demo/src/main.rs)：
两个画布分别使用 WinitCanvasHost 的 SurfaceInfo 驱动独立的
Novadraw Runtime 和离屏 backend，通过同一 Coordinator 路由和确认重绘。

```sh
cargo test -p novadraw-platform-winit --lib --features canvas
cargo check -p novadraw-platform-winit --lib --features canvas,editor
cargo run -p winit-canvas-demo -- --verify
```

本次验证：workspace check、canvas+editor 的 13 项单元测试、
平台库严格 Clippy、Native 双 Canvas 自检以及文档/格式检查通过。
默认配置不依赖 winit-canvas，canvas 配置不依赖 WGPU/Vello。
运行记录位于 `target/verification/winit-canvas-host/`。
