# K07 修订 2 复核

类型：`architecture-review`

日期：2026-10-23

状态：暂不通过；剩余一项 P1 契约缺口，未批准实施。

对象：[共享视图与宿主背景方案](../../design/architecture/shared-scene-view-background-proposal.md)
修订 2。依据：[首轮评审](k07-shared-scene-view-background-review-2026-10-23.md)与
[修订响应](k07-shared-scene-view-background-revision-2026-10-23.md)。
行号对应本轮读取的 335 行方案正文，后续按章节定位。

范围：文档契约复核；未扫描 Novadraw Rust 实现，未执行功能或平台验收。
本记录不改写首轮评审，也不将方案复核等同于用户实施批准。

## 主要问题已补齐，循环准入仍需修订

修订 2 明确了 source-local 捕获、独立 capture 出口、完整版本身份、有界订阅交接、
背景统一冻结和资源多版本共存。原 K07-R01 至 R07 的主要设计问题已覆盖；
其实现正确性仍需后续验收。K07-R08 的循环准入尚未形成与现行 mutation/derived
commit 契约一致的保证，因此本轮暂不通过。

| 原问题 | 本轮文档判断 | 依据章节 |
|---|---|---|
| K07-R01 | 主要问题已覆盖；继承 alpha 建议显式补齐 | 捕获坐标独立于主窗口 |
| K07-R02 | 已覆盖 | 与现行提交协议的关系 |
| K07-R03 | 已覆盖 | 身份与版本分别比较 |
| K07-R04 | 已覆盖 | 订阅关闭与慢消费者 |
| K07-R05 | 已覆盖 | 宿主背景纳入统一受检提交 |
| K07-R06 | 已覆盖 | 资源闭包与多版本共存 |
| K07-R07 | 已覆盖 | 展示策略与测量由目标拥有 |
| K07-R08 | 未完全解决；本轮判为 P1 | 循环绑定在准入时拒绝 |

## P1：依赖成员变化不能只靠捕获后的检查

位置：方案 L247–264，尤其“每个 snapshot 声明其捕获中实际包含的 imported source
依赖闭包”和“这些操作在结构或资源写入前拒绝”。

依赖按实际捕获内容确定时，几何、布局、可见性或裁剪变化也可能增加依赖。
当前预检描述覆盖建立/重绑、topology、view 内容和 capture config，
却未说明这些几何与派生变化如何遵守写入前拒绝。
首轮仅要求定义循环处理，列为 P2；修订新增写入前拒绝的保证后，
该保证与实际提交路径的缺口影响正确性，本轮按 P1 处理。

反例：A 的捕获子树内存在 B view，但它当前位于 capture rect 外；B 又捕获 A。
若只登记实际捕获内容，A 此时没有到 B 的依赖。
普通移动或 LayoutOutput 将 B view 放入 capture rect 后，A 的捕获才出现到 B 的依赖，
从而形成 A 与 B 的循环。capture 阶段再报错虽然能拒绝新快照，
却无法兑现“导致循环的操作在写入前拒绝”的承诺。

现行 [派生状态收敛](../../design/architecture/derived-state-convergence.md)规定
LayoutOutput、callback 和直接 Runtime API 统一由 Runtime 提交；
[ADR-014](../../adr/adr-014-extensibility-and-lifecycle-boundaries.md)规定
派生失败不回滚已接受 source 输入。
composition root 持有独立准入对象并串行传递结果，还不足以说明这些 Runtime 内部
提交入口如何强制检查，也没有定义检查结果与实际提交之间的图版本有效性。
不能要求应用逐次手工协调布局输出。

建议首批采用保守的结构依赖：按已登记捕获根的整棵子树统计 imported view，
不因 rect 相交、可见性或 clip 暂时过滤而删除依赖边。
这样移动、布局和裁剪变化不会新建依赖；捕获根、拓扑、view 来源或配置改变时，
仍须由 Runtime 的统一提交路径强制准入。
明确 graph revision 与受检候选的绑定及过期拒绝规则，不能只依赖 Host 自律。
这是建议取舍；若选择精确可见依赖，则必须另行定义所有相关派生提交的预检机制，
并保持已接受 source 输入不回滚的现行边界。

补验收条件：rect 外的 view 被移动或布局到 rect 内、隐藏 view 显示、
clip 放宽、callback reparent、LayoutOutput 提交及过期准入候选。
所选模型必须一致解释这些变化，拒绝时不能留下部分更新的依赖图或资源引用。

## P2：捕获根的继承 alpha 应显式保留

位置：方案 L202–211，尤其 L205 只列出 resolved foreground/background/font。

[Figure 样式契约](../../design/architecture/figure-style.md)还规定 alpha 使用最近祖先值。
默认捕获排除了 source 外部绘制路径，应明确在捕获入口解析并冻结 resolved alpha，
之后沿子树应用 local override；不能丢掉继承值，也不能沿祖先链重复相乘。
方案已有“resolved style 属于同一稳定结果”的概括，本项是补齐显式列表和验收。

补验收条件：祖先 alpha 为 0.5，source 未覆盖 alpha 时，capture 保留该值；
子节点显式覆盖 alpha 后仍按最近值继承规则绘制。

## 后续处理

先补齐循环依赖模型和 Runtime 强制准入边界，再复核是否通过。
其他已覆盖条目不需要重写；共享场景与宿主背景仍分别固化决策和验收。
本轮不修改设计状态或现行 accepted 契约，不批准实施。

交付前按“好好说话”规范复核事实与表达，并实际扫描中文半角标点、中文破折号、
编号标题、对比句式和空泛元话语；结果见本次交付说明。
文档门禁只核验格式、链接和命令引用，不证明运行行为。
