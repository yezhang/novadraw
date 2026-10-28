# 理想架构代码迁移计划

类型：`migration-guide`

本文定义从 `GPT-5.6-sol` 标签对应的实现基线迁移到
`doc/design/architecture/overview.md` 所述理想架构的完整步骤。

迁移必须保持每个阶段独立可编译、可测试、可手动验收和可回滚。不得在一个阶段中
同时改变所有权、坐标语义和渲染结果。

## 1. 基线

- 设计基线 tag：`GPT-5.6-sol`
- 设计决策：`doc/adr/adr-003-rust-runtime-and-geometry-boundaries.md`
- 总体设计：`doc/design/architecture/overview.md`
- 坐标 SSOT：`doc/design/coordinates/coordinate-system.md`
- 更新 SSOT：`doc/design/rendering/update-manager.md`

## 2. 强制工作流

每个迁移阶段严格执行：

```text
定义本阶段契约和测试
→ 给出手动验证步骤
→ 用户执行并明确回复 PASS
→ 才开始下一阶段代码修改
→ 自动测试
→ 更新迁移状态
→ 再给出下一阶段手动验证步骤
```

手动验证失败时：

1. 不进入下一阶段；
2. 记录复现环境和失败步骤；
3. 在当前阶段修复；
4. 重跑自动门禁；
5. 重新执行同一手动验证。

每个阶段建议使用独立中文 Git commit；稳定节点可以增加 annotated tag。

## 3. 全局完成条件

迁移完成必须同时满足：

- 新代码以 `Runtime` 为唯一事务组合根；
- `FigureTree` 不拥有 InteractionState 或 UpdateManager；
- `FigureNode` 组合 NodeState、LayoutState 和 Figure；
- Figure 不再保存通用 bounds、visibility、enabled 和 validation 状态；
- Figure 回调只产生 effect，不直接借用 Runtime 服务；
- mutation 严格 FIFO 且原子；
- bounds 使用 parent content domain；
- paint、hit-test、event point 和 damage 共用 `Affine2D` 变换链；
- PlatformHost 与 RenderBackend 分离；
- DisplayList 不成为核心依赖；
- 所有应用迁移到新 API；
- 兼容别名和旧路径完成弃用周期后删除；
- 自动门禁和全部手工验证通过。

## 4. R1：所有权骨架与事务入口

状态：`manually_approved`

范围：

- 引入 `FigureId`、`FigureNode`、`FigureTree` 架构名称；
- 抽出 NodeState、LayoutState、InteractionState；
- 引入具体 Runtime；
- Runtime 统一执行 dispatch 和 mutation flush；
- Figure callback 先记录 effect，再在借用释放后提交；
- mutation 改为 FIFO；
- LayoutManager 使用独占 Box；
- disabled 不阻断 validation；
- 移除核心 Figure/Layout/Event/Update/Host trait 的 blanket `Send + Sync`；
- 增加 PlatformHost、SurfaceInfo、ResourceDelta；
- `Affine2D` 成为二维变换的规范名称；
- 公共 DemoApp 使用 Runtime。

兼容边界：

- `BlockId`、`FigureBlock`、`FigureGraph` 暂时保留；
- FigureGraph 内仍保留 legacy InteractionState，供尚未迁移的调用方使用；
- Figure 内 bounds 暂时与 NodeState bounds 同步；
- editor 的专用交互核心尚未迁移；
- 坐标仍保持旧 Draw2D 分段域语义。

自动门禁：

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy -- -D warnings
cargo test --workspace
```

手动门禁见第 12 节。

## 5. R2：Runtime 全面接管交互状态

前置条件：R1 手动验证 `PASS`。

状态：`manually_approved`

工作：

- 将 editor 和剩余直接 dispatcher 调用迁移到 Runtime；
- `SceneDispatchContext` 只接受 Runtime 拥有的 InteractionState；
- 删除 FigureGraph 中的 legacy interaction 字段和访问器；
- 用 `PointerId -> PointerState` 替代单一 capture；
- gesture session 同时固定 target 和 typed controller；
- 节点删除、隐藏、禁用时由 Runtime 统一清理引用；
- selection 移出 NodeState，进入 editor/viewer 层。

自动测试：

- 多 pointer capture；
- focus/hover/cursor 相互独立；
- 删除 target 时 cancel；
- gesture target/controller 固定；
- callback effect 因果顺序；
- 所有输入 app verification。

手动验证：

- `event-app` 四场景；
- editor 点击、拖拽、移出释放、键盘焦点；
- scroll-pane pointer capture 与触控板手势并行。

## 6. R3：Parent-local 坐标迁移

前置条件：R2 手动验证 `PASS`。

状态：`approved`

工作：

- NodeState.bounds 成为树、布局、命中和 damage 的唯一权威来源；
- Figure 内 bounds 在 R4 删除 capability 兼容层时移除，本阶段不得作为树算法的数据源；
- Figure 使用 local border-box 绘制；
- 删除 `use_local_coordinates` 和移动后代 bounds 的旧模型；
- 每条树边统一为 `Affine2D`；
- Viewport scroll、ScalablePane scale 和 insets 进入同一变换链；
- event point 转为 target local domain；
- damage 使用可覆盖的 local visual bounds，并沿统一 Affine2D 链投影；
- 提供一次性旧场景坐标转换工具
  `FigureTree::migrate_legacy_bounds_to_parent_local()`，不在运行时长期保留双模式。

自动测试：

- parent 移动不改变 descendants bounds；
- affine 往返与不可逆变换；
- paint/hit/event/damage 同源；
- old/new projected damage；
- nested viewport + scale；
- 10,000 层边界。

手动验证：

- `transform-app` 全场景截图前后对比；
- editor 场景 1、2、5、9；
- viewport-app 四场景；
- scroll-pane-demo 缩放和四边可达性。

## 7. R4：Figure Capability 与节点状态收口

前置条件：R3 手动验证 `PASS`。

状态：`approved`

工作：

- Figure 基础接口只保留 paint、intrinsic measure 和 precise hit；
- 输入、生命周期、accessibility 拆为可选 capability；
- 删除 Shape 对 Figure 的 blanket impl；
- 每个内置 Figure 显式实现所需能力；
- bounds、insets、visible、enabled、opaque、size override 和 style override
  统一归 NodeState；
- 删除 Figure 内兼容 bounds 镜像；
- selection 完全移出引擎核心节点状态。

自动测试：

- 非交互 Figure 不需要空事件方法；
- Shape 可独立定制 Figure 行为；
- NodeState 与具体 Figure 无重复真源；
- style inheritance、border/client area；
- attach/detach 生命周期。

手动验证：

- shape-app；
- style-app；
- border-app；
- editor selection overlay。

## 8. R5：布局快照与缓存

前置条件：R4 手动验证 `PASS`。

状态：`approved`

工作：

- LayoutManager 输入改为不可变 LayoutSnapshot；
- 布局结果通过 LayoutOutput 原子提交；
- LayoutState 保存 manager、typed constraints 和 generation cache；
- 删除布局期间对 FigureTree 的可变回调；
- 明确 constraint 类型错误；
- 为不收敛 validation 增加结构化错误和诊断链。

自动测试：

- 六种布局；
- constraint remove/reparent；
- zero size 不作为 fallback sentinel；
- panic/error 后 manager 和队列恢复；
- 1,024 节点布局；
- non-converging validation。

自动验证结果：

- `cargo test -p novadraw-scene --test m5_layout_contract`：10 项通过；
- `cargo test --workspace --lib --tests`：全部通过；
- `cargo clippy -p novadraw-scene --lib -- -D warnings`：通过；
- `cargo run -p update-app -- --verify
  --report=target/visual-verification/update-app-r5.json`：5 项通过。

手动验证：

- layout-app 场景 0-9；
- update-app；
- resize 后布局稳定且无闪烁。

## 9. R6：Update、Damage 与提交边界

前置条件：R5 手动验证 `PASS`。

状态：`approved`

工作：

- UpdateManager 成为 Runtime 内部具体组件；
- mutation、validation、damage、recording 顺序统一；
- RenderSubmission 完整携带 Damage、SurfaceInfo 和 ResourceDelta；
- backend 依据能力选择 partial 或 full；
- surface lost/resize 后强制 Full；
- notification 在稳定事务边界 flush。

自动测试：

- None/Full/Partial；
- full 与 partial 像素等价；
- surface lost/retry；
- notification 因果顺序；
- retained surface 正确性。

自动验证结果：

- `cargo check --workspace`：通过；
- `cargo clippy -- -D warnings`：通过；
- `cargo test --workspace --lib --tests`：全部通过；
- `cargo run -p update-app -- --verify
  --report=target/visual-verification/update-app-r6.json`：6 项通过。

手动验证：

- update-app 全场景；
- 窗口 resize、最小化、恢复；
- 按 `U` 对比增量和全量路径；
- 检查无残影、透明帧或黑帧。

## 10. R7：平台边界与应用迁移

前置条件：R6 手动验证 `PASS`。

状态：`approved`

工作：

- PlatformHost 只负责 redraw、surface、cursor、IME 和 accessibility；
- RenderBackend 不再拥有 WindowProxy；
- Winit adapter 服务 macOS/Windows/Linux；
- Web adapter 使用相同 InputEvent 和 logical units；
- HeadlessHost 支持确定性测试；
- 删除 SceneHost/NovadrawSystem 兼容层；
- 所有 app 只依赖 Runtime 命名操作。

自动测试：

- logical/physical resize；
- DPI change；
- headless frame；
- host redraw 合并；
- backend retry；
- 平台类型依赖扫描。

自动验证结果：

- `cargo check --workspace`：通过；
- `cargo clippy -- -D warnings`：通过；
- `cargo test --workspace`：全部通过；
- `cargo tree -p novadraw-scene -e normal`：不包含 winit、Vello 或 wgpu；
- `novadraw-apps` 的 Winit/Web adapter 与 `HeadlessHost` 契约测试通过。
- `cargo check -p novadraw-apps --no-default-features`：通过；
- `./scripts/build_web_validation.sh`：生成可由浏览器加载的 wasm-bindgen 产物；
- `cargo run -p update-app -- --verify
  --report=target/visual-verification/update-app-r7.json`：6 项通过；
- `cargo run -p event-app -- --verify
  --report=target/visual-verification/event-app-r7.json`：4 项通过。

手动验证：

- macOS 全部核心 demo；
- 至少一个 Web 构建和浏览器输入验证；
- Windows/Linux 延期到对应跨平台应用开发或发布资格验证，不阻塞后续引擎迁移。

## 11. R8：清理、性能与扩展验证

前置条件：R7 手动验证 `PASS`。

状态：`approved`

执行细则：

- [`r8-execution-plan.md`](r8-execution-plan.md)

工作：

- 删除 BlockId/FigureBlock/FigureGraph 等兼容名称；
- 清理旧 context、旧坐标模式和失效文档；
- profile 大树、深树、文本和 viewport 场景；
- 确认 DisplayList 仍只是 proposal；
- 增加 2.5D ProjectiveComposition 最小 capability 测试；
- Scene3D 只保留独立扩展接口，不实现伪 3D。

完成门禁：

- workspace format/check/clippy/test 全通过；
- public API 文档无旧术语；
- dependency graph 无反向平台依赖；
- benchmark 不低于迁移前已记录基线；
- 所有手工验证记录为 PASS。

## 12. R1 手动验收记录

状态：`approved`

用户已明确要求开始 R2，视为 R1 手动门禁通过。

## 13. R2 手动验收记录

状态：`approved`

- 平台：macOS
- 结果：PASS
- 失败项：无

## 14. R3 手动验收记录

状态：`approved`

- 平台：macOS
- 结果：PASS
- 失败项：无

以下为本次验收使用的步骤。

在开始 R4 前执行以下步骤。

### 14.1 自动验证工具

```bash
cargo run -p event-app -- --verify \
  --report=target/visual-verification/event-app-r3.json

cargo run -p scroll-pane-demo -- --verify \
  --report=target/visual-verification/scroll-pane-r3.json
```

通过标准：

- event-app 输出四项 `PASS`；
- scroll-pane-demo 输出四项 `PASS`；
- 两个 JSON 顶层 `"passed": true`。

### 14.2 Transform App

```bash
cargo run -p transform-app
```

1. 按 `0`：嵌套图形位置正确，父子边距一致。
2. 按 `1`：白色轮廓与红色绝对投影完全重合。
3. 按 `2`：移动父节点后，子节点保持相对位置，没有二次偏移。
4. 按 `3`：点击红色目标的四角和中心，命中点与光标一致。

### 14.3 专项覆盖说明

历史 Figure selection/input probe 已移除。图形树、变换、输入和 DPI 行为分别由
`shape-app`、`transform-app` 与 `event-app` 的专项场景验证。

### 14.4 Viewport

```bash
cargo run -p viewport-app
```

依次按 `0` 到 `3`：

1. viewport 外内容被裁剪；
2. origin 滚动方向与距离正确；
3. zoom 后边框内必须有内容，所有矩形按相同比例放大且保持原宽高比；
4. 嵌套 viewport 没有重复平移或缩放。

### 14.5 Scroll Pane

```bash
cargo run -p scroll-pane-demo
```

1. 滚动条按钮、轨道和 thumb 点击位置准确。
2. 拖动垂直 thumb，内容连续移动且释放后不粘连。
3. 触控板滚动可到达四边。
4. pinch 缩放时锚点稳定，所有矩形保持统一缩放比例和原宽高比，缩放后仍可滚动到四边。

### 14.6 验收回复

请回复：

```text
R3: PASS
平台:
失败项: 无
```

若失败，请附应用名、场景编号、操作步骤和可见结果。

## 15. R4 手动验收记录

状态：`approved`

- 平台：macOS
- 结果：PASS
- 失败项：无

以下为本次验收使用的步骤。

在开始 R5 前执行以下步骤。通用 demo 使用 `Home` 返回首场景、左右方向键或
`PageUp`/`PageDown` 切换场景、`S` 保存当前截图。

### 15.1 Shape App

```bash
cargo run -p shape-app
```

按 `Home` 后逐个切换 0-7：

1. `0 Rectangle Fill`：五个矩形完整显示，位置、尺寸和填充色互不串扰。
2. `1 Ellipse Fill`：五个椭圆完整显示，边缘平滑且没有退化为矩形命中外观。
3. `2 Rounded Rect`：圆角、填充和描边组合正确，粗描边没有越界裁剪。
4. `3 Polyline`：不同点数、线宽、line cap 和 line join 均可区分，无整体偏移。
5. `4 Mixed Shapes`：矩形、椭圆和折线组合的位置及尺寸正确。
6. `5 Z-Order`：后添加图元稳定遮挡先添加图元。
7. `6 Triangle`：不同方向、线宽、填充和描边均正确。
8. `7 Parent-Child`：嵌套图元保持 parent-local 相对位置，边框和子内容不重叠。

可选截图留档：

```bash
cargo run -p shape-app -- --screenshot-all
```

### 15.2 Style App

```bash
cargo run -p style-app
```

按 `Home` 后逐个切换七个场景：

1. `Fill Colors`：四种填充色正确。
2. `Alpha/Transparency`：透明度从不透明到透明逐级变化，叠加背景正确。
3. `Stroke Width`：描边宽度递增且保持在图元边界内。
4. `Stroke Color`：描边颜色互不串扰。
5. `LineCap`：Butt、Round、Square 端点差异正确。
6. `LineJoin`：Miter、Round、Bevel 转角差异正确。
7. `Stroke vs Border`：三行分别显示 shape stroke、border、二者叠加；绿色内描边
   与红色外边框都可见。

### 15.3 Border App

```bash
cargo run -p border-app
```

按 `Home` 后逐个切换五个场景：

1. `RectangleBorder`：三种宽度边框完整显示。
2. `Border+insets`：10、20、30 像素 inset 递增，内容区同步收缩。
3. `LineBorder`：不同颜色和宽度均正确。
4. `MarginBorder`：边距递增，四边间距一致。
5. `Stroke vs Border`：stroke、border 和组合三组没有错位、覆盖或异常裁剪。

### 15.4 验收回复

```text
R4: PASS
平台: macOS
失败项: 无
```

若失败，请附应用名、场景编号、操作步骤和可见结果。

## 16. R5 手动验收记录

状态：`approved`

- 平台：macOS
- 结果：PASS
- 失败项：无

自动门禁与人工窗口验收均已通过，可以开始 R6。

### 16.1 Layout App

```bash
cargo run -p layout-app
```

按 `Home` 后逐个切换 0-9：

1. `XYLayout + Constraints`：约束位置和尺寸正确；
2. `FillLayout (First Fills)`：首个子节点填满 client area；
3. `FlowLayout`：子节点按可用宽度稳定换行；
4. `Nested Layouts`：嵌套布局没有重复偏移；
5. `Constraint Update`：更新约束后结果稳定；
6. `GridLayout`：行列、间距和填充正确；
7. `ToolbarLayout`：主轴压缩和次轴拉伸正确；
8. `StackLayout`：所有子节点填满 client area；
9. `No Layout (Raw)`：无布局节点保持原始 bounds；
10. `Border Layout (XY)`：五区布局完整且无重叠。

拖动窗口改变大小，确认布局连续稳定、无闪烁和残影。

### 16.2 Update App

```bash
cargo run -p update-app -- --verify \
  --report=target/visual-verification/update-app-r5.json

cargo run -p update-app
```

自动报告应包含 5 项 `PASS`。窗口中切换全部场景，确认 validation、partial damage
和 1,024 Figure 场景均正常显示，resize 后布局稳定。

### 16.3 验收回复

```text
R5: PASS
平台: macOS
失败项: 无
```

若失败，请附应用名、场景编号、操作步骤和可见结果。

## 17. R6 手动验收记录

状态：`approved`

- 平台：macOS
- 结果：PASS
- 失败项：无

自动门禁与人工窗口验收均已通过，可以开始 R7。

### 17.1 Update App

```bash
cargo run -p update-app
```

1. 依次查看 `baseline`、`partial_damage`、`validation`、`stress_1024`；
2. 在每个场景按 `U` 往返切换增量与全量路径，画面应保持一致；
3. 连续拖动窗口边缘改变大小，内容应稳定，无残影、透明帧或黑帧；
4. 最小化窗口后恢复，首个可见帧应完整；
5. 多次切换场景并 resize，确认没有停帧或旧帧残留。

### 17.2 自动报告复核

```bash
cargo run -p update-app -- --verify \
  --report=target/visual-verification/update-app-r6.json
```

报告应包含 6 项 `PASS`，其中 `submission_lifecycle` 验证 frame ID、ResourceDelta
和 retry 后的 Full damage 恢复。

### 17.3 验收回复

```text
R6: PASS
平台: macOS
失败项: 无
```

若失败，请附场景名、是否启用 UpdateManager、窗口操作步骤和可见结果。

## 18. R7 手动验收记录

状态：`approved`

首次 macOS 人工验收未通过：

- editor 场景 5 按 `T` 平移时出现旧帧残影；
- editor 左右方向键切换场景失效。

以上问题已修复并增加回归测试。

macOS 复验结果：

- 平台：macOS
- 结果：PASS
- 失败项：无

Web 验收结果：

- 平台：Chrome / wasm32-unknown-unknown
- 结果：PASS
- 默认 Vello WebGPU backend 正常初始化并通过 Wasm Runtime 绘制；
- Canvas2D backend 作为 `?backend=canvas2d` 诊断基线保留；
- Pointer、Wheel、Keyboard 事件均进入 Runtime，计数与状态更新正确；
- 1x/2x DPR 切换触发 Full damage，logical/physical surface 尺寸正确；
- 浏览器控制台无错误，JavaScript 与 Wasm 资源请求成功。

延期验证平台：

- Windows
- Linux

### 18.1 跨平台验证暂缓与后续方案

决策（2026-09-02）：R7 的目标是证明平台边界与引擎运行时解耦，不是完成所有桌面
平台的发布资格认证。macOS 覆盖原生 Winit + Vello 路径，Web 覆盖独立 Web adapter +
Vello WebGPU 路径，HeadlessHost 与自动契约覆盖确定性行为；三类证据足以批准 R7 并
继续引擎迭代。

Windows/Linux 验证延期到对应跨平台应用开始开发、进入 CI 支持矩阵或准备发布时执行。
延期不代表这些平台已支持、已兼容或已通过验收，也不得用于发布声明。

平台资格验证分层如下：

1. **持续集成构建门禁**：在目标平台 runner 上运行
   `cargo fmt --all -- --check`、`cargo check --workspace`、
   `cargo clippy --workspace -- -D warnings` 与 `cargo test --workspace`。
   交叉编译只可作为本地提前发现 target 编译问题的补充，不能替代目标平台 runner。
2. **目标平台基础输入验证**：在目标平台实机或 CI runner 上启动
   `event-app` 和至少一个图形 demo，验证窗口创建、Pointer、Wheel、Keyboard、resize
   与 Full damage。
3. **发布前图形验收**：在真实目标机器及原生 GPU 驱动上运行 `event-app`、
   `update-app`、`viewport-app` 和 `scroll-pane-demo`，覆盖 resize、DPI 缩放、最小化/
   恢复与 WebGPU/Vello 渲染。虚拟机可用于辅助人工检查，但不能替代此项验收。

后续启动 Windows/Linux 应用工作时，先补齐第 1 层，再按第 2、3 层顺序执行并将结果
写入本节或对应平台验证文档。

### 18.2 macOS 核心 Demo

依次运行以下应用，确认启动、输入、场景切换、resize、最小化和恢复均正常：

```bash
cargo run -p layout-app
cargo run -p event-app
cargo run -p update-app
cargo run -p viewport-app
cargo run -p scroll-pane-demo
```

重点检查：

1. 所有应用 resize 后首帧完整，无旧帧拉伸、残影、透明帧或黑帧；
2. 最小化后恢复会产生 Full damage；
3. `update-app` 按 `U` 切换增量与全量路径时画面一致。

### 18.3 Event App Focus / Keyboard

先运行确定性验证：

```bash
cargo run -p event-app -- --verify --scenario=focus_keyboard
```

预期输出 `PASS focus_keyboard`。该场景依次验证：

1. 鼠标按下目标后收到 `FocusGained`；
2. 焦点目标收到 `Ctrl+A` 的 key pressed；
3. 焦点目标收到 `A` 的 key released；
4. 主动释放焦点后收到 `FocusLost`，最终 `focus_owner` 为 `None`。

窗口中可按 `1` 进入 `focus_keyboard`，点击蓝色目标；释放鼠标后目标应保持紫色，
表示焦点仍由该目标持有。按键事件本身不改变颜色，因此键盘事件顺序以以上
`--verify` 结果为验收依据。

### 18.4 Web 与其他桌面目标

完整环境准备、资源服务、浏览器操作、失败分类和记录模板见
[`../verification/manual/web-platform.md`](../verification/manual/web-platform.md)。

Web 环境构建与运行：

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked --root target/wasm-tools
./scripts/build_web_validation.sh
./scripts/serve_web_validation.sh
```

默认访问 `http://127.0.0.1:4173/` 使用 Vello WebGPU；访问
`http://127.0.0.1:4173/?backend=canvas2d` 使用 Canvas2D 诊断基线。后端初始化失败时
页面进入 `ERROR`，不得静默降级。2026-09-02 的 Chrome 验收已确认：

1. 页面进入 `READY · Vello WebGPU` 且 GPU Canvas 非空；
2. Pointer hover/click、Keyboard `A` 与 Wheel 事件计数递增；
3. Figure 在 idle/focus 状态间正确切换颜色；
4. DPR 从 2x 切换至 1x 后，surface 从 `1560×975 px` 更新为 `780×488 px`，
   logical size 保持 `780×488`，并产生 Full damage；
5. Canvas2D 查询参数可独立启动，未与 WebGPU canvas context 混用；
6. 控制台无 Wasm/WebGPU 错误，`web_validation.js` 与
   `web_validation_bg.wasm` 加载成功。

Web pointer 坐标保持 CSS logical units，wheel 的 pixel/line/page delta 映射到统一引擎
事件。Windows/Linux 属于延期的平台资格验证，不阻塞 R8。

### 18.5 验收回复

```text
R7: PASS
引擎迁移验证平台: macOS / Web / Headless
延期平台资格验证: Windows / Linux
失败项: 无
```

延期平台不得记为平台 PASS；R7 PASS 只表示当前引擎迁移门禁已满足。
