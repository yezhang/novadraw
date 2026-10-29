# Editor／GEF 框架能力补充方案

类型：`proposal`

能力归属：Editor（`novadraw-editor`）；本文“框架能力”指 GEF 编辑协议。

日期：2026-10-29。用户授权开始补充 `novadraw-editor` 的 GEF 框架能力。
本方案将能力分析转为分批交付范围；首批架构取舍已获批准。
执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
首轮评审九项建议及修订 1 复核的三项建议均采纳，
ADR-055 修订 2 已通过复核，用户批准实施。公共容器协议、移动和创建/drop 服务
已接入；无窗口契约与独立消费者已验证，平台输入验收仍按矩阵保留未完成。

## 能力归属与依赖

本文各批次均归 Editor。模型编辑、选择、Tool／Policy、容器编辑、吸附、
导航与多视图协调由 Editor 提供；应用持有业务模型、Command 实现和持久化。
Core 提供现有 Figure、命中、坐标、布局、Runtime 和绘制基础协议。
平台／宿主负责原生输入、DPI、capture 与快捷键映射。

使用 Core API 是依赖关系，不将这些基础能力重新计为 GEF 迁移成果。
若发现必须新增或修订 Core 契约，单列 Core delta 与验收，不扩张本 Editor 批次。
S08 输入不变量属于 Core 整改；S09 是跨包服务整改，均与本文的 Editor 交付分开。
当前 Editor 主线为 E03 剩余验收、E04 开放吸附设计及其后续 GEF 能力。

## 目标与基线

已有 ModelAdapter、CommandStack、EditPart/Viewer、开放 Tool/Request/Policy/Handle、
连接编辑、视口、直接文本编辑、模板创建和节点完全包含框选。
这些基线继续按 [Editor 架构](architecture.md)和
[GEF 语义账本](../../parity/gef/api-coverage.md)解释。

本轮补足复杂图形编辑器的通用交互。每批必须由独立消费者证明扩展性，
不能以 Shapes 的应用实现代替框架能力验收。

## 分批范围与依赖

| 顺序 | 能力归属 | 范围 | 主要交付 | 依赖与边界 |
|---:|---|---|---|---|
| 1 | Editor | 容器编辑 | 祖孙选择去重、排除源子树的 targeting、父坐标映射、换父、有序插入、嵌套创建 | 首批决策见 ADR-055；模型事务仍由应用 Command 提供 |
| 2 | Editor | 吸附 | Grid、Geometry、Guide 的开放策略组合；预览与提交共享修正 | 建立在首批坐标语义上；阈值为逻辑像素，参数由消费者配置 |
| 3 | Editor | 键盘导航与编辑 | 空间/层级/连接导航、移动与缩放、显露目标 | Figure 消费与 Tool 仲裁一致；快捷键绑定归宿主 |
| 4 | Editor | 多视图协调 | ModelId 选择映射、防回环、通知分发、共用历史和独立视口 | 先定义共享 adapter、事件游标与 gesture owner，不复制运行时 ID |
| 5 | Editor | 框选策略 | 相交/包含节点、连接和关联连接的选择 | 同一开放策略模型；精确几何与包围盒候选分开 |
| 6 | Editor | 可选编辑支持 | XY/Ordered/Flow policy、Handle 编排、通用对齐与等尺寸操作 | 沿用开放协议；应用提供模型命令和布局约束 |

这张表说明建议顺序，不批准后续批次的接口。每批设计、执行、验证完成后，
再进入下一批，不一次引入所有服务。

## 首批消费者

使用独立测试模型表达两层容器、不同 content pane、节点和连接。
验证从容器 A 拖入 B、拖出到根、同容器重排、嵌套创建，以及撤销重做。
应用 schema、文件存储和 GUI 外壳不参与 Editor 契约。
两个消费者以不同 Request/Payload/Plan 关联类型使用同一个容器 Tool，
通过通用受检 host 聚合和统一资格校验。嵌套创建使用独立创建计划，
共用目标查询与坐标服务。混合父批次与完整选择/focus 恢复属于首批退出范围。
另以同 Viewer 的 XY→Ordered 场景验证两侧注册同一个活动 P 的跨布局手势；
这与两个不同关联协议的独立消费者分开验收，不承诺自动切换协议。
多选用唯一锚点计算批次 surface 平移，各 source 再分别映射到目标父域。

Shapes 当前是平面模型，继续作为原行为回归。嵌套容器消费不能通过修改
Shapes 的业务语义来凑验收分母。

## 性能与产物

首批先测量操作集合构建、目标查询、坐标转换、反馈更新和模型提交后的同步成本。
已有同步在收到通知后抓取完整模型快照，并遍历 containment 与 connections；
本批不宣称将其改为局部同步。记录该成本，后续以单独证据决定是否优化。

新增协议不引入 GUI、序列化、字体或图片依赖。以相同 release 配置的消费者比较
启用/不启用首批能力的最终 Native 产物；Web 编译验证平台无关边界。
Web 体积、传输和加载成本需要同配置实际产物，不能用 Native 差值推算。

## 对标边界

只参考本地 GEF Classic `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
的 `org.eclipse.gef` 与 `org.eclipse.draw2d`，Java 仅作只读参考。

复制粘贴、属性描述、Palette/Outline 适配和系统 DnD 可作为后续可选支持包。
Eclipse Workbench、JFace 和 TreeViewer 不进入 Editor。
打印/导出与平台 AT 沿用既有 P2-O01/P2-A01，避免重复队列。

首批契约见 [ADR-055](../../adr/adr-055-container-editing-protocol.md)，
退出场景见 [验证矩阵](../../verification/plans/editor-container-editing.md)。
