# Editor 开放吸附验证矩阵

类型：`verification-plan`

能力归属：Editor（`novadraw-editor`），验收目标为 GEF 开放吸附行为。
Core 基础协议的使用作为依赖验证；平台输入与应用消费者证据分别归属，
不将 Core 回归通过或应用网格实现计为 Editor 吸附完成。

状态：修订 2 已接受，E04 实施中，当前补 E04-C；2026-11-02。对应
[ADR-058](../../adr/adr-058-open-editor-snapping.md)与
[专题](../../design/editor/open-snapping.md)。首段结果见
[E04-A 实施记录](../reviews/p2-e04-a-implementation-2026-11-02.md)，其余要求仍待执行。
依赖源码与修订原因见
[本轮依赖核验](../reviews/adr-058-dependency-and-design-review-2026-11-02.md)。
评审意见的逐项修改见
[修订回应](../reviews/adr-058-revision-2-response-2026-11-02.md)；
下表保留完整验收要求；局部通过不能代替整行场景毕业。
设计结论见[修订 2 复核](../reviews/adr-058-revision-2-review-2026-11-02.md)，
复核通过后，用户的条件实施授权已满足；行为结果按实际执行补录。

| ID | 场景 | 必须观察 |
|---|---|---|
| SN-01 | 外部 typed 策略与三个内置策略 | 相同注册、候选、仲裁、反馈与错误通道；第三方与 Grid 共用阈值捕获／精确投影准入，无类型免检分支 |
| SN-02 | 优先级、精确平局、空候选、耦合候选 | 排序确定；每自由度一次修正；耦合整体接受／跳过 |
| SN-03 | 负坐标、网格中点、非法间隔／阈值、格距 100／半径 8／原始坐标 40 的显式量化 | 确定取整；捕获模式不接受到 0 的超半径候选，精确投影可接受并验证最近格点；非法／越权仍拒绝 |
| SN-04 | 混合父多选和祖孙选择 | 唯一锚点；全批同一 surface 平移；源子树排除 |
| SN-05 | 嵌套 content pane、zoom／scroll／DPI 1／2、非均匀缩放与旋转 | 完整映射和真实角点；不以 AABB 或 zoom 标量代替；surface 阈值稳定，layout 不可表达时拒绝 |
| SN-06 | 无 pointer move 的 auto-expose／zoom／resize | 当前版本重算；release 不使用旧反馈 |
| SN-07 | 目标删除、模型或策略配置变化 | 模型／权限变化沿用 E03 取消；独立配置可使输入重算；不把新模型当旧手势基线 |
| SN-08 | resize 固定边、比例和最小尺寸 | 候选遵守自由度，拒绝不改变模型 |
| SN-09 | create／drop、XY→Ordered 换父、布局持续改写吸附位置 | 共用坐标；不误查移出资格；布局约束进入输入，实际计划破坏命中则有限拒绝，无无限重算或伪命中 |
| SN-10 | 预览、释放、undo／redo；最后 move 在 A、release 在 B 且版本相同 | release 使用 B 和最新 E03 完整计划；每 release 一次必要流程、零额外重试；实际计划／采用事实／反馈一致，一次 Command |
| SN-11 | cancel／capture lost／tool switch 与清理失败 | 无残留参考线；清理拒绝先于 Command 构造 |
| SN-12 | 外部错误、非有限输出、panic | source 保留；整次候选失败；panic 遵循 Viewer fault |
| SN-13 | guide attachment 和网格显示／启用 | 临时反馈不写业务模型；显示独立于启用 |
| SN-14 | N=64／1024／10000，单选／64 项批次 | 记录重建、K、坐标查询、分配、复制、P50／P95 |
| SN-15 | Native／Wasm 未消费、内置、第三方三组 API 与依赖边界 | 领域导入、外部 payload／Error 不新增 Clone／Send／Sync 要求；体积只在用户明确要求时测 |
| SN-16 | prepare／query 前后变化、release 最新位置、释放流程中再次变化 | 不能因版本相同复用 A 点 receipt；一次必要释放流程，变化后拒绝而非额外重试；清理后仍失效不得构造 Command |
| SN-17 | sibling 查询失败、合法不参与、外部操作边界 | empty 与 Error 分离；source 可追溯；默认和第三方 provider 共用 registry，不走 Option 吞错 |
| SN-18 | 斜向／比例耦合、后续修正破坏此前对齐、独立轴对角修正、稳定平局 | 写入集／依赖集与可求值约束共同检查；冲突跳过较晚候选；最终实际计划重验全部事实，反馈无失效命中 |
| SN-19 | 多选 Guide、不同 Guide 资源 owner | 唯一锚点驱动 batch；模型 Guide 变化取消，独立资源更新重算；attachment 只由应用 Command 决定 |
| SN-20 | sibling 新增／隐藏／恢复、provider 替换、完整来源验证与模型投影 | 按依赖表核对取消／prepare／query；新增集合成员使旧索引失效；分别记录来源扫描、重建和 query 成本 |
| SN-21 | 有界特征／候选／反馈预算及外部回调 | 内置与外部相同预算，超限整次拒绝，无截断成功或 partial receipt；不宣称可中断任意回调 |

公开独立消费者证明正常生命周期；故障注入不增加公共测试后门。
Native／Web 必须通过真实 capture、DPI、cancel 和无 pointer move 版本变化，
无窗口 Tool 测试不能替代平台输入验收。
定向 suite 名在实施时使用真实 xtask 入口，不预造 suite ID。
最终 workspace、API、文档和性能证据完成后再提升 `snap.guides` 的采用状态。

## 分片门禁

E04-A 验证 SN-01～03／12／16～18／21 的纯协议与受检输入；
E04-B 增加 SN-04／05／07／13／14／19 的内置和外部策略；
E04-C 以真实 Tool／Policy／Command 验证 SN-06／08～11／16 的失败前缀；
E04-D 完成平台、API、SN-14／20 的计数和 P50／P95，以及最终复核。
同一用例可以在不同层补证，但统计须分别列出，不能重复累加为独立测试数。

2026-11-02 的当前证据如下，完整用例及测试入口见
[实施记录](../reviews/p2-e04-a-implementation-2026-11-02.md)：

| 范围 | 已有证据 | 剩余 |
|---|---|---|
| SN-01～03／12／17／21 | 32 项协议／策略测试和 20 项公开 integration 覆盖注册、仲裁、准入、预算、查询失败及八类回调；另有 renderer source／panic、生命周期和整组反馈预算证据 | 完整反馈矩阵复核 |
| SN-08／18 | 比例网格退化方程；resize 八方向固定边、比例角、最小尺寸、旋转映射与实际计划重验；第三方 limits／Handle 及一次 Command、undo／redo | 完整矩阵复核；居中／多源 resize 不在本切片 |
| SN-09／10／11／16 | 容器 integration 46 项通过，验证开放布局规则、实际计划改写拒绝、缺失几何、多源失真、release A→B、一次历史、undo／redo、清理失败和 renderer 拒绝；创建固定起点／尺寸、zoom refresh 和缩放／旋转 drop；移动静止指针重算、取消／切换／模型发布、创建布局规则与 release query 配置失效 | 平台 capture lost、其余完整矩阵组合 |
| SN-04～07／13／19 | 原有 E03 容器组合、纯策略真实边及 Guide 唯一锚点证据；吸附混合父缩放／旋转与静止指针 scroll／zoom／resize 的真实移动 Command | 多选 Guide 资源联合矩阵及平台输入 |
| SN-14／15／20 | 独立消费者 Native／Wasm 编译包含移动／创建／resize 计划、第三方 limits 与 ReferenceLineRenderer，prepared 增删和配置失效测试通过；六组无窗口正式采样完成，含实际命中、阶段 P50／P95、分配及 owner／版本计数 | 三类 API 边界、坐标查询／复制／完整阶段计时；10000 sibling 完整 release 的 validation 预算阻塞、成本整改及平台采样 |

各行均保留未完成状态，不用局部通过关闭整行。

注册、替换和注销至少覆盖 healthy／foreign／retired／faulted，
未准入不得执行策略 callback。策略注册只是登记，不提前 prepare 或构造业务 Command。
receipt 字段不可由外部构造；编译失败用例证明外部不能伪造已检查输入或结果。
模型 revision／权限代际的注入使用已有公开变化或隔离私有注入，
不为吸附新增公共故障后门。

SN-14／20 记录冷准备与热查询各自的正式样本、预热和环境，
并核对 N、K、重建次数、源父链／特征扫描量、分配、复制、反馈和命令数。
稳定化和既有全模型 projection 的成本单列，不能从候选微基准推导整体性能通过。
未建立匹配基线前不宣称提速；外部回调的任意计算和自建资源不受框架预算强制中止。

## 修订 1 评审的定向反例

以下是假设场景的待执行要求，不是已复现缺陷：

- 释放位置：A 点 move 后在 B 点 release，不改变任何版本；断言目标、候选和
  E03 计划都使用 B。再注入首次释放 query 内配置变化，断言零额外重试、无 Command。
- 准入规则：相同 Grid 及第三方投影分别用捕获／精确投影绑定，
  验证超捕获半径的精确结果只在获授权投影规则下通过；
  自行更换准入身份、非有限值和固定边越权仍为错误。
- 对齐约束：较早事实为 `dx + dy = 0`，较晚修正拟写入 dy；
  断言完整依赖集阻止破坏。另用外部求值器验证暂定合成和最终布局求值均被调用，
  假设最终布局破坏该式时断言整体拒绝，无旧参考线或业务提交。
- 布局终点：布局持续将吸附位置改回另一位置，断言每次流程只取得一次完整计划，
  实际事实不成立则拒绝，不再求解相同冲突，也不退成无吸附成功。
- 缓存边界：模型新增 sibling 取消旧 E03 手势，新手势重新准备；
  独立 provider 新增候选、隐藏特征或替换 provider 时，
  来源仍有效的手势重新 prepare。共同 viewport 映射变化且相对特征几何不变时，
  才允许仅 query；不能证明相对不变则重新 prepare。
