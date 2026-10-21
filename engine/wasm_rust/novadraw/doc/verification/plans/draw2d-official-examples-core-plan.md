# 官方示例 Core 实施计划

类型：`verification`

日期：2026-10-19

状态：执行中；按用户最新要求收回到 Canvas / LightweightSystem 范围

## 目标与边界

实现全部 25 个官方示例中 Canvas 内的 Draw2D 能力，包括进行中、未移植和阻塞入口。
官方源码只作为只读语义参考；本项目不编写、修改、编译或运行 Java 验证代码。
不建设 SWT 原生控件的 Novadraw 替代体系，不以原生 Text/Combo/Scale/Group、
Shell pack 或打印对话框的复刻作为引擎毕业条件。

Canvas 内由 Draw2D Figure 实现的 Label、ScrollBar、ScrollPane、FlowPage、
MultiLineLabel、Connection、布局和边框仍在范围内。Canvas 外的控制面板可以
使用宿主已有控件或临时示例工具，工具只驱动场景参数；临时面板不定义 Core API，
也不作为原生控件对等证据。窗口、字体、DPI、输入、surface 和背景源只保留
LightweightSystem 所需的宿主适配，不把整个 SWT 产品栈纳入项目。

Core 负责 Figure 树、几何、测量/布局、输入分发、资源、连接和稳定更新；
扩展实现负责可替换算法与 Draw2D Figure 组件；backend/platform 负责栅格与宿主服务。
示例控制器持有选中目标、配对关系和临时高亮/追加状态。不新增 GEF/Editor 依赖。
当前执行队列只在 [P2 backlog](../../roadmap/p2-delta-backlog.md) 维护。
Grid、Flow、Toolbar 和 Tree 的宿主控制面板接入与验证见
[egui 控制面板](draw2d-egui-panel.md)。

## 本次撤销与保留

撤销本轮新增的 Java 探针、运行脚本、Java 生成基线、反射菜单验证和上游运行门禁。
撤销 SWT 原生控件桥接、原生面板完整复刻和真实打印作业作为本计划目标。
既有第三方参考源码不修改，也不删除；历史 Git 提交可追溯，工作树不再保留这些探针。

保留符合 Canvas 语义的 Core 修复：完整子树/回执、资源准入、contents 替换、
Connection binding、Grid span/grab、measurement hints、Flow/Toolbar 轴向测量、
整数分配和布局失效。验证改为 Rust 外部契约、Canvas 场景、Render IR 与受影响平台
运行测试。移除 Java 依赖后，不再用此前 Java 基线组数作为现行验收结论。

## 分批能力与退出条件

K/H 编号沿用已有任务身份，收窄其定义；不创建新 M/G 编号。
每批按只读源码契约 → 定向实现核对 → Rust 失败反例 → 修复 → 消费者 → 验证推进。
新重大公共契约继续先做 ADR 评审。内置/外部能力共用相同公开扩展模型。

| 步骤 | Canvas 能力与消费者 | 退出条件 |
|---|---|---|
| K00 | 25 入口的 Figure/helper/resource 与可达行为清单 | 本地清单与 catalog 一致；区分 Draw2D Figure 和宿主控件，不要求 Java 运行 |
| K01 | 动态 subtree、回执、资源、scene replacement、Connection binding、style/cursor | 拒绝不发布虚假身份；FIFO、重入、panic、释放有 Rust 契约；Tree/Grid/连接消费 |
| K02 | Grid/Flow/Toolbar、tree、border、measurement hints、坐标与 damage | Rust 验证 hints、span/grab、size/reset、insets、换行、旧新 damage；不改受保护递归主循环 |
| K03 | owner-aware Locator 与官方 visibility 路由扩展 | ConnectionAndAnchor/UML 的 child measurement 与四象限；Obstruction 的路径/障碍/配对生命周期 |
| K04 | Draw2D Figure 组件的 ScrollBar 五 children、range、repeat、取消 | Zoom/Path/ScrollPane 的可替换 Figure 组件和共享 range；SWT Scale/Text/Combo 不纳入 |
| K05 | styled/nested flow、fragment、block 方向、FlowBorder、真实 inline Figure | 六个 text 入口和 Path/UML 的结构、baseline、几何、mutation、hit/selection 与绘制 |
| K06 | Directed/Compound 算法、tree contour 与 presentation 动画 | Rust fixture 输入、算法不变量、geometry 输出及 Figure 投影；不复制 Java 静态状态或反射菜单 |
| K07 | live snapshot、range/source/view、背景输入与资源 epoch | Thumbnail 实时比例投影、Bidi shared range、透明 Canvas 背景输入、释放和失效 |
| K08 | Graphics/IR/backend 的 AA、hairline、低 miter、dash/alpha、SVG、文字装饰 | Canvas 表达能力或明确受检错误；结构化拒绝不等于功能完成，缺能力仍要推进实现 |
| K09 | PrintExample 的 Canvas Figure 树与通用图形输出消费 | 图文、连接、资源和 transform/clip 可输出；Printer/Dialog/job 不属于本计划 |
| H01 | LightweightSystem 宿主适配 | Canvas resize、DPI/font、平台输入、focus、surface/窗口生命周期；不建设原生控件库 |
| H02 | backend 栅格、资源及合成 | Render IR 完整消费、surface repair、背景/alpha、失效和 late completion |
| H03 | 原真实打印平台任务 | 从本计划退出；未来有独立产品需求再规划 |
| H04 | Canvas 结构、行为、资源与视觉验收 | 逐入口记录通过范围，不以测试数代替全部 Canvas 功能完成 |

## 当前进度与下一批

K01 已完成多个通用切片并接入 Tree、Grid 与 ConnectionAndAnchor，仍有后续消费者
依赖 K03/K05/K06。K02 已修复 Grid span/grab、最小测量和整数居中；
当前 Flow/Toolbar 的 hint 传递、二次测量、整数收缩、overflow 对齐与 Grid hints
已修复，正在改用独立 Rust 契约验证。K02 未完成，真实文字换行、size/reset、
owner border 和复杂布局边界继续推进。
K03 首批修复普通 Locator child 使用旧 bounds 尺寸的问题，改为统一受检 preferred
measurement；非法测量报告 ChildMeasurementFailed，拒绝路由发布并可恢复，
外部 Figure 与内置图元共用路径。owner quadrant、child-size 输入及 EndpointLocator
开放 placement 契约尚未完成，不能将本切片视为 K03 整体关闭。

2026-10-21 补齐 Locator child 测量失效：此前只修改 preferred size 时，端点几何
不变，路由不会重新提交，导致连接标签停留在旧尺寸。现在绑定追踪普通 child 的
layout generation，尺寸覆盖、重置与文字测量变化会触发同一受检路由批次；
提交后确认新 generation，避免定位本身导致反复路由。Decoration 通过已有 capability
识别，其边界来自 route orientation，不参与 preferred 测量失效。
Rust 契约覆盖尺寸变化、重置、不变值、测量失败后的自动恢复与失败稳定；
UMLClassDiagram 覆盖只修改连接文字且类和路由不动时的重新测量与居中。

接下来先完成当前 Rust 布局回归，再推进 K03 Locator 与 Obstruction 解阻，
随后按依赖推进 K04/K05、K06、K07/K08。K00/H04 随每批维护，不等待全量先验收。
每个入口有 [Canvas 收口清单](../../../examples/native/draw2d-official-examples/completion-contracts.tsv)，
只登记 Canvas 内的结构、行为和失败/资源需求。

## 全计划完成判定

25 个入口都具备独立 Canvas 实现；Canvas 功能清单没有未移植或阻塞项；
Figure composition、算法、输入、测量、绘制、资源和退出有 Rust 验证或场景证据。
原生控制面板是否等同 SWT 不影响 Canvas 完成判定。声明支持的平台只按实际验证
范围报告，不把尚未验证的平台标为通过。

完整源码需求见 [全入口矩阵](draw2d-official-examples-remaining-contracts.md)。
矩阵中的 SWT 描述只说明上游环境，不自动构成本项目建设任务；与本计划冲突时，
以用户最新 Canvas 范围及本计划为准。
