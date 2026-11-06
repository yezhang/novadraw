# RichTextFigure 混合字号、emoji 与长文压力验证

类型：`verification`

能力归属：`rich-text-figure-app` 拥有测试文档、显式字体注册、准备预算和 CPU 回放；
`novadraw-richtext` 提供既有完整快照复用、排版与绘制。Editor／GEF 不在本次范围。

日期：2026-11-05。状态：四档数据、Runtime 输入检查、三轮 CPU 回放和 Perfetto 分析完成。
混合档的字号与彩色 emoji 输出通过目视核对；25 万字符长文的 Native 截图为黑帧，
长文绘制验收失败。不能把 CPU 回放通过记为 Native 性能或绘制通过。

## 数据与测量口径

原来的 27 段保留为 `basic`。新增 `mixed` 展示 14／18／28／36／40／64 px，
其中一个段落同时包含 18／28／40／64 px，另一个段落使用 36 px 彩色 emoji，
覆盖肤色、职业／家庭 ZWJ、旗帜与键帽。`large` 在展示区之后追加 128 个长段落
与一个更长的段落，作为默认打开的数据；`giant` 则追加单个超长段落。
所有语料确定性生成，使用同一画布和相同类图。

emoji 字体由 app 显式加载 macOS 自带的 `Apple Color Emoji.ttc`，
没有将系统字体拷贝到仓库，也不依赖引擎自动发现系统字体。
当前压力档需要该字体路径；其他平台先使用 `basic`。

`large`／`giant` 的 preparation budget 为 work 80,000,000、allocation 32,000,000，
`basic`／`mixed` 保持 Core 默认的 10,000,000／2,000,000。
这是样例的工作准入政策；allocation 是累计计费单位，不能据此推导实际堆内存。
大文档在放宽准入后完成，不代表它们通过 Core 默认预算。

每档回放 240 个操作，覆盖连续变宽、仅高度变化、滑块拖拽。
计时包含输入／尺寸变更、稳定化及 CPU 绘制命令记录；不执行 GPU 提交，
不包含 WindowServer 或 GPU 完成等待。拖拽必须实际推进正文，否则回放报错。
metadata 记录字符数（Unicode scalar count）、字节数、段落数、最长段落、
样式跨度数、预算，以及创建场景至首次稳定化的耗时。
首次打开计时包含字体读取和注册，不含首次 GPU 绘制。

## 数据规模与 CPU 结果

Apple M1 Pro／arm64，Rust／Cargo 1.99.1，release，Parley 0.12.0、Vello 0.10.0。
冻结执行文件 `/tmp/novadraw-richtext-stress`，来自当前工作树的压力样例与快照缓存实现，
包含同期 Core 变更；不是先前优化报告中旧 binary 的匹配对照。

| 数据档 | 字符数 | UTF-8 字节数 | 段落数 | 最长段落字符数 | 初次创建／稳定化中位时间 |
|---|---:|---:|---:|---:|---:|
| basic | 3,611 | 6,768 | 27 | 146 | 14.28 ms |
| mixed | 2,955 | 5,537 | 29 | 117 | 84.83 ms |
| large | 249,763 | 464,849 | 134 | 9,360 | 412.76 ms |
| giant | 46,947 | 87,505 | 6 | 46,800 | 137.28 ms |

每档三轮共 720 个操作，各种操作 240 个样本；第二轮逆序执行数据档。
以下为关闭 tracing 的 CPU 时间，P50／P95 分别取排序样本的
`floor((N−1)×0.50)`／`floor((N−1)×0.95)` 项。
初轮采样与本次构建重叠，因此完成检查并停止自己重复排队的构建后，
再采三轮 `final` 数据作为本表。补采前后仍检测到其他本地 rustc 进程，
不能声称环境完全无干扰。本表用于判断成本数量级和暴露瓶颈，
不能作为隔离环境中的发布性能基线。

| 数据档 | 连续变宽 P50／P95 | 仅改高度 P50／P95 | 滑块拖拽 P50／P95 |
|---|---:|---:|---:|
| basic | 3.350／3.649 ms | 0.565／0.594 ms | 0.435／0.471 ms |
| mixed | 3.352／3.696 ms | 0.569／0.602 ms | 0.434／0.465 ms |
| large | 189.463／198.530 ms | 6.505／7.643 ms | 6.470／7.762 ms |
| giant | 35.364／36.629 ms | 1.515／1.721 ms | 1.357／1.561 ms |

初轮三轮汇总的 large 变宽 P50／P95 为 212.197／392.598 ms，
拖拽为 7.082／13.280 ms；补采缩小了高分位，但没有改变瓶颈判断。

25 万字符下，最终变宽稳定化本身 P50 为 182.979 ms；仅高度变化和拖拽的稳定化
P50 分别为 0.502／0.409 ms，其余大部分时间来自命令记录。
此时高度变化与拖拽每帧仍记录 14,046 条命令，变宽为 14,439–15,113 条。
原样例为 688 条，混合档为 706–707 条。
组件正文没有段落子 Figure，但绘制仍遍历全文并提交各段的 glyph runs，
因此固定树规模没有消除随文档规模增长的绘制成本。

混合档字符数少于 basic，超长单段的总字符数也少于 large；
这些档位验证不同负载形态，不能从档间差值单独推导字体、emoji 或单段结构的成本。

## Perfetto 阶段归因

每档另采一轮 240 操作 trace，计时不与上表混合。Perfetto v59.1 读取成功，
`stats` 中 error severity 的非零记录为空。解析器的 arm64 syscall 提示不影响
本次 TrackEvent slice 查询；本次没有采集系统调用或调度轨迹。

| 数据档 | 全文快照次数／累计时间 | 段落准备次数 | analysis／shaping 累计时间 | CPU 命令记录累计时间 |
|---|---:|---:|---:|---:|
| basic | 173／261.47 ms | 4,671 | 173.19 ms | 59.95 ms |
| mixed | 173／264.81 ms | 5,017 | 172.72 ms | 59.52 ms |
| large | 173／17,424.10 ms | 23,182 | 11,992.84 ms | 1,869.47 ms |
| giant | 173／3,410.82 ms | 1,038 | 2,366.03 ms | 364.74 ms |

各阶段有嵌套，累计时间不能相加。所有档位的 height／drag 操作内都没有全文快照构造，
width 操作内仍有 160 次，即每次变宽两次构造。当前缓存能避免相同宽度的重复计算，
但仍未复用跨宽度的 analysis／shaping，也没有限制正文绘制输出到可见区域。
这两处分别解释长文变宽与滚动成本的增长；黑帧根因尚未得到运行时证据确认。

```sql
SELECT name, COUNT(*) AS calls, ROUND(SUM(dur)/1e6,2) AS total_ms
FROM slice WHERE dur >= 0
  AND (name GLOB 'richtext.*'
       OR name IN ('text-analysis-shaping','stabilization'))
GROUP BY name;

SELECT p.name, COUNT(c.id) AS snapshots,
       ROUND(SUM(c.dur)/1e6,2) AS snapshot_ms
FROM slice p LEFT JOIN slice c
  ON c.ts >= p.ts AND c.ts+c.dur <= p.ts+p.dur
 AND c.name='richtext.snapshot'
WHERE p.name GLOB 'replay.*' GROUP BY p.name;

SELECT name,value FROM stats WHERE severity='error' AND value>0;
```

回放在同一线程顺序执行，以上包含关系查询适用于这组不重叠的父操作。
本环境 named daemon 没有跨 Shell 保持，因此采用直接读取文件、一次执行全部 SQL。

## 正确性与 Native 输出

四档 `--headless` 均通过真实 Runtime 的选择、双击模式、滚轮、滑块 capture、
滚动隔离、typed 编辑、NoChange、缩窄重排和错误文档身份拒绝检查。
large 初始内容高 153,733 logical units，giant 高 28,345；视口高 680。
组件 6 项单元测试及 profiling app 的定向 release Clippy 通过。

系统 emoji 字体有两个 face，其 format-12 cmap 覆盖本次测试的基础 emoji 字符。
混合档 Vello 截图中可见不同字号；笑脸、火箭、庆祝、心形、带肤色拇指、
女程序员、家庭、旗帜和键帽都有彩色输出。此项同时核对了复合序列的实际形态，
没有仅凭字体 cmap 或字符串存在就宣称绘制成功。

- 混合档：`target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793886689.png`
- 长文档：`target/visual-verification/screenshots/rich-text-figure-app_Document_and_diagram_1793886716.png`

第二张图为整帧黑色。进程退出码为 0，Native 日志未报错，
说明截图宿主的 Presented 回执和无窗口检查都没有捕获这一图像失败。
当前保留该失败证据，不缩短测试文档、不关闭 emoji，也不将 CPU 数据改记为 GPU 数据。
large Native 绘制与真实连续 resize／拖拽的验收仍未通过；
giant 尚无 Native 图像验证。下一步需要确认黑帧的 backend／GPU 运行时原因，
再对可见 glyph 输出与跨宽度排版复用分别建立修改合同和匹配成本对照。

## 本地产物

最终普通样本：`/tmp/novadraw-richtext-stress-final-{basic,mixed,large,giant}-{1,2,3}/`
下的 `samples.json` 与 `metadata.json`。
初轮样本仍保留在不含 `final-` 的同名目录，不与最终统计合并。
trace：`/tmp/novadraw-richtext-stress-{basic,mixed,large,giant}-trace/`
下的 `resize.pftrace` 与 `stages.csv`。
Native 日志：`/tmp/novadraw-richtext-stress-{mixed,large}-native.log`。
这些中间产物没有纳入 Git。

## 复跑

```sh
cargo run --release -p rich-text-figure-app
cargo run --release -p rich-text-figure-app -- --text-load mixed
cargo run --release -p rich-text-figure-app -- --text-load giant
cargo run --release -p rich-text-figure-app -- --headless --text-load large
cargo run --release -p rich-text-figure-app -- --benchmark-resize --text-load large --output /tmp/richtext-stress-large
cargo run --release -p rich-text-figure-app --features profiling -- --benchmark-resize --text-load large --trace --output /tmp/richtext-stress-large-trace
```

回放原样例时须显式传 `--text-load basic`；
此前[缩放优化报告](rich-text-figure-resize-optimization-2026-11-05.md)的原始数据不变。
不同负载的耗时差异不能直接当作优化前后收益。
