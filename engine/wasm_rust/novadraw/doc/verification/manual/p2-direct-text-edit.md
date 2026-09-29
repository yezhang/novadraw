# P2 Direct Text Edit / IME 人工验收

类型：`manual-verification`

状态：`pending`

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

Web 产品 composition root 应创建 `WebTextInputHost`，将
`GraphicalViewer::take_text_input_effects()` 逐项传给 host，并把
`WebTextInputHost::take_events()` 逐项交给 `EditorDomain::handle_text_input_event()`。

验收步骤：

1. hidden textarea 获取焦点但不可见、不遮挡 canvas；
2. 中文或日文 composition 不产生重复文本；
3. emoji、Backspace/Delete、方向键、Shift selection 与全选可用；
4. composition 后的 `input` 不重复提交；
5. canvas resize、page scroll 与 DPR 变化后 host 跟随 caret；
6. accept/cancel 后 textarea blur，迟到事件不修改新 session。

## 通过记录

人工执行后记录日期、系统、浏览器/输入法版本及失败项。Native 与 Web 均通过前，
P2-E02 保持 `in_progress`。
