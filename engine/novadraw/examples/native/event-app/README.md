# Event App - 事件管线验证

`event-app` 验证 Draw2D 风格的单目标事件分发、鼠标捕获、焦点与键盘路由、
扩展鼠标事件以及坐标根事件点降域。Novadraw 不采用 DOM 的捕获阶段和冒泡阶段。

## 运行

```bash
cargo run -p event-app
cargo run -p event-app -- --verify
cargo run -p event-app -- --screenshot-all
```

## 场景

| 场景 | 名称 | 验证内容 |
|---|---|---|
| 0 | `pointer_capture` | enter/exit、press/release、拖出边界后的 mouse capture |
| 1 | `focus_keyboard` | 点击取得焦点、键盘投递、焦点释放 |
| 2 | `wheel_hover_double` | wheel、hover、double-click |
| 3 | `coordinate_root` | 嵌套坐标根命中和 target-domain 事件点 |
| 4 | `hover_highlight` | 鼠标进入卡片后显示蓝色高亮边框，移开后恢复 |

窗口中按 `Home` 回到场景 0，按数字键 `0`-`4` 切换场景，按 `Esc` 退出。

按 `4` 后，将鼠标移入中央的浅蓝色卡片。卡片会显示 4 像素宽的蓝色边框；
移出卡片或窗口后，边框消失。边框由 `Entered` / `Exited` 事件切换，
通过 repaint 更新，不需要点击。

## Verification

```bash
cargo run -p event-app -- --verify \
  --report=target/visual-verification/event-app.json
```

通过时输出：

```text
PASS pointer_capture
PASS pointer_leave_cleanup
PASS focus_keyboard
PASS wheel_hover_double
PASS coordinate_root
PASS hover_highlight
```

完整的人工动作、颜色状态和通过标准见
[`doc/verification/manual/core-pipeline.md`](../../../doc/verification/manual/core-pipeline.md)。
