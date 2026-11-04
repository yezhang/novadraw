# ADR-059 B：Parley 存储补丁已接入

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）文字引擎的第三方依赖。
Editor、backend、platform 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`，既有语义保持。

已完成可复现的只读存储补丁及定向验证，并按用户本轮批准接入生产依赖。
它补齐 Parley Layout 十个私有数组的容量观察和共享字体枚举。
B 的阶段适配器、操作前计费和退出门禁仍未完成；C／D 未开始。
普通 Parley／Outline 与 Flow 继续使用完整布局。
后续已补齐复制和对齐的操作前准入，见
[操作实施记录](text-stages-b-operations-2026-11-02.md)；本记录保留最初存储补丁的验证范围。

用户已经批准 ADR-059 的 A→D 实施，并于本轮回复“批准吧”，
解除存储补丁的独立批准门禁。原暂停依据为：
[ADR-059](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt)明确写明
“本 ADR 目前没有授权或实现该补丁”，并要求取得“上游或经评审依赖补丁”。
下文保留候选验证范围，并在末节记录本轮生产接入；B 的后续退出条件保持。

## 补丁来源和作用范围

| 项目 | 本次记录 |
|---|---|
| 上游 | `https://github.com/linebender/parley` |
| 源版本 | crates.io `parley 0.7.0` |
| 发布源码对应提交 | `10fedab4bfd4254a227a1a0f9602b6218e1fe447`，包内 `.cargo_vcs_info.json` |
| 原始归档 SHA-256 | `ada5338c3a9794af7342e6f765b6e78740db37378aced034d7bf72c96b94ed94`，与生产 `Cargo.lock` 一致 |
| 许可证 | `Apache-2.0 OR MIT`；原始版权与许可证保留 |
| 本地新增模块 | `src/layout/storage.rs`，本地版权及同一双许可证 |
| 上游源码改动 | `src/layout/mod.rs` 只增加模块声明和重导出 |
| 补丁状态 | 用户已批准生产接入，自审与定向验证完成；尚未独立评审、未提交上游 |

实际产物：

- [完整依赖补丁](../../../dependencies/parley-0.7.0-storage.patch)
- [可直接阅读的新增模块](../../../dependencies/parley-0.7.0/src/layout/storage.rs)
- 本地来源、源码保持与验证 receipt：
  `target/verification/text-stages-b-2026-11-02/verification.json`
- [隔离消费者及运行说明](../../../tests/consumers/parley-storage/README.md)
- [准备脚本](../../../tools/scripts/prepare_parley_storage_candidate.py)与[复核脚本](../../../tools/scripts/check_parley_storage_candidate.py)

补丁增加 `Layout::owned_storage` 和 `Layout::retained_fonts`。
前者返回固定大小的快照，记录 fonts、coords、styles、inline boxes、runs、
items、clusters、glyphs、lines、line items 的长度、实际容量及元素大小，
以及包含容器头的 Layout 对象大小。容量乘法和总量相加使用受检算术。
后者借用现有 FontData，不复制字体或建立第二份 FontContext。
接口不访问 allocator，不改变排版算法，也不借助私有内存转型。

快照只覆盖 Layout 直接拥有的存储。字体 backing、Arc 控制块、brush 的外部
存储及 FontContext／LayoutContext 内部缓存须由对应 owner 另行申报。
`usize` brush 本身没有外部存储；这个事实不能推广到任意实现 Brush 的类型。
读取完成后的快照也不能替代将来 clone 或断行的操作前准入计划。

## 留存计费的可行边界

默认引擎的字体来源是自身登记的 `Arc<Vec<u8>>`，FontContext 关闭系统字体。
因此适配器可在登记字体时记录 Blob 身份、资源 revision 和原始 Vec 容量，
以同一登记集合解析 Layout 的共享引用，保持现有字体选择与 fallback。
本次只验证这个依据，没有修改生产字体登记。

Blob ID 只承担共享引用身份，Blob 数据长度不代表 backing 容量。
消费者专门使用有多余容量的字体 Vec，证明按字节切片长度申报会低报；
同时验证 clone 保留同一 backing，最后一个布局释放后 backing 才释放。
生产接入还需覆盖共享字体控制块、登记 metadata 与责任域代次，
不能在本次快照总量上简单加一个固定“字体费用”。

模板及新行结果可以分别调用同一快照接口观察实际容量。
旧模板保持不变，不会因为 clone 的新行容量而原地增加申报。
clone、输出和旧发布资产的存储统计仍属于各自 owner，
不自动计入分析缓存留存，也不声称全进程内存限额。

## 操作前计费仍须实施

| 操作 | 已核对的源码事实 | 接入前需要完成 |
|---|---|---|
| 分析与 shaping | Parley 使用 LayoutContext、字体选择和 Harfrust；字体表和临时集合不属于 Layout 快照 | 在执行前取得有依据的扫描与集合上界；未知时返回成功缺失，不登记资产 |
| clone | Layout／LayoutData 使用派生 Clone；十个 Vec 重新分配，FontData 共享 backing；通用 Brush 的 Clone 可有额外行为 | 为默认 `usize` brush 提供独立复制计划与可拒绝分配依据，不用原模板容量代替复制容量 |
| 断行 | BreakLines 复用并清空原行 Vec，可回退扫描；commit／finish 会生成 line items；Bidi reorder 按 level 重复扫描 | 覆盖回退、空末行、行与临时容量及重复扫描；在对应工作或扩容前扣费 |
| 对齐 | 对行扫描，Justify／unjustify 还扫描行项和 cluster；该实现不增加 Layout 的数组容量 | 从当前有效布局规划元素访问次数，不能按一次常数调用计费 |
| 导出 | Core 生成 glyph、来源、几何和交互结果，并保留 Parley 布局 | 在生成前按输出责任划分工作与分配，避免和 Flow 已有费用重复 |

Harfrust 0.3.2 的 `max_ops` 在 GSUB／GPOS、重排及部分 AAT／kerning 路径扣减，
并非所有字体表读取、扫描及集合分配的统一计数器。
其默认值和增长因子不能直接作为完整 shaping 的 Core work 上界。
本次没有用字符数乘常数、事后耗时或 allocator 申请量填补这项缺口。

这份只读补丁本身不满足 B 的全部退出条件。依赖路线批准后，
仍须补齐上表的操作计划、同一字体集合的阶段适配和拒绝测试，
再按原授权推进 C／D。必要费用无法证明时，默认适配不声明可复用阶段能力。

### 继续核对操作计划的结果

生产依赖接入后，按 ADR-059 的操作前扣费合同继续检查了分析、断行和导出。
以下为源码检查结果，没有修改这些算法，也没有生成可准入的操作计划。

| 源码入口 | 本轮确认的工作 | 计划必须覆盖的部分 |
|---|---|---|
| `context.rs::analyze_text` | 扫描字符和样式，调用 Swash 分析；需要时调用 Bidi resolver | 多次字符扫描、分析器和 resolver 的内部工作及集合增长 |
| `builder.rs::build_into_layout` | 再按样式扫描字符、生成布局样式、稳定排序 inline，然后 shaping | 样式扫描、稳定排序的比较与临时分配，不能只按最终 inline 数收费 |
| `shape/mod.rs::shape_item` | 创建或命中 ShapePlan、准备 UnicodeBuffer、调用 `shape_with_plan`、导出 shaped run | 冷缓存 ShapePlan、字体表处理、buffer 增长和 shaping 内核；buffer 初始 reserve 不是整个操作的分配上界 |
| `line_break.rs::break_next` | 扫描 item 和 cluster；换行可恢复到之前的 boundary | 重复扫描及 ligature 连续 cluster；不能假定每个 cluster 只访问一次 |
| `line_break.rs::finish_line` | 重算行项、空白和 cluster advance，按 Bidi level 重排；空末行可能追加 line item | 行项和 cluster 的重复访问、重排和空末行新增容量 |
| `line_break.rs::Drop` | 遍历生成的行，计算整体尺寸并把行集合换回 Layout | 退出时仍会发生的扫描；若增加可失败的断行入口，须在进入前覆盖退出工作，不能在预算失败后隐式做未准入工作 |
| `render/text.rs::layout_styled_with_diagnostics` | 对每个绘制片段重新扫描所属 run 的 visual clusters，再生成 glyph、来源、镜像及交互结果 | 片段与 run 的嵌套扫描、临时 HashMap、字体解析的字节比较和输出集合；不能只按最终 glyph 数收费 |

这些入口均位于本地已锁定的 Parley 0.7.0 或当前 Core。
原始依赖来源和校验值见上文；检查范围不包括整个 Harfrust、Swash 或 Fontique
内核的独立成本证明。因此本轮没有把任何未证明的 kernel 记作常数操作。

下一步的依赖操作补丁须先提供可审阅的计划及执行接口，
再把这些工作接到同一 `TextStageContext::charge`。
接口应明确计划绑定的输入／模板、各步新增容量、拒绝后已发生的工作，
以及退出扫描的收费时点；只读容量 API 继续承担事后存储观察。
在分析前已知无法取得 shaping 或后续 reflow 的必要依据时，应返回成功缺失，
不先 shaping 再登记一个无法履行 reflow 合同的模板。
当前仍没有默认 Parley 阶段能力，B 保持进行中，C／D 的前置条件未满足。

## 已执行验证与限制

| 验证 | 实际结果 |
|---|---|
| 私有容量与算术 | 5 项通过；覆盖十个数组、逻辑清空保留容量、零大小元素、溢出和缺字段安全读取 |
| Native 默认配置消费者 | 3 项通过，其中 120 组读取／断行／对齐对照 |
| Native portable 配置消费者 | 同样 3 项通过、120 组对照 |
| Wasm portable | `wasm32-unknown-unknown` 的 library／cdylib 构建通过，未运行 Wasm 测试 |
| 独立消费者 Clippy | `--lib -- -D warnings` 通过 |
| Parley 自身严格 Clippy | 原始版与候选均有相同 7 条上游错误；新模块未增加错误，未宣称该门禁通过 |
| 源码保持 | 从原始归档逐文件核对 Rust 源码与许可证；除模块接入和隔离测试 harness 外保持原样 |
| 上游全套测试 | 未运行；发布包缺少 `parley_dev`，定向 harness 只运行新增模块测试 |
| 文档与格式 | 492 份 Markdown 门禁、`git diff --check`、新增 Rust 文件格式及两份 Python 脚本语法检查通过 |

Native 与 Wasm 消费者锁定 `parley 0.7.0`、`fontique 0.7.0`、
`harfrust 0.3.2`、`skrifa 0.37.0`、`swash 0.2.6`，
对应生产文字依赖版本；完整消费者依赖见其 Cargo.lock。
工具链为 Rust 1.96.0，Native 为 aarch64 macOS。

120 组用例覆盖空文本、组合字符／连字／ZWJ、CJK／Arabic 混排、末尾换行、
inline、零宽／窄宽与四种 Parley 对齐，比较读取前后和控制路径。
这是存储 API 的只读行为检查。两条对照都使用同一补丁库，
不作为生产 Core 的公开来源、interaction 或资源 revision 精确等价证明。
本轮按实际循环更正旧报告的 160 组分母；原始日志与 receipt 保持历史字节。

私有测试的首次直接编译因发布包缺少 `parley_dev` 失败。
隔离 harness 给副本增加单独 workspace 和测试 feature，仅在定向测试中
排除上游总测试模块，新增模块测试保持启用。harness 改动没有进入依赖候选补丁。
严格 Clippy 的 7 条上游错误保留原始与候选 JSON 诊断，复核脚本逐项比较，
没有通过禁用 lint 或修改无关上游算法来取得通过结论。

## 本轮生产接入

上述只读存储补丁已进入 `dependencies/parley-0.7.0/` 依赖目录，
原始源码、许可证、来源和补丁均已保留；[维护说明](../../../dependencies/README.md)
记录 owner、批准和升级验证要求。该目录排除出一方 workspace。
生产 `workspace.dependencies.parley` 使用精确版本与本地 path，
独立存储消费者也切换到该生产路径，未采用仅在根生效的 registry override。

实际影响是 Cargo 依赖来源、锁文件及默认文字依赖的编译输入。
Novadraw 公共签名继续不暴露 Parley 存储类型，默认完整布局行为保持。
接入检查记录如下：

- Core `--lib` check 与 workspace check 通过。
- Core Wasm check、生产路径存储消费者 Native 测试及 portable Wasm 构建通过。
- 四个文字定向 target 合计 32 项测试通过，覆盖布局、绘制、共享 glyph 与阶段协议。
- release 性能驱动构建及 5 次原始 fixture 扩展 smoke 通过；
  纯滚动每次新增 layout 为 0，resize 每次仍调用 6 次 layout，
  原生 glyph 保持 51,233，未作统计提速结论。
  本地原始样本 `target/verification/text-stages-b-2026-11-02/production-perf-smoke.ndjson`
  及分母核对 `target/verification/text-stages-b-2026-11-02/production-perf-smoke-receipt.json` 保留。
- 后续 5 个会话的正式 CPU 采样完成：1,000 个同进程新建 Runtime 样本、
  200 个冷进程样本全部分母核对通过，失败为空。
  纯滚动新增布局仍为 0，resize 仍为 6；原始样本、环境及散列核对已保存。
  本轮电池供电且后台负载较高，仅记录当前单版本成本；
  具体分位数与限制见[验证指南](../performance/text-optimization-validation.md#历史单版本基线)。
  不作为 ADR-059 C／D 或默认启用证据。
- 从原始归档核对 50 个未修改文件；模块声明是唯一原始源码改动，
  新模块与已验证补丁完全一致，不带隔离测试副本的 harness 改动。
- 原始 `cargo fmt --all` 会扫描本地 path 依赖；增加仅作用于
  `dependencies/` 的上游格式保留配置后，该检查仍被并发 Editor 改动挡住。
  新增存储模块显式关闭该保留配置，单独 Rustfmt 检查通过。

生产传递依赖按接入前工作区 Cargo.lock 解析，Core 目前使用 Swash 0.2.10；
候选隔离验证锁定 Swash 0.2.6。两者的版本范围均来自未改动的上游声明，
Core 接入只新增本地 Parley 包身份，没有借机升级传递依赖。
本轮没有改字体注册，也没有决定默认缓存限额。

本次接入不会关闭 B 或允许默认宽度复用。
后续操作接口如需修改依赖算法，仍必须给出相应源码依据、补丁及复核结果。
当前复核为本 Agent 自审和工具验证，尚无独立评审。
已有优化的操作和采样入口见[文字优化验证指南](../performance/text-optimization-validation.md)。
