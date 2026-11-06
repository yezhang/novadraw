# 富文本点击的局部 damage 优化

类型：`verification`

能力归属：Vello backend。Editor／richtext 沿用既有会话与反馈；Core 的命令记录
范围未改变。日期：2026-11-06。

状态：后端局部字形选择与小纹理修补已实现，定向测试和 mixed release 离屏复测
通过。Core 局部录制、完整 IME／滚动／缩放组合及屏幕呈现延迟仍未验收。

## 结果与口径

[此前诊断](richtext-click-gpu-bottleneck-2026-11-06.md)确认，只移动光标时，
局部 damage 没有阻止两份全文进入字形编码。本轮在编码之前排除与 damage／clip
无关的字形和图片，并按修补范围缩小 GPU 临时目标。完整输出的点击到 GPU 完成
P50 从 61.644 ms 降到 7.614 ms，降幅约 87.6%；第二次复跑为 8.443 ms。
没有删除 Emoji 或改变文档内容，文本 raster 实验保持关闭。

两轮各 24 次合成点击，mixed 共 20,772 字符，按头部／中部／尾部交替移动光标，
每帧串行等待 GPU 完成。全部排版快照复用，全部提交为 Partial。使用相同
release profiling feature，以下数据关闭 tracing。目标为 2240×1520，DPI 2，
Vello 0.11.0、WGPU 30.0.1、官方 winit 与默认系统 Rust／Cargo。
该口径不包含系统输入排队、WindowServer 和显示扫描。

| 阶段 P50，ms | 改前 | 改后 1 | 改后 2 |
|---|---:|---:|---:|
| 输入与反馈 | 0.318 | 0.327 | 0.421 |
| Core 帧准备 | 2.044 | 1.991 | 2.166 |
| 后端同步提交 | 39.267 | 3.809 | 4.118 |
| GPU 完成等待 | 21.419 | 1.398 | 1.386 |
| 输入到 GPU 完成 | 61.644 | 7.614 | 8.443 |

改后整体 P95 分别为 11.959、14.454 ms。分位数不能直接相加。
首次会话激活和首帧 GPU 初始化不计入点击样本，仍有独立成本。

## 实际改动

私有边界见[后端局部 damage 契约](../../design/architecture/backend-local-damage-selection.md)。
普通字形使用实际轮廓墨迹边界；位图字形读取 strike 尺寸、bearing 和原点，
按 Vello 的字号、skew 与变换顺序计算边界。选择发生在 `draw_glyphs` 之前，
因此远处的位图不会进入 PNG 解码或图像编码。图片按变换后的目标范围判断。
这是一套统一的范围判断，不是 Emoji 缓存。

设备空间边界保留 2 像素覆盖余量。COLR、stroke、无法确定的轮廓和超过缓存
variation 维度的字形继续绘制，避免漏画。缓存上限为 65,536 条、64 个 variation
坐标；session、snapshot、资源 upsert／remove 均清理对应条目。
状态命令按原顺序执行，Restore／ResetClip 只能改变内部 clip，不能解除 damage。

修补矩形按 16 像素 tile 对齐，完成的场景平移整数设备像素，再渲染到相应尺寸
的 scratch texture；各 damage 区域使用相对原点复制回 retained texture。
原生 `submit` 与 `render_for_screenshot` 使用相同规则。22 次局部移动的目标
分别为 64×80、48×64、16×64；另外两次跨区域定位仍触发保守的大 damage，
目标分别为 1792×1520、1616×1520，不能将这两帧描述为光标大小的修补。

## Trace 与剩余成本

另开 tracing，按同线程 `probe.click` 的包含关系过滤 24 次点击：

| 阶段平均墙钟，ms | 改前 | 改后 |
|---|---:|---:|
| `backend-encode` | 26.119 | 2.972 |
| `backend-vello-render` 同步调用 | 17.718 | 0.736 |
| `ir-record` | 1.555 | 1.425 |

实际进入 Vello 的 glyph run 从 24×4194＝100,656 个降到 329 个；
7 次点击没有需要重新编码的字形，其他点击只恢复修补区下的正文。
点击内 `richtext.snapshot` 为零。后端仍需扫描候选字形并查询边界；
Core 每帧仍记录 4,488 条命令、4,194 个 run、40,366 个 glyph。
这轮解决了无关编码和完整尺寸 GPU 临时目标，未完成整条记录链的局部化。
后续 Core 录制复用继续遵循已有独立复核门禁。

`backend-vello-render` 是同步 API 的 CPU／提交墙钟，GPU 等待是尚未完成队列工作
及通知等待；本报告不把二者分别等同于纯 CPU 与纯 GPU 时间。

## 正确性与验证边界

后端 `--lib` 共 45 项通过、1 项成本测试保持 ignored。新增测试覆盖 clip
Restore／Reset、未知 stroke 保守路径，以及 DPI 1／1.5／2 下的轮廓、实际 Sbix
位图、skew、仿射变换和整数 tile 平移，共 36 组修补区像素对照。
远处位图的测试直接检查 Vello encoding 中没有资源 patch。
已有资源替换测试补查墨迹缓存的 session／snapshot／upsert／remove 清理。

普通轮廓和未旋转位图要求像素严格相等；旋转位图的 f32 坐标重定位允许各通道
最多 1/255 的末位舍入差异。当前应用第一轮最终 retained 与同状态 full 截图相等，
第二轮有 6 个像素相差最多 1/255；
改前与改后的 full 截图有 110 个像素的通道差异，最大为 1/255。改后两次 full
之间另有 6 个像素的末位差异。额外在头部／中部／尾部各抓取 retained 与同状态
full，对应 297／180／43 个像素有差异，最大均为 1/255，没有光标残影等级的
差异。因此不宣称所有浮点渲染组合都逐字节不变。

Backend `check --features native --lib`、严格 Clippy、定向 rustfmt 和文档检查
通过。未修改 Core 渲染主循环，没有新增公开 API，也没有接入 Figure 回调录制。
本轮不是长文编辑、系统 IME 或所有滚动／缩放路径的完整验收。
可见窗口探针在启动回放中返回 `Skipped`，未取得连续 `Presented` 的采样；
本轮不把离屏收益换算为实际屏幕呈现延迟。

## 本地证据与复验

数据、trace、SQL 与探针源码保存在
`target/verification/text-richtext-local-damage/`，不纳入 Git：
`current-full-1/2` 为改前，`local-damage-full-1/2` 为改后；
`current-trace/` 与 `local-damage-trace/` 分别保存阶段 trace。
`region-pixels/` 保存头／中／尾三对同状态截图。
`pixel-comparison.json` 保存比较收据；`probe/` 保存独立测量入口，
`phases.sql` 保存同线程阶段查询。

```sh
cargo test --offline -p novadraw-backend-vello --features native --lib -j 2
cargo clippy --offline -p novadraw-backend-vello --features native --lib -j 2 -- -D warnings
cargo xtask docs
cargo build --offline --release -j 2 --manifest-path target/verification/text-richtext-local-damage/probe/Cargo.toml --features profiling --target-dir /tmp/novadraw-text-stage-target
/tmp/novadraw-text-stage-target/release/novadraw-click-probe mixed target/verification/text-richtext-local-damage/repeat --offscreen
```
