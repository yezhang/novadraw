# S09 现状盘点与设计复核

类型：`architecture-review`

日期：2026-10-30

能力归属：Core 的 Animation、Connections 和 Observations；Inspector 的本地故障与订阅；
Editor 的 Command／Policy 错误。三类改动分别验收。

状态：已完成定向源码盘点与设计自审，形成 ADR-057 修订 1。
本轮先完成方案复核和实施准备；尚未修改 Rust 或进入 Execute。
没有独立 reviewer，以下“已核对”只表示源码与契约核对。

基线：`9825957efb1859ffca504e03eafd79ff19a226bd`。
不扫描渲染主循环、不扩大长文本或 Editor 功能范围。
目标见 [ADR-057](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-057-service-failure-and-inspector-lifecycle.md.txt)，
退出条件见 [验证矩阵](../plans/p2-s09-service-failure.md)。

## 实际入口与缺口

| owner／入口 | 当前行为与源码 | 最小处理 |
|---|---|---|
| Core：`AnimationMut::set_mode` | `animation/service.rs:2763` 返回 bool；fault 和同值都返回 false | 返回领域 `Result<bool, AnimationError>`，健康检查先于同值短路 |
| Core：surface suspension | `animation/service.rs:2695` 与 `:1409`；没有健康检查，返回值只累计轨道终止，服务 suspended 值改变也可能返回 false | 私有入口 fallible；配置变化与 presentation damage 分开核算 |
| Core：behavior processing | `animation/service.rs:2705`、`:1500`；普通工厂／plan 拒绝进入有界队列并继续，panic 清理 presentation、标 fault 后 resume unwind | 保留普通拒绝队列与成功前缀；只将服务健康失败显式传播，不能改成整批回滚 |
| Core：帧调用方 | `runtime/runtime/frame_submission.rs:741–824` 忽略上述 bool | 显式处理服务错误，映射已有 `FramePreparationError::Animation`；不把异常报告为 Idle／Suspended |
| Core：Anchor／Router 注册 | `runtime/runtime/service_facades.rs:302–345` 同时有直接 register 与 try_register；`connection_service.rs:15–107` 的直接版本 expect | 一个 fallible register；迁移调用方，删除同语义 panic alias |
| Core：注册准入 | `connection/runtime.rs:625/:736` 将对象存入 RuntimeArena，没有对象校验回调 | 按实际 fault 准入与身份条件验证；不虚构 invalid Router／Anchor 校验接口 |
| Core：observation 注册 | `service_facades.rs:943–956`；`runtime.rs:2786–2804` 的 Runtime 简写 expect，而 scoped 版本可返回 fault／scope 错误 | 简写与 scoped 均 fallible；两者参数语义不同，不删除 scoped |
| Core：listener 注销 | `runtime.rs:2806–2811` 在 fault 时直接 false；`update/deferred.rs:291` 移除 owner 与各类 listener | foreign 返回领域错误；本域缺失返回未变化；fault 后允许无 callback 清理 |
| Core：listener 身份分配 | `update/deferred.rs:303–309` checked_add 后 expect；各类 listener 共用此分配器 | 分配耗尽在发布 ID 与登记前拒绝；不能只修 observation 而保留同分配器的 panic 路径 |
| Inspector：attach／读取／清空 | `novadraw-inspector/src/lib.rs:157–203`；attach 只给 ListenerId，events poison 返回空，clear poison 被忽略 | attach 给 owner token；events／clear 返回结构化本地错误 |
| Inspector：observer | 同文件 `:210–223`；锁失败仍 Keep，无首 fault 状态 | 独立首 fault 存储，记录未采集位置，返回已有 Remove directive |
| Inspector：缓冲 | 同文件 `:126–138`；push 后超容量才 pop | 逻辑 FIFO 已有界；实施时先淘汰后 push，测实际 VecDeque 容量与复制成本 |
| Editor：Command／Policy 原因 | `command/mod.rs:14–54` 与 `policy/mod.rs:238–262` 只存 String，Error 没有 source | 增加 owned source；保留分类与显示；不要求外部错误 Clone／Send／Sync |
| Editor：外层 source | `CommandStackError::source`、`EditorDomainError::source` 和 `ViewerError::source` 已存在 | 补叶层原因，回归多层 downcast；不是从零重建错误链 |
| Editor：panic 诊断 | CommandStack 只记 operation／label；Viewer 的 `panic_message` 保留文本，其他退化为通用字符串 | 明确文本／非文本分类；不对任意 Any 执行用户格式化 |

行号只定位本轮基线。实施前以符号名和源指纹再次确认，不把行号当长期契约。

## 必须修订的设计边界

**S09-DR01：普通 behavior 拒绝不等于服务故障。**
一个工厂拒绝后，其他工厂仍可成功启动计划；失败队列也会变化。
因此私有 processing 的成功 bool 只能表示 presentation 是否变化，
不能宣称 `Ok(false)` 代表整个服务一字节未变。
普通拒绝保持原有诊断队列，不重放已消费事实，不撤销前面的成功计划。
工厂 panic 仍遵循既有 Runtime fault 与 presentation 清理。

**S09-DR02：surface suspension 的返回值不能冒充配置未变化。**
当前没有 active track 时，suspended 配置改变却返回 false。
需要在 service 层区分配置变化与终止结果；damage 继续按实际 old／new presentation 比较。
帧结果仍由 surface 是否可绘制及服务处理结果决定，不因 bool 改名改变平台提交协议。

**S09-DR03：订阅生命周期不能只改 Inspector。**
attach 必须避开 fault 下的注册 panic，detach 必须真的移除 faulted Runtime 上的登记。
shared allocator 的耗尽也必须先拒绝再登记，内部 owner 表不能留下孤立项。
八类 listener 的简写入口共用 fault／scope 准入和分配器，迁移范围按共享机制确定，
不是按 `try_` 名称批量删除。

**S09-DR04：注册测试必须有实际拒绝条件。**
当前 Anchor／Router 对象注册没有数值输入校验，route 的输入错误属于后续计算阶段。
SF-09 改为已有 fault 准入与身份行为，不为了制造负测新增 validate hook。
RuntimeArena 的通用容量／代际策略与 listener 的显式 u64 计数器分别记录；
不在本批重写所有 arena，也不把资源耗尽解释成普通路由输入拒绝。

**S09-DR05：错误 source 与相等比较必须分开。**
CommandError／PolicyError 及部分外层当前派生 Eq／PartialEq。
任意外部 Error 没有可靠的值相等协议；不能按 message 或 Arc 地址伪造原因相等。
支持 source 后，仅迁移受影响类型的相等比较，消费者断言改用分类、位置、
原始原因 downcast 和显式字段。文本构造仍可保留，Clone 可共享 owned source。

Inspector 多订阅和 Clone 当前共享一个缓冲；本轮不默默增加单订阅限制。
下一片必须明确首 fault 对共享实例的影响、每个 token 的 owner 与独立注销，
以及多 Runtime 记录的来源标识；不承诺跨 Runtime 的全局 sequence 顺序。

上述 DR01–DR05 已写入修订稿及矩阵，属于静态自审通过；
新增行为未运行，不能据此关闭来源审计项。

## 首个实施切片

S09-A 只修改 Core Animation 及直接帧调用方：

1. `set_mode` 健康检查先行，成功区分同值／变化，fault 返回 AnimationError。
2. suspension 与 behavior processing 使用 fallible 私有入口，保留原普通拒绝与 panic 行为。
3. frame preparation 接收领域错误；补齐该 Animation 分支的 Error source。
4. 定向迁移 Core animation tests、官方 Tree 和 animation-app 等调用方；
   不因名字同为 set_mode 而修改 Logic 应用的工具切换。
5. 公开 fault／同值消费者、空 effects 私有健康检查、混合成功与拒绝、
   surface reason、清理与 damage 回归完成后，才进入后续 Inspector 片。

第二片包含 Observations 基础改动与 Inspector 生命周期，第三片收口 Connections 注册，
第四片补 Editor source。各片可以定向验证，但 S09 整批关闭仍需最终公共 API、
workspace 和文档门禁。

## 已执行基线

本轮运行既有 `p2_animation_behavior_contract`：7 项通过；
运行既有 `figure_inspector_contract`：2 项通过。
前者已覆盖拒绝队列有界、scope dispose、事实不重放与 panic 清理；
后者只证明健康状态下快照、FIFO、逻辑容量和清空。
九项通过不证明新的 fault／同值／poison／fault detach 合同。
源指纹与原始日志见 [基线收据](../evidence/p2-s09-design-2026-10-30/receipt.json)。

未修改 Rust，未运行 workspace 全量测试或性能比较。
实施成本沿用 ADR：同值成功路径、入队与满容量、失败路径分别测量；
不消费 Inspector 的产物与实际消费 Inspector／Animation 的产物分别比较。

本报告使用技术文档清晰度与好好说话复核事实、逻辑和中文；
机械扫描只定位表达候选，不替代设计接受或行为验证。
