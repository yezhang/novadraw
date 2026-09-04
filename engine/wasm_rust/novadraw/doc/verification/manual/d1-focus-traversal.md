# D1.5c 焦点遍历手工验收

类型：`verification`

状态：`pending`

本流程验证 Native/Web 的 Tab/Shift+Tab 平台适配、焦点边界和普通 key 隔离。

## 1. macOS

### 1.1 启动并进入目标场景

```bash
cargo run -p event-app
```

启动后窗口默认显示最后一个场景，标题应为
`Event Pipeline Verification - coordinate_root`。

1. 按数字键 `1`。
2. 确认窗口标题变为 `Event Pipeline Verification - focus_keyboard`。
3. 确认画布中有一个约位于 `(250, 180)`、尺寸约 `300 × 200` 的蓝色 probe。

数字键使用从 `0` 开始的场景索引：

| 按键 | 场景 |
|------|------|
| `0` | `pointer_capture` |
| `1` | `focus_keyboard` |
| `2` | `wheel_hover_double` |
| `3` | `coordinate_root` |

也可用 `←` / `PageUp` 切换到上一场景，`→` / `PageDown` 切换到下一场景。

### 1.2 正向遍历与边界

1. 按 `0`，再按 `1`，确保重新创建 `focus_keyboard` 场景并清空 focus owner。
2. 将鼠标移到 probe 外的灰色背景，例如窗口内约 `(100, 100)`，确认 probe 为蓝色。
3. 按一次 `Tab`。
4. 确认 probe 变为紫色，表示它通过 forward traversal 获得焦点。
5. 再按一次 `Tab`。
6. 确认 probe 仍为紫色且没有颜色闪烁：当前只有一个 candidate，第二次 Tab 到达
   boundary，不应循环触发 lost/gained。

### 1.3 反向遍历

1. 按 `0`，再按 `1`，再次重建场景并清空 focus owner。
2. 按 `Shift+Tab`。
3. 确认 probe 变为紫色：无 current owner 时，backward traversal 从最后一个
   candidate 开始。
4. 再按 `Shift+Tab`，确认 probe 仍为紫色且不重复切换。

### 1.4 鼠标直接焦点与键盘回归

1. 按 `0`，再按 `1` 重置场景。
2. 把鼠标移入 probe，确认颜色由蓝色变为绿色。
3. 在 probe 内按住鼠标左键，确认颜色变为红色。
4. 松开左键，确认颜色变为紫色：handled press 通过 direct focus 取得焦点。
5. 按字母键 `A`，确认应用保持运行且 probe 保持紫色。
6. 再按 `Tab`，确认到达 boundary 后 probe 仍保持紫色。

### 1.5 Pointer capture 回归

1. 在 probe 内按住鼠标左键。
2. 保持按下并把指针拖到 probe 外。
3. 在 probe 外松开，确认红色 pressed 状态被清除。
4. 将鼠标移回 probe，确认 hover、点击和 Tab 仍可正常工作。

## 2. Web

### 2.1 构建并启动

```bash
./scripts/build_web_validation.sh
./scripts/serve_web_validation.sh
```

若默认端口 `4173` 已占用，可使用：

```bash
PORT=4174 ./scripts/serve_web_validation.sh
```

打开：

```text
http://127.0.0.1:4173/?backend=vello&theme=input&scene=0
```

页面加载完成后确认：

1. 顶部选中的主题为 `Input`。
2. 场景标题为 `Pointer, keyboard and wheel`，计数为 `1/1`。
3. 画布内 probe 初始为蓝色。
4. 右侧 `Pointer` 显示 `Idle`，`Keyboard` 显示 `No key received`。
5. 顶部状态中的 `key N` 记为初始值 `N`。

### 2.2 正向遍历与浏览器边界

1. 点击 canvas 左上方的空白区域，不要点击 probe。此操作只让 canvas 获得 DOM
   focus，不应给 Figure 设置 focus。
2. 按一次 `Tab`。
3. 确认 probe 变为紫色，右侧 `Pointer` 变为 `Focused`。
4. 确认浏览器焦点仍在 canvas 内，没有移动到右侧按钮。
5. 确认顶部状态仍为 `key N`，Tab 没有进入普通 key callback。
6. 再按一次 `Tab`。
7. 确认浏览器焦点移动到右侧 `Toggle 1x / 2x DPR` 按钮，probe 恢复蓝色，
   `Pointer` 恢复 `Idle`。
8. 确认顶部状态仍为 `key N`。这一步证明 boundary 未被 `preventDefault` 拦截。

### 2.3 反向进入与 backward traversal

此时 DOM focus 应位于 `Toggle 1x / 2x DPR` 按钮：

1. 按一次 `Shift+Tab`，浏览器默认行为把 DOM focus 从按钮移回 canvas；probe
   此时仍为蓝色。
2. 再按一次 `Shift+Tab`。这次事件由 canvas 收到并转换为 backward traversal。
3. 确认 probe 变为紫色，右侧 `Pointer` 显示 `Focused`。
4. 确认顶部状态仍为 `key N`。
5. 再按一次 `Shift+Tab`，确认到达 backward boundary 后浏览器焦点离开 canvas。

### 2.4 普通键盘事件

1. 按照 2.2 的步骤让 probe 再次获得焦点。
2. 记录当前顶部 `key N`。
3. 按下并松开字母键 `A`。
4. 确认右侧 `Keyboard` 显示 `Character('a')`。
5. 确认顶部 key 计数增加 2，分别对应 pressed 和 released。

### 2.5 Canvas2D 对照

打开：

```text
http://127.0.0.1:4173/?backend=canvas2d&theme=input&scene=0
```

重复 2.2 至 2.4，确认 traversal、boundary、状态文字和计数与 Vello WebGPU 一致。
若服务运行在 `4174`，将以上 URL 的端口同步改为 `4174`。

## 3. 通过条件

- forward/backward 顺序符合 tree-order policy；
- traversal 成功时同一次 Tab 不再作为普通 key 投递；
- Web 仅在 `Moved` 时阻止默认行为，`Boundary` 能离开 canvas；
- blur 后 focus owner 被清除且只产生一次 FocusLost；
- 无重复 focus event、状态泄漏或渲染残影。

验收后记录：

```text
D1.5c: PASS
平台: macOS / Web
失败项: 无
```

## 4. 验收记录

- 日期：
- 平台：
- 结果：
- 失败项：
