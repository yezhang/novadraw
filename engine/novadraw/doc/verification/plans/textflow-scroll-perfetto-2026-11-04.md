# TextFlowLarge 滚动卡顿采集计划

类型：`verification-plan`

能力归属：Core 的输入、viewport、准备和 IR；Vello backend 的命令处理与提交；
Native 样例宿主的输入、帧调度及回放。Editor 不在范围。

日期：2026-11-04。用户已授权针对滚动卡顿增补 Perfetto SDK 埋点、
生成新 trace 并给出诊断和解决方案。当前正在实施采集，不修改引擎行为契约。

## 要回答的问题

滚动后的慢帧分别花在输入适配、Core 更新、重复布局/Flow 组装、IR、
backend 编码、surface 获取还是提交/等待？宿主每次输入都执行 Stable 并强制
full redraw，是否放大了观测到的卡顿？普通 glyph 路径与 AA-OFF CPU 准备是否被误混？

## 埋点和关联

使用官方 SDK；时间戳、缓冲和 trace 编码继续由 SDK 管理。
Core 既有阶段保持不变，新增样例与 backend 诊断埋点。
backend 使用默认关闭的 Native 可选 feature，领域签名不暴露 SDK 类型。
细粒度 glyph/AA 事件单独按 category 控制，避免关闭时生成参数或分配记录。

| Owner | 增补区间与事件 | 关联事实 |
|---|---|---|
| Native 宿主 | wheel 输入、Core dispatch、稳定查询、等候重绘、draw 回调 | 输入种类、phase/delta、operation、帧尝试、滚动前后状态 |
| Core 消费者 | 实际 submission 生成与确认 | full/自然 damage、区域、surface 尺寸、命令和 glyph 数 |
| Vello backend | validation、AA 准备、资源、resize、damage、encode、texture、surface、render、blit、queue、present | 同步嵌套于 host-frame，正常与错误路径均结束区间 |
| Vello 文字/AA | glyph run 编码、特殊轮廓、必要时 AA 缓存/采样 | AA 模式与真实分支，不假定普通文字走 CPU mask |

GPU wait 若启用，作为独立同步诊断，不能当作 GPU timestamp 或屏幕呈现时间。
Vello/MSAA、字体、Core 默认预算等保持原场景配置。

## 重新采集

在真实 Native/GPU 宿主中进行固定滚动回放。区分直接 viewport 滚动与通过
既有 Winit adapter 的固定滚轮输入，并保留 Stable/指针活动控制组。
固定回放不是人工 OS 输入到屏幕的延迟测量；手动滚轮事件使用相同新增埋点。
主对照为原 full redraw 与自然 damage，使用相同构建、字体、步数和 delta。
优先 release；若受环境限制改用 debug，须明确标注，不能用作 release 验收。

回放开始先建立并预热可见场景，各操作等待帧确认后推进；
失败/Skipped/Retry 保留并单列。记录实际位移，零位移和边界钳制不计为成功滚动样本。
trace 与分析导出放在显式本地目录，另记录 hash 和路径，避免依赖已丢失的旧 target。

## 分析和交付

使用官方 Perfetto skill 和持久 Trace Processor 会话，先检查采集丢失和未结束区间。
按输入与帧关联统计，计算直接子区间扣除后的自身耗时，避免重复累加嵌套阶段。
报告描述性样本分布、实际次数和成功/失败分母；不把合成回放数据写成真实 OS 延迟，
不把 submit 或 GPU queue wait 写成 GPU 执行或 compositor 完成。

最终交付新增 trace、可复核 SQL、滚动专用诊断及按 owner 划分的可能方案。
没有运行时证据的解释保持候选，不把观测方案直接写成生产优化完成。

## 采集调整记录

第一份 Native release 配对 trace 已生成，但出现 producer 共享内存满导致的
数据丢失：`traced_buf_data_loss_smb_full=1`、sequence loss=6、
writer packet loss=5，不能用于完整成本归因。
保留原文件，将 SDK producer 共享内存 hint 调整为官方允许的 32 MiB，
session 缓冲调整为 256 MiB，并缩短为每组一轮，重新采集。
这属于采集配置调整，未修改 Core 或 backend 的渲染与缓存行为。

补采已通过健康检查，完成 96 个有效滚动样本和 48 个控制样本。
结果、原始文件 hash、可复跑 SQL 与方案见
[滚动诊断](../reviews/textflow-scroll-perfetto-2026-11-04.md)。
