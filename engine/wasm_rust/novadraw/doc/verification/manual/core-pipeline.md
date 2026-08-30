# M1-M7 核心渲染管线手工验证

类型：`verification`

本文给出 Novadraw 核心通用机制的最终态人工验收路径，覆盖 M1-M7 的绘制、
Figure 树、裁剪、坐标域、事件分发、两阶段更新与通知。

本流程不验收 M8-M10 的 Viewport、Connection、文本或控件能力，也不以
`editor` 中的业务场景代替引擎契约验证。

本文是可重复执行的指导手册，不是某次验收已经通过的记录。实际签收必须记录
commit、平台、图形设备、命令结果和窗口观察结论。

## 1. 验收原则

一次有效验收必须同时包含三类证据：

| 证据 | 目的 | 判定方式 |
|---|---|---|
| 构建与测试 | 排除基础编译、静态检查和契约回归 | 命令退出码为 0 |
| 无窗口 verification | 验证肉眼不可见的时序、坐标和状态机 | 所有 case 输出 `PASS`，JSON 中 `passed` 为 `true` |
| 窗口手工操作 | 验证宿主、GPU 提交和用户可见闭环 | 按本文动作观察画面，无空白、残影、错位或状态丢失 |

静态截图只能证明某一帧的绘制结果，不能单独证明事件顺序、鼠标捕获、
Validation 先于 Damage Repair、dirty 合并或 panic 恢复。

### 1.1 M1-M7 证据映射

| Milestone | 核心契约 | 自动证据 | 人工验收入口 |
|---|---|---|---|
| M1 | 几何、Graphics 状态栈、基础命令 | geometry/render 单元测试与 `m1_product_existence` | `update-app`、`clip-app` |
| M2 | Figure 树、盒模型、生命周期、z-order | graph 单元测试与 `m2_product_existence` | `clip-app` 的层级与绘制顺序 |
| M3 | 递归 paint、祖先裁剪、状态隔离 | paint/hit-test/clip restore 回归测试 | `clip-app` 场景 1、2 |
| M4 | 坐标根、往返转换、事件点降域 | `m4_coordinate_contract` | `transform-app` 四个场景 |
| M5 | 六布局、Validation、UpdateManager、damage | `m5_layout_contract`、`update-app --verify` | `update-app` 四个场景 |
| M6 | hit-test、capture、hover、focus、key/wheel | `m6_event_contract`、`event-app --verify` | `event-app` 四个场景 |
| M7 | typed listener、事务通知因果顺序 | deferred update 单元测试、`notification_order` | 以无窗口报告为主，窗口状态变化作辅助证据 |

人工观察只签收用户可见闭环；涉及队列、因果顺序和 target-domain 数值的契约，
必须以自动测试或 verification 报告为准。

## 2. 统一工作目录

以下命令均在 workspace 根目录执行：

```bash
cd /Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw
```

窗口 app 使用系统图形设备。通过 SSH 或无图形会话执行时，只运行第 3、4 节，
不要把窗口启动失败判定为引擎契约失败。

## 3. 启动前门禁

依次执行：

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

通过标准：

- 四条命令均以退出码 0 结束。
- 不存在 panic、测试失败或 clippy warning。
- 后续窗口异常如果不能通过这里复现，应优先检查窗口系统、GPU 和 surface。

## 4. 核心事务基线

先运行无窗口验证，并保留 JSON 报告：

```bash
cargo run -p update-app -- --verify \
  --report=target/visual-verification/update-app.json

cargo run -p event-app -- --verify \
  --report=target/visual-verification/event-app.json
```

终端必须按发生顺序出现以下结果：

```text
PASS damage_modes
PASS notification_order
PASS dirty_coalescing
PASS panic_recovery
PASS stress_1024

PASS pointer_capture
PASS focus_keyboard
PASS wheel_hover_double
PASS coordinate_root
```

同时检查两个 JSON 顶层的 `"passed": true`。其中：

- `damage_modes` 证明 NoOp、Full、Partial 没有混淆。
- `notification_order` 证明通知保持
  `Validating -> FigureMoved -> Validated` 的因果顺序。
- `dirty_coalescing` 证明同一 Figure 的 dirty region 会合并。
- `panic_recovery` 证明监听器 panic 后更新管理器可恢复。
- `stress_1024` 证明 1,024 Figure 的更新事务能够收敛；debug 构建耗时不作为
  性能基准。
- `pointer_capture`、`focus_keyboard`、`wheel_hover_double` 分别证明 capture、
  focus/key 和扩展鼠标事件状态机。
- `coordinate_root` 证明入口点会转换为 target 所属坐标域。

任一 case 失败时停止窗口验收。窗口画面正常不能覆盖契约失败。

需要定位单个 milestone 时，可使用以下定向命令：

```bash
cargo test -p novadraw-geometry --test m1_product_existence
cargo test -p novadraw-render --test m1_product_existence
cargo test -p novadraw-scene --test m2_product_existence
cargo test -p novadraw-scene --test m4_coordinate_contract
cargo test -p novadraw-scene --test m5_layout_contract
cargo test -p novadraw-scene --test m6_event_contract
cargo test -p novadraw-scene typed_listeners_dispatch_and_remove_independently
cargo test -p novadraw-scene validation_figure_effects_preserve_causal_order
```

## 5. 窗口 app 通用操作

所有以下 app 均支持：

| 操作 | 效果 |
|---|---|
| `Home` | 切到第一个场景 |
| `End` | 切到最后一个场景 |
| `0`-`9` | 按索引切换到存在的场景 |
| `Left` / `PageUp` | 上一个场景 |
| `Right` / `PageDown` | 下一个场景 |
| `S` | 保存当前帧到对应 `apps/<app>/screenshot/` |
| `U` | 切换 UpdateManager 与直接渲染，仅用于问题定位 |
| `Esc` | 退出 |

窗口标题末尾必须显示当前场景名。当前主线没有 `I` 键或迭代渲染模式。

## 6. 更新与宿主提交

启动：

```bash
cargo run -p update-app
```

按顺序操作和观察：

| 步骤 | 动作 | 通过标准 |
|---|---|---|
| 1 | 按 `Home`，进入 `baseline` | 灰色背景上显示红、绿、蓝三个矩形，边界完整 |
| 2 | 缩小窗口，再恢复或放大 | 恢复后画面非空白，无旧尺寸残影、撕裂或 panic |
| 3 | 按 `1`，进入 `partial_damage` | 绿色矩形位于基线位置的右下方；旧位置由背景完整覆盖，无拖影 |
| 4 | 在场景 `0`、`1` 间反复切换 5 次 | 每次都得到稳定完整帧，不出现偶发透明帧 |
| 5 | 按 `2`，进入 `validation` | 紫、橙、青三个矩形按约束排布，彼此不重叠、不落在窗口外 |
| 6 | 按 `3`，进入 `stress_1024` | 32 × 32 色块网格完整出现；切出再切回仍可稳定显示 |

`partial_damage`、`validation` 场景在展示前已构造相应更新结果，因此人工观察验证
的是最终像素和宿主提交闭环。两阶段顺序、dirty 合并和事务收敛仍以第 4 节报告为准。

## 7. 事件分发与事件驱动重绘

启动：

```bash
cargo run -p event-app
```

颜色语义：

| 颜色 | 状态 |
|---|---|
| 蓝色 | idle |
| 绿色 | hovered |
| 红色 | pressed |
| 紫色 | focused |

按顺序操作和观察：

| 步骤 | 动作 | 通过标准 |
|---|---|---|
| 1 | 按 `Home` 进入 `pointer_capture`，把鼠标移入蓝色矩形 | 矩形立即变绿；移出后恢复蓝色 |
| 2 | 在矩形内按住左键 | 矩形变红 |
| 3 | 保持按下并拖到矩形外的灰色区域 | 矩形保持红色，说明拖拽仍投递给 captured target |
| 4 | 在灰色区域释放左键 | 矩形由红色变为紫色：release 到达 captured target，pressed/capture 已清除，press 建立的 focus 仍保留 |
| 5 | 按 `1` 进入 `focus_keyboard`，点击矩形并释放 | 按下时红色，释放后紫色，说明点击建立 focus |
| 6 | 保持窗口焦点，按 `A` 和 `Ctrl+A` | app 不崩溃、不切场景；键盘投递的精确结果看第 4 节报告 |
| 7 | 切走系统窗口焦点再返回 | focus 状态被释放；重新点击后可再次变为紫色 |
| 8 | 按 `2` 进入 `wheel_hover_double`，移入目标并滚动滚轮 | enter 状态可见，滚动后无状态卡死或画面丢失；wheel/hover/double-click 是否齐全看报告 |
| 9 | 按 `3` 进入 `coordinate_root` | 目标位于嵌套容器内，画面无父子坐标错位 |
| 10 | 再按一次 `3` 重置场景，先点击目标外部，再点击目标内部 | 外部点击不改变蓝色目标；内部点击产生红色/紫色状态变化 |

当前窗口颜色不编码键值、滚轮增量、双击次数或 target-domain 数值，因此这些内容
不得仅凭肉眼签收。共享 `novadraw-apps::DemoApp` 当前不合成 `Hover` 和
`DoubleClicked`，这两个事件只在 dispatcher verification 中验证；`editor` 已有
double-click 平台适配，但不作为 `event-app` 的端到端证据。未来共享宿主接入后，
应在此处补充对应窗口动作，不能用现有报告宣称平台适配链路已经覆盖。

Capture 与 focus 是两条独立状态轨：release 必须释放 capture，但不会自动释放
focus。只有显式 `release_focus`、窗口失焦或焦点转移才会产生 `FocusLost`。因此
步骤 4 的紫色是正确结果；若仍为红色才表示 release/capture 链路异常。

## 8. 坐标域与事件点降域

启动：

```bash
cargo run -p transform-app
```

按顺序操作和观察：

| 场景 | 动作 | 通过标准 |
|---|---|---|
| `0 nested_coordinate_roots` | 按 `Home` | 蓝色外层、橙色内层和绿色子 Figure 按 border insets 逐层嵌套 |
| `1 coordinate_roundtrip_overlay` | 按 `1` | 红色 absolute 描边与白色 local 描边完全重合，不出现双边或偏移 |
| `2 coordinate_root_move` | 按 `2` | 灰框保留旧区域；蓝色坐标根及绿色子 Figure 整体移动到右下方，子 Figure 相对位置不变 |
| `3 event_point_reduction` | 按 `3`，先点击目标外，再点击红色目标内部 | 外部点击不出现选中框，内部点击出现选中框，证明命中与绘制使用同一空间 |

第 4 节 `coordinate_root` 报告中的 `entry_x=160`、`target_x=60` 是事件点降域的
数值证据。

## 9. 递归绘制、Z-order 与裁剪

启动：

```bash
cargo run -p clip-app
```

执行：

1. 按 `1` 进入 `nested_clip`。
2. 观察绿色 child 只出现在橙色 parent 的可见边界内，不能越过 parent。
3. 按 `2` 进入 `multi_layer_clip`。
4. 观察红、绿、蓝三层区域逐层相交，最内层不能逃逸任一祖先裁剪。
5. 在场景 `1`、`2` 间反复切换，并缩放窗口。

通过标准：

- 子节点绘制顺序稳定，后绘制内容不破坏祖先裁剪。
- 切换和 resize 后裁剪边界不漂移、不扩大、不残留。
- 不出现后续兄弟继承前一节点颜色、透明度或 clip 的状态泄漏。

这里只把 `nested_clip` 和 `multi_layer_clip` 作为核心裁剪验收场景。其他
`clip-app` 场景包含展示性或尚未形成独立交互断言的内容，不纳入本流程结论。

## 10. 留存截图

静态场景可以批量截图：

```bash
cargo run -p update-app -- --screenshot-all
cargo run -p event-app -- --screenshot-all
cargo run -p transform-app -- --screenshot-all
cargo run -p clip-app -- --screenshot=1
cargo run -p clip-app -- --screenshot=2
```

截图命令会在完成后自动退出，文件位于各 app 的 `screenshot/` 目录。交互后的状态
使用窗口内 `S` 键留存。验收记录至少保留：

- 两份 `target/visual-verification/*.json` 报告。
- `update-app` 四个场景截图。
- `transform-app` 四个场景截图。
- `clip-app` 场景 1、2 截图。
- `event-app` capture 拖出、focus 建立等交互状态的人工结论；静态截图不能替代。

### 10.1 验收记录模板

每次正式验收复制并填写以下记录，建议保存在发布记录或对应 architecture delta 中：

```text
Commit:
日期:
操作系统:
图形设备 / 后端:

自动门禁:
[ ] cargo fmt --check
[ ] cargo check
[ ] cargo clippy -- -D warnings
[ ] cargo test

无窗口 verification:
[ ] update-app 5/5 PASS，报告路径:
[ ] event-app 4/4 PASS，报告路径:

窗口验收:
[ ] update-app：切场景、partial damage、1,024 Figure、resize/恢复
[ ] event-app：hover、press、capture 拖出释放、focus
[ ] transform-app：嵌套、往返重合、坐标根移动、点击命中
[ ] clip-app：nested clip、multi-layer clip、resize/恢复

未通过项与复现步骤:
结论: PASS / FAIL
```

## 11. 最终通过判定

只有以下条件全部满足，才能判定核心基础管线手工验收通过：

- 第 3 节全部门禁通过。
- `update-app` 与 `event-app` verification 全部 `PASS`。
- 更新 app 无空白帧、残影，Validation 结果和 1,024 Figure 场景完整。
- 事件 app 的 hover、pressed、capture、release、focus 可见状态符合顺序。
- transform app 的嵌套、往返重合、坐标根移动和点击命中均正确。
- clip app 的两层和多层祖先裁剪均正确。
- 所有 app 在切场景、resize、最小化后恢复时无 panic、设备丢失后永久黑屏或状态卡死。

失败时按最小范围归因：

| 现象 | 优先检查 |
|---|---|
| 首帧、resize 或切场景后透明/黑屏 | SceneHost 的 Full/NoOp 决策、surface resize 与 Vello 提交 |
| 旧位置残影或局部区域未更新 | dirty region、旧新 bounds union、Damage Repair 父链映射 |
| 布局结果错误但直接绘制正常 | invalidation、Validation root、LayoutManager |
| 拖出后立即失去 pressed 状态 | captured target 与 release 路由 |
| 点击位置与绘制位置不一致 | DPI 入口换算、坐标根、target-domain 事件点 |
| 子 Figure 越过父边界或污染兄弟 | Graphics 状态栈、祖先 clip 恢复、递归 paint 顺序 |
