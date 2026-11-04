# TextFlowLarge Perfetto trace 诊断

类型：`verification`

能力归属：Core 的准备、测量和 Flow 组装；Native 研究宿主的输入与帧编排；
Vello backend 的提交。Editor 无新增分析项。

日期：2026-11-04。范围：进程 92502 的四份 Native 手动操作 trace。
本文提供诊断和后续验证建议，不修改实现，也不宣布性能门禁通过。

## 诊断结论

这组数据确认了两类 Core 成本：内容稳定时仍重复做准备签名和批量校验；
部分交互和宽度变化反复走 Flow 测量组装，即使文字布局已经命中缓存。
第二份 trace 中，14 次 `Stable` 操作各有两次测量组装，中位耗时 101.68 ms；
61 次没有组装的 `Stable` 操作中位耗时 21.25 ms。两组来自手动操作，
不是受控优化前后对照，不能把差值当作已证明的优化收益。

放大回原宽度的操作没有调用文字引擎，仍耗时 568.02 ms，
其中列明的组装阶段自身耗时为 399.90 ms。下一轮 Core 排查应优先检查
测量组装复用与准备校验成本。只优化 shaping 无法覆盖这条已观测到的慢路径。

研究宿主每个研究帧都请求全量重绘；第二份 trace 中 15 个帧尝试总耗时
2070.26 ms，其中 backend `submit` 区间 1460.68 ms，约占 70.6%。
这是 backend 调用的墙钟区间，不能由此认定 GPU 是瓶颈。

## 数据范围与可信度

文件位于 `target/verification/textflow-large-app/`：

| 标识 | 文件名 | 操作数 | 帧尝试数 | 实际 Flow build 次数 |
|---|---|---:|---:|---:|
| T1 | `textflow-1793772130712470000-92502-0.pftrace` | 20 | 12 | 14 |
| T2 | `textflow-1793772166039786000-92502-0.pftrace` | 80 | 15 | 74 |
| T3 | `textflow-1793772195750456000-92502-0.pftrace` | 19 | 3 | 0 |
| T4 | `textflow-1793772213967315000-92502-0.pftrace` | 2 | 1 | 13 |

四份文件包含连续的 operation 1–121，覆盖同一进程中的三次 Runtime 重建。
合计 121 个操作记录均报告成功；31 个帧尝试中，26 个返回 `Presented`，
5 个返回 `Skipped`，全部位于 T1。`Presented` 是 backend 确认，不是屏幕呈现证据。

配置记录为 `profile=debug`、Rust 1.99.1、`headless=false`、
`requests=false`、`gpu_wait=false`、`requested_damage=full`。
T1 的 adapter 记录为 Apple M1 Pro / Metal / IntegratedGpu。
构建记录的 Git HEAD 为 `72077f53`，且 `dirty=true`；它不是完整源码内容散列。
源码定位采用当前工作区中对应埋点，只用于解释区间覆盖的行为。

官方 Trace Processor v59.1 已解析四份文件：

- 未报告 `severity=error` 或 `severity=data_loss` 的非零统计。
- 所有 slice 均已结束，直接子区间扣除后的自身耗时没有负值。
- T2、T3、T4 分别有 `traced_chunks_discarded=1/2/3`，严重性为 `info`。
  该值不能直接按丢失事件次数解释，也不能取代正式的数据丢失检查。
- 四份文件均没有 `novadraw.request` category；缺少完整请求和来源版本证据。
- `sched` 和 `cpu_profile_stack_sample` 均为空，没有 GPU 阶段 slice。
  因此不能判断 CPU 饱和、线程抢占、锁等待、GPU 执行时间或 compositor 延迟。

以下毫秒值都是带 SDK 埋点的 debug 运行所记录的墙钟区间。
它们适合定位当前路径，不能作为 release 性能验收或 P95/P99 结论。
同一操作内的测量请求是否满足复用资格，仍需检查完整输入和依赖版本；
正文和宽度相同本身不足以证明可以复用。

## 内容稳定时的准备成本

T3 的 19 个操作全部是 `Stable`，没有 Flow build，也没有文字引擎调用：

| 项目 | 实际结果 |
|---|---:|
| `Stable` 操作中位耗时 | 21.90 ms |
| `Stable` 操作范围 | 20.03–32.33 ms |
| `stabilization` 次数 / 累计耗时 | 22 / 503.61 ms |
| `prepare-signature` 次数 / 累计自身耗时 | 44 / 249.50 ms |
| `prepare-batch-validation` 次数 / 累计自身耗时 | 44 / 180.01 ms |

这两个互不嵌套的校验阶段合计 429.51 ms，占本段累计稳定化区间约 85.3%。
每次稳定化平均出现两次准备刷新。这说明缓存命中后仍有较高的校验底成本。
T1、T2、T4 的无组装 `Stable` 组也有约 20–23 ms 的中位成本，现象跨文件复现。

源码定位：

- [准备刷新与签名](../../../crates/novadraw/src/graph/presentation.rs#L139-L211)：
  构造 style/map 输入，计算场景 revision、source prefix 和依赖资格。
- [批量校验](../../../crates/novadraw/src/graph/presentation.rs#L213-L278)：
  埋点同时覆盖依赖 owner 保留和既有 prepared route 检查。
  即使候选为空，也不能把整个区间理解为“校验了新候选”。

建议进一步拆分签名计算、source prefix、依赖保留与 route 校验，
核实哪些检查必须执行，哪些可在受检版本未变化时复用结果。
这里建议减少重复工作，不能通过删除校验或放宽一致性条件实现。

## Flow 测量组装是慢交互的主要组成

T2 中代表性操作的区间如下。组装自身耗时为文末列明阶段集合的直接子区间扣除值，
不会把 `flow-visit`、`flow-dispatch`、`flow-handler` 的 inclusive 耗时重复相加。

| operation | 操作 | 总耗时 ms | Flow build | 文字引擎调用 | 组装自身 ms |
|---:|---|---:|---:|---:|---:|
| 21 | Reset | 781.59 | 13 | 6 | 399.38 |
| 23 | Append | 326.79 | 5 | 2 | 164.34 |
| 43 | Stable | 103.02 | 2 | 0 | 61.39 |
| 93 | Scroll(100) | 111.40 | 2 | 0 | 65.56 |
| 95 | Resize(400) | 712.41 | 13 | 6 | 395.61 |
| 97 | Resize(600) | 568.02 | 13 | 0 | 399.90 |

每次 Flow build 有 582 次 `flow-visit`；13 次 build 对应 7566 次访问。
在放大回 600 的操作中，11 次 build 的父区间是 `layout`，
另外两次是 `prepare-callback`。无文字引擎调用仍不能避免组装和输出构造。
三次 Reset 均有 13 次 build、6 次文字引擎调用，耗时分别为
817.96、781.59、749.02 ms；它们是同一进程中的重建，不是三个冷进程启动样本。

T2 的 75 个 `Stable` 操作可进一步分为：

| 分组 | 操作数 | 总耗时中位值 ms | 组装自身耗时中位值 ms |
|---|---:|---:|---:|
| 没有 Flow build | 61 | 21.25 | 0 |
| 每次两个 Flow build | 14 | 101.68 | 60.67 |

14 次较慢的 Stable 共贡献 28 次 Flow build；operation 43 的两次 build
都发生在 `layout` 下，属于测量路径。其快照仍报告 51233 bytes、586 的有效宽度，
未新增文字引擎调用。这支持检查“为什么交互又触发布局测量”和
“组装输入是否具备复用资格”，尚不足以直接认定这些调用都可以安全删除。
Native 宿主也会把普通输入适配后编排成 `Stable`，
因此操作标签不能单独证明用户当时没有移动指针、改变 selection 或触发其他状态。

## 快照计数存在观测局限

T2 的 operation 43、47 等 Stable 快照仍显示 `builds=18`、
`styled_layout_calls=8`，但对应区间实际各有两个 Flow build。
Scroll operation 93 的快照也仍显示 `builds=18`，其测量区间同样有两个 build。

原因与当前所有权一致：`FlowSnapshot.statistics` 是发布快照时复制的统计，
测量过程可以推进 service 工作计数，而旧不可变快照不会同步更新。
定位见 [工作计数递增](../../../crates/novadraw/src/figure/flow/snapshot.rs#L615)
及[发布前复制计数](../../../crates/novadraw/src/figure/flow/snapshot.rs#L1272)。
因此，两个快照计数相同不能证明中间没有布局测量。

此次诊断以实际 SDK slice 计数为分母。后续报告应把这组值标明为
“已发布快照携带的统计”，不能重新引入自研采集器来补偿这一局限。

## Native 帧与后端提交

T2 的 15 帧全部返回 `Presented`。总帧区间为 2070.26 ms，
`prepare-submission` 为 603.31 ms，`host-backend-submit` 为 1460.68 ms。
平均每帧约 138.02 ms，平均 submit 区间约 97.38 ms。
T3 没有重新排版，三帧仍分别耗时 134.84、144.46、131.73 ms。

大部分帧记录约 7609 条命令、51232/51233 个 glyph。
[研究宿主](../../../crates/novadraw/examples/native/textflow-large-app/src/research.rs#L223-L248)
在每次帧尝试前显式调用 `request_full_redraw()`。
这是研究场景的完整帧口径，不能据此判断默认增量 damage 或实际可见 glyph 的成本。

[Vello submit](../../../crates/novadraw-backend-vello/src/lib.rs#L1576-L1693)
涵盖输入与 capability 校验、资源同步、Scene 编码、surface 获取以及
`render_to_texture` 等步骤。现有区间无法将 CPU 编码、驱动等待和 GPU 工作拆开。
下一轮应在 backend 内增加对应 SDK 阶段，并用 GPU timestamp 或平台工具
补充 GPU 与呈现证据；仅观察 submit 区间不能得出“GPU-bound”结论。

T1 的五次 `Skipped` 帧也已产生完整命令和 IR。应单列跳过原因与尝试次数，
再判断这些重试是否必要；不能把它们混入成功呈现帧的延迟分布。

## 分析工具与 MCP

| 选择 | 能力与适用性 | 本轮状态 |
|---|---|---|
| 官方 Trace Processor CLI | PerfettoSQL、标准表、统计、持久 Unix/HTTP 会话；适合当前自定义 Rust stage | 已用 v59.1 实际分析 |
| Perfetto UI | 时间线、层级区间、SQL；适合人工检查代表性慢操作 | 可打开原文件 |
| 官方 Perfetto AI skill | 指导 Agent 加载 trace、发现表、运行 SQL 和解释结果；通用分析适合当前事件 | 已核实官方文档，未安装 |
| `perfetto-mcp-rs` | 社区 MCP，提供 `load_trace`、表结构查询、`execute_sql`；包装官方 Trace Processor | 已核实包文档，未安装、未验证本机连接 |

优先选官方 AI skill 配合现有 CLI。当前官方 CLI 已支持 Agent 输出和持久会话，
无需为了让 Agent 分析 trace 先搭建 MCP。
官方 `server stdio` 使用长度前缀 RPC 协议，不能直接当作 MCP server 配置。[cite:1][cite:3]

官方提供通用安装器，文档支持安装到指定 skills 目录：

```sh
curl -fsSL https://get.perfetto.dev/agents-install |
  python3 - --target .trae/skills
```

这是安装参考命令，本轮没有执行或改变 Agent 配置。[cite:3]
若希望通过 MCP 客户端使用 trace 查询，社区包文档提供 Cargo 安装与 JSON 配置，
并支持 `PERFETTO_TP_PATH` 指向已有二进制，避免再次下载。
其 Chrome 专用指标不适合直接套在这组 Native Rust 自定义事件上，
应使用通用 SQL 工具。第三方 MCP 的运行与客户端兼容性仍需单独验收。[cite:5]

## 复核方法与证据定位

本轮使用官方 Trace Processor 的持久 Unix socket 会话，多条 SQL 复用同一次加载；
分析后会话均已关闭。临时查询辅助文件放在 `/tmp`，没有新增生产采集工具。
原 trace 未修改。完整查询、121 个操作与 31 个帧的归因结果保存于
`target/verification/textflow-large-app/perfetto-diagnosis-2026-11-04-data.json`，
该数据文件位于忽略的验证目录，不纳入版本控制；2026-11-04 后续检查时本地文件
已不存在，因此保留产物路径说明，不作为可点击的现存证据。
以下 SQL 可在 CLI 或 Perfetto UI 中复核关键结论：

```sql
-- 正式采集错误和丢失检查；再单独检查 info 统计。
SELECT name,severity,value
FROM stats
WHERE value>0 AND severity IN ('error','data_loss');

-- 实际阶段自身耗时。前提：同步单线程嵌套区间、已结束、直接子区间不重叠。
WITH children AS (
  SELECT parent_id,sum(dur) AS ns
  FROM slice WHERE dur>=0 GROUP BY parent_id
)
SELECT s.name,count(*) AS calls,
       sum(s.dur)/1e6 AS inclusive_ms,
       sum(s.dur-coalesce(c.ns,0))/1e6 AS self_ms
FROM slice s LEFT JOIN children c ON c.parent_id=s.id
WHERE s.dur>0
GROUP BY s.name ORDER BY self_ms DESC;

-- 业务事件及快照。完整 JSON 经 SQL 提取，不能只看操作名称。
SELECT json_extract(EXTRACT_ARG(arg_set_id,'debug.record'),'$.kind') AS kind,
       count(*) AS records
FROM slice WHERE category='novadraw.host' GROUP BY kind;
```

操作归因先将 `host-operation` 与其直接子 `host-record` 关联，
再按相同 track、完全包含于操作区间的阶段计数和求和。
所有业务操作记录均找到匹配区间。组装自身集合为 `flow-visit`、`flow-dispatch`、
`flow-resolve`、`flow-handler`、`flow-contributor`、`flow-text`、
`flow-source-map`、`flow-block`、`flow-children`。
独立组别的中位值只分别报告，不将中位值相加推导总耗时。

| 文件 | SHA-256 |
|---|---|
| T1 | `8e6cf125487c594a5cdbcc3ca16b0d73d2efad6fdcaa89e06beb00b3b42a4a17` |
| T2 | `9ee2bc5c8e766c8bf950334e9e742a64539314bf3e1d803b6daba14a3d0d0ead` |
| T3 | `ec47d57c3d1c50825ccd024a73e2a7c2a5fc6d9119addaddfc92fd2f89e5dfcf` |
| T4 | `052639ed5d1da41e41ed11135074b2c1815f4e0942d6a9a6cb7e69844d11f0fb` |

## 外部工具来源

[cite:1] [Trace Processor CLI 官方参考](https://perfetto.dev/docs/reference/trace-processor-cli)。

[cite:3] [官方 Perfetto AI skill](https://perfetto.dev/docs/getting-started/using-ai)。

[cite:5] [社区 perfetto-mcp-rs 0.13.3 的包与功能文档](https://crates.io/crates/perfetto-mcp-rs/0.13.3)。
