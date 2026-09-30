# P2-E02b Text Input Bridge 验证记录

类型：`verification`

日期：2026-09-29

结论：`in_progress`

## 1. 完成范围

- Editor 定义 session-tagged `TextInputEvent` 与 `TextInputEffect`；
- acquire、candidate area update、release 与迟到 lease event 拒绝；
- preedit、commit、composition cancel、delete、movement、select-all、accept/cancel；
- Winit `Ime` 与 keyboard input 归一化，IME commit/text 去重；
- Winit logical candidate rectangle 更新和 IME enable/disable；
- Web composition、beforeinput、input、keydown 与 focus-loss 归一化；
- wasm-only hidden textarea host、UTF-16 selection 到 UTF-8 range 转换；
- DOM input 入队后唤醒 composition root，且唤醒前释放 bridge 借用，允许同步回写 effect；
- candidate area 从 canvas logical surface 转换到 browser client coordinates；
- Viewer 默认绘制 selection、caret 与 preedit underline，并由 host 单调时间驱动 blink；
- DOM host 不承载可见文本或业务模型状态；
- Native `node-editor-demo` 提供 F2 rename、IME、accept/cancel 和 undo/redo。
- Web validation 提供 `?mode=direct-edit` 可运行 composition root。

平台 adapter 的 Editor 集成通过可选 `editor` feature 提供；默认依赖图仍只有 Core。

## 2. 自动验证

Suite：`platform.p2-e02-text-input`

结果：PASS：

- Editor direct-edit contract：8 项；
- Winit adapter tests：5 项；
- Web adapter tests：8 项；
- Web DOM host `wasm32-unknown-unknown` 编译：PASS；
- Web validation release bundle：PASS；
- Native node-editor direct-edit application contract：PASS。
- 注入时间下 caret hide/show、preedit 无 caret 和 session cleanup 契约：PASS。
- 单行长文本 viewport clip、caret reveal、selection clip 与目标移动 old/new damage：
  PASS。

## 3. 平台验收

2026-09-30 验收结果：

- macOS 14.7.8 Native/Vello 与系统输入法：PASS（用户人工确认）；
- Google Chrome 154.0.8037.58 Web/Vello 桌面与移动输入法：PASS（用户人工确认）；
- Web/Canvas2D 自动浏览器复核：点击启动、Unicode draft、accept、cancel、focus
  release、无应用控制台错误与非空 canvas 像素均 PASS。

完整步骤和环境边界见
[`../manual/p2-direct-text-edit.md`](../manual/p2-direct-text-edit.md)。

## 4. 后续复验

2026-09-30 后续人工复验发现旧构建存在首帧背景、编辑文本定位与 caret 可见性回归。
修正版已通过 Web Vello 自动视觉复核；P2-E02 保持 `in_progress`，等待 Native/Web
人工复验后恢复 `complete`。
