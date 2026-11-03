# ADR 与专题设计的归属

类型：`archive`

能力归属：文档维护；Core、Editor、Inspector、后端的契约与既有批准保持原效力。

本页保存原编号与知识入口的对应关系。当前决策从
[ADR 索引](../adr/README.md)进入，运行协议从
[设计索引](../design/00-index.md)进入。分类依据见
[知识维护](../knowledge-maintenance.md)。

原稿按路径保存在
[维护前快照](../verification/evidence/adr-classification-2026-11-03/before.tar.gz)，
[manifest](../verification/evidence/adr-classification-2026-11-03/manifest.json)
记录字节数与 SHA-256。快照内的引用保留当时路径。

## 分类依据

ADR 记录影响长期边界的选择：解决什么问题、采用什么、为何不采用替代方案、
承担什么代价。公开 API 变更不自动构成架构决策；字段、公式、具体策略和工具行为
由设计正文负责。已接受方案的效力随内容迁移，不因移出 ADR 变成候选。

混合文档保留决策，运行契约归专题；过期实施步骤、评审回应和阶段计数从当前正文
退出，原始判断仍可从快照及既有验证报告追溯。执行顺序和未完成项归 roadmap，
验证方法归 verification/plans，某次结果归 verification/reviews 或原始证据。

## 逐篇归属

已复核 59 篇：8 篇具体设计迁出，17 篇拆出独立协议，另有 5 篇归并到已有设计。
ADR 目录保留 51 个决策或真实替代记录。接受效力、编号及原验证基线保持。

| 原编号 | 内容判断 | 正文归属 |
|---|---|---|
| 001 | 技术栈与可替换适配器选型 | ADR |
| 002 | 内部失效与提交后通知分离 | ADR；时序由 UpdateManager 设计负责 |
| 003 | Runtime 所有权、parent-local 几何与扩展边界 | ADR；各领域设计展开 |
| 004 | bounds/extent/scale 单一真源 | ADR；Layer/Freeform 设计 |
| 005 | 路由纯计算与 Runtime 原子提交 | ADR；Connection 设计 |
| 006 | Shape/Border 只读策略与 Graphics 边界 | ADR；Shape/Border 设计，交付范围归 roadmap |
| 007 | 排版与栅格化独立替换 | ADR；Text Layout 设计 |
| 008 | Manhattan/Fan 作用域与严格视口准入 | ADR；Connection 设计，M9 状态归 roadmap |
| 009 | 唯一运行期提交与 callback FIFO | ADR；mutation 与子树准入设计 |
| 010 | 观察者所有权与非重入自注销 | ADR；通知设计，局部修订由 057 决策 |
| 011 | 类型化稳定化与源/派生/呈现分离 | ADR；派生收敛设计 |
| 012 | 有序资源日志与显式 backend session | ADR；资源生命周期设计 |
| 013 | 已被替代的迁移承诺 | ADR 保留替代身份；旧全文归档 |
| 014 | 开放更新、分层原子性与生命周期修订 | ADR；组件/生命周期/测量设计 |
| 015 | 独立 Editor 与三种身份分离 | ADR；Editor 架构，产品交付归 roadmap |
| 016 | Inspector 独立只读诊断边界 | ADR；Inspector 设计 |
| 017 | Builder/Runtime 写权限与结构化失败 | ADR；公共 API 设计 |
| 018 | Runtime 驱动与领域测量类型 | ADR；公共 API/坐标/测量设计 |
| 019 | 提交权威与调用命名空间分离 | ADR；公共 API 设计 |
| 020 | 可执行 IR、受检基础值与产品边界 | ADR；Geometry/Graphics 设计 |
| 021 | 已被后续包与公开路径决策替代 | ADR 保留替代身份 |
| 022 | 第三方类型隔离与依赖归属 | ADR；依赖验证归 verification |
| 023 | 按真实安装/发布需求划分 package | ADR；目录与静态架构设计 |
| 024 | 统一 Paint/Stroke/clip 与能力预检 | ADR；Graphics 扩展设计 |
| 025 | 排版/轮廓/后端预处理分离 | ADR；文字与图形整合设计 |
| 026 | 源状态与动画表现正交 | ADR；Animation 设计 |
| 027 | owned typed descriptor 与开放发现 | ADR；Figure Capability 设计 |
| 028 | 属性行为身份与诊断显示分离 | ADR；typed 属性设计 |
| 029 | 成功处置、失败分类与恢复权威分离 | ADR；后端失败设计 |
| 030 | 平台加载、纯解码与 Core 发布仲裁分离 | ADR；资源生命周期设计 |
| 031 | 命令录制与帧出版权限分离 | ADR；Graphics 录制设计 |
| 032 | 模型编辑会话与宿主驱动权限分离 | ADR；Editor 会话设计 |
| 033 | Runtime 无状态子 facade 与统一 mutation | ADR；公共 API 与 facade 设计 |
| 034 | Tool/Request/Policy/Handle 同一开放协议 | ADR；Editor 扩展设计 |
| 035 | 唯一领域路径，删除库级 prelude | ADR；公共 API 设计 |
| 036 | 按阶段投影权限，Query/Snapshot 分离 | ADR；Context 设计 |
| 037 | 局部新增资源隔离，拒绝全局同步回滚 | ADR；Editor 失败隔离设计 |
| 038 | 身份、合法缺失与领域失败分离 | ADR；查询与测量设计 |
| 039 | owner-side 标签定位公式与兼容策略 | Connection 专题设计 |
| 040 | 完整组求解扩展与原子发布；具体 visibility 算法 | 决策留 ADR；组路由设计 |
| 041 | 五部件 ScrollBar、范围同步与活动调度 | Widget 专题设计 |
| 042 | 保留 Figure 来源身份、段落整体 shaping 与统一快照 | 决策留 ADR；结构化 Flow 设计 |
| 043 | 独立图模型、开放算法与 Figure adapter | 决策留 ADR；图布局/投影设计；实现过程归证据 |
| 044 | 独占可变源与不可变跨 Runtime 捕获 | ADR；共享场景设计 |
| 045 | 背景与 Canvas 统一冻结提交 | ADR；宿主背景设计 |
| 046 | 统一表现几何版本与计划容量预留 | ADR；Tree 动画设计 |
| 047 | 逐操作覆盖率与有限准备预算 | ADR；AA 设计 |
| 048 | 布局方向继承及文字片段定位规则 | Figure/Text 专题设计 |
| 049 | 低 miter 退化规则；Cocoa 样例零宽映射 | Graphics 设计；样例 profile 属消费者适配 |
| 050 | 按实际字体准备下划线片段 | Text/Graphics 专题设计 |
| 051 | 显式精确复用合同、共享资产与安全剪枝 | 决策留 ADR；Flow 复用与 glyph 存储设计 |
| 052 | 逐角轮廓、矩形分边区域与命中 | Shape/Border 专题设计 |
| 053 | 具体 Vello Scene 输出、DPI 与目标发布协议 | 后端专题设计；包边界沿用 023 |
| 054 | 模板创建、框选与应用接入行为 | Editor 专题设计；开放分派沿用 034 |
| 055 | 应用原子模型命令与 Editor 受检容器计划分工 | 决策留 ADR；容器编辑设计 |
| 056 | 在原始输入边界保护不变量 | ADR；Detached 输入设计 |
| 057 | 领域失败、观测故障隔离与恢复 checkpoint | 决策留 ADR；Core/Inspector/Editor 分 owner 设计 |
| 058 | Editor 吸附归属与统一策略/仲裁模型 | ADR；开放吸附设计 |
| 059 | 可选阶段合同、实例资产与精确完整回退 | 决策留 ADR；阶段调用/资产计费设计；启用门禁归验证 |

编号不重排、不复用。迁入设计的编号只作为历史来源，不继续创建同编号的 ADR 正文。

## 拆分后的协议入口

8 篇完全迁出的路径见
[原编号映射](../adr/README.md#已迁入设计的原编号)。混合文档的当前协议如下；
对应 ADR 只维护选择、理由和代价。

| 原编号 | 完整协议 |
|---|---|
| 009 | [Mutation 与子树准入](../design/architecture/runtime-mutation-and-subtree-admission.md) |
| 029 | [backend 失败与恢复](../design/rendering/backend-failure-and-recovery.md) |
| 030 | [资源加载与请求](../design/architecture/resource-loading-and-request-tokens.md) |
| 031 | [Graphics 录制与出版](../design/rendering/graphics-recording-and-publication.md) |
| 032 | [Viewer 会话与 drive](../design/editor/viewer-edit-session-and-drive.md) |
| 033 | [Runtime facade](../design/architecture/runtime-subsystem-facade.md) |
| 034 | [Tool/Request/Handle](../design/editor/tool-request-handle-protocol.md) |
| 036 | [Context 阶段权限](../design/architecture/context-phase-authority.md) |
| 037 | [Editor 增量失败隔离](../design/editor/incremental-failure-isolation.md) |
| 038 | [查询身份与失败](../design/architecture/query-identity-and-failure.md) |
| 040 | [组求解与 visibility](../design/architecture/group-route-solving.md) |
| 042 | [结构化 Flow](../design/architecture/structured-flow-layout.md) |
| 043 | [图模型与算法](../design/architecture/directed-compound-graph-layout.md)、[投影准备](../design/architecture/graph-projection-preparation.md) |
| 044/045 | [共享场景与宿主背景](../design/architecture/shared-scene-view-background-proposal.md) |
| 046 | [Tree 表现几何](../design/animation/tree-layout-presentation-proposal.md) |
| 047 | [逐操作 AA](../design/rendering/shape-antialias-proposal.md) |
| 051 | [精确布局复用](../design/architecture/exact-flow-layout-reuse.md) |
| 055 | [容器编辑](../design/editor/container-editing-protocol.md) |
| 057 | [服务/观测/Inspector 恢复](../design/architecture/service-failure-and-inspector-lifecycle.md) |
| 058 | [Editor 吸附](../design/editor/open-snapping.md) |
| 059 | [阶段文字引擎](../design/architecture/staged-text-width-reuse.md) |

样例方案中的方向、低 miter 和下划线改为引用专题；
未批准的[设备 hairline](../design/rendering/device-hairline-proposal.md)独立保留候选身份。
长文共享 IR 与宽度复用各由单一专题负责，旧细粒度失效表限定为未来候选；
采样分母、计时和统计方法归
[验证计划](../verification/plans/long-text-benchmark-protocol.md)。
测试计数、迁移流水及逐项回应退出当前协议，原事实从上方快照或既有报告追溯。

## 维护核验

维护前快照共 418 个文件，逐项字节数与 SHA-256 匹配，包含 59 篇 ADR 原稿。
迁移后保留 51 篇 ADR，新增的 26 篇设计及 1 篇验证计划均能从当前总入口到达。
当前引用已重定向，冻结证据内部的历史路径保持原样。
文档门禁、改动范围的章节锚点及 diff 空白检查通过。
本次仅调整知识归属，不以文档维护宣称新的实现或平台验收完成。
