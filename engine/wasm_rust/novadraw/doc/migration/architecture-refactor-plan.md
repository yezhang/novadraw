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

状态：`awaiting_manual_approval`

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

手动验证：

- layout-app 场景 0-9；
- update-app；
- resize 后布局稳定且无闪烁。

## 9. R6：Update、Damage 与提交边界

前置条件：R5 手动验证 `PASS`。

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

手动验证：

- update-app 全场景；
- 窗口 resize、最小化、恢复；
- 按 `U` 对比增量和全量路径；
- 检查无残影、透明帧或黑帧。

## 10. R7：平台边界与应用迁移

前置条件：R6 手动验证 `PASS`。

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

手动验证：

- macOS 全部核心 demo；
- 至少一个 Web 构建和浏览器输入验证；
- Windows/Linux 在 CI 或目标机器验证 build 与基础输入。

## 11. R8：清理、性能与扩展验证

前置条件：R7 手动验证 `PASS`。

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

## 14. 当前手动验证：R3

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

### 14.3 Editor

```bash
cargo run -p editor
```

1. 按 `0`：四个控制点分别贴合灰色矩形的四角。
2. 按 `1`、`2`：嵌套图形内部关系和尺寸一致；场景 2 为便于对照，整体位置刻意右移。
3. 按 `5`，再按 `T`：父节点移动时后代整体移动，内部相对位置保持不变。
4. 按 `9`：点击 DPI 探针中心和四边内侧，命中位置与光标一致。

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

若失败，请附应用名、场景编号、操作步骤和可见结果。收到 `R3: PASS` 后开始 R4。
