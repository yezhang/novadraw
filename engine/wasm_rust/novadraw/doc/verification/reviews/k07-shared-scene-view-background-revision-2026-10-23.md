# K07 方案修订响应

类型：`architecture-review`

日期：2026-10-23

状态：修订响应已记录，待复核；未批准实施。

依据：[原评审记录](k07-shared-scene-view-background-review-2026-10-23.md)。
对象：[共享视图与宿主背景方案修订 2](../../design/architecture/shared-scene-view-background-proposal.md)。

本记录说明如何处理 K07-R01 至 K07-R08，不替代独立复核。
原报告的优先级、问题描述和“待修订”状态保持原样；
本次只修订设计和导航，未扫描项目 Rust 推导架构，未执行实现测试。

## 逐项采纳与章节对应

| 编号 | 处理 | 修订章节与验收边界 |
|---|---|---|
| K07-R01 | 采纳 | [捕获坐标独立于主窗口](../../design/architecture/shared-scene-view-background-proposal.md#捕获坐标独立于主窗口)：默认 source-local 捕获，冻结 resolved style 和子树内部变换/clip；外部 Viewport 与 surface clip 不自动带入。主窗口滚动/缩放、非零 rect、嵌套 clip 与 DPI 分别验证 |
| K07-R02 | 采纳 | [与现行提交协议的关系](../../design/architecture/shared-scene-view-background-proposal.md#与现行提交协议的关系)：明确拟增加 ADR-031 的受检 SceneSnapshot 例外；capture 复用稳定化内核，不分配 session/frame、不消费 source damage、资源前缀或确认 cursor。暂停、在途、backend 失败与单订阅错误有独立结果 |
| K07-R03 | 采纳 | [身份与版本分别比较](../../design/architecture/shared-scene-view-background-proposal.md#身份与版本分别比较)：区分源、订阅、目标绑定、配置 revision 与 stable epoch；完整身份判重，同版本异内容报冲突。配置变化不伪造 epoch，同源配置切换不得倒退已安装 epoch |
| K07-R04 | 采纳 | [订阅关闭与慢消费者](../../design/architecture/shared-scene-view-background-proposal.md#订阅关闭与慢消费者)：初次主动捕获；有界最新更新槽和独立终止槽；生命周期立即交出 Closed，Host 接管确认独立于帧确认。合并更新后完整修复旧/新 view，迟到结果不能复活终态 |
| K07-R05 | 采纳 | [宿主背景纳入统一受检提交](../../design/architecture/shared-scene-view-background-proposal.md#宿主背景纳入统一受检提交)：Host 交候选，destination Runtime 统一冻结背景、Canvas、资源和 surface 版本。补齐背景单独唤醒、移除、失败、迟到结果、Retry 和 device reset；移动与滚动使用独立输入 |
| K07-R06 | 采纳 | [资源闭包与多版本共存](../../design/architecture/shared-scene-view-background-proposal.md#资源闭包与多版本共存)：不同来源 revision 使用不同 destination ResourceId，同 revision 去重；候选、快照、binding、CPU 导入表和在途 lease 保留资源。reset 先转交 CPU lease，再失效旧 token，保留所需全部版本 |
| K07-R07 | 采纳 | [展示策略与测量由目标拥有](../../design/architecture/shared-scene-view-background-proposal.md#展示策略与测量由目标拥有)：捕获配置归 producer，aspect fit/alignment/clip/测量归 destination。补齐无提示、单轴、双轴、显式 preferred、minimum、空尺寸和比例变化；insets 由 Core 统一扣加一次 |
| K07-R08 | 采纳 | [循环绑定在准入时拒绝](../../design/architecture/shared-scene-view-background-proposal.md#循环绑定在准入时拒绝)：composition-scoped Core 准入对象串行校验源依赖闭包，创建/重绑和后续 topology/config 变化都预检。直接/间接环在写入前拒绝，不建设通用依赖调度器 |

以上“采纳”只说明建议已写入修订稿，不表示验收已通过或问题经复核关闭。
方案的 [逐项验收与外部消费者](../../design/architecture/shared-scene-view-background-proposal.md#逐项验收与外部消费者)
保留八项待执行反例，默认与外部实现共用同一扩展模型、权限和错误通道。

## 两个独立决策

共享场景与宿主背景分别批准、固化 ADR 和验收。共同使用资源闭包，不把 Thumbnail
成功等同于背景提交协议成功；仍只观察 committed source state。
拟修订的 ADR-031、资源 lease/恢复、派生稳定化和宿主坐标输入边界在方案中显式列出，
现行 accepted 契约保持有效，本轮不提前改写。

测量规则补充了明确的 Rust 变体：无尺寸提示时默认自然尺寸来自 capture rect，
不依赖 destination 当前 bounds；minimum content 为零。该规则避免测量依赖布局结果，
需在后续复核和外部消费者中验证，不能声称逐字复制全部 Draw2D 分支。

执行状态只由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
本记录没有新增实施队列，也没有把原评审改成已完成报告。

## 文档复核

交付前按“好好说话”规范复核事实、术语、表达与节奏，并实际扫描中文半角标点、
破折号和对比句式。方案与本响应的中文半角标点、破折号均为 0 处。
方案命中 3 处“不是”表达，分别限定上游实现与 Core 协议、policy 权限和待执行验收
的证据性质；均为明确的技术边界，保留原义。响应记录未命中对比句式。
八项评审编号都有修订章节与验收映射，新增本地章节链接逐项校验通过。
`cargo xtask docs` 通过，检查 349 份 Markdown；`git diff --check` 通过。
`cargo xtask docs` 与 `git diff --check` 只验证文档格式、链接和命令引用，
不证明 capture、资源导入或背景合成已经实现。
