# macOS Live Resize 与 Core Animation Transaction 根因复盘

类型：`verification`

日期：2026-09-12

状态：`complete`

验证基线：

- macOS 14.7.8 (23H730), arm64；
- rustc 1.94.1；
- Vello 0.10.0、wgpu 29.0.4；
- objc2-core-graphics / objc2-quartz-core 0.3.2。

## 1. 范围

本文记录 `layout-app/root_viewport_resize` 在 macOS 连续缩放窗口时出现白色拖影和
内容垂直抖动的运行时证据、排除过程、最终根因与后续迭代门禁。

规范性契约仍以
[`dynamic-architecture.md`](../../design/architecture/dynamic-architecture.md)
的“Resize 与 DPI”为准。本文只解释该契约的证据来源，不另行定义 Runtime、
LayoutManager 或 ZoomManager 语义。

## 2. 问题表现

复现路径：

1. 运行 `cargo run -p layout-app`。
2. 按 `End` 进入 `root_viewport_resize`。
3. 连续执行“缩小 -> 放大 -> 再缩小”，并分别拖动窗口宽度和高度。

初始表现：

- Figure 内容区域能随 logical viewport 实时重排。
- 缩小时窗口底部和右侧出现旧窗口内容或白色容器拖影。
- 放大时没有同等程度的方向性拖影。

这一区分很关键：布局结果已经更新，异常发生在平台 presentation 阶段，而不是
RootFigure、StackLayout、BorderLayout 或 ZoomManager。

## 3. 三个尺寸时钟

macOS live resize 同时涉及三个独立时钟：

1. **AppKit/Winit model size**：窗口和 `SurfaceChanged` 当前报告的尺寸。
2. **wgpu surface/drawable size**：`SurfaceConfiguration` 与当前 acquired drawable
   的 physical size。
3. **Core Animation presentation tree**：屏幕当前实际合成的 CALayer frame/bounds。

model layer 表示目标状态，presentation layer 表示屏幕正在显示的状态。live resize
期间 presentation tree 合法地落后 model tree 一次或多次合成提交。仅证明
`SurfaceConfiguration` 与 drawable 尺寸正确，不能证明窗口当前显示的 layer 已与
该 drawable 同步。

## 4. Core Animation Transaction 核心原理

### 4.1 Transaction 是 layer tree 的提交边界

`CATransaction` 不是数据库事务，也不是 GPU command buffer。它是 Core Animation
对一组 layer tree 属性变更的提交边界。应用修改 `bounds`、`position`、`opacity`
等属性时，首先修改的是 model layer；Core Animation 收集这些变更，在当前显式或
隐式 transaction 提交时生成一份供 compositor 消费的 layer tree 状态。

没有显式调用 `CATransaction.begin/commit` 时，Core Animation 仍会为 run loop
创建隐式 transaction，并在适当的 run-loop 边界提交。AppKit live resize 也通过
这一机制持续推进窗口和 view-backed layer 的几何状态。

因此：

- 多个 model layer 属性写入可以作为一个合成批次提交；
- transaction commit 表示状态交给 Core Animation，不表示像素已经显示；
- compositor 仍可能在后续 vsync 才展示该状态；
- transaction 不保证 CPU 等待 GPU 完成，也不等价于 `waitUntilCompleted`。

### 4.2 Model tree 与 presentation tree

Core Animation 同时暴露两个容易混淆的视图：

- **model layer**：应用希望最终达到的属性值；
- **presentation layer**：compositor 当前用于显示或插值的属性值。

当窗口连续改变尺寸时，model bounds 可以已经是新值，而 presentation bounds
仍对应前一次已提交或正在显示的状态。这种差异本身是正常的，不一定表示 resize
事件丢失，也不一定表示存在隐式动画。

诊断时应区分：

```text
model bounds == 新窗口尺寸
presentation bounds == 当前屏幕合成尺寸
```

transactional present 修复后，两者仍可能相差一个合成阶段。验收标准不是要求二者
在日志中始终数值相等，而是要求 compositor 展示某个 presentation geometry 时，
使用与该 geometry 属于同一 transaction 的 drawable。

### 4.3 CAMetalLayer 增加了独立的 drawable 时钟

普通 CALayer 内容通常由 Core Animation 管理。`CAMetalLayer` 则由应用取得
`CAMetalDrawable`，通过 Metal command buffer 写入纹理，再调用 present。于是
除了 layer tree transaction，还存在一条 GPU drawable 提交链：

```text
acquire drawable
-> encode Metal commands
-> submit render command buffers
-> request drawable presentation
-> compositor consumes drawable
```

如果 drawable present 与 Core Animation transaction 独立推进，layer geometry 和
drawable 虽然各自正确，却可能来自不同的提交代次。live resize 时这种错位尤其
明显：

```text
AppKit/model bounds       N+1
Core Animation display   N
Metal drawable            N+1
```

缩小时，旧 presentation geometry 比新窗口 model bounds 更大，差异会集中暴露在
右侧或底部；放大时新区域更容易被背景填充，因此症状具有方向性。

### 4.4 `presentsWithTransaction` 的语义

`CAMetalLayer.presentsWithTransaction` 控制 drawable 是否随 Core Animation
transaction 一起 present：

- `false`：Metal drawable 使用常规直接 present 路径，适合普通连续渲染，延迟更低；
- `true`：drawable present 被纳入当前 Core Animation transaction，使内容与该
  transaction 的 layer geometry 在同一合成边界生效。

它提供的是**合成原子性**，不是“立即显示”：

- 不强制 presentation layer 立即追上 model layer；
- 不消除 compositor 的正常流水线延迟；
- 不负责决定 contents gravity；
- 不修复错误的 drawable 尺寸或漏掉的 surface configure；
- 不保证 GPU 已完成，只要求 drawable 达到可由 Core Animation 安排 present 的
  状态。

目标不变量可以写成：

```text
visible frame = presentation geometry(Tn) + drawable(Tn)
```

错误状态是混合两个代次，例如
`presentation geometry(Tn) + drawable(Tn+1)`。transactional present 约束的是
这个代次配对，不是消除 `Tn` 相对最新 model state 的显示延迟。

这解释了为什么最终日志仍可观察到 presentation/model 差值，但用户不再看到拖影：
几何与像素虽然整体晚一个合成阶段，却不再互相错代。

### 4.5 wgpu 29 的 acquire/present 边界

在本次依赖基线中，wgpu Metal surface 在 acquire drawable 时读取
`CAMetalLayer.presentsWithTransaction`，并把结果保存在当前 surface texture 的
present 状态中。之后 `Queue::present` 根据该快照选择路径：

- non-transactional：先把 `presentDrawable` 编入内部 present command buffer，
  再 commit；
- transactional：先 commit 内部 present command buffer，等待它达到 scheduled
  状态，再调用 drawable 的直接 `present()`，由 CAMetalLayer 将其纳入 transaction。

由此得到严格顺序：

```text
set presentsWithTransaction(true)
-> get_current_texture / acquire drawable
-> submit Metal work
-> surface texture present
-> set presentsWithTransaction(false)
```

如果在 acquire 之后才设置 `true`，当前 drawable 已经选择了 non-transactional
路径，修改只可能影响后续 acquire，无法修复当前 resize 帧。

这里等待的是 **scheduled**，不是 **completed**：

- scheduled 表示 GPU 已接受该 command buffer，Core Animation 可以安全安排
  drawable；
- completed 表示 GPU 已完成全部工作，等待它会造成更强的 CPU/GPU 串行化；
- 本问题只需要与 CA transaction 建立提交顺序，不需要等待像素计算完全结束。

源码核对入口（只描述上述版本基线，不是 Novadraw 公共 API）：

- `wgpu-hal-29.0.4/src/metal/surface.rs`
  `Surface::acquire_texture`：快照 `presentsWithTransaction`；
- `wgpu-hal-29.0.4/src/metal/mod.rs`
  `Queue::present`：选择 `presentDrawable` 或
  `waitUntilScheduled -> drawable.present()`；
- Zed `crates/gpui_macos/src/metal_renderer.rs`
  `set_presents_with_transaction` 与 render/present 分支：提供同类平台实践。

### 4.6 为什么只在 resize 帧启用

普通渲染帧没有 AppKit bounds transaction 需要对齐，永久启用 transactional
present 会把所有 drawable 都绑定到 Core Animation transaction/run-loop 节奏，
可能增加等待和输入到显示延迟。

Novadraw 因此把该模式限制在检测到 `pending_resize` 的提交：

- resize 帧在 acquire 前开启；
- present 后立即关闭；
- surface error/recovery 分支也关闭；
- 普通帧保持 wgpu 默认的直接 present 路径。

`contentsGravity=topLeft`、opaque background 和 transactional present 分别解决
锚点、间隙颜色和提交代次问题，三者职责不能互相替代。类似地，
`masksToBounds` 只定义 presentation tree 内的裁剪；当父子 presentation geometry
一起滞后时，它不能建立新的同步关系。

## 5. 证据链

### 5.1 Surface 与 drawable 没有滞后

首轮“缩小 -> 放大 -> 再缩小”共采集 145 个 resize 帧：

- 145/145 帧 `requested == configAfter`。
- 145/145 帧 `requested == acquired drawable`。
- 145/145 帧 model layer bounds、layer frame size 与 superlayer bounds 相同。
- layer frame origin 始终为 `(0, 0)`。
- 提交耗时最小 4.85 ms、平均 5.93 ms、最大 29.88 ms。
- 仅 3 帧超过 16.67 ms，且缩小、放大、再缩小各一次。

因此排除：

- Winit resize 事件领先 surface configure 一帧；
- acquired drawable 仍使用旧尺寸；
- model layer 未覆盖当前窗口内容区；
- 普遍性 CPU 提交超时导致只在缩小时出现拖影。

### 5.2 `topLeft` 只修正旧内容锚点

将 Metal presentation layer 的 contents gravity 从 `bottomLeft` 改为 `topLeft`
后，人工 A/B 结果为：

- 底部拖影消失；
- 右侧拖影仍存在。

这证明 gravity 影响的是旧 drawable 在尺寸切换期间的锚定方向。`topLeft` 是正确
的左上逻辑原点契约，但它不能解决整个 presentation frame 相对窗口 model bounds
滞后的问题。

`topLeft` 也不是最终观察到的垂直抖动根因。垂直抖动是在后续启用父 layer 裁剪后
出现的。

### 5.3 父 layer 裁剪是错误修复方向

第二轮插桩同时记录 CAMetalLayer model frame、presentation frame 和父 layer
裁剪状态。典型证据为：

- model width 已为 652 pt，presentation width 仍为 669 pt；
- 父 layer 的 `masksToBounds=false`。

据此尝试对 CAMetalLayer 的父 layer 启用 `masksToBounds`。人工复验结果：

- 右侧拖影仍存在；
- 内容随窗口高度变化出现垂直抖动。

该轮 1,946 个 resize 帧全部确认父层裁剪已启用，但 child presentation/model
差值仍持续存在：

- width delta：`-201..+52 pt`；
- height delta：`-30..+48 pt`。

继续采集父 layer 的 presentation bounds 后发现，父 layer 与 CAMetalLayer 的
presentation bounds 一起滞后；1,025 个样本中的差值范围为：

- width delta：`-64..+68 pt`；
- height delta：`-27..+33 pt`；
- `presentsWithTransaction=false`：1,025/1,025 帧。

因此父层裁剪无法解决问题：裁剪边界自身也属于滞后的 presentation tree。它只会把
连续变化的 presentation/model 差值转换成可见裁剪跳动。该方案已撤销。

### 5.4 Transactional present 闭合 presentation 时序

本地参考实现与依赖源码提供了相同方向的证据：

- Zed/GPUI 在 Core Animation 驱动的同步显示路径中临时启用
  `presentsWithTransaction`；
- wgpu 29 Metal surface 在该模式下通过 Core Animation transaction 调度 drawable
  present，并等待 Metal command buffer 已 scheduled。

Novadraw 最终只对 resize 帧执行：

```text
detect pending resize
-> configure existing surface
-> set presentsWithTransaction(true)
-> acquire drawable
-> encode and submit
-> present drawable in Core Animation transaction
-> set presentsWithTransaction(false)
```

错误或恢复分支同样恢复 `false`。普通帧保持 non-transactional present，不承担
live-resize 同步所需的额外等待。

最终验证采集 1,417 个 resize 帧：

- 1,417/1,417 帧启用 transactional present；
- 1,417/1,417 帧 `requested == configured surface == acquired drawable`；
- 提交耗时最小 5.804 ms、平均 7.914 ms、最大 18.515 ms；
- 人工连续缩小、放大和再次缩小后，底部/右侧拖影与垂直抖动均不可复现。

## 6. 根因结论

根因不是 Figure 布局、logical viewport、surface configure 或 drawable 尺寸错误，
而是 resize 帧的 drawable presentation 未与 Core Animation 的窗口 bounds
transaction 同步：

- AppKit model bounds 已进入新尺寸；
- wgpu 已取得并绘制新尺寸 drawable；
- Core Animation presentation tree 仍显示前一合成状态；
- non-transactional drawable present 与该 presentation 状态独立推进，形成缩小时
  可见的旧边缘和容器底色。

最终修复由三个互补部分组成：

1. `topLeft` gravity 保持旧 drawable 的左上逻辑原点，消除方向错误。
2. opaque layer 与引擎 full-frame 清屏色一致，避免 drawable 间隙透出白色底层。
3. 仅 resize 帧使用 transactional present，将新 drawable 与窗口 bounds
   transaction 对齐。

前两项只能控制旧内容的锚定和间隙颜色，第三项才是右侧拖影与最终时序问题的根因
修复。

## 7. 已拒绝方案

| 方案 | 拒绝原因 |
|---|---|
| 平台 resize 隐式触发 fit/zoom | 改变产品语义，且内容布局已经正确 |
| 每个 resize 事件销毁并重建 surface | 增加资源抖动，不能保证 CA presentation 同步 |
| 只设置 layer 背景色 | 只能隐藏透明间隙，不能消除旧 presentation frame |
| 只改 `topLeft` gravity | 修复底部锚点，但右侧 presentation lag 仍存在 |
| 对父 layer 启用 `masksToBounds` | 父 presentation bounds 同样滞后，并引入垂直抖动 |
| 所有帧永久启用 transactional present | 无必要地增加普通帧延迟；问题只属于 resize 边界 |
| throttle/debounce resize 帧 | 降低交互跟手性，并掩盖而非修复时序错误 |

## 8. 当前实现边界

实现入口：

- `novadraw-render/src/backend/vello/mod.rs`
  - `configure_macos_presentation_layer`
  - `set_macos_transactional_present`
  - `VelloRenderer::submit`
- `novadraw-render/Cargo.toml`
  - macOS Core Graphics / Quartz Core 依赖

约束：

- 仅 macOS + Vello/Metal 路径使用 Core Animation API。
- `pending_resize` 必须在 `apply_pending_resize` 前捕获，用于标识当前提交是否为
  resize 帧。
- transactional mode 必须在 acquire drawable 前开启。
- present 完成、surface error 和 recovery 分支都必须恢复 non-transactional mode。
- resize 不得隐式修改 Figure 世界坐标或 ScalablePane scale。
- 禁止在该热路径加入常态日志；诊断只能使用临时插桩并在验证后删除。

## 9. 后续迭代门禁

以下变更必须重新执行本节检查：

- 升级 Vello、wgpu、winit、objc2 或 macOS SDK；
- 修改 surface acquire/present/recovery 顺序；
- 修改 resize coalescing、redraw 调度或首帧重试；
- 更换 Metal layer 创建方式或引入新的父/子 CALayer；
- 修改默认 full-frame 背景色或透明合成策略。

检查项：

1. 核对当前 wgpu Metal 实现是否仍在 acquire 时读取
   `presentsWithTransaction`，以及 transactional present 是否仍等待 command buffer
   scheduled。
2. 在 `layout-app/root_viewport_resize` 分别只改变宽、只改变高、同时改变宽高。
3. 执行“缩小 -> 放大 -> 再缩小”，每个方向至少连续操作 5 秒。
4. 检查 header/footer/sidebar/center 无错位、垂直抖动、白边、旧帧拉伸或拖影。
5. 检查普通静态帧和非 resize 动画没有额外延迟。
6. 验证 surface lost/outdated/timeout 后 transactional mode 能恢复为 `false`。
7. 运行：

```bash
cargo fmt --check
cargo check
cargo clippy -- -D warnings
cargo test
```

若问题复现，最小插桩应同时记录：

- requested physical size；
- surface config before/after；
- acquired drawable size；
- CAMetalLayer model frame 与 presentation frame；
- superlayer model bounds 与 presentation bounds；
- `presentsWithTransaction`；
- resize 帧提交耗时。

只记录 model size 无法判断本问题。

## 10. 失败模式与维护原则

- 如果 bottom/right 拖影重新出现，先区分 gravity 锚点错误与 transaction 不同步，
  不要直接恢复父层裁剪。
- 如果只在普通帧出现延迟，检查 transactional mode 是否在 resize 结束或错误分支后
  留为 `true`。
- 如果升级后 `as_hal::<Metal>()` 或 CAMetalLayer 行为变化，应在 backend 平台边界
  重建等价机制，不把 Core Animation 类型上移到 Runtime 或 apps。
- Windows、Linux 和 Web 不应复制该实现；它们只需满足相同的用户可见 resize
  契约，并采用各自 presentation 后端的正确同步机制。
