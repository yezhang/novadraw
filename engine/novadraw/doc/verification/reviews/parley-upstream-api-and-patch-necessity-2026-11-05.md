# Parley 上游 API 与本地补丁必要性复核

类型：`architecture-review`

日期：2026-11-05。状态：源码复核与 Native API 验证完成；依赖迁移未执行。
能力归属：Core 文字引擎及直接依赖。Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。
用户进一步要求以 v0.12.0 重查容量接口和预算必要性；
最新[存储与预算再评估](parley-v012-storage-and-budget-reassessment-2026-11-05.md)
区分公开快照替代、逐行控制和应重新裁决的深层预算要求。

## 结论与此前判断的修正

**上游已经提供此前调查遗漏的重要接口，继续围绕 Parley 0.7.0 扩张补丁，
不应再作为默认路线。** Parley 0.12.0 的 `parley_engine` 公开分离文本分析、
shaping 和 shaped 数据；高层 `parley` 还提供扁平样式输入、显式段落方向与
既有布局重新断行。这些能力应先用于评估无补丁接入。

不过，公开数据访问没有同时解决所有合同。当前项目要求分析、复制、
断行、导出及内部临时集合在操作发生前接受可拒绝预算检查，并按真实容量
登记留存资产。检查过的 0.12.0 和固定 main 源码仍没有完整实现这两项协议。
因此“上游已具备核心功能”成立，“现在删掉全部补丁便能保持所有既有合同”
尚不成立。

复核建议是：停止新增旧版补丁，优先验证上游公开 API；将普通文本功能、
性能复用与严格预算隔离分别裁决。预算合同若保留，针对剩余缺口提出最小
上游接口或独立方案；若调整，明确修订合同后再退役对应补丁。
不能把已有补丁数量或已投入工作当作继续维护的理由。

此前判断需要具体收窄：

- “缺乏独立分析／shaping 接口”只适用于当前锁定的旧高层入口，
  不适用于已经发布的 `parley_engine 0.12.0`。
- “宽度变化必须修改依赖才能避免重新 shaping”不成立。
  0.7.0 的公开 Layout 本就支持重新断行；0.12.0 也支持。
  项目重复 shaping 的直接原因是适配器每次重新调用 builder，
  并把完成的 Layout 移入交互 provider，未保留可复用模板。
- “需要私有字段才能取得 shaped 字符、cluster、字体和 glyph”在新低层引擎中
  已不成立；“需要真实私有容量才能精确统计上游对象的留存字节”仍成立。
- 深层 Harfrust／Fontique 补丁服务于操作前预算拒绝，不是获得基本文本功能
  或宽度复用的必要前提。此前未充分核对上游新 API 就推进旧版补丁，路线评估不完整。

## 1. 版本、来源与核验范围

本轮分别阅读发布归档和当前 main，未混用两者签名。
发布源码通过 crates.io 给出的 SHA-256 核对；main 按提交读取，
没有仅凭 README 推断实际实现。

| 基线 | 身份与本轮用途 |
|---|---|
| 本项目 | `parley = "=0.7.0"`，path 指向 `dependencies/parley-0.7.0`；该包再引用本地 Fontique 0.7.0 与 Harfrust 0.3.2 |
| 已发布上游 | Parley／Parley Engine／Parley Data／Fontique 0.12.0，2026-10-09 发布；Engine 归档的 VCS 提交为 `6e1143dda817360dd3a2a64f9133c6e5dfabd881` |
| 当前 main | `a2cec63ad375f6e8dafb6e0145da34c066294d51`，最新提交时间为 2026-11-04；本轮下载入口及相关实现源码 |
| 运行验证 | 独立 Native 消费者，直接使用 registry 的无补丁 0.12.0；不引用本项目 `dependencies/` |

0.12.0 的 Engine 声明 Harfrust 0.12.0、Skrifa 0.44.0 与 ICU4X 2.3.0 系列。
独立消费者实际解析到 ICU4X 2.4.0，锁文件保存在本地产物中。
main 已使用 Harfrust 0.15.0、Skrifa 0.49.0 等不同版本。
不能把 main 的依赖或功能直接记为 0.12.0 已发布能力。
0.12.0 的 MSRV 为 Rust 1.88，本轮 Native 构建成功。[S1](#source-1)

完整发布包源码与选定 main 文件用于 API 和实现核对。
未完整下载 main 全仓，未执行上游全量测试、Wasm、GPU 或 Novadraw 迁移测试。
公开接口事实由官方文档、发布源文件和消费者交叉核对；
这些是同一上游的不同证据，不宣称三个独立外部来源。

## 2. 仓库里的库分别解决什么

| 库 | 实际职责 | 本项目适用性 |
|---|---|---|
| `parley` | 高层富文本样式、inline、断行、对齐、行几何和编辑／选择工具 | 保留现有行为时优先升级的入口 |
| `parley_engine` | 分析、itemization、调用者字体选择、shaping、字符／cluster／glyph 输出 | 适合需要自有断行与输出所有权的引擎适配；不是现成完整富文本布局替代品 |
| `parley_data` | Unicode 属性的紧凑静态查询表 | 可替代重复的属性数据查询；不提供 Layout 内存数据、行布局或预算统计 |
| `parlance` | 字体、语言、方向、OpenType tag、变体坐标等基础类型 | 内部适配可用；本项目公开签名仍需遵守第三方类型边界 |
| `fontique` | 字体注册／枚举、族匹配、fallback、字体资源加载 | 普通字体服务无需本地准入补丁即可工作 |
| `parley_emoji` | Emoji 数据与识别支持 | 随引擎使用，不能替代布局与字形资源服务 |
| `parley_dev`／测试与 benchmark 包 | 字体测试资产、测试辅助与性能开发工具 | 不能当作稳定生产 API |
| `parley_data_gen` | 生成 Unicode 数据表 | 不是运行时文本引擎 |
| `attributed_text` | 独立 attributed text 方向；固定 main README 仍含占位说明 | 不据名称宣称能替换本项目内容、revision 或 Flow 合同 |

`parley_data::Properties::get` 一次查询获得 script、general category、
grapheme break、Bidi class、emoji 等属性。
0.12.0 通过 `baked` feature 提供查询；main 改了字段与 feature 组织，
文档明确它当前面向 Parley 内部使用。
**Data 在这里指 Unicode 数据，不是可公开读取的 LayoutData。**[S2](#source-2)

## 3. 已经能直接采用的 API

### 3.1 独立分析与 shaping

0.12.0 的实际调用顺序为：

```rust
analyzer.analyze(text, &options, &mut analysis);
shaper.shape_text(text, &analysis, &style_indices, items, selector, &mut shaped);
```

`Analysis` 公开 `char_info()`、`bidi_levels()` 与 `paragraph_level()`。
分析不依赖字体；同一分析结果可以参与后续 shaping。
`Analyzer` 和输出缓冲区可重复使用，调用会覆盖旧结果并保留容量。
这提供阶段分离，**不提供按 request 自动查缓存，也不证明编辑增量分析**。
源文本、方向、断行配置等不变条件仍由本项目负责。[S3](#source-3)

`FontSelector` 允许外部消费者参与逐 cluster 字体选择。
因此项目有机会把注册字体的预计算信息与自己的字体选择策略接入，
不必修改 Parley 私有 FontSelector 才能控制选择。
其内部仍使用 Harfrust 冷缓存、buffer 和真实 shaping，
外部字体回调不自动覆盖这些操作的预算。[S4](#source-4)

### 3.2 ShapedText 与来源数据

0.12.0 公开六个只读数组：

| API | 可以取得的内容 |
|---|---|
| `runs()` | 字符与字节范围、字体索引、坐标范围、Bidi level、advance、字体指标 |
| `characters()` | 源字节起点、style index、grapheme／word／wrap 信息 |
| `shaped_clusters()` | 字符范围、字形信息、advance 与 cluster 标记 |
| `glyphs()` | 独立存储的 glyph 数组 |
| `fonts()` | 使用的 FontData、collection index 和 synthesis |
| `normalized_coords()` | 归一化变体坐标 |

`run_slice()` 返回借用的 `ShapedSlice`；
`narrow()` 可在 atom 边界缩小借用范围；
`atoms_start/end/from` 和 `graphemes_start` 可遍历可拆分单元；
`atom_at_char/text_byte` 提供来源定位。
这些足以建立本项目自有的来源索引和布局数据，不需要 unsafe 读取私有结构。
但从借用 slice 构造持久资产，仍需持有 owner 或复制。[S5](#source-5)

一个重要细节：单 glyph、零偏移 cluster 可以把 glyph 内联在
`ShapedCluster` 中。`ShapedText::glyphs()` 不是全部可绘制 glyph。
必须使用 `ShapedSlice::shaped_cluster_glyphs()`，或正确处理内联表示。
本轮消费者实际得到 **28 个可绘制 glyph、独立 glyph 数组长度 0**；
仅检查 `glyphs().is_empty()` 会误判没有文字。

### Shaper 中的存储：工作区、shaped 结果与行布局

用户进一步指定 `parley_engine/src/shape/shaper.rs` 后，本轮补充核对该文件
与 `ShapedText::push_run`、高层 `LayoutData` 的所有权关系。
在线 main 的 Shaper／ShapedText 内容与前述固定提交的本地收据一致。
本节是源码分析，没有新增 main 运行或性能验证。

**Shaper 确实生成了供布局使用的存储数据，结果由独立的 ShapedText 持有。**
高层 Parley 自己也使用同一对象作为断行输入；
不能把“Engine 不包含完整行布局器”理解成“Engine 没有布局基础数据”。

| 对象 | 保存什么 | 生命周期与用途 |
|---|---|---|
| `Shaper` | 字体数据／实例／计划的三个 LRU、Harfrust buffer、feature 工作区、当前 CharCluster | 多次 shaping 共用的可变工作区；不代表某一段落的冻结结果 |
| `ShapedText` | 字符、clusters、glyph、fonts、coords、runs；main 另保存 features | 调用者持有的段落结果，可以独立保留和只读共享，供后续布局使用 |
| 高层 `LayoutData` | ShapedText、样式与指标、inline、布局 items、lines、line items、对齐位置与尺寸 | 在 shaped 结果上增加断行、对齐和 inline 行几何 |

关键调用链可直接从源码核对：

1. `Shaper::shape_text(..., shaped_text: &mut ShapedText)` 接收外部输出对象，
   并先执行 `clear`／`reserve`。
2. 内部 `shape_run` 选择缓存的字体实例与 shaping plan，填充 Harfrust buffer。
3. `harfrust::shape` 生成 glyph info／position。
4. `shaped_text.push_run(...)` 将结果写入持久数组：记录来源与样式，
   计算字体指标，换算 glyph 坐标和 advance，保存字体及归一化坐标。
5. 高层 `LayoutData` 明确将 `shaped_text`、`runs`、`items` 标为
   “Output of shaping (input to line breaking)”；
   `lines` 与 `line_items` 则单独标为断行输出。

来源：
[Shaper 工作区与输出签名](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/parley_engine/src/shape/shaper.rs#L85-L162)；
[shaping 后写入结果](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/parley_engine/src/shape/shaper.rs#L369-L390)；
[ShapedText 数组](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/parley_engine/src/shape/shaped_text.rs#L148-L174)；
[高层 shaping 与断行存储分工](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/parley/src/layout/data.rs#L201-L228)。

对本项目，这支持一个值得验证的方案：冻结后共享 ShapedText，
每个宽度的行布局只持有来源／cluster 范围和行几何。
这样不必为了取得 glyph 与来源而把全部 shaped 数组复制一遍。
但当前公开高层 Layout 没有从外部 ShapedText 构建布局的入口，
直接采用该所有权方式意味着由 Core 承接相应断行、对齐和交互适配，
或向上游请求共享 shaped 存储的高层接口；本轮没有实现这一方案。

ShapedText 不保存完整源字符串，来源范围仍需与原始文本和 revision 绑定。
glyph 的 x／y 表示 shaping 偏移，advance 用于排版推进，
它们不是已经完成断行与对齐后的画布绝对坐标。
再次调用 `shape_text` 会覆盖输出；发布后应冻结其修改权限，
包括 main 的 `set_inline_object_advance`。

main 还可选择生成 safe-to-concat 标志，提供局部重塑的安全边界依据；
文档同时说明生成标志可能增加成本，某些字体的安全区间仍会扩展到整行。
`safe_to_break` 与安全重新拼接的保证不同，不能把文档中的
“almost always gives the same result”当作严格输出等价。
这些标志并不构成现成的增量编辑缓存。

就补丁范围而言，应分别判断：
取得布局基础数据已有公开接口；取得真实保留容量仍缺接口；
要求每个内部操作在发生前接受预算拒绝，也仍缺通道。
这三项不能合并成一个“缺少布局存储 API”的判断。

### 3.3 高层布局的直接复用

`Layout::break_all_lines(width)` 与 `Layout::align(...)` 是公开 API，
可在同一已 shaping 的 Layout 上按新宽度重做断行／对齐。
它们不重新进入 builder 的分析和字体 shaping。
普通 `Layout::clone()` 保留独立可变数组，可用于冻结交互布局；
复制成本与留存容量需另行评估。[S6](#source-6)

本项目 `layout_styled_with_diagnostics` 目前每次执行
字体栈准备 → builder → `build` → 断行 → 导出，
最后把 Layout 移入 `ParleyTextInteraction`。
因此先解决 Core 的模板与交互所有权，便能评估公开重排接口的收益，
不能从项目当前重复 build 推导出上游没有重排能力。

0.12.0 的 `LayoutContext::style_run_builder` 还接受完整样式表和
连续、非重叠的 style runs，直接跳过内部范围分裂；
`reserve(styles, runs)` 可提前预留。
本项目已有结构化 Flow 样式输入，值得先核对能否直接提供这一形式。
仍需保持 span 覆盖／重叠语义，不能只按来源列表顺序填入。[S7](#source-7)

### 3.4 显式方向与新上游优化

0.12.0 的各 builder 已有 `set_base_direction`；
Engine 的 `AnalysisOptions::base_direction` 也支持 Auto／Ltr／Rtl。
本项目现在用私有方向字符前缀设置段落上下文，升级后应评估删除这一适配，
同时复验所有 source offset、caret、selection 与 mirrored 映射。

0.12.0 的 glyph 迭代已按 shaped cluster 范围读取，
`GlyphRunIter` 保存可恢复的 atom cursor。
旧版多样式片段反复跳过 glyph 前缀的观察不能原样套到新版；
Core 当前逐片段扫描来源的重复访问仍需独立整改。

main 新增 safe-to-concat 标志开关、shaped feature 数据、
inline object 支持和可注入的 AnalysisDataSources 等变化，
并刚删除空文本假空格处理。
0.12.0 消费者没有测试这些 main 能力，不能把它们算作迁移已完成。

## 4. 仍没有满足的项目合同

| 项目需要 | 上游现有能力 | 未解决的部分 |
|---|---|---|
| 读字符、cluster、glyph、font、来源 | Engine 公开数组与 slice | 基本数据访问已解决；Core 数据转换仍需实现 |
| 宽度变化避免重新 shaping | 高层 Layout 重新断行 | 项目模板、缓存身份及交互所有权没有自动接好 |
| 字体指标 | `ShapedRun::font_metrics`、`FontMetrics::from_font_instance` | 后者仍会解析字体和变量轴，不能记为零成本 |
| 完整 inline、对齐、hit-test／selection | 高层 Parley 已提供相关体系 | 低层 Engine 不提供完整替代；自有布局需承接这些责任 |
| 任意断点的正确局部 reshaping | 官方声明仍在开发 | 连字内部、连写文字断词不能随意按字节切开；只能遵守已有 atom 等边界 |
| 保留上游对象并精确统计容量 | 有数组 slice 长度 | Analysis／ShapedText／Layout 的内部 Vec 容量仍不可读，slice len 不等于 capacity |
| 操作发生前拒绝 work／allocation | 可以在外部阶段入口和字体回调检查 | 内部 reserve／resize、Bidi、冷缓存、shaping 和导出没有统一可拒绝通道 |
| 结构化原始错误与中断恢复 | 字体 selector 可返回 None | `shape_text` 返回 `()`、允许部分结果；无端到端错误合同 |
| Core typed registry、lease、revision 绑定 | 上游不定义这些项目机制 | 继续由 Core 提供，不能用第三方对象替代 |

### 4.1 数据长度不能代替容量证书

`Analysis` 包含私有 info／levels Vec；
`ShapedText` 包含六个私有 Vec，`reserve` 也是内部接口；
高层 Layout 另包含样式、style metrics、inline、runs、lines／line items 等数据。
公开 `&[T]` 可以给出初始化长度，不能给出实际保留容量。
`clear()` 恰好保留容量，说明长度为零时也可能持有大量存储。
`Clone` 也没有公开承诺可用于精确留存统计的容量公式。

若 Core 只留存自己复制出的数组，使用自己可核对的容量或 boxed slice，
可避免观察上游对象的私有容量。这个方案适合自有 shaped snapshot／布局；
若仍把上游 Layout 长期放入交互 provider，就没有消除其留存责任。
复制不是免费操作，还需验证吞吐与字节成本。

### 4.2 公开阶段不等于可拒绝的内部操作

0.12.0 的 Analyzer 先 clear，再进入普通分析；
分析中调用 `info.reserve(text.len())`、构造边界位置数组并解析 Bidi。
Shaper 调用 `shaped_text.reserve`，普通 LRU 冷构造，
`buffer.reserve/add`，再进入 Harfrust shaping。
ShapedText 的输出增长与 Skrifa metrics 也发生在普通函数内部。
固定 main 的相应入口依然如此。

外层一次预算检查不能证明这些操作均已在发生前计费。
在调用后观察结果，也不能撤销已经发生的分配／shaping。
这解释了旧补丁为什么存在，但不证明深改三个依赖是最合适的产品路线。
可以重新选择预算责任边界；选择变化必须写入合同，
不能在实现中把硬拒绝悄悄换成事后统计。[S8](#source-8)

字体回调返回 None 时还须检查 `characters().len()` 与输入字符数、
来源覆盖和字体有效性，拒绝不完整结果。
一个回调只能中止字体选择，不是可在任意 shaping 操作点取消的 token。

## 5. dependencies 中 35 份补丁的处理判断

清点结果：Parley 4 份、Fontique 1 份、Harfrust 30 份。
这些是递进补丁文件的数量，不等于 35 项性能优化。
部分后续补丁已包含前面的模块，回放关系需按 README 核对。

| 补丁／组 | 原目的 | 上游替代与删除条件 |
|---|---|---|
| Parley `storage` | 私有数组容量、共享字体观察 | Engine 的字体和数据读取可替代部分接口；真实 capacity 未替代。自有留存数据或上游容量接口接好后退役 |
| Parley `operations` | 复制／对齐前拒绝与 fallible 存储 | 普通 clone／align 已有；可拒绝合同没有。放弃或替换该合同后可退役 |
| Parley `line-breaking` | 断行步骤与增长前拒绝 | 普通重排已支持；无需它取得重排功能。严格预算若保留，仍需对应通道 |
| Parley `analysis-frontend` | 初始化、样式解析、边界分析、查询前拒绝 | Engine 与 StyleRunBuilder 替代部分私有编排需要；内部拒绝尚未替代。应暂停扩张，先评估新 API |
| Fontique `matching` | family 匹配、查询及临时增长前拒绝 | 普通 Fontique 能提供功能；外部 selector 可改变查询责任域。删除前重新裁决内部查询预算 |
| Harfrust `buffer`／`instance`／`variation` | buffer、轴坐标增长前拒绝 | 上游 Engine 会执行普通构造；没有自动替代这些准入保证 |
| Harfrust `cold`／`lookup`／`aat-headers`／`configuration` | 冷字体缓存、lookup、表头与配置准入 | 新引擎仍有冷缓存；不能由公开 shaped 数组替代 |
| Harfrust `plan`／`plan-layout`／`private-language`／`general-language`／`script-callbacks` | shaping 计划、语言和脚本规划准入 | 新版仍要生成计划；普通功能无需本地补丁，严格准入另议 |
| Harfrust `shape`／`single`／`multiple`／`ligature`／`context`／`context-spill`／`gpos-single`／`gpos-pair` | 必经 shaping、GSUB／GPOS 与临时匹配存储准入 | 新 Engine 调用 Harfrust；返回公开结果不意味着过程可拒绝 |
| Harfrust `rtl`／`simple-unicode`／`gdef`／`general-scalar`／`normalization`／`space`／`decomposition-miss` | 扩大受准入路径的文字／字体语义范围 | 不是引擎功能补齐；是预算支持范围扩大。无需为使用普通上游功能继续扩张 |
| Harfrust `normalizer-kernel`／`dotted-circle-kernel`／`grapheme-kernel` | 共享规范化、输出同步与分组内核准备 | 不能由新 API 直接替换其严格过程合同；停止扩张并重新评估必要性 |

**普通文本排版和宽度重排，都不以这 35 份补丁为功能前提。**
但这是普通上游功能层的判断，不是它们已被上游逐一合并，
也不是当前仓库已经能直接删除补丁。

本轮搜索 `crates/` 与独立消费者：准入／容量专用 API 的显式调用在测试、
依赖内部和消费者中；Core 默认 `ParleyTextEngine` 没有覆盖
`staged_layout()`，继续使用普通 builder 路径。
因此这套新增准入能力尚未成为默认生产路径实际使用的阶段服务。
删除它们主要需要处理第三方源码／manifest、契约测试和计划状态，
而不是拆除已经上线的默认分析缓存。
不过普通路径已与补丁共享部分内核，不能只删 `.patch` 文件，
让 path 目录仍运行修改后的源码却失去来源记录。

若最终决定全部退役，应一并切换依赖声明／锁文件，
移除旧 path 接入并明确退出尚未兑现的预算能力；
替换专用验证入口、更新依赖来源与 LT-04 状态，保持普通语义回归。
已提交的历史证据保留，其存在不要求生产继续保留 fork。

## 6. 无补丁消费者验证

使用真实 Inter Variable、Noto Sans SC、Noto Sans Arabic；
关闭系统字体发现，显式注册并提供字体族列表。
测试没有复制或修改上游源码，不使用 test-font identity 选择生产算法。

| 验证 | 结果与边界 |
|---|---|
| 低层 analyze → shape | 成功；29 字符、28 clusters，公开数据可读，shaping 后分析数组保持 |
| 完整 glyph 读取 | 28 个 glyph 可绘制，独立 glyph 数组为 0；内联 cluster 路径验证通过 |
| 字体 selector 拒绝 | 第四次回调返回 None，调用返回 `()`，结果为不完整的零字符；证明必须检查完整性 |
| 显式 RTL | `123 / 456` 的 paragraph level 为 1，无需私有前缀 |
| 宽度与对齐复用 | 5 段输入 × 2 种 inline 配置 × 5 次宽度 × 3 种对齐，共 150 次，与 fresh build 签名一致 |
| 冻结布局 | 每次保留 clone，再重排原布局；冻结副本签名未改变 |
| 扁平样式输入 | 两段连续 style runs 与 ranged 输入的布局／glyph 签名一致 |

签名覆盖尺寸、行指标和来源范围、run 方向／字号／坐标、
brush、glyph id／位置／advance 和 inline 输出。
未覆盖所有 caret／selection、字体资源身份、像素、全部脚本或全项目验收。
未采性能数据，不用正确性探针宣称毫秒收益。
成稿经技术结构与中文表达自审；没有独立 reviewer 复核。

本地源码、锁文件、运行日志和收据：
`/tmp/novadraw-parley-audit-20261105/`。
这些是可重建的临时产物；发布归档 checksum 与提交永久记录在本文及来源链接。
复跑入口：

```sh
cargo run --offline --manifest-path /tmp/novadraw-parley-audit-20261105/probe/Cargo.toml
```

## 7. 建议的后续顺序

1. 保持旧依赖和现有默认行为作为回归基线，停止继续补齐旧版深层准入。
2. 优先做无补丁 Parley 0.12.0 的高层适配验证：显式方向、扁平样式输入、
   字体与 inline API、交互几何、公开 Layout 重排及冻结所有权。
3. 将低层 Engine 用作受控候选：验证自有 shaped snapshot 的数据完整性和
   留存计费，估算自有断行、对齐和交互维护成本。不要因 API 更低层就默认重写全部布局。
4. 单独裁决严格预算：保留时列出新上游仍缺的最小接口和责任域；
   调整时修订阶段合同、拒绝语义与验收，不能把事后计数包装成原合同等价。
5. 在无补丁路径通过功能回归和匹配拖拽／resize 成本后，
   按用途退役旧补丁、专用消费者与接入配置。迁移版本先选已发布版本，
   main 新增特性另列必要性与语义差异。

本轮只完成复核，不修改 Rust 生产代码、不升级依赖、不删除补丁，
也不关闭默认分析／重排或预算验收。

## 来源

<a id="source-1"></a>
**S1 — 发布与版本。**
[官方 CHANGELOG](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/CHANGELOG.md)；
[Engine 发布 metadata](https://crates.io/api/v1/crates/parley_engine/0.12.0)。
发布归档 SHA-256：
Parley `61bd0c0c1737f0b3d3726dc06b225083af213b61913e4fbae1670613a106c6ce`；
Engine `b74c0d373e462e504ad4436710a39f099e18e848e1ee711f70cd2dcd5a69b290`；
Data `c43b63f808f6cc8f4a3b07de0d62a977bddd2e0bb5c232bd9e93ba8ef92e5e4b`；
Fontique `bbf1df16263783918b7da396077521f92b729ef7c429444dfcf03581f1448b77`。

<a id="source-2"></a>
**S2 — Unicode 数据库。**
[0.12.0 Data 源码](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_data/src/lib.rs)；
[固定 main Data 文档与实现](https://github.com/linebender/parley/blob/a2cec63ad375f6e8dafb6e0145da34c066294d51/parley_data/src/lib.rs)。

<a id="source-3"></a>
**S3 — 分析接口。**
[Engine Analyzer](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_engine/src/analyzer.rs)；
[Analysis 与数据源](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_engine/src/analysis.rs)。

<a id="source-4"></a>
**S4 — 外部字体选择与 shaping。**
[0.12.0 Shaper](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_engine/src/shape/shaper.rs)；
[公开 Engine 文档](https://docs.rs/parley_engine/0.12.0/parley_engine/)。

<a id="source-5"></a>
**S5 — ShapedText 与 atom／grapheme。**
[ShapedText](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_engine/src/shape/shaped_text.rs)；
[ShapedSlice／Atom](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley_engine/src/shape/atom.rs)。

<a id="source-6"></a>
**S6 — 高层重排与输出。**
[0.12.0 Layout](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley/src/layout/layout.rs)；
[glyph 迭代](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley/src/layout/line.rs)。

<a id="source-7"></a>
**S7 — 样式与方向输入。**
[LayoutContext](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley/src/context.rs)；
[Ranged／StyleRun／Tree builder](https://github.com/linebender/parley/blob/6e1143dda817360dd3a2a64f9133c6e5dfabd881/parley/src/builder.rs)。

<a id="source-8"></a>
**S8 — 项目合同与实际接入。**
[阶段文字协议](../../design/architecture/staged-text-width-reuse.md)；
[分析准入方案](../../design/architecture/text-analysis-admission-proposal.md)；
[生产依赖补丁](../../../dependencies/README.md)；
[Core 文本适配器](../../../crates/novadraw/src/render/text.rs)。
