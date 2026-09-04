# D1.5c 焦点遍历手工验收

类型：`verification`

状态：`pending`

本流程验证 Native/Web 的 Tab/Shift+Tab 平台适配、焦点边界和普通 key 隔离。

## 1. macOS

启动：

```bash
cargo run -p event-app
```

切换到 `focus_keyboard` 场景，然后验证：

1. 场景初始无 Figure focus 时按 Tab，probe 获得焦点并显示 focused 状态。
2. 再按 Tab 到达 traversal boundary，不循环回自身，也不产生普通 Tab key event。
3. 让窗口失焦再恢复，按 Shift+Tab，probe 作为最后一个 traversal candidate 获得焦点。
4. 点击 probe 后输入 `A`，probe 收到普通 key pressed/released。
5. 点击、Tab 和 Shift+Tab 不影响 pointer capture、hover 或 wheel 行为。

## 2. Web

构建并启动：

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

验证：

1. canvas 获得 DOM focus 后按 Tab，probe 获得引擎 focus，浏览器不离开 canvas。
2. 再按 Tab 到达 boundary，浏览器按默认顺序把 DOM focus 移出 canvas。
3. 反向进入 canvas 时按 Shift+Tab，probe 获得引擎 focus。
4. Tab keydown/keyup 不增加页面的普通 key event 计数。
5. 字符键仍只投递给当前 focus owner。
6. Canvas2D 对照入口行为一致：
   `http://127.0.0.1:4173/?backend=canvas2d&theme=input&scene=0`。

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
