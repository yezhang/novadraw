# P2 Direct Text Edit / IME 人工验收

类型：`manual-verification`

状态：`in_progress`

## Native macOS

启动：

```bash
cargo xtask manual platform.p2-e02-text-input
```

验收步骤：

1. 启动后无需点击，蓝色与绿色节点背景均立即可见；
2. 单击蓝色或绿色节点并按 `F2`，窗口标题出现 `TEXT EDIT`，文字保持水平和垂直居中，
   caret 在文本末尾可见并持续闪烁；
3. 连续输入超过节点宽度的英文，文本仅在编辑 feedback 中变化；文字不越过矩形边界，
   不出现省略号，内容在矩形内水平滚动且 caret 始终可见；
4. 使用系统拼音输入法输入中文，preedit 与候选窗跟随 caret；
5. 使用方向键、Option + 左右键和 Shift 扩展选择；
6. 按 Enter 接受，节点稳定文本更新；Command-Z / Shift-Command-Z 可撤销和重做；
7. 再次按 `F2`，按 Escape 取消，模型文本不变；
8. 编辑期间滚动、缩放和 resize，文本与候选窗保持对齐；
9. 编辑期间切换窗口焦点，session、feedback 与 IME 均按 focus-loss policy 清理；
10. 提交超长文本后，节点尺寸保持不变，稳定标签可以显示省略号；再次按 `F2` 时恢复
    完整文本并可继续编辑。

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

1. 页面加载后无需点击，标签背景立即为蓝色；
2. 单击标签后文字保持水平和垂直居中，caret 在文本末尾可见并持续闪烁；
3. 输入超过标签宽度的文本，文字不越界、不出现省略号，内容在标签内滚动且 caret
   保持可见；
4. hidden textarea 获取焦点但不可见、不遮挡 canvas；
5. 中文或日文 composition 不产生重复文本；
6. emoji、Backspace/Delete、方向键、Shift selection 与全选可用；
7. composition 后的 `input` 不重复提交；
8. canvas resize、page scroll 与 DPR 变化后 host 跟随 caret；
9. accept/cancel 后 textarea blur，迟到事件不修改新 session。

## 通过记录

日期：2026-09-30

| 平台 | 环境 | 结果 |
|---|---|---|
| Native | macOS 14.7.8、Vello、系统输入法 | 待复验：旧构建首帧缺少节点底色，F2 后文字未居中且 caret 不可见 |
| Web | Google Chrome 154.0.8037.58、Vello | 自动视觉复核 PASS；等待用户复验 |
| Web 自动复核 | Chrome、Vello WebGPU 与 Canvas2D 诊断后端 | PASS |

Web 自动复核覆盖：

- Vello 首帧直接显示蓝底白字，不依赖点击刷新；
- Vello 编辑态文字保持水平和垂直居中；
- Vello caret 的显示与隐藏帧均可观察，frame/wake 计数持续推进；
- Vello 超长 draft 保持完整，Home/End 后 candidate area 分别停在标签左右边界内；
- canvas 点击启动 session，hidden textarea 获得焦点；
- Unicode draft 更新但提交前模型保持不变；
- Enter 接受后模型更新，textarea blur 且 lease release；
- Escape 取消后丢弃 draft，已提交模型不变；
- host wake 按 Viewer deadline 驱动 caret blink，session 结束后 deadline 清空；
- canvas 像素采样非空，页面无应用控制台错误；
- candidate area 加上 canvas client origin，resize 时重新同步。

2026-09-30 用户复验发现旧构建存在首帧背景、编辑文本定位和 caret 可见性回归。
修正版已完成 Web Vello 自动视觉复核；Native Vello 与真实输入法仍需按上述步骤复验。
