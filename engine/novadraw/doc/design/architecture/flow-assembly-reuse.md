# FR-02 贡献组装复用合同

类型：`proposal`

能力归属：Core（`novadraw`）Flow、共享文字请求内容、Preparation 输入证明传递与
Runtime 发布检查。
状态：草案，修订 1，2026-11-03；按 FR02-DR01～DR06 修订，待复核。
2026-11-04 用户通过滚动方案修订 4 并授权执行；已开始共享内容／request 基础迁移。
整页采用首片已接通，默认关闭；完整反例、容量与成本仍须按本文阶段门禁逐项交付。
选择、替代与接受门槛由本文维护；[原编号 060 草案](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-060-flow-assembly-reuse.md.txt)仅供追溯，
现行 [精确 Flow 复用](exact-flow-layout-reuse.md) 继续要求贡献重读。
`api_semantics`：`text.flow`、`text.interaction`、`layout.manager`、
`viewport.scroll_zoom`、`notification.layout_update`。

## 问题与目标

Flow measure 与 prepare 都先组装贡献，随后查询完整段落缓存。
本合同要省掉同一精确输入下的贡献回调、handler 和归一化组装，
同时保持段落内容、来源顺序、失败和发布结果。缓存资格由开放声明提供，
不能由 Figure 名称、闭合内置枚举、散列或计时结果决定。

本片包含整页组装资产、公开共享文字内容，以及声明版本到发布的证明传递，
保留当前布局反馈与事务发布顺序。
子树增量拼接、完整测量复用、Runtime 调度、宽度阶段化、glyph 裁剪和 worker
均属于后续独立切片。成本与实现证据不在本规范重复维护。

## 资产与所有权

拟议的 assembly asset 拥有不可变段落文本、默认样式、span、来源范围、
source offset／ancestors、有序 members、block 关系、traversal order 与 inline 意图。
真实 Figure box 保留 child 身份及逻辑插入点；不保存当前剩余宽度下的测量结果。
边框已测量的 metrics／renderer 只有另有有效声明与完整依赖时才可入资产。

FlowLayoutService 持有实例 cache 和账本 owner 句柄；资产 lease 同时保留账本句柄。
资产不持有 PreparationScene、
MeasureContext、Runtime 或树的活借用。measure 和 prepare 通过相同受检读取取得 lease；
measure 只消费尺寸，prepare 仍生成并验证完整来源、placement、边框与绘制几何。
消费 asset 不复制全文文本／span／ancestors；宽度相关 box 和段落工作数据单独按需分配。
旧 snapshot 与 asset lease 不因淘汰、字体变化或 source 删除而突变。

现有 `StyledTextRequest` 已共享文本 backing，但构造入口接收 owned String，
span 仍为 Vec；只给 FlowPlan 外面加 Arc 不能兑现上述消费合同。
这是基础迁移前的事实；已实现的共享内容签名与计费范围见下节。
首片同时引入受检、不可变共享的文字内容资产，包含文本、default style 和 spans，
完整请求再组合当前 constraints、paragraph 设置与 boxes。
owned 构造与共享构造都对外开放且进入相同验证路径；内容资产构造一次检查 span
顺序、UTF-8 边界与范围，当前 boxes／约束仍逐次检查。
外部引擎继续读取同一 StyledTextRequest，不暴露 Parley 类型或私有可信构造器。
公开共享存储、clone 和 retained 计费的迁移纳入本片验收，不能把全文复制藏到请求构造。

### 公开迁移范围（FR02-DR02）

2026-11-04 首步冻结并实现的公开入口为：
`StyledTextContent::new(text, default_style, spans)`、
`TextContentStore::new(limit)`／`create(String, TextStyle, Vec<TextStyleSpan>)`、
`StyledTextRequest::from_content(content, boxes, constraints)` 和 `content()`。
原 owned `StyledTextRequest::new` 与所有 engine 读取方法保持。
内容构造验证 span，shared 请求重新验证当前 boxes／constraints；clone 保留同一内容
lease。`TextContentStore::usage`／`set_limit` 与内容 `retained_units`／`usage`
提供本域留存和缩容状态；容量、溢出和账本 poison 通过 `TextError::Content`
保留结构化 `TextContentError`。

实际 Flow 消费迁移已开始：mutable `FlowPlan` 完成组装后，转换为不可变
`FrozenFlowPlan`。段落的文本、默认样式与 spans 进入 `StyledTextContent`；
`FlowParagraphSnapshot::sources` 从 `Vec<FlowSourceRange>` 改为
`FlowSourceRanges`。公开消费者继续通过索引、借用 slice 和 `iter()` 读取来源；
clone 保留共享来源记录及其独立 accounting lease，不能导出裸 Arc 脱离计费。
完整请求使用 `from_content`，来源及其 ancestors 由快照共享持有。
真实 inline 的 box 工作数据仍单独生成，前缀请求和当前剩余宽度测量保持原路。
基础迁移本身不跳过贡献组装。2026-11-04 后续首片已接通可选声明和整页采用，
实验默认关闭，已启用服务开始使用家族留存域；
计费与生命周期的完整范围仍待验证，见
[整页采用实施记录](../../verification/reviews/flow-assembly-adoption-implementation-2026-11-04.md)。

该基础域只负责内容 backing：计费为记录与 Arc counters 的固定大小，加文本、
spans 及字体名的 owned capacity。账本 owner 固定开销不属于 backing charge；
调用方在传入 owned buffers 前的分配、allocator 开销和 RSS 另计。
owned 兼容入口使用独立无上限域（`usize::MAX`），受限消费者显式指定 limit；
这不是生产 Flow 服务家族的容量默认值。服务家族采用、assembly／proof metadata、
cache 驻留与跨域引用计费仍在后续接口冻结范围，基础域不宣称已覆盖这些对象。
阶段分析 registry 采用同一内容时，将其登记为 shared backing，按本域唯一身份计费；
request／box metadata 继续独立收费，不把引用复制当作全文复制。

### Preparation 证明传递接口

2026-11-04 开始实现统一的 typed 输入证明收集器。公开入口为
`PreparationInputKey<P>`、`PreparationInputPayload`、`PreparationInputReader<P>`、
`PreparationInputRegistry::register` 与
`PreparationScene::input(registry, key, source)`。`input` 返回不可变的
`PreparationInputSnapshot`；`value(key)` 只允许读取对应 payload 类型。
payload 必须拥有完整、不可变的输入值，完整相等比较须纯且有界。
reader 在捕获和重验时走同一注册入口，返回的普通错误保持原分类和 source。
该接口已用于 contributor／handler 声明的整页输入证据；
基础证明链通过不代表整页生产矩阵完成。

注册实例由 registry 拥有；registry clone 保留已有注册身份。证明保留首次 payload
和注册的弱引用，不保活 reader、Runtime 或 Flow 服务。最后一个注册 owner 销毁后，
旧 evidence 仍可读取，重验返回 `RetiredRegistration`，不再调用 reader。
证明绑定 root、source 和测量调用约束；重验先在当前树权限内建立对应读取范围。
外部状态必须保证 Runtime 稳定化期间没有并发写入。允许 Runtime 串行回调改变输入，
后续检查必须发现该变化；异步外部写入域的稳定读取保证仍未实现。

收集器随测量缓存和准备 metadata 自动传递证明，公开
`PreparationMeasurement` 的尺寸字段和 Copy 语义暂时保持。
布局协调器在采用 `LayoutOutput` 前检查测量证明，并将其留在对应容器的派生状态中。
新受检布局输出替换该容器的旧证明；失败回滚同时恢复旧证明。
移除或替换布局管理器时，该管理器的旧测量证明退役；
子图准备结果所持有的证明仍由准备结果独立维护。
2026-11-05 修复布局缓存的跨轮输入检查：Runtime 在新一轮稳定化的回调开始前，
重验仍有效的布局缓存所持有的输入证明。输入变化使对应容器及其验证路径失效，
由布局管理器重新计算并安装新的证明；准备结果的尺寸未变也不能跳过这一步。
reader 错误、注册退役或预算失败继续传播。该检查不替换旧证明的首次值，
本轮回调开始后的输入变化仍由消费、批次及最终发布检查拒绝。
重读和临时来源快照使用同一 preparation 预算，其成本须计入后续性能验证。
准备批次同时收集新候选和本批复用的旧准备结果，发布前重验首次值。
Runtime 在路由及准备协调完成后，再检查当前布局输出和准备结果的完整证明集合。
重复读取或合并冲突不会覆盖首次值，忽略错误也不能清除当前收集器的失败状态。

2026-11-05 管线删减实施：同一收集器内的重验直接借用有序首次证明，
不再为每个检查点复制整份 receipt 列表。reader 获得独立 `input_scope` 收集器，
不能向被借用的原列表追加；检查失败后先释放借用，再锁存原始错误。
每个 reader、注册存活检查、约束绑定和最终发布检查仍执行。
需要向测量缓存或候选转移证明时，继续生成并计费独立 receipt；
本项只删除检查点内部的交接复制，不扩大检查结果的有效区间。

资格 metadata 的共享范围限定为同一不可变 source prefix。每次读取仍调用
全部 contributor／handler 声明，比较完整身份、generation、贡献类型序列和输入快照。
当结果逐项相同，借用已有不可变 participants backing，省去重新物化 participants
及 handlers 数组；临时声明的实际分配继续准入计费。首次不同才创建新 backing，
旧 backing 保持不可变，收集器继续拒绝首次值与后读值冲突。
该共享仅保存弱引用，不保活注册或过期源；来源 backing 地址只定位候选，
存活的弱引用和完整内容比较决定相等，不承担版本权威。
无声明、普通错误、注册退役、预算失败和回调屏障后的重验规则不变。

首个贡献类型内联存储试验未通过净成本验收，已退役。
声明继续使用完整有序数组，重复类型不去重，内置和外部声明共用存储。

不可变 metadata 的计费摘要试验也未通过净成本验收，已退役。
当前继续按实际 backing 与嵌套记录计算留存费用。两项试验的源码、
前后计时与 Perfetto 证据保存在逐项验证报告所指的本地产物目录。

本基础片使用现有 `PreparationBudget` 为 reader 临时工作、payload 接收、
proof metadata 和回放计费。它尚未提供跨调用留存账本、服务家族容量或资产 lease
去重，不关闭 FA-R20／R21／R28／R29 的 proof 与 assembly 子范围。
具体实施及反例证据见
[输入证明实施记录](../../verification/reviews/flow-input-proof-implementation-2026-11-04.md)。

| 能力／API | 状态 owner | 迁移与兼容性要求 |
|---|---|---|
| owned／shared 内容构造与内容读取 | Core Text 的不可变内容资产 | owned 调用语义和非法 span 拒绝保持；新增 shared 入口接收受检只读句柄，不能直接接收未校验 backing |
| StyledTextRequest 组合与读取 | Core Text request | 请求保留当前 constraints、paragraph、boxes；引擎方法及读取语义不变，公开构造与快照需独立编译验证 |
| 内容校验、clone 与 backing lease | Core Text content／所属账本 | 构造一次完整校验内容，clone 保留 lease；新请求逐次校验 boxes／constraints；计费同时覆盖实际 backing 与 metadata |
| FlowPlan 内容消费 | Core FlowLayoutService | measure／prepare 共用 shared 内容，不能在请求构造重新复制全文和 spans |
| 声明证明收集与测量传递 | Core Preparation | 在当前调用作用域记录证明，随测量回执／准备 metadata 传递；不能因为只返回尺寸丢掉发布依赖 |
| 最终候选检查 | Core Runtime | 检查 typed 输入证明并传播原始错误，不持有可变 Flow 服务，不改变原子提交权威 |

签名与容器布局仍待接口评审，不能声称所有源代码自动兼容。owned 入口、外部 engine
读取与既有行为是迁移约束；任何必须变更的公开类型、构造、返回 metadata、
API 快照和消费者调用点在实施启动前逐项冻结。共享内容按下文独立 lease 计费；
请求 clone 不能漏掉 spans backing 或重复计费相同 backing。

缓存责任域必须显式定义服务 clone 的含义：共享 cache 的 clone 同时共享实例身份、
绑定和账本；不同 registry／root 的新服务不得意外继承资格。
同一 Runtime 的不同 root 与不同 Runtime 都不互用资产，即使文本相同。
不存在全局缓存、跨 Runtime 活对象或第三方文本库类型穿透。

## 可选声明的公开边界

2026-11-04 首片已在现有 contributor／handler trait 上追加以下缺省方法。
两者同名同返回类型，缺省为成功缺失；实验入口默认关闭。

```rust
fn assembly_reuse(
    &self,
    context: &FlowContributionContext<'_>,
) -> Result<Option<FlowReuseDeclaration>, FlowError> {
    Ok(None)
}
```

已实现的声明构造入口为 `FlowReuseIdentity::new`、`declaration(generation)`、
`with_contribution(key)` 和 `with_input(snapshot)`；
闭包适配器为 `DeclaredFlowHandler::new(process, declaration)`。
服务保留单个完整组装候选，使用完整 source 和有序参与者比较，不建立 hash 命中权威。
服务 clone 共用 cache、开关、注册身份与留存配置。
默认无上限兼容域、显式容量接口及尚待核对的 metadata 范围由实施记录说明，
不能据接口存在认定生产容量门禁已完成。

`FlowReuseDeclaration` 包含 owned `FlowReuseIdentity`、generation 和完整输入声明。
identity 由显式构造器创建并归扩展实例持有，clone 保留身份；
generation 单调变化，溢出必须换新 identity 或结构化拒绝，不能回绕。
registry token 由注册拥有，不要求 handler 自己用指针猜注册身份。
handler 声明不接收待生成 payload：相同 contributor 输出版本已保证完整 payload 序列，
handler token 与自身版本另保证解释行为。若 handler 的资格需要检查 payload 内容，
首片整页不合格，继续原调用；不能为命中而先生成 payload 又声称省去 contributor。

输入域使用开放 typed key＋payload trait＋registry；owned 输入 payload 通过 typed
完整相等比较，不能只有显示名或 hash。Core 标准的静态样式／结构、几何／测量依赖
与第三方资源域走同一登记机制。域同时定义受检捕获、版本重读、观察回放及失败；
未登记、未覆盖或无法完整比较的域使页面不合格。不能用闭合 read-kind 枚举
只允许内置声明，也不能提供通用“可信／不检查”布尔值。
留存的输入 payload 必须是不可变快照；比较需纯、有限并按对应域计费，
不能保留指向实时可变状态的对象后把同一对象的相等当作版本证明。

| 扩展入口 | 拟议合同 | 缺省行为 |
|---|---|---|
| FlowContributor | 可选 fallible 声明，绑定具体实例、typed identity、版本和输入域；同版本承诺输出与失败行为相同、无必须执行的副作用 | 无声明，每次 contribute |
| FlowContributionHandler<P> | 可选 fallible 声明，绑定注册实例、payload 类型、版本、读取域与可省略 process 承诺 | 无声明，每次 process |
| FlowContributionRegistry | 为每次注册建立稳定 token，资产绑定整个参与集合及注册代际；registry clone／替换语义明确 | 沿用开放 register，不以 TypeId 单独代表实例 |
| FlowFragmentBorder | 可选 renderer／metrics 资格，绑定边框实例、版本、owner style、字体与引擎身份／revision及读取依赖 | 无声明，整页资产不合格，沿用 prepare_renderer／metrics |
| 受检依赖描述 | 用 typed key、owned payload 和登记实例记录外部状态；Core 不解释任意 Any，也不按名字匹配 | 未覆盖的读取使资产不合格 |

内置 Text／Block／Group 只能通过上述相同入口声明，外部拥有相同 typed 入口、
版本、错误与容量能力。现有闭包 handler 的 blanket impl 必须保持源兼容；
需要声明的外部 handler 使用统一的声明适配器；该适配器接受原 process 闭包和
fallible 声明闭包，通过相同注册 API 进入 registry。显式 struct 实现同样可用。
首片不另建第二套可选 capability，也不同时保留内置 typed 快路径与外部 erased 慢路径。

immutable payload 不能证明生产 payload 的回调纯。contributor 声明不能替代 handler
声明，也不能替代 border／renderer。引用指针地址不能独立承担实例身份；
移除后重注册、地址复用、相同 payload 类型的不同 handler 均产生不同绑定。
版本必须覆盖内部可变文本、控制分支、访问依赖、成功／缺失及错误行为，
不能只借用 Figure component revision。
Core 能检查声明身份、版本与已登记依赖，不能发现扩展故意漏报的私有变化；
这属于扩展作者承担的合同，和现有文字引擎 cache_identity 的承诺性质相同。
验收须给出内部可变状态的正反例，不能将可选声明宣传为对任意回调自动证明纯度。

声明本身仍在每次必要检查时读取。它必须无业务副作用、稳定且有界，
不保证调用次数，失败不能解释为“没有能力”。cache 查找、重建和发布延续当前
Runtime 串行执行模型，不新增并发重建；只读 lease 的线程边界由公开类型约束决定。

## 输入域与精确命中

首片使用保守完整输入，命中比较完整有序记录：

| 输入 | 必须绑定的事实 |
|---|---|
| 实例与结构 | Runtime／root／service／registry 身份，完整树成员与父子顺序、component／capability 代际 |
| 参与者 | 实际访问的 contributor、每个 payload 的 handler token、声明版本、读取域及注册配置 |
| 样式与资源 | 完整 resolved style、方向、insets、component 与资源输入；其他源属性按声明读取域绑定，外部 provider 版本逐次重读 |
| 派生读取 | 回调实际读取的 geometry／measurement／prepared 及传递依赖，包括失败与合法缺失 |
| 引擎与字体 | 凡声明回调或边框使用 MeasureContext，绑定具体 engine identity、revision、font registry 与字体描述；缺失 engine 身份不复用这类结果 |
| 环境与调用域 | 若依赖约束、measure selection、页面宽度或 origin，按完整值隔离；未声明宽度独立不得跨宽度命中 |

现有 source prefix／scene revision 可以提供候选定位或部分事实，
不因一个 u64 相等省略完整比较。哈希碰撞必须落到不同完整输入。
PreparationScene 的观察日志用于 Core 派生读取，不能证明任意扩展私有状态已被观察。
raw figure／capability 读取范围广；若无法覆盖其所有可观察行为，则降为不合格。

宽度变化后能否共享 assembly，必须由全部参与者承诺不依赖 width／约束／派生几何；
对这些输入有依赖的页面走精确 key 或重读，不能从标准 Large 推断通用资格。
宽度独立资产的完整输入不包含未读取的 root width／origin；完整 source prefix
中这些字段只能作候选事实，不能原样作为必相等 key 又宣称跨宽度命中。
任何省略须由相同的开放读取域合同证明；Insets、文字和结构等实际输入仍完整比较。
首片允许为安全保留完整依赖扫描，不以命中率或声明存在证明净性能收益。
隐藏、成员增删、源删除／重挂载、祖先样式与方向变化都进入完整输入检查。

### 命中的工作分母（FR02-DR03）

组装遍历负责执行 contributor／handler 并重建 paragraphs、members、spans 和来源。
命中验证遍历负责确认当前成员、绑定、完整输入与动态版本；首片仍允许扫描整树。
两种遍历分别计数，不把“组装未执行”写成“Figure 未访问”。
观察回放、验证遍历、声明回调、完整比较和 shared 内容消费继续计入净成本，
并保留 cold、miss 与不合格页面的分母。诊断细阶段耗时不作为可回收预算。

## 冷路径、命中与发布（FR02-DR01）

1. 先检查 preparing root 的可选声明；成功缺失立即走现行路径，不为未声明页面
   先捕获整树缓存输入。合格 root 再在当前权限内捕获完整静态输入与其余必要声明。
   发现其他确定无声明者也回到现行路径；声明或依赖读取失败则返回原始错误。
2. 保留本次首次捕获的完整输入与声明证明，再与 cache 的历史输入比较。
   历史输入不相等是普通 miss，可以按本次首次值重建；本次两次读取不相等是
   输入变化错误，不能降为 miss。命中仍重验每个动态版本及观察。
   检查调用保留当前 root、selection、预算和测量 cycle 规则。
3. 未命中时按现行遍历执行 contributor／handler，收集全体参与者和实际读依赖；
   创建新不可变候选，所有新增分配在增长前准入。
4. 冷构建后重新读取最初参与集合和版本。后一个 callback 修改了前一个参与者，
   或产生新的成员／依赖变化时，本次 measure／prepare 失败；不安装资产、不返回
   由混合版本组成的尺寸或准备结果，首次值不被后读值覆盖。
5. 只有组装后的重验通过，才允许将候选用于本次私有计算。段落排版、inline 前缀与
   Figure 测量、prepare 几何及结果校验仍运行；调用返回前再次验证首次证明。
   任一不一致使本次调用失败，不返回尺寸或准备结果。cache 安装还需调用成功与
   预算检查通过；measure 成功可安装组装资产，但不发布 preparation。
6. 观察按首次读取顺序与 generation 回放到当前 scene；回放前预留预算，
   当前已有同 subject 的首次记录不能被覆盖。新声明证明按下文路径进入最终检查，
   不能只回放现有 geometry／measurement 观察。

### 首次证明如何进入提交

拟议的 `FlowReuseProof` 是 owned、不可变的输入证明，包含 Runtime／root／service
绑定、参与者和注册 token、有序的首次 declaration identity／generation、完整输入
快照以及各 typed 输入域的重读句柄。contributor、handler、border、renderer 及
外部输入域分别入证明；已有 Core geometry／measurement 观察仍独立保留。
重读句柄是已登记、受检的只读 verifier lease，不保存 Runtime／scene 活借用，
不延长可变 Flow 服务寿命，也不能只保存缓存散列。

| 检查点 | owner 与证明传递 | 输入变化后的结果 |
|---|---|---|
| 组装后、measure／prepare 返回前 | Flow 服务对本次首次证明逐项重读 | 本次调用返回结构化输入变化错误；不安装候选，不返回过期尺寸或准备结果 |
| 测量消费者采用结果前 | Preparation 收集器随测量回执保留证明，传递依赖合并保留首次值；布局协调器在应用该测量产生的 LayoutOutput 前检查 | 拒绝对应布局输出，按现行稳定化失败／恢复合同退出；外部 query 不获得这次失败计算的尺寸 |
| PreparedScene 自身与批次候选检查 | 准备结果 metadata 携带相同证明；Preparation 的统一 typed 输入域验证器重读 | 拒绝候选，不安装本次尚未安装的资产，不发布新快照 |
| Runtime 最终发布前 | Runtime 候选批次收集所有测量消费及准备结果的传递证明，完成可能调用扩展的其他检查后，再统一检查首次版本 | 整批返回错误并取消提交，保留此前成功发布结果；不把旧快照当作本次成功返回 |

证明须在未命中的完整计算与命中消费两条路径都收集。直接 measure 返回的尺寸只代表
该调用返回前已验证的输入；任何后续布局／准备提交必须携带其测量回执证明。
Core 的测量缓存、投影和返回 metadata 不能把证明降为裸尺寸后丢弃。
如果现有公开入口不能保证此传递，实施前必须把测量回执／metadata 的接口迁移纳入
本片公开 API 冻结；不能依赖调用方自愿额外检查。
同一个 proof subject 再次读取或合并时发现版本冲突，立即使本次候选失败，
不只保留首次值后忽略冲突。Runtime 收集的是当前待提交输出的依赖闭包；
早前轮次的证明只有在对应输出被新的完整受检结果替换后才能退出候选。
正常稳定化的新轮次独立捕获输入，不用新证明给旧尺寸或旧快照补证。

最终 verifier 先检查 owner 与 token 是否仍有效，再在当前 scene 权限和预算下
调用已登记输入域 reader，比较完整首次值。reader 必须纯且有界；
无法在最终读／提交区间提供稳定读取保证的外部状态域不具备复用资格。
稳定性须由覆盖最终检查到发布的受检只读 lease，或该域明确的无外部写入区间保证；
最终重读仍检查当前绑定与 generation，不能把首次快照的 clone 当作当前版本证明。
最终检查后到发布前不再调用可改变这些版本的扩展。下一次独立输入变化按正常失效
处理，本合同不承诺发布后永远有效。

### 输入变化与读取错误的区分

拟新增 owned `FlowReuseInputChanged` 错误，诊断字段包含 owner、typed subject、
参与者／注册身份、检查点、首次与本次版本；需比较的完整输入证据用 owned typed
句柄保留，不对任意 Any 格式化。名称与公开返回映射在实施前冻结。
Flow 返回的原因经 TextError 和 PreparationMeasureError／准备失败传递，
Runtime 保留相同 source；诊断检查点只用于显示，不承担行为身份。

历史 cache key 与当前首次输入不同可以重建；本次首次与后续值不同则立即失败。
reader 返回的普通 Error、foreign binding、预算或协议错误沿原有分类传播，
不能包装成输入变化，也不能当作成功缺失。所有失败都不返回过期结果。
局部检查通过后、发布前再变由 Runtime 最终证明检查检测；失败不撤销早前合法
measure 调用已经私有安装的资产，而是拒绝本次批次，其资产下次必须重新验输入。
此前 source mutation 的回滚范围和 Runtime fault／恢复行为保持现行合同。

缓存 hit 后也可能出现外部版本变化；检查失败不使用旧 asset 继续发布。
首片不建立无界重试，也不对同次预算失败重置账本或从完整入口重跑。
单次命中是否省去了完整组装，必须有 callback、handler 和组装计数证明。
静态输入允许按当前不可变借用流式比较，冷 miss 才构建 owned key；
这不省略全体必要成员与动态版本检查，避免命中时先复制整树才能查 cache。

## 边框与真实 inline 的处理

边框 prepare_renderer 和 metrics 在组装中发生，影响 block padding 与 marker。
只复用文本不能省掉它们。未声明的边框让整页 asset 不合格，正常行为仍完整执行；
这是统一依赖资格规则，任何内置／外部边框都适用。
声明边框应覆盖 renderer 生命周期、owner 字体、engine/font 变化与所有动态资源，
不能只声明数值 metrics 稳定而保留可变 renderer。

inline Figure 的 remaining width 依赖前缀布局、前面 boxes 与当前约束。
asset 只保留 child 身份与顺序；prefix layout、child measure、超宽重测保持现行行为。
如果 contributor／handler 自己先执行 measure_figure，首片不能把这种页面视为
与宽度无关；依赖与约束必须完整覆盖，否则整页重读。

## 容量、预算与寿命（FR02-DR04）

容量控制分为以下责任域，不能用 preparation 预算代替跨调用留存账本：
本域计费范围是本片组装、共享内容与证明相关的 backing／metadata；
既有 glyph、字体和 GPU 资源继续由各自 owner 管理，不将本账本称为全 Runtime 内存上限。

| 控制面 | 计量对象与 owner | 控制范围 |
|---|---|---|
| cache 驻留上限 | Flow 服务共享家族；cache 当前持有的 entries、backing 与 key metadata | 控制安装／驻留；缩容时淘汰 cache 引用，不强制释放外部 lease |
| 存活留存账本 | 服务家族与全部资产 lease 共同持有的独立 owner；包括 cache、当前候选、当前帧、旧 snapshot、外部 lease 和证明 metadata | 控制新增唯一 backing／metadata 的准入；旧合法对象可超过缩小后的 limit |
| 单次 preparation 预算 | 当前 preparation／稳定化调用 | 每次扫描、声明、比较、回放、消费和新增工作在执行前收费，耗尽后粘滞失败 |

同一责任域内，cache、候选、当前帧和外部 lease 可以引用同一 backing，
分类统计允许重叠，唯一留存总量不重复相加。子范围钉住完整 backing 时计费全文。
charge 使用明确的逻辑留存单位，随 public 配置冻结；allocator 请求字节、
唯一实际分配和 RSS 另行测量，不把逻辑单位命名为实测字节。

### 缩容和新增准入

缩小 cache 上限时按已冻结淘汰规则移除 cache 引用，直到驻留符合上限。
外部 lease 不被改写或撤销；某 backing 仍被引用时留存总量不减少。
若缩小后的留存 limit 小于已存活总量，账本进入可观察的超限状态，
记录当前留存、limit 与尚未释放的 reservation；不会追溯撤销已经成功的操作。

超限期间，已有合法资产的读取和不增加 backing／metadata 的 clone 仍允许。
新增正 charge 的 reservation 仅在当前已用量、未落地 reservation 和新增量之和
不超过对应 limit 时获准；否则返回容量准入拒绝。
旧 lease 释放或显式提高 limit 后重新满足条件，才可恢复新增准入。
不得通过忽略旧 lease、换一个同家族账本或 copy 到未计费 backing 绕过超限。

cache slot 不足只取消缓存驻留：若存活账本和本次 preparation 预算能准入，
仍可完整计算并返回带 lease 的合法结果。留存准入失败时不能返回未计费的新资产，
也不能把复制或 owned 输出作为容量回退；只有复用已有资产且新增 charge 为零的
路径可继续。已经产生的 PreparationBudgetError 保持粘滞失败，不通过完整重跑绕过。
查找已有合法资产先于新增 reservation，检查／回放／消费的本次工作预算仍收费。

### 账本如何活到最后一个 lease

共享服务 clone 属于同一服务家族，保留同一绑定、cache、容量配置和账本，
不能为每个 clone 重新授予完整容量。新 registry／root 的独立服务使用独立绑定，
不继承旧 cache；跨责任域采用 shared 内容时显式登记，其本域 charge 不冒充全局去重。

每个 backing 在增长前取得独立 reservation，候选失败且尚未分配时取消 reservation；
已分配对象的 reservation 随 owning backing record 留存。
对外只读内容、组装资产、snapshot、切片、request clone 和证明句柄携带 accounting
lease，不能导出脱离账本的 owned 裸 backing 引用。借用 slice 不改变所属责任域。
账本不强持有 backing，避免账本与 lease 形成保活环。

最后一个服务 clone 销毁时关闭本家族的新资产生产并释放 cache 引用，
账本继续由旧 lease 持有，不依赖 Runtime、Flow 服务或 registry 的活对象。
现存只读 lease 的合法性不变；最终 verifier 仍须检查其提交 owner／注册 token，
旧 lease 存活不表示旧提交资格仍有效。
最后一个 backing／metadata record 释放时，只执行内部扣账，不调用用户回调、
不访问 Runtime，不因诊断输出引入失败。最后 owner 句柄随全部 record 释放后销毁。

容量默认值、逻辑 charges、reservation 的增长前检查、淘汰顺序与超限查询接口
在实施启动前冻结，本次不补造数字。采用 imported backing 的责任域切换、
证明 metadata 和部分构造失败须同时进入留存与峰值验证。

## 错误与既有恢复合同

留存正 charge 准入拒绝使用拟议的 owned `FlowReuseCapacityError`，
保留账本责任域、当前 used／reservation、requested、limit 与拒绝原因；
其 source 经 Flow／Text／Preparation 保持，不混称 cache slot 不足或单次预算耗尽。
超限状态查询与返回签名仍在实施启动前冻结。

扩展普通 Error、协议／绑定错误、预算耗尽保持原 cause；
新诊断层不得改成 String，也不得把错误变成 Ok(None)。
panic 沿用所属 preparation／Runtime 已接受的 fault 和恢复合同；
若引入新的捕获边界或 poison 恢复，需要独立评审，首片不许悄悄改变。
失败不安装半成品、覆盖最后成功资产或发布部分快照；
此前已提交 source mutation 不承诺回滚，旧合法 lease 按现行合同继续存活。

## 对 ADR-051 的局部修订效力（FR02-DR06）

当前仍为提议，ADR-051 的贡献重读要求未改变。只有本方案获接受后，
完整声明、绑定、输入证明和所有对应检查通过的页面才允许省略贡献执行。
任一参与者无声明或依赖域无法提供完整证明时，页面按 ADR-051 走现行完整路径；
已经出现输入变化、绑定、协议或预算错误时，按本方案失败规则退出，不能回退掩盖。

接受时同步[现行精确布局设计](exact-flow-layout-reuse.md#精确缓存与共享协议)
中“贡献、边框度量仍读取当前场景”和“每次测量仍读取其当前贡献”的条款：
分别链接本局部例外，保留不合格页面、真实 inline 当前测量、完整布局、
来源几何和发布一致性要求。提议期间不提前改写现行条款。
首片组装资产不扩大为完整测量或整页快照缓存；用户于 2026-11-05 授权的
管线删减增量按下节独立实施、测量，不据此扩大原首片验收结论。

## 管线删减增量：纯文字测量复用

能力归属：Core。首个增量只保存测量尺寸，不保存整页绘制快照。
完整贡献声明合格、无真实 inline boxes、无 fragment border 时，测量生产是
完整组装输入、文字引擎实例与 revision、有效宽度和测量约束的纯函数。
所有这些值完整比较；散列、事件名称和 Figure 类型不授予复用资格。
来源已含结构、样式、字体、资源、source bounds 和普通 border insets；
声明读取的派生依赖继续逐项重验并保留。每次仍捕获首次输入、执行 adoption
和 return 检查，后续回调与最终发布的证明链不变。

服务最多保留四个近期测量记录，用于有界／无界宽度及滚动条反馈的交替请求；
FIFO 淘汰不改变旧发布。每个记录的固定 metadata 与观察数组进入留存账本，
其输入快照使用原共享 lease；cache limit 同时约束记录及所引用输入。
没有 engine identity、任何不合格声明、inline 或 fragment border 时执行完整测量。
宽度、约束、引擎或输入变化导致 miss；普通错误与预算拒绝继续传播。
此增量不省略真实 inline 回调，不缓存未知 renderer，也不改变布局版本或投影。

验收对照必须分别报告测量生产次数、拖拽和 resize 整体耗时与帧耗时。
输入／字体／宽度变化、预算失败、外部声明与后续私有状态变化仍由既有反例验证。
完整快照与可见原生字形提交继续属于后续增量。

## 接受、实施和启用门禁（FR02-DR05）

| 阶段 | 所需证据 | 不由该阶段授予的结论 |
|---|---|---|
| 架构接受 | 明确的公开接口职责、测量／准备／Runtime 证明链、结构化错误、容量行为；独立消费者接入流程及无私有 hook 的可行性评审 | 不要求已完成生产用例；不等于授权代码实施 |
| 实施启动 | 用户实施授权；具体公开签名／metadata、容量默认值／charges／淘汰规则、编译与行为 target、冷路径容限和启用判据评审冻结 | 不等于实施完成或默认启用 |
| 实施完成 | 实际迁移、独立 crate 编译与全部适用行为／寿命矩阵；现行 owned 调用和外部引擎回归通过 | 不以测试通过代替净性能与默认启用 |
| 生产启用 | 按冻结判据的匹配净收益、留存／峰值、cold／miss／不合格路径回退验证；未解释回退和范围限制明确裁决 | 不自动关闭 LT、ADR-059、GPU／present 或平台门禁 |

接受前的消费者可行性检查按公开接入流程进行：独立 crate 创建 contributor 和
payload／handler，通过普通 registry 注册声明与外部 typed 输入域；
从公开 owned／shared 内容构造建立同一 StyledTextRequest，交给既有 engine；
测量／准备回执在 Core 消费链保留证明，外部读取旧 lease 不绕过账本。
评审核对每一步的权限、类型、失败和寿命是否足够，不将流程草图写成已编译结果。

本轮不要求、未构建可编译原型。若接受前后需要原型，单列隔离目录／依赖／接口模拟
与授权范围，原型结果不代替生产实现；未经授权不为满足接受条件修改 Core。
任何尚未冻结的签名、capacity 默认值、charges 和量化回退判据，保持待评审；
全部满足实施启动条件后才进入生产 Execute。

## 验收与开放扩展证据

### 管线删减增量：共享结构性 route 归属

能力归属：Core Preparation。范围限定为同一不可变 tree/style 借用。
资格 reader 的 Figure／capability 读取仍保留 route 输出 dependency observation；
只合并反复发现“该节点或祖先是否具有 Connection capability”的结构查询。
Connection descriptor 的公开构造使用 `of<T>`，投影由 blanket `AsAny` 的具体类型
决定，不读取可变的 provider revision。parent 和已安装 descriptor 也受树借用保护。

RootInputs 在已有 subtree membership 构造中同时汇总 route 子树成员；
先检查 root 及其树外祖先，再将归属向子节点传播。输入验证的独立 scene 共享该
结构摘要，保留自己的 observation 和首次证明日志。每次读取仍检查成员权限，
命中 route 成员时仍记录原节点的 FigureGeometry，不能改为祖先的 geometry。
普通读取不产生新 dependency；几何 generation、provider revision 和输入值不缓存。
外部 `ConnectionCapability::of<T>` 与内置连接使用同一投影和汇总算法。

摘要不跨 tree 借用、反馈写入、挂载或注册替换复用。构造临时记录与 route 成员
在预算中计费，失败不能安装部分 RootInputs。验证包括 root 外的连接祖先、
混合连接／普通 sibling、独立 input scope、实际 geometry observation、
跨借用重挂载与预算失败，并独立比较拖拽和 resize 的总体成本。

[验收矩阵](../../verification/plans/flow-assembly-reuse.md)负责逐项反例和完成判据。
实施启动前冻结最终接口和机器入口；现有精确 flow tests 不足以证明新声明／绑定，
须登记独立消费者 integration target，不能只在私有单元测试注入缓存资格。
归因测量用当前实现，生产前后比较以实施前最新源码重新冻结。
两者分开，诊断细阶段收益不算生产收益。

## 替代与代价

仅复用文本 payload 改动较小，但保留 handler、遍历和来源重建，不能兑现整页收益。
仅按 scene revision 或内置 Figure 类型复用缺少扩展状态与身份依据，不采用。
首片不减少 Runtime 反馈轮次，也不同时实施宽度阶段协议。

完整组装复用需要新的扩展声明和输入检查，增加缓存留存与冷路径成本。
共享内容与证明传递增加公开 API、clone 计费和测量／准备 metadata 的迁移范围，
需验证现有 owned 调用、外部引擎和跨包消费者的兼容性。
无法证明安全的页面可能一直走原路径；必须用真实工作次数和端到端分布验证收益，
不得把协议实现或局部阶段减少视为 16 ms 更新目标通过。

原提议范围、修订与历史判断见[原稿 060](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-060-flow-assembly-reuse.md.txt)。
