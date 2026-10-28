# 官方示例剩余工作

类型：`verification`

日期：2026-10-23

范围：25 个入口的 Canvas / LightweightSystem 能力。执行队列仍只由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

## 还有 8 个入口未收口

2026-10-28 当前 catalog 为 17 complete、8 in_progress，25 个入口均可运行。
下表将待验证的消费者行为和确实缺失的 Core 能力分开记录。
完成判据取自 [Canvas 收口清单](../../../examples/native/draw2d-official-examples/completion-contracts.tsv)，
不以可运行、测试数或一幅截图代替完整验收。

| 入口 | 剩余工作 | 能力批次 |
|---|---|---|
| FlowLayoutExample | 形状与文字消费者、真实面板、收缩恢复及 retained-partial 擦除已验证；连续窗口事件、surface present 与跨平台像素仍待验收 | K02/H04 |
| GridLayoutExample | span/grab/hints/indent、分配列宽后的文字重测、真实面板及 retained-partial 擦除已验证；连续窗口事件、surface present 与跨平台像素仍待验收 | K01/K02/H04 |
| ToolbarLayoutExample | 形状与文字 minimum、方向/stretch/恢复、真实面板及 retained-partial 擦除已验证；连续窗口事件、surface present 与跨平台像素仍待验收 | K02/H04 |
| PrintExample | Canvas 组合、原始 ICO 与通用输出已验证；宿主已采用可用系统字族并按 point 映射逻辑像素，macOS 使用 Helvetica/Times New Roman，Perpetua 缺失时明确回退；其他平台字体与像素仍待验收 | K09/H01/H04 |
| ScrollPaneExample | 五部件 ScrollBar 已由 ADR-041 实现，不再作为缺失 Core 能力；现有拖拽、resize、自动显隐与 cursor 回归继续保留，尚需完整主题与 Canvas 视觉验收 | K02/K04/H04 |
| ZoomExample | 保留 SVG 源、缩放/DPI 密度选择、资源 revision 替换及 Native GPU 像素恢复已验证；宿主已采用可用 Arial bold 12 point；跨显示器系统 DPI、其他平台字体与完整视觉对等仍待验收 | K08/H01/H02/H04 |
| ShapeStylesExample | 逐操作 AA、低 miter 与 Cocoa 零宽 profile 已实现并通过 GPU 矩阵；上游原生栅格、系统颜色和跨平台像素仍待验收，通用设备 hairline 是独立可选增量 | K08/H02/H04 |
| BidiBlockExample | 继承镜像、段落方向、相对/绝对对齐、caret/selection 和共享纵向 range 已实现；上游栅格与跨平台呈现仍待验收 | K05/K07/H04 |

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
执行结果见 [本批证据](../evidence/official-examples-followup-2026-10-23/implementation.md)。

K07 后返回布局主线的新增修复与核验见
[K02 消费者记录](../reviews/k02-layout-consumers-evidence-2026-10-23.md)。
Flow/Toolbar 已使用同一 MarginBorder/FillLayout 机制；右侧工具面板跟随窗口边缘。
旧实施记录属于先前批次证据，其当时 catalog 和 K07 草案状态保留原文。

## K07 已完成批准切片

K07 的 [共享视图与背景设计](../../design/architecture/shared-scene-view-background-proposal.md)
从上游 Thumbnail 的 source/update/deactivate 契约和 InheritBackground 的宿主背景来源
推导，修订 3 已获用户批准。不可变源捕获、结构准入、资源导入和只读视图已实现；
Thumbnail 的真实拖拽、等比 overview resize 与 Native 图像已验收，见
[K07-4 证据](../reviews/k07-4-thumbnail-evidence-2026-10-23.md)。
宿主背景与透明 Canvas 也已完成独立消费者和 Native 验收，见
[K07-6 证据](../reviews/k07-6-inherit-background-evidence-2026-10-23.md)。
Web 背景像素、Windows/Linux runner 和跨显示器系统 DPI 验收仍未完成。

K08 SVG 栅格密度切片已实现，见
[SVG 证据](../reviews/k08-svg-density-evidence-2026-10-23.md)。
K06 Tree 动画已按 ADR-046 完成，契约与 Native retained-partial 证据见
[实施复核](../reviews/tree-layout-presentation-implementation-2026-10-26.md)。
K08 AA 与混合字体段的通用文字装饰继续保留。
Path 已消费单字体的下划线指标，不能据此关闭混合字体装饰增量。
依赖顺序和状态统一回写 P2 backlog。
