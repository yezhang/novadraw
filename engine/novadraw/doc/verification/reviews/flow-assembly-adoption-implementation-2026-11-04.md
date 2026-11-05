# FR-02 整页组装采用实施记录

类型：`verification-record`

能力归属：Core Flow 的开放声明、不可变组装资产和 Preparation 输入证明。
日期：2026-11-04。状态：实施中，默认关闭；长文本研究驱动已接入，
无窗口匹配功能回放已完成。2026-11-05 原生失焦与临时 drawable 不可用的
两处退出路径已定位并修复，定向回归通过；原生回放已成功呈现，
用户已确认 F2 不再退出、resize 更流畅，窗口切换及重置后的按键仍待补验。
[最新原生滚动对照](flow-assembly-scroll-comparison-2026-11-05.md)
确认开启复用后成本回退，净性能门禁仍未关闭。

## 已实现的接口与采用路径

`FlowContributor` 和 `FlowContributionHandler<P>` 增加同名缺省方法
`assembly_reuse`，返回 `Result<Option<FlowReuseDeclaration>, FlowError>`。
现有闭包 handler 的实现保持；声明闭包通过公开 `DeclaredFlowHandler::new` 适配器，
再由普通 `register` 注册。标准文字、无片段边框的 block 和 group 使用相同入口。

`FlowReuseIdentity::new` 创建实例身份，clone 保留身份；
`declaration(generation)`、`with_contribution(key)` 和 `with_input(snapshot)`
分别声明版本、完整有序 payload 类型序列和受检输入。
generation 不允许回绕，耗尽时更换 identity。
首次范围要求组装不依赖当前测量约束；派生读取须由完整受检输入覆盖。
这仍是扩展作者对输出、失败和副作用的承诺，不是自动证明任意回调纯度。

服务的输入 reader 通过统一 `PreparationInputRegistry` 捕获声明和完整 source。
先检查 root 声明，再捕获其余成员、注册 token、声明和 source 输入；
任一必要声明缺失则完整组装。历史值不同是 miss，
本次首次值与后续重验不同则保留 `TextError::PreparationInput` 的完整证据并失败。
冷构建检查实际 payload 类型序列，拒绝与声明不符的输出。

启用后，measure 与 prepare 可借用同一个 `FrozenFlowPlan`；
width 相关的排版、inline box 工作及来源几何仍继续。
旧快照直接返回路径在启用模式下不采用，
本次输入证据仍随已有测量／准备／Runtime 证明链进入消费和发布检查。
片段边框尚无声明，含 decoration 的候选不安装组装 cache。

`FlowLayoutService::with_assembly_reuse` 默认关闭；
`work_statistics` 提供累计 assembly build、hit 和 ineligible 计数。
`set_assembly_limits(cache_units, retained_units)` 分开控制单槽驻留与家族留存，
`assembly_usage` 查询共用文字存储域。默认兼容构造使用无上限域；
研究消费者显式预算见下节；生产默认配置和完整成本判据仍待验收。

## 长文本接入与本轮修正

InteractivePage 使用公开 `FlowReuseIdentity`、完整贡献类型声明和普通服务，
不走内置 root 特权路径。`--flow-assembly-reuse` 默认关闭，重置后保留配置；
`Research` 持有同一家族的 service clone，直接读取累计工作计数，
snapshot 的旧统计不用于代表本次工作。
开关可与 `--text-raster-cache` 组合，但各自的收益必须单独测量。

研究配置的 cache／留存上限分别为 16 × 1024² 和 64 × 1024² 逻辑单位。
单次 preparation 的 work／allocation 为 128 × 1024² 和 64 × 1024²，
关闭和开启两组一致。该配置不修改 Core 默认预算，也不代表全 Runtime 内存上限。
首轮发现不可变来源 metadata 将字节数加入 collection entry 预算；
已按原域的条目单位纠正，来源 backing 的留存仍按其实际容量逻辑 charge 准入。
默认预算下的六个官方文字场景继续通过。

新组装结果作为 build candidate 返回；prepare 的 projection、PreparedScene 校验和
最终 Flow 输入检查成功后才安装，失败保留原成功槽。measure 在成功返回时安装。
启用模式不再写入旧完整 snapshot cache，避免持有无用途的旧准备结果。

`FlowSourceRanges` 替代公开裸 Arc；来源及 ancestors 独立持有 accounting lease。
单独克隆来源、销毁 prepared／service／文字引擎后仍合法，最后来源 clone 释放后扣账。
输入 proof 改为共享不可变 record，快照与 proof record 分别取得 reservation；
克隆不再复制整个 proof record。声明参与者、handler 列表和来源导入先按
当前 preparation 域准入，再分配；扩展 callback 负责其自身临时分配。
`FlowError::into_text` 公开供消费者 adapter 保留结构化 Text／Preparation 错误。

保守 source 比较保留 root 的 source bounds。只改变 measure constraints 可命中；
真正修改 source bounds 的 resize 仍可能 miss，不将此实现描述为全部宽度独立命中。

## 第一轮验证

Core `figure::flow::reuse::tests` 的 6 项通过：

- 同页面在两个测量宽度下只贡献一次，第二次命中，断行尺寸仍随宽度变化；
- 独立服务不继承旧缓存；
- 私有 generation 在两次调用间变化后重建；
- generation 在贡献过程中变化后返回错误，不安装资产；
- handler 未声明时每次完整执行；
- 服务 clone 共用缩容配置，旧内容仍留存，正 charge 新增被拒绝，提额后恢复。

标准文字和外部节点使用相同声明路径的单位反例通过。
这不代替独立 crate 编译、完整 Runtime 发布反例和机器性能对照。

后续续验已覆盖注册身份与受计费的紧凑输入快照：
Core lib Clippy（`-D warnings`）通过，Flow 模块 11 项、
Preparation 输入证明 10 项通过。本次未重复 workspace 全量或深树释放测试。
既有 Harfrust qualification 和测试夹具 unused 警告保持，未作为本片改动。

## 尚待完成

本轮后续已加入输入快照注册身份和 proof 留存准入并通过上述定向续验；
首轮完整 source 证据在 reader 内复制，成本对照发现回退；
后续改为共享 immutable prefix，并为导入的 backing 单独取得家族 lease。
同一个仍存活的 backing 通过弱引用索引复用 reservation；索引不保活来源，
指针只定位候选，成功升级的引用持有原来源。source 有变化时仍比较完整值；
相同不可变 backing 可免去重复值扫描，provider 与声明仍按原检查点重读。
弱引用索引及嵌套输入的完整容量门禁仍未关闭。
共享来源 slice 的独立导出寿命及 proof record 的共享已补齐并有定向证据；
容器级 proof metadata、跨域输入 backing 去重及全部增长前准入仍须核对。
不能将现有 `assembly_usage` 当作这些范围均已验收。

还需补齐声明／handler 动态失败、registry 替换、不同 root／Runtime、
完整输入碰撞、边框与真实 inline、旧快照寿命、最终发布前变化的生产反例，
并完成独立消费者 Native／Wasm、API 门禁及相关回归。
长文本驱动开关及原 Large 的匹配工作分母已交付；完整场景净成本尚未验收，
本记录不关闭 FA-G03／G04、FR-02 或 LT 延迟目标。

## 首轮成本对照与整改原因

同一 release 可执行文件，Rust 1.99.1，相同字体、full redraw、
有界 preparation 配置，执行 Reset／Append／两个 Scroll／Stable／八个 Resize，
两组均 13 操作、13 CPU 记录帧，无 trace health error／data loss。
每个操作的文字量、行数、runs、fragments、glyph 与命令数相同。
初始为 40 段／51,232 字节，追加后 51,233；排版引擎调用两组均为 32 次，
滚动不增加排版调用。

| 首轮观察 | 关闭 | 开启 |
|---|---:|---:|
| Flow build 次数 | 126 | 126 |
| assembly build 次数 | 126 | 13 |
| assembly hit 次数 | 0 | 113 |
| trace 中 host operation 总时间 | 1020.271 ms | 610.656 ms |
| trace 中 host frame 总时间 | 69.982 ms | 108.092 ms |
| flow contribution inclusive | 751.290 ms | 75.035 ms |
| qualification 重读 | 无 | 830 次／278.094 ms |
| tracing 关闭的整进程两次耗时 | 557.927／428.277 ms | 723.638／685.286 ms |

宿主 operation 与 frame 不重叠，其他阶段存在包含关系，不能全部求和。
关闭路径产生更多细粒度事件，trace 里的耗时下降不等于无诊断净收益。
无 tracing 的 ABBA 对照反而回退，因此拒绝以首轮 trace 宣称提速或默认启用。
上述 source backing 共享针对这一结果继续整改；本轮完整复测另行记录。
两次整进程样本只包含启动与整轮回放，不能用于 P95／P99、FPS 或 OS 输入延迟。

首轮原始文件均在 `target/verification/text-fr02-2026-11-04/`：
关闭 `assembly-off/textflow-1793801567453038000-94607-0.pftrace`，
开启 `assembly-on/textflow-1793801568412657000-94701-0.pftrace`，
无 tracing 收据 `assembly-timing-off.json`。
可重跑查询位于同目录的 `assembly-analysis.sql`：

```sh
trace_processor query TRACE.pftrace -f target/verification/text-fr02-2026-11-04/assembly-analysis.sql
```

该查询读取 host-record 的完整 JSON，按 parent host-operation 对应真实操作；
对照其 snapshot 分母及各帧 glyph／commands，同时检查 trace health。

## 来源共享后的续验与手动验证

最新 Flow 单位用例 13 项通过，包括同源 proof 重读共享一个计费 backing、
来源句柄独立寿命、声明变化、未声明回退及缩容反例。
独立消费者 8 项、Native／Wasm 编译、Core lib Clippy 与公开 API 门禁通过。
普通库代码中的 `ReusablePage` 编译外部 root、handler、声明及 measure／prepare
adapter；不是仅通过 `cfg(test)` 的 Native 编译。
研究应用 SDK 的操作／失败／重试／新导出合同通过；未重复 workspace 全量。

最新关闭 trace 为
`assembly-off-shared-source/textflow-1793803647039809000-99762-0.pftrace`，
开启 trace 为
`assembly-on-shared-source/textflow-1793803647840253000-99768-0.pftrace`。
两组仍均 13 操作、13 CPU 记录帧，完整输出分母相同且无 health error／data loss。
assembly 仍为 126 → 13 次，113 次命中，排版调用仍同为 32；
qualification 为 830 次／257.314 ms，重读本身尚未消除。
trace 中 host-operation 为 945.491 → 568.304 ms，host-frame 为
64.286 → 104.438 ms。该数据仍不能排除观测扰动。
关闭 tracing 的续采入口为同证据目录的 `measure-assembly-replay.py`，
收据为 `assembly-timing-off-shared-source.json`；源文件哈希在
`assembly-source-receipt.json`。
此次无 tracing 的 ABBA 整进程样本为：关闭 585.237／418.874 ms，
开启 684.014／646.365 ms，四次均成功。开启路径仍有回退；
来源共享尚不足以抵消声明、proof 与最终重验的成本，因此性能门禁继续保持未通过。
这些小样本用于发现方向性问题，不能据此定量承诺生产回退比例。

此前按无窗口回放给出的手验入口如下。2026-11-05 用户报告原生窗口退出，
该次手验准备判断遗漏了原生输入路径；退出修复和续验见下节。保持纹理缓存关闭：

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --clt --target-dir /tmp/novadraw-text-stage-target -- --flow-assembly-reuse --natural-damage --output target/verification/text-fr02-manual
```

上下滚动后按 F2 追加、F4 反复变宽、F1 重置，检查文字、断行、光标、滚动位置和响应。
关闭窗口，移除 `--flow-assembly-reuse` 启动同配置对照；macOS 功能键可能需要 Fn。
手动感受先关闭 tracing，采证时两组都加 `--trace`；组合纹理缓存另做一组。
这轮手验不关闭完整反例、容量／峰值、OS 输入延迟或生产启用门禁。

## 2026-11-05 原生失焦退出修复

故障属于 Core Preparation／布局缓存的输入证明生命周期。
研究应用捕获稳定化错误后主动退出；不是 macOS 系统崩溃。
原生 trace 显示初始化和三次 600 × 500 尺寸事件成功，
随后输入事件触发的第五次 `Stable` 操作在 `runtime publication` 返回
`flow.assembly-input` 的 `Changed` 错误。
原始 trace 已按原字节复制到
`target/verification/text-fr02-native-exit-2026-11-05/textflow-1793846203520913000-76743-0.pftrace`，
SHA-256 为 `91e471c0b014c00485ba9065ac4c75475a075a269e03294364a278de4a1c135c`。

无窗口定向回归用 `focus().release()` 复现了相同错误。
失焦通过 `SetFocused` 提交 paint 更新，改变页面 component revision，
Flow 保守输入因此变化。准备结果已经刷新，但尺寸未变，
`publish_prepared_figure` 不触发布局失效；布局仍持有上一轮的输入证明，
最终校验拒绝发布。

修复在新一轮稳定化开始、扩展回调执行前检查有效布局缓存的证明。
`Changed` 使对应容器及其祖先验证路径失效，布局重新消费当前输入并安装新证明。
其他 reader／注册／预算错误继续传播。
本轮计算中发生的输入变化仍失败并回滚，不通过覆盖首次值来消除冲突。
这是统一 typed 输入协议的缓存检查，外部 reader 与 Flow 共用该路径。
检查增加一次缓存输入重读，工作与分配由当前 preparation 预算承担；
其成本尚未加入新的无 tracing 对照，净性能判断继续保持未通过。

定向验证已通过：失焦后重新聚焦，尺寸和 placements 保持；
Core 输入证明 11 项覆盖旧证据保持、相同绘制尺寸下布局输入刷新、
布局计算中变化拒绝、晚于布局的回调变化拒绝和失败回滚。
Flow reuse 7 项、外部消费者 8 项及官方文字交互 6 项通过；
Core lib Clippy 和原生 release 构建通过。

修复后的原生续验额外开启 `--replay --trace --screenshot`。
trace
`target/verification/text-fr02-native-exit-2026-11-05/textflow-1793846764695738000-81979-0.pftrace`
显示初始化成功，8 次提交全部进入 `backend-surface-acquire` 后返回 `Skipped`，
研究应用报 `native frame retry limit reached` 并退出；没有完成一次呈现。
这次没有到达失焦操作，不能用该 trace 宣称失焦路径的原生验收通过。
启动脚本的 `open -W` 返回 0 也不能代替应用成功证据。

第二处故障归属应用／Native 宿主。后端在 drawable 临时不可用时跳过呈现，
研究应用原先将 `Skipped` 与实际 `Retry` 合并成布尔失败，连续主动重绘耗尽
8 次上限。现在 `FrameAttempt` 保留两类恢复结果：`SurfaceUnavailable`
通过 `ControlFlow::WaitUntil` 每隔 100 ms 重试，并允许窗口事件打断等待；
实际 `Retry` 仍受原 8 次上限约束。成功呈现或新操作重置恢复状态。
该间隔只用于研究宿主的表面恢复，不代表引擎帧率或性能目标。

最终应用 bin Clippy 与 release 构建通过；应用 3 项测试通过：
16 次临时表面不可用不耗尽失败预算，
混合 Skipped／Retry 保留实际失败上限并在成功后重置，以及 SDK 操作、失败、
重试和新导出合同。最终修复后尚未再次启动原生窗口；
本请求已完成两次启动尝试，手验入口仍使用上节的无 tracing 命令。
应补验窗口首次呈现、切走后恢复、追加、滚动、变宽及重置，
结果完成前保留原生验收未完成状态。

### F2 退出修复（2026-11-05）

能力归属：应用／Native 宿主。用户反馈窗口启动后按 F2 退出。
定向反例确认：`focus().release()` 后，Core 正确拒绝无焦点的键盘输入；
研究应用仍分发追加 `Z`，随后因 `fixture did not handle append Z` 报错退出。
窗口原先只处理 `Focused(false)`，没有恢复 `Focused(true)`。
这一故障与上一节布局输入证明失效、drawable 重试耗尽分别处理。

研究应用记录该场景的文字页目标，窗口重新聚焦和 F2 执行前均调用
Core 的 checked focus request。F1 创建新 Runtime 时同步更新 FigureId，
追加事件仍须确认已被处理，不吞掉其他输入错误。
这是研究场景的目标选择和操作编排，不新增 Core 焦点机制。

应用 4 项测试通过，新增回归直接调用 Native 的窗口焦点处理：
失焦、稳定化、重新聚焦，以及操作执行前再次失焦；
连续追加和重置后的追加均成功，重置后目标不等于旧 FigureId，
没有 deferred mutation errors。官方文字交互 7 项测试通过，
新增反例确认无焦点时未处理，恢复焦点后追加 `Z` 并完成重新布局。
这些是无窗口回归证据，实际窗口呈现和键盘交互另行验证。

应用 bin Clippy、相关包 fmt 检查和 release 构建通过。修复后原生执行：

```sh
python3 tools/scripts/run_textflow_large_macos.py --release --clt --target-dir /tmp/novadraw-text-stage-target -- --flow-assembly-reuse --natural-damage --replay --trace --screenshot --exit-after-replay --output target/verification/text-fr02-f2-native-2026-11-05
```

trace
`target/verification/text-fr02-f2-native-2026-11-05/textflow-1793847516633761000-88373-0.pftrace`
的 SHA-256 为
`45e659cbda807a082d25a51985a0b1fc6d2f50a0db23e887f72204aa00e190ed`。
13 个操作全部成功，12 次 `Presented`、1 次 `no_damage`；
初始化的 1 次 `Skipped` 后恢复呈现，没有 failure／frame_failure，
最终记录 `replay_complete`。Append 的首来源文字为 `Normal text.Z`，
后续滚动及 600 → 400 → 600 宽度回放均完成。
初始 GPU target 截图 `native-1793847515451939000.png` 已检查；
它不是桌面截图，也不证明实体按键经过 OS 输入链路。

随后使用上节无 tracing 命令打开手验窗口。仍待用户确认实体 F2、
窗口切走再切回后的 F2，以及 F1 重置后的 F2。
本轮只验证故障修复，未补做净性能对照，FR02 性能门禁保持未通过。
