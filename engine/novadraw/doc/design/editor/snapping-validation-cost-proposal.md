# 开放吸附校验成本整改提案

类型：`proposal`

能力归属：Editor（`novadraw-editor`）。本提案不授权 Core 改动。

状态：待评审，2026-11-02。属于 E04 的性能续项，不新增并行执行批次；
现行契约仍以 [ADR-058](../../adr/adr-058-open-editor-snapping.md)为准。
行为实施和采样见[实施记录](../../verification/reviews/p2-e04-a-implementation-2026-11-02.md)。

## 问题与目标

prepared 命中只省略策略 prepare；每次回调前后仍重读全部 sibling、
可见性、坐标和模型／配置依赖。`Runner::validate` 还在版本回调两侧执行完整
owner 检查。当前 `validate_snap_input` 每次构造 owned scene 再比较，
因此索引 query 的成本不能代表整个事件。

正式采样覆盖 64／1024／10000 个 sibling、1／64 个移动源，分别测量
冷计划、热计划、反馈清理和整次 release。大场景存在明显延迟与分配；
实际命中时单源热计划 P95 在 1024 sibling 为 178.19 ms，
10000 sibling 为 2114.33 ms；最后一次求解记录 468 次 owner 检查和
6368 次版本回调。原始数据在实施记录中保留，不以 prepared 命中率宣称提速。
目标是减少重复分配与查询，保留失败前缀、完整来源验证、真实几何和一次 release。
本轮没有设定或验证统一帧率预算。

## 推荐的实施顺序

第一步只改变 Editor 私有实现，保留每个现有 owner 检查边界。
把“重建 owned scene 后比较”改为按序流式比较：同时核对完整成员、排除关系、
合法不参与、可见性、映射和四角。内部读取固定四角，只有公开
`SceneFeature` 或首次输入构建才生成 owned 集合。
流式验证仍遍历完整集合，不能跳过新增或隐藏的 sibling。
合法查询失败与数值错误保持原有 Error，首次失败立即拒绝。

第二步检查 registry、特征索引和 prepared 比较的临时集合，
在有界调用生命周期内复用容量。准备资产仍归手势；
失败尝试不得发布部分缓存，取消必须释放持有的 payload。
内置和第三方共用这些机制，不增设内置免检分支。
同时记录版本依赖的实例数、关联 payload 身份、实际比较次数与计时，
解释少量候选产生大量版本回调的原因。任何合并都必须先证明同一应用身份，
不能只按规则类型合并不同 payload，也不能省略扩展修改版本的检测。

第三步单独评估是否能减少完整 owner 扫描。
在拿到覆盖全部相关变化的可靠证明前，不用 ModelRevision、
InteractionRevision 或单个 viewport 值替代几何验证。
需要逐项证明模型发布、投影更新、policy 代际、成员增删、有效可见性、
content 映射、Runtime 几何及扩展资源变化如何使证明失效；
特别验证通过内部可变状态修改 child transform 的反例。
若现有 Core 契约不能提供所需证明，先按
[Editor 缺失 Core 依赖规则](../../../CLAUDE.md)核对 GEF→Draw2D 依赖，
再提出独立 Core delta；不能为了性能在 Editor 中跳过事实校验。
第三步的具体协议与实现需另行评审，前两步不预先批准它。

## 所有权与公开边界

| 项目 | Owner | 预期变化 |
|---|---|---|
| 流式 scene 比较与临时集合 | Editor Viewer／求解调用 | 私有实现，不新增公共输入构造入口 |
| prepared 容量与失败退休 | Editor SnapSession | 生命周期不变，计入 retained 分配 |
| provider／rule version 与错误 | 外部注册实例＋Editor 编排 | 每次必要回调及 source／panic 语义保持 |
| 实际几何、反馈与 Command | 应用计划＋Editor 受检提交 | 全部事实重验，清理先于 Command，release 零额外重试 |
| Runtime validation 上限 | Core | 本提案仅记录阻塞，不改为无限预算或绕过 stabilization |

10000 sibling 的实际 release 触发
`NonConvergingValidation { budget: 10000, .. }`。这是 Runtime validation
工作上限，不是递归深度或 preparation 预算。该反例不能单独证明缺少
GEF 必需的 Core 能力，需在独立调查中区分工作量与真实不收敛。
在解决前保留该档 release 未验收。

## 验收与退出条件

先冻结本轮源码、环境、预算和合成模型，复用同一采样入口做前后对比。
保持八次预热、十六次正式样本，单列 N、源数、候选数、采用数、
owner 检查、prepare／query、分配次数和请求字节；请求字节不解释为留存量或复制字节。
没有内存复制埋点时，复制成本明确标为未测，不从 allocator 推导。

回归必须包含 sibling 新增／隐藏／恢复、旋转和混合父、回调修改版本、
释放清理后失效、非有限坐标及 Error／panic。
第三方 provider、规则和 renderer 与内置策略执行相同反例。
比较应同时展示总耗时与分配，不能只报告 query 提速；
平台输入和最终 workspace 门禁仍按原矩阵单独完成。
任一变化削弱上述失败语义则回到设计评审，不以性能收益接受。
