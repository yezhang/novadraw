# Core 按局部 damage 选择轮廓文字

类型：`verification`

能力归属：Core Graphics 拥有记录范围与文字索引查询，Runtime 决定完整修补；
Vello backend 提供现行填充路径覆盖声明和像素对照。Editor／richtext 无新增协议。
日期：2026-11-06。状态：内部切片已实现，Core 回归、定向验证与最终采样通过。
本片不关闭 FR-02、默认 native 正文记录或屏幕呈现验收。

`api_semantics`：`graphics.context`、`paint.protocol`、`clipping.strategy`、
`damage.repaint`、`frame.preparation`。

## 问题与目标合同

Core 在绘制前已经持有投影到 surface 的 damage，但文字查询只读取有效 clip。
因此局部光标重绘仍可能生成整个 viewport 的轮廓路径。
本片让已取得覆盖资格的轮廓输出同时受 damage 约束，
保持 Figure 回调、全文布局、资源身份和 painter 可观察 clip 的现行语义。

Draw2D 的只读依据为 `DeferredUpdateManager.repairDamage()`：
先把 dirty region 沿父链投影并相交，再用 `getGraphics(damage)` 调用 `root.paint`。
本片采用局部修补的输出范围语义；不修改 Java，不修改 Core 递归渲染主循环。
现行账本见[Draw2D API 覆盖](../../parity/draw2d/api-coverage.md)。

## 实际机制

`NdCanvas::covered_text_clip` 把 damage union 与有效 clip 的覆盖外扩包络相交，
再用当前 transform 的逆映射换回 layout 坐标，并减去文字 origin。
外扩沿用当前 backend 的 `filled_path_coverage_outset_pixels`，按 DPI 换算。
没有声明、映射奇异或包络无法证明时继续完整退化。
多个 damage region 使用保守 union；本片没有逐 region 的稀疏查询。

该范围仅供现有 OutlinedText／FlowSnapshot 的包络索引选择输出。
`PaintContext::effective_clip_bounds` 仍返回当前实际 clip，
ResetClip 也不会撤销 Runtime 的 damage 约束。未知 callback 仍在原位置执行；
没有新增回调缓存、公共容器或文字专属准入 hook。
native 字形、描边和未知包络仍走现有保守路径。

完整修补必须在记录前决定。Runtime 已知 session 恢复、Full 请求或 backend
不支持 Partial 时，关闭 damage 选择，继续保留正常 viewport clip 选择。
这样完整输出可以安全标为 Full，并避免先局部记录再重复调用所有 painter。
如果一个实际受 damage 限制的记录后来升级 Full，则重新完整记录，
不能仅改 damage 标签。host background 沿用既有完整重绘合同。

源码入口：
[Graphics](../../../crates/novadraw/src/render/context.rs)、
[内部 update 入口](../../../crates/novadraw/src/graph/mod.rs)、
[Runtime 帧准备](../../../crates/novadraw/src/runtime/runtime/frame_submission.rs)。

## 定向验证

| 验证 | 结果与范围 |
|---|---|
| 公开消费者 `long_text_paint_contract` | 5 项通过；包括局部输出缩小、Full 恢复、无证书／描边退化，以及远处墨迹与负 bearing 原反例 |
| Graphics 状态测试 | transform／origin 换算、ResetClip、painter 可观察 clip 和奇异映射退化通过 |
| Vello GPU 对照 | `core_outline_damage_selection_preserves_repaired_pixels` 通过；DPI 1／1.5／2，真实非空字形与小数 damage，复制区域全部 RGBA 差值不超过 1 |
| Core 定向 Clippy | lib 与 `long_text_paint_contract` 的 `-D warnings` 通过；lib test 的既有 dead-code／未消费确认告警不算本片新增 |
| Core lib 回归 | 538 项通过，8 项 ignored；ignored 包含本片显式执行的 release 成本探针 |
| 全仓格式检查 | 并行 Editor 的 `src/viewer/failure.rs` 存在格式差异，本片未修改该文件；不记为全仓格式通过 |
| 工作区编译与文档 | `cargo check --workspace` 通过；`cargo xtask docs` 533 文件通过；本片直接修改文件的 rustfmt 检查及 backend lib Clippy 通过 |

GPU 对照从相同 Runtime 先生成完整输出，确认 Presented 后请求局部 repaint，
直接比较局部记录与完整输出在实际向外取整复制区域的全部像素。
该夹具不覆盖 native glyph、真实窗口、跨平台或任意极端字体／数值输入。

## 成本采样口径

release 探针在同一二进制中交错执行两种记录方式，复用同一已准备布局：
基线在生成命令后附加 damage；优化在生成前附加同一 damage。
两者的字体、文字、clip、coverage 声明及最终 DamageSet 相同。
预热 20 对，正式 400 对，关闭 tracing；计时包含 canvas 构造、clip／paint 设置、
索引选择和路径生成，不包含首次 layout、树稳定化、GPU 或屏幕呈现。
同机并行构建及其他任务的负载未受控，两轮基线耗时不同；结论限定为此配对夹具。

负载为 Inter 16 px，`AB CD EF GH\n` 重复 2,000 次；
clip 为 100 × 800，damage 为 `(4, 8, 2, 12)`，覆盖外扩为 2。
首轮 P50 从 64.041 μs 降至 3.875 μs，P95 从 125.834 μs 降至 4.375 μs；
选中字形 320 → 1，path operations 7,240 → 18。
这是有限轮廓夹具的记录成本，不能代替富文本 native 点击延迟。
最终复测 P50 为 50.333 → 3.833 μs，P95 为 75.583 → 4.167 μs，
选择数量与路径操作数相同。正式 400 对原始 CSV 与
`recording-cost-final.log` 保存在本地产物目录。
首次 CSV 导出因 Cargo 测试工作目录不同而失败；改用绝对输出路径后复测通过，
失败日志仍保留在 `recording-cost.log`，不计为有效采样。

复跑：

```bash
NOVADRAW_LOCAL_DAMAGE_SAMPLES="$PWD/target/verification/text-core-local-damage/recording-samples.csv" \
  cargo test --release -p novadraw --lib \
  render::context::tests::local_damage_recording_cost_probe -- --ignored --nocapture
```

运行前确保输出父目录存在。探针默认 ignored，不进入常规性能门禁。
日志、CSV 与源码摘要放在 `target/verification/text-core-local-damage/`，不纳入 Git。

## 剩余局部化边界

默认 native 正文在 Core 仍逐段落／run 生成命令。后端私有 damage 筛选无法省去
已经发生的这部分记录；现行填充路径 outset 也不授予 native 覆盖资格。
下一片需要静态墨迹摘要、操作级覆盖绑定和原序输出范围，
不能用高度树或固定像素常量删除未知 native 字形。

不可变命令段的候选仍受[录制专题](../../design/architecture/figure-drawing-recording.md)
与独立复核门禁约束。本片只使用现行协议，不提前实施候选公开 stream。
此前高性能技术的采用情况见[核对清单](text-performance-adoption-inventory-2026-11-06.md)。
