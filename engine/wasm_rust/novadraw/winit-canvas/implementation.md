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

`novadraw-backend-vello/src/texture.rs` 提供 `VelloTextureRenderer` 和
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
