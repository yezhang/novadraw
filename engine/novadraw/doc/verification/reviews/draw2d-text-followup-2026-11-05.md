# Draw2D 文本优化第二批验证

类型：`verification`

能力归属：Core Figure／Flow／Text。日期：2026-11-05。状态：部分实施、验收未关闭。
用户已授权实施三项，范围见[采用方案](../../design/architecture/draw2d-text-algorithm-adoption.md#第二批采用范围)。
Label 等值修改保护和 Flow 工作区复用已实现，95 项定向测试通过；
默认 Parley 分析／重排分离尚未实现，缺口与恢复顺序见下文。
本批属于 FR-02，不关闭整体交互性能或平台验收。
用户随后要求复核上游 API 与补丁必要性，旧版补丁扩张已暂停。
最新路线评估见[Parley 上游复核](parley-upstream-api-and-patch-necessity-2026-11-05.md)；
下文准入子片保留为已经实施的证据，不代表继续补齐旧版依赖的决定。

`api_semantics`：`builtin.figures`、`text.flow`、`text.interaction`、
`figure.properties`。已核对[语义账本](../../parity/draw2d/api-coverage.md)
中的 Label 缓存／typed mutation 与不可变 Flow／交互合同，本批不增加 Core 公开 API。

## 契约与执行范围

等值 Label setter 保留已有缓存和 revision；不同值与非法输入仍按原合同处理。
Flow 工作区只复用独占临时集合的容量，清空所有 FigureId 和 source offset；
已发布文本、来源、片段和布局保持独立不可变所有权。
工作区命中不删除访问、结构、深度或准备预算检查。

默认 Parley 的分析／重排分离受既有分析准入合同约束。
当前 `RangedBuilder::build` 仍直接进入 `build_into_layout`，
`LayoutContext::ranged_builder` 在返回 builder 前已经解析字体样式；
`shape_item` 调用 Swash parser、FontSelector 和 Harfrust shaping。
这些生产入口还没有完整的会话准入通道，不能用普通完整排版生成模板再补报预算。
第三项需补齐实际支持路径的 B1 shaping、B2 字体选择和 Parley 编排、
B3 Core 全输出计费以及 B4 资产适配，再接入 Flow；未完成前保留未采用状态。

本次核对的生产入口如下。它们是恢复实施的边界，不是测试引擎可替代的步骤。

| 入口 | 尚缺的生产合同 |
|---|---|
| `dependencies/parley-0.7.0/src/context.rs::ranged_builder` | `resolve_style_set`、style builder 初始化和 source cache prune 在返回 builder 前执行，需先接入同一分析会话 |
| `dependencies/parley-0.7.0/src/shape/mod.rs::shape_item` | Swash parser、逐 cluster 字体选择、LRU 冷构造和真实 Harfrust shaping 须分别证明访问／增长成本 |
| `crates/novadraw/src/render/text.rs` 的 `builder.build(text)` | 普通完整布局不能作为受准入的分析资产生成入口；默认引擎尚未实现 `StagedTextLayoutEngine` |

支持路径可以按已证明的算法结构逐步接入，未知路径在执行未知操作前返回成功缺失；
不能要求先支持所有字体和脚本，也不能把测试字体身份作为支持条件。
恢复顺序为支持路径的 shaping 与字体选择准入 → Parley 分析编排 →
Core 全输出计费与资产适配 → Flow 阶段接入 → 匹配 resize 性能验收。
锁定的 Swash 版本为 0.2.10；cluster 存储虽为固定数组，parser 仍有按脚本分支、
token 读取和 Myanmar lookahead，字体映射另有回调及重复映射。
固定存储本身不足以证明整个 parser／字体查询的成本，也不证明必须修改 Swash。
若证明需要修改 Swash、read-fonts 等未授权依赖，按已接受的
[分析准入方案](../../design/architecture/text-analysis-admission-proposal.md#建议保留预算合同增加依赖准入入口)
先提出单独补丁建议；本批没有扩张这些依赖。

### 第三项实施中的字体匹配子片

Fontique 0.7.0 已按原始归档导入 `dependencies/`，
仅本地 Parley 0.7.0 改为精确引用该 path。family 匹配新增
`FamilyInfo::match_index_admitted`，普通入口与准入入口共用原 CSS 算法。
候选从字体属性初始化前准入；超过 16 项时先对 SmallVec 执行可失败精确预留。
每轮扫描分别覆盖候选访问和 min／max 比较上界，width／style retain 覆盖读写；
普通入口保留原 `from_iter → extend → reserve` 的容量策略。

独立消费者使用真实 Inter 字体及公开 metadata override 构造不同 family。
Native／portable 分别核对 4,900 组原版、普通及准入匹配索引。
每个实际准入点逐一拒绝，原始错误／source、停止和后续普通调用均通过。
依赖内部三项匹配测试覆盖空 family、费用溢出及 SmallVec 容量溢出。
生产 SmallVec 1.15.1 已用于消费者复验，Wasm 只构建；Core lib check／Clippy 通过，
保留一条既有 Harfrust qualification 告警。

随后补齐私有 collection 的注册内存字体查询。`query_admitted` 在 shared／system
访问前返回缺失；`set_families_admitted`、`set_attributes_admitted`、
`set_fallbacks_admitted` 与 `matches_admitted` 共用普通查询的匹配及缓存内核。
ASCII 名称归一化、generic 展开、fallback cache 的冷／热访问、候选加载前的
字体属性扫描、Vec 增长和最终回调均在执行前准入。
非 ASCII 字体名和文件来源在未知操作前返回 `Unsupported`，普通入口行为保持。
迭代器、类型转换与候选回调的用户代码仍归调用者计费。

查询消费者用 19 项真实字体 metadata 变体，比较候选顺序、family 内索引、
字体内容、synthesis 和 charmap。140 字节 ASCII 名称、五种 Han locale、
generic 重复项、冷／热匹配、逐点拒绝及同一查询恢复均通过。
Native／portable 各七项普通消费者测试通过；内部共四项准入测试通过，
其中新增 Vec 溢出与存储失败测试保留原始 reservation source。
Wasm 仅构建。九个源码变更文件的补丁精确重放通过，其他归档字节保持。

查询 release 计时每轮执行 100,000 次新 Query，九轮交替对照。
原版与本地普通入口中位数分别为 144.106ms／144.753ms，差值 +0.646ms（+0.45%）。
准入回调只执行 `black_box` 时为 182.022ms，相比原版 +26.31%；
这不包含 Core 实际扣费。样本仍有调度波动，不能据此宣称渲染净收益，
也不能把准入基础设施本身记为性能优化。
原始计时见 `target/verification/text-draw2d-followup-2026-11-05/fontique-query-timing-long.txt`；
早期 5,000 次短采样波动更大，保留在同目录，不作为收益依据。

查询子片仍不是分析资产生成入口。Parley 编排、Core 导出与资产适配继续实施；
第三项没有阶段复用或新的渲染收益结论。
后续采样须冻结只含本步差异的前／后源码，避免与同工作区的管线删减批次混用。
旧独立消费者已随依赖补丁退役；替代边界见[上游迁移](parley-upstream-migration-2026-11-05.md)。

### Parley 分析前段的准入子片

Parley 新增 `AnalysisPolicy`，普通路径使用无拒绝策略；
受准入路径接收操作类别、逻辑访问量和新增元素容量。
本子片已覆盖下列内部入口，尚未提供完整的受准入布局构建入口：

- `RangedStyleBuilder::finish_with`：共享原有范围搜索、分裂、属性应用与合并算法；
  在搜索、插入移动、容量增长及清理前扣费。Copy brush 使用逐字段复制，
  不调用可能被外部类型重写的 `Clone`。
- `LayoutContext::analyze_text_with`：共享 Swash 边界分析；
  逐个字符预留 info，费用包含当前字符、最多一个前瞻字符、
  三层 Unicode 索引表、属性记录和断行 pair table 的访问。
  需要 Bidi resolution 时，受准入入口在调用解析器前返回未支持。
  这不表示组合字符或复杂脚本的后续 shaping 已获支持。
- `ResolveContext::resolve_stack_admitted`：ASCII source／借用列表／单族输入
  通过 Fontique 查询解析，再从 `Query::family_ids` 借用有序 ID；
  保留重复项。Source 在进入 Unicode trim／族名规范化前拒绝非 ASCII。
  动态属性缓存的普通与受准入入口共用 `Cache::insert_with`，
  只在两个目标集合均已成功预留后写入完整 entry。

info 和属性缓存的受准入增长使用可证明的几何容量计划；
扣费后以 `try_reserve_exact` 预留计划容量。新增容量与旧元素迁移分开统计，
避免逐个字符精确增长导致平方级复制。费用预付新记录的写入和最终释放；
brush 的自定义 equality／default 行为仍由提供方负责，Core 使用 `usize` brush。
本子片没有执行 shaping，没有登记分析 lease，也没有修改 Flow 的能力选择。

Native／portable 各十项隔离测试通过，其中新增八项、既有断行准入两项。
新增测试覆盖等价范围输出、拒绝每个准入点后的停止与恢复、
Copy brush 的 `Clone` 禁用、4096 字符容量增长及热复用、
族顺序／重复项／缓存命中，以及未知族名和 Bidi 的提前拒绝。
私有测试副本排除了未随 crates.io 发布的上游测试支持；
portable 另排除上游依赖 std prelude 的 LRU 测试，生产源码未改动该测试。

锁文件核对发现：当前仓库解析到 Swash 0.2.10，早期消费者证据使用 0.2.6。
两版 `text/analyze.rs` 逐字节相同，`get_record_index` 均为三层只读索引。
初次 Native 内核测试在 0.2.10 通过；保存的 Native／portable 十项日志固定使用
0.2.6。此结论仅覆盖本文列出的边界分析 helper，不涵盖两版全部字体 helper。

普通入口另与 SHA-256 锁定的 Parley 原始归档对照：
960 组输入分别检查对齐、宽度重排和部分断行后的 drop，共 2880 个输出签名；
两侧使用相同 Fontique／Harfrust 生产依赖与 Swash 0.2.6。
签名覆盖 glyph、坐标、字体实例、样式、行与 inline，包含多语种输入；
日志 SHA-256 均为
`800a8e96c151bad25f2172a92ca12bf770d80c67d73962c4ef1df37d412421e5`。
这证明普通路径输出保持，不代表完整受准入 shaping 或 Core 输出等价已经验收。

本地产物位于同一证据目录：
`parley-analysis-native.log`、`parley-analysis-portable.log`、
`parley-frontend-original.log`、`parley-frontend-production.log`、
`parley-frontend-oracle.json`。新增
`dependencies/parley-0.7.0-analysis-frontend.patch`
已核对五个 source 的增量回放；Fontique 九个 source 的补丁回放仍通过。
本子片是第三项优化的合同补齐，没有产生新的渲染耗时或交互收益结论。

### Builder 初始化与样式准备

受准入 builder 已补齐初始化、默认属性、范围属性、内联对象与分析准备。
本入口仍为 Parley 内部接口，尚未生成完整 layout 或分析 lease。
此前完整缓存可以保留；旧属性缓存、styles、info、inline 与 Bidi 清理所访问的
记录均先扣费。注册内存字体的 source cache 使用独立的标量 prune；
shared 或非空文件缓存在进入 retain 扫描前拒绝，普通 prune 保持。

FontStack 继续使用已补齐的 Fontique 查询。Copy brush 逐字段读取，
其余标量属性共用普通解析分支；借用的 feature／variation 列表先预留，
按实际比较与移动执行稳定排序，重复 tag 的先后顺序保持。
非空字符串 settings 与 locale parser 尚未支持，在解析前拒绝。
内联对象也使用逐步准入的稳定排序，同一文本位置的输入顺序保持。
这些前段支持条件不代表后续 shaping 已支持相同输入。

当前锁定 Swash 0.2.10 的 Native／portable 各 13 项隔离测试通过，
包括既有十项与新增三项 builder 测试。新增测试核对 styles、字符信息与
内联顺序等价，feature／variation 的重复 tag，以及每个前段准入点的
停止和同一上下文恢复。Fontique 内部拒绝的嵌套原始错误也在断言范围内。
Parley 七个 source 的增量回放与 Fontique 十个 source 的回放均逐字节通过。
可重复入口为 `tools/scripts/check_parley_analysis_admission.py`，
日志与 `analysis-frontend-receipt.json` 保存在本批证据目录。
本轮没有新交互计时；默认分析／重排复用继续保持未采用状态。

## 采样与证据

中间产物位于 `target/verification/text-draw2d-followup-2026-11-05/`。
每项使用冻结的 release 可执行文件，关闭 tracing 的独立计时不与构建并行。
等值 setter 样本重复设置同一内容与展示属性，再执行 intrinsic／presentation refresh，
记录 layout 调用次数及总耗时。
Flow 使用相同拖拽、滚轮和连续 resize 回放比较上一版与本版，Perfetto 用于归因。
同时采样默认完整组装与开启 `--flow-assembly-reuse` 的路径。
比较脚本新增 `--no-flow-assembly-reuse`，用于默认路径的前后对照；
保留原有脚本默认行为。只测开启复用的路径会在多数 resize 中跳过工作区，
不足以评价临时容量复用。
匹配回放摘要不替代完整像素、屏幕呈现或真实 OS 拖动节奏验收。

### Label 等值修改

冻结的 `baseline-tests` 与 `adopted-tests` 按前／后、后／前、前／后交替采样。
每份样本包含 100 轮等值 setter 和缓存刷新。

| 指标 | 修改前 | 修改后 |
|---|---:|---:|
| 每份 layout 调用 | 1,400 | 0 |
| 三份总耗时（μs） | 244,262／251,478／247,826 | 21／21／22 |
| 总耗时中位数（ms） | 247.826 | 0.021 |

`CountingEngine` 包装真实 `ParleyTextEngine` 并注册 `BuiltinFont::ALL`，
仅在转发 `layout` 时累计调用数。样本证明等值 setter 不再触发重复的真实布局；
计时覆盖局部 setter 与刷新，不包含 GPU 或整页交互。原始结果保存在
`label-comparison.json` 和六份 `*-label-*.txt`。

### 默认路径的交互成本

每版运行三次，每次一轮相同原生事件回放，使用离屏 GPU 并等待 GPU 队列。
下表为关闭 tracing 的活动时间与帧处理时间中位数；活动时间包含输入处理、
帧处理及显式 GPU 等待。

| 操作 | 活动时间前／后（ms） | 帧处理前／后（ms） |
|---|---:|---:|
| 拖拽移动 | 32.025／31.335 | 13.297／13.292 |
| 滚轮 | 31.323／31.581 | 13.114／13.288 |
| 连续缩放 | 58.535／57.991 | 14.199／14.302 |
| 追加 | 41.293／41.041 | 13.804／13.690 |

这些差异没有显示一致的整体收益，不能仅凭中位数的小幅变化归因于工作区。
原始配置、可执行文件散列、每次操作和分布保存在
`default-comparison/receipts.json` 与 `comparison.json`。
378 次配对操作的文本字节、行、run、片段数、有效宽度、首个来源文本和滚动位移
全部一致；这是摘要级核对，未验证完整像素或屏幕呈现。

### 开启 assembly reuse 的对照

相同配置和采样次数下，另外 378 次配对操作的摘要核对通过。
这组只用于检查兼容性，不与上组跨配置比较速度。

| 操作 | 活动时间前／后（ms） | 帧处理前／后（ms） |
|---|---:|---:|
| 拖拽移动 | 39.632／39.310 | 17.016／16.771 |
| 滚轮 | 39.072／39.468 | 16.121／16.543 |
| 连续缩放 | 66.507／66.666 | 17.063／17.054 |
| 追加 | 47.392／49.169 | 16.410／17.350 |

原始证据保存在 `reuse-comparison/`。两组各次运行的中位数保存在
`run-medians.json`；例如默认路径缩放的运行中位数前为
57.448～59.236 ms，后为 56.821～61.487 ms，存在明显重叠。
工作区已通过正确性门禁，但没有证实一致的交互净收益，不关闭其性能验收。

### Perfetto 阶段核对

另采默认完整组装路径的一轮前后回放。每份主 trace 都有 128 个
`host-operation` 和 696 个 `flow-contribution`；主 trace 包含初始化／重置，
独立交互摘要排除这些操作。退出后另保存的尾部 trace 不计入对照。
PerfettoSQL 查询未发现正值的 error、lost 或 drop 计数。
阶段 SQL 与结果保存在 `stages.sql`、`baseline-stages.csv`、
`adopted-stages.csv`，捕获边界见 `trace-validation.json`。

| 阶段 | 次数前／后 | 累计时长前／后（ms） |
|---|---:|---:|
| `flow-contribution` | 696／696 | 4,022.595／3,971.057 |
| `flow-build` | 696／696 | 4,494.492／4,452.650 |
| `text-analysis-shaping` | 128／128 | 92.644／91.484 |
| `text-wrap` | 128／128 | 2.006／1.757 |
| `text-export` | 128／128 | 8.414／8.329 |

组装累计时长约下降 1.28%，分析调用次数不变。此处为带 tracing 的阶段经过时间，
父子 scope 有包含关系，不能把表中时长相加，也不能当作 CPU 调度时间。
没有 allocator 采样，容量保留由单元测试证明；该 trace 不单独证明分配次数或净收益。
结合独立计时，本批确认等值 Label 的局部收益，Flow 的整体交互收益仍未确认。

## 正确性与边界验证

| 验证 | 结果 |
|---|---|
| Core Label／Flow 单元测试 | 23 项通过，2 项 ignored 成本测试不计入；本批等值成本另采样 |
| `m10_label_contract` | 13 项通过 |
| `p2_t03_structured_flow_contract` | 22 项通过 |
| `p2_t03_external_flow_engine` | 1 项通过 |
| `flow_assembly_reuse_contract` | 8 项通过 |
| `p2_text_stages` | 20 项通过；不表示默认 Parley 已接入 |
| 独立 `flow-assembly-reuse` 消费者 | 8 项通过 |
| Core `--lib` Clippy | 通过，Core 无诊断；保留 Harfrust 既有 qualification 警告 |
| workspace fmt／check、`cargo xtask docs` | 通过；check 保留既有警告，文档检查覆盖 498 份 Markdown |

工作区用例核对复用容量、FigureId 和 source offset 清空、超限释放、错误及
unwind 释放未计费增长、独立实例隔离。
既有服务用例核对输入证明在组装中变化时的原始失败和留存释放，
以及已发布来源在服务销毁后的独立 lease。
等值 Label 用例核对缓存和 revision 不变、真实变化按原范围失效，
非法 gap 仍在等值检查之前报错。
