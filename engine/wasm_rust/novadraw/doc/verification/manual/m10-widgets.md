# M10.4 基础控件人工验收

类型：`verification`

## 验收信息

- 日期：2026-09-08
- 平台：macOS
- 应用：`examples/native/widgets-app`
- 基线：`7b3a033`
- 结果：PASS

## 验收范围

- Button normal、pressed、focus、disabled visual；
- Toggle selected/unselected visual；
- pointer click；
- press 后拖出取消；
- 拖回控件后恢复并在 release 时触发；
- Tab/Shift+Tab 焦点遍历；
- Enter/Space 键盘激活。

## 结果

全部检查通过，无失败项。M10.4 的契约测试、截图与人工窗口验证闭合，可标记
`complete`。
