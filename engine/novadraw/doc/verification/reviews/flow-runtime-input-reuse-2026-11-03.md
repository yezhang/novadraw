# FR-01：同一只读作用域的准备输入复用

类型：`implementation-verification`

日期：2026-11-03。能力归属：Core（`novadraw`）preparation 与 graph。
Editor、backend、platform 无新增能力。
`api_semantics`：`text.flow`、`layout.manager`、`notification.layout_update`。
状态：内部复用实现、定向行为与匹配 release 采样完成；整体延迟门禁未通过。

用户已授权根据[重复工作分析](flow-runtime-repeat-work-analysis-2026-11-03.md)实施。
本片只落实 FR-01 内部输入复用；ADR-051 的贡献重读规则继续适用。
当前执行状态见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 改动与边界

原来每个测量投影都会重建子树成员集合和局部 revision 容器，准备批次末尾又
为候选建立一份相同输入。现在 `PreparationInputs` 持有同一份不可变
FigureTree／resolved styles 借用，按 root 保存输入，在这个借用内重复使用。
它不跨布局效果发布、下一次布局器调用或下一轮稳定化。

| 状态 | 复用范围 | 保留的检查 |
|---|---|---|
| 子树成员集合 | 相同 root；不同 root 各自建立 | 每次查询仍检查 Figure 身份与子树权限 |
| revision 静态基础 | 相同 root，source／derived 各一份 | 每次重读全部外部 preparation provider revision |
| 精确 source prefix | 相同 root，静态内容和 provider revision 都相同 | 每次重读 provider；变化时复制新 prefix，保留旧值 |
| FontDescriptor 留存 | 同一不可变 tree／styles 借用 | 先检查当前 root 权限及共享预算，再按完整 descriptor 查找 |
| 测量结果与 observations | 维持原有测量调用作用域 | 独立准备 owner 不共享结果、约束、重入路径或观察日志 |

准备批次在 callback 与末尾复核之间共享输入，候选验证和依赖 generation
重验继续执行。发布任何布局效果前释放输入作用域。
布局器调用的顶层 scene 继续独立建立 styles，测量子视图继承该调用的输入；
布局器返回后应用效果，因此不将缓存扩大到多个布局器调用。

provider revision 是动态字段。source prefix 首次捕获完整精确输入，
后续查询只重读 provider 列表；完全相同时返回已有不可变 Arc。
有变化时先按现有 collection-entry 预算计费，再复制并更新 provider 字段。
缓存查询也检查预算，耗尽状态不会因命中而被绕过。
根输入与 provider 索引在生成前计费；失败的根构造不会安装部分缓存。
多个嵌套 root 会留存重叠成员集合，峰值容量取决于这一借用内访问的 root，
而非单个最大子树。全部条目仍进入共享预算，借用结束时一起释放；
预算计数不能替代真实峰值内存测量。

`parsed_font` 留存的是字体描述符，不是字体解析、shaping 或 glyph 资产。
本次不缓存 contributor、handler、border callback 的结果，不改变 Flow 构建次数，
也不修改 Runtime 稳定化调度或派生回滚机制。

## 行为与工作量验证

scene 私有测试通过 8 项，其中新增 3 项，覆盖相同 root 的成员集合及 revision
复用、测量子树权限、独立 owner 状态隔离、动态 provider、旧 prefix 留存、
输入构造中途预算耗尽和新作用域重试。

两个节点、一个 provider 的用例中，第二个同 root scene 查询 source prefix 与
revision 合计增加 4 个 work 单位：root 查找、prefix 查找和两次 provider 重读；
新增 allocation 为 0，source prefix 的 Arc 身份保持相同。
重复测量子视图的 revision 和字体查询也不再新增输入 allocation。
这些是预算计数与对象身份断言，不是整个 Large 的 allocator 采样。

六个消费者测试目标通过 60 项：

| 目标 | 项数 | 主要覆盖 |
|---|---:|---|
| `p2_scene_preparation_contract` | 8 | 批次原子发布、活动源修改、viewport／route 回滚、样式失效 |
| `p2_preparation_measurement_contract` | 18 | 不同约束、Preferred／Minimum／Both、inline、错误主体、预算失败重试 |
| `long_text_layout_contract` | 7 | 宽度／字体失效、外部引擎、旧布局与退休来源留存 |
| `long_text_paint_contract` | 4 | 墨迹覆盖、未知轮廓回退及绘制坐标 |
| `p2_t03_external_flow_engine` | 1 | 独立文字引擎与交互几何 |
| `p2_t03_structured_flow_contract` | 22 | 外部贡献、inline、普通／片段边框、来源移动／移除、字体资源 |

新增跨 owner 用例让第二个 callback 修改第一个候选的外部 revision，
不依赖 owner 遍历顺序。整批被拒绝，两个旧 presentation 保持原值；
下一次新作用域重试成功。它防止同批输入复用意外跳过末尾冻结检查。

Core `--lib` check、Core `--lib` 与修改的消费者目标 Clippy（`-D warnings`）
已通过。既有 Harfrust qualification 警告保留，不属于本片。
本片未关闭 LT 里程碑，也未执行 workspace 最终提交门禁。

## 匹配性能验证

先从当前源码构建并冻结 baseline，再修改 Core，按相同参数构建 optimized。
两份二进制的 patch、未跟踪源码归档、Cargo.lock、字体与 manifest 保存在
`target/verification/text-fr01-2026-11-03/`，不纳入 Git。
baseline 来源为提交 `ef7d46c1` 加分析文档工作区修改，生产源码尚无 FR-01。
不是 2026-11-02 旧冻结二进制与当前版本的混合对照。

构建使用 Rust 1.96.0、默认 release、`--locked`、
`CARGO_TARGET_DIR=/tmp/novadraw-text-stage-target`、`CARGO_INCREMENTAL=0`、
`CARGO_BUILD_JOBS=1`。C 编译器与链接器使用 CommandLineTools clang，
SDK 为 MacOSX15.0。采样期间不运行本 Agent 发起的构建或测试。
冻结脚本的 `rustc`／`cargo` 字段读取默认 shell 工具链（1.99.1），不是构建命令
显式选择的 1.96.0。原 manifest 保留，实际编译器、完整构建环境和二进制哈希
另记在本地 `build-receipt.json`，避免把采样器元数据误当编译器证据。

Apple M1 Pro 上交替运行 5 个会话，每个版本每会话有 100 个 fresh Runtime 样本、
10 个预热样本和 3 个独立进程样本。fresh Runtime 合计每版本 500 个，
独立进程合计 15 个；统计排除预热及预检，失败记录保留，失败数为 0。
独立进程不代表受控 OS 冷缓存。Runtime timing 继续开启，未单独测量开关成本。
取最近秩分位数，外部后台负载未受控。

下表为 fresh Runtime 的 ms，单元格按 baseline → optimized 排列。
追加更新指输入提交后的稳定化，追加端到端还包括 dispatch：

| 操作 | P50 | P95 | P99 |
|---|---:|---:|---:|
| 首次稳定化 | 37.182 → 35.728 | 40.236 → 40.199 | 44.472 → 55.144 |
| 追加更新 | 16.001 → 15.306 | 17.450 → 16.865 | 18.849 → 20.583 |
| 追加端到端 | 16.066 → 15.366 | 17.508 → 16.935 | 18.892 → 20.676 |
| 稳定查询 | 2.140 → 2.224 | 2.609 → 2.713 | 3.286 → 3.579 |
| 滚动更新 | 6.169 → 6.302 | 6.980 → 7.250 | 8.129 → 8.943 |
| resize 更新 | 34.007 → 33.141 | 37.236 → 36.439 | 42.928 → 47.380 |
| CPU IR 录制 | 3.944 → 4.009 | 4.536 → 4.611 | 4.951 → 5.871 |

准备批次验证的累计 exclusive P95 下降，单位同为 ms：

| 操作 | 批次验证 | 贡献组装 | 布局阶段自身 |
|---|---:|---:|---:|
| 首次 | 3.042 → 1.721 | 10.305 → 10.708 | 7.426 → 7.623 |
| 追加 | 2.102 → 1.138 | 4.046 → 4.086 | 1.761 → 1.755 |
| resize | 2.458 → 1.537 | 10.287 → 10.382 | 7.378 → 7.408 |

resize 批次验证 P95 减少约 37.5%，端到端 P95 只减少约 2.1%。
首次和追加中位数下降，但 P99 上升；稳定查询、滚动及录制没有整体改善。
优化版会话 3、4 的尾部波动较大，报告保留这些样本，不把差异全部归因于缓存，
也不把它们删掉后宣称整体提速。当前证据支持局部输入复用有效，
尚不支持稳定尾延迟改善。独立进程 15 个样本只作补充，原值保留在收据中。

两个版本另外各做 3 次完整请求追踪，每版本 3,960 个成功请求，无 dropped。
各次均保持以下分母：

| 操作 | Flow 构建 | 请求／命中 | measure／prepare 请求 |
|---|---:|---:|---:|
| 首次 | 13 | 520／514 | 400／120 |
| 追加 | 5 | 200／198 | 120／80 |
| 滚动 | 2 | 80／80 | 80／0 |
| resize | 13 | 520／514 | 440／80 |

初始布局仍有 40 个段落、51,232 个文本字节、561 行、1,022 个 run 和
4,168 个 fragment；追加后原生 glyph 仍为 51,233，IR 命令为 7,889。
请求追踪只验证调用次数，不作为计时样本。
本片没有省去 13 次贡献组装，整体 16 ms 门禁继续未通过。

本地证据入口均在 `target/verification/text-fr01-2026-11-03/`：

- `build-receipt.json`：实际构建参数、两份二进制与原 manifest 哈希、验证日志哈希。
- `comparison/{baseline,optimized}/samples.ndjson`：交替采样原记录及失败记录。
- `comparison-receipt.json`：分位数、分会话统计、阶段成本、请求分母与输入哈希。
- `{baseline,optimized}-requests.ndjson`：独立结构追踪。
- `collect.py`：本地收据及统计生成器；两个冻结 source patch 不包含这个忽略产物。

计数证明复用边界和调用分母，阶段计时说明局部成本下降；二者不证明整个 Large
的静态遍历次数或真实分配字节。FR-01 首片验证退出，整体性能工作继续。

## 后续范围

FR-02 的贡献复用声明与不可变组装资产仍需具体合同和评审；
FR-04 的活动捕获与调度归因仍需独立证据。
本片用内部缓存计数和原有 Flow 请求诊断核验输入复用，没有补齐所有布局器、
selection 和再入队来源的结构化观测，也没有测量拒绝路径的延迟分位数。
这些缺口不被计为已完成，不能据本片合并相同 hash 的测量回调。

## 交付复核

成稿先核对 owner、状态生命周期、动态字段和证据范围，再按好好说话检查中文。
最后回查测试数字、性能分母、授权范围与未完成项。
这是作者自审，不是独立 reviewer 验收。
