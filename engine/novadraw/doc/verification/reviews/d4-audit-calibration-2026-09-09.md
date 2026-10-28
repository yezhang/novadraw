# D4.0 长期架构审计校准

类型：`verification`

日期：2026-09-09

## 结论

以 `4a559c3` 后的当前 HEAD 复核长期架构审计。D3.3/D3.4 已完成，但 A01-A08
均未被 listener 增量改变，证据继续成立。它们已进入唯一迁移计划的 D4，不另建
第二份 roadmap。

## 当前 HEAD 复现

复跑 `target/architecture-review-probe`：

```text
identity: equal_ids=true, foreign_mutation=true, second_x=12
remove: attached=false, node_still_stored=true, drops=0
remove: drops_after_runtime_drop=1
resource: status=Ready { revision: 2 }, delta_add=1, delta_remove_same_id=true
backend_replacement: full_redraw_resource_count=0
routing: after_frame=Dirty { revision: 3 }, old_bounds_unchanged=true, pending=false
label: final_width=500, first_glyphs=3, second_glyphs=11, pending_after_first=false
```

源码复核同时确认：

- `prepare_submission` 不消费 `dirty_connections`；
- Label refresh 发生在 parent layout validation 前；
- ResourceDelta 仍分离 `added` 与 `removed`，Vello 先 add 后 remove；
- remove 只 detach，SlotMap/UUID/Figure 仍保留；
- FigureId 没有 Runtime namespace；
- TextLayout 没有外部可用的非空构造入口；
- full redraw 不包含 Ready resource snapshot；
- 递归 renderer 每个节点重新回溯 ancestor chain 解析 style。

## D4 映射

| Finding | 状态 | 执行批次 |
|---|---|---|
| A01 connection dirty 不自动 route | confirmed | D4.1 |
| A02 text/layout 首帧不收敛 | confirmed | D4.1 |
| A03 ResourceDelta 同 ID 顺序错误 | confirmed | D4.2 |
| A04 detach 不释放节点 | confirmed | D4.3 |
| A05 FigureId 跨 Runtime 冲突 | confirmed | D4.3 |
| A06 外部 TextLayoutEngine 无法构造真实结果 | confirmed | D4.4 |
| A07 backend 重建不重发 Ready 资源 | confirmed | D4.2 |
| A08 deep-tree style O(N²) | confirmed | D4.5 |

A09 模块拆分、A10 完整输入协议与 A11 全面错误模型不整体纳入 D4；只实现 D4
当前批次明确依赖的最小部分，其余保留为后续 architecture delta。

## 下一门禁

D4.1 在实现前必须先接受派生状态固定阶段与收敛预算契约，避免用“多跑一帧”或应用
手工 resolve 掩盖因果顺序问题。

## 后续关闭状态

2026-09-10：A01-A08 已分别由 D4.1-D4.5 关闭，D4.6 最终门禁通过。原 probe 输出
继续作为修复前证据保留，关闭矩阵见
[`adr014-d4.6-completion-2026-09-10.md`](adr014-d4.6-completion-2026-09-10.md)。
