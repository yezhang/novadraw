# ADR-031：Graphics 录制与帧出版边界

类型：`architecture-decision`

## 状态

已接受，2026-10-09。

本决策关闭临时概念审计 TC-01，并替代以下旧决定：

- ADR-020 第 4 节“公开 `NdCanvas` 作为规范入口”；
- ADR-021 第 1.1 节“crate root 绘图入口为 `NdCanvas`”；
- ADR-018 第 1 节中把 `prepare_frame`、`record_full_frame` 列为生产宿主入口的部分。

其余关于 Render IR、Runtime 单一提交权威和 Draw2D 绘制顺序的决定保持有效。

## 背景

P2-G02 已引入 `Graphics`、`MeasureContext`、`PaintContext`、`CommandRecorder` 和
`RecordedDrawing`，但旧绘制协议尚未退出：

- `Figure`、`Shape` 和 `Border` 仍接收公开 `NdCanvas`；
- `NdCanvas` 同时允许绘制、清空命令、修改 damage 和构造 submission；
- `Runtime::prepare_frame` 与 `record_full_frame` 向调用方返回内部录制状态；
- 新的受检绘制方法返回 `GraphicsError`，旧 Figure 回调却只能静默返回。

这形成两套可长期使用的绘制语言，并允许应用或 Figure 绕过 Runtime 的稳定帧边界。

## 决策

### 1. 只保留一套公开绘制语言

公开绘制入口固定为：

```text
standalone caller
    -> CommandRecorder
    -> Graphics / PaintContext
    -> RecordedDrawing

Figure / Shape / Border
    -> PaintContext

Runtime
    -> internal recording state
    -> RenderSubmission
```

`Graphics` 组合文字测量服务和录制器；`PaintContext` 是只绘制的短借用。
`CommandRecorder` 是唯一公开拥有命令流的对象。`NdCanvas` 降为 crate 内部实现，
不从 root、`graphics` 或 `render` 模块公开；库级 prelude 已由 ADR-035 删除。

`PaintContext` 可以读取当前绘制状态，并提供结构化 rect、path、line、polyline、
ellipse、text、image、transform、clip 和 state-stack 操作。它不能：

- 清空或替换既有命令；
- 读取或修改 damage；
- 构造 `RenderSubmission`；
- 设置 session、frame 或 surface 身份；
- 取得内部录制器的可变引用。

### 2. Figure 与可复用 painter 使用同一失败合同

`Figure::paint` 是唯一 Figure 自绘回调：

```rust,ignore
fn paint(
    &self,
    context: &mut PaintContext<'_>,
    bounds: Rectangle,
) -> Result<(), GraphicsError>;
```

默认实现为空绘制。旧 `paint_figure` / `paint_figure_in_bounds` 直接删除，不保留
deprecated 双入口。

`Shape` 的 fill/outline 和 `Border` 的 paint/snapshot 同样接收 `PaintContext` 并返回
`Result<(), GraphicsError>`。内置与外部实现使用相同协议，不通过内部画布旁路。

调用返回错误时，递归 renderer 记录首个 `GraphicsError`，继续完成内部 state-stack
复原，但该帧不得出版。`Runtime::prepare_submission` 返回
`FramePreparation::Error(FramePreparationError::Graphics(_))`，并保留待重绘工作供重试。

### 3. 独立录制与 Runtime 出版分离

`CommandRecorder::paint()` 提供不带文字测量服务的 `PaintContext`。需要测量、字体或
资源 lease 时使用 `Graphics::new(text_system, recorder)`。

`CommandRecorder::finish()` 只生成 `RecordedDrawing`，并验证录制错误和 state-stack
平衡。`RecordedDrawing` 不携带 surface、session 或 frame 身份，不等于已提交帧。

`Runtime::prepare_submission` 是唯一公开帧出版入口。它负责：

- 稳定 derived state；
- 冻结 damage 和资源同步；
- 校验录制错误、backend capability 和设备输入；
- 分配 session/frame 身份并建立 in-flight 状态。

`Runtime::prepare_frame` 与 `record_full_frame` 退出公开 API。测试和内部算法直接检查
crate 内录制结果；外部合同测试通过 `prepare_submission` 观察公开结果。

### 4. 本批次直接 breaking

项目仍处于 `0.1.0`，workspace 内已知调用方一次迁移。不得保留：

- 接收 `NdCanvas` 的 Figure、Shape 或 Border 回调；
- `NdCanvas` 的公开重导出；
- 返回 `NdCanvas` 或其他未出版命令容器的 Runtime 公共帧方法；
- 从 `PaintContext` 到内部画布的公开逃逸。

## 错误处理

- 所有可在录制阶段识别的非法几何、路径、opacity、图像和资源错误返回
  `GraphicsError`；
- 一次原子绘制操作失败时不追加该操作的部分命令；
- Figure、Shape 和 Border 不得吞掉绘制错误；
- state-stack 越过当前 `PaintContext` 的 floor 返回 `UnbalancedState`；
- backend capability、surface/device 和提交失败继续由既有 Render 协议处理。

## 扩展性与稳定性

- 外部 Figure 只依赖 `PaintContext`、backend-neutral 值类型和 `GraphicsError`；
- 可复用绘制 helper 可同时被 `Graphics::paint()` 与 Figure 回调调用；
- 新 backend 只消费 `RenderSubmission`，不接触 Figure 或 `CommandRecorder`；
- 新绘制 primitive 先进入 `PaintContext` 的受检语言，再由内部 recorder lower 到
  Render IR，不能公开第二套状态机。

## 验证

1. 外部 Figure consumer 仅实现 `paint(PaintContext, bounds) -> Result`；
2. Figure 绘制错误通过 `prepare_submission` 结构化返回，且修正后可重试；
3. `CommandRecorder::paint` 与 `Graphics::paint` 复用相同 helper；
4. `NdCanvas` 无法从 root、`graphics` 或 `render` 导入，且库级 prelude 不存在；
5. Runtime 公共 API 不再包含 `prepare_frame` / `record_full_frame`；
6. public API snapshot、Facade doctest、定向 Graphics suite 与 quick gate 通过；
7. 最终 full gate 只执行一次，并单独记录既有无关阻塞。

## 日期

2026-10-09
