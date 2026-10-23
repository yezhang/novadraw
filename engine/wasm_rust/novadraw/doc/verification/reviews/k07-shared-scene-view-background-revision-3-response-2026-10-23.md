# K07 修订 3 响应

类型：`architecture-review`

日期：2026-10-23

状态：方案已修订，待复核；未批准实施。

依据：[K07 修订 2 复核（暂不通过）](k07-shared-scene-view-background-revision-2-review-2026-10-23.md)。
对象：[共享视图与宿主背景方案修订 3](../../design/architecture/shared-scene-view-background-proposal.md)。
本记录回应本轮剩余 P1 和 alpha 补充项；前两轮评审与修订响应保留原判断。
本次没有扫描项目 Rust 推导架构，也没有执行功能或平台验收。

## 循环准入改为结构依赖

采纳复核建议。依赖成员取已登记捕获根的整棵子树，包括根自身，view 的 rect 相交、
可见性、尺寸和 clip 不过滤依赖边。接受保守模型可能拒绝始终不可见 view 的结构环。
因此，A 子树中的 B view 在 rect 外时已经形成 `A → B`；
再建立 `B → A` 会在绑定提交前被拒绝。普通移动、LayoutOutput、显示或 clip 放宽
只改变绘制，不增加结构依赖，避免“移动已提交后 capture 才发现环”的缺口。

Runtime 持有同一 Core binding group 的受限提交句柄。直接 API、callback 延迟 Effect、
typed update 与 derived output 都经过统一 Runtime 提交器；
改变捕获根、topology 或 view 来源的候选须在写入前整批验证。
仅改变几何的 LayoutOutput 使用现有原子输出提交，不要求应用手工协调图更新。
获准关系 effect 必须走同一准入，不能由外部或内置 LayoutManager 直接修改图。

Core 准入候选绑定 group identity、`base_graph_revision`、相关 Runtime generations
和完整操作。提交点在 group 独占保护下重验版本并检查候选图；
过期候选返回 `StaleSceneAdmission`，循环候选返回 `CyclicSceneViewBinding`，
均不能留下部分源结构、图边、资源或 lease 更新。
通过后才在同一提交保护中安装本地结构与图边，再生成 committed facts。
这一保证由 Runtime 强制，不依赖 Host 把早先“检查通过”当作当前许可。

源操作预检失败不写该操作，FIFO 已提交前缀保留；
derived output 失败丢弃完整候选并保留 dirty work，不回滚先前接受的 source 输入。
历史 snapshot 图版本仅作为准入来源记录，安装须依据当前版本重新预检；
不会因图更新伪造新的快照内容身份。

对应方案章节：
[循环绑定在准入时拒绝](../../design/architecture/shared-scene-view-background-proposal.md#循环绑定在准入时拒绝)、
[Runtime 统一提交强制准入](../../design/architecture/shared-scene-view-background-proposal.md#runtime-统一提交强制准入)和
[图版本与候选在提交点核对](../../design/architecture/shared-scene-view-background-proposal.md#图版本与候选在提交点核对)。

待验收：rect 外 view 移入、hidden view 显示、clip 放宽、callback reparent、
LayoutOutput 的几何提交与关系 effect、捕获根/来源变更、过期候选及跨 Runtime
交错提交。验证准入拒绝不部分更新图或资源，派生失败不回滚已接受源输入，
也不生成持续 wakeup。以上仅为修订后的验收条件，不是已验证实现。

## 捕获入口显式保留 alpha

采纳复核建议。在
[捕获坐标独立于主窗口](../../design/architecture/shared-scene-view-background-proposal.md#捕获坐标独立于主窗口)
中将 resolved alpha 加入入口冻结列表。样式在捕获入口解析一次，再沿子树应用
local override。祖先 alpha 为 0.5、source 无覆盖时保留 0.5；
子节点覆盖为 0.8 时，该分支及未覆盖后代使用 0.8，不与祖先值相乘。
alpha 变化和几何、资源使用同一稳定快照，并触发重新捕获。

K07-R01 验收表已补齐这一反例。R02 至 R07 的已覆盖契约保持原有设计，
共享场景与宿主背景仍分别批准和验收，状态只由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 文档复核

交付前复核结构依赖不变量、提交版本校验与最近值 alpha 继承，并按“好好说话”规范
检查表达、术语和节奏。方案及本响应的中文半角标点、中文破折号、编号标题、
对比句式和空泛元话语扫描均为 0 处；新增本地章节链接校验通过。
`cargo xtask docs` 通过，检查 351 份 Markdown；`git diff --check` 通过。
文档检查不证明方案已通过独立复核、已获实施批准或运行行为正确。
