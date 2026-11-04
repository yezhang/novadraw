# TextFlowLarge 文字纹理复用实验

类型：`verification`

能力归属：Core 保持现有布局与提交协议；Vello backend 实施纹理块复用；
Native 研究宿主提供开关、GPU 离屏回放与像素读回。Editor 不在范围。

日期：2026-11-04。用户授权“按这个思路尝试下”；实验默认关闭，不作为生产启用。
方案见[纯滚动栅格复用](../../design/rendering/text-scroll-raster-reuse.md)。

## 匹配结果

最终有效裁剪版本确认，纯滚动命中纹理后可以省去文字重新编码与栅格化。
同一 release 二进制按关闭 → 开启 → 开启 → 关闭顺序串行回放；
构建检查已结束，性能采样不包含图片读回。
环境为 Rust 1.96.0、Apple M1 Pro／Metal、1200 × 1000 设备像素、DPR 2、MSAA16。
使用宿主和后端 SDK category，关闭 Core 细阶段；没有 GPU wait。

每个模式有 48 次固定滚轮样本，实际位移均为正负 40 逻辑像素；
全部发布到 GPU 离屏 target，无回放失败、trace error／data loss、
未结束区间或负自身耗时。表中耗时为墙钟，P95 为 nearest-rank。

| 固定滚轮 | 关闭缓存 | 开启缓存 |
|---|---:|---:|
| 活跃耗时中位 ms | 21.09 | 11.46 |
| 活跃耗时 P95 ms | 24.61 | 13.19 |
| host-frame 中位 ms | 14.39 | 5.18 |
| host-frame P95 ms | 17.74 | 5.88 |
| Vello 调用平均 ms | 9.61 | 0.69 |
| 最终 Vello Scene 中 glyph | 51232 | 0 |
| 冷块补画 | 不适用 | 0 |

活跃中位耗时降低约 45.7%，帧中位耗时降低约 64.0%。
两种模式的 Core IR 仍为 7609 commands／51232 glyph；
输入操作中位耗时为 6.33／6.08 ms，上游重复工作没有被缓存跳过。
这里的 0 glyph 指最终合成 Scene，不包含冷块生成的临时 Scene。

每个模式另有 48 次直接滚动和 48 个 Pointer／Stable 控制。
开启缓存的直接滚动共补画 2 块；Pointer／Stable 全为 no_damage，
不计为呈现成功。直接滚动仍有较长尾耗时；有限串行回放不能证明实际 FPS、
连续惯性滚动、事件积压或屏幕呈现已达标。细 SDK 的前轮 31 ms 也不能与本表
直接相减，净收益只取同构建、同观测边界的这组对照。

## 已验证的行为

内容坐标中的文字纹理可以跨滚动复用。初次细阶段离屏 trace 的 48 次正负 40
逻辑像素滚动中，119 次可见块查找命中、1 次补画；24 次滚轮样本全部无冷块。
热帧仍执行 Vello 合成，但无需重新处理该范围的 glyph。
Core 仍每帧录制 7609 条命令、51232 glyph，滚轮期间仍有 48 次 Flow build，
即每次两次测量组装。实验没有消除排版上游的重复工作。

该 trace 中滚轮的活跃中位耗时为 23.54 ms，host-frame 为 5.61 ms；
Vello 合成调用平均 0.81 ms，缓存检查平均 0.22 ms，encode 平均 0.59 ms。
这些是开启细 SDK 的墙钟区间，不能与前轮 Native 呈现基线直接算净收益。
最终匹配结果采用上节的相同离屏路径、构建和观测边界。

首份 Native 试跑遇到 macOS 锁屏，八次 surface 提交均为 `Skipped`，
没有完成滚动。该 trace 和两份未产生图片的 Native 像素试跑均保留，
不作为性能或像素通过证据。后续采用既有 `render_for_screenshot` GPU 路径，
记录为 `gpu_offscreen_published`；没有 drawable 获取、窗口 blit 和 present。

## 像素、失效与容量

关闭／开启缓存各保存 43 帧 GPU target，包含连续向下滚动 3200 逻辑像素、
回滚、追加 `Z`、宽度 600 → 400 → 600 和重置。两组图片序列及尺寸一致，
共比较 50,800,000 个像素。

| 检查 | 实测 |
|---|---|
| 像素完全相同的帧 | 0／43 |
| 有差异的像素 | 361,140；大部分只差 1 色阶 |
| 任一通道差异超过 1 | 409 个像素，占 0.000805% |
| 最大通道差异 | 32／255 |
| 全部通道平均绝对差异 | 0.005423／255 |
| 原始纹理最大驻留 | 14,745,600 bytes，低于 16 MiB |
| 追加后的首帧 | 0 命中、2 冷块 |
| 变宽到 400 的首帧 | 0 命中、2 冷块 |
| 恢复到 600 的首帧 | 0 命中、3 冷块 |

因此不能声称逐像素相同，也不满足全部通道差异 ≤1；本轮没有接受新的抗锯齿容差。
首帧超过 1 的差异只有 3 个像素，位于字形边缘而非块边界。另将关闭缓存的首帧与向下滚动
200 后的重叠正文按 400 设备像素平移比较，665,190 个像素中也有 8 个差异，
最大 16。这说明当前直接路径本身不保证整数平移后的位级覆盖一致；
尚不足以把全部差异归因于某个浮点算法。生产启用前仍需确定抗锯齿容差与原因。

连续滚动不是全部热帧：验证中的 35 次滚动有 24 帧需新增一块，
其余 11 帧全部命中。冷块仍编码并 resolve 完整文字范围；
该验证含同步读回，不作为连续滚动性能结论。
有预算的预取、冷块的安全可见文字提交，以及字体／DPI／复杂 clip 的扩展验收
仍待后续处理。

GPU 驻留统计只包括缓存注册，不包括 Vello atlas、临时分配、
在途 GPU 工作仍保留的淘汰纹理及 CPU backing。
不合格绘制、亚像素平移、可见边界未对齐的矩形 clip、路径 clip 或不同 draw clip
继续走原路径。屏幕外的分数边界不影响资格；最终版本的追加与 resize 均确认
eligible=true、0 命中并重新补画，未把原路径回退误当作缓存正确。
旧资源/session 不命中，失败继续经 Runtime 原确认与恢复协议处理；
本轮没有实机注入 device loss，不能据既有恢复单测关闭该验收。

## 复跑与本地产物

细阶段 trace 与分析分别位于：

`target/verification/textflow-scroll-20261104/raster-offscreen-first/`

`target/verification/textflow-scroll-20261104/raster-offscreen-first-analysis/`

最终匹配 trace、逐帧统计和哈希清单位于同根目录的 `accepted-clip/`：
`off-a/`、`on-a/`、`on-b/`、`off-b/` 为原始 trace，
对应 `*-analysis/` 为解析结果，`pooled.json` 为汇总，`manifest.json` 为哈希。
该目录名只标识通过有效 clip 条件的实验版本，不代表设计或生产启用已接受。
二进制 SHA-256：
`c689c0390dbde540e2a416e032bac03a2f2d73b3607e90d16abb1d42581633c2`。

最终像素对照使用 `final-pixels-reference/` 与 `accepted-clip/pixels-cached/`；
结果为 `accepted-clip/pixel-comparison.json`，与首次对照的差异分母一致。
最终失效／容量解析位于 `accepted-clip/validation-analysis/`。
这些文件均为忽略的本地产物，清理 target 前应另存。

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --toolchain 1.96.0 --clt --target-dir /tmp/novadraw-text-stage-target -- --scroll-replay --natural-damage --offscreen-gpu --text-raster-cache --coarse-trace --exit-after-replay --output target/verification/textflow-scroll/raster
python3 tools/scripts/run_textflow_large_macos.py --release --toolchain 1.96.0 --clt --target-dir /tmp/novadraw-text-stage-target -- --raster-validation --offscreen-gpu --text-raster-cache --trace --exit-after-replay --output target/verification/textflow-scroll/pixels-cached
```

关闭缓存的对照移除 `--text-raster-cache`，其余参数保持一致。
[分析器](../../../tools/scripts/analyze_textflow_scroll.py)先检查 trace health，
再关联 operation、frame 与缓存记录；失败回放拒绝归因。
[像素比较](../../../tools/scripts/compare_text_raster_frames.py)使用 Pillow，
同时报告完整序列、尺寸、平均差异和最大差异，不将成功运行当作像素通过。

真实 OS 连续滚轮、窗口呈现、DPI、复杂文字／clip，以及无采集净成本仍待验收。
本实验不关闭 FR-02、LT-02／03 或整个长文本性能队列。

## 代码与文档验证

后端 library 回归 41 项通过，1 项既有 release 成本测试保持 ignored。
缓存等价、样式失效、亚像素滚动／clip、不同 clip 和不支持的中间绘制走回退，
由两个定向测试覆盖；最后收窄 clip 条件后已重跑。
后端 Native／SDK 与无默认 feature 的 Clippy、研究应用 Clippy 通过；
workspace check、workspace fmt 检查和文档门禁通过。没有执行 workspace 全量
Clippy／test，不作为生产里程碑关闭；没有 Git 提交。

粗阶段试跑曾关闭宿主根区间：宿主借用 Core category，禁用细 Core 打点时一并禁用。
现已改为宿主直接使用 SDK 的 `novadraw.host` category，关闭路径不创建 scope；
`matched/` 中缺根区间的四份 trace 不用于耗时归因。
`matched-host/` 与 `coarse-*/` 的开发期采样未作最终比较，
最终匹配采集在本轮构建检查结束后串行执行。
`final/` 和 `final-pixels-cached/` 为过严 clip 条件的反例：整个内容边界存在分数值，
即使该边界在屏幕外也被拒绝，48 次滚轮全部 eligible=false、仍编码 51232 glyph；
43 帧完全相同只证明回退，不能证明缓存正确。最终版本只检查有效可见边界，
反例单测、实际命中与像素对照均已补验；未按这份回退数据宣称优化完成。
