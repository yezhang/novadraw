# P2 TextFlow 交互与直接编辑验证计划

类型：`verification-plan`

状态：`complete`

范围：P2-T02、P2-E02

规范入口：

- [`../../design/editor/p2-direct-text-edit.md`](../../design/editor/p2-direct-text-edit.md)
- [`../../design/architecture/text-layout.md`](../../design/architecture/text-layout.md)

本页定义完成证据。P2-T02 与 P2-E02a 的实际 command 和 suite 已注册到
`verification/suites.toml`；P2-E02b 不注册不可执行的占位门禁。

## 1. 计划 Suite

| Suite ID | 层 | 平台 | 目标 |
|---|---|---|---|
| `core.p2-t02-text-interaction` | contract | headless | `complete`：文档位置、caret、selection 与 movement 几何 |
| `editor.p2-e02-direct-text-edit` | contract/application | headless | `complete`：session、draft、policy、Command 与 cleanup |
| `platform.p2-e02-text-input` | platform/application | native-macos、web | `complete`：自动门禁与 Native/Web 人工复验通过 |

## 2. Core 契约矩阵

必须覆盖：

1. ASCII、CJK、emoji、combining mark 与多 code-point grapheme；
2. 非 UTF-8 boundary、越界 paragraph/offset 和 stale layout revision 拒绝；
3. soft wrap 两侧 affinity、hard break、空 paragraph 和 paragraph boundary；
4. LTR、RTL 与 mixed bidi 的 logical position/visual caret 双向映射；
5. point hit-test、visual left/right、word、line、paragraph 和 document movement；
6. 同行、跨行、跨 fragment、跨 paragraph selection quad 顺序；
7. truncate 后不可见位置拒绝，visible range 仍可稳定定位；
8. nested transform、viewport scroll/zoom 后 local 到 surface caret rectangle；
9. 相同 layout/revision 查询确定，纯查询不产生 validation 或 damage；
10. 外部 TextLayoutEngine 的 `TextInteractionProvider` 校验与非 Parley 实现路径。

## 3. Editor 契约矩阵

必须覆盖：

1. request location/feature 解析与 unsupported feature；
2. 同一 Viewer 只允许一个 active session，foreign/retired part 拒绝；
3. session 启动固定 source、feature、source revision 与 host lease；
4. click/drag selection、Shift 扩展、visual movement 和 grapheme-safe delete；
5. Viewer selection 与 text selection 隔离；
6. draft 变化只更新 feedback，不修改应用模型或 history；
7. 无变化 accept 不生成 Command；
8. accept 前清理 feedback，Command 修改模型，通知刷新稳定 Figure；
9. cancel、Escape、source retire、Viewer dispose 和 Editor fault 全量清理；
10. commit/undo/redo 不复用旧 EditPartId、FigureId 或 session identity；
11. stale source revision 拒绝覆盖，recoverable rejection 保留 draft；
12. cleanup failure 进入 faulted，不留下 active host lease；
13. multiline Enter、显式 accept shortcut 和 focus-loss policy；
14. caret blink 使用注入时间，headless replay 不依赖 wall clock。

## 4. IME 序列矩阵

归一化 replay 至少覆盖：

```text
enable
-> preedit("n")
-> preedit("ni", caret=2)
-> preedit("你", selection)
-> clear preedit
-> insert_text("你")
```

以及：

- preedit 替换已有 selection；
- preedit cursor range 为 `None` 时隐藏 caret；
- composition cancel 恢复 composition base；
- IME text commit 只进入 draft，不 accept 编辑会话；
- composition 中不重复消费 keyboard text；
- lease release 后的迟到 preedit/commit 被拒绝；
- focus lost 与 composition end 到达顺序变化；
- dead key 组合字符不重复插入；
- invalid relative UTF-8 range 结构化拒绝。

## 5. Native 人工验收

macOS Native/Vello：

1. 单击已选文本 feature 与显式 rename action 均能启动；
2. 英文、简体中文拼音、日文 composition 的候选窗贴近 caret；
3. 左右移动、跨行上下移动、Shift selection 与 bidi caret 可辨识；
4. scroll/zoom/resize 后 editor、selection、caret 和候选窗继续对齐；
5. accept 后只产生一个 undoable Command；undo/redo 文本和布局同步；
6. cancel、窗口失焦、删除目标和关闭 Viewer 后无残留 feedback 或 IME；
7. composition 期间 Enter/Escape 不误触发重复 accept/cancel。

## 6. Web 人工验收

浏览器：

1. hidden input host 能获得 composition、beforeinput/input 和软键盘输入；
2. DOM 文本不可见，不遮挡 canvas，也不成为业务模型真值；
3. 中文/日文 composition、emoji 与移动端软键盘至少各验证一个路径；
4. 浏览器自动更正或不可取消 input 通过 value reconcile 得到唯一 draft；
5. page scroll、canvas resize 与 device pixel ratio 变化后 caret host 对齐；
6. accept/cancel 后 DOM focus、selection 和 composition 状态被释放；
7. Native 与 Web 对同一 normalized replay 产生相同 draft 和 Command。
8. `text-input=edit-context` 时 canvas 直接持有 `EditContext`，不创建隐藏 textarea；
9. EditContext 的 UTF-16 buffer/selection/composition 原子同步到 Editor UTF-8 状态；
10. `characterboundsupdate` 使用 TextFlow 几何同步回应，不使用平均字宽近似；
11. EditContext 不可用或初始化失败时回退 textarea host。

## 7. 完成门禁

P2-T02 complete：

- Core suite 全部通过；
- public API 不暴露 Parley 类型；
- interaction map 与绘制 layout revision 一致；
- Draw2D parity `text.interaction` 提升为 `verified`。

P2-E02 complete：

- 三个计划 suite 均进入 `verification/suites.toml` 并通过；
- Native/Web 人工验收有日期、平台和原始结果；
- GEF parity `direct_edit` 提升为 `verified`；
- `cargo xtask check --quick` 通过；
- 最终提交边界运行一次 `cargo xtask check --full`。

P2-E02a 当前证据：

- typed feature、descriptor、policy plan 与 Viewer-scoped 单 session；
- draft、selection、preedit base/恢复和 TextFlow interaction geometry；
- accept 前 feedback cleanup、单 Command、undo/redo 与无变化 accept；
- stale revision、recoverable rejection、unsupported feature 和 source retire；
- `cargo xtask verify editor.p2-e02-direct-text-edit`：PASS。

P2-E02b 当前自动证据：

- session-tagged event/effect 与 stale lease 拒绝；
- Winit IME/keyboard bridge 与 logical candidate area；
- Web hidden textarea、composition/beforeinput/input/keydown/focus bridge；
- DOM UTF-16 selection 到 Editor UTF-8 range 转换；
- Web compositionupdate 的 provisional text 与 post-DOM input selection 分代处理，
  `pinyin` 末尾 caret 校准为 UTF-8 `6..6`；
- Viewer 默认 selection/caret/preedit 装饰与 host 注入单调时间 caret blink；
- Native node-editor F2 rename 与 undo/redo application contract；
- Web validation `?mode=direct-edit` composition root、canvas start/caret selection、
  event/effect drain、surface-origin 同步与可观测 host lease；
- Web validation `?mode=direct-edit&text-input=edit-context` 复用同一 Editor composition
  root，覆盖 canvas attachment、完整 draft/selection 同步和 textarea fallback；
- `cargo xtask verify platform.p2-e02-text-input`：PASS。

P2-E02 长文本编辑视口必须补充：

- 单行 draft 超过目标宽度后继续输入，不产生 ellipsis；
- 文本、selection、preedit 与 caret 统一裁剪到目标 client area；
- caret 位于首部、中部和尾部时，内部 scroll offset 分别保证其可见；
- 内容滚动只重绘编辑视口，不在 viewport 外留下 glyph；
- target、ancestor、viewport、zoom 或 resize 变化后，旧、新 presentation damage
  完整且所有反馈使用同一 generation；
- 编辑会话与节点拖动互斥；accept 失败时不启动拖动；
- `PreserveBounds` 提交后稳定 Label 可以省略，但再次编辑恢复完整模型文本；
- 可选 `AutoSizeOnCommit` 将文本和 bounds 作为一个 Command 撤销/重做。

P2-E02 人工证据：

- 2026-09-30 后续复验发现 Native/Web 首帧背景、编辑文本定位与 caret 可见性回归；
- 修正版 Web Vello 自动视觉复核：PASS；
- 修正版 Native macOS/Vello 人工复验：PASS；
- 修正版 Web Chrome/Vello 系统拼音人工复验：PASS，`pinyin` caret 始终位于末尾；
- 详细环境与自动浏览器复核见
  [`../manual/p2-direct-text-edit.md`](../manual/p2-direct-text-edit.md)。
