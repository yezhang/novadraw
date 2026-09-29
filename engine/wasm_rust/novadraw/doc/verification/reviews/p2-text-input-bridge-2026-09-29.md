# P2-E02b Text Input Bridge 验证记录

类型：`verification`

日期：2026-09-29

结论：自动门禁通过，Native/Web 人工验收待执行

## 1. 完成范围

- Editor 定义 session-tagged `TextInputEvent` 与 `TextInputEffect`；
- acquire、candidate area update、release 与迟到 lease event 拒绝；
- preedit、commit、composition cancel、delete、movement、select-all、accept/cancel；
- Winit `Ime` 与 keyboard input 归一化，IME commit/text 去重；
- Winit logical candidate rectangle 更新和 IME enable/disable；
- Web composition、beforeinput、input、keydown 与 focus-loss 归一化；
- wasm-only hidden textarea host、UTF-16 selection 到 UTF-8 range 转换；
- DOM host 不承载可见文本或业务模型状态；
- Native `node-editor-demo` 提供 F2 rename、IME、accept/cancel 和 undo/redo。

平台 adapter 的 Editor 集成通过可选 `editor` feature 提供；默认依赖图仍只有 Core。

## 2. 自动验证

Suite：`platform.p2-e02-text-input`

结果：PASS：

- Editor direct-edit contract：6 项；
- Winit adapter tests：5 项；
- Web adapter tests：8 项；
- Web DOM host `wasm32-unknown-unknown` 编译：PASS；
- Native node-editor direct-edit application contract：PASS。

## 3. 未关闭项

以下项目需要真实窗口、系统输入法和浏览器人工验证：

- macOS 拼音/日文候选窗位置与 composition 顺序；
- Native resize、scroll、zoom 后候选窗对齐；
- 浏览器 hidden textarea focus、移动端软键盘与不可取消 input；
- Web page scroll、DPR 和 canvas resize 后候选窗对齐。

人工清单见
[`../manual/p2-direct-text-edit.md`](../manual/p2-direct-text-edit.md)。两端人工证据完成前，
P2-E02 与 GEF `direct_edit` 保持 `in_progress` / `partial`。
