# RichTextFigure 窗口缩放优化验证

类型：`verification`

能力归属：`novadraw-richtext` 拥有完整排版快照缓存；
Core 只公开已有文字引擎缓存身份查询；`rich-text-figure-app` 拥有成本回放和 Perfetto capture。
本轮没有修改断行算法、Core 渲染主循环或 Editor／GEF。

日期：2026-11-05。状态：重复测量／准备优化已实现，定向正确性和 CPU 成本对照通过。
Native 初始场景图像一致；真实连续窗口缩放及 GPU 完成成本未验收。

## 问题与改动

用户反馈样例窗口 resize 性能差。原实现的 `measure_scene` 和 `prepare_scene`
分别逐段物化输入、调用文字服务并建立全文快照。同一宽度在一次稳定化中被反复测量，
仅高度变化和滑块拖拽也触发重复排版。

新增组件私有 `SnapshotCache`，最多保存两个完整成功快照。key 包含文档快照版本、
文字引擎实例身份、测量 generation、受检有效宽度及布局方向。
测量和正式准备消费同一结果；正文尺寸、高度索引、glyph 与文本位置来自完整快照。
字体 generation、文档、方向或引擎身份变化后重新计算；没有缓存身份的外部引擎
继续完整计算。缓存命中检查共享 preparation 预算，访问计费不豁免；
失败不安装候选，也不覆盖已有成功快照。

Core 新公开 `MeasureContext::cache_identity()`，透传现有
`TextLayoutEngine::cache_identity()`，没有新增默认引擎特权或资源写权限。
合同见[方案](../../design/architecture/rich-text-figure-proposal.md#窗口缩放的首轮优化合同)。
宽度变化仍需全文排版，本轮没有跨宽度 shaping／analysis 复用。

## 关闭 tracing 的成本比较

使用同一工作树和 release 配置，优化前后分别冻结 binary：
`/tmp/novadraw-richtext-before` 与 `/tmp/novadraw-richtext-after`。
两份 binary 均包含相同回放与阶段插桩，普通成本比较不启动 tracing。
工作树还包含先前未提交的通用投影切片，因此这不是仓库 HEAD 的独立性能基线。

每轮回放 240 个操作：5 组 ×（16 次连续变宽、16 次仅改高度、16 次滑块拖拽）。
另运行两轮交替顺序对照，合计每份 binary 720 个操作，各场景 240 个样本。
窗口初始 1120 × 760；宽度每步减少 8，高度每步减少 4，滑块拖拽每步移动 4。
所有数值以 canvas logical units 表示。回放包含类图和 27 段正文。

| 操作 | 优化前 CPU P50 | 优化后 CPU P50 | P50 下降 | 优化前 P95 | 优化后 P95 |
|---|---:|---:|---:|---:|---:|
| 连续变宽 | 17.491 ms | 3.566 ms | 79.61% | 22.828 ms | 5.437 ms |
| 仅改高度 | 10.658 ms | 0.583 ms | 94.53% | 16.958 ms | 0.844 ms |
| 滑块拖拽 | 3.381 ms | 0.455 ms | 86.54% | 4.335 ms | 0.772 ms |

CPU 计时覆盖 mutation、`stabilize_for_query` 和 `prepare_submission` 的命令记录；
没有窗口、WindowServer、backend 编码、真实 GPU 提交或完成等待。
回放宿主仅确认记录完成。前后各个匹配操作的记录命令数量相同，
不能据此将表中结果称为 Native 总帧耗时。
P95 取排序样本的 `floor((N-1)×0.95)` 项。

## Perfetto 归因

另采集每份 binary 的一轮 240 操作 trace，不与关闭 tracing 的计时混合。
Perfetto v59.1 能读入两份 trace，error severity 的非零 stats 为空；
两份均有 80 个 width、80 个 height、80 个 drag 回放 slice。

| 阶段 | 优化前次数／累计时间 | 优化后次数／累计时间 |
|---|---:|---:|
| `richtext.snapshot` | 1678／2476.74 ms | 173／249.81 ms |
| `richtext.paragraph` | 45306／2457.15 ms | 4671／247.77 ms |
| `text-analysis-shaping` | 45326／1596.52 ms | 4691／159.05 ms |
| `richtext.cache-hit` | 0 | 1505 |
| `stabilization` | 514／2744.94 ms | 514／390.18 ms |

以上阶段有嵌套关系，累计时间不能相加。
优化后的 height 和 drag 回放内部没有 `richtext.snapshot`；
width 内仍有 160 次快照构造。全文快照减少 89.69%，与 CPU 对照中的改善一致。

已运行的 SQL：

```sql
SELECT name, COUNT(*) AS calls, ROUND(SUM(dur)/1e6,2) AS total_ms
FROM slice WHERE dur >= 0 GROUP BY name ORDER BY SUM(dur) DESC;

SELECT p.name, COUNT(c.id) AS snapshot_calls,
       ROUND(SUM(c.dur)/1e6,2) AS snapshot_ms
FROM slice p JOIN slice c
  ON c.ts >= p.ts AND c.ts+c.dur <= p.ts+p.dur
WHERE p.name GLOB 'replay.*' AND c.name='richtext.snapshot'
GROUP BY p.name;

SELECT name, value FROM stats WHERE severity='error' AND value>0;
```

回放 slice 在同一线程上顺序执行，互不重叠；第二条查询用于这些固定父操作，
不能直接推广到并行跨线程 trace。
工具可使用 bundled `trace_processor query TRACE "SQL"`。
本环境的 daemon session 未跨 Shell 保留，后续查询使用直接加载方式。

## 正确性与边界

组件 6 项单元测试通过，其中 3 项新回归验证同约束复用、
宽度重排、方向和文档失效、字体 generation、不同引擎身份、
外部引擎 opt-out、预算失败、非法宽度、两条目上限和旧快照有效性。
app `--headless` 的选择、双击模式、滚动隔离、typed 编辑、相同源、
resize 和错误文档身份检查通过。组件及 profiling app 的 Clippy、
Core lib Clippy 和组件 Wasm check 通过。
最终 workspace fmt/check 及文档门禁通过（509 份 Markdown）。

优化前后 Native 初始场景截图逐像素一致，尺寸 2048 × 1388：

- `target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793884795.png`
- `target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793884796.png`

这两张图不是连续 resize 的像素或实际鼠标验收。
首次打开、新宽度的全文排版、全段落绘制输出、backend 和 GPU 成本仍在；
glyph backing 的精确留存容量也仍需单独验收。有限快照容量不代表整体内存预算已完成。

## 本地产物与复跑

关闭 tracing 的样本位于 `/tmp/novadraw-richtext-resize-{before,after}{,-2,-3}/samples.json`；
trace 位于 `/tmp/novadraw-richtext-resize-{before,after}-trace/resize.pftrace`。
构建、测试和 Native 日志位于 `/tmp/novadraw-richtext-resize-*.log`。
这些中间产物没有纳入 Git。

```sh
cargo run --release -p rich-text-figure-app -- --benchmark-resize --output /tmp/richtext-resize
cargo run --release -p rich-text-figure-app --features profiling -- --benchmark-resize --trace --output /tmp/richtext-resize-trace
cargo test -p novadraw-richtext --lib
```
