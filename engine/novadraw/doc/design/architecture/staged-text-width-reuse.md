# 分阶段文字引擎与宽度复用协议

类型：`normative-design`

能力归属：Core 阶段协议/资产账本；默认文字引擎提供分析与重排，Flow 负责接入。
可选能力与完整回退选择见 [ADR-059](../../adr/adr-059-staged-text-width-reuse.md)。

日期：2026-11-02。修订：2。状态：`accepted`，用户已接受。
本页只维护已接受合同。评审结论、依赖补丁范围和最新验证见
[文字阶段验证状态](../../verification/reviews/text-stages-status.md)；
当前执行顺序见[P2 backlog](../../roadmap/p2-delta-backlog.md#当前推进顺序)。
A 已验证，B1 扩张已暂停，C／D 和默认启用门禁尚未完成；
暂停实施不改变本决策的设计效力。

能力归属：Core（`novadraw`）Text 引擎、实例资产登记及 Flow 调用。
Runtime 使用既有准备预算和候选发布；backend、platform、Editor 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`；保持原有精确语义，不增加编辑功能。

## 要解决的问题

ADR-051 的完整请求缓存正确地把宽度保留在 key 中。
因此内容未变、宽度改变时，完整引擎仍重新分析、选择字体和 shaping。
[隔离实验](../../verification/reviews/text-width-reuse-feasibility-2026-11-02.md)
证明 Parley 0.7.0 在指定长文上复制模板、重新断行及导出具有明显收益；
它没有验证 Core 全输出、真实 inline 反馈或生产预算。

本决策选择增加统一可选阶段合同，先复用同内容的分析资产。
既有完整布局入口和完整 request 缓存继续有效。
局部编辑、paint-only 失效、inline 度量分离、worker、原生 glyph 裁剪
不进入本次实现范围；它们仍由[增量提案](long-text-incremental-layout-proposal.md)
及所属后续决策负责。

## 当前接入事实

初稿按目标契约定向核对源码；修订 1／2 依据评审补齐合同，未重新扫描生产 Rust：

| 入口 | 已有行为 | 本次所需增量 |
|---|---|---|
| `TextLayoutEngine` | 完整布局、可选精确缓存身份、资源 revision | 统一可选阶段借用 |
| `MeasureContext` | 借用一个引擎，直接委托完整布局 | Flow 可携同一会话预算进入阶段路径 |
| `FlowWork::layout` | 先查完整请求缓存，未命中则调用引擎 | 在完整缓存未命中时尝试分析资产 |
| `ParleyTextEngine` | 私有 FontContext／LayoutContext，完整导出后移交交互 provider | 同一实例完成分析及 reflow，不建立第二份字体集合 |
| `OutlineTextEngine` | 完整请求与轮廓 provider revision 共同决定最终输出 | 首片保留完整入口；透传阶段能力须另证最终轮廓依赖 |
| `PreparationBudgetHandle` | 累计 work／allocation 单位，失败后保持失败 | 保留原单位；另设分析资产字节准入，不混用 |

源码入口：
[引擎](../../../crates/novadraw/src/render/text.rs)、
[测量借用](../../../crates/novadraw/src/text/system.rs)、
[Flow 服务](../../../crates/novadraw/src/figure/flow/snapshot.rs)、
[预算](../../../crates/novadraw/src/figure/preparation/budget.rs)。
对标依据为既有
[Draw2D 文本 Flow 和交互账本](../../parity/draw2d/api-coverage.md)；
本次优化不改变 TextFlow 换行、片段来源、caret 和 selection 合同。

## 输入分离只做一件事

新增 `text::shaping::TextAnalysisRequest` 是完整受检 `StyledTextRequest`
去掉 `constraints.max_width` 后的不可变投影。
构造只接受已经受检的 request，不提供自行拼接未经验证字段的入口。
文本、默认样式、span 范围与顺序、颜色、字号、字体描述符、
所有 inline 的身份／位置／尺寸、paragraph direction、layout direction、
alignment 和 wrap 均保留。
第一片不进一步判断某个字段只影响 paint 或行布局。

完整请求仍作为 reflow 参数；完整结果仍记录当前完整请求。
分析请求相等只允许省去分析，不能直接返回上一个宽度的 StyledTextLayout。
宽度 `None` 和零、受约束 NoWrap、alignment 与镜像继续使用既有精确行为。

引擎实例身份与 revision 另作为资产依赖。
文本或其他保留字段、字体／fallback 策略或引擎 revision 改变时，
旧模板不得复用。真实 inline 在 resize 中改变尺寸，也按新分析请求计算；
新增、移除或移动 inline 同样失效。
未来给 StyledTextRequest 增加影响输出的字段时，分析投影默认保留该字段，
删除依赖必须另有声明和验证，不把本合同解释成永久的字段白名单。

## 所有引擎使用同一个可选能力

公共领域为 `text::shaping`，不新增 root 重导出或第三方公开类型。
以下统一阶段签名已在 A 实现；默认 Parley 适配与 Flow 选择阶段路径仍待 B／C：

```rust
// TextLayoutEngine 的新增缺省方法：所有内置和外部引擎均可覆写。
fn staged_layout(&mut self) -> Option<&mut dyn StagedTextLayoutEngine> {
    None
}

trait StagedTextLayoutEngine {
    fn analysis_key(&self) -> ErasedTextAnalysisKey;
    fn analysis_registry(&self) -> &TextAnalysisRegistry;

    // 仅在分析缓存 miss 后查询；None 表示费用下界未知。
    fn analysis_admission_lower_bound(
        &self,
        _request: &TextAnalysisRequest,
        _context: &TextStageContext,
    ) -> Result<Option<TextAnalysisAdmissionCost>, TextStageError> {
        Ok(None)
    }

    fn analyze(
        &mut self,
        request: &TextAnalysisRequest,
        context: &TextStageContext,
    ) -> Result<Option<TextAnalysisLease>, TextStageError>;

    fn reflow(
        &mut self,
        analysis: &TextAnalysisLease,
        request: &StyledTextRequest,
        context: &TextStageContext,
    ) -> Result<StyledTextLayout, TextStageError>;
}
```

`None` 能力缺省走完整入口。`analyze` 的成功缺失仅表示本输入不支持阶段复用、
留存上界未知或缓存无法准入；它不是失败的字符串编码。
`TextStageContext` 提供同一会话的预算、诊断及 Core 封装的只读调用绑定，
不开放树 mutation 或字体注册，也不允许扩展自行构造或改写绑定。
独立 TextSystem 消费者可显式创建会话上下文；Flow 使用当前 scene 的上下文，
measure／prepare 重入及回退不得换成新预算。

拥有阶段能力的引擎仍须实现完整布局。两条路径必须对同一 request 精确等价，
包括公开结果、来源映射、交互和资源依赖。
不按 Parley／Large 类型分派，也不通过 downcast 引擎选择性能路径。
普通 `MeasureContext::layout_styled` 的行为保持；首片由受预算的 Flow 调用选择阶段。

### Core 捕获阶段调用绑定

一次阶段调用使用不可变 `TextStageBinding`，包含外层引擎的
`cache_identity`、`revision`，以及阶段 registry 的独立实例身份和 `analysis_key`。
registry 身份使用不可复用的实例／代次，不能用地址或诊断名称代替。
registry 创建者声明所属引擎身份；Core 检查它与当前外层引擎身份一致。
getter 只读这些身份，不执行改变绑定或结果的工作。

Core 的阶段调用器位于 MeasureContext／Flow 受预算入口。
它先从外层引擎取得身份与 revision，再借用阶段能力读取 registry 身份和 key，
形成绑定并捕获完整分析请求。没有能力或缓存身份时使用完整入口。
TextStageContext 的绑定字段由 Core 构造和封装，扩展不能自行填写或改写，
独立 TextSystem 也经过同一调用器；应用不直接拼接绑定调用阶段实现。

登记时，Core registry 入口检查 context 的绑定、所属引擎、registry、
typed key 与完整分析请求，才允许进入准入事务。
读取 lease 时，Core 检查其绑定与本次绑定一致，并完整比较分析请求；
生产者再通过相同 typed key 读取 payload，不能用生产者的自报身份代替 Core 检查。
旧 revision／旧请求的普通缓存条目是 miss；
扩展返回外国或不匹配的 lease 是协议错误，不当作普通 miss。

每次费用下界查询、analyze 或 reflow 返回后，Core 释放阶段借用，再重新读取外层身份／revision
并借用当前阶段能力读取 key 和 registry 身份，将四项与捕获绑定完整比较。
返回 `Ok(None)` 或 `Err` 时同样执行返回检查和会话失败检查；
绑定变更、能力消失或预算失败时拒绝本次结果，不完整回退。
analyze 的 `Ok(None)` 只有通过这些检查后才允许进入一次完整回退；
费用下界查询的 `Ok(None)` 通过检查后表示未知，继续按分析 miss 的规则处理。
候选最终发布前再次检查绑定，并完成既有资源与场景依赖验证。

registry 的登记和读取检查针对 Core 捕获的调用绑定，
不声称在持有扩展可变借用时能再次进入该引擎观察内部状态。
扩展不得在一次调用内变更绑定后恢复旧身份，或复用已退出的 registry 代次；
Core 的前后检查不构成对未暴露内部写入的独立监测。
发现任一字段变化记录结构化 `StageBindingChanged`；多种失败同时发生时按下节选主错误。
本次请求不发布、不写入完整结果缓存，也不无限重试。已经发生的工作继续计费。

### 返回检查与复合失败规则

Core 先保留回调原始返回值／错误，再执行安全的返回绑定和会话预算检查；
不得因回调先返回 Err 而跳过后续检查，也不得用最后一次检查的错误覆盖前一个错误。
有预算、绑定或回调错误时，不继续读取不匹配的 payload 或生成输出。
仅在这些检查通过且得到成功结果时，才执行对应的 lease／输出校验；
已经取得的 Core 校验错误同样保留。

由 Core 构造的 `TextStageFailure` 记录 `primary` 和独立的结构化字段：
`budget_failure`、`binding_failure`、`stage_error`、`validation_error`。
存在的失败全部保存，包括主错误自身的信息；缺失的字段明确为空。
预算失败依据同一会话 handle 的粘滞状态，不从扩展错误文本推断。
绑定信息保存捕获值、返回值与变化字段；原始 stage error 及其 source 按原对象保留。
这些字段属于返回错误，不依赖诊断开关或日志。

主错误按固定优先级选取，选择与检查的执行先后无关：

| 已发现的失败 | `primary` |
|---|---|
| 会话预算失败，可能同时有其他失败 | 会话预算失败 |
| 无预算失败，但绑定改变／阶段能力消失 | `StageBindingChanged` |
| 前两项均无失败，但回调返回 Err | 原始阶段／扩展错误 |
| 前三项均无失败，但 Core 的 lease／输出校验失败 | Core 校验错误 |

预算失败决定会话是否还能继续，绑定失败决定结果是否属于本次调用；
扩展执行错误和 Core 校验错误在前两项缺失时作为主错误。
不同选择不改变已发生事实，也不清除会话的粘滞失败。

`TextError` 保留该复合失败对象作为结构化 source。
`TextStageFailure::source()` 在存在原始 stage error 时返回它，
使 `TextError → TextStageFailure → 原始 stage error → 原始扩展 source` 可追溯；
主错误的分类由 `primary` 读取，不能根据 source 链的第一项猜测。
没有 stage error 时，source 返回所选主错误的原始错误对象。
预算和绑定等伴随失败通过对应字段访问，不要求多分支错误都压入一条 source 链。

| 同时发生的情形 | 主错误 | 必须保留的信息 |
|---|---|---|
| 扩展 Err ＋绑定变化 | `StageBindingChanged` | 原始扩展 Err／source、完整绑定变化 |
| 扩展 Err ＋会话预算失败 | 会话预算失败 | 原始扩展 Err／source、粘滞预算错误 |
| 三者同时发生 | 会话预算失败 | 原始扩展 Err／source、完整绑定变化、粘滞预算错误 |
| 返回 None ＋绑定变化／预算失败 | 按相同优先级选择 | 发生的绑定／预算失败；不得按成功缺失回退 |

任一失败都禁止本次完整回退、完整结果缓存写入和候选发布，
不因另一个错误优先而恢复预算或继续使用候选。
已经产生的临时候选按既有释放合同处理，仍不承诺回滚外部副作用。

### 资产身份、payload 与 registry

`TextAnalysisKey<A>` 以类型化身份登记 `A: TextAnalysisPayload`；
`ErasedTextAnalysisKey` 只用于异构索引，诊断名称不承担行为身份。
实例 `TextAnalysisRegistry` 的统一受检登记入口接收 key、不可变 payload、
完整分析请求、Core 捕获的阶段绑定和留存计费描述，成功返回 lease。
内置 Parley 和第三方 crate 都使用该入口，没有内置专用登记或读取通道。

registry 维护有界分析缓存；完整阶段绑定和完整分析请求一起定位资产。
长度或散列可缩小候选范围，最终命中必须完整比较输入。
生产者通过相同 typed key 受检读取 payload，错误 key、外国 registry、
过期 engine revision 和输入不一致分别拒绝，禁止 unchecked cast。
Core 不解释 Parley 的私有 Layout，也不登记闭合的内置资产 enum。
阶段引擎用 `analysis_key` 声明当前生产者的 key，Core 只在该 key 下查找，
不从异构 registry 随意选择一个 payload 再要求引擎解释。

lease 保留不可变 payload、绑定与计费凭证。缓存淘汰只释放缓存引用，
仍被 lease 持有的资产继续计入该实例的留存预算；注销不使旧已发布布局可变。
有效 lease 的 typed 读取不要求缓存索引仍含该条目，但仍检查当前绑定与输入。
revision 改变时旧 lease 不能生成新结果，已发布的旧交互／绘制 lease
继续遵守现有资源和 revision 校验。

### 登记事务与计费账本生命周期

Core registry 先检查计费描述、完整绑定和输入，计算本次独占存储、
新增唯一 backing、请求副本、索引、计费凭证与 backing 登记表的费用。
共享 backing 的身份包含责任域和代次；相同身份的存储描述必须一致。
登记不能因去重把未覆盖的引用或 metadata 隐去。

登记以以下顺序形成一个事务：

1. 计算受检费用；只在实际引用已经释放时扣减留存量。
2. 预留新增字节与条目额度，形成尚不可读的临时登记。
3. 完成必要的 fallible 存储预留，再一次提交条目、backing 引用计数和计费凭证。
4. 全部提交成功后才产生可用 lease；在此之前不返回可读 payload。

描述非法、算术溢出、容量不足或存储预留失败时，释放本次临时字节／条目预留，
撤销本次新增的 backing 记录与引用计数，不留下部分可命中条目或计费泄漏。
容量不足属于缓存未准入；非法描述、算术溢出和实际存储预留失败属于结构化错误，
不能都转成 `None`。已经发生的 work／allocation 单位不退回，
已完成的缓存淘汰和外部副作用不在回滚承诺内。

计费账本由 registry 和相关 lease 共同持有，不随 registry 的外壳销毁而消失。
淘汰条目或销毁 registry 只释放各自引用；最后一个钉住某 backing 的相关引用释放，
才扣减该 backing 的费用并释放登记记录。仍在占用容量的索引／登记表存储继续计费，
不因逻辑删除就声称它已释放；实际缩容或释放后再扣减。
因此条目已经淘汰，但旧 lease 仍可能让新资产无法准入。
旧 lease 对绑定已退出的 registry 不再用于新 reflow，其留存计费仍持续至释放。

## 正常调用和失败

Flow 先重读当前贡献、inline 测量和依赖，再形成完整 request。
完整请求缓存命中时使用 ADR-051 路径；未命中才进入以下阶段流程：

1. Core 捕获阶段绑定，检查实例启用策略。没有阶段能力／缓存身份，
   或实例显式关闭、配置总额度为零时，选择完整入口。
2. 在当前绑定和完整分析请求下查询有效分析资产，验证命中的 lease。
3. 命中则 reflow，不预判新增模板费用，也不重新预留已有模板的 backing；
   当前剩余留存额度为零仍可复用。reflow 的 work／allocation 仍需当前会话准入。
4. 只有 miss 时，才调用统一可选的费用下界查询。确认新资产无法准入时，
   转一次完整入口；下界未知或不能证明额度不足时，按合同进入 analyze。
5. analyze 成功登记后 reflow；成功缺失／分析后缓存未准入只有通过返回检查，
   才允许使用当前剩余会话预算完整回退一次。
6. reflow 输出经过现有 StyledTextLayout、来源、交互、字体和候选发布校验，
   再登记当前宽度的完整结果。每次回调返回和最终发布均检查完整阶段绑定。

“配置总额度为零”表示该实例关闭可选缓存；“剩余额度不足”只限制新增资产，
不能代替分析缓存查询。缓存已满但模板有效时，不调用 analyze 或完整布局。
已有资产不重复收取新增模板字节，不表示复制、断行和导出免除会话计费。

`analysis_admission_lower_bound` 是所有阶段引擎共用的可选协议，缺省返回 `Ok(None)`，
不能按 Parley 或其他具体类型分派。`None` 在这里只表示新增费用下界未知，
不能解释成“不支持”并跳过已有资产或直接拒绝 analyze。
`TextAnalysisAdmissionCost` 申报当前绑定与请求下必需新增的留存字节／条目下界，
包含必须新增的 metadata；共享 backing 按当前账本去重，不重复申报已经覆盖的存储。
生产者负责下界依据，Core 检查描述、绑定、输入和账本观测的有效性，
在使用时核对当前新增费用和剩余额度；观测过期时按未知处理，不据过期下界拒绝。
费用查询本身的 work 也计入当前会话，返回 None／Err 都执行相同的返回检查。
下界只能证明确定不足，不能代替登记事务或被当作完整留存上界。

| 条件 | 行为 |
|---|---|
| 没有阶段能力、未提供精确缓存身份 | 完整入口；不猜测引擎纯度 |
| 实例显式关闭或配置总额度为零 | 选择完整入口；不进入可选分析缓存 |
| 有效分析命中，包括当前剩余额度为零 | reflow；不查询新增模板费用，不调用 analyze／完整布局 |
| 分析 miss，新增费用下界确定超过当前额度 | 提前走一次完整入口；不调用 analyze |
| 分析 miss，新增费用下界未知或未证明不足 | analyze；成功登记后 reflow |
| analyze 后成功缺失或字节缓存未准入 | 释放未登记候选；返回绑定与预算检查通过后，在当前剩余预算内完整计算一次 |
| 查询发现旧 revision／旧输入 | 缓存 miss；不读取旧 payload |
| 会话预算耗尽／会话计费算术溢出 | 粘滞预算错误为主错误，按复合失败规则保留其他错误；不回退 |
| 外国 lease、错误 key、非法输出或阶段实现错误 | 结构化错误；不以完整回退掩盖协议错误 |
| 任一返回分支中引擎身份、revision、key、registry 改变 | 保存 `StageBindingChanged`；无预算失败时作为主错误，不完整回退 |
| reflow 无法提供必要的操作前计费计划 | `ReflowAccountingUnavailable`；在复制／断行前退出，不记零成本 |

`TextStageError` 区分绑定变化、身份、revision、输入、输出、计费描述、
操作计划缺失、存储预留、预算与扩展执行错误，
通过 `TextError` 保留结构化阶段 source，不压缩为 `InvalidMetric` 或字符串。
同时发生的预算、绑定、扩展和已取得的 Core 校验错误按复合失败规则返回，
不得只保留主错误而丢失伴随信息。
外部执行错误保留原始 `Error::source`；诊断默认关闭，热路径不打印日志。
新错误不承诺回滚外部引擎副作用或已经提交的源 mutation。

## 字节准入与 Parley 前置条件

现有 `PreparationBudget.allocation` 表示逻辑分配单位，不是字节，
不能把模板的 MB 数直接传入旧单位后宣称已实现内存上界。
新增分析留存额度单独记字节，使用受检加法；精确缓存仍按自己的旧合同管理，
本 ADR 不宣称整个 Runtime 的缓存或进程 RSS 已有全局上限。
registry 的 `TextAnalysisCacheLimits` 分别限制条目数和留存字节，
由实例创建者显式提供；零额度表示不保留分析缓存。
metadata 仍计字节，条目数限额不能代替字节限额。

### 生产者申报与 Core 可检查的范围

`TextAnalysisPayload` 的生产者负责提供完整、有效且在 lease 生命周期内成立的留存上界：
自有数组按容量与元素大小计费，包含容器及必要 metadata；
共享 backing 以身份登记，按本责任域的唯一 backing 计费。
钉住字体或来源的共享引用也需说明责任域和计费依据。
单纯按可见片段、glyph 数或固定“每字符若干字节”估算不能准入。
未知上界时可以完整计算，但不得长期留存阶段资产。

生产者负责证明所有自有数组、共享引用和内部缓存已被上界覆盖；
Core 不解释第三方 payload，不独立证明其实际内部存储。
registry 只检查描述结构、受检算术、共享 backing 身份一致性、费用去重和额度。
一次自报字节数或通过受检登记，都不能被报告为 Core 已验证任意第三方对象的实际内存。
默认适配的描述须有具体存储 API 与测试依据；外部生产者承担同样的合同义务。

内部可变性不得扩大分配容量或增加未被申报覆盖的 backing，也不得改变分析语义。
预留容量内、上界已经覆盖的内部缓存允许使用；
需要增长时创建新的 payload、身份和计费描述，重新准入，
旧 lease 的 payload 和费用仍保留，不能原地增长后补报字节。
隐瞒引用、复用 backing 身份或超出申报上界属于扩展合同违约，
不把结构校验通过解释成对这种行为的防护证明。

### 操作成本、拒绝入口与诊断

缓存引用、尚在执行的分析 lease 和淘汰后仍被保留的 lease 共用留存额度。
缓存字节额度不是 allocator 硬限额，也不保证 shaping 瞬时峰值或物理内存。
操作前预算扣费与事后存储统计分别记录，不能用统计替代可拒绝的准入。
生产者提供绑定到当前 lease／请求的 `TextStageCostPlan`：
work 以各扫描／复制步骤访问的元素次数计，重复遍历重复计；
allocation 以新增集合容量项数计，包含临时集合，不把字节直接折入旧单位。
opaque kernel 需要有依据的保守单位上界，不能默认记作一次常数操作。
Core context 在每个步骤开始前按计划扣费，未用完的保守预留不退回；
若只能增量规划，下一步计划仍须在该步工作发生前完成准入。

| 成本 | 单位与责任域 | 管理主体、扣费时点 | 拒绝／记录行为 |
|---|---|---|---|
| 分析模板、共享 backing 与登记 metadata | 留存字节；实例分析账本 | 生产者申报，Core registry 在提交 lease 前事务预留 | 缓存容量不足返回未准入；非法描述、溢出和存储预留失败为错误 |
| analyze 的分析、扫描和新增集合 | work／allocation；当前会话 | 生产者提供步骤计划，Core context 在各步骤前扣费 | 不能提供必要依据时，在该步前返回成功缺失；预算失败保持粘滞 |
| clone 的元素复制与集合容量 | work／allocation；当前会话 | 生产者提供复制计划，Core context 在复制前扣费 | 缺计划返回 `ReflowAccountingUnavailable`；预算失败直接传播 |
| 断行／对齐的扫描、行与临时集合 | work／allocation；当前会话 | 按受检步骤上界在计算／扩容前扣费 | 同上；不允许先算完再把未知费用记零 |
| 导出的 glyph、来源、几何及交互集合 | work／allocation；当前会话 | 阶段生产者与 Core 按各自输出责任，在生成前扣费 | 缺计划／预算失败拒绝候选；避免同一输出重复收费 |
| clone、行结果、导出与旧发布资产的实际存储 | 实际字节、共享 backing 与留存统计；各自 owner | 适配器／现有输出 owner 记录，不伪装成分析缓存费用 | 诊断统计，不单独提供拒绝语义；不声称全进程字节限额 |

reflow 从有效资产计算当前操作计划；必要计划无法取得时，在复制和断行前返回
`ReflowAccountingUnavailable`，不启动完整回退掩盖该错误。
阶段生产者在 analyze 前已知无法履行后续 reflow 计费合同时，应提前返回 `None`，
不登记这种资产。不能提供任意合法宽度的计费依据的实现不声明该阶段合同。
阶段失败前的 work／allocation 继续计入当前会话。
Flow 既有来源／段落准入计费保持，阶段只追加它实际新增的工作和集合分配，
不重复收取已有输出的同一笔费用，也不因缓存命中退还已经发生的来源捕获成本。
每个阶段和完整回退结束后检查粘滞失败，外部回调忽略预算错误也不能发布候选。

**Parley 0.7.0 当前尚不满足上述留存计费的生产前置条件。**
LayoutData 的 fonts、coords、styles、inline boxes、runs、items、
clusters、glyphs、lines、line items 均为私有 Vec。
公共 Layout 没有容量／完整存储用量 API。
上一轮 allocator 申请量与微基准时间不是任意输入的留存上界证书。

默认适配的下一片先取得上游或经评审依赖补丁的只读存储计费接口，
覆盖模板数组、共享字体、clone 以及行结果容量。
不依赖私有结构内存转型，不在业务引擎安装全局 allocator 探针。
新增依赖补丁须单独记录来源、许可证、版本与 Native/Wasm 兼容；
本 ADR 初次接受时未授权该补丁。用户后续已明确批准只读存储补丁接入，
来源与验证见 B 记录；操作计费接口仍须给出相应依据与复核。
本 ADR 没有预设默认缓存字节常量。
B 只取得计费接口和适配依据，C 只允许显式实例配置的受控接入。
默认限额与默认启用须等 C 正确性和 D 匹配性能门禁全部通过后再决定；
没有完成这条决策前，普通生产配置保持完整入口。

## 外部消费者证据

必须新增独立消费者验证统一入口，而不是只由默认引擎证明：

- 外部 engine 实现同一可选方法，用自定义 typed key 和 payload 登记分析资产，
  在不同宽度生成完整受检布局；Core 不需修改或识别该 engine 类型。
- 外部无阶段 engine 仍走完整路径，保留调用次数、副作用和错误 source。
- 外部 lease 测试外国 registry、错误 key、revision 改变、内容变更、
  淘汰后留存、最后一个 lease 释放、缓存不足及会话预算粘滞失败。
- 分别改变引擎身份、revision、analysis key 和 registry 身份，
  覆盖返回 lease、返回 None 和返回错误的分支，确认 Core 不接受旧绑定或错误回退。
- 缓存恰好被有效模板装满，下一次只改变宽度时，应仅调用 reflow；
  不调用费用下界查询、analyze 或完整布局，仍测试 reflow 会话预算失败。
- 分析 miss 且新增下界确定不足时，只调用一次完整布局；
  下界未知、观测过期和费用查询 Err／绑定改变均按各自合同处理。
- 扩展 Err 分别与绑定变化、预算失败及两者同时发生，
  验证主错误优先级、所有伴随字段、原始 source 可追溯，以及粘滞失败和禁止发布。
- 登记中途失败、共享 backing 去重和描述冲突、metadata 容量、registry 销毁后持有 lease，
  以及旧 lease 钉住额度导致新资产未准入，均验证计费释放与责任域存活。
- 复制、断行和导出分别验证计划缺失、预算不足与回调忽略失败，
  不将实际存储统计误用为成功准入依据。
- 外部 provider 与 OutlineTextEngine 保持完整输出；后续透传阶段能力
  必须单独验证轮廓 revision，不由本次默认路径代为证明。

以上为实施门禁，A 的协议与外部消费者结果见实施记录；
Parley 全输出等价、真实 inline 与默认性能门禁仍待 B／C／D。
既有官方 TextFlow 样例继续作为完整 Core 消费者，不加入样例专用机制。

## 实施切片与退出标准

| 顺序 | 切片 | 退出标准 | 允许启用范围 |
|---|---|---|---|
| A | 统一阶段、typed registry、lease、预算与错误 | 外部消费者、完整绑定、事务准入／释放及不支持路径通过 | 契约测试中的外部阶段实现；生产缺省不变 |
| B | Parley 计费接口与默认适配 | 存储申报与操作计划有依据；同一字体集合；未知计费不留存 | 适配测试显式 opt-in；不默认开启 |
| C | Flow 接入与完整等价 | 按下述口径通过全输出、交互、资源、反馈和旧 lease 验证 | 显式实例配置的受控测试／基准；不默认开启 |
| D | 匹配性能与默认配置决策 | 热命中、首次与拒绝路径数据齐全；依据 C／D 证据记录启用及限额决策 | 通过并记录默认决策后才允许生产默认启用；否则保持完整入口 |

### C 的精确等价口径

比较同一完整输入、同一字体／资源与 provider 依赖绑定、同一引擎 revision 的
新鲜完整路径和阶段路径。测试不依赖两份独立构造的随机身份恰好相等，
需要共享测试资源身份或明确建立身份对应，不能由对应关系掩盖资源变化。

全文尺寸、行指标、glyph ID／顺序／位置／变量坐标、样式、墨迹与绘制顺序、
inline placement、来源范围和 source-local 映射按公开字段逐项比较。
有限几何按相同数值精确比较，不引入任意 epsilon 或逐产品容差；
编码差异如有影响既有 revision 或行为，也作为不等价处理。
来源、段落、布局及交互 revision 按同次输入的身份合同比较，
不能把上个宽度的 revision 绑定到新几何。

字体／资源 handle 比较资源身份、revision、face／变量实例和有效依赖绑定；
provider handle 比较所属引擎／provider 依赖与可观察行为，
不要求新建 Arc 或 provider 对象的地址相同。
没有独立公开 provider 身份时，测试保存其来源绑定证据并比较实际查询和失败语义，
不能只因 glyph 数相同就宣布 provider 等价。

交互覆盖全部合法测试位置及两种 affinity 的 caret、selection 和公开移动操作，
并在几何边界、行边界、内外部点核对 hit test；非法位置的错误类别也需一致。
固定合同用例包括 CJK／RTL／混合方向、组合字符／ZWJ／emoji／连字、
fallback 与字体替换、显式段落方向、五种对齐与镜像、空文本和极窄／零宽、
受约束 NoWrap、真实 inline 的尺寸／结构反馈，以及旧 lease 保留。
有限用例通过不等于任意 Unicode 内容的证明，仍保留压力与差分验证。

### D 的首次、拒绝和淘汰门禁

性能对照除热命中外，必须包含首次布局、零额度、单资产过大、容量不足、
持续 resize 与淘汰、多视图以及旧 lease 钉住额度的情况。
区分提前拒绝、分析后拒绝、阶段命中、完整回退及协议错误，
记录 analyze／reflow／完整布局调用次数、已发生 work／allocation、
留存与实际存储、失败率，以及完整操作的 P50／P95／P99。

显式关闭／零配置额度或分析 miss 后确定无法准入的路径应只有一次完整布局，
没有预先 analyze；有效分析命中即使剩余额度为零，也不归入新增资产拒绝样本。
计数另记录费用下界查询次数，确认该查询只发生在分析 miss 后。
分析后拒绝可以承担额外分析成本，但必须原样记入当前请求和对照数据。
不把回退样本、首次样本或失败样本从正式分母中删除，也不重置预算。
连续输入若反复承担分析后拒绝的额外 shaping，需报告其原因和尾延迟，
将它纳入默认配置决策，不以热命中的收益抵消未解释的回退退化。
若正确性、计费或匹配性能证据不足，不作默认启用决定。

一次只推进一个 Execute 切片，状态只在
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
16 ms 更新与 4 ms 查询／IR 目标不变；隔离 reflow 约 2 ms 不能代替整体验收。
新增依赖、配置和公开 API 使用项目对应门禁；产物体积仅按用户明确要求比较。

## 取舍

不直接给默认 Parley 塞入固定条目数私有缓存：缺少字节计费，且外部引擎无法共用。
不从任意完整 Layout 抽取“可复用 shaping”：现有完整缓存身份没有声明阶段分离。
不首片推进真正的零复制分析与行资产：隔离结果支持 clone 候选，
更深拆分需要上游支持，不能据此省略当前存储计费问题。

所选路径增加小型可选协议与分析资产留存，保留完整回退和精确几何。
它优先解决同内容 resize 的重复分析，对编辑和 inline 度量变化仍可退化完整重算。
批准本 ADR 只接受这些边界；阶段是否完成以对应验证记录为准。
