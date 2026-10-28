# 长文本滚动与 resize 首批实施

类型：`implementation-verification`

日期：2026-10-28。状态：请求与阶段观测已实现并通过定向契约验证；
观测与共享 IR 首批实施、匹配采样及门禁已完成；
resize 16 ms 与查询/完整 IR 4 ms 性能目标尚未验收。

## 授权与范围

用户授权“先把最新修改提交 git，然后开始实施这个方案”。
前序最新修改已形成七个提交，最后为 `728ace3f`；未推送。
随后按[批准方案](../../design/architecture/long-text-scroll-resize-optimization-proposal.md)
在既有 LT-00 内登记单一测量切片。实施合同记入
[ADR-051](../../adr/adr-051-exact-flow-layout-reuse.md)，不另建算法执行队列。

## 已实现的观测

`RuntimeFrames::set_timing_enabled` 控制实例诊断，缺省关闭。
`take_flow_layout_observations` 读取后清空完整 request、root/paragraph owner、
服务 build 次序、既有 scene signature、来源 component versions、
engine identity/revision、measure/prepare、inline prefix、缓存命中和失败结果。
最多留存 4096 次请求，溢出报告 dropped，不把不完整分母当作完整证据。
关闭时不读计时时钟，不复制 request 或生成诊断记录；工作量整数计数仍保留。
既有 Flow 累计计时也改为随开关启用。

诊断不新调用 provider，不改变准备预算、依赖观察、源状态或发布结果。
请求分类核对完整输入；scene signature 只作归属，不能证明外部依赖相等。
请求持有不可变副本；启用时仍有诊断留存与复制成本，不能忽略这部分开销。

`TextLayoutEngine::layout_styled_with_diagnostics` 缺省委托原接口。
外部引擎与 Parley 均可通过同一个 `TextLayoutDiagnostics::time` 报告同步细阶段；
未报告的外部细阶段保持未知。Parley 当前分开换行、对齐、位置导出、
交互和校验；分析与 shaping 仍合并，不能进一步细分其占比。
Flow 另记录贡献读取、完整 request lookup、引擎计算与来源几何。

Runtime 将 IR 录制、IR 校验、retention/resources 与 submission 构造单独计时。
作用域退出包含失败前成本；inclusive/exclusive 保留嵌套归属。
驱动成功路径不再丢弃滚动和 resize 阶段，并将 unchanged query 单独归属。
Sampler 核对已报告的滚动、resize、submission 阶段自身耗时之和；
旧驱动缺少这些阶段时显式保留缺口。

## 原 Large 完整请求预检

使用独立冻结 source 和独立 Cargo target 编译的 Release 驱动。
三个 fresh Runtime 序列均成功；共保留 3960 次请求，无 dropped。
每个序列初始 520、追加 200、滚动 80、resize 520 次；
unchanged query 与随后 submission 的请求数为 0。
源追加仍只提交一次，后续稳定化不重复改变来源版本。

每个 resize 的完整分母为 measure 440、prepare 80：

| 路径 | 请求次数 | 真正调用引擎 | 缓存命中 |
|---|---:|---:|---:|
| measure | 440 | 6 | 434 |
| prepare | 80 | 0 | 80 |
| 合计 | 520 | 6 | 514 |

六次 miss 是三种不同完整输入分别在宽度 400 与 386 下计算，
所有 `inline_boxes` 为空，无真实 Figure prefix shaping。
它们不是同一完整 request 的六次重复；这不证明所有中间宽度计算都不可优化。
宽度反馈与调度是否能减少工作，还需保持完整测量与几何消费合同的消费者证明。

滚动的 80 次全部为 measure/cache hit，无引擎计算。
它仍执行两次 Flow 构建并重新读取贡献；不能把整个稳定查询都归为 shaping。

## 正式 CPU 阶段采样

冻结版本五个会话完成 1000 次热样本、200 个新进程样本、100 次预热，
无失败。1300 个序列的初始/追加阶段核算及滚动/resize/submission 核算通过；
200 个新进程样本不混入下表，预热也排除。分位数使用整数 ns 的 nearest rank。

| 热样本操作，n=1000 | p50 | p95 |
|---|---:|---:|
| 初始精确稳定化 | 38.579 ms | 50.714 ms |
| 固定源追加至稳定 | 16.784 ms | 21.551 ms |
| 滚动至稳定查询 | 6.635 ms | 8.866 ms |
| 宽度 resize 至稳定查询 | 35.722 ms | 43.674 ms |
| resize 后完整 CPU submission 准备 | 4.825 ms | 6.565 ms |

每个热样本均保持滚动新增引擎调用 0、resize 新增 6、
7889 条命令与 51233 个原生 glyph。没有实施可见裁剪。
最后一行包含再稳定化、IR 录制、校验与 submission 构造，不能全部归为数组复制。

| 热样本阶段 | 每次操作的平均 ms | 归属 |
|---|---:|---|
| resize：贡献读取 | 9.907 | exclusive，13 次 Flow 构建 |
| resize：布局自身 | 7.250 | exclusive，3 次布局阶段 |
| resize：文字引擎 | 4.923 | inclusive，6 次完整 request |
| resize：分析/shaping | 4.337 | exclusive，引擎子阶段 |
| resize：Flow 构建自身 | 2.683 | exclusive |
| scroll：布局自身 | 2.229 | exclusive |
| scroll：贡献读取 | 1.517 | exclusive，两次 measure 构建 |
| submission：再次稳定化 | 2.597 | inclusive |
| submission：IR 录制 | 1.700 | exclusive |
| submission：IR 校验 | 0.249 | exclusive |
| submission：构造 | 0.328 | exclusive |

inclusive 值包含子阶段，不能与这些子阶段再次相加；
各阶段分位数不能相加推导端到端分位数或显示帧率。
目前真实成本支持随后共享 IR，同时显示出重复贡献读取与布局/校验的显著成本。
宽度复用即使省去全部本轮分析/shaping，也不单独证明能达到 16 ms。

## 验证与证据归属

已通过：Core lib check、驱动 check、Core lib Clippy、两项计时单元测试、
六项长文本布局外部契约测试、一项独立 ASCII 引擎的绘制/交互/诊断失败测试，
以及 416 文件文档门禁。诊断开关不改变几何、glyph 输出与引擎调用次数；
独立引擎通过公开入口报告自己的细阶段，失败记录保留新来源版本。
最终 workspace fmt/check/Clippy/test 已通过：1575 项通过、0 失败，
19 项预设忽略。后续 xtask 编排与 sampler 配置修改另通过格式、Clippy 和
Python 编译检查；共享范围与真实 Large backing 测试也单独通过。
文档门禁通过 418 文件。本页尚不宣称整批优化毕业。

## 共享 glyph IR 实施

公开 `text::shaping::GlyphBuffer::new` 接收 owned 数据并检查有限坐标，
`GlyphRange` 以私有字段保存完整 backing 与受检 glyph indices。
相对范围切分仅检查索引，不复制或重扫全文。空范围也持有完整 backing；
读取提供 slice/Deref/只读迭代，不提供可变数组入口。
范围相等按 view 内的 glyph 值判断，保留原 Vec 的值语义，不纳入 backing
身份或范围外数据；该最终调整另通过 11 项定向测试与 Core/test Clippy。

`GlyphRun.glyphs` 改为 `GlyphRange`，`normalized_coords` 改为 `Arc<[i16]>`。
内置 Parley、独立 ASCII 引擎与其他外部 owned 构造均迁移至同一受检入口。
`draw_glyph_runs` 与 submission 的 command clone 保留同一 backing。
outline 的坐标 key 也共享；字体指标的 owned API 仍显式转换，
Vello 编码的 glyph/variation 转换成本不属于本次消除的复制。
行间距确实修改坐标时重新构造缓冲，旧布局与提交保持不可变。
字体、size/skew、资源版本、设备坐标和 session 准入继续走既有校验。

`RecordedDrawing::command_bytes` 仍保守地按每条命令钉住的完整 backing 和 Arc
计数器准入。`shared_glyph_backing_bytes` 单独统计一个 drawing 的唯一 backing：
重叠范围与空范围不重复计同一 allocation，不能以 view 长度替代完整 backing。
该观察包含 payload 与 Arc 计数器；command metadata、字体资源与 allocator
开销不混入这个值。缓存、当前帧和外部旧 lease 属于不同责任域，不能将
单一 drawing 的观察冒充整个 Runtime 的留存总量。

已通过独立范围/非有限坐标测试、保守预算与唯一计费测试、
外部 backend 的 Recorder→command clone→submission 缓冲身份检查、
缓存 257 次更新/淘汰后的旧 lease 检查，以及真实 Runtime 源退休后的旧提交留存。
文字绘制、组合字符/RTL/affinity、下划线、动画 glyph/font 留存的定向回归通过。
18 组长文本 Full/Partial GPU 对照通过；此门禁沿用实际 GPU demo，
并不表示已完成生产原生 glyph 裁剪或真实窗口滚动验收。

独立 `shared_glyph_backing` consumer 使用原 `TextFlowLargeExample` 工厂，
在固定追加前冻结一个实际 Runtime submission：绘制 glyph 分母为 51232，
唯一 glyph/coords payload 为 58788 bytes，Arc 计数器为 2592 bytes。
这说明重复完整 request 的布局缓存也让不同段落保留相同 run backing；
不能直接用绘制 glyph 数 × 单 glyph 大小作为唯一内存。
逐命令核对 layout、submission 和整帧 clone 的 backing identity，
resize 并销毁 Runtime 后旧帧仍保持原数据。
观察在计时外执行；它不是 51233-glyph 追加后性能样本的内存测量，
也不是整个缓存或字体的峰值归因。

公开 API 快照新增 `module.text.shaping` 范围，
覆盖两个共享类型及原 shaping 入口，并补齐本轮诊断类型。
独立受控源码的 Native/wasm rustdoc 依赖边界与 Runtime method budget 门禁通过，
`novadraw-platform-web` 的 wasm32 构建通过。

共享版另运行三个完整 request 预检，3960 次请求无 dropped 或失败：
scroll 仍为每序列 80 次 measure/hit，resize 仍为每序列 6 miss 与 514 hit。
三个序列均保持 7889 commands、51233 native glyph，无新增 scroll shaping；
共享改变存储与克隆方式，没有提前获得可见裁剪收益。
原始记录为 `target/verification/long-text/shared-ir-requests-20261028.ndjson`。

注册的 `text.long-layout` suite 中七组测试共 40 项通过，
含实际 GPU Full/Partial 测试，Release 构建也通过。
首次在恢复归档中运行时 smoke 因缺少 Git metadata 失败，未计为成功；
随后补齐 `CARGO_TARGET_DIR` 和无 Git 归档模式，并单独恢复 smoke 步骤。
旧驱动失败预检与上述工具失败均保留原始记录，排除正式性能分母。

## 匹配 CPU 对照的冻结输入

比较使用最终范围值语义修正版，早期共享版不纳入正式对照。
两版的驱动、workloads、原工厂、字体、实际 Cargo.lock、Cargo 配置及计时入口
逐项一致；源码与二进制分别冻结。五个会话交替先后次序，
新进程按两版成对交错。采样期间不运行本 Agent 的构建或测试；
外部后台负载仍不受控。

完整恢复材料位于 `target/verification/long-text/scroll-resize-shared-20261028`，
每版保留源码归档、manifest、可执行文件与原始样本，
会话环境与 power/load 记录保存在比较目录。摘要的顶层 extended 合并热/新进程，
最终热样本表须单独筛选 `fresh-runtime`；不合并预热或新进程。

| 最终共享版输入 | SHA-256 |
|---|---|
| 源码归档 | `8ca3468b1bd61d3b6ea3248fe483bf94143fd07e7040a35bc22300c34d4d0e4e` |
| 二进制，63601728 bytes | `b8d875d918de1813af54e57c0ace3007f9ecb126446f01f9439ace2e0f23203b` |
| sampler | `16cd6ab47bb8771db621c36e5c46802b2895cf6f844bcdf8065c29e83543479d` |

两版 Native CPU 测试可执行文件分别为 63631936 与 63601728 bytes，
共享版减少 30208 bytes。这是同配置的 Native 产物观察，
不表示 Web 传输、生产应用包或 GPU 存储的体积已验收。

## 匹配结果与切片退出

每版五个会话完成 1000 热样本、200 新进程、100 预热，无失败。
每版 1300 个序列的源提交与阶段核算通过；所有扩展序列保持
scroll 新增 shaping 0、resize 新增 6、7889 commands、51233 native glyph。
归档模式 smoke 的失败步骤已恢复，五个序列成功；不是完整统计验收的替代。

| 热样本 n=1000；ms | 基线 p50 | 共享版 p50 | 基线 p95 | 共享版 p95 |
|---|---:|---:|---:|---:|
| 初始精确稳定化 | 34.675 | 34.227 | 36.638 | 35.944 |
| 固定源追加至稳定 | 14.917 | 14.705 | 15.919 | 15.513 |
| 滚动至稳定查询 | 5.900 | 5.797 | 6.435 | 6.316 |
| resize 至稳定查询 | 31.975 | 31.528 | 33.835 | 32.952 |
| resize 后完整 CPU submission 准备 | 4.442 | 4.131 | 4.935 | 4.561 |
| 其中：IR 录制 | 1.477 | 1.343 | 1.752 | 1.602 |
| 其中：submission 构造 | 0.276 | 0.194 | 0.367 | 0.291 |

完整 CPU submission 的 p95 减少约 7.6%，五个会话分别从
4.820/4.873/4.842/5.125/4.915 ms 降至
4.470/4.550/4.579/4.540/4.567 ms。
录制与构造的两个数组 clone 已消除，均值分别从 1.501/0.284 ms
变为 1.367/0.203 ms。阶段分位数不相加；完整提交仍包含再次稳定化，
共享版该部分 inclusive 平均为 2.241 ms，IR 校验平均为 0.217 ms。

scroll/resize 的变化较小，部分会话 resize 的方向相反；
不把这些差异单独归为共享 IR 的算法收益。本轮原 Large 追加的共享版五个
会话 p95 都低于 16 ms，但基线聚合 p95 也已低于 16 ms。
长段/长行未在本轮重采样，不能据此关闭长文本整体目标。
resize 32.952 ms 与完整查询/IR 4.561 ms 仍超过本方案的 16/4 ms 目标。
该 CPU 合成输入不包含真实窗口、GPU 完成或屏幕呈现，不能据此承诺显示帧率。

共享版 resize 中贡献读取、layout 自身和分析/shaping 平均分别为
8.733/6.009/3.848 ms（exclusive）；其余签名、准备和发布仍有成本。
这些证据支持下一步优先核验重复贡献读取与稳定化的复用边界，
而不是仅用减少 shaping 推断 resize 会达到 16 ms。
外部无纯度/版本声明的 contributor 仍须重读；不得按样例或内置类型省略 callback。
原生覆盖与多宽度复用继续依各自准入取证，未自动进入实现或验收完成状态。

每版另做五个交错新进程 RSS 观察，无失败；范围分别为
125976576～132497408 与 124551168～132726784 bytes。
中位数分别为 129810432 与 130711552 bytes；分布重叠，
未证明整个进程峰值下降。该 RSS 驱动仅覆盖标准构造/追加，
没有扩展场景的完整 frame/GPU 留存；当前帧 backing、缓存、字体与旧 lease
仍须分别归因。不可用 RSS 替代完整 backing 的实际计费或硬预算证明。

完整对照与复现入口：

- [热样本与五会话分布](../../../target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json)
- [基线 manifest](../../../target/verification/long-text/scroll-resize-shared-20261028/baseline/manifest.json)、
  [共享版 manifest](../../../target/verification/long-text/scroll-resize-shared-20261028/optimized/manifest.json)
- [基线原始样本](../../../target/verification/long-text/scroll-resize-shared-20261028/baseline/samples.ndjson)、
  [共享版原始样本](../../../target/verification/long-text/scroll-resize-shared-20261028/optimized/samples.ndjson)
- [五对进程的 RSS 原始记录](../../../target/verification/long-text/scroll-resize-shared-20261028/memory.json)

首批观测与共享 IR 切片已验证退出；整体 LT-00～LT-03 和窗口/平台验收仍未关闭。

最初复制默认 `target/release` 的预检遇到旧驱动断言并失败，明确排除。
保留失败资料，不将其纳入有效 CPU 样本或成功次数。
随后使用 `/tmp/novadraw-sr-measurement-source-20261028` 独立源码、
`/tmp/novadraw-sr-measurement-build-20261028` 独立构建目录，
产物及完整源码归档冻结在 `/tmp/novadraw-sr-measurement-verified-20261028`。
包含实际 Cargo.lock、全部原字体、manifest 与二进制散列。
三个完整 request 序列保存在 `/tmp/novadraw-sr-requests-verified-20261028.ndjson`。
正式序列为冻结目录中的 `samples.ndjson`；`summary.json` 的顶层 extended
统计合并了热样本与新进程，本页热样本分布单独从原始记录筛选。

| 冻结输入 | SHA-256 |
|---|---|
| 源码归档 | `bd16be6bad9e63705b24ae2280c4798c7f3b79f8bcd5f2054ec9ae50538d0bc4` |
| 实际 Cargo.lock | `90e304dcf7be0ca5c010bd50107031d80940c83414d95ec67d48aa9181defb3c` |
| 二进制，63631936 bytes | `ab01e7fae2474662cff4708d34c98b058724dfcbde0eef015142dfff2eb8608b` |
| 阶段核算 sampler | `bd92819c88aedaf86d38336ed80679609198613899aa4385576c8a3f2c26a0dd` |

本轮为 CPU 合成 viewport/root resize 输入，不是连续鼠标、真实窗口、
GPU 完成或屏幕呈现证据；外部后台负载未受控。
不与历史二进制计算严格提速比。16 ms resize 和 4 ms 查询/IR 目标保持未验收。
原生覆盖 SR-R01、默认宽度复用 SR-R02、局部 shaping／checkpoint LTI-R01/R02
不因观测通过而关闭。
