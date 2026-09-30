# P2 Direct Text Edit / IME 人工验收

类型：`manual-verification`

状态：`complete`

## Native macOS

启动：

```bash
cargo xtask manual platform.p2-e02-text-input
```

验收步骤：

1. 单击蓝色或绿色节点并按 `F2`，窗口标题出现 `TEXT EDIT`；
2. 输入英文，文本仅在编辑 feedback 中变化；
3. 使用系统拼音输入法输入中文，preedit 与候选窗跟随 caret；
4. 使用方向键、Option + 左右键和 Shift 扩展选择；
5. 按 Enter 接受，节点稳定文本更新；Command-Z / Shift-Command-Z 可撤销和重做；
6. 再次按 `F2`，按 Escape 取消，模型文本不变；
7. 编辑期间滚动、缩放和 resize，文本与候选窗保持对齐；
8. 编辑期间切换窗口焦点，session、feedback 与 IME 均按 focus-loss policy 清理。

## Web

启动：

```bash
cargo xtask verify web.build
cargo xtask manual web.build
```

打开
<http://127.0.0.1:4173/?mode=direct-edit&backend=vello>。该入口创建
`WebTextInputHost`，将 `GraphicalViewer::take_text_input_effects()` 逐项传给 host，
并把 `WebTextInputHost::take_events()` 逐项交给
`EditorDomain::handle_text_input_event()`。

验收步骤：

1. hidden textarea 获取焦点但不可见、不遮挡 canvas；
2. 中文或日文 composition 不产生重复文本；
3. emoji、Backspace/Delete、方向键、Shift selection 与全选可用；
4. composition 后的 `input` 不重复提交；
5. canvas resize、page scroll 与 DPR 变化后 host 跟随 caret；
6. accept/cancel 后 textarea blur，迟到事件不修改新 session。

## 通过记录

日期：2026-09-30

| 平台 | 环境 | 结果 |
|---|---|---|
| Native | macOS 14.7.8、Vello、系统输入法 | PASS（用户人工确认 8 项全部通过） |
| Web | Google Chrome 154.0.8037.58、Vello、桌面与移动输入法 | PASS（用户人工确认 6 项全部通过） |
| Web 自动复核 | Chrome、Canvas2D 诊断后端 | PASS |

Web 自动复核覆盖：

- canvas 点击启动 session，hidden textarea 获得焦点；
- Unicode draft 更新但提交前模型保持不变；
- Enter 接受后模型更新，textarea blur 且 lease release；
- Escape 取消后丢弃 draft，已提交模型不变；
- host wake 按 Viewer deadline 驱动 caret blink，session 结束后 deadline 清空；
- canvas 像素采样非空，页面无应用控制台错误；
- candidate area 加上 canvas client origin，resize 时重新同步。

自动化环境无法完成 WebGPU 初始化，因此 Canvas2D 只用于 DOM/Editor 链路和非空像素
复核；Vello 文本、caret、selection、preedit 与输入法候选窗以人工验收结果为准。
