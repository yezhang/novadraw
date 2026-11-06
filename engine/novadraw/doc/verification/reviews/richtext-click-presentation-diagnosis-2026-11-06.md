# 富文本光标呈现延迟诊断

类型：`verification`

能力归属：Editor 的反馈发布；richtext 的布局与绘制资产；Core 的帧记录和 damage；
Vello 后端的编码与提交；Native 宿主的输入和呈现。日期：2026-11-06。

状态：已定位到选区变化后重建完整反馈、重新记录正文及后端编码成本。
本轮完成诊断，没有实施新的反馈协议或渲染优化；屏幕呈现延迟验收仍未关闭。

## 结论

上一轮消除了点击时重复构造排版快照，但其基准排除了 CPU 帧记录、后端提交、
GPU 完成和屏幕呈现。0.23 ms 的选区处理不能证明光标立即显示。
原始通过范围见[上一轮记录](richtext-pointer-and-caret-2026-11-06.md)，不改写其基线。

这次点击期间文档版本保持不变，24 次点击全部复用排版快照，仍提交 4,488 条
命令和 `DamageMode::Full`。关闭 tracing 的 GPU 离屏回放中，debug 点击处理到
GPU 完成的 P50 为 431.208 ms，release 为 66.774 ms。
debug 放大了延迟；release 也仍承担远高于选区定位的绘制成本。

当前优先问题是编辑反馈与渲染的更新粒度。已有不可变文档和布局几何可以继续使用，
这组证据不足以要求重写整个文档模型。文字插入后的全文排版则是另一项仍开放的
问题，需要按段落失效与布局资产复用推进。

## 观察与测量范围

排查时运行中的窗口来自 `target/debug/richtext-figure-edit-app`。一次读取窗口标题
显示 20,772 字符、输入与反馈 1.16 ms、帧记录 11.54 ms、2,297 条命令。
这只是一次窗口状态观察；该标题没有报告后端提交和 GPU 完成时间，
也不据此断定当时存在活动草稿。

独立探针直接编译当前应用的 `editor.rs`、`embedded.rs` 和 `native.rs`，
使用相同文档、字体、1120 × 760 逻辑窗口、准备预算及当前 Winit 补丁。
当前工作区依赖为 Parley 0.12.0、Vello 0.11.0、WGPU 30.0.1，
Winit 为 `storytold/winit` 的 `a517b384`。这与上一轮冻结的依赖基线分开记录。

每组首、中、尾各 8 次合成 press/release，定位滚动排除在点击计时之外；
文档版本与布局快照身份逐次检查。提交经过真实 Vello GPU 管线，
每帧额外等待 GPU 完成，所以是串行诊断回放。
关闭 tracing 的 debug/release 对照均使用 GPU 离屏入口；
另有一次窗口提交的 debug 回放，点击到 GPU 完成 P50 为 435.482 ms。

后续窗口提交遇到遮挡状态，后端正常返回 `Skipped`。不能将其当作成功呈现，
因此使用同一绘制管线的离屏入口补齐配置对照及 trace。
这些结果没有包含真实 OS 鼠标事件排队、WindowServer 合成与扫描显示，
不能称为 input-to-photon；GPU 完成等待也不等于完整 GPU 执行耗时。
机器同时存在其他编译任务，绝对耗时属于本机诊断数据，不是生产预算验收。

## 关闭 tracing 的配置对照

每组 24 次，表中为 P50 / P95，单位 ms。分阶段分位数不能相加当作整体分位数。

| 阶段 | debug | release |
|---|---:|---:|
| 选区输入及反馈准备 | 3.325 / 4.077 | 0.572 / 0.880 |
| CPU 帧准备与命令记录 | 15.973 / 16.789 | 2.105 / 2.798 |
| 后端同步提交调用 | 387.983 / 398.889 | 38.705 / 58.185 |
| 提交之后的 GPU 完成等待 | 22.394 / 24.695 | 23.663 / 28.185 |
| 点击处理开始到 GPU 完成 | 431.208 / 441.858 | 66.774 / 84.067 |

两组均为 24/24 快照复用、4,488 条命令、整帧 damage。
初始正文帧为 2,297 条命令，进入编辑后为 4,488 条；
白底遮盖没有免掉已提交正文的命令记录。

首次进入会话单次观察：debug 196.305 ms，release 10.831 ms。
这项准备发生在首个点击路径中，且未包含后续帧绘制。
它不是上述 warm-click 分位数的一部分。

## Perfetto 归因

trace 使用 debug、GPU 离屏入口，开启 `novadraw.phase` 与 `novadraw.backend`；
它用于阶段归因，不混入关闭 tracing 的配置对照。
SQL 按 `probe.click` 的时间区间及 track 过滤，排除初帧、激活和定位滚动。

| 同一点击区间内的切片 | 样本数 | 平均 ms |
|---|---:|---:|
| `probe.click` | 24 | 435.576 |
| `backend-encode` | 24 | 265.253 |
| `backend-vello-render` | 24 | 118.611 |
| `backend-validation` | 24 | 4.671 |
| `backend-resources` | 24 | 0.922 |
| `ir-record` | 24 | 9.308 |
| `richtext.cache-hit` | 48 | 0.002 |

点击期间 `richtext.snapshot` 为 0 次。后端编码与 `render_to_texture` 调用合计
平均 383.864 ms，占点击区间约 88%。它们是同步调用的墙钟时间，
不能把 `backend-vello-render` 的 118.611 ms 解释为 GPU 设备执行时间。
Native 在窗口事件线程同步执行这段提交，因此后续事件和光标帧会等待它返回。

## 源码证据与优化顺序

`crates/novadraw-editor/src/viewer/mod.rs:3196–3229`：选区状态替换仍调用
`plan.feedback`，挂载新反馈，再删除旧反馈。应用的
`embedded.rs:293–321` 每次贡献白底遮盖和完整草稿正文；缓存复用没有改变
这项反馈替换行为。

`crates/novadraw-richtext/src/layout.rs:238–249`：绘制遍历全部段落，交给 Core
裁剪命令流。当前切片明确没有采用按实际绘制覆盖筛选的输出。
屏幕只看到一小部分文字，也不代表 CPU 只处理了可见文字。

`crates/novadraw/src/runtime/runtime/frame_submission.rs` 的 `try_stabilize`：
旧投影节点消失时，在缺少完整旧视觉覆盖契约的情况下保守要求完整修复。
反复替换正文反馈会进入这条路径；不能通过关闭该保护来获得表面上的局部重绘。

建议按以下顺序设计和验证：

1. **Editor 反馈增量发布**：正文贡献未变时保留其节点与资产，
   只更新 caret、selection 和 preedit 装饰，并验证旧新 damage。
   plain 与外部控制器使用同一公开协议；仍先准备候选、成功发布后才提交状态，
   拒绝时保留旧状态，不能让选区回调直接改旧反馈绕过事务。
2. **richtext 正文绘制资产复用**：进入会话时，在文档、宽度、字体和引擎证明
   匹配的前提下共享已有准备结果；正文与草稿选择有效的绘制贡献。
   后续按真实墨迹与设备覆盖契约筛选可见段落／行，复用未变的绘制资产。
   不能只依据逻辑段落高度裁剪，否则可能丢失字形 overhang、Emoji 或抗锯齿覆盖。
3. **richtext 编辑增量排版**：文字改变时只重新准备受影响段落，复用其他段落布局，
   更新高度索引和下游位置；文档与布局身份改变后重新准入光标位置。
   不把段落或字符展开成 Figure/EditPart 子树。

正文、装饰和缓存继续进入同一 Core Runtime 与后端提交边界，不另建光标 GPU
循环或第二份可变草稿。精确协议与长期边界修改需按
[ADR-061](../../adr/adr-061-edit-session-and-specialized-controller-boundary.md)
及专题设计评审后实施。
验收应覆盖真实连续点击、拖选、IME、滚动、缩放、失效恢复，以及
“输入到首个显示新光标的帧”；不能再次只用控制器 CPU 基准关闭呈现延迟。

## 本地产物与复验

探针及原始数据在 `/tmp/novadraw-click-probe-20261106/`：
`src/main.rs`、`Cargo.toml`、`Cargo.lock`、
`debug-mixed/`（窗口入口）、`debug-offscreen-mixed/`、
`release-offscreen-mixed/`、`trace-debug-offscreen-mixed/click.pftrace`。
中间产物不纳入 Git，`/tmp` 清理后需重新生成。

已验证的 SQL 保存在同目录 `warm-click-phases.sql`。
该环境的命名 daemon 未被后续查询发现，改为一次批量查询同一 trace；
trace_processor 报告 arm64 syscall 解析限制，本轮没有使用 syscall 数据。

```sh
/Users/bytedance/.trae-cn/skills/perfetto/bin/trace_processor query /tmp/novadraw-click-probe-20261106/trace-debug-offscreen-mixed/click.pftrace -f /tmp/novadraw-click-probe-20261106/warm-click-phases.sql
```
