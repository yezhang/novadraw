# FR-02 贡献组装复用验收矩阵

类型：`verification-plan`

能力归属：Core Flow、共享文字内容／request、Preparation 证明传递与 Runtime 发布。
研究驱动只提供消费者，Editor／backend／platform 无新增能力。
状态：生产矩阵草案修订 1，2026-11-03；FR02-DR01～DR06 已回应，待复核。
成本归因的既有完成范围保持，新生产行为仍未执行。

用户已授权贡献组装内部成本归因、设计交付及本轮文档修订。生产复用实现尚未授权；
现行 [精确 Flow 复用](../../design/architecture/exact-flow-layout-reuse.md) 的贡献重读合同保持有效。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md) 维护。

## 本轮归因合同

沿用默认关闭、实例持有的 Runtime TimingRecorder；只增加固定名称的同步细阶段。
树遍历、registry 分发、贡献回调、handler、文本与来源组装、block 和边框分别计时。
递归调用的 inclusive 不作为自身成本；exclusive 扣除所有已记录子阶段。
真实 inline 测量在 FlowPlan 组装之后，独立记录前缀排版与 Figure 测量，不计入贡献组装。
诊断不重读 provider、不改变预算、错误或发布结果，不在热路径打印日志。

先构建当前实现的 coarse baseline，再构建细阶段诊断版本，采用相同 Rust 工具链、
release、锁文件、字体与驱动。两版均执行 timing-on／timing-off，交替采样；
这比较的是诊断扰动，不是生产优化收益。原始日志、冻结源码与收据放在
`target/verification/flow-assembly-2026-11-03/`，不纳入 Git。

原 Large、单长段、NoWrap 长行、真实 inline 混排分别报告分母与失败。
外部边框的正例、错误和旧快照留存由既有结构化 Flow 测试复核。
本轮不增加可见裁剪、减少反馈轮次、开启 ADR-059 缓存或修改渲染主循环。

## 归因退出要求

本轮 30 项定向回归、8 组预检和 3200 个正式样本通过。
[归因报告](../reviews/flow-assembly-attribution-2026-11-03.md)记录完整分母、扰动与限制：
关闭路径无性能回退尚未证明，不能把本轮退出解释为生产性能门禁关闭。

| ID | 检查 | 要求 |
|---|---|---|
| FA-D01 | 诊断开关 | 相同完整 request、行指标、glyph、调用次数与失败；关闭时无计时记录 |
| FA-D02 | 计时守恒 | 每操作 root inclusive 等于全部阶段 exclusive 之和；失败退出无活动 scope |
| FA-D03 | 原始分母 | Large 初始 40 段／51,232 字节；resize 后 51,233 glyph／7,889 commands，滚动新增布局为 0 |
| FA-D04 | 递归归因 | 同阶段递归按 exclusive 汇总，不能相加 inclusive 或相加各阶段 P95 |
| FA-D05 | 诊断扰动 | coarse／fine 与 on／off 同配置采样，报告 P50／P95／P99、失败与后台负载限制 |
| FA-D06 | 组装外工作 | inline 前缀排版与真实测量单列，未实际触发的边框阶段不补造耗时 |

## 生产复用行为矩阵（待评审、未执行）

合同见 [FR-02 设计](../../design/architecture/flow-assembly-reuse.md)；
各行必须同时验证布局请求、完整输出、回调次数、错误与发布状态。
“命中”指复用完整 assembly asset，不等于段落缓存命中。

| ID | 触发条件／反例 | 必须观察到的结果 |
|---|---|---|
| FA-R01 | 全部 contributor 与 handler 声明；同完整输入连续 measure／prepare | 组装资产命中，contribute／process 不重做；布局请求、文本、span、source offset、ancestors、顺序与原路径相同 |
| FA-R02 | 任一 contributor 或 handler 无声明；混合内置与外部节点 | 整页按原路径重读；其他节点不能获内置免检；合法结果和原错误不变 |
| FA-R03 | 相同 payload 类型，不同 handler 实例；registry clone／替换／移除后重注册 | token／版本变化使旧资产失效；共享 clone 的域与账本一致，地址复用不能制造命中 |
| FA-R04 | contributor 内部可变状态改变，Figure component revision 不变 | 声明版本变化被检测，重新读取新贡献；版本遗漏者不具备合法复用合同 |
| FA-R05 | handler 内部状态、配置、合法缺失或错误行为改变 | 独立版本与输入失效；不得仅用 contributor 版本复用 |
| FA-R06 | 两个不同 root／Runtime／service，文本相同 | 不互用组装资产；foreign 身份返回所属结构化错误 |
| FA-R07 | 新增／隐藏／恢复／删除／重挂载 sibling，父子或贡献顺序变化 | 完整成员和顺序失效；退役 source 不进新快照，旧 lease 保持合法不可变 |
| FA-R08 | 祖先 font／foreground／方向／insets 改变；空 block／空 text | 完整样式与资源失效；空来源、margin collapse、bidi 与 block 继承保持 |
| FA-R09 | 同候选散列／长度但不同完整输入 | 不命中；测试采用可注入候选索引碰撞，不能只测试自然不同散列 |
| FA-R10 | 只改 width／origin；所有参与者明确宽度独立 | 可复用 assembly，仍按新约束断行与组合；不能复用旧页面 origin／placement |
| FA-R11 | contributor／handler 读取 measurement constraints、派生几何或 prepared | 按完整依赖与约束隔离，证明不足整页重读；Preferred／Minimum／Both 不能合并 |
| FA-R12 | engine 实例／revision、字体注册／移除／fallback 改变 | 受影响的边框或测量贡献失效；布局引擎段落缓存另按现行合同检查 |
| FA-R13 | 边框无声明，或 metrics 稳定但 renderer 内部状态变化 | 无声明整页重读；声明必须绑定 renderer 与读依赖，不能只缓存数值 |
| FA-R14 | 外部 border prepare_renderer／metrics 返回错误或非有限数值 | 原 cause／分类保持；无半成品 cache，新 snapshot 不发布，旧结果不突变 |
| FA-R15 | 真实 inline：前缀换行、多个同 offset box、child 超宽重测 | prefix layout、remaining width、child measure 与必要重测继续；box 顺序、baseline、命中与选择相同 |
| FA-R16 | callback 主动测量引发 cycle，或跨 root 访问失败 | 当前 cycle／权限错误原样传播；cache hit 不掩盖非法读取 |
| FA-R17 | 后一个 callback 改前一个声明或外部 provider；命中或构建检查中版本变化 | Flow 返回 owned 输入变化错误；分别断言不安装过期资产、不返回过期尺寸、不返回或发布过期准备结果；首次值不被覆盖，不降为 miss 或重试 |
| FA-R18 | 候选失败，下一次合法输入重试；某扩展 panic | 失败不替换成功 cache；原预算／fault／恢复合同保持，不承诺 source mutation 回滚 |
| FA-R19 | 回放已有同 subject 不同 generation；测量或 prepared 传递声明依赖 | 保留当前首次值与顺序，冲突或重验变化返回错误；完整 typed 证明进入消费与最终批次检查，分配先准入，失败不部分追加 |
| FA-R20 | 有效 asset、cache 驻留为零／留存已超限；检查或消费预算耗尽 | 已有 backing 不重复计费，零新增 charge 合法复用；正 charge 留存拒绝不回退为未计费 owned 输出；cache 不驻留与预算粘滞失败分别断言 |
| FA-R21 | cache 缩容后外部旧 lease 超过留存 limit，长期持有后释放 | 旧文本、glyph、来源继续合法；驻留下降但 backing 留存不虚降，超限可查询、正 charge 新增拒绝；释放或提高 limit 后恢复准入 |
| FA-R22 | 第三方 crate 自定义 contributor、payload、handler 和声明版本 | 不改 Core、同 typed 注册入口、同命中／失效／错误／容量合同；不得仅用私有 hook 验证 |
| FA-R23 | owned／shared 内容构造、非法 UTF-8 span、新 boxes／constraints、request clone | 同一公开受检路径；非法内容／boxes／constraints 拒绝，文本与 spans backing 共享；旧 owned 调用与外部 engine 编译及行为兼容，内容和 metadata 计费均断言 |
| FA-R24 | 局部返回检查通过后，其他候选回调在 Runtime 最终发布前修改 contributor／handler／border／输入域版本 | 首次证明随准备 metadata 进入最终统一 verifier；Runtime 返回同一 cause 的输入变化错误、整批不提交、旧成功快照不突变；不得仅断言 cache 未安装 |
| FA-R25 | measure 已私有返回给布局器，采用其 LayoutOutput 前外部版本改变 | 测量回执保留 proof；布局协调器消费检查拒绝输出，稳定查询返回错误而非过期尺寸；测量 cache 命中亦覆盖 |
| FA-R26 | proof reader 返回普通 Error、foreign／retired token、预算错误或协议错误 | 分类与原始 source 保持，不降为 input-changed 或 Ok(None)；检查失效 owner 后不调用已经退役的 reader |
| FA-R27 | 历史 cache key 与本次稳定首次输入不同；另一例为本次前后输入不同 | 前者是 miss 并可成功重建，后者返回输入变化错误；不以新值覆盖首次值后伪造一致 |
| FA-R28 | 最后 service clone 销毁，外部长期持内容／asset／snapshot／proof lease 后依次释放 | 新生产关闭；账本不依赖 Runtime／服务存活，旧读取合法；每个唯一 backing／metadata 最后释放时扣账，无用户回调、无保活环 |
| FA-R29 | 多个 service clone 同时持 cache／候选／旧 lease，缩容或采用跨责任域 backing | 同家族共享配置／账本，无容量倍增；同域唯一 charge 与分类重叠分别断言；跨域采用显式计入本域，不冒充全局去重 |

拟新增 integration target：`flow_assembly_reuse_contract`；独立消费者需在实施前
登记到既有消费者机制。目标尚不存在，不能声称通过或把拟议名称当可执行命令。
只读 lease、foreign binding 和未声明行为还需独立 compile-fail／正向编译用例，
在接口评审通过后冻结用例与公开 API 快照。
新增 FA-R24～FA-R29 及修订 FA-R17／R19／R20／R21／R23 均未执行；
实施启动前冻结实际注入点、typed verifier 和 public metadata 接口，
不把已有 30 项回归当作这些反例的通过证据。

## 性能与留存验收（待评审、未执行）

| ID | 检查 | 完成判据 |
|---|---|---|
| FA-P01 | 正确性分母 | 原 Large、单长段、NoWrap 长行与真实 inline 各自前后相同；输出字节／来源／glyph／commands 无解释外差异 |
| FA-P02 | 命中工作量 | contribute／process、组装 visit 与重建次数分别下降；命中验证的成员／依赖扫描另计，不要求消除所有 Figure 访问；cold／miss／不合格页面分别留分母 |
| FA-P03 | 净端到端成本 | 同配置、字体、预算、诊断模式，重新冻结实施前后；首次、追加、stable、scroll、resize、IR 分列 P50／P95／P99 和失败 |
| FA-P04 | 检查代价 | 组装遍历与验证扫描、声明 reader、完整比较、观察／proof 回放、最终 verifier、cache lookup 与 shared 内容消费分别计数／计时 |
| FA-P05 | 留存与峰值 | 分开报告 cache 驻留、全部存活唯一 backing／metadata、reservation／候选、当前帧／外部旧 lease、缩容超限；允许分类重叠，不重复求总量；allocator／RSS 与逻辑 charges 分开 |
| FA-P06 | 反例与耗尽 | source 变化、registry 更换、border／inline、预算耗尽与淘汰路径有同口径成本和正确性结果 |
| FA-P07 | 诊断扰动 | 生产比较使用相同观测边界，同时留 timing-off 对照；fine 与 coarse 差异不算复用收益 |
| FA-P08 | 回归裁决 | 大场景净收益和 cold／不合格页面回退均须评审；无法解释的尾延迟上升不宣称稳定改善 |
| FA-P09 | 目标与平台 | 16 ms 更新、4 ms 查询／IR 分开判断；本片 CPU 验收不关闭 GPU／present／平台或 ADR-059 C／D |

本矩阵不预设“13 次必降为 1”；实际有多少完整输入域和必要反馈轮次需由追踪确认。
量化的冷路径回退容限、容量默认值与生产启用条件须在实施前评审冻结；
不从某次本机波动补造统一阈值。

## 阶段证据与接受后效力（FR02-DR05／DR06）

| ID | 阶段 | 证据与退出要求 | 当前状态 |
|---|---|---|---|
| FA-G01 | 架构接受 | 评审共享 request 范围、typed proof 提交链、容量行为，以及独立 crate 通过公开入口接入的流程；无需先完成生产实现 | 待复核，不代表接受 |
| FA-G02 | 实施启动 | 用户实施授权；签名／metadata、容量默认值／charges／淘汰规则、target、冷路径回退容限和启用判据冻结；接受时同步 ADR-051 关联条款 | 未完成，不进入生产 Execute |
| FA-G03 | 实施完成 | 公开 API／消费者编译、FA-R／适用 FA-P 与既有回归；独立 crate 不改 Core 通过同注册、source 和 lease 通道 | 未执行 |
| FA-G04 | 生产启用 | 重新冻结匹配基线，净收益、峰值／容量与回退达标；关闭路径的未解释变化经评审裁决 | 未执行 |

当前不要求可编译原型，不把未授权的生产实现设为接受前置。
需要原型时先单列隔离范围与授权，不据原型结果关闭 FA-G03／G04。
提议期间 ADR-051 仍有效；接受后仅完整声明与证明通过的页面省略贡献执行，
其余页面继续重读，已经出现的输入／绑定／预算错误仍失败。
局部效力与接受时需同步的原条款见
[复用合同](../../design/architecture/flow-assembly-reuse.md)。

## 可执行的本轮诊断回归

现有 target 已包含公开消费者及边框／inline／失败反例：

```bash
cargo test -p novadraw --test long_text_layout_contract
cargo test -p novadraw --test p2_t03_structured_flow_contract
cargo test -p novadraw --test p2_t03_external_flow_engine
```

计时守恒检查由采样器逐 sample 执行，关闭路径不得发出 Runtime 阶段。
生产矩阵全部保留未执行；本轮诊断通过不能替代新协议的验收。
