# ADR-003：Runtime 所有权与二维坐标边界

类型：`architecture-decision`

能力归属：Core Runtime、树与二维坐标；platform 仅适配宿主。

状态：已接受，2026-08-30；2026-09-10 生命周期补充保持有效。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

Java 对象模型、普遍共享锁和树／交互混合所有权，不能自然表达 Rust 中的
提交权威及 Native／Web 的单线程场景。二维坐标也需要独立于未来 3D 扩展。

## 决策

Runtime 是单场景最小完整可变所有权单元；树拓扑、交互、资源和更新各有明确职责。
核心默认单线程独占，不强制所有扩展 Send／Sync，不使用通用全局状态或共享锁。
回调通过受限查询计算 effects，释放借用后由 Runtime 提交。

bounds 存在 parent content domain，父子关系通过二维 Affine2D edge transform 表达；
父级移动不改写后代 bounds。根 Figure 不拥有平台对象。
只有真实替换边界使用 trait；未来真实 3D 使用独立 Scene3D，不改变二维基础词汇。

## 替代与代价

扩大万能 FigureGraph／IFigure 会耦合不同生命周期；将所有对象抽成 trait
增加不必要的动态分发和借用成本。全面 4×4 矩阵不能补齐 3D 场景语义。
该选择承担 parent-local 与 Draw2D 内部存储差异及 effects 准备成本；
不预先冻结缺少第二消费者的二进制 DisplayList ABI。

## 关联设计

[所有权与模块](../design/architecture/runtime-ownership-and-module-boundaries.md)、
[坐标](../design/coordinates/coordinate-system.md)、
[阶段权限](../design/architecture/context-phase-authority.md)。

原批准范围和修订依据见[原稿 003](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-003-rust-runtime-and-geometry-boundaries.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
