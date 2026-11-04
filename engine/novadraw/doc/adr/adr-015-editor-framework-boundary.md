# ADR-015：Core、Editor 与应用模型边界

类型：`architecture-decision`

能力归属：Editor 管理编辑；Core 提供图形基础；应用拥有业务模型与持久化。

状态：已接受，2026-09-13；2026-09-28 产品边界修订保持有效。
本次压缩归并仅调整知识表达，不新增架构授权；实现状态以 roadmap／verification 为准。

## 问题

编辑选择、模型命令和 undo 历史的生命周期不同于 Figure 场景；
把它们纳入 Core 会限制普通绘图应用及跨 Runtime 重建。

## 决策

Editor 是只依赖 Core 的独立可选框架。模型身份、EditPart 身份与 Figure 身份分域，
显式 registry 连接它们。Command 保存模型身份和数据，不保存活 Part／Figure 身份。
Viewer 拥有 selection 与工具交互，反馈通过 Core Figure 投影。

应用定义 document schema、serializer、业务模型和产品 UI；引擎提供重建和注入合同。
GEF Classic 作为语义参考，不承诺逐类翻译或 Java API 兼容。
Figure-native 输入与 Editor Tool 按明确的消费／capture 结果仲裁。

## 替代与代价

把编辑状态置于 Runtime 会污染 Core 所有权；历史保存活 Figure 会阻碍重建。
预先为每个 Editor 模块拆包增加发布负担，因此按真实独立消费需求拆分。
代价是显式维护模型／Part／Figure 映射、增量投影和输入仲裁；
undo 后重建不恢复旧运行时身份。

## 关联设计

[Editor 架构](../design/editor/architecture.md)、
[Viewer 会话](../design/editor/viewer-edit-session-and-drive.md)、
[增量失败隔离](../design/editor/incremental-failure-isolation.md)；
执行范围见 [Editor roadmap](../roadmap/editor/00-index.md)。

原批准范围和修订依据见[原稿 015](../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-015-editor-framework-boundary.md.txt)；
其他原编号与当前入口见[归并记录](../archive/adr-consolidation-2026-11-03.md)。
