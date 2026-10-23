# 官方示例剩余工作

类型：`verification`

日期：2026-10-23

范围：25 个入口的 Canvas / LightweightSystem 能力。执行队列仍只由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 还有 11 个入口未收口

当前 catalog 为 14 complete、8 in_progress、3 blocked，22 个可运行。
下表将待验证的消费者行为和确实缺失的 Core 能力分开记录。
完成判据取自 [Canvas 收口清单](../../../examples/native/draw2d-official-examples/completion-contracts.tsv)，
不以可运行、测试数或一幅截图代替完整验收。

| 入口 | 剩余工作 | 能力批次 |
|---|---|---|
| FlowLayoutExample | 11 个 Shape 的颜色、轮廓已移植；继续验证窄容器换行、双轴 alignment/spacing、stretch、方向切换后的尺寸恢复、窗口 resize 与旧新 damage | K02/H04 |
| GridLayoutExample | 本批接入窗口 resize；继续验收真实面板的 span/grab/hints/indent、动态 Add/Clear 与选择释放、缩至最小空间后的 geometry/damage | K01/K02/H04 |
| ToolbarLayoutExample | 5 个 Shape 的颜色、轮廓已移植；本批验证方向切换、stretch、收缩和恢复；窗口 resize、minimum 边界及 damage 仍待验收 | K02/H04 |
| PrintExample | Canvas 组合、原始 ICO 和通用命令输出已验证；三个上游系统字体目前均以 Inter 替代，需宿主字体来源及真实字体指标证据 | K09/H01/H04 |
| ScrollPaneExample | 五部件 ScrollBar 已由 ADR-041 实现，不再作为缺失 Core 能力；现有拖拽、resize、自动显隐与 cursor 回归继续保留，尚需完整主题与 Canvas 视觉验收 | K02/K04/H04 |
| UMLClassDiagram | owner-aware 端点标签和重新测量已实现；类标题/成员字体、图标、StickyNote 的 DogEar 边框与窄宽度换行仍需视觉对等验收 | K02/K03/K05/H04 |
| TreeExample | 19 节点、增删、样式/间距、选中渐变和非动画折叠已移植；动画中的 branch/line geometry、viewport 跟踪、取消与释放未闭合 | K06/K07/H04 |
| ZoomExample | 原始 SVG 已加载；缩放/DPI 改变时仍使用固有尺寸的栅格，需保留矢量来源并按资源版本重栅格化 | K08/H01/H02 |
| ThumbnailExample | 尚缺单一可变 Figure 树的实时次级视图、等比缩放、source/view 身份、资源版本、damage 与释放协议 | K07/H02/H04 |
| InheritBackgroundExample | 尚缺宿主背景输入与透明 Canvas 合成；需对照 opaque/transparent root，验证背景移动、DPI、alpha 与 repair | K07/H01/H02/H04 |
| ShapeStylesExample | 尚缺 per-shape AA ON/OFF/DEFAULT 的 Graphics/IR/backend 完整链路；OFF 必须有像素证据，结构化拒绝不能视为完成 | K08/H02/H04 |

## 本批继续推进已有契约

Grid 的容器原来固定为 790 × 720。Host 更新逻辑视口后，其可用布局空间仍不变。
现在用普通 `MarginBorder` 保留左侧工具区，用 `FillLayout` 将第一个子节点铺满剩余
空间；通用坐标、布局失效与 damage 均由 Core 处理。
此修复用于 Native 面板入口，回归直接消费同一入口，不增加示例私有 resize 事件链。
旧的 Figure mock 控制面板只保留原有测试坐标，不作为窗口 resize 的验证入口。

回归使用真实官方 Shape，先设置跨两列且水平 grab/fill 的 GridData，再依次缩小、
放大、恢复窗口，检查 Canvas bounds、Shape 的宽度/行距和恢复后的 geometry。
Toolbar 回归依次切换 horizontal、stretch、vertical，缩小主轴后恢复，检查已提交
Shape geometry，不只检查控制器布尔值。

本批不将上述入口改为 complete；原始完整验收边界继续保留。
执行结果见 [本批证据](../../../verification/evidence/official-examples-followup-2026-10-23/implementation.md)。

## 新公共契约进入评审

下一批 K07 的 [共享视图与背景设计](../../design/architecture/shared-scene-view-background-proposal.md)
从上游 Thumbnail 的 source/update/deactivate 契约和 InheritBackground 的宿主背景来源
推导。该文档为 proposal，尚未实现，不覆盖现有规范。
批准后先落地不可变源快照、资源闭包和消费者生命周期，再接入 Thumbnail，
随后接入宿主背景与透明 Canvas，分别提供外部消费者和 Native 证据。

K06 动画、K08 AA/SVG 重栅格化与混合字体段的通用文字装饰继续保留。
Path 已消费单字体的下划线指标，不能据此关闭混合字体装饰增量。
依赖顺序和状态统一回写 P2 backlog。
