# 文字分析准入：补齐下层依赖的拒绝通道

类型：`proposal`

日期：2026-11-02。状态：`accepted`，用户已批准实施。
能力归属：Core（`novadraw`）文字引擎及其生产依赖；
Editor、backend、platform 不新增能力。
`api_semantics`：`text.flow`、`text.interaction`。

本文补充 [ADR-059](../../adr/adr-059-staged-text-width-reuse.md) 的 B 切片。
已完成的复制、对齐和断行准入保持。下一步需要补齐 Fontique 与 Harfrust
内部操作的费用依据和拒绝通道；仅修改 Parley 外层不能覆盖这些操作。
本提案不改变 Core 的预算单位、阶段协议和默认启用条件。
用户已明确批准本提案限定的 Parley 0.7.0、Fontique 0.7.0 与
Harfrust 0.3.2 分析准入补丁。授权从 B1 开始分片实施；授权不等于验收通过，
也不扩大到其他传递依赖或默认启用。

## 当前卡在分析阶段，而非缓存查询

宽度复用需要先生成可留存的分析模板，再复制、重新断行和导出。
ADR-059 要求分析过程中的扫描和集合增长在发生前准入，
包括冷缓存与临时集合。当前普通 Parley 入口没有这样的拒绝通道。
因此阶段适配器不能先调用普通 `builder.build`，事后补报成本并登记模板。

锁定的 Harfrust `max_ops` 也不能替代这个合同。
它在部分 buffer 操作中递减，限制相应算法的执行；
`ShaperData::new`、`ShapePlan::new`、变体坐标增长和 buffer 扩容
并不统一经过该计数器。即使保留其原始算法保护，仍须另行完成 Core 会话准入。
本次源码核对没有证明任意字体下的整体成本上界，也没有产生新的延迟数据。

## 按执行顺序划分费用责任

以下入口在 2026-11-02 的锁定源码中核对。
Parley 路径以 `dependencies/parley-0.7.0/src/` 为起点；
其余路径以对应 registry package 的 `src/` 为起点。
依赖版本、归档散列和源码一致性见
[源码 receipt](../../verification/evidence/text-stages-b-2026-11-02/analysis-dependency-source.json)。

| 顺序 | 入口与已确认事实 | 所需依据或拒绝通道 |
|---|---|---|
| 1 | Core `layout_styled_with_diagnostics` 构造方向前缀、字体栈、span 栈和 builder 输入 | 字符串、family、span、inline 的访问及新增集合容量；同一会话计费 |
| 2 | Parley `context.rs::analyze_text` 增长 info；`bidi.rs::BidiResolver` 持有七个 Vec，并有排序与多轮扫描 | 每轮分析、Bidi 扫描和排序均有可追溯的保守上界；增长前 fallible 预留 |
| 3 | Parley `builder.rs::build_into_layout` 写入 style 索引、复制视觉样式、稳定排序 inline | style 区间访问、排序比较与临时容量，不能只收文本长度 |
| 4 | Fontique `collection/query.rs::set_families`／`set_fallbacks` 增长两个查询集合 | family 匹配、fallback 展开、属性变化重置及容量增长的准入 |
| 5 | Fontique `matches_with` 在调用候选字体回调前执行 family 查询与 `load_font`；后者进入 `FamilyInfo::match_index` | 查询内部的字体属性扫描及缓存访问也要覆盖；候选回调不是完整准入边界 |
| 6 | Parley `shape/mod.rs::shape_item` 进入 Swash parser 和逐 cluster 字体选择；三个 LRU 有线性查找及可变长度键复制 | parser、字体覆盖检查、LRU 比较、键复制和冷构造分别给出依据 |
| 7 | Harfrust `hb/face.rs` 冷构造 OT／AAT 缓存；变体坐标使用 SmallVec 并按 axis 数 resize | 字体表扫描、坐标转换和溢出到堆的容量准入 |
| 8 | Harfrust `hb/ot_shape_plan.rs::new` 先收集 features 再编译；`hb/ot_map.rs` 构造 feature、lookup、stage 集合 | 冷计划构造、排序、lookup 展开及所有临时集合的准入 |
| 9 | Harfrust `hb/buffer.rs::enlarge` 对 info／pos 两个 Vec 直接 resize；`hb/ot_shape.rs::shape_internal` 调用多个算法阶段 | buffer 初始化与增长、重扫、替换和定位须在各步前计费；原 `max_ops` 另行保留 |
| 10 | Parley `layout/data.rs::push_run` 导出 coordinates、fonts、runs、items、clusters、glyphs；`finish` 再调整 spacing | glyph 输入访问、字体去重和所有输出增长均须准入，不能只读最终长度 |
| 11 | Core 导出行指标、glyph、来源、inline 与交互，随后验证公开结果 | 按实际输出 owner 计费；包括临时 HashMap、request 副本、来源和交互集合 |

Core 当前使用 `system_fonts: false` 的 Fontique collection，
字体字节由既有注册流程提供。上述缺口在注册字体路径上已经存在，
不以系统字体 I/O 作为本轮阻塞理由。应用读取系统字体并注册属于既有资源路径，
本提案不重建字体服务。

Swash parser 和 `read-fonts` helper 位于必经调用链；
本轮只确认其源码版本，尚未完成逐操作上界证明。
不能根据它们的 API 外形断言零分配或常数成本。
Skrifa、资源 handle 和其他 helper 若在实际支持路径上执行，
也须纳入对应 owner 的证明；当前 receipt 不声称覆盖全部传递依赖。

## 建议保留预算合同，增加依赖准入入口

建议将新增补丁范围限定为 Parley 0.7.0、Fontique 0.7.0 和 Harfrust 0.3.2
的分析必经路径。先使用依赖消费者验证，覆盖完成后再接 Core。
版本、feature 和算法语义保持；不自动修改其他版本的 Fontique／Harfrust，
也不升级锁定的文字栈。

依赖入口接收泛型准入策略，成功返回完整候选，拒绝返回带原始 source 的错误。
策略只接受操作类别与 `work`／`allocation`，不接受 Novadraw 类型；
Core 适配器将费用映射到当前 `TextStageContext`。
普通入口继续使用无准入策略，并与准入入口共享算法内核。
这沿用已验证的断行方式，避免维护两套 shaping 算法。
具体 Rust 签名随首个补丁列入复核，不在本文承诺尚未实现的 API。

同一策略向下传递到冷缓存、字体查询、buffer、字体表与 glyph 导出。
不能只在顶层函数入口计一次常数费用，也不能只在 buffer 层计费。
小型只读 helper 可以使用源码证明的保守上界；
上界依赖输入长度、实际表项、轴数或查询集合时，必须明确依赖及其读取费用。
需要遍历才能获得计划时，遍历本身先准入；不能通过一次未计费的预执行获得计划。

集合增长先检查算术并扣费，再 fallibly 预留和写入。
费用覆盖潜在旧元素迁移、新容量、初始化、临时集合与重复扫描。
SmallVec 的 inline 分支和 spill 分支分别核对；
排序不能仅按元素数收费，必须覆盖所选算法的比较上界和临时容量。
不引入每字符固定倍数，也不把字节折算成旧 allocation 单位。

对于无法证明的 helper，先尝试在本责任域增加准入通道。
如果必须修改 Swash、read-fonts、Skrifa 或资源 handle 的生产源码，
停止扩大补丁范围，提供具体调用、未覆盖操作和单独补丁建议。
不能据本提案把整个传递依赖树自动纳入本地维护。

## 从拒绝到恢复

1. 在执行分析前确认所选路径具备完整费用依据。未知时返回 `Ok(None)`，
   不进入未知 shaping，也不登记资产。已完成的受准入检查保留费用。
2. 进入已支持路径后，每步准入失败立即传播；不转成成功缺失，
   不通过普通入口重做以绕过粘滞预算。
3. 只在冷缓存对象完整构造成功后提交该对象。
   失败不能留下可被下次查询使用的半成品；此前完整缓存允许保留，
   不承诺回滚所有内部性能状态。
4. buffer 的所有权转移必须有明确恢复路径。不能在 `mem::take` 后提前返回，
   让后续普通调用在 `unicode_buffer.unwrap()` 处 panic。
   恢复空可用 buffer 或保留仍有效的 buffer，具体方式在补丁中核对。
5. 失败释放临时候选，不执行隐式 finalize、完整 shaping 或未准入重扫。
   清理中必要的元素释放成本需由对应存储操作的计划覆盖，
   不把“候选已丢弃”解释为 O(1) 或零成本。
6. 只有分析成功、完整存储描述有效、后续任意合法宽度的 reflow 可履行合同，
   才通过既有 registry 事务产生 lease。
   Core 的返回绑定检查和复合错误优先级保持。

支持范围可以按可证明的算法路径逐步扩大。
未支持输入仍走已有完整路径，但必须在未知操作发生前决定；
不能以测试字体白名单或 warm cache 偶然存在作为任意输入的成本证明。
新字体、不同脚本或不同 feature 选择可能切换算法路径，应重新检查。

## 导出也需要覆盖重复访问

Parley `layout/line.rs::GlyphRunIter::next` 从视觉 cluster 序列开始，
通过 `skip(glyph_start)` 定位下一个样式片段。
`GlyphRun::glyphs` 也使用同样的前缀跳过。
Core 对每个片段先调用 `glyphs().count()`，随后扫描其 run 的全部
`visual_clusters()` 计算来源，再调用 `glyphs()` 导出。

因此同一个 run 中有多个颜色或样式片段时，导出会重复访问前缀和 cluster。
计费不能只收最终 glyph 数，单样式长文也不能代表多片段成本。
这是源码观察，尚未测得该重复访问的实际毫秒占比。

首片应先覆盖现有访问次数。若要改成共享视觉 cursor 或来源索引，
须同时保留 RTL 顺序、零 glyph cluster、跨片段 cluster 的来源交集、
方向前缀扣除以及 line-local run 身份。
优化前后的访问计数和精确输出一起验证；
不把增加只读 glyph 数量 getter 当作消除了底层 `skip`。

## 验收按 B 内部顺序推进

| 顺序 | 可审阅产物 | 退出条件 |
|---|---|---|
| B1 | Harfrust 准入补丁：先 buffer 与冷构造，再必经 shaping 路径 | 每处增长／重扫的依据及错误通道明确；未知子路径不成功返回；拒绝后的 buffer 可继续普通调用 |
| B2 | Fontique 查询准入与 Parley 分析编排补丁 | 候选回调之前的查询成本覆盖；Swash、字体表 helper 已证明或有明确拒绝；冷／热路径均受同一会话约束 |
| B3 | Parley glyph 导出与 Core 输出适配 | 所有来源、几何和交互集合先准入；多样式重复访问覆盖；同一输出不重复收取同一笔费用 |
| B4 | backing 描述与显式实例适配 | 字体 backing 责任域、模板容量、metadata 和旧 lease 计费完整；不默认开启；转入 C |

每片分别保存原始归档、可应用补丁、源码保持核对和定向验证，
不将本轮静态分析记为 B1 完成。

定向测试需要覆盖冷／热 font query 和三个 LRU、变量轴 spill、
feature／lookup 集合增长、多 glyph 替换、RTL／CJK／组合字符、
inline 稳定排序、空文本的假空格，以及多样式导出。
每个实际准入点逐一拒绝，检查后续操作停止、错误 source、预算粘滞、
无 lease／候选发布和下一次普通调用可用。

Native 与 portable 消费者使用原始版本对照全部可观察输出；
Wasm 至少通过构建，并分别说明是否执行测试。
普通路径与受准入路径需匹配采样首次、热命中和拒绝尾延迟；
这些小片数据不能代替 C 全输出等价或 D 默认启用决策。
既有断行的 640／960 组对照不覆盖这里新增的分析操作。

## 来源结论与实施授权

本轮核对 Fontique、Harfrust、Swash 和 read-fonts 的锁定归档散列，
其 registry 源码共 252 个 `src/` 文件与原始归档逐字节一致。
该结果只确认分析来源；没有执行新增 shaping 测试、实现补丁或采集性能。
成稿经过技术结构与中文表达自审，未取得独立评审结论。

用户在上述提案完成后回复“批准”，已批准三个版本的分析准入补丁范围，
以 B1 为实施入口。Core 的统一可选能力、预算单位和默认关闭保持；
其他传递依赖需要新增生产补丁时另列依据。
只读 Parley 存储补丁的既有批准继续有效。
执行状态仍只在 [P2 backlog](../../roadmap/p2-delta-backlog.md) 维护。
