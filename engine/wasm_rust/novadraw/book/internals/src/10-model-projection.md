# 10. 模型、EditPart 与确定性投影

图形编辑器不能把 Figure 树当作业务模型。Figure 是视图投影，业务事实必须能够脱离
当前 Viewer 和 Runtime 持久存在。

本章解释：

- 业务模型、ModelAdapter 和 ModelRevision；
- EditPart 作为控制器而不是第二份模型；
- containment、connection 与 visual ownership；
- ModelId、EditPartId 和 FigureId 的映射；
- 稳定快照和 revision 连续性；
- 投影的 plan、validate、commit 与 publication。

核心结论：

> Viewer 可以被销毁和重建，而业务事实、命令语义和最终投影结果必须保持不变。

本章应通过节点重排、删除与连接重建说明确定性投影，并讨论重复模型身份、缺失端点和
读取期间版本漂移应如何被拒绝。
