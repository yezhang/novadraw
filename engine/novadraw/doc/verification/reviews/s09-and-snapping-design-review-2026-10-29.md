# S09 与开放吸附设计复核

类型：`architecture-review`

日期：2026-10-29。本次为设计自审，未使用独立 reviewer，
没有实现或平台验收结果。两项设计均待用户评审后进入 Execute。

## S09

审阅 [ADR-057](../../adr/adr-057-service-failure-and-inspector-lifecycle.md)与
[矩阵](../plans/p2-s09-service-failure.md)。
目标从 ADR-010／014／016／033／038 及现有审计提取，未扫描 Rust 现状。

| 检查 | 结论 |
|---|---|
| fault 与未变化 | 已明确先检查 owner 健康，再判断同值 |
| Inspector 自身故障 | poison 返回结构化错误，单独保存首个 fault 并 self-remove |
| 一般 observer panic | 仍使 Runtime faulted，不套用本地缓冲故障隔离 |
| 订阅所有权 | 宿主保存 namespaced token，显式 detach；不声称 Drop 即时注销 |
| 注册双入口 | 只删除同语义 panic alias，保留真正不同语义的入口 |
| extension source | owned source 与领域分类分开，文本／非文本 panic 单独表达 |
| 验证与成本 | 公开生命周期和私有注入分开；成功路径、故障路径和可达产物分别测量 |

实施前仍需盘点 S03 后的真实方法、scope 和注销返回值。
detach 的 foreign token 结构化错误与 fault 后清理是本草案的显式增量；
若需要调整 Observations 的底层 remove 合同，应列入接受后的入口清单，
不能同时改变其他 remove 的幂等／缺失语义。
本批不恢复或重放丢失的 observer journal。

## 开放吸附

审阅 [ADR-058](../../adr/adr-058-open-editor-snapping.md)、
[专题](../../design/editor/open-snapping.md)与[矩阵](../plans/editor-open-snapping.md)。
只读参考 GEF 的 SnapToHelper、Grid、Geometry、Guides 和 DragEditPartsTracker。

| 检查 | 结论 |
|---|---|
| 内置与扩展 | 相同 typed registry、候选、仲裁、反馈和错误路径 |
| 多选与换父 | 唯一锚点、单一 surface 修正，成员各自映射父域 |
| targeting | 先按未吸附输入解析目标，吸附不能把目标反复切换 |
| 阈值与变换 | 逻辑像素比较；不用单一 zoom 标量替代一般变换 |
| 版本失效 | model／投影／坐标／target／策略共同核对，释放前必要时重算 |
| resize／耦合 | 固定边、比例和最小尺寸进入准入，耦合候选不可拆分 |
| 预览与提交 | 同一受检结果；布局改写使旧结果失效 |
| 性能 | O(N log N) 建索引、O(log N + K) 查询是目标，须用原始计数验证 |

GEF 的 helper 可假定手势期间其他对象静止；本草案明确支持版本变化后重建，
不沿用该静态假设。整数 right−1／bottom−1 属于原像素模型，连续几何不复制。
Grid 的始终量化模式与阈值捕获模式显式分开；参考线不代表业务 guide attachment 已保存。

## E03 剩余验收

E03 的 change_bounds／reparent 状态已在 GEF 账本中更正为
“已实现，无窗口契约通过，故障组合与平台验收未全部完成”。
已有提前退休反馈的清理拒绝不能替代真实 Core revision 耗尽。
公开 API 没有安全的耗尽注入方式，继续保留两种 Tool 的端到端缺口，
不增加测试后门或使用另一个拒绝场景勾选。

下一次 E03 采样把一次完整提交拆成应用 Command 模型复制、
Viewer 投影退休／重建、反馈准备／清理、Runtime 稳定化四段。
沿用原 16／64 源父、32／2048 后代 fixture，
每段记录调用量、节点量、分配与 P50／P95，源码和 allocator 原始字节另存。
先完成 Native／Web capture、DPI、取消与静止指针视口变化，
再决定是否需要新的增量投影设计；不把模型全量同步问题并入吸附实现。

执行仍遵循单链：先关闭 S08，再实施已接受的 S09；
E03 缺失验收保留，吸附设计不构成新增实现授权。

成稿已按好好说话技能分别回读事实、表达和句子节奏；没有修改协议或验收范围。
八份本轮文档的编号标题、对比句模板、元话语和中文正文半角标点均为零命中，
见[扫描记录](../evidence/p2-s08-s09-chinese-review-2026-10-29.log)。
该检查只证明表达复核，不替代架构接受或实施验证。
