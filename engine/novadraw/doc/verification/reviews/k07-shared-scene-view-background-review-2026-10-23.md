# K07 共享视图与背景方案评审

类型：`architecture-review`

日期：2026-10-23

状态：评审已记录；方案待修订，未批准实施。

评审对象：[共享视图与宿主背景草案](../../design/architecture/shared-scene-view-background-proposal.md)，
2026-10-23 的 `design-draft`。下文行号对应添加评审入口前的 140 行版本，修订时请按章节和原句定位。

范围：方案文档、现行架构契约和 Draw2D 只读源码。未扫描 Novadraw Rust 实现，
未运行实现测试。本报告记录本轮判断，不覆盖 design、ADR、parity 或 roadmap。

## 总体方向可以保留

建议认可方向，补齐关键契约后再进入实施。以下取舍可以保留：

- source Runtime 独占可变 Figure 树与 attached-state mutation authority。
- 跨 Runtime 传递 owned、不可变的完整快照；destination 独立导入资源。
- view 只读，默认组件与外部扩展共用同一模型、权限和错误通道。
- 宿主背景作为独立帧输入。
- 首批只观察 committed source state，动画 presentation 另行定义。

问题分类：P1 表示实施前需明确的正确性或协议边界；P2 表示详细设计需收口的职责或扩展边界。
以下均为设计缺口或契约歧义，不代表已验证的实现缺陷。

| 编号 | 优先级 | 问题 | 本轮状态 |
|---|---|---|---|
| K07-R01 | P1 | 捕获边界与主窗口 ancestor clip 混淆 | 待修订 |
| K07-R02 | P1 | 快照发布与帧准备协议关系未定义 | 待修订 |
| K07-R03 | P1 | 源身份、绑定身份和快照版本混合 | 待修订 |
| K07-R04 | P1 | 关闭传递与慢消费者规则不完整 | 待修订 |
| K07-R05 | P1 | 宿主背景尚未明确纳入统一受检提交 | 待修订 |
| K07-R06 | P1 | 多个资源 revision 共存规则不明确 | 待修订 |
| K07-R07 | P2 | 捕获策略与目标展示策略职责交叉 | 待修订 |
| K07-R08 | P2 | 视图依赖环的处理未定义 | 待修订 |

## K07-R01：捕获边界应独立于主窗口裁剪

位置：草案“坐标、裁剪与 damage”，原 L89–99。
原句：“resolved style、ancestor transform 和有效 clip 必须冻结”。

Draw2D `ThumbnailUpdater` 在独立图像中按捕获矩形平移、缩放，再调用
`sourceFigure.paint(tileGraphics)`。它没有沿 source 外部父链执行主窗口的绘制路径。
继承颜色和字体则由 `createTileGraphics` 显式取得。
按草案字面冻结外部 ancestor clip，主窗口 Viewport 外的内容可能从 overview 消失；
冻结外部缩放也可能影响缩略图比例。

修改建议：默认 Thumbnail 明确使用 source-local 捕获坐标，保留 resolved style、
source 子树内部变换和裁剪；外部 Viewport 的滚动、缩放及 surface clip 不自动带入。
如果另有消费者需要主窗口可见区域的捕获，应明确为独立策略。
沿用坐标 SSOT 的转换机制，不能复制手工绝对偏移算法。

验收条件：主窗口滚动或缩放后，overview 仍按约定显示完整 source；
非零 source rect、内部嵌套 clip 和非单位 DPI 保持正确。

## K07-R02：快照出口需明确修订帧协议边界

位置：草案“按因果顺序发布”，原 L55–67。

草案增加 Runtime 的公开 drawing 输出，而 ADR-031 限制 Runtime 对外返回未出版命令容器，
并将 `runtime.frames().prepare_submission(...)` 定义为唯一公开帧准备出口。
新的 scene capture 可以形成独立领域契约，但需要显式说明与 ADR-031 的关系，
不能全部留到实现核对阶段。

修改建议：定义 capture 复用同一个稳定化内核，同时明确它是否分配 source session/frame、
是否消费 source damage、资源 pending log 或确认 cursor。
建议 capture 不改变这些提交状态。明确 source surface 暂停、AwaitingCompletion、
source backend 失败时的捕获行为。捕获失败应只影响对应订阅，不能默认阻断主窗口提交。
继续只输出经过校验的 drawing 和资源闭包，不公开内部录制器或跨 Runtime paint 回调。

验收条件：捕获不会提前确认或清除 source 提交工作；destination 更新不依赖 source
Presented。暂停、在途提交、capture 失败及恢复都有明确的返回结果和调度行为。

## K07-R03：源身份与消费者版本应分开

位置：草案“单一源状态与只读视图”和“按因果顺序发布”，原 L44–48、L69–72。

`SceneSourceId` 同时包含 producer namespace 与 source binding generation。
同一 source 连接多个 view 时，各 binding 有自己的生命周期。
捕获矩形或 policy 改变，也可能在同一 source epoch 生成不同快照；
只按 source epoch 判重无法区分这些结果。

修改建议：分别定义源身份、订阅/绑定 generation、捕获配置 revision 和稳定场景 epoch。
明确各身份的所有者、比较范围和耗尽行为。
幂等判定覆盖实际内容身份与版本；同一身份版本携带不同内容返回明确错误。
destination resize 仅重新布局已有快照，无须制造新的 source epoch。

验收条件：一个源的多个绑定互不干扰；同 epoch 的捕获配置更新可被正确接受；
源替换后的迟到结果被拒绝；重复结果幂等，冲突结果报错。

## K07-R04：关闭事件与更新合并需要明确传递规则

位置：草案原 L46–47、L69–72、L103–104。

草案要求 source 释放后清空 view，但没有说明关闭事件如何可靠到达 destination，
也没有定义更新队列是否有界。source 已停止驱动时，不能依靠它再生成普通帧来通知关闭。
合并多个 epoch 后，最后一次更新的局部 dirty 也未必涵盖被跳过更新的全部变化。

修改建议：增加订阅状态机和宿主传递规则。初次订阅主动提供当前快照；
普通更新允许合并为最新值；关闭、源替换属于不能被普通更新覆盖的终止事件。
慢消费者不阻塞 source，也不无限保留历史快照。首批合并更新后直接修复完整 view
区域，避免引入未定义的跨 epoch 局部 damage 协议。

验收条件：慢或暂停的 destination 恢复后获得最新结果；
source 不再绘制时，关闭仍能使 destination 清空并擦除旧区域；
迟到更新不能恢复已关闭 view；合并更新不留下旧像素。

## K07-R05：宿主背景应与场景统一冻结和提交

位置：草案“宿主背景是帧输入”，原 L106–118。

草案给出了合成次序，但没有明确谁冻结背景、资源和 surface 输入。
若 Host 在 Runtime 准备后另行拼接背景，会绕过统一预检、retry 和确认协议。

修改建议：Host 提供候选背景，提交权威统一冻结背景、Canvas drawing、资源和 surface
版本。明确背景单独变化如何产生 pending work 和唤醒；Retry/device reset 如何恢复背景；
背景移除、失败和迟到结果如何处理。宿主坐标变换不得叠加 Canvas viewport origin。

验收条件：只更新背景也能生成正确帧；失败不会提交混合版本；
Retry/device reset 恢复背景和场景。分别验证 Canvas 在父宿主中移动与 Canvas 滚动，
并提供 opaque/transparent root、DPI 和 repair 对照。

## K07-R06：目标端需允许多个资源 revision 同时存活

位置：草案“资源闭包不能借用 producer cache”，原 L80–84。

两个 view 可以共享同一 revision，也可能分别停留在旧、新 revision；
旧提交还可能处于 in-flight。只更新同一个 destination ID 的最新 payload，
无法保证这些 drawing 都读到各自冻结的资源版本。

修改建议：明确不同来源 revision 映射到可同时存活的 destination 资源身份。
同一来源 revision 可以去重，不同 revision 不能相互覆盖。
CPU Registry、快照、binding 和 in-flight 提交的引用共同保证 payload 留存；
最后一个有效引用释放后才允许回收。

验收条件：同一 destination 的两个 view 分别使用旧、新字体或图像；
存在旧帧在途时触发 device reset，仍能恢复所需全部 revision。
导入失败不部分替换当前 view，旧 drawing 不读取新 payload。

## K07-R07：捕获策略与展示策略应按所有者拆分

位置：草案候选概念表，原 L48–49，以及 aspect fit，原 L95–99。

`SceneViewPolicy` 同时决定 source rect、aspect fit、clip 和 refresh 合并策略，
横跨 producer subscription 与 destination view。测量只描述了绘制缩放公式，
尚未明确 preferred/minimum size、尺寸提示及 source 比例变化后的 revalidate。

修改建议：source rect 和捕获更新合并归 producer subscription；
aspect fit、alignment、clip 和测量归 destination view。
可以复用值类型，但应分别明确运行位置、输入版本和错误通道。
补完整测量契约，默认和外部 policy 共用同一入口。

验收条件：调整 destination 展示策略不要求修改 source；
source 比例变化能正确触发测量与重绘，Border insets 只扣除一次。

## K07-R08：循环视图绑定需有确定性处理

位置：草案 source/view 依赖与绑定生命周期，原 L35–49。

A 捕获含有 B view 的场景，B 又捕获 A，可能形成持续产生快照的更新环。
当前没有定义这类绑定能否创建或如何终止。

修改建议：首批可明确拒绝循环绑定，并定义拒绝时的错误与状态保持规则；
不必为此建设通用依赖图调度器。

验收条件：直接和间接循环绑定被确定性拒绝，不产生残留订阅、资源引用或持续 wakeup。

## 接手修改时保留本轮证据

本次授权是记录评审意见；不表示用户已批准架构实施。
接手 agent 应先修订方案，逐项回应 K07-R01 至 K07-R08，
注明采纳、替代方案或不采纳理由，并关联修订后的章节与验收条件。
修订仍从需求、现行契约和 Draw2D 参考出发，不先扫描本项目 Rust 推导目标架构。

建议保留单链实施顺序，并把共享场景快照与宿主背景合成拆成两个独立决策。
两者复用资源闭包机制，分别验收；Thumbnail 通过不能替代背景协议验收。
先明确 ADR-031 等现行契约的修订范围，未经批准不提前标记 accepted 或 implemented。
执行队列仍只由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

本报告保留本轮原判断。后续响应和复核应放在新记录中并链接本报告，
不要将本轮“待修订”改写为已完成，也不要复制出第二份执行队列。

## 参考依据

- [ADR-031：Graphics 录制与帧出版边界](../../adr/adr-031-graphics-recording-and-publication-boundary.md)。
- [派生状态收敛](../../design/architecture/derived-state-convergence.md)。
- [资源因果与 Backend Session](../../design/architecture/resource-lifecycle.md)。
- [坐标与变换契约](../../design/coordinates/coordinate-system.md)。
- [官方示例剩余工作](../plans/draw2d-official-examples-next-slices-2026-10-23.md)。
- Draw2D 仓库：`/Users/bytedance/Documents/code/GitHub/gef-classic/`。
  参考 `org.eclipse.draw2d/src/org/eclipse/draw2d/parts/Thumbnail.java` 中
  `ThumbnailUpdater.run`、`createTileGraphics`、`getSourceRectangle`、
  `adjustToAspectRatio`、`getPreferredSize`、`notifyPainting`、`setSource` 和 `deactivate`。
  消费者为 `org.eclipse.draw2d.examples` 中的 `ThumbnailExample` 与
  `InheritBackgroundExample`。Java 只读，不编写、编译或运行验证代码。

## 记录与检查

本次将会话中的评审意见整理为独立记录，补充稳定问题编号、逐项验收条件和接手说明；
方案仅增加评审链接与结论摘要，评审索引增加入口。各问题保留原判断和风险程度。

交付前按“好好说话”规范复核事实、表达和节奏，并使用 Python 正则实际扫描：
中文半角标点、中文破折号、编号与量词标题、对比句式和空泛元话语均为 0 处。
未发现需要豁免的命中。
文档检查使用 `cargo xtask docs` 与 `git diff --check`；
这些检查只证明文档格式、链接及命令引用，不证明方案已经实施或运行行为正确。
