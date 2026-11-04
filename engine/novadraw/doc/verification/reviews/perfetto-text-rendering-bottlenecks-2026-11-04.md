# Perfetto 文本渲染性能瓶颈分析

类型：`verification`

能力归属：Core Text、Flow、准备与 Render IR；Vello backend 提交；
Native 研究宿主的输入和全量帧编排。Editor 不在分析范围。

日期：2026-11-04。采用官方 Perfetto AI skill 的查询流程和其随附
Trace Processor v59.1，实际加载 trace、发现表结构并查询持久 Unix 会话。
本轮仅分析与记录，不修改生产实现。

## 当前应优先处理什么

文本渲染慢路径首先应检查 **Flow 重复组装和准备校验**，随后细分 backend 提交。
文字引擎内部主要耗在分析/shaping，但它在当前整个已记录活跃区间中只占 4.75%。
现有段落缓存已消除了大量文字引擎调用，继续只优化换行或缓存查找不能覆盖主要成本。

最有区分力的反例是放大回原宽度：文字引擎调用为零、段落查找全部命中，
操作仍耗时 568.02 ms，并有 13 次 Flow build。普通交互中也有 14 次 Stable
各执行两次测量组装，中位耗时 101.68 ms。缓存后的构造与校验路径仍需要复核。

## 输入和测量口径

现有四份 Native trace 的通用诊断、SHA-256、121 个操作和 31 个帧的归因见
[前轮完整诊断](perfetto-trace-diagnosis-2026-11-04.md)。
本轮重点查询交互覆盖最充分的：

`target/verification/textflow-large-app/textflow-1793772166039786000-92502-0.pftrace`

该文件包含 80 次操作、15 次帧尝试，配置为 debug、Rust 1.99.1、
Native/Metal、完整请求采集关闭、GPU wait 关闭、研究帧 full redraw。
SDK 埋点的开销尚未单独测量；所有耗时是带观测的 debug 墙钟区间。
这些结果用于定位当前路径，不能直接用作 release 延迟、CPU 利用率或掉帧结论。

本轮重新查询得到：无未结束 slice、无负自身耗时、无正式 error/data_loss 统计。
文字引擎的 14 个区间位于名为 `main` 的线程；没有调度或 CPU 采样数据。
进程名称字段为空，身份与配置沿用业务记录，不能虚构线程或进程名称。

本轮官方查询成功完成后，写入最终分析数据时遇到磁盘空间不足；
随后检查发现原 `target/verification/textflow-large-app/` 目录已不在原路径。
本文保留已成功查询的数字和 SQL；没有重新采集或补造结果。
复跑需先找回原 trace，T2 的原 SHA-256 为
`9ee2bc5c8e766c8bf950334e9e742a64539314bf3e1d803b6daba14a3d0d0ead`。
未判断目录消失的原因，也未执行缓存或 trace 清理。

自身耗时为 slice 时长扣除其直接子区间时长。本 trace 的阶段同步嵌套，
没有跨 track 子区间或负值；分类求和与根区间总和相等。
因此以下分类不会把 Flow、文字引擎和外层稳定化重复相加。

## 整条链路的成本分布

分母为全部已记录、已结束根区间的累计墙钟时长 **7513.18 ms**，
包括输入、操作和帧区间。它不等于整个录制期间的时间，也不等于 CPU on-CPU 时间。
分类按 slice 名称归属，余项保留为“其他”，不将未知耗时强行归到文字绘制。

| 分类 | 累计自身耗时 ms | 占根区间总和 |
|---|---:|---:|
| Flow 贡献与文本组装 | 2372.00 | 31.57% |
| 准备签名与批量校验 | 1820.12 | 24.23% |
| backend submit | 1460.68 | 19.44% |
| 其他已记录工作 | 1211.20 | 16.12% |
| 文字分析/shaping | 357.04 | 4.75% |
| IR 录制、校验与 submission 构造 | 275.38 | 3.67% |
| 其他文字引擎阶段 | 16.75 | 0.22% |

Flow 与准备校验合计约 **55.80%**。这个百分比说明当前采集中的成本集中位置，
不能解释为移除相应工作后可获得同等提速。
本轮 Flow 集合包含 `flow-text-append`；前轮组装集合未包含它，
所以本轮 2372.00 ms 与前轮 2274.88 ms 的差异是口径增加，不能当作性能变化。

## 文字引擎内部的瓶颈

`flow-engine-layout` 共 14 次，累计 **374.34 ms**：

| 引擎子阶段 | 累计耗时 ms | 占引擎区间 |
|---|---:|---:|
| text-analysis-shaping | 357.04 | 95.38% |
| text-export | 12.92 | 3.45% |
| text-wrap | 2.05 | 0.55% |
| text-validation | 1.44 | 0.38% |
| text-interaction | 0.29 | 0.08% |
| text-align | 0.06 | 0.02% |

其余约 0.55 ms 是 `flow-engine-layout` 自身未被这些子阶段覆盖的时间。
换行、对齐和 interaction map 构造都不是当前已观测到的主要成本。
如果下一轮明确要优化文字引擎内部，应先细分 analysis/shaping，而不是优先重写换行。

[analysis/shaping 区间](../../../crates/novadraw/src/render/text.rs#L1778-L1848)
包括方向前缀与文本构造、字体回退与 font family 解析、span 样式、
Parley ranged builder 和 `builder.build(text)`。
这不是独立的 HarfRust 调用区间，357.04 ms 不能全部归因于 HarfRust。
[换行与对齐](../../../crates/novadraw/src/render/text.rs#L1849-L1879)
位于后续独立区间；
[导出](../../../crates/novadraw/src/render/text.rs#L1881-L2072)
覆盖行指标、glyph run 等 Core 布局输出的构造。
导出 Core glyph 数据不等于 GPU 轮廓准备或光栅化。

## 缓存已命中，组装仍重复

代表性 trace 有 **2960 次段落查找、14 次文字引擎调用**。
按这条同步成功路径的查找/调用关系，推得 2946 次命中，命中率约 **99.53%**。
查找累计只耗时 **12.41 ms**，平均约 4.19 微秒；它不是当前最大的热点。
这里推导的是查找分支命中率，不是新增的自研采集计数。

| 操作 | 实际引擎调用 | 引擎区间 ms | analysis/shaping ms | 查找次数 |
|---|---:|---:|---:|---:|
| Reset（21） | 6 | 159.56 | 152.45 | 520 |
| Append（23） | 2 | 70.84 | 67.67 | 200 |
| Stable（43） | 0 | 0 | 0 | 80 |
| Scroll（93） | 0 | 0 | 0 | 80 |
| Resize 400（95） | 6 | 143.94 | 136.92 | 520 |
| Resize 600（97） | 0 | 0 | 0 | 520 |

Resize 600 的 520 次查找累计约 2.09 ms，来源几何阶段约 0.99 ms；
与操作总时长 568.02 ms 相比很小。该操作重复做了 13 次 Flow build，
其中 11 次在 layout 测量路径、两次在准备 callback 下。
前轮列明的组装自身集合在该操作中约 399.90 ms。

Stable（43）与 Scroll（93）各有两个测量 build，没有引擎调用。
这支持优先排查测量触发次数及贡献组装复用资格。
由于没有开启 `novadraw.request`，仍缺少完整请求、source prefix 与依赖版本证据；
不能仅凭正文和宽度相同，断定不同测量可直接共享结果。

## 稳定校验与帧提交的限制

前轮对第三份 trace 的独立检查已确认：没有 Flow build 或文字引擎调用，
Stable 中位耗时仍为 21.90 ms。准备签名和批量校验累计占稳定化区间约 85.3%。
因此完全消除 shaping 仍不能消除这类稳定查询的校验底成本。

代表性 trace 的 15 帧平均总区间约 138.02 ms，backend submit 平均约 97.38 ms。
Core IR 录制和校验分别累计 134.64 ms、127.03 ms。
这些区间已经覆盖所有图形命令，不能从中单独推导每个 glyph 的成本。
研究宿主强制 full redraw，常见帧约 7609 条命令和 51233 个记录 glyph，
不能作为默认增量 damage 路径或实际可见文字量的分母。

重新查询确认，**独立 glyph、outline、GPU 阶段 slice 数为零**。
现有埋点不能回答以下问题：

- 轮廓准备是否重复、缓存是否命中，以及它在帧中的耗时。
- Vello Scene 编码、资源同步、surface 获取各占多少。
- backend submit 的长区间有多少来自驱动等待或 GPU 执行。
- GPU 完成、WindowServer 合成和屏幕呈现的时间。

这些都是待补的观测项，不是已经确认的瓶颈或缓存缺陷。

## 后续排查顺序

| 优先级 | Owner | 应先确认的事实 | 对应验证 |
|---|---|---|---|
| 高 | Core Flow/Layout | Stable 为什么再次触发测量；重复 build 的完整输入与依赖是否等价 | requests category、来源版本、候选与已发布快照区分 |
| 高 | Core Preparation | 缓存命中时哪些签名与批量检查重复遍历；哪些结果可按受检版本复用 | 拆分 signature/source prefix、依赖保留、route 校验 SDK 区间 |
| 高 | Vello backend | submit 内 Scene 编码、资源、surface 与 GPU 工作分别占多少 | SDK 子阶段及真实 GPU timestamp；不以 submit 返回当作 GPU 完成 |
| 次 | Core Text/Parley | analysis/shaping 中字体回退、样式构造与实际 shaping 的比例 | 细分已有 SDK 区间，再评估阶段复用或算法优化 |
| 次 | Core Render/Graphics | glyph/outline 准备的生命周期、缓存和帧成本 | 独立 SDK 区间及实际输出等价验证 |

下一轮性能对照应使用相同 release 构建、字体、场景、输入与预算，
区分未编译观测、编译但不采集、阶段采集和完整请求采集。
观测完善后再选择整改，不通过删除校验、放宽一致性或引入新的自研聚合器降低数字。

## 查询与复核

查询结果和分类集合保留在本文，查询辅助脚本在
`/tmp/novadraw-perfetto-text-focus-20261104.py`。
独立 JSON 导出未能完成保留，不将它列为可交付文件。官方 Unix 会话已关闭。

关键文字引擎查询如下：

```sql
SELECT name,count(*) AS calls,sum(dur)/1e6 AS ms
FROM slice
WHERE dur>=0 AND (name GLOB 'text-*' OR name='flow-engine-layout')
GROUP BY name ORDER BY ms DESC;
```

整条链路分类先生成直接子区间扣除值，再按不重叠集合求和：

```sql
CREATE OR REPLACE PERFETTO TABLE text_perf_self AS
WITH children AS (
  SELECT parent_id,sum(dur) AS ns
  FROM slice WHERE dur>=0 GROUP BY parent_id
)
SELECT s.*,s.dur-coalesce(c.ns,0) AS self_ns
FROM slice s LEFT JOIN children c ON c.parent_id=s.id;

SELECT sum(dur)/1e6 AS root_ms
FROM slice WHERE parent_id IS NULL AND dur>0;

SELECT CASE
  WHEN name IN (
    'flow-visit','flow-dispatch','flow-resolve','flow-handler',
    'flow-contributor','flow-text','flow-text-append','flow-source-map',
    'flow-block','flow-children'
  ) THEN 'flow_assembly'
  WHEN name IN ('prepare-signature','prepare-batch-validation')
    THEN 'preparation_checks'
  WHEN name='host-backend-submit' THEN 'backend_submit'
  WHEN name='text-analysis-shaping' THEN 'analysis_shaping'
  WHEN name IN (
    'ir-record','ir-validate','submission-retention-resources','submission-build'
  ) THEN 'ir_submission'
  WHEN name GLOB 'text-*' THEN 'other_text_engine'
  ELSE 'other'
END AS group_name, sum(self_ns)/1e6 AS self_ms
FROM text_perf_self WHERE dur>0
GROUP BY group_name ORDER BY self_ms DESC;
```

各分类自身耗时求和为 7513.176672 ms，与根区间之和相等，未舍入值差为零。
单个操作的归因使用相同 track 上完全包含的区间，
并与 `host-operation` 内的业务记录关联；不会把跨操作的时间归到当前操作。
