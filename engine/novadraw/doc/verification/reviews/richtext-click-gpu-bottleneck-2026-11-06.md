# 富文本点击到 GPU 完成的 63 ms 瓶颈

类型：`verification`

能力归属：richtext 提供正文绘制；Editor 发布正文和光标反馈；Core 记录命令与
damage；Vello 后端编码和提交；Native 宿主驱动帧。日期：2026-11-06。

状态：已用 release trace 和 Emoji 消融确认局部 damage 未约束记录与编码范围；
后续[后端局部修补实施](richtext-local-damage-2026-11-06.md)已完成定向验证。
本报告保留改前诊断，不关闭屏幕呈现延迟验收。

## 结论与测量口径

63.423 ms 来自[节点与段落复用验证](richtext-incremental-feedback-2026-11-06.md)
的 mixed release 离屏回放：20,772 字符，24 次已激活会话中的合成点击，每帧串行
等待 GPU 完成。它不包含系统鼠标输入排队、WindowServer 合成和显示扫描，也不
包含首次会话激活。不能称为实际点击到屏幕显示的完整延迟。

根因是局部 damage 没有约束实际记录和编码范围。只移动光标时，修复范围已经很小，
两份全文却仍进入 Vello 编码；与修复区域无关的 Emoji 因此也被解码并处理图像资源。
当前源码的匹配消融只移除 Emoji 绘制命令，就使整体 P50 从 61.644 降到
25.065 ms，后端提交从 39.267 降到 10.089 ms。
这证明了无关绘制的成本，不能据此把 Emoji 缓存列为首要整改。
整改应先让局部修复范围约束绘制贡献的选择，在编码之前排除无关内容。

| 阶段 | 原始 P50，ms | 原始 P95，ms |
|---|---:|---:|
| 输入与反馈 | 0.332 | 0.422 |
| CPU 帧准备和命令记录 | 2.138 | 2.443 |
| 后端同步提交 | 40.233 | 48.524 |
| 提交后 GPU 完成等待 | 20.273 | 23.395 |
| 输入开始到 GPU 完成 | 63.423 | 69.372 |

各阶段分位数不能直接相加。原始 24 个样本的平均值依次为 0.326、2.185、
41.103、19.857 ms；整体平均为 64.120 ms，后端提交与等待约占 95.1%。
余量包括提交确认和探针本身的操作。

复跑同一保存的 release 探针，关闭 tracing，整体 P50 为 62.254 ms：
输入 0.314、CPU 准备 2.185、提交 37.839、等待 21.516 ms。结果支持原始
63 ms 仍可复现。该探针沿用原始依赖构建，不代表当前官方 winit 的重新验收。

## Release trace 与匹配消融

独立探针使用当前应用及引擎源码、crates.io Vello 0.11.0／WGPU 30.0.1／
Parley 0.12.0、官方 winit 0.30.13，以及默认系统 Rust/Cargo。
沿用原探针 lock 中的可用依赖版本，移除其历史 winit patch。
后端的文本 raster 实验保持默认关闭。

消融先按字体资源 bytes 与系统 `Apple Color Emoji.ttc` 的完整相等关系识别
资源身份，仅在 Core 完成帧准备之后移除对应的 `DrawGlyphRun` 命令。
文档、排版、点击位置、Core 准备流程、窗口和纹理尺寸均不改变。
两组 24 次点击的 damage 逐项一致，排版快照全部复用。
这项实验故意改变输出，不是生产修复或视觉等价验证。

关闭 tracing，按完整输出 → 移除 Emoji → 完整输出的顺序串行回放：

| 阶段 P50，ms | 完整输出 1 | 移除 Emoji | 完整输出 2 |
|---|---:|---:|---:|
| 输入与反馈 | 0.318 | 0.338 | 0.320 |
| CPU 帧准备 | 2.044 | 2.227 | 2.096 |
| 后端同步提交 | 39.267 | 10.089 | 37.567 |
| GPU 完成等待 | 21.419 | 12.272 | 21.408 |
| 输入到 GPU 完成 | 61.644 | 25.065 | 61.138 |

两次完整输出夹住消融组，支持改善并非仅来自启动后的逐步预热。
每帧原有 4,488 条命令、4,194 个 glyph run、40,366 个 glyph；
消融后为 3,334 条命令、3,040 个 glyph run、39,180 个 glyph。
仅移除 1,186 个 glyph，约占 2.9%，整体延迟却下降约 59%。
移除的是 1,154 个 Apple Emoji run，不把 run 数当作字形数。

两组另外开启 `novadraw.phase`、`novadraw.backend` 和 `novadraw.glyph`，
各捕获 24 次点击。按同线程点击区间过滤，平均值如下：

| 点击内阶段 | 完整输出，ms | 移除 Emoji，ms |
|---|---:|---:|
| `backend-encode` | 26.119 | 2.012 |
| 其中 `backend-glyph-run` 累计 | 25.004 | 1.254 |
| `backend-vello-render` 同步调用 | 17.718 | 7.049 |
| `ir-record` | 1.555 | 1.397 |
| `backend-validation` | 0.159 | 0.127 |
| `backend-resources` | 0.087 | 0.061 |
| 点击内 `richtext.snapshot` 次数 | 0 | 0 |

glyph-run 是 encode 的嵌套切片，不能再与 encode 相加。
完整组每帧 4,194 个 glyph 切片，约 95.7% 的 encode 墙钟耗时落在这一层；
移除 Emoji 后累计 glyph-run 耗时减少 23.750 ms。
结合位图分支源码，确认 Emoji 编码是本负载中最大的耗时项。
耗时归因与整改优先级需要区分：先消除局部修复中的无关全文编码，
再判断修复区域内实际需要绘制的字形是否还需要缓存优化。
Vello 同步调用和后续等待也随消融下降，支持图像资源处理造成下游成本；
这组 trace 还不能把其内部耗时精确分摊给 atlas、上传或某个 GPU pass。

trace 开启时完整组整体平均为 67.259 ms，不能与关闭 tracing 的分位数混算。
两组都用同一份带 profiling feature 的二进制，关闭 tracing 仅停止采集；
诊断检查和 profiling feature 的固定开销仍可能存在。

## 为什么局部 damage 没有带来相应收益

原始优化组全部 24 帧为 Partial，全部复用排版快照，仍有 4,488 条命令。
其中 22 个小 damage 样本的平均提交为 41.292 ms，整体平均为 64.118 ms；
另外两个大 damage 样本整体平均为 64.143 ms。后者样本很少，只用于描述，
不作为受控面积缩放实验。

整体中位数对应的实际点击样本，修复范围只有 `[40,126,29.5,34]` 逻辑像素，
提交仍耗时 42.309 ms，提交后等待 18.529 ms。这说明小修复范围已经存在，
前面的处理量却没有随之缩小。

根因由三处源码衔接起来：

1. [应用反馈](../../../crates/novadraw-richtext/examples/native/richtext-figure-edit-app/src/embedded.rs)
   的 `Plan::feedback` 在模型正文之上贡献白底遮盖和草稿正文。白底能遮住旧像素，
   不能取消旧正文的命令生成。初帧 2,297 条命令，编辑会话 4,488 条，
   原正文和草稿正文同时参与绘制。节点复用保留了这些贡献。
2. [richtext 绘制](../../../crates/novadraw-richtext/src/layout.rs) 的
   `RichTextLayoutSnapshot::paint` 遍历全部段落。它明确将实际墨迹覆盖筛选留待后续，
   当前没有只输出可见正文或光标修复范围内的正文。
3. [Vello 后端](../../../crates/novadraw-backend-vello/src/lib.rs) 的
   `SceneEncoder::encode_with_raster` 每帧创建新 Scene，然后遍历全部命令。
   damage 进入外层 clip，不能免掉发生在 clip 求交之前的字形编码。
   `render_for_screenshot` 仍以完整 `width`、`height` 调用 `render_to_texture`，
   最后才把 damage 区域从 scratch 复制到 retained texture。

本例完整目标为 2240×1520 像素，使用 `Msaa16`。crates.io Vello 0.11.0 的
`render_encoding_coarse` 先调用 resolver 处理完整 encoding，再生成渲染录制；
`vello_encoding::WorkgroupCounts::new` 根据完整目标设置 fine dispatch。
16×16 tile 对应 140×95 个 workgroup。clip 会影响实际覆盖与部分着色工作，
因此不能宣称每个像素成本完全相同；但本路径没有把修复区域变成小纹理渲染任务。

## Emoji 是无关全文编码的成本证据

crates.io Vello 0.11.0 的 `src/scene.rs` 在位图 Emoji 分支中直接读取位图数据。
PNG 分支调用 `png::Decoder`、分配解码缓冲、`next_frame`，随后创建新的
`Blob` 和 `ImageData`。上游代码注释明确提出，应移到 resolve 阶段以获得缓存。

mixed 正文重复包含 `🙂 👩‍💻 👍🏽`，另有 Emoji 专用段落；
两份全文绘制会重复进入这条路径，屏幕外字形也未在应用绘制阶段筛掉。
这是明确的重复工作机制。`vello_encoding` 的图像缓存按 `image.data.id()` 查找，
新建 Blob 会产生新身份，不能复用上一帧对应的 atlas 图像；未命中的图像进入
上传列表。具体解码、atlas 和上传耗时仍需由运行时证据区分，不能只凭源码分摊。

## GPU 时间的证据边界

`backend-vello-render` 是同步 API 的墙钟时间，包含 CPU 资源解析、场景打包、
GPU 命令编码和提交，不能直接当作 GPU 执行时间。
`wait_for_gpu_idle` 使用 `device.poll(PollType::Wait)`，测到的是提交之后尚未
完成的队列工作及完成通知等待；此前 GPU 已可能执行部分工作。
两者不能简单划分为“CPU 40 ms、纯 GPU 20 ms”。

当前可以确定后端路径占绝大多数延迟，并且全文工作量仍进入这条路径。
要把等待进一步分为上传、coarse/fine raster、纹理复制及驱动通知，需要设备
时间戳或 Metal GPU capture；本轮已有数据不支持这一级归因。

## 建议的整改顺序

首先打通 damage 到绘制贡献选择、命令记录和后端编码的约束。
光标移动的修复范围覆盖旧、新光标及必要的设备像素和抗锯齿覆盖；
与该范围不相交的正文、Emoji 和图片应在字形解码、图像生成及编码之前排除。
长文本是一整个 Figure，仅按 Figure 整体 bounds 判断仍会命中全文，
需要在其内部按实际墨迹覆盖选择段落、行或 run。

局部修复要恢复旧光标位置的底层正文和背景，再绘制新光标。
因此不能只提交新光标，也不能简单删除状态命令；变换、裁剪、绘制顺序和
覆盖该修复区域的有效贡献必须保持完整。模型正文与草稿正文的重复贡献也应
在同一绘制选择协议中处理。

后端提交的工作范围也要随修复范围缩小，或采用能免掉完整目标重建成本的等价机制。
仅设置外层 clip、最后复制小区域，不能证明前面的 CPU／GPU 工作已经局部化。
Emoji 缓存留作后续评估，只有局部修复中确实需要绘制的 Emoji 仍有重复成本时，
才据实测决定是否推进。此前消融故意删除全部 Emoji，并不是局部更新的实现。
这些改动都应通过统一 Core／后端提交路径，保留 failure、damage 和资源生命周期契约。

验收要分别覆盖正文改变、仅移动 caret、拖选、IME、滚动、缩放，以及同状态
完整重绘的像素对照。还应证明：在旧、新光标修复范围不涉及 Emoji 时，
无关 Emoji 不进入位图解码和编码；正文中远处增加 Emoji 不应增加这类点击的绘制工作量。
性能验证继续计入后端提交和 GPU 完成，不能只用控制器
CPU 耗时宣布点击延迟解决。Core 局部录制仍遵循
[既有设计](../../design/architecture/figure-drawing-recording.md)的独立复核门禁。

## 原始数据和复验

原始数据：`target/verification/text-richtext-incremental/final-after-mixed-gpu/`。
保存的探针：`target/verification/text-richtext-incremental/harness/after-probe`。

探针 SHA-256：
`e937a2c3f5e26bbdb4b08c9aee33fd4a74095c466b902ed83dacc72cf08269c9`。
原始 `samples.json` SHA-256：
`440d8d6676e90b8b25f350d3eca75d369792ea0a1ac5f1f09e6529f576a10e02`。

本轮复跑、独立探针源码及 SQL 位于
`/tmp/novadraw-richtext-click-latency-20261106/`，属于可删除的中间产物。
其中 `current-full-1/`、`current-noemoji/`、`current-full-2/` 是关闭 tracing 的
消融数据；`current-trace/click.pftrace` 和
`current-noemoji-trace/click.pftrace` 是两份 release trace。
`probe/src/main.rs` 保存消融和 glyph 计数逻辑，`phases.sql` 保存验证过的查询。

```sh
target/verification/text-richtext-incremental/harness/after-probe mixed /tmp/novadraw-richtext-click-latency-20261106/replay --offscreen
```

以下命令复现当前探针的消融和阶段查询。移除 Emoji 的截图不用于像素验收：

```sh
cargo build --offline --release -j 2 --manifest-path /tmp/novadraw-richtext-click-latency-20261106/probe/Cargo.toml --features profiling --target-dir /tmp/novadraw-text-stage-target
/tmp/novadraw-text-stage-target/release/novadraw-click-probe mixed /tmp/novadraw-richtext-click-latency-20261106/full-repeat --offscreen
NOVADRAW_PROBE_OMIT_EMOJI=1 /tmp/novadraw-text-stage-target/release/novadraw-click-probe mixed /tmp/novadraw-richtext-click-latency-20261106/noemoji-repeat --offscreen
/Users/bytedance/.trae-cn/skills/perfetto/bin/trace_processor query /tmp/novadraw-richtext-click-latency-20261106/current-trace/click.pftrace -f /tmp/novadraw-richtext-click-latency-20261106/phases.sql
```

阶段 SQL 的核心查询如下，两份 trace 使用同一查询：

```sql
WITH clicks AS (
  SELECT ts, dur, track_id FROM slice WHERE name = 'probe.click' AND dur >= 0
), per_click AS (
  SELECT c.ts AS click_ts, s.name, SUM(s.dur) / 1e6 AS ms
  FROM clicks c JOIN slice s
    ON s.track_id = c.track_id AND s.ts >= c.ts
   AND s.ts + s.dur <= c.ts + c.dur AND s.dur >= 0
  WHERE s.name GLOB 'backend-*' OR s.name IN ('ir-record', 'richtext.snapshot')
  GROUP BY c.ts, s.name
)
SELECT name, COUNT(*) AS clicks, ROUND(AVG(ms), 3) AS mean_ms_per_click
FROM per_click GROUP BY name ORDER BY mean_ms_per_click DESC;
```

此前 debug trace 的阶段 SQL 已重新执行验证：点击内平均 encode 265.253 ms、
Vello 同步调用 118.611 ms，点击内 `richtext.snapshot` 为零。
这些 debug 数字只保留历史归因效力，不用于估算 release 的阶段比例。
trace_processor 对 arm64 syscall 解析给出提示，本分析没有使用 syscall 数据。

原始样本的均值与 damage 分组可用以下 Python 复算；不对分位数相加：

```python
import json
import statistics
from pathlib import Path

samples = json.loads(Path(
    "target/verification/text-richtext-incremental/"
    "final-after-mixed-gpu/samples.json"
).read_text())
metrics = ("input_ns", "prepare_ns", "submit_ns", "gpu_wait_ns",
           "input_through_gpu_ns")
for name in metrics:
    print(name, statistics.mean(s[name] for s in samples) / 1e6)
for small in (True, False):
    group = [s for s in samples
             if (s["damage_union"][2] * s["damage_union"][3] < 10000) == small]
    print("small" if small else "large", len(group),
          {name: statistics.mean(s[name] for s in group) / 1e6
           for name in metrics})
```
