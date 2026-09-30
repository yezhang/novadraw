# Web EditContext 集成验证

类型：`verification`

日期：2026-09-30

结论：`partial`

## 1. 结论

`EditContext` 已接入 Novadraw Web direct-edit：支持该 API 时可直接把系统文本服务
绑定到 canvas，并通过既有 `EditorDomain`、`DirectTextEditState`、TextFlow feedback
和模型 Command 完成编辑。它不是独立状态机，也不创建隐藏 `textarea`。

当前不能删除 `WebTextInputHost`：

- `EditContext` 仍是 limited availability / experimental，Firefox 与 Safari 未提供
  可用实现；
- 当前 `web-sys 0.3.104` 没有生成这些实验性类型，平台 crate 使用局部
  `wasm-bindgen` 绑定，不能把类型泄漏到 Editor 公共契约；
- `textformatupdate` 的 composition 样式仍未映射，当前沿用统一 preedit 下划线。

因此可接受的产品结构是运行时 feature detection：支持且适配完整时优先使用
EditContext，否则回退到隐藏 textarea。两种 host 必须归一化到同一个
session-tagged Editor 协议。

## 2. 集成入口

集成入口：

<http://127.0.0.1:4173/?mode=direct-edit&backend=vello&text-input=edit-context>

源码：

- [`../../../novadraw-platform-web/src/edit_context.rs`](../../../novadraw-platform-web/src/edit_context.rs)
- [`../../../novadraw-platform-web/src/text_input.rs`](../../../novadraw-platform-web/src/text_input.rs)
- [`../../../examples/web/web-validation/src/direct_edit_mode.rs`](../../../examples/web/web-validation/src/direct_edit_mode.rs)

会话启动后实际执行：

```javascript
canvas.editContext = editContext;
```

并形成以下闭环：

- Viewer 的 `DirectTextEditState::input_snapshot()` 把 draft、selection 和
  composition 同步到 EditContext；
- `textupdate` 的 UTF-16 全缓冲区状态转换为 `TextInputEvent::Synchronize`，由
  `EditorDomain::handle_text_input_event()` 原子更新会话；
- Viewer 重新生成 TextFlow、selection、preedit 和 caret feedback；
- `updateControlBounds()` 与 `updateSelectionBounds()` 接收转换后的浏览器客户区
  矩形；
- `characterboundsupdate` 的 UTF-16 code unit 范围转换为 UTF-8 range，并通过
  `GraphicalViewer::direct_text_range_bounds()` 查询 TextFlow 几何；
- Accept 仍生成一个模型 Command，Cancel 仍丢弃草稿；
- 不支持或初始化失败时创建原 `WebTextInputHost`。

## 3. 自动验证结果

环境：

- TraeCN 1.107.1；
- Electron 39.2.7；
- Chromium 142.0.7444.235；
- macOS；
- localhost secure context。

结果：

| 检查项 | 结果 |
|---|---|
| `window.EditContext` 构造器 | PASS |
| `HTMLElement.prototype.editContext` | PASS |
| direct-edit 选择 `WebEditContextHost` | PASS |
| 点击节点启动 Viewer session | PASS |
| `attachedElements()` 返回 canvas | PASS |
| 初始 draft / selection 同步 | PASS |
| DOM `textarea` 数量 | PASS，`0` |
| Canvas 像素输出 | PASS，非空 |
| 页面控制台 | PASS，无当前页面错误 |
| textarea direct-edit 回归 | PASS |
| Editor snapshot / UTF-16 转换单元测试 | PASS |
| 自动化键盘触发可信 `textupdate` | 未覆盖：当前浏览器自动化键盘未进入系统文本服务 |
| 系统 IME composition 与候选窗 | 待人工验收 |
| `characterboundsupdate` 真实请求 | 待人工验收 |

自动化结果证明 EditContext 已进入真实 Novadraw direct-edit 组合根，而不只是独立
Canvas 探针。它仍不能替代真实操作系统输入法验收。

## 4. 已落实的适配契约

### 文本更新

`EditContext.textupdate` 给出：

- `updateRangeStart/updateRangeEnd`；
- replacement `text`；
- 更新后的 `selectionStart/selectionEnd`。

这些位置是 JavaScript 字符串的 UTF-16 offset。`WebEditContextBridge` 将完整
EditContext buffer、selection 与 composition 转换为 UTF-8 `TextInputSnapshot`；
Editor 在一个状态替换中应用快照，避免可观察的半更新状态。

### 几何回传

EditContext 要求应用维护三类浏览器客户区几何：

- `updateControlBounds`：编辑区域；
- `updateSelectionBounds`：selection 或 caret；
- `updateCharacterBounds`：输入法请求范围内的逐字符矩形。

control bounds 来自 canvas 客户区，selection bounds 来自 Viewer 的可见 caret。
character bounds 由 Viewer 查询当前 TextFlow selection geometry，平台层只负责
UTF-16/UTF-8 range 与浏览器客户区坐标转换，不使用平均字宽近似。

### 组合样式

`textformatupdate` 可以表达输入法要求的下划线样式和粗细。当前 Editor 只绘制统一的
preedit 下划线；若正式采用 EditContext，需要决定哪些格式进入平台无关反馈协议。

## 5. 人工验收

完整步骤见
[`../manual/p2-direct-text-edit.md`](../manual/p2-direct-text-edit.md) 的
“Web EditContext 集成”。

重点验证：

1. 真实 ASCII、emoji 输入产生 `textupdate`；
2. 拼音或日文输入产生 composition start/end；
3. 输入法请求 `characterboundsupdate` 后候选窗稳定贴合 caret；
4. resize、page scroll 和 DPR 变化后几何继续对齐；
5. Enter/Escape 和 undo/redo 保持原有模型命令语义；
6. 不支持 EditContext 时现有 textarea host 仍可用。

## 6. 参考

- [MDN EditContext](https://developer.mozilla.org/en-US/docs/Web/API/EditContext)
- [MDN Using the EditContext API](https://developer.mozilla.org/en-US/docs/Web/API/EditContext_API/Guide)
- [W3C EditContext Working Draft](https://www.w3.org/TR/edit-context/)
