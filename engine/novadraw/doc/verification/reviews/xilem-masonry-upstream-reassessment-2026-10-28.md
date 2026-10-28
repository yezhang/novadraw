# Xilem / Masonry 上游依赖重新评估

类型：`implementation-verification`

2026-10-28，用户要求更新本地参考仓库后，重新评估自定义 Masonry Canvas /
Xilem View 是否必须使用 vendor。本记录只给出依赖与迁移评估，不改变既有
[接入契约](../../design/editor/masonry-xilem-canvas-integration.md)或完成状态。
本记录保留迁移前的评估范围；后续授权实施与无 vendor 验证结果见
[迁移记录](masonry-upstream-migration-2026-10-28.md)。

## 结论

自定义 Widget / View 不要求修改第三方框架，也不要求把其源码提交到本仓库。
现有 vendor 是针对旧提交的兼容实现，不能直接作为长期依赖方案。

建议先验证“固定未修改的上游 Git 提交 + 独立 imaging 输出适配”：
Masonry 消费自己的 imaging recording，Novadraw 现有 Vello 后端保持版本和行为。
这个方向能够避开直接交换不同版本 Vello Scene 的要求，但需要实现和验证另一种
输出目标。评估尚未证明全能力、性能及原生窗口行为通过，不能立即删除 vendor。

不建议只把 vendor 更新到最新源码后继续机械套用旧补丁。最新上游已改变绘制协议，
原 Scene 合成方案需要重新设计。把兼容副本放到远端 fork 再用 Git 引用，只改变
源码存放位置，不减少补丁维护责任。

## 核验基线

本地 `/Users/bytedance/Documents/code/GitHub/xilem` 原工作区干净，`main` 从
`5281ffb308732b00f42e0755c6095b6ae234ae16` 快进更新到远端 `origin/main`：
`66c8893f12232b9d5cdf4296f773aa3dc07ee776`。更新后工作区仍干净。

| 对象 | 版本与日期 | 核验来源 |
|---|---|---|
| 原固定上游提交 | 2026-02-11；Vello 0.7.0 | 原提交的 `Cargo.toml`，用 `git show` 核验 |
| 最新上游提交 | 2026-10-27；Vello 0.8.0、WGPU 28.0.0、Parley 0.8.0 | 更新后的 `Cargo.toml` 和 `Cargo.lock` |
| 最新上游工具链要求 | Rust 1.96 | `workspace.package.rust-version` |
| 当前本地工具链 | Rust 1.94.1 | `rustc --version`、`rustup toolchain list` |
| 当前 Novadraw | Vello 0.10.0 | [根 manifest](../../../Cargo.toml) |
| crates.io Xilem / Masonry 最新稳定版 | 0.4.0，2025-10-29 发布，依赖 Vello `^0.6.0` | crates.io 官方版本及依赖 API |

crates.io 发布版、旧本地提交和最新 Git main 是不同基线。它们的包版本字段都可能
写着 `0.4.0`，不能据此认定 API 或传递依赖相同。
Vello 最新稳定发布版为 0.11.0；其存在不代表 Xilem main 已采用该版本，
也不构成本次升级 Novadraw 的理由。

## 最新上游的绘制协议变化

最新 `masonry_core::Widget::paint` 接收 `imaging::Painter`。
内置 Canvas 保存 `imaging::record::Scene`，paint 中调用 `painter.replay`。
当前 [EditorCanvas](../../../crates/novadraw-platform-masonry/src/widget.rs)
则接收 `vello::Scene`，直接 append
[EditorSession](../../../crates/novadraw-platform-masonry/src/session.rs)的 Vello Scene。
两者不仅 Vello 版本不同，输出协议也不同。

因此，把 Novadraw 降为 Vello 0.8，或把 Masonry 提升到 Vello 0.10，都不能单独
让当前 Widget 实现兼容最新上游。最新上游还改变了 measure 的 `Length` 类型、
Xilem View 的状态参数以及测试 harness 的入口，迁移范围超过 manifest 修改。

`imaging_vello 0.0.1` 的已发布 feature 只提供 `vello-0-7` 和 `vello-0-8`。
升级它的 Vello 版本还涉及 imaging adapter，自有 workspace 中添加 Vello 0.10
不能覆盖这些传递依赖要求。Cargo 可以并存多个版本，但它们的 Scene、Device /
Queue 等类型不能直接交换。

最新上游还提供 `VisualLayerKind::External` / `PaintLayerMode::External`，
但源码明确说明当前宿主只跳过这些占位层。不能把占位契约视为已经实现的外部
GPU 纹理合成，也不能据此宣称现有 Vello 0.10 纹理可以直接嵌入。

## 现有补丁逐项处理建议

将 vendor 中全部 Rust 文件与原固定提交比较，实际修改了五个源文件：
`masonry_core/src/app/render_root.rs`、`masonry_core/src/core/contexts.rs`、
`masonry_testing/src/harness.rs`、`masonry_winit/src/event_loop_runner.rs`、
`masonry_winit/src/vello_util.rs`。以下建议针对最新上游路线。

| 当前改动 | 最新上游情况 | 处理建议 |
|---|---|---|
| Vello 0.7 → 0.10 | 上游默认绘制经过 imaging，使用 Vello 0.8 | 改为实现 imaging 输出目标；不继续强制框架升级 Vello |
| WGPU 29 instance / surface API 迁移 | 上游宿主使用 WGPU 28 | 使用未修改上游时删除；如仍强制共享 WGPU 29，则另行验证 adapter 与宿主 |
| UpdateCtx 增加 `get_scale_factor` | 上游公开 `scale_factor()`，包含 UpdateCtx | 删除补丁，调用上游 API |
| LayoutCtx 增加 `request_anim_frame` | 上游仍没有该直接方法，但有 `mutate_self_later` 和布局 typed action | 用公开延迟 mutation 请求一次更新，或通过 typed action 驱动；无需修改框架 |
| Rescale 后为整树请求动画 | 上游仍只请求重新渲染，没有等价的全树动画通知 | 将 Canvas DPI 失效放入宿主事件桥，只驱动有关会话；不能未经验证直接删除功能 |
| 测试渲染应用最终 DPI | 最新 TestHarness 已读取 `RenderRoot::scale_factor()` 并应用变换 | 删除旧补丁；迁移 DPI 用例 |
| 直接 Resize 时同步截图尺寸 | 最新 `process_window_event(Resize)` 仍不直接更新 harness `window_size`；SetSize 信号分支会更新 | 迁移到公开的尺寸设置路径并验证实际纹理尺寸；必要时向上游提交独立 harness 修复 |
| show 后重绘 / Occluded(false) 重绘 | 最新源码未包含这两项相同修复；WGPU 28 和旧补丁的 WGPU 29 acquisition 不同 | 先验证未修改上游冷启动，不能沿用旧白屏原因或宣称已解决 |
| 删除未导入 example / bench、缩减 members、显式嵌套 workspace | 这些改动服务于手工复制源码 | 使用 Git / registry 依赖后不再需要 |
| 导入测试字体 | 编译期测试资源属于第三方包 | 由 Cargo 管理上游包资源，不必在本仓库保留兼容副本资源 |

布局延迟 mutation 的依据是：最新 LayoutCtx 可排队 `mutate_self_later`；
MutateCtx 可请求动画；rewrite pass 包含 mutate / layout / compose，并再次检查待处理
mutation。回调只请求更新，不重复创建 Viewer 或执行业务命令。
仍需消费者验证去重、移除后的回调清理和 resize 顺序。

DPI 桥接的依据是公开 `MasonryState::handle_window_event` 委托入口与
`MasonryState::roots` / `RenderRoot::scale_factor`。可在平台适配层包装宿主事件，
在合法 mutation 阶段驱动 Canvas。现成 Xilem 启动入口如何组合这一桥接仍需设计
和编译验证，不能把 API 存在当作已接通。

## 路线与成本

| 路线 | 能否保留现有接入 | 主要成本 | 建议 |
|---|---|---|---|
| 直接使用 crates.io 0.4.0 | 不能原样保留 | 比当前 vendor 更早的 API、Vello 0.6 与 View 迁移 | 不作为本次优先路线 |
| 固定最新未修改 Git 提交，输出 imaging recording | 保留会话所有权；迁移绘制与 View API | Rust 工具链升级、独立输出适配、宿主 DPI 桥接、GPU 验证迁移 | 优先做可行性验证 |
| 更新兼容 vendor 或维护远端 fork | 仍需迁移最新绘制协议 | 继续承担 UI / imaging / GPU 升级与补丁维护 | 只在公开扩展点确实不足时考虑 |

imaging 输出适配应通过现有 Core 的 RenderBackend / RenderSubmission 消费协议实现，
不让第三方 imaging 类型进入 Core 公共签名。它不能只是把已有 Vello Scene 改名，
也不应在 paint 中调用业务命令或确认同一 Runtime frame 多次。

imaging 的 fill / stroke / glyph payload 支持基础图文绘制，但本次读取的接口没有
逐操作 AA 选择字段。Novadraw 的 OFF / ON / AUTO、设备像素描边、glyph 预处理、
图片源矩形和资源失败语义仍需明确映射，不能为删除 vendor 而静默降低能力。
新的输出目标可能增加 retained recording 和重放成本；复用现有几何/抗锯齿算法前
应确认归属，避免复制一套 Vello lowering。

未测实际包体积或性能，不给出增减比例。后续以相同 release 配置比较 vendor 路线
和未修改上游路线的 executable、压缩体积、重复依赖、初始化/RSS、编码分配及
连续拖动/resize/DPI 变化帧耗时。Canvas 无活动更新时不得持续 tick。

## 验证边界与后续门禁

本次完成远端更新、manifest / lock 核验、原补丁全量比对、公开 API 与处理顺序审阅，
以及 crates.io 官方发布元数据和 imaging 接口核验。没有修改应用依赖或执行迁移。
最新上游 MSRV 高于本地工具链；本次没有安装新工具链、绕过 MSRV 或声明编译通过。

迁移前先完成一个独立消费者验证，覆盖基础图元、路径 clip、文字 glyph、图片、
逐操作 AA、非 1.0 DPI、resize、Retry 保持旧目标与重复 paint。
Widget / View 需保留 session 身份、历史、capture / cancel 与 teardown 的既有契约。
最新上游 TestHarness 使用 CPU renderer；应保留独立的真实 GPU 对照测试，
不能将旧 GPU 截图结论自动继承到 CPU harness。
冷启动 show / occlusion 与完整原生交互须重新取证。

通过上述验证并接受迁移设计后，才能切换依赖、删除兼容副本并更新当前接入契约。

## 可复核来源

- [原兼容副本记录（保留原文）](../evidence/masonry-vendor-baseline-2026-10-28.txt)
- [现有 Scene 输出边界](../../adr/adr-053-embedded-vello-scene-target.md)
- [原生首帧与 resize 既有证据](masonry-native-first-frame-followup-2026-10-28.md)
- 上游源码固定基线：`66c8893f12232b9d5cdf4296f773aa3dc07ee776`；
  本地路径 `/Users/bytedance/Documents/code/GitHub/xilem`，重点文件：
  `Cargo.toml`、`Cargo.lock`、`masonry_core/src/core/widget.rs`、
  `masonry_core/src/core/contexts.rs`、`masonry_core/src/app/render_root.rs`、
  `masonry_core/src/app/visual_layers.rs`、`masonry_testing/src/harness.rs`、
  `masonry_winit/src/event_loop_runner.rs`、`xilem_masonry/src/view/canvas.rs`。
- [crates.io Xilem 发布信息](https://crates.io/api/v1/crates/xilem)
- [Xilem 0.4.0 发布依赖](https://crates.io/api/v1/crates/xilem/0.4.0/dependencies)
- [Masonry 0.4.0 发布依赖](https://crates.io/api/v1/crates/masonry/0.4.0/dependencies)
- [imaging_vello 0.0.1 发布依赖](https://crates.io/api/v1/crates/imaging_vello/0.0.1/dependencies)
