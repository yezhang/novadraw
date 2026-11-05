# Parley v0.12.0 存储 API 与预算边界再评估

类型：`architecture-review`

日期：2026-11-05。状态：源码分析与独立 Native API 探针完成；建议未实施。
能力归属：Core 文本适配、缓存与准备会话；第三方库负责其内部算法。
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。

本文补充[上游与补丁复核](parley-upstream-api-and-patch-necessity-2026-11-05.md)，
专门回答两个问题：v0.12.0 的公开 API 能否替代容量观察和操作前拒绝；
本项目是否值得为第二项维护深层依赖补丁。

## 结论

**没有找到能直接返回上游对象真实容量或提供端到端操作前拒绝的公开 API。**
但这不意味着必须修改上游才能完成有界缓存与性能复用：
公开 shaped 数组足以构造 Core 自有、可精确计费的快照；
公开逐行接口和输出回调也提供了比“每个内部操作扣费”更粗的控制边界。

**Core 的遍历、稳定化和缓存留存限额值得保留；
默认文字后端的逐操作可拒绝预算，目前缺少足够的产品收益依据。**
不建议再把它作为普通宽度复用的前置条件。
更合理的下一步是验证无补丁上游适配，并把预算约束放在 Core 能实际控制的
调用、迭代、输出和缓存边界。若确实需要限制任意字体下单次 shaping 的
算法执行，再单独论证该需求和实现方式。

这是对既有设计的重新评估，不是已经接受的合同变更。
现有阶段合同仍有效，不能在代码中悄悄省略要求；本轮不删除补丁或修改公开 API。

## 1. 固定 v0.12.0，不混用 main

分析基线为用户指定的 [v0.12.0](https://github.com/linebender/parley/tree/v0.12.0)。
读取了发布包的源码和导出声明：Parley 39 个、Engine 15 个、Fontique 21 个
Rust source 文件；API 检索后定向阅读实现。
在线 tag 的 `shape/shaper.rs` 与 `shape/shaped_text.rs`
均与前轮已校验的发布归档逐字节一致。

v0.12.0 的 ShapedText 有六个数组，不含 main 新增的 features 数组；
其 Shaper 使用旧式 UnicodeBuffer／GlyphBuffer 接口。
main 的 safe-to-concat 开关、inline object 扩展等不算入本轮可用 API。
来源 checksum 和前轮收据见[版本复核](parley-upstream-api-and-patch-necessity-2026-11-05.md#1-版本来源与核验范围)。

另外校验 Harfrust 0.12.0 原始归档 SHA-256：
`c03d949a14aa089bbb282f7dd76a498a7f684428e4257202efc119ec010376f9`。
用于分析的 lib、buffer、face 三个文件与归档一致。
内部 `pub` 方法必须结合 module 可见性判断，不能把私有 `hb` 模块中的字段当成公开 API。

## 2. 已有 API 能做到什么

| 候选 API | 能直接做到 | 不能从中推导 |
|---|---|---|
| Analysis 的 `char_info`／`bidi_levels` | 借用初始化数据、获得长度 | Vec capacity、完整 heap 用量 |
| ShapedText 的六个数组 getter | 读取 runs、characters、clusters、glyphs、fonts、coords | 原对象真实容量或整个 shaping 工作区占用 |
| `ShapedText::new`／`clear`／`Clone` | 构造、复用或复制结果 | 清空会释放容量；复制会得到可公开核对的容量证书 |
| `ShapedSlice::narrow`、atom／grapheme cursor | 借用子区间、控制调用者自己的遍历 | 新建零复制持久 owner；取消已经开始的 shaping |
| `Layout::styles`／`lines`／`inline_boxes` | 读取公开结果 | 私有 runs、line_items、style_metrics 等所有数组的容量 |
| `StyleRunBuilder::reserve` | 预留指定数量以减少增长 | 所有其他数组不会增长；预留失败是结构化预算拒绝 |
| `BreakLines::break_next`／state／revert | 逐行推进、调整几何、回退检查点 | 一次 break_next 有固定时间上界；析构不做任何工作 |
| `Selection::geometry_with` | 回调导出几何，避免额外结果 Vec | 回调返回后内部遍历自动停止；拥有完整 cancellation token |
| `FontSelector::select_font` 返回 None | 在字体选择处停止，产生部分结果 | 任意 shaping 步骤可中止；返回结构化完整失败 |
| Fontique 的 `QueryStatus::Stop` | 停止后续候选匹配 | 已执行的加载／匹配可撤销或提前统一计费 |
| Blob 的 `id`／`len`／`data`／`into_raw_parts` | 识别共享 backing、借用数据、取回 Arc handle | 任意 backing 的实际容量；从 erased `AsRef<[u8]>` 恢复 Vec capacity |
| Harfrust `UnicodeBuffer::reserve -> bool` | 请求 buffer 容纳指定字符数 | 所有分配可恢复失败；配置全局额度或观察真实 capacity |

源码入口：
[Analysis](https://github.com/linebender/parley/blob/v0.12.0/parley_engine/src/analysis.rs)、
[ShapedText](https://github.com/linebender/parley/blob/v0.12.0/parley_engine/src/shape/shaped_text.rs)、
[Layout](https://github.com/linebender/parley/blob/v0.12.0/parley/src/layout/layout.rs)、
[breaker](https://github.com/linebender/parley/blob/v0.12.0/parley/src/layout/line_break.rs)、
[selection](https://github.com/linebender/parley/blob/v0.12.0/parley/src/editing/selection.rs)、
[query](https://github.com/linebender/parley/blob/v0.12.0/fontique/src/collection/query.rs)。

### 2.1 容量观察：没有直接 getter，但可以改变计费对象

需要区分三个问题：

1. 读取已生成的文本数据：公开 API 已满足。
2. 精确统计仍被持有的上游 Analysis／ShapedText／Layout：私有 capacity 仍不可读。
3. 构造一份可精确计费的 Core 缓存：可以只使用公开 API。

第三项的方案是复制六个公开数组到 Core 自有 Vec 或 boxed slices，
保留必要字体 backing，并让缓存只持有这些已计费数据。
自有 Vec 可以读取 capacity；boxed slice 的逻辑数据存储可以按长度和元素大小计算。
容器、引用计数和共享 backing 分别计费，不能把浅层字节当作总内存。

本轮探针已完成六数组复制和独立生命周期验证。样本得到：

- Core 自有快照的浅层存储为 1,212 bytes；
- 原 ShapedText 清空后，快照内容和浅层字节保持；
- 字体 backing 数据长度为 879,708 bytes，单独列出，未计入 1,212 bytes。

879,708 是 Blob 数据长度，不是任意字体 backing 的容量证书。
本项目注册字体由 Core 持有，可以从注册 owner 提供真实计费信息，
不需要从 erased Blob 反推。多 face 或多资产引用同一 backing 时仍需去重。

这个办法只解决自有快照的留存。上游工作区、分析、复制时的瞬时峰值，
以及仍留在交互 provider 中的上游 Layout 不会自动消失。
当前高层 Layout 也没有从外部 ShapedText 构建的公开接口；
自有快照要参与重排，须承接相应布局适配或取得上游共享存储接口。
不建议仅为避免一个小型容量接口就默认重写完整断行和交互。

`size_of::<ShapedText>()` 只包含对象及 Vec headers；
`size_of_val(shaped.characters())` 只包含初始化元素。
二者都不能给出原 Vec 的备用容量。
按 glyph 数、字符数或 Clone 的当前实现猜容量，不能替代稳定的存储合同。

### 2.2 逐行与回调接口：已有可用控制点

v0.12.0 可以调用一次 `BreakLines::break_next` 后停止。
探针构建长段落、只排一行并 drop breaker，确认保留一行，
剩余文字没有被隐式排完。

但析构仍扫描已经生成的行，计算尺寸并交换输出数组。
一次 break_next 也可能扫描很长的 atom 序列；API 控制粒度是行，
不是每个比较、每个 glyph 或固定毫秒。
部分行布局不能被冒充成当前 Core 协议要求的完整段落结果。

`Selection::geometry_with` 可以让 Core 自行持有输出容器并检查容量，
避免先生成第三方 Vec 再复制。
该回调返回 `()`，拒绝继续保存输出不会自动停止上游遍历。
`FontSelector::None` 也只在选择边界起作用；
由其产生的部分 ShapedText 必须在发布前拒绝。

这些 API 足以支持粗粒度适配，不能组合成原严格逐操作合同的等价实现。
不能用 panic、跳过 Drop 或私有内存转型伪造可取消入口。

### 2.3 Harfrust 已有保护，但不是 Core 的通用预算

本轮核对的 Harfrust 0.12.0 内部有 max_len、max_ops 与 successful 状态。
shaping 的 enter 根据输入数量设置额度，部分替换、匹配和 buffer 操作消耗它。
这是算法保护，说明使用普通上游并非完全没有防护。

这些字段属于私有 `hb_buffer_t`，公开 UnicodeBuffer 没有
`set_max_ops`、`set_max_len` 或统一 caller budget API。
它们也没有覆盖所有冷计划、字体表、Bidi 与 Parley 输出操作。

尤其不能误读 `reserve -> bool`：此版本的 enlarge 超过 max_len 时返回 false，
否则对 info 和 pos 执行普通 `Vec::resize`。
它不是通过 `try_reserve` 返回所有 OOM 原始错误的通道。
Parley Engine 还在内部持有 buffer，外部不能直接替换其额度策略。

源码依据为 Harfrust 发布归档的 `src/hb/buffer.rs`：
ensure／enlarge、enter／leave、UnicodeBuffer::reserve，
以及 `src/lib.rs` 的私有 `mod hb` 与公开 reexports。

## 3. 为什么曾要求可拒绝预算

预算最初要解决的是准备会话累计成本：树遍历、路径查找、
测量、贡献回调、输出生成以及重试／稳定化不能无限累积。
多个消费者共用同一会话，失败保持粘滞，避免重入或回退重新获得额度。
这些是 Core 掌握执行顺序的地方，有明确控制价值。

阶段文字合同后来要求把预算继续向下传入第三方内部，
覆盖 analyze、clone、断行、alignment、冷 shaping 和导出。
这样理论上能在超出逻辑额度前停止所覆盖的操作，
防止“先耗掉资源再报告超限”。

但“这样可以提供更强保证”不等于“本项目需要这项保证”。
此前以已接受合同说明补丁必要性，只能证明实现没有履行原合同，
不能证明原合同的成本与收益合理。

### 3.1 必须区分的几种限额

| 要保护什么 | 合适的控制对象 | 逐操作逻辑预算能否直接保证 |
|---|---|---|
| Core 遍历与反复稳定化 | 节点访问、迭代轮次、回调次数 | 对主动检查的 Core 操作有效 |
| 分析缓存长期增长 | 自有容量、唯一 backing、活跃 lease 字节 | 应由留存账本解决，不依赖 shaping 每步扣费 |
| 单帧响应时间 | 工作安排、重复计算、可见工作、实际耗时 | 逻辑单位不能换算成固定毫秒保证 |
| 单次 shaping 异常执行 | 算法保护、可取消边界或必要的执行隔离 | 有限支持路径可保护；目前不是全链路保证 |
| 瞬时峰值／OOM | 真实分配、资源 ownership、隔离或可失败 allocation | 元素项数预算不等于 allocator／进程内存限额 |
| 不配合的扩展回调 | 执行隔离或系统调度边界 | 回调不检查就无法中断，外层计数不能沙箱化它 |

当前 `PreparationBudget` 默认限额为 10,000,000 work 与 2,000,000 allocation。
work 是逻辑操作量，allocation 是集合项数；都不是纳秒或字节。
小元素和大元素占同一项数，可以有不同的内存与 CPU 成本。
所有步骤有保守上界也只能约束定义的逻辑工作，不自动保证
16.67 ms、某个 RSS 或操作系统调度延迟。

## 4. 对当前项目的价值和代价

### 4.1 有价值的部分应保留

Core 的结构遍历、稳定化与重试限额能终止由 Core 自己驱动的累积工作。
缓存字节准入能控制实例留存，活跃 lease 不能因淘汰就免计费。
输入与输出检查、revision 绑定和错误 source 同样需要保留。
这些机制均不要求在 Harfrust 每次查表前调用预算回调。

对于单个不配合的回调或一段已经进入的 opaque shaping，
Core 前后检查不能抢占它。这是应写清的责任边界，
不能靠把内部计数器命名为预算就声称已解决。

### 4.2 深层合同目前没有证明值得作为默认门禁

实际代码显示：

- 默认 ParleyTextEngine 没有覆盖 `staged_layout()`，普通入口直接 build。
- `FlowWork::layout` 当前直接调用完整布局；现有 preparation 计费主要覆盖外围工作。
- 阶段调用器的 `full` 回退也调用普通完整入口，再检查会话失败；
  它并没有把预算传入 Parley 内部。

因此尚未获得整个 Runtime 所有文字调用的逐操作保障，
却已经把默认宽度复用的接入阻挡在更强的内部预算要求上。
可以为可选阶段提供更强保证，但不能据此声称它是基础缓存的必要条件。
这是需要重新裁决的设计不对称。

代价已有具体证据：前批 Fontique 的 100,000 次新 Query 对照，
普通本地入口中位 144.753 ms，仅执行 black_box 的准入回调为 182.022 ms；
报告记录相对原版的变化为 +26.31%。
它不是完整渲染 benchmark，也不是实际 Core 扣费成本，
但足以说明新增通道不是免费的性能优化。

实际 `TextStageContext::charge` 调用 consume 和 allocate；
成功时两条路径都访问共享 Mutex 计数器。
若在高频内部步骤重复调用，会增加同步与函数调用成本。
本轮未测量该开销，不能用上面的 Query 数据代替实际 Mutex 或整帧结论。
还要承担依赖升级、失败恢复、部分缓存提交及原始算法对照的长期维护。

### 4.3 对“真的有用吗”的判断

**有限的 Core 准备预算有用；完整逐操作依赖预算的收益尚不足以支持继续扩张，
不建议把它作为默认文本性能优化的必要条件。**

更强的算法限额只有在明确要求限制单次第三方调用、
并有代表性超限场景时，才值得单独投入。
当前证据主要证明补丁能按测试拒绝并恢复；
没有证明它能解决已经观察到的重复组装、重排与滚动延迟，
也没有证明正式交互场景获得净收益。
这与“全部预算无用”不同：应缩小保证范围，保留真正能发挥作用的边界。

## 5. 建议的边界调整

建议作为后续方案评审的方向：

1. 保留 Core 遍历、稳定化、重试、输入／输出检查与缓存留存限额。
2. 默认文字适配使用上游公开 API，把不具备内部取消接口的调用视为
   明确的第三方执行边界，不宣称其已被 Core 逐操作管控。
3. 缓存按实际 owner 计费：优先核对公开快照、自有数组或最小只读容量接口；
   不为精确留存统计自动引入整个 shaping 的操作预算。
4. 在已验证的段落、行或输出边界检查是否继续安排工作，
   同时优化失效范围和重复计算。边界检查不能中断正在执行的单个块，
   大段落的拆分必须保持 Bidi、连字和上下文语义。
5. 有明确的单次调用隔离需求时另设方案。普通线程 timeout 不能安全终止
   已进入的 Rust shaping；worker／进程隔离也有数据传输和生命周期代价，
   不作为本轮自动新增机制。
6. 先评审阶段协议的预算责任调整，再移除相应补丁／消费者和门禁。
   不能用跳过计费或普通回退绕过仍有效的合同。

本建议不承诺无补丁路径已完成生产迁移。
高层 Layout 的留存容量和交互 owner 仍需解决；
复制快照与最小上游接口之间，须以实际性能和维护成本比较。

### 5.1 高层不足时直接使用低层库

用户进一步询问能否直接使用 `parley_engine` 等低层库，
从而删减或完全移除不需要的补丁。
**可以作为替代实现路径，但不是把高层 Layout 换成 ShapedText 就完成接入。**
上游继续负责 Unicode 分析、字体服务和 shaping；
Core 接管断行后的状态、输出几何与交互，并满足现有公开合同。
这里没有理由重写 Harfrust 的字体表或 GSUB／GPOS 算法。

| 能力 | 可直接使用的上游 API | Core 仍需负责 |
|---|---|---|
| 字体注册、family 与 fallback | Fontique Collection／Query，Engine FontSelector | ResourceId／revision、选字策略和 backing 计费 |
| Unicode、方向、grapheme 与断行机会 | Analyzer／Analysis；Engine 内部已使用 Parley Data／ICU | 请求转换、段落身份、配置失效 |
| 字体 shaping 与指标 | Shaper、ShapeOptions、ShapedRun::font_metrics | 样式 item、错误／部分结果检查及字体身份映射 |
| 布局基础数据 | ShapedText 的六个数组、run_slice、atom／grapheme cursor | 自有冻结数据或共享 owner，留存计费 |
| 宽度重排 | Atom 的 advance、soft-wrap 与 safe-to-break 信息 | 选择断点、紧急换行、尾部空白、行指标与约束 |
| 对齐与 Bidi 行输出 | ShapedRun 的 bidi level、来源范围与字形数据 | 每行视觉顺序、Leading／Trailing／Left／Right／Center、mirrored 映射 |
| Inline | 低层可用于分段与 glyph 输出；v0.12.0 没有 main 的完整 inline object API | 原 TextInlineBox 的插入、测量、基线与位置；不能套用 main 签名 |
| caret、命中、selection 与移动 | Grapheme 的来源／advance、ShapedSlice 定位接口 | TextInteractionProvider 的完整实现，行索引、affinity、视觉／词／行移动 |
| 图形输出 | glyph id、偏移、advance、字体实例 | 转为现有 GlyphRun、StyledGlyphRun、TextInlinePlacement 与 Graphics 输入 |

低层 API 提供的是计算素材，而不是已经满足本项目几何合同的成品：
grapheme 的 advance 可供 caret 布局，但不同方向的 affinity 与跨行移动仍需实现。
上游对连字跨越的 grapheme 等分 advance，这个行为可以直接核对，
不能自动当作平台字体的真实 ligature caret 数据。
颜色和样式分段也需要保持 Core 请求中的范围与顺序。

可评估的所有权结构为：

```text
Core request、字体注册与 revision
  → 上游 Analysis／Shaper（工作区复用）
  → Core 冻结的 shaped 数据与来源
  → 各宽度的行记录、视觉 run、baseline 和几何
  → 当前 TextLayout／StyledTextLayout／TextInteractionProvider
```

这允许同一内容和字体的 shaped 数据由多份行布局共享。
行记录只引用受持有 owner 保护的索引范围；
不能把从 ShapedText 借出的 slice 留存到原对象已销毁之后。
若直接长期持有 `Arc<ShapedText>`，真实 capacity 缺口依旧存在。
若转换成 Core 自有存储，应在一次分析／shaping 后做一次转换，
不能每次宽度变化都复制整段；实际收益仍需匹配成本验证。

#### 完全移除补丁需要满足什么

| 路径 | 补丁删减空间 | 需要付出的代价 |
|---|---|---|
| 无补丁高层 Parley + Core 缓存适配 | 不需要深层逐操作补丁；若直接留存 Layout，容量问题需解决 | 迁移成本较低，可能仍需一个小型只读存储接口 |
| 无补丁 Engine + Core 自有布局／交互 | 基本功能不依赖旧 35 份补丁；自有存储可直接计费 | Core 需长期维护断行、行几何和交互适配 |
| 低层覆盖已支持路径，高层负责兼容输入 | 可先验证公开低层路径，避免一次重写所有场景 | 两条入口需共同遵守请求、字体 revision、完整性和缓存身份合同 |

完全退役旧补丁的可行条件是：不再依赖旧专用 API；
缓存与交互不再持有未覆盖的私有存储；
默认后端的内部逐操作预算合同经过明确修订；
公开文字语义、外部消费者与成本验证通过。
“使用低层 API”本身没有自动满足这些条件。

还有一个必须先验证的功能限制：Engine 的 Atom 是同时满足
grapheme 和 shaped-cluster 边界的最小单元，
拆开 atom 需要重新 shaping；部分 atom 边界也标记为非 safe-to-break。
官方 v0.12.0 文档明确正确的行重塑仍在开发。
所以只写 advance 累加和按 cluster 切片，不能承诺任意合法宽度、
连字内部紧急断行、连写文字断词等行为与当前高层完全等价。

合理的推进方式是先证明低层输出与现有合同对等。
需要重塑的边界，要有保持上下文的正确实现或经过验证的兼容路径；
不能通过少支持脚本、忽略 inline、丢弃交互或者悄悄改断行语义换取零补丁。
兼容路径应在能力判断时选择；真实错误或预算失败不能被静默吞成成功回退。
不同实现的选择也必须进入缓存依赖和 revision 规则。

**本项目当前更适合把低层方案作为正式候选，先比较“少量接口补充”与
“Core 自有布局”两条路线，而不是仅按补丁数量决定。**
评估重点是来源与交互正确性、每宽度的复制／扫描成本、留存字节、
以及未来上游升级成本。零补丁可以成为结果，不应成为牺牲这些合同的目标。
本轮没有实现新的低层后端或更改迁移授权。

新增源码依据：
[Atom／Grapheme 的边界与 advance](https://github.com/linebender/parley/blob/v0.12.0/parley_engine/src/shape/atom.rs)、
[ShapedCluster 的安全断点](https://github.com/linebender/parley/blob/v0.12.0/parley_engine/src/shape/data.rs)、
[Engine 文档中的行重塑限制](https://github.com/linebender/parley/blob/v0.12.0/parley_engine/src/lib.rs)、
[Core interaction 合同](../../../crates/novadraw/src/render/text.rs)、
[Core styled 输出合同](../../../crates/novadraw/src/text/styled.rs)。

## 6. 验证、证据与限制

独立消费者直接使用 registry 的无补丁 Parley／Engine 0.12.0。
新增验证包括六个公开数组的 boxed snapshot、源清空后独立存活，
以及长段落只排一行后 drop、不继续排剩余文本。
前轮的 150 次 rebreak／fresh build 对照与扁平样式输入验证再次通过。
这不代表已实现 Core 完整自有重排，未执行新性能采样、Wasm、GPU 或全量迁移。

临时产物位于 `/tmp/novadraw-parley-audit-20261105/`：
`v012-storage-budget-probe.log`、`v012-storage-budget-build.log`、
`v012-storage-budget-receipt.json` 与 `probe/`。
探针源码 SHA-256：
`6834ee57e8c373c24091d9bd80d50e8b9e3c60241ceae7fef7aaa1e0ac4eba4f`。
成稿经技术推理与中文表达自审，没有独立 reviewer 复核。

项目依据：
[budget 实现](../../../crates/novadraw/src/figure/preparation/budget.rs)、
[阶段 context 与完整回退](../../../crates/novadraw/src/text/staged.rs)、
[测量入口](../../../crates/novadraw/src/text/system.rs)、
[实际 Flow 调用](../../../crates/novadraw/src/figure/flow/snapshot.rs)、
[原阶段合同](../../design/architecture/staged-text-width-reuse.md)、
[准入 Query 成本](draw2d-text-followup-2026-11-05.md#第三项实施中的字体匹配子片)。
