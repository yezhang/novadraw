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
3. 确认画布显示以下共享树状结构：

```text
Root
├─ A · 1
├─ Group（浅灰容器，不可聚焦）
│  ├─ B · 2
│  ├─ Skip（灰色、disabled）
│  └─ C · 3
└─ D · 4
```

大致位置：

- `A · 1`：左侧，约 `(50, 80)`；
- `B · 2`、`Skip`、`C · 3`：位于中部浅灰 Group 内；
- `D · 4`：右侧，约 `(620, 80)`。

普通可聚焦节点为蓝色，`Skip` 固定为灰色。

数字键使用从 `0` 开始的场景索引：

| 按键 | 场景 |
|------|------|
| `0` | `pointer_capture` |
| `1` | `focus_keyboard` |
| `2` | `wheel_hover_double` |
| `3` | `coordinate_root` |

也可用 `←` / `PageUp` 切换到上一场景，`→` / `PageDown` 切换到下一场景。

### 1.2 正向遍历与边界

1. 按 `0`，再按 `1`，确保重新创建场景并清空 focus owner。
2. 将鼠标移到节点外的背景，确认 `A/B/C/D` 均为蓝色，`Skip` 为灰色。
3. 连续按 `Tab`，逐次确认唯一的紫色 focused 节点依次为：

```text
A · 1 → B · 2 → C · 3 → D · 4
```

4. 确认浅灰 Group 自身从不变紫，但遍历会进入其子节点。
5. 确认灰色 `Skip` 从不变紫，顺序从 `B · 2` 直接进入 `C · 3`。
6. 在 `D · 4` 为紫色时再按一次 `Tab`。
7. 确认 `D · 4` 仍为紫色且没有闪烁：已到 forward boundary，不应循环到 A。

### 1.3 反向遍历

1. 按 `0`，再按 `1`，再次重建场景并清空 focus owner。
2. 按 `Shift+Tab`。
3. 确认 `D · 4` 变为紫色：无 current owner 时从最后一个 candidate 开始。
4. 继续按 `Shift+Tab`，逐次确认顺序为：

```text
D · 4 → C · 3 → B · 2 → A · 1
```

5. 确认 `Skip` 与 Group 均被跳过。
6. 在 `A · 1` 为紫色时再按一次 `Shift+Tab`，确认停留在 A 且不闪烁。

### 1.4 鼠标直接焦点与键盘回归

1. 按 `0`，再按 `1` 重置场景。
2. 把鼠标移入 `B · 2`，确认 B 由蓝色变为绿色。
3. 在 B 内按住鼠标左键，确认 B 变为红色。
4. 松开左键，确认 B 变为紫色：handled press 通过 direct focus 取得焦点。
5. 按字母键 `A`，确认应用保持运行且 probe 保持紫色。
6. 再按 `Tab`，确认焦点从 B 移到 `C · 3`，而不是按鼠标位置或 Z-order 跳转。

### 1.5 Pointer capture 回归

1. 在任一蓝色节点内按住鼠标左键。
2. 保持按下并把指针拖到该节点外。
3. 在节点外松开，确认红色 pressed 状态被清除。
4. 将鼠标移回节点，确认 hover、点击和 Tab 仍可正常工作。

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
2. 场景标题为 `Focus traversal tree`，计数为 `1/1`。
3. 画布显示与 macOS 相同的 `A / Group(B, Skip, C) / D` 结构。
4. `A/B/C/D` 初始为蓝色，`Skip` 为灰色。
5. 右侧 `Pointer` 显示 `Idle`，`Keyboard` 显示 `No key received`。
6. 顶部状态中的 `key N` 记为初始值 `N`。

### 2.2 正向遍历与浏览器边界

1. 点击 canvas 左上方的空白区域，不要点击任何节点。此操作只让 canvas 获得 DOM
   focus，不应给 Figure 设置 focus。
2. 连续按四次 `Tab`，确认紫色节点依次为
   `A · 1 → B · 2 → C · 3 → D · 4`。
3. 每一步确认右侧 `Pointer` 分别显示 `Focused A · 1`、`Focused B · 2`、
   `Focused C · 3`、`Focused D · 4`。
4. 确认 Group 与 `Skip` 始终不会获得紫色焦点。
5. 四次遍历期间浏览器焦点应始终留在 canvas，顶部状态始终为 `key N`。
6. 在 D 为紫色时再按一次 `Tab`。
7. 确认浏览器焦点移动到右侧 `Toggle 1x / 2x DPR` 按钮，D 恢复蓝色，
   `Pointer` 恢复 `Idle`。
8. 确认顶部状态仍为 `key N`。这一步证明 boundary 未被 `preventDefault` 拦截。

### 2.3 反向进入与 backward traversal

此时 DOM focus 应位于 `Toggle 1x / 2x DPR` 按钮：

1. 按一次 `Shift+Tab`，浏览器默认行为把 DOM focus 从按钮移回 canvas；
   此时所有可聚焦节点仍为蓝色。
2. 再按一次 `Shift+Tab`。这次事件由 canvas 收到并转换为 backward traversal。
3. 确认 `D · 4` 变为紫色，右侧 `Pointer` 显示 `Focused D · 4`。
4. 继续按 `Shift+Tab`，确认顺序为 `D · 4 → C · 3 → B · 2 → A · 1`。
5. 确认顶部状态始终为 `key N`。
6. 在 A 为紫色时再按一次 `Shift+Tab`，确认到达 backward boundary 后浏览器焦点
   离开 canvas。

### 2.4 普通键盘事件

1. 按照 2.2 的步骤让任一节点获得焦点。
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
